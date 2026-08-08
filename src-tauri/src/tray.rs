//! Icone de bandeja e ciclo de vida da aplicacao.
//!
//! **Fechar fecha.** O "x" da janela principal encerra o processo inteiro —
//! overlay, atalhos globais e bandeja juntos. Sem a intercepcao de [`init`] o
//! Tauri so destruiria a janela principal e o processo continuaria vivo por
//! causa da overlay, que e invisivel: o app viraria um fantasma no gerenciador
//! de tarefas, sem janela e sem jeito de sair.
//!
//! A bandeja existe enquanto o app roda, com "Abrir" (traz a janela de volta
//! quando ela esta minimizada) e "Sair" (o mesmo encerramento do "x"). Para
//! espiar durante o jogo, o app fica aberto — minimizado basta.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

use crate::error::Result;

const LABEL_MAIN: &str = "main";
const ID_ABRIR: &str = "abrir";
const ID_SAIR: &str = "sair";

/// Traz a janela principal de volta ao primeiro plano.
///
/// Usada pelo clique/menu da bandeja e pelo callback de segunda instancia —
/// os dois caminhos que existem hoje para "reabrir" o app.
pub fn open_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL_MAIN) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Cria o icone de bandeja e faz o "x" da janela principal encerrar o app.
pub fn init(app: &AppHandle) -> Result<()> {
    encerrar_ao_fechar(app);

    let abrir = MenuItem::with_id(app, ID_ABRIR, "Abrir", true, None::<&str>)?;
    let sair = MenuItem::with_id(app, ID_SAIR, "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&abrir, &sair])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| crate::error::Error::Platform("icone padrao da janela ausente".into()))?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("PapaPlay")
        .on_menu_event(|app, event| match event.id.as_ref() {
            ID_ABRIR => open_main(app),
            ID_SAIR => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_main(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Fechar a janela principal encerra o processo.
///
/// `prevent_close` + `exit` em vez de deixar o Tauri destruir a janela: a
/// overlay continuaria viva e seguraria o processo. Encerrar pelo `exit`
/// tambem e o que garante que os atalhos globais sejam devolvidos ao Windows —
/// matar o processo com eles registrados deixa `Alt+X` inutilizavel por outros
/// apps ate o Windows reciclar o registro.
fn encerrar_ao_fechar(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL_MAIN) else {
        return;
    };
    let app = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            app.exit(0);
        }
    });
}
