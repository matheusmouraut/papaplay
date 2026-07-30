//! Erro unico do core, serializavel para a UI.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Funcionalidade ainda nao implementada (scaffold).
    #[error("nao implementado: {0}")]
    NotImplemented(&'static str),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A UI recebe apenas a mensagem, nunca a estrutura interna do erro.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
