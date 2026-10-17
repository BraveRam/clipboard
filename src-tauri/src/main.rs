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

    // A busy or unreachable existing daemon must never trigger another GUI startup.
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|builder| {
            builder.method_timeout(std::time::Duration::from_millis(750)).build()
        });
    let Ok(conn) = connection else {
        eprintln!("[clipboard] session bus unavailable; not starting another instance");
        return true;
    };
    let owner = conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &("com.plxor.clipboard.SingleInstance",),
        )
        .and_then(|reply| reply.body().deserialize::<bool>());
    match owner {
        Ok(false) => false,
        Ok(true) => {
            if conn
                .call_method(
                    Some("com.plxor.clipboard.SingleInstance"),
                    "/com/plxor/clipboard/SingleInstance",
                    Some("org.SingleInstance.DBus"),
                    "ExecuteCallback",
                    &(args, cwd),
                )
                .is_err()
            {
                eprintln!("[clipboard] running instance did not respond; not starting another instance");
            }
            true
        }
        Err(_) => {
            eprintln!("[clipboard] cannot check running instance; not starting another instance");
            true
        }
    }
}
