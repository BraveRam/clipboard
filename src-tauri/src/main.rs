// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod activation;

fn main() {
    #[cfg(target_os = "linux")]
    if try_forward_to_running_instance() {
        return;
    }

    clipboard_lib::run()
}

#[cfg(target_os = "linux")]
fn try_forward_to_running_instance() -> bool {
    let token = activation::environment_token();
    let mut args: Vec<String> = std::env::args().collect();
    if let Some(token) = token {
        args.push(format!("--xdg-token={token}"));
    }
    activation::trace(if activation::argument_token(&args).is_some() { "forwarding: token present" } else { "forwarding: token absent" });

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
