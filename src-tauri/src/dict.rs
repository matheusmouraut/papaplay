//! Dicionario offline: SQLite read-only embarcado em `resources/dict.db`.
//!
//! Origem dos dados: Wiktionary EN via kaikki.org/wiktextract + Wiktionary PT +
//! wordfreq. Licenca CC BY-SA — a atribuicao precisa aparecer na tela "Sobre",
//! e esta gravada na tabela `meta` do proprio banco.
//! Pipeline de build: `pnpm run build:dict` (ver `scripts/build-dict.mjs`).
//!
//! # Como uma palavra da tela vira verbete
//!
//! O OCR entrega o que estava escrito, com pontuacao colada e caixa qualquer:
//! `"Quest,"`, `"Sun's"`, `"RAN"`. A busca tenta, em ordem:
//!
//! 1. a palavra normalizada, como lema;
//! 2. o lema dela via `lemma_forms` (`"ran"` -> `"run"`);
//! 3. o mesmo, sem o possessivo (`"Sun's"` -> `"sun"`).
//!
//! Quando uma forma serve a mais de um lema (`"left"` vem de `"leave"` e
//! tambem e lema proprio), ganha o lema que tem verbete e melhor frequencia.

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

/// Variavel de ambiente que sobrescreve o caminho do dict.db.
const DICT_ENV: &str = "PAPAPLAY_DICT_DB";

/// Abrir o banco custa pouco, mas nao a cada palavra sob o cursor: o tooltip
/// tem orcamento de 300 ms (doc 03).
static CONEXAO: Mutex<Option<Connection>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sense {
    pub pos: String,
    pub gloss_pt: String,
    pub gloss_en: Option<String>,
    pub examples: Vec<String>,
}

/// Acepcao como esta gravada no `senses_json`.
///
/// Sem `pos`: a classe gramatical e coluna da linha e vale para todas as
/// acepcoes dela — repeti-la em cada acepcao inflaria o banco a toa.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcepcaoGravada {
    gloss_pt: String,
    gloss_en: Option<String>,
    #[serde(default)]
    examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictEntry {
    pub lemma: String,
    pub ipa: Option<String>,
    pub senses: Vec<Sense>,
    /// Posicao na lista de frequencia; menor = mais comum.
    pub freq_rank: Option<u32>,
    /// A forma que estava na tela, quando diferente do lema. E o que deixa o
    /// popup dizer "ran → run" em vez de so "run".
    pub matched_form: Option<String>,
}

/// Tira a pontuacao das bordas e baixa a caixa.
///
/// So as bordas: apostrofo e hifen no meio sao parte da palavra (`didn't`,
/// `well-known`), e a spike 02 mostrou que o OCR os preserva.
pub fn normalizar(bruto: &str) -> String {
    bruto
        .chars()
        .map(|c| if c == '\u{2019}' { '\'' } else { c })
        .collect::<String>()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// Grafias a tentar, da mais literal para a mais processada.
fn candidatas(palavra: &str) -> Vec<String> {
    let mut lista = vec![palavra.to_string()];
    // Possessivo: o dicionario tem "sun", nunca "sun's".
    if let Some(base) = palavra
        .strip_suffix("'s")
        .or_else(|| palavra.strip_suffix("s'"))
    {
        if !base.is_empty() {
            lista.push(base.to_string());
        }
    }
    lista
}

/// Lema de uma forma flexionada, se houver.
///
/// Prefere o lema que tem verbete; entre varios, o mais frequente. Sem isso
/// `"left"` cairia num lema qualquer dos que a tabela oferece.
fn lema_de(conn: &Connection, forma: &str) -> Result<Option<String>> {
    let lema = conn
        .prepare_cached(
            "SELECT f.lemma
               FROM lemma_forms f
               LEFT JOIN dict_entries e ON e.lemma = f.lemma
              WHERE f.form = ?1
              ORDER BY (e.id IS NULL), COALESCE(e.freq_rank, 2147483647)
              LIMIT 1",
        )?
        .query_row([forma], |linha| linha.get::<_, String>(0))
        .optional()?;
    Ok(lema)
}

/// Junta todas as linhas de um lema (uma por classe gramatical) num verbete.
fn verbete(conn: &Connection, lema: &str, forma: Option<&str>) -> Result<Option<DictEntry>> {
    let mut consulta = conn.prepare_cached(
        "SELECT lemma, pos, ipa, senses_json, freq_rank
           FROM dict_entries
          WHERE lemma = ?1
          ORDER BY id",
    )?;

    let linhas = consulta.query_map([lema], |linha| {
        Ok((
            linha.get::<_, String>(0)?,
            linha.get::<_, String>(1)?,
            linha.get::<_, Option<String>>(2)?,
            linha.get::<_, String>(3)?,
            linha.get::<_, Option<u32>>(4)?,
        ))
    })?;

    let mut entrada: Option<DictEntry> = None;
    for linha in linhas {
        let (lemma, pos, ipa, senses_json, freq_rank) = linha?;
        // JSON quebrado numa linha nao pode derrubar a consulta inteira: a
        // palavra ainda pode ter outras classes gramaticais boas.
        let gravadas: Vec<AcepcaoGravada> = serde_json::from_str(&senses_json).unwrap_or_default();
        let acepcoes = gravadas.into_iter().map(|a| Sense {
            pos: pos.clone(),
            gloss_pt: a.gloss_pt,
            gloss_en: a.gloss_en,
            examples: a.examples,
        });

        match &mut entrada {
            Some(atual) => {
                atual.senses.extend(acepcoes);
                atual.ipa = atual.ipa.take().or(ipa);
                atual.freq_rank = atual.freq_rank.or(freq_rank);
            }
            None => {
                entrada = Some(DictEntry {
                    lemma,
                    ipa,
                    senses: acepcoes.collect(),
                    freq_rank,
                    matched_form: forma.map(str::to_string),
                })
            }
        }
    }

    Ok(entrada.filter(|e| !e.senses.is_empty()))
}

/// Busca o verbete de uma palavra vinda da tela. Testavel: recebe a conexao.
fn buscar(conn: &Connection, bruto: &str) -> Result<Option<DictEntry>> {
    let palavra = normalizar(bruto);
    if palavra.is_empty() {
        return Ok(None);
    }

    for candidata in candidatas(&palavra) {
        // A forma so vira `matched_form` quando difere do que estava na tela —
        // senao o popup mostraria "run → run".
        let forma = (candidata != palavra).then_some(palavra.as_str());
        if let Some(entrada) = verbete(conn, &candidata, forma)? {
            return Ok(Some(entrada));
        }
        if let Some(lema) = lema_de(conn, &candidata)? {
            if let Some(entrada) = verbete(conn, &lema, Some(&palavra))? {
                return Ok(Some(entrada));
            }
        }
    }

    Ok(None)
}

/// Reduz uma forma flexionada ao lema ("ran" -> "run").
///
/// Devolve a propria palavra normalizada quando nao ha lema conhecido — quem
/// chama quer uma chave de card, e a palavra crua e a melhor que existe.
pub fn lemmatize(app: &AppHandle, form: &str) -> Result<String> {
    let palavra = normalizar(form);
    com_conexao(app, |conn| {
        for candidata in candidatas(&palavra) {
            if verbete(conn, &candidata, None)?.is_some() {
                return Ok(candidata);
            }
            if let Some(lema) = lema_de(conn, &candidata)? {
                return Ok(lema);
            }
        }
        Ok(palavra.clone())
    })
}

/// Busca as acepcoes de uma palavra, lematizando antes.
pub fn lookup(app: &AppHandle, word: &str) -> Result<Option<DictEntry>> {
    com_conexao(app, |conn| buscar(conn, word))
}

/// Abre o dicionario numa thread de fundo, no boot.
///
/// Uma busca de verdade junto: abrir o arquivo e barato, mas a primeira
/// consulta ainda paga o `prepare` das duas consultas e a leitura das paginas
/// de indice do disco. Fazer isso antes tira esse custo do primeiro tooltip,
/// que e o momento em que o produto tem 300 ms para responder (doc 03).
///
/// Silencioso: se o dicionario faltar, quem avisa e a primeira consulta de
/// verdade, com a mensagem completa.
pub fn preload(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = lookup(&app, "the");
    });
}

// ---------------------------------------------------------------------------
// Conexao
// ---------------------------------------------------------------------------

fn com_conexao<T>(app: &AppHandle, acao: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let mut guarda = CONEXAO
        .lock()
        .map_err(|_| Error::Dict("conexao do dicionario envenenada".into()))?;
    if guarda.is_none() {
        *guarda = Some(abrir(&caminho_do_banco(app)?)?);
    }
    acao(guarda.as_ref().expect("conexao acabou de ser aberta"))
}

fn abrir(caminho: &PathBuf) -> Result<Connection> {
    // Read-only de verdade: o dicionario e artefato de build e nunca deve ser
    // escrito em runtime, nem por acidente.
    Connection::open_with_flags(
        caminho,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| Error::Dict(format!("nao foi possivel abrir {}: {e}", caminho.display())))
}

/// Caminho do dict.db: variavel de ambiente, recursos do app, arvore do repo.
fn caminho_do_banco(app: &AppHandle) -> Result<PathBuf> {
    if let Some(bruto) = std::env::var_os(DICT_ENV) {
        return Ok(PathBuf::from(bruto));
    }
    if let Ok(recursos) = app.path().resource_dir() {
        let candidato = recursos.join("dict.db");
        if candidato.is_file() {
            return Ok(candidato);
        }
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dict.db");
    if repo.is_file() {
        return Ok(repo);
    }
    Err(Error::Dict(format!(
        "dict.db nao encontrado — rode `pnpm run build:dict` ou aponte {DICT_ENV} para o arquivo"
    )))
}

/// Fecha o banco. Existe para a tela de configuracoes poder soltar o arquivo.
pub fn close() {
    if let Ok(mut guarda) = CONEXAO.lock() {
        *guarda = None;
    }
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn dict_lookup(app: AppHandle, word: String) -> Result<Option<DictEntry>> {
    tauri::async_runtime::spawn_blocking(move || lookup(&app, &word))
        .await
        .map_err(|e| Error::Dict(format!("consulta ao dicionario abortada: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Banco em memoria com o mesmo esquema do `build-dict.mjs`.
    fn banco() -> Connection {
        let conn = Connection::open_in_memory().expect("sqlite em memoria");
        conn.execute_batch(
            "CREATE TABLE dict_entries (
               id INTEGER PRIMARY KEY,
               lemma TEXT NOT NULL COLLATE NOCASE,
               pos TEXT NOT NULL,
               ipa TEXT,
               senses_json TEXT NOT NULL,
               freq_rank INTEGER
             );
             CREATE TABLE lemma_forms (
               form TEXT NOT NULL COLLATE NOCASE,
               lemma TEXT NOT NULL COLLATE NOCASE,
               PRIMARY KEY (form, lemma)
             ) WITHOUT ROWID;",
        )
        .expect("esquema");
        conn
    }

    fn inserir(
        conn: &Connection,
        lemma: &str,
        pos: &str,
        ipa: Option<&str>,
        senses: &str,
        freq: Option<u32>,
    ) {
        conn.execute(
            "INSERT INTO dict_entries (lemma, pos, ipa, senses_json, freq_rank)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![lemma, pos, ipa, senses, freq],
        )
        .expect("insert verbete");
    }

    fn forma(conn: &Connection, form: &str, lemma: &str) {
        conn.execute(
            "INSERT INTO lemma_forms (form, lemma) VALUES (?1, ?2)",
            [form, lemma],
        )
        .expect("insert forma");
    }

    fn so_glosa(pt: &str) -> String {
        format!(r#"[{{"glossPt":"{pt}","glossEn":null,"examples":[]}}]"#)
    }

    const CORRER: &str =
        r#"[{"glossPt":"correr","glossEn":"to move fast","examples":["He ran home."]}]"#;

    fn base() -> Connection {
        let conn = banco();
        inserir(&conn, "run", "verb", Some("/ɹʌn/"), CORRER, Some(300));
        inserir(&conn, "run", "noun", None, &so_glosa("corrida"), Some(300));
        forma(&conn, "ran", "run");
        forma(&conn, "running", "run");
        conn
    }

    #[test]
    fn normaliza_pontuacao_de_borda_e_caixa() {
        assert_eq!(normalizar("  Quest,  "), "quest");
        assert_eq!(normalizar("\"RAN\"!"), "ran");
        assert_eq!(normalizar("(ledge)"), "ledge");
    }

    #[test]
    fn normalizacao_preserva_apostrofo_e_hifen_internos() {
        // A spike 02 mostrou que o OCR entrega contracoes inteiras; quebra-las
        // aqui perderia justamente a palavra que o usuario quer.
        assert_eq!(normalizar("didn't"), "didn't");
        assert_eq!(normalizar("well-known."), "well-known");
    }

    #[test]
    fn normalizacao_troca_apostrofo_tipografico() {
        // Fontes de jogo usam ’ e o dicionario guarda '.
        assert_eq!(normalizar("didn\u{2019}t"), "didn't");
    }

    #[test]
    fn palavra_so_de_pontuacao_vira_vazio() {
        assert_eq!(normalizar("---"), "");
        assert_eq!(normalizar("..."), "");
    }

    #[test]
    fn busca_direta_junta_as_classes_gramaticais_num_verbete() {
        let conn = base();
        let e = buscar(&conn, "run").expect("consulta").expect("verbete");
        assert_eq!(e.lemma, "run");
        assert_eq!(e.senses.len(), 2);
        assert_eq!(e.senses[0].pos, "verb");
        assert_eq!(e.senses[1].pos, "noun");
        // O IPA vem da primeira linha que tiver um.
        assert_eq!(e.ipa.as_deref(), Some("/ɹʌn/"));
        assert!(e.matched_form.is_none(), "a palavra veio como lema");
    }

    #[test]
    fn busca_lematiza_e_registra_a_forma_da_tela() {
        let conn = base();
        let e = buscar(&conn, "Ran.").expect("consulta").expect("verbete");
        assert_eq!(e.lemma, "run");
        assert_eq!(e.matched_form.as_deref(), Some("ran"));
    }

    #[test]
    fn busca_ignora_caixa_e_pontuacao() {
        let conn = base();
        assert!(buscar(&conn, "RUNNING!").expect("consulta").is_some());
    }

    #[test]
    fn possessivo_cai_no_lema_base() {
        let conn = banco();
        inserir(&conn, "sun", "noun", None, &so_glosa("sol"), Some(900));
        let e = buscar(&conn, "Sun's").expect("consulta").expect("verbete");
        assert_eq!(e.lemma, "sun");
        assert_eq!(e.matched_form.as_deref(), Some("sun's"));
    }

    #[test]
    fn forma_ambigua_prefere_o_lema_com_verbete() {
        // "left" aponta para "leave" (que tem verbete) e para "lefting" (que
        // nao tem). Sem a preferencia, a consulta poderia devolver nada.
        let conn = banco();
        inserir(&conn, "leave", "verb", None, &so_glosa("partir"), Some(500));
        forma(&conn, "left", "lefting");
        forma(&conn, "left", "leave");
        let e = buscar(&conn, "left").expect("consulta").expect("verbete");
        assert_eq!(e.lemma, "leave");
    }

    #[test]
    fn forma_ambigua_desempata_pela_frequencia() {
        let conn = banco();
        inserir(&conn, "raro", "noun", None, &so_glosa("raro"), Some(90000));
        inserir(&conn, "comum", "noun", None, &so_glosa("comum"), Some(120));
        forma(&conn, "x", "raro");
        forma(&conn, "x", "comum");
        let e = buscar(&conn, "x").expect("consulta").expect("verbete");
        assert_eq!(e.lemma, "comum", "o lema mais frequente ganha");
    }

    #[test]
    fn palavra_que_e_lema_nao_e_lematizada() {
        // "left" tambem e lema proprio (adjetivo): nesse caso vale o verbete
        // dele, nao o de "leave".
        let conn = banco();
        inserir(&conn, "left", "adj", None, &so_glosa("esquerdo"), Some(400));
        inserir(&conn, "leave", "verb", None, &so_glosa("partir"), Some(500));
        forma(&conn, "left", "leave");
        let e = buscar(&conn, "left").expect("consulta").expect("verbete");
        assert_eq!(e.lemma, "left");
        assert_eq!(e.senses[0].gloss_pt, "esquerdo");
    }

    #[test]
    fn palavra_desconhecida_devolve_nada_em_vez_de_erro() {
        let conn = base();
        assert!(buscar(&conn, "zzzznaoexiste").expect("consulta").is_none());
        assert!(buscar(&conn, "!!!").expect("consulta").is_none());
    }

    #[test]
    fn verbete_sem_acepcao_nao_conta_como_achado() {
        let conn = banco();
        inserir(&conn, "vazio", "noun", None, "[]", None);
        assert!(buscar(&conn, "vazio").expect("consulta").is_none());
    }

    #[test]
    fn json_quebrado_numa_classe_nao_derruba_as_outras() {
        let conn = banco();
        inserir(&conn, "meio", "noun", None, "{isto nao e json", None);
        inserir(&conn, "meio", "verb", None, &so_glosa("meio"), None);
        let e = buscar(&conn, "meio").expect("consulta").expect("verbete");
        assert_eq!(e.senses.len(), 1);
        assert_eq!(e.senses[0].pos, "verb");
    }

    /// Le o dict.db de verdade, quando ele existe na arvore do repo.
    ///
    /// Os testes acima usam um banco em memoria montado a mao, entao nenhum
    /// deles pega o risco real desta feature: o esquema e escrito por um script
    /// em JavaScript e lido por Rust. Uma divergencia entre os dois passa por
    /// todos os outros testes e so aparece em runtime.
    ///
    /// Fica silencioso quando o banco nao foi construido — `pnpm run build:dict`
    /// baixa 500 MB e nao pode ser pre-requisito de `cargo test`.
    #[test]
    fn dict_db_de_verdade_responde_as_palavras_do_roteiro() {
        let caminho = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dict.db");
        if !caminho.is_file() {
            eprintln!("pulando: {} nao existe", caminho.display());
            return;
        }
        let conn = abrir(&caminho).expect("abrir dict.db");

        for palavra in ["run", "dread", "ledge", "quest", "give up"] {
            let e = buscar(&conn, palavra)
                .expect("consulta")
                .unwrap_or_else(|| panic!("dict.db nao tem \"{palavra}\""));
            assert!(
                !e.senses[0].gloss_pt.is_empty(),
                "\"{palavra}\" sem glosa em portugues"
            );
        }

        // Lematizacao ponta a ponta, que e o que o roteiro da spike cobra.
        let told = buscar(&conn, "Told.").expect("consulta").expect("told");
        assert_eq!(told.lemma.to_lowercase(), "tell");
        assert_eq!(told.matched_form.as_deref(), Some("told"));
    }

    #[test]
    fn exemplos_e_glosa_inglesa_chegam_inteiros() {
        let conn = base();
        let e = buscar(&conn, "run").expect("consulta").expect("verbete");
        assert_eq!(e.senses[0].gloss_en.as_deref(), Some("to move fast"));
        assert_eq!(e.senses[0].examples, vec!["He ran home."]);
        assert_eq!(e.freq_rank, Some(300));
    }
}
