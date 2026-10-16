//! Session commands for the minimal controller (one outgoing session at a time).
//! Mirrors `flutter::session_add` + `flutter::session_start_`.

use crate::session::TauriSessionHandler;
use hbb_common::rendezvous_proto::ConnType;
use librustdesk::ui_session_interface::{io_loop, Session};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex, RwLock,
};
use tauri::{
    ipc::{Channel, InvokeResponseBody},
    AppHandle,
};

/// RustDesk mouse event types (`MOUSE_TYPE_*` in `src/common.rs`).
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_TYPE_MOVE: i32 = 0;
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_TYPE_DOWN: i32 = 1;
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_TYPE_UP: i32 = 2;
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_TYPE_WHEEL: i32 = 3;
/// RustDesk mouse buttons (`MOUSE_BUTTON_*`).
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_BUTTON_LEFT: i32 = 1;
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_BUTTON_RIGHT: i32 = 2;
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub const MOUSE_BUTTON_WHEEL: i32 = 4;

/// `mask = (button << 3) | event`, as `send_mouse` expects. The frontend mirrors this
/// in `session.ts::mouseMask`.
#[allow(dead_code)] // reference for session.ts; exercised by tests
pub fn mouse_mask(button: i32, event: i32) -> i32 {
    (button << 3) | event
}

struct Current {
    gen: u64,
    session: Session<TauriSessionHandler>,
}

static SESSION: Mutex<Option<Current>> = Mutex::new(None);
static GEN: AtomicU64 = AtomicU64::new(0);

fn with_session(f: impl FnOnce(&Session<TauriSessionHandler>)) {
    if let Some(c) = SESSION.lock().unwrap().as_ref() {
        f(&c.session);
    }
}

/// Sends `Data::Close` to the running session, if any (also used on window close).
pub fn close_current() {
    if let Some(c) = SESSION.lock().unwrap().take() {
        c.session.close();
    }
}

#[tauri::command]
pub fn session_connect(
    app: AppHandle,
    id: String,
    password: String,
    frames: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    let id = id.trim().replace(' ', "");
    if id.is_empty() {
        return Err("ID remoto vazio".into());
    }
    close_current();

    let handler = TauriSessionHandler::new(app, frames);
    let session: Session<TauriSessionHandler> = Session {
        // Preset password: `client::handle_hash` hashes it with the peer's salt and logs in,
        // no UI round trip. A wrong one comes back as msgbox `re-input-password`, answered
        // through `session_login`.
        password,
        ui_handler: handler.clone(),
        server_keyboard_enabled: Arc::new(RwLock::new(true)),
        server_file_transfer_enabled: Arc::new(RwLock::new(true)),
        server_clipboard_enabled: Arc::new(RwLock::new(true)),
        reconnect_count: Arc::new(AtomicUsize::new(0)),
        ..Default::default()
    };
    session.lc.write().unwrap().initialize(
        id.clone(),
        ConnType::DEFAULT_CONN,
        None,  // switch_uuid
        false, // force_relay
        None,  // adapter_luid
        None,  // shared_password
        None,  // conn_token
    );

    let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
    *SESSION.lock().unwrap() = Some(Current {
        gen,
        session: session.clone(),
    });
    log::info!("everydesk: connecting to {id}");
    std::thread::Builder::new()
        .name(format!("session-{id}"))
        .spawn(move || {
            let round = session.connection_round_state.lock().unwrap().new_round();
            io_loop(session, round);
            let mut cur = SESSION.lock().unwrap();
            if cur.as_ref().map(|c| c.gen) == Some(gen) {
                *cur = None;
            }
            drop(cur);
            log::info!("everydesk: session {gen} ended");
            // A newer session_connect superseded this one: its UI must not be torn down.
            if GEN.load(Ordering::Relaxed) == gen {
                handler.emit("session:closed", ());
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn session_close() {
    close_current();
}

/// Answers an `input-password` / `re-input-password` msgbox.
#[tauri::command]
pub fn session_login(password: String) {
    with_session(|s| s.login(String::new(), String::new(), password, false));
}

#[tauri::command]
pub fn session_mouse(
    mask: i32,
    x: i32,
    y: i32,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    with_session(|s| s.send_mouse(mask, x, y, alt, ctrl, shift, command));
}

#[tauri::command]
pub fn session_key(
    name: String,
    down: bool,
    press: bool,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    with_session(|s| s.input_key(&name, down, press, alt, ctrl, shift, command));
}

#[tauri::command]
pub fn session_switch_display(index: usize) {
    with_session(|s| s.switch_display(index as i32));
}

/// The webview drew a frame packet; lets `on_rgba` send the next one.
#[tauri::command]
pub fn session_frame_ack() {
    with_session(|s| s.ui_handler.frame_ack());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_mask_matches_rustdesk() {
        assert_eq!(mouse_mask(0, MOUSE_TYPE_MOVE), 0);
        assert_eq!(mouse_mask(MOUSE_BUTTON_LEFT, MOUSE_TYPE_DOWN), 9);
        assert_eq!(mouse_mask(MOUSE_BUTTON_LEFT, MOUSE_TYPE_UP), 10);
        assert_eq!(mouse_mask(MOUSE_BUTTON_RIGHT, MOUSE_TYPE_DOWN), 17);
        assert_eq!(mouse_mask(MOUSE_BUTTON_RIGHT, MOUSE_TYPE_UP), 18);
        assert_eq!(mouse_mask(MOUSE_BUTTON_WHEEL, MOUSE_TYPE_DOWN), 33);
        assert_eq!(mouse_mask(0, MOUSE_TYPE_WHEEL), 3);
    }
}
