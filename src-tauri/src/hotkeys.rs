//! Atalhos globais (tauri-plugin-global-shortcut).
//!
//! O atalho de lookup precisa funcionar com o jogo em foco, por isso e global
//! e nao um listener da janela.
//!
//! `Esc` merece atencao: um Esc global permanente seria **roubado do jogo**
//! (menu de pausa pararia de abrir). Por isso ele so fica registrado enquanto
//! a overlay esta em modo lookup — ver [`sync_escape`].

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::error::Result;
use crate::overlay;

/// Atalho padrao para entrar/sair do modo lookup. Configuravel depois.
pub const DEFAULT_LOOKUP_SHORTCUT: &str = "Alt+X";

/// Alterna passivo <-> lookup.
pub fn lookup_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::KeyX)
}

/// Volta para passivo, sempre.
pub fn escape_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

/// Roda a troca de modo fora da thread de hotkeys.
///
/// `set_mode` faz dispatch bloqueante para o event loop e pode (re)registrar
/// atalhos; fazer isso dentro do callback do proprio atalho trava a fila de
/// hotkeys do processo.
fn spawn_set_mode(app: &AppHandle, interactive: Option<bool>) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = match interactive {
            Some(value) => overlay::set_mode(&app, value),
            None => overlay::toggle(&app),
        };
        if let Err(err) = result {
            eprintln!("[hotkeys] falha ao alternar a overlay: {err}");
        }
    });
}

/// Registra os atalhos globais no start do app.
pub fn register(app: &AppHandle) -> Result<()> {
    app.global_shortcut()
        .on_shortcut(lookup_shortcut(), |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                spawn_set_mode(app, None);
            }
        })?;
    Ok(())
}

/// Mantem o `Esc` global registrado apenas em modo lookup.
///
/// Idempotente: chamada em toda troca de modo, so age quando o estado do
/// registro diverge do modo pedido.
pub fn sync_escape(app: &AppHandle, interactive: bool) -> Result<()> {
    let manager = app.global_shortcut();
    let shortcut = escape_shortcut();
    match (interactive, manager.is_registered(shortcut)) {
        (true, false) => manager.on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                spawn_set_mode(app, Some(false));
            }
        })?,
        (false, true) => manager.unregister(shortcut)?,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atalho_de_lookup_bate_com_a_constante_documentada() {
        let shortcut = lookup_shortcut();
        assert_eq!(shortcut.key, Code::KeyX);
        assert!(shortcut.mods.contains(Modifiers::ALT));
        assert_eq!(DEFAULT_LOOKUP_SHORTCUT, "Alt+X");
    }

    #[test]
    fn escape_nao_tem_modificador() {
        let shortcut = escape_shortcut();
        assert_eq!(shortcut.key, Code::Escape);
        assert!(shortcut.mods.is_empty());
    }
}
