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
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Another instance was launched (e.g. via OS keyboard shortcut).
            // Toggle the overlay on the primary instance with any forwarded XDG activation token.
            let token = argv
                .iter()
                .find(|a| a.starts_with("--xdg-token="))
                .map(|a| a.trim_start_matches("--xdg-token=").to_string());
            let app_handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                toggle_overlay(&app_handle, token.as_deref());
            });
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

            #[cfg(target_os = "linux")]
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(gtk_win) = win.gtk_window() {
                    use gtk::prelude::*;
                    gtk_win.set_skip_taskbar_hint(true);
                    gtk_win.set_skip_pager_hint(true);
                    gtk_win.set_keep_above(true);
                }
            }

            // Launched normally (keyboard shortcut) -> show the overlay.
            // Launched at login with --minimized -> stay hidden; the clipboard
            // watcher already runs in the background and the tray stays available.
            let args: Vec<String> = std::env::args().collect();
            if !should_start_hidden(&args) {
                let token = std::env::var("XDG_ACTIVATION_TOKEN")
                    .or_else(|_| std::env::var("DESKTOP_STARTUP_ID"))
                    .ok();
                show_overlay(app.handle(), token.as_deref());
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
            "show" => show_overlay(app, None),
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

#[cfg(target_os = "linux")]
fn focus_webview(gtk_win: &gtk::ApplicationWindow) {
    use gtk::prelude::*;
    fn walk(gtk_win: &gtk::ApplicationWindow, widget: &gtk::Widget) -> bool {
        let type_name = widget.type_().name();
        if type_name.contains("WebView") || type_name.contains("WebKit") {
            widget.set_can_focus(true);
            gtk_win.set_focus(Some(widget));
            widget.grab_focus();
            return true;
        }
        if let Some(container) = widget.downcast_ref::<gtk::Container>() {
            for child in container.children() {
                if walk(gtk_win, &child) {
                    return true;
                }
            }
        }
        if widget.can_focus() {
            widget.grab_focus();
            return true;
        }
        false
    }
    walk(gtk_win, gtk_win.upcast_ref::<gtk::Widget>());
}

/// Reveals, focuses and centers the overlay window, then notifies the UI.
pub fn show_overlay<R: Runtime>(app: &tauri::AppHandle<R>, token: Option<&str>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };

    #[cfg(target_os = "linux")]
    if let Ok(gtk_win) = win.gtk_window() {
        use gtk::prelude::*;
        if let Some(t) = token {
            gtk_win.set_startup_id(t);
        }
    }

    let _ = win.center();
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.as_ref().set_focus();

    #[cfg(target_os = "linux")]
    if let Ok(gtk_win) = win.gtk_window() {
        use gtk::prelude::*;
        gtk_win.present();
        focus_webview(&gtk_win);

        let win_weak = gtk_win.downgrade();
        gtk::glib::idle_add_local_once(move || {
            if let Some(w) = win_weak.upgrade() {
                focus_webview(&w);
            }
        });
        let win_weak2 = gtk_win.downgrade();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            if let Some(w) = win_weak2.upgrade() {
                focus_webview(&w);
            }
        });
    }

    let _ = app.emit("overlay:opened", ());
}

/// Toggles overlay visibility: hides it if shown and focused, otherwise reveals and focuses it.
pub fn toggle_overlay<R: Runtime>(app: &tauri::AppHandle<R>, token: Option<&str>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let is_visible = win.is_visible().unwrap_or(false);
    let is_focused = win.is_focused().unwrap_or(false);

    if is_visible && is_focused {
        let _ = win.hide();
    } else {
        show_overlay(app, token);
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
