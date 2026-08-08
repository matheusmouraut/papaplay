//! Fila do dia e registro das revisoes (F5).
//!
//! # Quem calcula o agendamento
//!
//! Nao e este modulo. Regra inviolavel #4: o proximo `due`, a estabilidade e a
//! dificuldade saem do `ts-fsrs`, no wrapper `src/shared/srs/`. Aqui o core so
//! monta a fila (uma consulta) e grava o que a UI calculou (um update mais um
//! insert, na mesma transacao).
//!
//! # Por que o dia vem da UI
//!
//! "Novos por dia" e "revisado hoje" sao perguntas sobre o **dia local**, e o
//! banco guarda tudo em UTC. Em vez de o core adivinhar o fuso — que muda com
//! horario de verao e com viagem —, quem chama manda o instante de agora e o
//! comeco do dia local ja resolvidos. O core so compara strings ISO.
//!
//! # A ordem da fila
//!
//! Vencidos primeiro, na ordem em que venceram; novos depois, na ordem em que
//! foram salvos. Cartao vencido esquecido custa mais que palavra nova adiada:
//! o vencido esta prestes a sair da memoria, o novo nunca esteve nela.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::deck::{no_banco, CardContext, FsrsFields};
use crate::error::{Error, Result};

/// Teto de cards que uma sessao carrega de uma vez.
///
/// A fila inteira vai para a memoria da UI porque revisar precisa ser
/// instantaneo entre um card e outro. Passar disso e sinal de deck abandonado
/// por semanas, e o usuario nao vai revisar 500 cards numa sentada de qualquer
/// forma.
const TETO_DA_FILA: u32 = 500;

/// Card com o estado completo do FSRS — o que o `ts-fsrs` precisa para calcular
/// a proxima data. A `CardRow` da tela Deck nao serve: ela omite estabilidade,
/// dificuldade e os passos de aprendizado.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCard {
    pub id: i64,
    pub lemma: String,
    pub created_at: String,
    pub suspended: bool,
    pub fsrs_due: String,
    pub fsrs_stability: f64,
    pub fsrs_difficulty: f64,
    pub fsrs_state: String,
    pub fsrs_reps: u32,
    pub fsrs_lapses: u32,
    pub fsrs_scheduled_days: i64,
    pub fsrs_learning_steps: i64,
    pub fsrs_last_review: Option<String>,
}

/// Um item da fila: o card mais o contexto que vai na frente dele.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCard {
    pub card: DeckCard,
    /// Contexto mais recente — a frase e o screenshot que a frente mostra.
    /// `None` so acontece com card cujo unico contexto foi apagado.
    pub context: Option<CardContext>,
    /// Quantas vezes a palavra ja foi encontrada, para a UI dizer "3 contextos".
    pub contexts: u32,
}

/// O pedido da fila, com o dia local ja resolvido pela UI.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueQuery {
    /// Instante de agora em ISO-8601 UTC: o que define "vencido".
    pub now: String,
    /// Meia-noite local de hoje, em ISO-8601 UTC.
    pub day_start: String,
    /// Quantos cards novos por dia (F5: padrao 15, configuravel).
    pub new_limit: u32,
}

/// A fila mais os numeros que a tela mostra antes de comecar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueue {
    /// Vencidos primeiro, novos depois. Ja limitada pelo teto e pela cota.
    pub cards: Vec<ReviewCard>,
    /// Vencidos na fila.
    pub due: u32,
    /// Novos na fila (ja descontada a cota gasta hoje).
    pub fresh: u32,
    /// Novos que ja foram introduzidos hoje — o que consome a cota.
    pub introduced_today: u32,
    /// Cards novos no deck que nao couberam na cota de hoje.
    pub new_left_over: u32,
    /// Cards nao suspensos no deck. E o que distingue "deck vazio" de "tudo em
    /// dia" na tela quando a fila volta sem nada.
    pub total: u32,
}

/// Uma linha de `review_log`, como a UI a produz.
///
/// Guarda o estado **antes e depois** de proposito: e o que permite re-otimizar
/// os parametros do FSRS com o historico real do usuario mais tarde (docs/04).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLogEntry {
    pub reviewed_at: String,
    /// 1=Errei, 2=Dificil, 3=Bom, 4=Facil (o `Rating` do ts-fsrs).
    pub rating: u8,
    pub elapsed_days: f64,
    pub state_before: String,
    pub state_after: String,
}

/// O que a UI manda depois de o usuario dar a nota.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewInput {
    pub card_id: i64,
    /// Ja calculado pelo wrapper do ts-fsrs.
    pub fsrs: FsrsFields,
    pub log: ReviewLogEntry,
}

fn deck_card(linha: &rusqlite::Row<'_>) -> rusqlite::Result<DeckCard> {
    Ok(DeckCard {
        id: linha.get("id")?,
        lemma: linha.get("lemma")?,
        created_at: linha.get("created_at")?,
        suspended: linha.get::<_, i64>("suspended")? != 0,
        fsrs_due: linha.get("fsrs_due")?,
        fsrs_stability: linha.get("fsrs_stability")?,
        fsrs_difficulty: linha.get("fsrs_difficulty")?,
        fsrs_state: linha.get("fsrs_state")?,
        fsrs_reps: linha.get("fsrs_reps")?,
        fsrs_lapses: linha.get("fsrs_lapses")?,
        fsrs_scheduled_days: linha.get("fsrs_scheduled_days")?,
        fsrs_learning_steps: linha.get("fsrs_learning_steps")?,
        fsrs_last_review: linha.get("fsrs_last_review")?,
    })
}

/// Colunas do card + o contexto mais recente, compartilhadas pelas duas
/// consultas da fila. `{extra}` recebe o filtro que distingue vencido de novo.
fn sql_da_fila(extra: &str, ordem: &str) -> String {
    format!(
        "SELECT c.id, c.lemma, c.created_at, c.suspended,
                c.fsrs_due, c.fsrs_stability, c.fsrs_difficulty, c.fsrs_state,
                c.fsrs_reps, c.fsrs_lapses, c.fsrs_scheduled_days,
                c.fsrs_learning_steps, c.fsrs_last_review,
                (SELECT COUNT(*) FROM contexts n WHERE n.card_id = c.id) AS contexts,
                ctx.id              AS ctx_id,
                ctx.form            AS ctx_form,
                ctx.sentence_en     AS ctx_sentence_en,
                ctx.sentence_pt     AS ctx_sentence_pt,
                ctx.game_name       AS ctx_game_name,
                ctx.screenshot_path AS ctx_screenshot_path,
                ctx.captured_at     AS ctx_captured_at
           FROM cards c
           LEFT JOIN contexts ctx ON ctx.id = (
                SELECT id FROM contexts u WHERE u.card_id = c.id
                 ORDER BY u.captured_at DESC, u.id DESC LIMIT 1
           )
          WHERE c.suspended = 0 AND {extra}
          ORDER BY {ordem}
          LIMIT :limite"
    )
}

fn review_card(linha: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewCard> {
    let card = deck_card(linha)?;
    // Sem contexto o card ainda revisa (o lema esta la), so fica sem frase.
    let context = linha
        .get::<_, Option<i64>>("ctx_id")?
        .map(|id| -> rusqlite::Result<CardContext> {
            Ok(CardContext {
                id,
                card_id: card.id,
                form: linha.get("ctx_form")?,
                sentence_en: linha.get("ctx_sentence_en")?,
                sentence_pt: linha.get("ctx_sentence_pt")?,
                game_name: linha.get("ctx_game_name")?,
                screenshot_path: linha.get("ctx_screenshot_path")?,
                captured_at: linha.get("ctx_captured_at")?,
            })
        })
        .transpose()?;
    Ok(ReviewCard {
        card,
        context,
        contexts: linha.get("contexts")?,
    })
}

/// Quantos cards distintos sairam do estado `new` hoje.
///
/// Conta pelo `review_log`, e nao pelo `fsrs_state` dos cards: um card
/// introduzido hoje que ja voltou para `relearning` continua tendo consumido a
/// cota do dia, e o log e o unico lugar onde isso fica registrado.
fn introduzidos_hoje(conexao: &Connection, day_start: &str) -> Result<u32> {
    Ok(conexao.query_row(
        "SELECT COUNT(DISTINCT card_id) FROM review_log
          WHERE state_before = 'new' AND reviewed_at >= ?1",
        [day_start],
        |linha| linha.get(0),
    )?)
}

/// Monta a fila do dia: vencidos + a cota de novos que ainda sobrou.
pub fn queue(conexao: &Connection, consulta: &QueueQuery) -> Result<ReviewQueue> {
    let mut stmt = conexao.prepare(&sql_da_fila(
        "c.fsrs_state <> 'new' AND c.fsrs_due <= :agora",
        "c.fsrs_due ASC, c.id ASC",
    ))?;
    let mut cards = stmt
        .query_map(
            rusqlite::named_params! { ":agora": consulta.now, ":limite": TETO_DA_FILA },
            review_card,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let due = cards.len() as u32;

    let introduced_today = introduzidos_hoje(conexao, &consulta.day_start)?;
    let cota = consulta.new_limit.saturating_sub(introduced_today);
    let espaco = TETO_DA_FILA.saturating_sub(due).min(cota);

    if espaco > 0 {
        let mut stmt = conexao.prepare(&sql_da_fila(
            "c.fsrs_state = 'new'",
            "c.created_at ASC, c.id ASC",
        ))?;
        let novos = stmt
            .query_map(rusqlite::named_params! { ":limite": espaco }, review_card)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        cards.extend(novos);
    }

    // O total de novos e uma contagem a parte, e nao o tamanho da lista: a
    // lista ja vem cortada pela cota, e o que a tela quer dizer e "ha mais 30
    // esperando amanha".
    let novos_no_deck: u32 = conexao.query_row(
        "SELECT COUNT(*) FROM cards WHERE suspended = 0 AND fsrs_state = 'new'",
        [],
        |linha| linha.get(0),
    )?;
    let fresh = cards.len() as u32 - due;
    let total = conexao.query_row(
        "SELECT COUNT(*) FROM cards WHERE suspended = 0",
        [],
        |linha| linha.get(0),
    )?;
    Ok(ReviewQueue {
        cards,
        due,
        fresh,
        introduced_today,
        new_left_over: novos_no_deck.saturating_sub(fresh),
        total,
    })
}

/// Grava a nota: novo agendamento no card e uma linha no historico.
///
/// Os dois numa transacao so — um card reagendado sem linha no log e uma
/// revisao que aconteceu e sumiu, e e justamente o log que permite re-otimizar
/// os parametros do FSRS depois.
pub fn apply(conexao: &Connection, entrada: &ReviewInput) -> Result<()> {
    let existe: Option<i64> = conexao
        .query_row(
            "SELECT id FROM cards WHERE id = ?1",
            [entrada.card_id],
            |linha| linha.get(0),
        )
        .optional()?;
    if existe.is_none() {
        return Err(Error::Deck(format!("card {} nao existe", entrada.card_id)));
    }

    conexao.execute("BEGIN IMMEDIATE", [])?;
    let resultado = gravar(conexao, entrada);
    match &resultado {
        Ok(()) => conexao.execute("COMMIT", [])?,
        Err(_) => conexao.execute("ROLLBACK", [])?,
    };
    resultado
}

fn gravar(conexao: &Connection, entrada: &ReviewInput) -> Result<()> {
    let f = &entrada.fsrs;
    conexao.execute(
        "UPDATE cards
            SET fsrs_due = ?2, fsrs_stability = ?3, fsrs_difficulty = ?4,
                fsrs_state = ?5, fsrs_reps = ?6, fsrs_lapses = ?7,
                fsrs_scheduled_days = ?8, fsrs_learning_steps = ?9,
                fsrs_last_review = ?10
          WHERE id = ?1",
        params![
            entrada.card_id,
            f.due,
            f.stability,
            f.difficulty,
            f.state,
            f.reps,
            f.lapses,
            f.scheduled_days,
            f.learning_steps,
            f.last_review,
        ],
    )?;

    let log = &entrada.log;
    conexao.execute(
        "INSERT INTO review_log (
             card_id, reviewed_at, rating, elapsed_days, state_before, state_after
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            entrada.card_id,
            log.reviewed_at,
            log.rating,
            log.elapsed_days,
            log.state_before,
            log.state_after,
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn review_queue(app: AppHandle, query: QueueQuery) -> Result<ReviewQueue> {
    no_banco(app, move |_, conexao| queue(conexao, &query)).await
}

#[tauri::command]
pub async fn review_apply(app: AppHandle, input: ReviewInput) -> Result<()> {
    no_banco(app, move |_, conexao| apply(conexao, &input)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::deck::{save_card, SaveCardInput};

    fn banco() -> Connection {
        let conexao = Connection::open_in_memory().expect("abriu em memoria");
        db::preparar(&conexao).expect("migrou");
        conexao
    }

    fn fsrs(estado: &str, due: &str) -> FsrsFields {
        FsrsFields {
            due: due.into(),
            stability: 1.0,
            difficulty: 5.0,
            state: estado.into(),
            reps: 1,
            lapses: 0,
            scheduled_days: 1,
            learning_steps: 0,
            last_review: Some("2026-08-07T12:00:00Z".into()),
        }
    }

    /// Salva um card novo (o estado zerado que o `createEmptyCard` produz).
    fn salvar(conexao: &Connection, lemma: &str, frase: &str) -> i64 {
        let entrada = SaveCardInput {
            lemma: lemma.into(),
            form: lemma.into(),
            sentence_en: frase.into(),
            sentence_pt: Some("traducao".into()),
            game_name: Some("Skyrim".into()),
            fsrs: FsrsFields {
                due: "2026-08-08T00:00:00Z".into(),
                stability: 0.0,
                difficulty: 0.0,
                state: "new".into(),
                reps: 0,
                lapses: 0,
                scheduled_days: 0,
                learning_steps: 0,
                last_review: None,
            },
            lookup_id: None,
            line_index: None,
        };
        save_card(conexao, &entrada).expect("salvou").resumo.id
    }

    fn consulta(new_limit: u32) -> QueueQuery {
        QueueQuery {
            now: "2026-08-08T18:00:00Z".into(),
            day_start: "2026-08-08T03:00:00Z".into(),
            new_limit,
        }
    }

    #[test]
    fn fila_vazia_num_deck_vazio() {
        let conexao = banco();
        let fila = queue(&conexao, &consulta(15)).expect("montou a fila");
        assert!(fila.cards.is_empty());
        assert_eq!(fila.due, 0);
        assert_eq!(fila.fresh, 0);
    }

    #[test]
    fn card_novo_entra_na_fila_com_a_frase_dele() {
        let conexao = banco();
        salvar(&conexao, "dread", "A dread silence fell.");
        let fila = queue(&conexao, &consulta(15)).expect("montou a fila");
        assert_eq!(fila.cards.len(), 1);
        assert_eq!(fila.fresh, 1);
        assert_eq!(fila.due, 0);
        let contexto = fila.cards[0].context.as_ref().expect("tem contexto");
        assert_eq!(contexto.sentence_en, "A dread silence fell.");
        assert_eq!(fila.cards[0].contexts, 1);
    }

    #[test]
    fn a_cota_de_novos_limita_quantos_entram() {
        let conexao = banco();
        for i in 0..5 {
            salvar(&conexao, &format!("palavra{i}"), &format!("Frase {i}."));
        }
        let fila = queue(&conexao, &consulta(2)).expect("montou a fila");
        assert_eq!(fila.cards.len(), 2);
        assert_eq!(fila.new_left_over, 3);
    }

    #[test]
    fn card_vencido_entra_e_o_agendado_para_depois_nao() {
        let conexao = banco();
        let vencido = salvar(&conexao, "dread", "A dread silence.");
        let futuro = salvar(&conexao, "grim", "A grim look.");
        aplicar(&conexao, vencido, fsrs("review", "2026-08-07T00:00:00Z"));
        aplicar(&conexao, futuro, fsrs("review", "2026-09-01T00:00:00Z"));

        let fila = queue(&conexao, &consulta(15)).expect("montou a fila");
        assert_eq!(fila.due, 1);
        assert_eq!(fila.cards[0].card.lemma, "dread");
    }

    #[test]
    fn suspenso_nao_entra_na_fila() {
        let conexao = banco();
        let id = salvar(&conexao, "dread", "A dread silence.");
        crate::deck::set_suspended(&conexao, id, true).expect("suspendeu");
        let fila = queue(&conexao, &consulta(15)).expect("montou a fila");
        assert!(fila.cards.is_empty());
    }

    #[test]
    fn introduzir_um_novo_hoje_gasta_a_cota_de_hoje() {
        let conexao = banco();
        let a = salvar(&conexao, "dread", "A dread silence.");
        salvar(&conexao, "grim", "A grim look.");
        aplicar(&conexao, a, fsrs("learning", "2026-09-01T00:00:00Z"));

        let fila = queue(&conexao, &consulta(2)).expect("montou a fila");
        assert_eq!(fila.introduced_today, 1);
        // Cota 2, uma ja gasta: so o segundo card novo entra.
        assert_eq!(fila.cards.len(), 1);
        assert_eq!(fila.cards[0].card.lemma, "grim");
    }

    #[test]
    fn a_nota_reagenda_o_card_e_deixa_uma_linha_no_historico() {
        let conexao = banco();
        let id = salvar(&conexao, "dread", "A dread silence.");
        aplicar(&conexao, id, fsrs("review", "2026-08-20T00:00:00Z"));

        let (due, estado, reps): (String, String, u32) = conexao
            .query_row(
                "SELECT fsrs_due, fsrs_state, fsrs_reps FROM cards WHERE id = ?1",
                [id],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .expect("leu o card");
        assert_eq!(due, "2026-08-20T00:00:00Z");
        assert_eq!(estado, "review");
        assert_eq!(reps, 1);

        let linhas: u32 = conexao
            .query_row(
                "SELECT COUNT(*) FROM review_log WHERE card_id = ?1",
                [id],
                |l| l.get(0),
            )
            .expect("contou o log");
        assert_eq!(linhas, 1);
    }

    #[test]
    fn nota_em_card_inexistente_falha_sem_gravar_log() {
        let conexao = banco();
        let entrada = ReviewInput {
            card_id: 999,
            fsrs: fsrs("review", "2026-08-20T00:00:00Z"),
            log: log_de("new", "review"),
        };
        assert!(apply(&conexao, &entrada).is_err());
        let linhas: u32 = conexao
            .query_row("SELECT COUNT(*) FROM review_log", [], |l| l.get(0))
            .expect("contou o log");
        assert_eq!(linhas, 0);
    }

    fn log_de(antes: &str, depois: &str) -> ReviewLogEntry {
        ReviewLogEntry {
            reviewed_at: "2026-08-08T18:00:00Z".into(),
            rating: 3,
            elapsed_days: 1.0,
            state_before: antes.into(),
            state_after: depois.into(),
        }
    }

    fn aplicar(conexao: &Connection, card_id: i64, novo: FsrsFields) {
        let depois = novo.state.clone();
        let entrada = ReviewInput {
            card_id,
            fsrs: novo,
            log: log_de("new", &depois),
        };
        apply(conexao, &entrada).expect("aplicou a nota");
    }
}
