use crate::clipboard;
use crate::clipboard::WriteGuard;
use crate::db::{Entry, Repo};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub repo: Arc<Repo>,
    pub guard: Arc<WriteGuard>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapturedEvent {
    pub id: i64,
    pub is_new: bool,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn entries_list(state: State<'_, AppState>) -> Result<Vec<Entry>, String> {
    state.repo.list().map_err(err)
}

/// Selects an entry: writes it back to the clipboard (with write-guard armed
/// to prevent the watcher from re-capturing) and bumps last_used_at.
#[tauri::command]
pub async fn entry_paste(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, String> {
    let Some(entry) = state.repo.get(id).map_err(err)? else {
        return Ok(false);
    };
    state.repo.touch(id).map_err(err)?;

    match entry.kind.as_str() {
        "text" => {
            if let Some(text) = entry.text.as_deref() {
                clipboard::write_text(text, &state.guard).map_err(err)?;
            }
        }
        "image" => {
            let Some(rel) = entry.image_path.as_deref() else {
                return Ok(false);
            };
            let full = state.repo.images_dir().join(rel);
            let bytes = std::fs::read(&full).map_err(err)?;
            let b64 = {
                use base64::{engine::general_purpose::STANDARD, Engine as _};
                STANDARD.encode(&bytes)
            };
            clipboard::write_image_base64_png(&b64, &state.guard).map_err(err)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[tauri::command]
pub async fn entry_pin_toggle(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, String> {
    let pinned = state.repo.toggle_pin(id).map_err(err)?;
    let _ = app.emit("clipboard:entries-changed", ());
    Ok(pinned)
}

#[tauri::command]
pub async fn entry_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    state.repo.delete(id).map_err(err)?;
    let _ = app.emit("clipboard:entries-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn entries_clear_unpinned(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.repo.clear_unpinned().map_err(err)?;
    let _ = app.emit("clipboard:entries-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn overlay_hide(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn overlay_show(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.center();
    }
    Ok(())
}
