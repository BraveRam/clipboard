mod clipboard;
mod commands;
mod db;

use clipboard::WriteGuard;
use commands::AppState;
use db::Repo;
use std::path::Path;
use std::sync::Arc;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;

/// CLI flag the autostart entry passes so a login-launched instance knows to
/// stay hidden in the tray instead of popping the overlay open.
const MINIMIZED_FLAG: &str = "--minimized";

/// Marker file (in the app data dir) recording that we've performed the
/// one-time "enable autostart on first run" step. Its presence means the
/// user's later choice (e.g. disabling via the tray) must not be overridden.
const AUTOSTART_MARKER: &str = ".autostart-initialized";

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
            Some(vec![MINIMIZED_FLAG]),
        ))
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("resolve appDataDir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("clipboard.db");
            let images_dir = data_dir.join("images");
            let repo = Arc::new(Repo::open(&db_path, images_dir).expect("open repo"));
            let guard = Arc::new(WriteGuard::new());

            app.manage(AppState {
                repo: repo.clone(),
                guard: guard.clone(),
            });

            clipboard::spawn_watcher(app.handle().clone(), repo, guard);

            // The first time the app ever runs, turn on launch-at-login so the
            // history keeps being captured across logout/login without the user
            // having to re-trigger the shortcut. Guarded by a marker file so the
            // user can later disable it (via the tray) and have that stick.
            init_autostart(app.handle(), &data_dir);

            build_tray(app.handle())?;

            // Launched normally (keyboard shortcut) -> show the overlay.
            // Launched at login with --minimized -> stay hidden; the clipboard
            // watcher already runs in the background and the tray stays available.
            let args: Vec<String> = std::env::args().collect();
            if !should_start_hidden(&args) {
                show_overlay(app.handle());
            }

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

/// Returns true when the process was launched with the autostart flag and
/// should therefore start hidden in the tray.
fn should_start_hidden(args: &[String]) -> bool {
    args.iter().any(|a| a == MINIMIZED_FLAG)
}

/// Enables launch-at-login exactly once, the first time the app runs.
fn init_autostart<R: Runtime>(app: &tauri::AppHandle<R>, data_dir: &Path) {
    let marker = data_dir.join(AUTOSTART_MARKER);
    if marker.exists() {
        return;
    }
    let manager = app.autolaunch();
    if !manager.is_enabled().unwrap_or(false) {
        let _ = manager.enable();
    }
    let _ = std::fs::write(&marker, b"1");
}

/// Builds the system tray icon and its menu (Show / Start at login / Quit).
fn build_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::with_id("show", "Show clipboard").build(app)?;
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart_item = CheckMenuItemBuilder::with_id("autostart", "Start at login")
        .checked(autostart_on)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&autostart_item)
        .separator()
        .item(&quit_item)
        .build()?;

    // Cloned handle so the menu-event closure can reflect the new state back
    // onto the checkbox after toggling.
    let autostart_checkbox = autostart_item.clone();

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Clipboard")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => show_overlay(app),
            "autostart" => {
                let manager = app.autolaunch();
                let enabled = manager.is_enabled().unwrap_or(false);
                let result = if enabled {
                    manager.disable()
                } else {
                    manager.enable()
                };
                if result.is_ok() {
                    let _ = autostart_checkbox.set_checked(!enabled);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Reveals, focuses and centers the overlay window, then notifies the UI.
fn show_overlay<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.center();
    let _ = app.emit("overlay:opened", ());
}

/// Toggles overlay visibility: hides it if shown, otherwise reveals it.
fn toggle_overlay<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        show_overlay(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_hidden_when_minimized_flag_present() {
        let args = vec!["clipboard".to_string(), MINIMIZED_FLAG.to_string()];
        assert!(should_start_hidden(&args));
    }

    #[test]
    fn visible_on_normal_launch() {
        let args = vec!["clipboard".to_string()];
        assert!(!should_start_hidden(&args));
    }

    #[test]
    fn ignores_unrelated_flags() {
        let args = vec!["clipboard".to_string(), "--debug".to_string()];
        assert!(!should_start_hidden(&args));
    }
}
