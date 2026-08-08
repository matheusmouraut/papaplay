pub mod capture;
pub mod db;
pub mod deck;
pub mod dict;
pub mod error;
pub mod hotkeys;
pub mod lookup;
pub mod media;
pub mod ocr;
pub mod overlay;
pub mod peek;
pub mod platform;
pub mod review;
pub mod settings;
pub mod setup;
pub mod stats;
pub mod translate;
pub mod tray;

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
                tray::open_main(app);
            }))
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .plugin(tauri_plugin_dialog::init());
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // A overlay comeca visivel e passiva — esse e o estado de repouso.
            overlay::init(app.handle())?;
            tray::init(app.handle())?;
            hotkeys::register(app.handle())?;
            // Em segundo plano, para a primeira espiada nao esperar por disco
            // nem pela criacao do device de captura.
            lookup::preload_engine(app.handle());
            dict::preload(app.handle());
            capture::warm_up();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            overlay::overlay_set_mode,
            overlay::overlay_status,
            overlay::overlay_check_geometry,
            overlay::overlay_bench,
            overlay::overlay_reset_size,
            lookup::lookup_run,
            peek::peek_close,
            peek::peek_state,
            dict::dict_lookup,
            translate::translate_run,
            deck::deck_save_card,
            deck::deck_card_status,
            deck::deck_list_cards,
            deck::deck_card_detail,
            deck::deck_games,
            deck::deck_set_suspended,
            deck::deck_update_context,
            deck::deck_delete_card,
            deck::deck_export_csv,
            review::review_queue,
            review::review_apply,
            stats::stats_summary,
            media::media_screenshot,
            settings::settings_get_shortcuts,
            settings::settings_set_shortcuts,
            settings::settings_reset_shortcuts,
            settings::settings_get_preferences,
            settings::settings_set_preferences,
            setup::nmt_status,
            setup::nmt_install,
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
