//! Erro unico do core, serializavel para a UI.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Funcionalidade ainda nao implementada (scaffold).
    #[error("nao implementado: {0}")]
    NotImplemented(&'static str),

    /// Uma janela declarada em tauri.conf.json nao existe mais em runtime.
    #[error("janela nao encontrada: {0}")]
    WindowNotFound(&'static str),

    /// Falha de uma API do Windows (monitor, foco, titulo da janela).
    #[error("api do windows falhou: {0}")]
    Platform(String),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    #[cfg(desktop)]
    #[error(transparent)]
    GlobalShortcut(#[from] tauri_plugin_global_shortcut::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A UI recebe apenas a mensagem, nunca a estrutura interna do erro.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
