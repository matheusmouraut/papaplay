//! Atalhos globais (tauri-plugin-global-shortcut).
//!
//! O atalho da espiada precisa funcionar com o jogo em foco, por isso e global
//! e nao um listener da janela.
//!
//! # Pressionar e soltar
//!
//! O gesto da F1 e "segurar para espiar": a espiada comeca no `Pressed` e
//! termina no `Released`. O plugin entrega os dois no Windows (ele observa a
//! tecla com `GetAsyncKeyState` depois do `WM_HOTKEY`), entao nao e preciso
//! hook nenhum — o que manteria a regra 1 em risco.
//!
//! O `Pressed` **repete** enquanto a tecla fica segurada, e o `Released` pode
//! chegar depois de um novo `Pressed` se o usuario teclar rapido. Quem absorve
//! isso e [`crate::peek`]: `begin` e `release` sao idempotentes.
//!
//! `Esc` merece atencao: um Esc global permanente seria **roubado do jogo**
//! (menu de pausa pararia de abrir). Por isso ele so fica registrado enquanto
//! ha um card aberto para ele fechar — ver [`sync_escape`].

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::error::Result;
use crate::peek;

/// Atalho padrao da espiada. Configuravel depois.
pub const DEFAULT_LOOKUP_SHORTCUT: &str = "Alt+X";

/// Atalho que abre o card sem usar o mouse.
pub const DEFAULT_CARD_SHORTCUT: &str = "Alt+C";

/// Segurar espia; soltar volta ao repouso.
pub fn lookup_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::KeyX)
}

/// Abre o card da palavra sob o cursor.
///
/// # Por que existe, se o clique ja faz isso
///
/// Porque o clique nem sempre pode ser impedido de chegar ao jogo. Jogos que
/// leem raw input recebem o botao do mouse pelo **foco**, e a overlay nunca
/// tira o foco do jogo — entao clicar numa palavra num FPS tambem atira.
///
/// Uma tecla registrada aqui nao tem esse problema: o `RegisterHotKey` do
/// Windows **consome** a combinacao, e o jogo nunca a ve. E o mesmo motivo de
/// o `Alt+X` nao digitar "x" no jogo.
///
/// `Alt+C` e nao apenas `C` porque a mao ja esta com o Alt pressionado
/// espiando, e um `C` solto seria roubado do jogo o tempo todo.
pub fn card_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::KeyC)
}

/// Fecha o card aberto.
pub fn escape_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

/// Roda `f` fora da thread de hotkeys.
///
/// Comecar e encerrar uma espiada mexe na janela (dispatch bloqueante para o
/// event loop) e re-registra atalhos; fazer isso dentro do callback do proprio
/// atalho trava a fila de hotkeys do processo.
fn fora_da_thread_de_hotkeys(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || f(&app));
}

/// Registra os atalhos globais no start do app.
pub fn register(app: &AppHandle) -> Result<()> {
    let manager = app.global_shortcut();

    manager.on_shortcut(lookup_shortcut(), |app, _shortcut, event| {
        match event.state() {
            ShortcutState::Pressed => fora_da_thread_de_hotkeys(app, peek::begin),
            ShortcutState::Released => fora_da_thread_de_hotkeys(app, peek::release),
        }
    })?;

    // Fica registrado o tempo todo, e nao so durante a espiada: registrar sob
    // demanda custaria alguns milissegundos no meio do gesto, e `Alt+C` nao e
    // uma combinacao que jogo nenhum vai sentir falta.
    manager.on_shortcut(card_shortcut(), |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            fora_da_thread_de_hotkeys(app, peek::open_card);
        }
    })?;

    Ok(())
}

/// Mantem o `Esc` global registrado apenas enquanto ha card aberto.
///
/// Idempotente: chamada em toda troca de modo, so age quando o estado do
/// registro diverge do pedido.
pub fn sync_escape(app: &AppHandle, interactive: bool) -> Result<()> {
    let manager = app.global_shortcut();
    let shortcut = escape_shortcut();
    match (interactive, manager.is_registered(shortcut)) {
        (true, false) => manager.on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                fora_da_thread_de_hotkeys(app, peek::end);
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
