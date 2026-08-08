//! Deck do usuario: cards por lema, com os contextos onde a palavra apareceu.
//!
//! O banco e o schema vivem em [`crate::db`]; aqui ficam as operacoes.
//!
//! # Um card por lema
//!
//! Decisao fechada no CLAUDE.md. Salvar "ran" numa tela e "running" em outra
//! nao cria dois cards: cria um card de "run" com dois contextos. E o que evita
//! o deck virar uma lista de conjugacoes — e o que faz a revisao ensinar a
//! palavra, nao a flexao.
//!
//! Por isso [`save_card`] nao tem par "criar/atualizar": salvar e sempre a
//! mesma operacao, e o `created` da resposta so diz o que aconteceu, para a UI
//! escolher entre "salvo" e "contexto adicionado".
//!
//! # O screenshot
//!
//! Cada contexto novo leva um recorte da frase na tela ([`crate::media`]). Ele
//! e gravado depois do insert, porque o nome do arquivo sai do id do contexto,
//! e nunca derruba o salvamento: card sem imagem ainda ensina a palavra.
//!
//! # Quem calcula o agendamento
//!
//! Ninguem, aqui. Regra inviolavel #4: os campos `fsrs_*` chegam prontos do
//! wrapper em `src/shared/srs/` e este modulo so os grava. Um card novo vem com
//! o estado zerado do `createEmptyCard`; anexar contexto a um card existente
//! **nao** toca no agendamento dele, senao reencontrar a palavra num jogo
//! bagunçaria a fila de revisao.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::db;
use crate::error::{Error, Result};

/// Estado do FSRS como o wrapper da UI o entrega. O core nao interpreta nenhum
/// destes campos — so persiste.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsrsFields {
    pub due: String,
    pub stability: f64,
    pub difficulty: f64,
    pub state: String,
    pub reps: u32,
    pub lapses: u32,
    #[serde(default)]
    pub scheduled_days: i64,
    #[serde(default)]
    pub learning_steps: i64,
    #[serde(default)]
    pub last_review: Option<String>,
}

/// Payload de "salvar palavra no deck" vindo do overlay.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCardInput {
    pub lemma: String,
    /// Forma exatamente como apareceu na tela.
    pub form: String,
    pub sentence_en: String,
    pub sentence_pt: Option<String>,
    pub game_name: Option<String>,
    /// So e usado quando o card ainda nao existe.
    pub fsrs: FsrsFields,
    /// Consulta de onde a frase saiu, para recortar o screenshot dela. Ausente
    /// quando o salvamento nao veio de um lookup — o card so fica sem imagem.
    #[serde(default)]
    pub lookup_id: Option<u64>,
    /// Linha da consulta em que a frase esta.
    #[serde(default)]
    pub line_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSummary {
    pub id: i64,
    pub lemma: String,
    pub contexts: u32,
    /// `true` quando este salvamento criou o card; `false` quando so anexou um
    /// contexto a um card que ja existia.
    pub created: bool,
}

/// O que aconteceu num salvamento, do ponto de vista de quem chamou.
#[derive(Debug, Clone, PartialEq)]
pub struct Salvo {
    pub resumo: CardSummary,
    /// Id do contexto inserido agora, ou `None` se a frase ja estava no card.
    /// E o que diz se vale recortar um screenshot — repetir o clique nao pode
    /// gerar arquivo novo.
    pub contexto_novo: Option<i64>,
}

/// Cria o card (ou anexa um contexto novo se o lema ja existe).
///
/// Salvar a mesma frase duas vezes no mesmo card e engano de clique, nao
/// contexto novo: o indice unico de `contexts` absorve a repeticao em silencio.
pub fn save_card(conexao: &Connection, entrada: &SaveCardInput) -> Result<Salvo> {
    let lemma = entrada.lemma.trim();
    if lemma.is_empty() {
        return Err(Error::Deck("card sem lema".into()));
    }
    let sentence_en = entrada.sentence_en.trim();
    if sentence_en.is_empty() {
        return Err(Error::Deck("card sem frase de contexto".into()));
    }

    let existente: Option<i64> = conexao
        .query_row("SELECT id FROM cards WHERE lemma = ?1", [lemma], |linha| {
            linha.get(0)
        })
        .optional()?;

    let criado = existente.is_none();
    let card_id = match existente {
        Some(id) => id,
        None => {
            conexao.execute(
                "INSERT INTO cards (
                     lemma, created_at, suspended,
                     fsrs_due, fsrs_stability, fsrs_difficulty, fsrs_state,
                     fsrs_reps, fsrs_lapses, fsrs_scheduled_days,
                     fsrs_learning_steps, fsrs_last_review
                 ) VALUES (
                     ?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 0,
                     ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                 )",
                params![
                    lemma,
                    entrada.fsrs.due,
                    entrada.fsrs.stability,
                    entrada.fsrs.difficulty,
                    entrada.fsrs.state,
                    entrada.fsrs.reps,
                    entrada.fsrs.lapses,
                    entrada.fsrs.scheduled_days,
                    entrada.fsrs.learning_steps,
                    entrada.fsrs.last_review,
                ],
            )?;
            conexao.last_insert_rowid()
        }
    };

    let inseridos = conexao.execute(
        "INSERT OR IGNORE INTO contexts (
             card_id, form, sentence_en, sentence_pt, game_name, captured_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        params![
            card_id,
            entrada.form.trim(),
            sentence_en,
            entrada.sentence_pt.as_deref().map(str::trim),
            entrada.game_name.as_deref(),
        ],
    )?;

    Ok(Salvo {
        resumo: CardSummary {
            id: card_id,
            lemma: lemma.to_string(),
            contexts: contar_contextos(conexao, card_id)?,
            created: criado,
        },
        // O `OR IGNORE` faz o insert repetido nao mexer em nada — e por isso
        // que a contagem de linhas, e nao o `last_insert_rowid`, e quem sabe
        // se houve contexto novo.
        contexto_novo: (inseridos == 1).then(|| conexao.last_insert_rowid()),
    })
}

/// Anexa o screenshot ja gravado em disco ao contexto.
///
/// Passo separado do insert porque o nome do arquivo sai do id do contexto: e
/// o que da um nome unico e rastreavel (`media/ctx-000012.webp`) sem inventar
/// um uuid nem arriscar dois cards escreverem no mesmo arquivo.
pub fn set_screenshot(conexao: &Connection, context_id: i64, caminho: &str) -> Result<()> {
    conexao.execute(
        "UPDATE contexts SET screenshot_path = ?2 WHERE id = ?1",
        params![context_id, caminho],
    )?;
    Ok(())
}

/// O card de um lema, se ele ja estiver no deck.
///
/// E o que deixa o botao da overlay dizer "salvo" antes de o usuario clicar —
/// e, mais adiante, o que colore as palavras ja salvas nos destaques (F2).
pub fn card_status(conexao: &Connection, lemma: &str) -> Result<Option<CardSummary>> {
    let lemma = lemma.trim();
    if lemma.is_empty() {
        return Ok(None);
    }
    let card: Option<(i64, String)> = conexao
        .query_row(
            "SELECT id, lemma FROM cards WHERE lemma = ?1",
            [lemma],
            |linha| Ok((linha.get(0)?, linha.get(1)?)),
        )
        .optional()?;

    match card {
        None => Ok(None),
        Some((id, lemma)) => Ok(Some(CardSummary {
            id,
            lemma,
            contexts: contar_contextos(conexao, id)?,
            // Consulta nunca cria nada.
            created: false,
        })),
    }
}

// A fila do dia (cards vencidos + a cota de novos) mora em [`crate::review`]:
// ela precisa do estado completo do FSRS, e nao do `CardSummary` que este
// modulo usa para o overlay.

// ---------------------------------------------------------------------------
// Gestao do deck (F4)
// ---------------------------------------------------------------------------

/// Card como a lista da tela Deck o mostra: o estado do card mais o contexto
/// mais recente, que e o que faz reconhecer a palavra sem abrir o detalhe.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRow {
    pub id: i64,
    pub lemma: String,
    pub created_at: String,
    pub suspended: bool,
    pub fsrs_due: String,
    pub fsrs_state: String,
    pub fsrs_reps: u32,
    pub fsrs_lapses: u32,
    pub contexts: u32,
    pub last_sentence: Option<String>,
    pub last_game: Option<String>,
}

/// Ocorrencia da palavra num jogo.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardContext {
    pub id: i64,
    pub card_id: i64,
    pub form: String,
    pub sentence_en: String,
    pub sentence_pt: Option<String>,
    pub game_name: Option<String>,
    /// Caminho relativo (`media/ctx-000012.webp`), lido por `deck_screenshot`.
    pub screenshot_path: Option<String>,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetail {
    pub card: CardRow,
    pub contexts: Vec<CardContext>,
}

/// Como ordenar a lista.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Ordem {
    /// Ultimos salvos primeiro — o que quase sempre e o que se procura.
    #[default]
    Recentes,
    Alfabetica,
    /// Proximos do vencimento primeiro: a previa da fila de revisao.
    Vencimento,
    /// Mais lapsos primeiro — as palavras que teimam em nao entrar.
    MaisDificeis,
}

impl Ordem {
    /// Trecho de `ORDER BY`. Sai de um enum, e nunca de texto da UI, porque
    /// ordenacao e a unica parte da consulta que nao da para parametrizar.
    fn sql(self) -> &'static str {
        match self {
            // Desempate por id em todas: sem ele, cards salvos no mesmo
            // segundo trocam de lugar entre uma consulta e outra.
            Ordem::Recentes => "c.created_at DESC, c.id DESC",
            Ordem::Alfabetica => "c.lemma ASC, c.id ASC",
            Ordem::Vencimento => "c.fsrs_due ASC, c.id ASC",
            Ordem::MaisDificeis => "c.fsrs_lapses DESC, c.id DESC",
        }
    }
}

/// Filtros da tela Deck. Tudo opcional: o padrao e "o deck inteiro, sem os
/// suspensos".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardQuery {
    /// Busca no lema e nas frases dos contextos.
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub game: Option<String>,
    /// Estado do FSRS (`new`, `learning`, `review`, `relearning`).
    #[serde(default)]
    pub state: Option<String>,
    /// Suspensos entram na lista? Fora dela por padrao: "ja sei" e justamente
    /// o que o usuario nao quer mais ver.
    #[serde(default)]
    pub include_suspended: bool,
    #[serde(default)]
    pub order: Ordem,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// Quantos cards a lista traz quando a UI nao pede um limite.
const LIMITE_PADRAO: u32 = 200;

/// Transforma o texto digitado em padrao de `LIKE`.
///
/// `%` e `_` sao curingas do SQL: sem escapa-los, procurar por "100%" traria o
/// deck inteiro. O `\` fica declarado como escape na propria consulta.
fn padrao_de_busca(bruto: &str) -> String {
    let escapado = bruto
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escapado}%")
}

fn card_row(linha: &rusqlite::Row<'_>) -> rusqlite::Result<CardRow> {
    Ok(CardRow {
        id: linha.get("id")?,
        lemma: linha.get("lemma")?,
        created_at: linha.get("created_at")?,
        suspended: linha.get::<_, i64>("suspended")? != 0,
        fsrs_due: linha.get("fsrs_due")?,
        fsrs_state: linha.get("fsrs_state")?,
        fsrs_reps: linha.get("fsrs_reps")?,
        fsrs_lapses: linha.get("fsrs_lapses")?,
        contexts: linha.get("contexts")?,
        last_sentence: linha.get("last_sentence")?,
        last_game: linha.get("last_game")?,
    })
}

/// A lista da tela Deck.
pub fn list_cards(conexao: &Connection, consulta: &CardQuery) -> Result<Vec<CardRow>> {
    let busca = consulta
        .search
        .as_deref()
        .map(str::trim)
        .filter(|texto| !texto.is_empty())
        .map(padrao_de_busca);
    let jogo = consulta.game.as_deref().filter(|texto| !texto.is_empty());
    let estado = consulta.state.as_deref().filter(|texto| !texto.is_empty());

    // O contexto mais recente entra por subconsulta em vez de `GROUP BY`:
    // agrupar devolveria uma frase qualquer do card, e a que interessa e a
    // ultima vez que a palavra apareceu.
    let sql = format!(
        "SELECT c.id, c.lemma, c.created_at, c.suspended, c.fsrs_due, c.fsrs_state,
                c.fsrs_reps, c.fsrs_lapses,
                (SELECT COUNT(*) FROM contexts n WHERE n.card_id = c.id) AS contexts,
                ultimo.sentence_en AS last_sentence,
                ultimo.game_name   AS last_game
           FROM cards c
           LEFT JOIN contexts ultimo ON ultimo.id = (
                SELECT id FROM contexts u WHERE u.card_id = c.id
                 ORDER BY u.captured_at DESC, u.id DESC LIMIT 1
           )
          WHERE (:incluir_suspensos = 1 OR c.suspended = 0)
            AND (:estado IS NULL OR c.fsrs_state = :estado)
            AND (:jogo IS NULL OR EXISTS (
                    SELECT 1 FROM contexts g
                     WHERE g.card_id = c.id AND g.game_name = :jogo))
            AND (:busca IS NULL
                 OR c.lemma LIKE :busca ESCAPE '\\'
                 OR EXISTS (SELECT 1 FROM contexts s
                             WHERE s.card_id = c.id
                               AND (s.sentence_en LIKE :busca ESCAPE '\\'
                                    OR s.form LIKE :busca ESCAPE '\\')))
          ORDER BY {}
          LIMIT :limite OFFSET :deslocamento",
        consulta.order.sql()
    );

    let mut stmt = conexao.prepare(&sql)?;
    let linhas = stmt.query_map(
        rusqlite::named_params! {
            ":incluir_suspensos": consulta.include_suspended as i64,
            ":estado": estado,
            ":jogo": jogo,
            ":busca": busca,
            ":limite": consulta.limit.unwrap_or(LIMITE_PADRAO),
            ":deslocamento": consulta.offset.unwrap_or(0),
        },
        card_row,
    )?;
    Ok(linhas.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Um card com todos os contextos, para a tela de detalhe.
pub fn card_detail(conexao: &Connection, card_id: i64) -> Result<Option<CardDetail>> {
    let card = conexao
        .query_row(
            "SELECT c.id, c.lemma, c.created_at, c.suspended, c.fsrs_due, c.fsrs_state,
                    c.fsrs_reps, c.fsrs_lapses,
                    (SELECT COUNT(*) FROM contexts n WHERE n.card_id = c.id) AS contexts,
                    NULL AS last_sentence, NULL AS last_game
               FROM cards c WHERE c.id = ?1",
            [card_id],
            card_row,
        )
        .optional()?;

    let Some(card) = card else {
        return Ok(None);
    };

    let mut stmt = conexao.prepare(
        "SELECT id, card_id, form, sentence_en, sentence_pt, game_name,
                screenshot_path, captured_at
           FROM contexts WHERE card_id = ?1
          ORDER BY captured_at DESC, id DESC",
    )?;
    let contexts = stmt
        .query_map([card_id], |linha| {
            Ok(CardContext {
                id: linha.get(0)?,
                card_id: linha.get(1)?,
                form: linha.get(2)?,
                sentence_en: linha.get(3)?,
                sentence_pt: linha.get(4)?,
                game_name: linha.get(5)?,
                screenshot_path: linha.get(6)?,
                captured_at: linha.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(CardDetail { card, contexts }))
}

/// Jogos com pelo menos um contexto, para o filtro da tela.
pub fn games(conexao: &Connection) -> Result<Vec<String>> {
    let mut stmt = conexao.prepare(
        "SELECT DISTINCT game_name FROM contexts
          WHERE game_name IS NOT NULL AND game_name <> ''
          ORDER BY game_name COLLATE NOCASE",
    )?;
    let jogos = stmt.query_map([], |linha| linha.get::<_, String>(0))?;
    Ok(jogos.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Marca (ou desmarca) "ja sei". Suspender tira o card da fila **sem** apagar
/// o historico nem o agendamento: reativar devolve o card onde ele estava.
pub fn set_suspended(conexao: &Connection, card_id: i64, suspenso: bool) -> Result<()> {
    let linhas = conexao.execute(
        "UPDATE cards SET suspended = ?2 WHERE id = ?1",
        params![card_id, suspenso as i64],
    )?;
    if linhas == 0 {
        return Err(Error::Deck(format!("card {card_id} nao existe")));
    }
    Ok(())
}

/// Corrige a traducao de um contexto — a maquina erra, e o card fica com o
/// erro na frente se nao der para arrumar (F4).
pub fn update_context(conexao: &Connection, context_id: i64, traducao: Option<&str>) -> Result<()> {
    let traducao = traducao.map(str::trim).filter(|texto| !texto.is_empty());
    let linhas = conexao.execute(
        "UPDATE contexts SET sentence_pt = ?2 WHERE id = ?1",
        params![context_id, traducao],
    )?;
    if linhas == 0 {
        return Err(Error::Deck(format!("contexto {context_id} nao existe")));
    }
    Ok(())
}

/// Apaga o card e devolve os screenshots que ficaram orfaos.
///
/// Os arquivos sao apagados por quem chamou, ja fora da transacao: o `ON DELETE
/// CASCADE` limpa o banco, mas ninguem avisa o disco.
pub fn delete_card(conexao: &Connection, card_id: i64) -> Result<Vec<String>> {
    let mut stmt = conexao.prepare(
        "SELECT screenshot_path FROM contexts
          WHERE card_id = ?1 AND screenshot_path IS NOT NULL",
    )?;
    let caminhos = stmt
        .query_map([card_id], |linha| linha.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let linhas = conexao.execute("DELETE FROM cards WHERE id = ?1", [card_id])?;
    if linhas == 0 {
        return Err(Error::Deck(format!("card {card_id} nao existe")));
    }
    Ok(caminhos)
}

// ---------------------------------------------------------------------------
// Export CSV (F4)
// ---------------------------------------------------------------------------

/// Cabecalho do CSV. A ordem e a mesma dos campos em [`linha_csv`].
const COLUNAS_CSV: &str =
    "lema,forma,frase,traducao,jogo,capturado_em,estado,repeticoes,lapsos,vencimento";

/// Escapa um campo pela regra do RFC 4180.
///
/// Sempre entre aspas, e nao so quando ha virgula: frase de jogo tem virgula,
/// aspas e — em dialogo com quebra — nova linha, e decidir campo a campo so
/// cria a chance de errar em um. Aspas internas viram aspas duplas.
fn campo_csv(valor: &str) -> String {
    format!("\"{}\"", valor.replace('"', "\"\""))
}

fn linha_csv(campos: &[&str]) -> String {
    campos
        .iter()
        .map(|campo| campo_csv(campo))
        .collect::<Vec<_>>()
        .join(",")
}

/// Monta o CSV do deck inteiro: uma linha por **contexto**, nao por card.
///
/// Uma linha por card perderia justamente o que este app tem de diferente — as
/// frases onde a palavra apareceu. Cards sem contexto ainda saem, com os campos
/// de frase vazios, para o export nunca esconder uma palavra salva.
pub fn export_csv(conexao: &Connection) -> Result<String> {
    let mut stmt = conexao.prepare(
        "SELECT c.lemma, c.fsrs_state, c.fsrs_reps, c.fsrs_lapses, c.fsrs_due,
                ctx.form, ctx.sentence_en, ctx.sentence_pt, ctx.game_name, ctx.captured_at
           FROM cards c
           LEFT JOIN contexts ctx ON ctx.card_id = c.id
          ORDER BY c.lemma COLLATE NOCASE, ctx.captured_at, ctx.id",
    )?;

    let linhas = stmt.query_map([], |linha| {
        let vazio = String::new();
        Ok(linha_csv(&[
            &linha.get::<_, String>(0)?,
            &linha.get::<_, Option<String>>(5)?.unwrap_or(vazio.clone()),
            &linha.get::<_, Option<String>>(6)?.unwrap_or(vazio.clone()),
            &linha.get::<_, Option<String>>(7)?.unwrap_or(vazio.clone()),
            &linha.get::<_, Option<String>>(8)?.unwrap_or(vazio.clone()),
            &linha.get::<_, Option<String>>(9)?.unwrap_or(vazio),
            &linha.get::<_, String>(1)?,
            &linha.get::<_, u32>(2)?.to_string(),
            &linha.get::<_, u32>(3)?.to_string(),
            &linha.get::<_, String>(4)?,
        ]))
    })?;

    let mut csv = String::from(COLUNAS_CSV);
    for linha in linhas {
        csv.push_str("\r\n");
        csv.push_str(&linha?);
    }
    csv.push_str("\r\n");
    Ok(csv)
}

fn contar_contextos(conexao: &Connection, card_id: i64) -> Result<u32> {
    Ok(conexao.query_row(
        "SELECT COUNT(*) FROM contexts WHERE card_id = ?1",
        [card_id],
        |linha| linha.get(0),
    )?)
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

/// Roda `f` com a conexao do app tomada.
///
/// A primeira chamada do processo ainda abre o arquivo e aplica as migrations —
/// e o que faz valer a pena tirar isto da main thread nos comandos abaixo.
fn com_conexao<T>(app: &AppHandle, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let guarda = db::conexao(app)?;
    let conexao = guarda
        .as_ref()
        .ok_or_else(|| Error::Deck("banco nao aberto".into()))?;
    f(conexao)
}

/// Recorta e grava o screenshot do contexto, devolvendo o caminho relativo.
///
/// `None` em qualquer tropeco — captura ja descartada, disco cheio, consulta
/// vencida. **Nunca** um erro: perder a imagem e um card mais pobre; perder o
/// salvamento e o usuario perdendo a palavra que ele acabou de encontrar.
fn screenshot_do_contexto(
    app: &AppHandle,
    entrada: &SaveCardInput,
    context_id: i64,
) -> Option<String> {
    let recorte = crate::lookup::recortar_linha(entrada.lookup_id?, entrada.line_index?)?;
    match crate::media::salvar(app, &format!("ctx-{context_id:06}"), &recorte) {
        Ok(caminho) => Some(caminho),
        Err(erro) => {
            eprintln!("[deck] contexto {context_id} ficou sem screenshot: {erro}");
            None
        }
    }
}

/// `async` pelo mesmo motivo do `dict_lookup`: a main thread desenha a overlay
/// e nao pode esperar disco.
#[tauri::command]
pub async fn deck_save_card(app: AppHandle, input: SaveCardInput) -> Result<CardSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        com_conexao(&app, |conexao| {
            let salvo = save_card(conexao, &input)?;
            if let Some(context_id) = salvo.contexto_novo {
                if let Some(caminho) = screenshot_do_contexto(&app, &input, context_id) {
                    set_screenshot(conexao, context_id, &caminho)?;
                }
            }
            Ok(salvo.resumo)
        })
    })
    .await
    .map_err(|e| Error::Deck(format!("salvamento abortado: {e}")))?
}

#[tauri::command]
pub async fn deck_card_status(app: AppHandle, lemma: String) -> Result<Option<CardSummary>> {
    tauri::async_runtime::spawn_blocking(move || {
        com_conexao(&app, |conexao| card_status(conexao, &lemma))
    })
    .await
    .map_err(|e| Error::Deck(format!("consulta ao deck abortada: {e}")))?
}

/// Roda `f` fora da main thread, com a conexao tomada. Todo comando que toca o
/// banco do usuario passa por aqui — nenhum deles pode travar a janela enquanto
/// le disco.
pub async fn no_banco<T, F>(app: AppHandle, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&AppHandle, &Connection) -> Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || com_conexao(&app.clone(), |c| f(&app, c)))
        .await
        .map_err(|e| Error::Deck(format!("operacao no deck abortada: {e}")))?
}

#[tauri::command]
pub async fn deck_list_cards(app: AppHandle, query: CardQuery) -> Result<Vec<CardRow>> {
    no_banco(app, move |_, conexao| list_cards(conexao, &query)).await
}

#[tauri::command]
pub async fn deck_card_detail(app: AppHandle, card_id: i64) -> Result<Option<CardDetail>> {
    no_banco(app, move |_, conexao| card_detail(conexao, card_id)).await
}

#[tauri::command]
pub async fn deck_games(app: AppHandle) -> Result<Vec<String>> {
    no_banco(app, |_, conexao| games(conexao)).await
}

#[tauri::command]
pub async fn deck_set_suspended(app: AppHandle, card_id: i64, suspended: bool) -> Result<()> {
    no_banco(app, move |_, conexao| {
        set_suspended(conexao, card_id, suspended)
    })
    .await
}

#[tauri::command]
pub async fn deck_update_context(
    app: AppHandle,
    context_id: i64,
    sentence_pt: Option<String>,
) -> Result<()> {
    no_banco(app, move |_, conexao| {
        update_context(conexao, context_id, sentence_pt.as_deref())
    })
    .await
}

/// Apaga o card, os contextos e os screenshots deles.
#[tauri::command]
pub async fn deck_delete_card(app: AppHandle, card_id: i64) -> Result<()> {
    no_banco(app, move |app, conexao| {
        for caminho in delete_card(conexao, card_id)? {
            // O card ja saiu do banco: um arquivo que resiste vira lixo em
            // media/, nao um erro na cara de quem so queria apagar o card.
            if let Err(erro) = crate::media::remover(app, &caminho) {
                eprintln!("[deck] screenshot {caminho} nao foi apagado: {erro}");
            }
        }
        Ok(())
    })
    .await
}

/// Escreve o CSV do deck no caminho que o usuario escolheu no dialogo.
///
/// Devolve quantas linhas de dados foram gravadas, para a UI dizer "42 linhas
/// exportadas" em vez de um "pronto" que nao prova nada.
///
/// O BOM de UTF-8 na frente e por causa do Excel: sem ele, "espião" abre como
/// "espiÃ£o" na maquina de quem vai de fato usar o arquivo.
#[tauri::command]
pub async fn deck_export_csv(app: AppHandle, path: String) -> Result<u32> {
    no_banco(app, move |_, conexao| {
        let csv = export_csv(conexao)?;
        let mut bytes = Vec::with_capacity(csv.len() + 3);
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        bytes.extend_from_slice(csv.as_bytes());
        std::fs::write(&path, bytes).map_err(|e| Error::Deck(format!("nao gravou {path}: {e}")))?;
        // Menos o cabecalho e a linha em branco do fim.
        Ok(csv.lines().count().saturating_sub(1) as u32)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn banco() -> Connection {
        let conexao = Connection::open_in_memory().expect("abriu em memoria");
        db::preparar(&conexao).expect("migrou");
        conexao
    }

    /// Estado que o `createEmptyCard` do ts-fsrs produz para uma palavra nova.
    fn fsrs_zerado() -> FsrsFields {
        FsrsFields {
            due: "2026-08-01T12:00:00Z".into(),
            stability: 0.0,
            difficulty: 0.0,
            state: "new".into(),
            reps: 0,
            lapses: 0,
            scheduled_days: 0,
            learning_steps: 0,
            last_review: None,
        }
    }

    fn entrada(lemma: &str, form: &str, frase: &str) -> SaveCardInput {
        SaveCardInput {
            lemma: lemma.into(),
            form: form.into(),
            sentence_en: frase.into(),
            sentence_pt: Some("traducao".into()),
            game_name: Some("Skyrim".into()),
            fsrs: fsrs_zerado(),
            // O recorte depende de uma captura viva no processo; estes testes
            // exercitam so o banco.
            lookup_id: None,
            line_index: None,
        }
    }

    #[test]
    fn salvar_cria_o_card_com_um_contexto() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        assert!(salvo.resumo.created);
        assert_eq!(salvo.resumo.lemma, "run");
        assert_eq!(salvo.resumo.contexts, 1);
        assert!(salvo.contexto_novo.is_some(), "contexto recem-inserido");
    }

    #[test]
    fn a_mesma_palavra_em_outra_frase_vira_contexto_do_mesmo_card() {
        // O coracao da decisao "um card por lema": duas flexoes, um card.
        let conexao = banco();
        let primeiro = save_card(&conexao, &entrada("run", "ran", "He ran away."))
            .expect("salvou")
            .resumo;
        let segundo = save_card(&conexao, &entrada("run", "running", "She is running."))
            .expect("salvou de novo")
            .resumo;

        assert_eq!(primeiro.id, segundo.id, "mesmo card");
        assert!(!segundo.created, "o segundo so anexou contexto");
        assert_eq!(segundo.contexts, 2);

        let cards: u32 = conexao
            .query_row("SELECT COUNT(*) FROM cards", [], |l| l.get(0))
            .expect("contou");
        assert_eq!(cards, 1);
    }

    #[test]
    fn salvar_a_mesma_frase_duas_vezes_nao_duplica_contexto() {
        let conexao = banco();
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let repetido =
            save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou de novo");
        assert_eq!(
            repetido.resumo.contexts, 1,
            "clique repetido nao vira contexto"
        );
        assert_eq!(
            repetido.contexto_novo, None,
            "sem contexto novo nao ha screenshot novo para gravar"
        );
    }

    #[test]
    fn o_screenshot_entra_no_contexto_recem_criado() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let contexto = salvo.contexto_novo.expect("contexto novo");
        set_screenshot(&conexao, contexto, "media/ctx-000001.webp").expect("anexou");

        let caminho: Option<String> = conexao
            .query_row(
                "SELECT screenshot_path FROM contexts WHERE id = ?1",
                [contexto],
                |l| l.get(0),
            )
            .expect("leu o contexto");
        assert_eq!(caminho.as_deref(), Some("media/ctx-000001.webp"));
    }

    #[test]
    fn anexar_contexto_nao_mexe_no_agendamento() {
        // Reencontrar a palavra num jogo nao pode adiantar nem atrasar a
        // revisao dela: so o wrapper do FSRS mexe nisso.
        let conexao = banco();
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        conexao
            .execute(
                "UPDATE cards SET fsrs_due = '2027-01-01T00:00:00Z', fsrs_reps = 3,
                     fsrs_state = 'review' WHERE lemma = 'run'",
                [],
            )
            .expect("simulou uma revisao");

        save_card(&conexao, &entrada("run", "running", "She is running.")).expect("anexou");

        let (due, reps, state): (String, u32, String) = conexao
            .query_row(
                "SELECT fsrs_due, fsrs_reps, fsrs_state FROM cards WHERE lemma = 'run'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .expect("leu o card");
        assert_eq!(due, "2027-01-01T00:00:00Z");
        assert_eq!(reps, 3);
        assert_eq!(state, "review");
    }

    #[test]
    fn o_contexto_guarda_a_forma_da_tela_nao_o_lema() {
        let conexao = banco();
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let (form, pt, jogo): (String, Option<String>, Option<String>) = conexao
            .query_row(
                "SELECT form, sentence_pt, game_name FROM contexts",
                [],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .expect("leu o contexto");
        assert_eq!(form, "ran");
        assert_eq!(pt.as_deref(), Some("traducao"));
        assert_eq!(jogo.as_deref(), Some("Skyrim"));
    }

    #[test]
    fn card_status_responde_antes_e_depois_de_salvar() {
        let conexao = banco();
        assert!(card_status(&conexao, "run").expect("consultou").is_none());
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let salvo = card_status(&conexao, "run")
            .expect("consultou")
            .expect("existe");
        assert_eq!(salvo.contexts, 1);
        assert!(!salvo.created);
    }

    #[test]
    fn card_sem_lema_ou_sem_frase_e_recusado() {
        let conexao = banco();
        assert!(save_card(&conexao, &entrada("  ", "ran", "He ran away.")).is_err());
        assert!(save_card(&conexao, &entrada("run", "ran", "   ")).is_err());
    }

    // -----------------------------------------------------------------------
    // Gestao do deck
    // -----------------------------------------------------------------------

    /// Deck de tres cards em dois jogos, com um suspenso.
    fn deck_de_exemplo() -> Connection {
        let conexao = banco();
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        save_card(&conexao, &entrada("dread", "dread", "I dread the night.")).expect("salvou");

        let mut outro = entrada("blade", "blades", "The blades are sharp.");
        outro.game_name = Some("Hollow Knight".into());
        save_card(&conexao, &outro).expect("salvou");

        set_suspended(&conexao, 2, true).expect("suspendeu dread");
        conexao
    }

    fn lemas(cards: &[CardRow]) -> Vec<&str> {
        cards.iter().map(|c| c.lemma.as_str()).collect()
    }

    #[test]
    fn a_lista_esconde_os_suspensos_por_padrao() {
        // "Ja sei" so vale a pena se a palavra realmente sumir da vista.
        let conexao = deck_de_exemplo();
        let cards = list_cards(&conexao, &CardQuery::default()).expect("listou");
        assert_eq!(lemas(&cards), vec!["blade", "run"]);

        let com_suspensos = list_cards(
            &conexao,
            &CardQuery {
                include_suspended: true,
                order: Ordem::Alfabetica,
                ..CardQuery::default()
            },
        )
        .expect("listou");
        assert_eq!(lemas(&com_suspensos), vec!["blade", "dread", "run"]);
    }

    #[test]
    fn a_lista_traz_o_contexto_mais_recente_do_card() {
        let conexao = banco();
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        save_card(&conexao, &entrada("run", "running", "She is running.")).expect("salvou");
        conexao
            .execute(
                "UPDATE contexts SET captured_at = '9999-01-01T00:00:00Z'
                  WHERE sentence_en = 'She is running.'",
                [],
            )
            .expect("datou o segundo contexto");

        let cards = list_cards(&conexao, &CardQuery::default()).expect("listou");
        assert_eq!(cards.len(), 1, "um card por lema");
        assert_eq!(cards[0].contexts, 2);
        assert_eq!(cards[0].last_sentence.as_deref(), Some("She is running."));
    }

    #[test]
    fn a_busca_alcanca_o_lema_e_as_frases() {
        let conexao = deck_de_exemplo();
        let buscar = |texto: &str| {
            list_cards(
                &conexao,
                &CardQuery {
                    search: Some(texto.into()),
                    include_suspended: true,
                    ..CardQuery::default()
                },
            )
            .expect("buscou")
        };

        assert_eq!(lemas(&buscar("bla")), vec!["blade"], "pedaco do lema");
        assert_eq!(lemas(&buscar("night")), vec!["dread"], "palavra da frase");
        assert_eq!(lemas(&buscar("RAN")), vec!["run"], "busca sem caixa");
        assert!(buscar("inexistente").is_empty());
    }

    #[test]
    fn curinga_digitado_e_texto_e_nao_curinga() {
        // Sem escapar, procurar por "%" traria o deck inteiro — e a busca
        // pareceria ignorar o que foi digitado.
        let conexao = deck_de_exemplo();
        let cards = list_cards(
            &conexao,
            &CardQuery {
                search: Some("%".into()),
                include_suspended: true,
                ..CardQuery::default()
            },
        )
        .expect("buscou");
        assert!(cards.is_empty(), "nenhum card tem '%' no texto: {cards:?}");
    }

    #[test]
    fn o_filtro_de_jogo_usa_os_contextos_do_card() {
        let conexao = deck_de_exemplo();
        let cards = list_cards(
            &conexao,
            &CardQuery {
                game: Some("Hollow Knight".into()),
                include_suspended: true,
                ..CardQuery::default()
            },
        )
        .expect("filtrou");
        assert_eq!(lemas(&cards), vec!["blade"]);
        assert_eq!(games(&conexao).expect("jogos"), ["Hollow Knight", "Skyrim"]);
    }

    #[test]
    fn a_ordem_alfabetica_e_a_de_vencimento_sao_diferentes() {
        let conexao = deck_de_exemplo();
        conexao
            .execute(
                "UPDATE cards SET fsrs_due = '2026-01-01T00:00:00Z' WHERE lemma = 'run'",
                [],
            )
            .expect("venceu o run");

        let ordenar = |ordem| {
            list_cards(
                &conexao,
                &CardQuery {
                    order: ordem,
                    include_suspended: true,
                    ..CardQuery::default()
                },
            )
            .expect("ordenou")
        };
        assert_eq!(
            lemas(&ordenar(Ordem::Alfabetica)),
            ["blade", "dread", "run"]
        );
        assert_eq!(
            lemas(&ordenar(Ordem::Vencimento))[0],
            "run",
            "o vencido primeiro"
        );
    }

    #[test]
    fn o_detalhe_traz_os_contextos_do_mais_novo_para_o_mais_velho() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        save_card(&conexao, &entrada("run", "running", "She is running.")).expect("salvou");
        conexao
            .execute(
                "UPDATE contexts SET captured_at = '9999-01-01T00:00:00Z'
                  WHERE sentence_en = 'She is running.'",
                [],
            )
            .expect("datou");

        let detalhe = card_detail(&conexao, salvo.resumo.id)
            .expect("consultou")
            .expect("existe");
        assert_eq!(detalhe.card.lemma, "run");
        assert_eq!(detalhe.contexts.len(), 2);
        assert_eq!(detalhe.contexts[0].sentence_en, "She is running.");
        assert_eq!(detalhe.contexts[1].form, "ran");
    }

    #[test]
    fn detalhe_de_card_que_nao_existe_e_nada_em_vez_de_erro() {
        let conexao = banco();
        assert!(card_detail(&conexao, 404).expect("consultou").is_none());
    }

    #[test]
    fn suspender_nao_mexe_no_agendamento() {
        // O card volta para a fila onde parou quando o usuario perceber que,
        // afinal, nao sabia a palavra.
        let conexao = deck_de_exemplo();
        let antes: String = conexao
            .query_row("SELECT fsrs_due FROM cards WHERE id = 1", [], |l| l.get(0))
            .expect("leu");

        set_suspended(&conexao, 1, true).expect("suspendeu");
        set_suspended(&conexao, 1, false).expect("reativou");

        let depois: String = conexao
            .query_row("SELECT fsrs_due FROM cards WHERE id = 1", [], |l| l.get(0))
            .expect("leu");
        assert_eq!(antes, depois);
    }

    #[test]
    fn suspender_card_que_nao_existe_e_erro() {
        let conexao = banco();
        assert!(set_suspended(&conexao, 404, true).is_err());
    }

    #[test]
    fn editar_a_traducao_do_contexto_grava_e_apaga() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let contexto = salvo.contexto_novo.expect("contexto novo");

        update_context(&conexao, contexto, Some("  Ele fugiu.  ")).expect("editou");
        assert_eq!(traducao(&conexao, contexto).as_deref(), Some("Ele fugiu."));

        // Apagar a traducao ruim e uma edicao valida: melhor sem do que errada.
        update_context(&conexao, contexto, Some("   ")).expect("limpou");
        assert_eq!(traducao(&conexao, contexto), None);
    }

    fn traducao(conexao: &Connection, contexto: i64) -> Option<String> {
        conexao
            .query_row(
                "SELECT sentence_pt FROM contexts WHERE id = ?1",
                [contexto],
                |l| l.get(0),
            )
            .expect("leu o contexto")
    }

    #[test]
    fn apagar_o_card_devolve_os_screenshots_para_quem_apaga_os_arquivos() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let contexto = salvo.contexto_novo.expect("contexto novo");
        set_screenshot(&conexao, contexto, "media/ctx-000001.webp").expect("anexou");

        let orfaos = delete_card(&conexao, salvo.resumo.id).expect("apagou");
        assert_eq!(orfaos, vec!["media/ctx-000001.webp".to_string()]);
        assert!(
            delete_card(&conexao, salvo.resumo.id).is_err(),
            "apagar duas vezes tem que reclamar"
        );
    }

    #[test]
    fn apagar_o_card_leva_os_contextos_junto() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        conexao
            .execute("DELETE FROM cards WHERE id = ?1", [salvo.resumo.id])
            .expect("apagou");
        let sobraram: u32 = conexao
            .query_row("SELECT COUNT(*) FROM contexts", [], |l| l.get(0))
            .expect("contou");
        assert_eq!(sobraram, 0);
    }

    #[test]
    fn o_csv_sai_com_cabecalho_e_uma_linha_por_contexto() {
        let conexao = banco();
        save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        save_card(&conexao, &entrada("run", "running", "She is running.")).expect("salvou");
        save_card(&conexao, &entrada("dread", "dread", "A dread silence.")).expect("salvou");

        let csv = export_csv(&conexao).expect("exportou");
        let linhas: Vec<&str> = csv.lines().collect();
        assert_eq!(linhas[0], COLUNAS_CSV);
        assert_eq!(linhas.len(), 4);
        // Ordenado por lema: "dread" antes de "run".
        assert!(linhas[1].starts_with("\"dread\""));
    }

    #[test]
    fn o_csv_escapa_aspas_e_virgulas_da_frase() {
        let conexao = banco();
        save_card(
            &conexao,
            &entrada("say", "said", "He said \"run\", and I ran."),
        )
        .expect("salvou");

        let csv = export_csv(&conexao).expect("exportou");
        assert!(
            csv.contains("\"He said \"\"run\"\", and I ran.\""),
            "aspas internas precisam virar aspas duplas: {csv}"
        );
    }

    #[test]
    fn card_sem_contexto_ainda_aparece_no_csv() {
        let conexao = banco();
        let salvo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        conexao
            .execute("DELETE FROM contexts WHERE card_id = ?1", [salvo.resumo.id])
            .expect("apagou o contexto");

        let csv = export_csv(&conexao).expect("exportou");
        let linhas: Vec<&str> = csv.lines().collect();
        assert_eq!(linhas.len(), 2);
        assert!(linhas[1].starts_with("\"run\",\"\",\"\""));
    }
}
