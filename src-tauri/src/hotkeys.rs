//! Atalhos globais (tauri-plugin-global-shortcut).
//!
//! O atalho de lookup precisa funcionar com o jogo em foco, por isso e global
//! e nao um listener da janela.

use tauri::AppHandle;

use crate::error::Result;

/// Atalho padrao para entrar/sair do modo lookup. Configuravel depois.
pub const DEFAULT_LOOKUP_SHORTCUT: &str = "Alt+D";

/// Registra os atalhos globais no start do app.
pub fn register(_app: &AppHandle) -> Result<()> {
    // Implementado junto com a spike de overlay/click-through.
    Ok(())
}
