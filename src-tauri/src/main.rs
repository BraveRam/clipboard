// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    if try_forward_to_running_instance() {
        return;
    }

    clipboard_lib::run()
}

#[cfg(target_os = "linux")]
fn try_forward_to_running_instance() -> bool {
    let token = std::env::var("XDG_ACTIVATION_TOKEN")
        .or_else(|_| std::env::var("DESKTOP_STARTUP_ID"))
        .unwrap_or_default();

    let mut args: Vec<String> = std::env::args().collect();
    if !token.is_empty() {
        args.push(format!("--xdg-token={}", token));
    }

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if let Ok(conn) = zbus::blocking::Connection::session() {
        let res = conn.call_method(
            Some("com.plxor.clipboard.SingleInstance"),
            "/com/plxor/clipboard/SingleInstance",
            Some("org.SingleInstance.DBus"),
            "ExecuteCallback",
            &(args, cwd),
        );
        if res.is_ok() {
            return true;
        }
    }
    false
}
