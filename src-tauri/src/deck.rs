//! Deck do usuario: SQLite em `%APPDATA%/papaplay/papaplay.db` + pasta `media/`.
//!
//! Um card por lema; contextos multiplos anexados ao mesmo card.
//! Os campos `fsrs_*` sao escritos apenas com valores vindos do wrapper
//! `src/shared/srs/` na UI — o core nao calcula agendamento.
//! Schema evolui somente via migration numerada (skill /new-migration).

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSummary {
    pub id: i64,
    pub lemma: String,
    pub contexts: u32,
}

/// Cria o card (ou anexa um contexto novo se o lema ja existe).
pub fn save_card(_input: SaveCardInput) -> Result<CardSummary> {
    Err(Error::NotImplemented("deck::save_card"))
}

/// Cards com `fsrs_due` no passado, na ordem em que devem ser revisados.
pub fn due_cards(_limit: u32) -> Result<Vec<CardSummary>> {
    Err(Error::NotImplemented("deck::due_cards"))
}
