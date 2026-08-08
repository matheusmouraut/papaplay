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

use std::str::FromStr;
use std::sync::Mutex;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::{Error, Result};
use crate::peek;
use crate::settings::{self, Shortcuts};

/// Atalho padrao da espiada. Configuravel pela tela de Configuracoes (F6).
pub const DEFAULT_LOOKUP_SHORTCUT: &str = "Alt+X";

/// Atalho que abre o card sem usar o mouse. Configuravel pela tela de
/// Configuracoes (F6).
pub const DEFAULT_CARD_SHORTCUT: &str = "Alt+C";

/// Os atalhos de fato registrados agora — para o `unregister` do
/// [`reregister`] saber o que tirar antes de por os novos.
static ATIVOS: Mutex<Option<Shortcuts>> = Mutex::new(None);

/// Fecha o card aberto. Sem modificador de proposito (regra de UX, nao
/// tecnica): e o unico atalho que nunca colide com `lookup`/`card`, entao fica
/// de fora da configuracao — ver o comentario em [`crate::settings::Shortcuts`].
pub fn escape_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

/// Converte a combinacao salva ("Alt+X") num `Shortcut`, com uma mensagem que
/// a UI pode mostrar direto — quem digitou uma combinacao invalida vai ver
/// esta string, nao um erro interno do parser.
pub fn validar(combinacao: &str) -> Result<Shortcut> {
    Shortcut::from_str(combinacao)
        .map_err(|e| Error::Platform(format!("atalho \"{combinacao}\" invalido: {e}")))
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

/// Registra os atalhos globais no start do app, lendo a combinacao salva (ou
/// os padroes, na primeira execucao).
pub fn register(app: &AppHandle) -> Result<()> {
    aplicar(app, settings::ler_atalhos(app)?)
}

/// Registra `lookup`/`card` e guarda em [`ATIVOS`] o que ficou de pe — e o que
/// [`reregister`] usa depois para saber o que desregistrar primeiro.
fn aplicar(app: &AppHandle, shortcuts: Shortcuts) -> Result<()> {
    let manager = app.global_shortcut();

    manager.on_shortcut(
        validar(&shortcuts.lookup)?,
        |app, _shortcut, event| match event.state() {
            ShortcutState::Pressed => fora_da_thread_de_hotkeys(app, peek::begin),
            ShortcutState::Released => fora_da_thread_de_hotkeys(app, peek::release),
        },
    )?;

    // Fica registrado o tempo todo, e nao so durante a espiada: registrar sob
    // demanda custaria alguns milissegundos no meio do gesto, e `Alt+C` nao e
    // uma combinacao que jogo nenhum vai sentir falta.
    manager.on_shortcut(validar(&shortcuts.card)?, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            fora_da_thread_de_hotkeys(app, peek::open_card);
        }
    })?;

    *ATIVOS.lock().expect("mutex dos atalhos envenenado") = Some(shortcuts);
    Ok(())
}

/// Troca `lookup`/`card` por uma combinacao nova sem reiniciar o app: tira as
/// que estao registradas agora e poe as novas no lugar.
///
/// `Esc` fica de fora (ver [`escape_shortcut`]) — nada aqui mexe nele.
pub fn reregister(app: &AppHandle, shortcuts: Shortcuts) -> Result<()> {
    let anteriores = ATIVOS
        .lock()
        .expect("mutex dos atalhos envenenado")
        .clone()
        .unwrap_or_default();
    let manager = app.global_shortcut();
    manager.unregister(validar(&anteriores.lookup)?)?;
    manager.unregister(validar(&anteriores.card)?)?;
    aplicar(app, shortcuts)
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
    use tauri_plugin_global_shortcut::Modifiers;

    #[test]
    fn atalho_de_lookup_padrao_bate_com_a_constante_documentada() {
        let shortcut = validar(DEFAULT_LOOKUP_SHORTCUT).expect("padrao e valido");
        assert_eq!(shortcut.key, Code::KeyX);
        assert!(shortcut.mods.contains(Modifiers::ALT));
    }

    #[test]
    fn escape_nao_tem_modificador() {
        let shortcut = escape_shortcut();
        assert_eq!(shortcut.key, Code::Escape);
        assert!(shortcut.mods.is_empty());
    }

    #[test]
    fn validar_aceita_o_formato_que_a_ui_manda() {
        let shortcut = validar("Alt+KeyZ").expect("combinacao valida");
        assert_eq!(shortcut.key, Code::KeyZ);
        assert!(shortcut.mods.contains(Modifiers::ALT));
    }

    #[test]
    fn validar_recusa_combinacao_sem_tecla() {
        assert!(validar("Alt+Ctrl").is_err());
    }

    #[test]
    fn validar_recusa_lixo() {
        assert!(validar("nao e um atalho").is_err());
    }
}
