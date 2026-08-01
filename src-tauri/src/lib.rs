pub mod capture;
pub mod db;
pub mod deck;
pub mod dict;
pub mod error;
pub mod hotkeys;
pub mod lookup;
pub mod ocr;
pub mod overlay;
pub mod platform;
pub mod translate;

use tauri::Manager;

/// Health-check do core. As telas placeholder chamam este comando para
/// provar que a ponte UI <-> Rust esta de pe.
#[tauri::command]
fn ping() -> String {
    format!("pong v{}", env!("CARGO_PKG_VERSION"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                // Segunda instancia: traz a janela principal de volta.
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }))
            .plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // A overlay comeca visivel e passiva — esse e o estado de repouso.
            overlay::init(app.handle())?;
            hotkeys::register(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            overlay::overlay_set_mode,
            overlay::overlay_toggle,
            overlay::overlay_status,
            overlay::overlay_check_geometry,
            overlay::overlay_bench,
            overlay::overlay_reset_size,
            lookup::lookup_run,
            dict::dict_lookup,
            translate::translate_run,
            deck::deck_save_card,
            deck::deck_card_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_reports_the_crate_version() {
        assert_eq!(ping(), format!("pong v{}", env!("CARGO_PKG_VERSION")));
    }
}
