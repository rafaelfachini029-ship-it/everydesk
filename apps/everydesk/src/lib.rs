mod cm;
mod commands;
mod contacts;
mod files;
mod record;
mod server;
mod session;
pub mod memorable;

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

/// `core_main` returns `None` when it handled a command-line mode itself (`--server`,
/// `--check-hwcodec-config`, `--get-id`, `--password`, `--option`, ...), and `Some(args)` for
/// what is left to the UI. Only a plain launch opens the EveryDesk window; leftover args
/// (e.g. `--cm`, which this app never needs: it serves `_cm` itself) exit instead.
fn should_start_gui(core_main_result: Option<&[String]>) -> bool {
    matches!(core_main_result, Some(args) if args.is_empty())
}

/// Product name. The engine derives the config/log dirs, IPC socket/pipe names, service
/// name and macOS plists from `hbb_common::config::APP_NAME`.
pub const APP_NAME: &str = "EveryDesk";
#[cfg(target_os = "macos")]
const ORG: &str = "online.vpsfachini";

/// Must run before anything touches `Config` (lazily loaded from the APP_NAME path), the
/// log or IPC, so it is the first statement of `main`.
pub fn apply_branding() {
    *hbb_common::config::APP_NAME.write().unwrap() = APP_NAME.to_owned();
    #[cfg(target_os = "macos")]
    {
        *hbb_common::config::ORG.write().unwrap() = ORG.to_owned();
    }
}

/// Process entry point. The host spawns this same exe with engine modes (hwcodec probe,
/// service, CLI settings); those must run headless, never as a second app window.
pub fn main() {
    // Branding first: every path below (log, config, IPC) is derived from APP_NAME.
    apply_branding();
    // Same per-mode log directory core_main would pick, except a plain launch logs under
    // `everydesk`; initialized here so the config fixes below are logged too.
    let args: Vec<String> = std::env::args().skip(1).collect();
    hbb_common::init_log(false, &log_name(&args));
    // The id must be a memorable code before any mode registers with the server.
    ensure_memorable_id();
    // Pinned server and click approval first: every mode below may register or take logins.
    server::apply_server_config();
    cm::force_click_approve_mode();
    let res = librustdesk::core_main::core_main();
    if should_start_gui(res.as_deref()) {
        run();
    } else if let Some(args) = res {
        log::warn!("everydesk: unsupported arguments, exiting: {args:?}");
    }
}

/// First boot (or an id from before EveryDesk, e.g. the numeric MAC-derived one): replace it
/// with a memorable code, and let the engine regenerate one if the server says it is taken.
fn ensure_memorable_id() {
    let _ = librustdesk::ID_REGENERATOR.set(memorable::new_code);
    let id = Config::get_id();
    if !memorable::is_valid_code(&id) {
        let code = memorable::new_code();
        Config::set_id(&code);
        log::info!("everydesk: id {id:?} replaced by memorable code {code}");
    }
}

fn log_name(args: &[String]) -> String {
    if args
        .iter()
        .any(|a| a.starts_with("--portable-service-shmem-name="))
    {
        return "portable-service".to_owned();
    }
    match args.first().map(|a| a.trim_start_matches("--")) {
        Some(name) if !name.is_empty() && args[0].starts_with("--") => name.to_owned(),
        _ => "everydesk".to_owned(),
    }
}

fn run() {
    log::info!(
        "everydesk: server={} id={}",
        server::EVERYDESK_SERVER,
        Config::get_id()
    );
    librustdesk::ui_interface::start_option_status_sync();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // `_cm` first, so the host's first login finds it (`try_start_cm`) and the
            // engine never spawns a `--cm` window.
            cm::start(app.handle().clone());
            contacts::start(app.handle().clone());
            files::start(app.handle());
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
            commands::session_toggle, commands::session_get_toggles, commands::session_quality, commands::session_get_quality, commands::session_fps, commands::session_restart, commands::session_ctrl_alt_del, commands::session_lock,
            commands::session_send_chat,
            commands::session_voice,
            commands::session_record, commands::session_is_recording,
            record::recordings_dir, record::open_recordings_dir,
            files::session_send_files,
            files::session_pick_and_send,
            files::session_cancel_job,
            files::session_override_answer,
            cm::cm_authorize,
            cm::cm_close,
            cm::cm_pending,
            cm::cm_reconfirm_result,
            cm::cm_pair_ack,
            cm::cm_elevate, cm::cm_elevation_dismiss, cm::cm_elevation_setting, cm::cm_elevation_set_setting, cm::cm_elevation_pending,
            cm::cm_send_chat,
            cm::cm_voice_answer,
            cm::cm_voice_close,
            contacts::contacts_list,
            contacts::contacts_set_policy,
            contacts::contacts_remove,
            contacts::contacts_presence,
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

#[cfg(test)]
mod tests {
    use super::{log_name, should_start_gui};

    #[test]
    fn app_name_is_everydesk() {
        super::apply_branding();
        assert_eq!(*hbb_common::config::APP_NAME.read().unwrap(), "EveryDesk");
        let p = hbb_common::config::Config::path("x");
        let p = p.to_string_lossy();
        assert!(p.contains("EveryDesk"), "{p}");
        assert!(!p.contains("RustDesk"), "{p}");
    }

    #[test]
    fn log_dir_per_mode() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(log_name(&v(&[])), "everydesk");
        let hw = v(&["--check-hwcodec-config"]);
        assert_eq!(log_name(&hw), "check-hwcodec-config");
        assert_eq!(log_name(&v(&["--server"])), "server");
        assert_eq!(log_name(&v(&["--"])), "everydesk");
        let ps = v(&["x", "--portable-service-shmem-name=a"]);
        assert_eq!(log_name(&ps), "portable-service");
    }

    #[test]
    fn gui_only_for_a_plain_launch() {
        assert!(should_start_gui(Some(&[])));
        // Handled by core_main (e.g. --check-hwcodec-config, --get-id, --server).
        assert!(!should_start_gui(None));
        // Left over by core_main: no second app window.
        assert!(!should_start_gui(Some(&["--cm".to_owned()])));
        let unknown = ["--unknown".to_owned(), "x".to_owned()];
        assert!(!should_start_gui(Some(&unknown)));
    }
}
