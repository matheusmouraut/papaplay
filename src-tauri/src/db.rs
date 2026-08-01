//! Banco do usuario: SQLite em `%APPDATA%/papaplay/papaplay.db`.
//!
//! E o banco que **acumula** — deck, contextos, historico de revisao. O
//! dicionario e outro arquivo, read-only e descartavel ([`crate::dict`]).
//!
//! # Migrations
//!
//! Numeradas em `src-tauri/migrations/`, embutidas no binario por
//! `include_str!` e aplicadas na ordem, cada uma dentro de uma transacao. O
//! controle e o `PRAGMA user_version`: ele guarda o numero da ultima migration
//! aplicada, entao abrir um banco ja atualizado nao custa nada e abrir um banco
//! antigo aplica so o que falta.
//!
//! Migrations sao **so-ida** (regra 3 do CLAUDE.md). Nao existe rollback: o
//! banco do usuario e insubstituivel, e desfazer schema com dados dentro e como
//! se perde deck.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

/// Variavel de ambiente que sobrescreve o caminho do banco. Existe para os
/// testes e para rodar duas instalacoes lado a lado sem misturar os decks.
const DB_ENV: &str = "PAPAPLAY_DB";

/// As migrations, em ordem. Numero = posicao na lista, comecando em 1, que e o
/// valor que vai para o `user_version`.
const MIGRATIONS: &[(&str, &str)] = &[("0001_deck", include_str!("../migrations/0001_deck.sql"))];

static CONEXAO: Mutex<Option<Connection>> = Mutex::new(None);

/// Caminho do banco: variavel de ambiente ou `%APPDATA%/papaplay/papaplay.db`.
///
/// O diretorio segue o CLAUDE.md (`papaplay`), nao o identificador do bundle
/// (`com.papaplay.app`) que o Tauri usaria por padrao: quem vai atras do proprio
/// deck procura pelo nome do app.
pub fn caminho(app: &AppHandle) -> Result<PathBuf> {
    if let Some(bruto) = std::env::var_os(DB_ENV) {
        return Ok(PathBuf::from(bruto));
    }
    let dir = app
        .path()
        .data_dir()
        .map_err(|e| Error::Deck(format!("diretorio de dados indisponivel: {e}")))?
        .join("papaplay");
    Ok(dir.join("papaplay.db"))
}

/// Abre (criando o arquivo e o diretorio, se preciso) e migra.
fn abrir(caminho: &std::path::Path) -> Result<Connection> {
    if let Some(pai) = caminho.parent() {
        std::fs::create_dir_all(pai)?;
    }
    let conexao = Connection::open(caminho)
        .map_err(|e| Error::Deck(format!("nao abriu {}: {e}", caminho.display())))?;
    preparar(&conexao)?;
    Ok(conexao)
}

/// Pragmas + migrations. Separado de [`abrir`] porque os testes rodam isto num
/// banco em memoria, sem tocar em disco.
pub fn preparar(conexao: &Connection) -> Result<()> {
    // As FKs do SQLite sao opt-in *por conexao*: sem isto os `ON DELETE
    // CASCADE` da migration seriam decoracao e apagar um card deixaria os
    // contextos dele orfaos.
    conexao.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrar(conexao)
}

/// Aplica as migrations que faltam, na ordem.
fn migrar(conexao: &Connection) -> Result<()> {
    let atual: i64 = conexao.query_row("PRAGMA user_version", [], |linha| linha.get(0))?;
    let atual = atual.max(0) as usize;
    if atual > MIGRATIONS.len() {
        return Err(Error::Deck(format!(
            "banco na versao {atual}, mas este app so conhece ate {}. \
             Provavelmente e de uma versao mais nova do PapaPlay.",
            MIGRATIONS.len()
        )));
    }

    for (indice, (nome, sql)) in MIGRATIONS.iter().enumerate().skip(atual) {
        let versao = indice + 1;
        // `user_version` nao aceita parametro ligado — daí o format!. O valor e
        // um indice nosso, nunca entrada do usuario.
        conexao
            .execute_batch(&format!(
                "BEGIN; {sql} PRAGMA user_version = {versao}; COMMIT;"
            ))
            .map_err(|e| Error::Deck(format!("migration {nome} falhou: {e}")))?;
    }
    Ok(())
}

/// Conexao unica do processo, aberta na primeira chamada.
///
/// Uma so porque a escrita em SQLite e serializada de qualquer jeito e o app
/// escreve pouco: o gargalo do produto e OCR e traducao, nao o banco.
pub fn conexao(app: &AppHandle) -> Result<MutexGuard<'static, Option<Connection>>> {
    let mut guarda = CONEXAO
        .lock()
        .map_err(|_| Error::Deck("banco envenenado".into()))?;
    if guarda.is_none() {
        *guarda = Some(abrir(&caminho(app)?)?);
    }
    Ok(guarda)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Banco em memoria ja migrado — o ponto de partida dos testes do deck.
    pub fn memoria() -> Connection {
        let conexao = Connection::open_in_memory().expect("abriu em memoria");
        preparar(&conexao).expect("migrou");
        conexao
    }

    fn tabelas(conexao: &Connection) -> Vec<String> {
        let mut stmt = conexao
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .expect("consultou o schema");
        let nomes = stmt
            .query_map([], |linha| linha.get::<_, String>(0))
            .expect("leu os nomes")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("nomes validos");
        nomes
    }

    #[test]
    fn migrations_criam_o_schema_do_doc_04() {
        let conexao = memoria();
        let tabelas = tabelas(&conexao);
        for esperada in ["card_senses", "cards", "contexts", "review_log", "settings"] {
            assert!(
                tabelas.iter().any(|t| t == esperada),
                "faltou a tabela {esperada}; existem {tabelas:?}"
            );
        }
    }

    #[test]
    fn user_version_marca_a_ultima_migration() {
        let conexao = memoria();
        let versao: i64 = conexao
            .query_row("PRAGMA user_version", [], |linha| linha.get(0))
            .expect("leu a versao");
        assert_eq!(versao as usize, MIGRATIONS.len());
    }

    #[test]
    fn migrar_de_novo_nao_faz_nada() {
        // O caminho real de toda abertura depois da primeira: se as migrations
        // nao fossem puladas, o app quebraria no segundo boot.
        let conexao = memoria();
        conexao
            .execute(
                "INSERT INTO settings (key, value) VALUES ('teste', 'sobrevive')",
                [],
            )
            .expect("gravou");

        preparar(&conexao).expect("migrou de novo");

        let valor: String = conexao
            .query_row("SELECT value FROM settings WHERE key = 'teste'", [], |l| {
                l.get(0)
            })
            .expect("leu de volta");
        assert_eq!(valor, "sobrevive");
    }

    #[test]
    fn banco_de_versao_futura_e_recusado() {
        let conexao = Connection::open_in_memory().expect("abriu");
        conexao
            .execute_batch("PRAGMA user_version = 99;")
            .expect("marcou versao futura");
        let erro = preparar(&conexao).expect_err("deve recusar");
        assert!(
            erro.to_string().contains("versao mais nova"),
            "mensagem precisa dizer o que houve: {erro}"
        );
    }

    #[test]
    fn abrir_cria_o_diretorio_e_o_arquivo() {
        // O que os testes em memoria nao cobrem: na primeira execucao
        // `%APPDATA%/papaplay/` ainda nao existe, e falhar aqui significaria um
        // app que so quebra na maquina de quem acabou de instalar.
        let unico = std::process::id();
        let dir = std::env::temp_dir().join(format!("papaplay-teste-{unico}/papaplay"));
        let _ = std::fs::remove_dir_all(&dir);
        let caminho = dir.join("papaplay.db");

        let conexao = abrir(&caminho).expect("abriu no disco");
        assert!(caminho.is_file(), "o arquivo do banco deveria existir");

        let versao: i64 = conexao
            .query_row("PRAGMA user_version", [], |linha| linha.get(0))
            .expect("leu a versao");
        assert_eq!(versao as usize, MIGRATIONS.len(), "migrou ao abrir");

        drop(conexao);
        let _ = std::fs::remove_dir_all(dir.parent().expect("tem pai"));
    }

    #[test]
    fn as_chaves_estrangeiras_estao_ligadas() {
        let conexao = memoria();
        let erro = conexao
            .execute(
                "INSERT INTO contexts (card_id, form, sentence_en, captured_at)
                 VALUES (999, 'ran', 'He ran away.', '2026-08-01T00:00:00Z')",
                [],
            )
            .expect_err("card 999 nao existe");
        assert!(
            erro.to_string().to_lowercase().contains("foreign key"),
            "esperado erro de FK, veio: {erro}"
        );
    }
}
