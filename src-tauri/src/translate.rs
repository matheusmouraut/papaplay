//! Traducao de frases en->pt-BR offline (Marian/Bergamot, modelos OPUS-MT).
//!
//! O modelo e carregado sob demanda (lazy) para segurar a RAM total < 400MB
//! enquanto o overlay esta passivo.

use crate::error::{Error, Result};

/// Traduz uma frase curta de ingles para portugues do Brasil.
pub fn translate_sentence(_text: &str) -> Result<String> {
    Err(Error::NotImplemented("translate::translate_sentence"))
}

/// Libera o modelo NMT da memoria quando o overlay volta ao estado passivo.
pub fn unload_model() {}
