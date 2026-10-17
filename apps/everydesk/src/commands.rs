//! Session commands for the minimal controller (one outgoing session at a time).
//! Mirrors `flutter::session_add` + `flutter::session_start_`.

use crate::session::{is_current_gen, next_gen, TauriSessionHandler};
use hbb_common::rendezvous_proto::ConnType;
use librustdesk::ui_session_interface::{io_loop, Session};
use std::collections::HashMap;
use std::sync::{
    atomic::AtomicUsize,
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

fn with_session(f: impl FnOnce(&Session<TauriSessionHandler>)) {
    if let Some(c) = SESSION.lock().unwrap().as_ref() {
        f(&c.session);
    }
}

/// The running session and its generation (a cheap clone: every field is shared), for
/// commands that must block (the file sender waits for the host) without holding the lock.
pub(crate) fn current() -> Option<(u64, Session<TauriSessionHandler>)> {
    SESSION
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| (c.gen, c.session.clone()))
}

/// Sends `Data::Close` to the running session, if any (also used on window close), and
/// makes it stale so its late events (timeout msgbox, displays...) are dropped.
pub fn close_current() {
    let taken = SESSION.lock().unwrap().take();
    if let Some(c) = taken {
        next_gen();
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
        return Err("Código vazio".into());
    }
    close_current();

    let gen = next_gen();
    let handler = TauriSessionHandler::new(app, frames, gen);
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
            if is_current_gen(gen) {
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

// ---- Session extras (Stage 4 Task 6) --------------------------------------------------

/// The only options the webview may toggle (closed list; the engine's own names, see
/// `LoginConfigHandler::toggle_option` in `src/client.rs`). Anything else is refused, so the
/// UI can never set arbitrary peer options. `privacy-mode` is deliberately out: the engine
/// toggles it through `toggle_privacy_mode(impl_key, on)` with a host-specific implementation
/// key, and a plain flip here would ask the host for a mode it may not support.
pub const TOGGLES: [&str; 8] = [
    "view-only",
    "disable-clipboard",
    "disable-audio",
    "show-remote-cursor",
    "follow-remote-cursor",
    "block-input",
    "unblock-input",
    "lock-after-session-end",
];

/// The toggles whose state the engine keeps (`get_toggle_option`). Block input is a request
/// to the host, not a stored option: its state comes back as `session:block_input`.
const STORED_TOGGLES: [&str; 6] = [
    "view-only",
    "disable-clipboard",
    "disable-audio",
    "show-remote-cursor",
    "follow-remote-cursor",
    "lock-after-session-end",
];

pub fn toggle_allowed(name: &str) -> bool {
    TOGGLES.contains(&name)
}

/// UI level → engine image quality.
pub fn quality_value(level: &str) -> Option<&'static str> {
    match level {
        "auto" => Some("balanced"),
        "sharp" => Some("best"),
        "smooth" => Some("low"),
        _ => None,
    }
}

/// Engine image quality → UI level (anything custom reads as automatic).
pub fn quality_level(value: &str) -> &'static str {
    match value {
        "best" => "sharp",
        "low" => "smooth",
        _ => "auto",
    }
}

pub fn clamp_fps(n: u32) -> i32 {
    n.clamp(5, 60) as i32
}

fn require_session() -> Result<Session<TauriSessionHandler>, String> {
    current().map(|(_, s)| s).ok_or_else(|| "Nenhuma sessão aberta".to_owned())
}

/// Flips one option of the running session and returns its new state. `block-input` /
/// `unblock-input` return the state asked for; the host's answer arrives as
/// `session:block_input`.
#[tauri::command]
pub fn session_toggle(name: String) -> Result<bool, String> {
    if !toggle_allowed(&name) {
        return Err(format!("Opção desconhecida: {name}"));
    }
    let s = require_session()?;
    s.toggle_option(name.clone());
    Ok(match name.as_str() {
        "block-input" => true,
        "unblock-input" => false,
        _ => s.get_toggle_option(name),
    })
}

/// The stored toggles of the running session (empty when there is none).
#[tauri::command]
pub fn session_get_toggles() -> HashMap<String, bool> {
    let mut out = HashMap::new();
    if let Some((_, s)) = current() {
        for n in STORED_TOGGLES {
            out.insert(n.to_owned(), s.get_toggle_option(n.to_owned()));
        }
    }
    out
}

/// `"auto" | "sharp" | "smooth"`.
#[tauri::command]
pub fn session_quality(level: String) -> Result<(), String> {
    let v = quality_value(&level).ok_or_else(|| format!("Qualidade desconhecida: {level}"))?;
    require_session()?.save_image_quality(v.to_owned());
    Ok(())
}

#[tauri::command]
pub fn session_get_quality() -> String {
    current()
        .map(|(_, s)| quality_level(&s.get_image_quality()).to_owned())
        .unwrap_or_else(|| "auto".to_owned())
}

#[tauri::command]
pub fn session_fps(n: u32) -> Result<(), String> {
    require_session()?.set_custom_fps(clamp_fps(n));
    Ok(())
}

#[tauri::command]
pub fn session_restart() -> Result<(), String> {
    require_session()?.restart_remote_device();
    Ok(())
}

#[tauri::command]
pub fn session_ctrl_alt_del() -> Result<(), String> {
    require_session()?.ctrl_alt_del();
    Ok(())
}

#[tauri::command]
pub fn session_lock() -> Result<(), String> {
    require_session()?.lock_screen();
    Ok(())
}

/// Longest chat message, in characters (the engine has no limit of its own).
pub const CHAT_MAX_CHARS: usize = 4000;

/// A chat message as it goes out: trimmed, not empty, at most `CHAT_MAX_CHARS` characters.
/// Shared by both sides (`session_send_chat`, `cm::cm_send_chat`).
pub fn chat_text(text: &str) -> Result<String, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("Mensagem vazia".to_owned());
    }
    if t.chars().count() > CHAT_MAX_CHARS {
        return Err(format!("Mensagem longa demais (até {CHAT_MAX_CHARS} caracteres)"));
    }
    Ok(t.to_owned())
}

/// Sends a chat message to the host of the running session.
#[tauri::command]
pub fn session_send_chat(text: String) -> Result<(), String> {
    let t = chat_text(&text)?;
    require_session()?.send_chat(t);
    Ok(())
}

/// "Voz" in the pill: `start` asks the host for a voice call (`session:voice` waiting, then
/// on or off), `!start` ends or cancels it (`session:voice` off).
#[tauri::command]
pub fn session_voice(start: bool) -> Result<(), String> {
    let s = require_session()?;
    if start {
        s.request_voice_call();
    } else {
        s.close_voice_call();
    }
    Ok(())
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
    #[test]
    fn toggles_are_engine_names() {
        let client = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/client.rs"
        ))
        .expect("read src/client.rs");
        for n in TOGGLES {
            assert!(client.contains(&format!("\"{n}\"")), "{n} not in client.rs");
        }
        for n in STORED_TOGGLES {
            assert!(toggle_allowed(n));
        }
    }

    #[test]
    fn toggle_list_is_closed() {
        for n in ["privacy-mode", "follow-remote-window", "", "VIEW-ONLY", "custom-x"] {
            assert!(!toggle_allowed(n), "{n}");
        }
        assert!(session_toggle("privacy-mode".into()).is_err());
    }

    #[test]
    fn quality_mapping() {
        assert_eq!(quality_value("auto"), Some("balanced"));
        assert_eq!(quality_value("sharp"), Some("best"));
        assert_eq!(quality_value("smooth"), Some("low"));
        assert_eq!(quality_value("best"), None);
        for l in ["auto", "sharp", "smooth"] {
            assert_eq!(quality_level(quality_value(l).unwrap()), l);
        }
        assert_eq!(quality_level("custom"), "auto");
    }

    #[test]
    fn fps_is_clamped() {
        assert_eq!(clamp_fps(0), 5);
        assert_eq!(clamp_fps(30), 30);
        assert_eq!(clamp_fps(240), 60);
    }

    #[test]
    fn chat_text_is_trimmed_and_bounded() {
        assert_eq!(chat_text("  oi  \n").unwrap(), "oi");
        assert!(chat_text("").is_err());
        assert!(chat_text(" \n\t ").is_err());
        let max = "é".repeat(CHAT_MAX_CHARS);
        assert_eq!(chat_text(&max).unwrap(), max);
        assert!(chat_text(&format!("{max}a")).is_err());
        // inner newlines stay (Shift+Enter)
        assert_eq!(chat_text("a\nb").unwrap(), "a\nb");
    }
}
