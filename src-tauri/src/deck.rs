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

/// Cria o card (ou anexa um contexto novo se o lema ja existe).
///
/// Salvar a mesma frase duas vezes no mesmo card e engano de clique, nao
/// contexto novo: o indice unico de `contexts` absorve a repeticao em silencio.
pub fn save_card(conexao: &Connection, entrada: &SaveCardInput) -> Result<CardSummary> {
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

    conexao.execute(
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

    Ok(CardSummary {
        id: card_id,
        lemma: lemma.to_string(),
        contexts: contar_contextos(conexao, card_id)?,
        created: criado,
    })
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

/// Cards com `fsrs_due` no passado, na ordem em que devem ser revisados.
pub fn due_cards(_conexao: &Connection, _limit: u32) -> Result<Vec<CardSummary>> {
    Err(Error::NotImplemented("deck::due_cards"))
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

/// `async` pelo mesmo motivo do `dict_lookup`: a main thread desenha a overlay
/// e nao pode esperar disco.
#[tauri::command]
pub async fn deck_save_card(app: AppHandle, input: SaveCardInput) -> Result<CardSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        com_conexao(&app, |conexao| save_card(conexao, &input))
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
        }
    }

    #[test]
    fn salvar_cria_o_card_com_um_contexto() {
        let conexao = banco();
        let resumo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        assert!(resumo.created);
        assert_eq!(resumo.lemma, "run");
        assert_eq!(resumo.contexts, 1);
    }

    #[test]
    fn a_mesma_palavra_em_outra_frase_vira_contexto_do_mesmo_card() {
        // O coracao da decisao "um card por lema": duas flexoes, um card.
        let conexao = banco();
        let primeiro = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        let segundo = save_card(&conexao, &entrada("run", "running", "She is running."))
            .expect("salvou de novo");

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
        assert_eq!(repetido.contexts, 1, "clique repetido nao vira contexto");
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

    #[test]
    fn apagar_o_card_leva_os_contextos_junto() {
        let conexao = banco();
        let resumo = save_card(&conexao, &entrada("run", "ran", "He ran away.")).expect("salvou");
        conexao
            .execute("DELETE FROM cards WHERE id = ?1", [resumo.id])
            .expect("apagou");
        let sobraram: u32 = conexao
            .query_row("SELECT COUNT(*) FROM contexts", [], |l| l.get(0))
            .expect("contou");
        assert_eq!(sobraram, 0);
    }
}
