mod cm;
mod commands;
mod server;
mod session;

use hbb_common::config::Config;
use std::sync::atomic::{AtomicI32, Ordering};

#[tauri::command]
fn get_my_id() -> String {
    Config::get_id()
}

/// Same source the old Sciter UI polled: `ui_interface` keeps an IPC
/// connection to the host server and caches its `OnlineStatus`.
#[tauri::command]
fn get_server_status() -> String {
    static LAST: AtomicI32 = AtomicI32::new(i32::MIN);
    let st = librustdesk::ui_interface::get_connect_status();
    // status_num > 0 only means the server answers; the ID is registered once
    // the server accepted our public key (key_confirmed), as in the Sciter UI.
    let n = if st.status_num > 0 && st.key_confirmed { 2 } else { st.status_num };
    let s = match n {
        2 => "registrado",
        1 => "aguardando autorização do servidor",
        -1 => "sem conexão com o servidor",
        _ => "conectando",
    };
    if LAST.swap(n, Ordering::Relaxed) != n {
        log::info!("everydesk: server status -> {s}");
    }
    s.to_owned()
}

pub fn run() {
    hbb_common::init_log(false, "everydesk");
    server::apply_server_config();
    cm::default_approve_mode();
    log::info!(
        "everydesk: server={} id={}",
        server::EVERYDESK_SERVER,
        Config::get_id()
    );
    librustdesk::ui_interface::start_option_status_sync();

    tauri::Builder::default()
        .setup(|app| {
            // `_cm` first, so the host's first login finds it (`try_start_cm`) and the
            // engine never spawns a `--cm` window.
            cm::start(app.handle().clone());
            // As the Sciter GUI did in core_main: reuse a running `--server` over IPC,
            // otherwise start the host (IPC + rendezvous registration) in this process.
            std::thread::spawn(|| librustdesk::start_server(false, false));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_my_id,
            get_server_status,
            commands::session_connect,
            commands::session_close,
            commands::session_login,
            commands::session_mouse,
            commands::session_key,
            commands::session_switch_display,
            commands::session_frame_ack,
            cm::cm_authorize,
            cm::cm_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while running EveryDesk")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Closing the window mid-session: tell the peer instead of just dropping
                // the socket, and give the io thread a moment to flush the close reason.
                commands::close_current();
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        });
}
