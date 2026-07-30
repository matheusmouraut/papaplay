//! Dicionario offline: SQLite read-only embarcado em `resources/dict.db`.
//!
//! Origem dos dados: Wiktionary EN via kaikki.org/wiktextract + wordfreq.
//! Licenca CC BY-SA — a atribuicao precisa aparecer na tela "Sobre".
//! Pipeline de build: `pnpm run build:dict` (ver skill /build-dict).

use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sense {
    pub pos: String,
    pub gloss_pt: String,
    pub gloss_en: Option<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictEntry {
    pub lemma: String,
    pub ipa: Option<String>,
    pub senses: Vec<Sense>,
    /// Posicao na lista de frequencia; menor = mais comum.
    pub freq_rank: Option<u32>,
}

/// Reduz uma forma flexionada ao lema ("ran" -> "run") via `lemma_forms`.
pub fn lemmatize(_form: &str) -> Result<String> {
    Err(Error::NotImplemented("dict::lemmatize"))
}

/// Busca as acepcoes de uma palavra, lematizando antes.
pub fn lookup(_word: &str) -> Result<Option<DictEntry>> {
    Err(Error::NotImplemented("dict::lookup"))
}
