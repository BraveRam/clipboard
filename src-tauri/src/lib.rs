mod clipboard;
mod commands;
mod db;

use clipboard::WriteGuard;
use commands::AppState;
use db::Repo;
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Another instance was launched (e.g. via OS keyboard shortcut).
            // Toggle the overlay on the primary instance instead of starting
            // a second copy.
            toggle_overlay(app);
        }))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("resolve appDataDir");
            let db_path = data_dir.join("clipboard.db");
            let images_dir = data_dir.join("images");
            let repo = Arc::new(Repo::open(&db_path, images_dir).expect("open repo"));
            let guard = Arc::new(WriteGuard::new());

            app.manage(AppState {
                repo: repo.clone(),
                guard: guard.clone(),
            });

            clipboard::spawn_watcher(app.handle().clone(), repo, guard);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::entries_list,
            commands::entry_paste,
            commands::entry_pin_toggle,
            commands::entry_delete,
            commands::entries_clear_unpinned,
            commands::overlay_hide,
            commands::overlay_show,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn toggle_overlay<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    match win.is_visible() {
        Ok(true) => {
            let _ = win.hide();
        }
        _ => {
            let _ = win.show();
            let _ = win.set_focus();
            let _ = win.center();
            let _ = app.emit("overlay:opened", ());
        }
    }
}
