mod cm;
mod commands;
mod contacts;
mod files;
mod record;
mod server;
mod service;
mod settings;
mod session;
mod update;
pub mod memorable;

use hbb_common::config::Config;
use std::sync::atomic::{AtomicI32, Ordering};

#[tauri::command]
fn get_my_id() -> String {
    settings::host_id()
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
/// what is left to the UI. Only a plain launch or `--hidden` opens the EveryDesk window
/// (`Some(hidden)`: `--hidden` starts it hidden, e.g. at login next to the service; it shows
/// itself on an incoming request or a re-launch); other leftover args (e.g. `--cm`, which this
/// app never needs: it serves `_cm` itself) exit instead.
fn gui_launch(core_main_result: Option<&[String]>) -> Option<bool> {
    match core_main_result? {
        [] => Some(false),
        [a] if a == HIDDEN_ARG => Some(true),
        _ => None,
    }
}

const HIDDEN_ARG: &str = "--hidden";
/// IPC postfix of the "show the window" listener (one app window per user session).
const SHOW_IPC: &str = "_show";
/// Host events that need someone at the window: a hidden app shows itself for them.
const ATTENTION_EVENTS: [&str; 4] = ["cm:incoming", "cm:reconfirm", "cm:pair_offer", "cm:elevation"];

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
    if let Some(hidden) = gui_launch(res.as_deref()) {
        if forward_to_running_window() {
            log::info!("everydesk: already open in this session; showed that window");
            return;
        }
        run(hidden);
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

/// A second launch shows the window of the one already running (also the way back to a
/// `--hidden` app) instead of opening a second app that would fight over the `_cm` IPC.
fn forward_to_running_window() -> bool {
    block_on(async {
        match librustdesk::ipc::connect(500, SHOW_IPC).await {
            Ok(mut conn) => conn.send(&librustdesk::ipc::Data::Empty).await.is_ok(),
            Err(_) => false,
        }
    })
    .unwrap_or(false)
}

fn block_on<F: std::future::Future>(f: F) -> Option<F::Output> {
    match hbb_common::tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => Some(rt.block_on(f)),
        Err(e) => {
            log::error!("everydesk: tokio runtime: {e}");
            None
        }
    }
}

fn start_show_listener(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("everydesk-show".into())
        .spawn(move || serve_show(app))
        .expect("spawn show thread");
}

fn serve_show(app: tauri::AppHandle) {
    block_on(async move {
        match librustdesk::ipc::new_listener(SHOW_IPC).await {
            Ok(mut incoming) => {
                while let Some(conn) = incoming.next().await {
                    if conn.is_ok() {
                        show_main(&app);
                    }
                }
            }
            Err(e) => log::warn!("everydesk: show ipc: {e}"),
        }
    });
}

fn show_main(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn window_show(app: tauri::AppHandle) {
    show_main(&app);
}

fn run(hidden: bool) {
    log::info!(
        "everydesk: server={} id={}",
        server::EVERYDESK_SERVER,
        Config::get_id()
    );
    librustdesk::ui_interface::start_option_status_sync();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // The only source of paths `session_send_files` accepts: what the OS handed the window.
        .on_window_event(|_w, e| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = e {
                files::register_dropped_paths(paths);
            }
        })
        .on_webview_event(|_w, e| {
            if let tauri::WebviewEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = e {
                files::register_dropped_paths(paths);
            }
        })
        .setup(move |app| {
            // `_cm` first, so the host's first login finds it (`try_start_cm`) and the
            // engine never spawns a `--cm` window.
            cm::start(app.handle().clone());
            contacts::start(app.handle().clone());
            files::start(app.handle());
            start_show_listener(app.handle().clone());
            if hidden {
                use tauri::Manager;
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            for ev in ATTENTION_EVENTS {
                use tauri::Listener;
                let h = app.handle().clone();
                app.listen_any(ev, move |_| show_main(&h));
            }
            // Service mode: the service's `--server` hosts; only sync config with it over IPC,
            // retrying until it answers, and never host here (`no_server`). Otherwise, as the
            // Sciter GUI did in core_main: reuse a running `--server` over IPC, or start the host
            // (IPC + rendezvous registration) in this process.
            let service_mode = service::is_service_mode();
            log::info!("everydesk: service mode {service_mode}");
            std::thread::spawn(move || librustdesk::start_server(false, service_mode));
            // The service's config is the host's: pin server/key/click approval there too.
            server::ensure_host_config_on_service();
            service::start_watcher(app.handle().clone());
            update::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_my_id,
            get_server_status,
            window_show,
            service::service_status,
            service::install_permanent,
            service::uninstall_permanent,
            update::update_status, update::update_install, update::update_dismiss, update::about_info, update::about_open,
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
            cm::cm_get_permissions,
            cm::cm_set_permission,
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
    use super::{gui_launch, log_name};
    fn should_start_gui(r: Option<&[String]>) -> bool {
        gui_launch(r).is_some()
    }

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

    #[test]
    fn hidden_launch() {
        assert_eq!(gui_launch(Some(&[])), Some(false));
        assert_eq!(gui_launch(Some(&["--hidden".to_owned()])), Some(true));
        assert_eq!(gui_launch(Some(&["--hidden".to_owned(), "x".to_owned()])), None);
        assert_eq!(gui_launch(None), None);
    }
}
