//! `TauriSessionHandler`: the engine's `InvokeUiSession` implemented on top of
//! Tauri events (control/status) and a Tauri `Channel` (video frames).
//!
//! Frame packet (one per decoded frame, sent as raw bytes → `ArrayBuffer` in JS):
//! 16-byte little-endian header `display u32, width u32, height u32, stride u32`
//! followed by `height * stride` bytes of RGBA (stride is always `width * 4`).

use base::message_proto::*;
use hbb_common::rendezvous_proto::ConnType;
use librustdesk::client::QualityStatus;
use librustdesk::ui_session_interface::InvokeUiSession;
use scrap::{ImageFormat, ImageRgb};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tauri::{
    ipc::{Channel, InvokeResponseBody},
    AppHandle, Emitter,
};

pub const FRAME_HEADER_LEN: usize = 16;
/// Frames handed to the webview but not yet drawn (acked through `session_frame_ack`).
/// Above this, new frames are dropped so a hidden/slow webview cannot grow the IPC
/// queue without bound (`Channel::send` has no backpressure).
const MAX_FRAMES_IN_FLIGHT: usize = 2;

/// Generation of the session the UI currently shows. Bumped by every connect and every
/// explicit close, so a superseded session (e.g. an unreachable peer still timing out
/// after "Desconectar") can no longer emit events or frames into the next one.
static CURRENT_GEN: AtomicU64 = AtomicU64::new(0);

/// Starts a new generation and returns it; every older handler becomes stale.
pub fn next_gen() -> u64 {
    CURRENT_GEN.fetch_add(1, Ordering::AcqRel) + 1
}

pub fn is_current_gen(gen: u64) -> bool {
    gen != 0 && CURRENT_GEN.load(Ordering::Acquire) == gen
}

pub fn encode_frame_header(display: u32, width: u32, height: u32, stride: u32) -> [u8; 16] {
    let mut h = [0u8; FRAME_HEADER_LEN];
    h[0..4].copy_from_slice(&display.to_le_bytes());
    h[4..8].copy_from_slice(&width.to_le_bytes());
    h[8..12].copy_from_slice(&height.to_le_bytes());
    h[12..16].copy_from_slice(&stride.to_le_bytes());
    h
}

/// Builds a frame packet from a decoded image of `w`x`h` pixels whose rows are `src_stride`
/// bytes apart. `bgra` = the source is libyuv "ARGB" (B,G,R,A in memory), which is what the
/// engine produces without the Flutter UI; it is swizzled to RGBA for `ImageData`.
/// Row padding is dropped, so the packet stride is `w * 4`.
pub fn build_frame_packet(
    display: u32,
    w: usize,
    h: usize,
    src_stride: usize,
    src: &[u8],
    bgra: bool,
) -> Option<Vec<u8>> {
    let row = w * 4;
    if w == 0 || h == 0 || src_stride < row || src.len() < src_stride * (h - 1) + row {
        return None;
    }
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + row * h);
    out.extend_from_slice(&encode_frame_header(display, w as u32, h as u32, row as u32));
    for y in 0..h {
        let line = &src[y * src_stride..y * src_stride + row];
        if bgra {
            for px in line.chunks_exact(4) {
                out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        } else {
            out.extend_from_slice(line);
        }
    }
    Some(out)
}

#[derive(Clone, Serialize)]
pub struct DisplayRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Serialize)]
struct MsgBox<'a> {
    kind: &'a str,
    title: &'a str,
    text: &'a str,
}

#[derive(Clone, Serialize)]
struct Quality {
    fps: Option<i32>,
    delay: Option<i32>,
    speed: Option<String>,
}

/// `session:job` — one file-transfer job update.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct JobEvent {
    pub id: i32,
    pub file_num: i32,
    /// `"progress" | "done" | "error"`.
    pub kind: &'static str,
    /// Bytes/s (progress only, else 0).
    pub speed: f64,
    /// Bytes transferred so far in the job (progress only, else 0).
    pub finished: f64,
    /// Error text (error only).
    pub err: Option<String>,
}

impl JobEvent {
    pub fn progress(id: i32, file_num: i32, speed: f64, finished: f64) -> Self {
        Self { id, file_num, kind: "progress", speed, finished, err: None }
    }
    pub fn done(id: i32, file_num: i32) -> Self {
        Self { id, file_num, kind: "done", speed: 0.0, finished: 0.0, err: None }
    }
    pub fn error(id: i32, file_num: i32, err: String) -> Self {
        Self { id, file_num, kind: "error", speed: 0.0, finished: 0.0, err: Some(err) }
    }
}

/// `session:override` — the destination file already exists; the UI must answer.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct OverrideEvent {
    pub id: i32,
    pub file_num: i32,
    pub to: String,
    pub is_upload: bool,
    pub is_identical: bool,
}

/// `session:folder` — a directory listing (or count) arrived. Entries are not sent.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct FolderEvent {
    pub id: i32,
    pub path: String,
    pub is_local: bool,
    pub only_count: bool,
    pub count: usize,
}

/// `session:confirm_delete` — the engine asks to confirm deleting a file of job `id`.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct ConfirmDeleteEvent {
    pub id: i32,
    pub file_num: i32,
    pub name: String,
}

/// `session:permission` — the host granted/revoked a permission (`keyboard`, `file`, ...).
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct PermissionEvent {
    pub name: String,
    pub value: bool,
}

/// `session:block_input` / `session:record`.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct OnEvent {
    pub on: bool,
}

/// `session:chat` — a chat message from the host. `at` = unix ms on receipt.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct ChatEvent {
    pub text: String,
    pub at: u64,
}

/// `session:voice` — `state`: `"waiting" | "on" | "off" | "incoming"`; `reason` only on `off`.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct VoiceEvent {
    pub state: &'static str,
    pub reason: Option<String>,
}

pub(crate) fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, Default)]
pub struct TauriSessionHandler {
    app: Option<AppHandle>,
    frames: Option<Channel<InvokeResponseBody>>,
    in_flight: Arc<AtomicUsize>,
    gen: u64,
    /// Host's home dir: the path of the first remote (`is_local == false`) folder listing,
    /// which is the home directory the engine requests when file transfer opens.
    remote_home: Arc<Mutex<Option<String>>>,
}

impl TauriSessionHandler {
    pub fn new(app: AppHandle, frames: Channel<InvokeResponseBody>, gen: u64) -> Self {
        Self {
            app: Some(app),
            frames: Some(frames),
            in_flight: Default::default(),
            gen,
            remote_home: Default::default(),
        }
    }

    pub fn is_current(&self) -> bool {
        is_current_gen(self.gen)
    }

    pub fn frame_ack(&self) {
        let _ = self
            .in_flight
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
    }

    pub fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if !self.is_current() {
            log::debug!("everydesk: dropped {event} from stale session {}", self.gen);
            return;
        }
        if let Some(app) = &self.app {
            if let Err(e) = app.emit(event, payload) {
                log::warn!("everydesk: emit {event} failed: {e}");
            }
        }
    }

    /// Host's home directory, once the first remote folder listing has arrived.
    /// Reachable through the session too (`Session<T>` derefs to the handler).
    #[allow(dead_code)] // consumed by the file-transfer commands (Task 4)
    pub fn remote_home(&self) -> Option<String> {
        self.remote_home.lock().ok().and_then(|g| g.clone())
    }

    /// Stores `path` as the host's home if this is a full remote listing and none is
    /// stored yet. `only_count` events (sub-directory counts) and empty paths never count.
    fn note_folder(&self, path: &str, is_local: bool, only_count: bool) {
        if is_local || only_count || path.is_empty() {
            return;
        }
        if let Ok(mut g) = self.remote_home.lock() {
            if g.is_none() {
                *g = Some(path.to_owned());
            }
        }
    }

    fn emit_displays(&self, displays: &[DisplayInfo]) {
        let v: Vec<DisplayRect> = displays
            .iter()
            .map(|d| DisplayRect {
                x: d.x,
                y: d.y,
                w: d.width,
                h: d.height,
            })
            .collect();
        self.emit("session:displays", v);
    }
}

impl InvokeUiSession for TauriSessionHandler {
    fn on_rgba(&self, display: usize, rgba: &mut ImageRgb) {
        let Some(ch) = &self.frames else { return };
        if !self.is_current() {
            return;
        }
        let bgra = match rgba.fmt() {
            ImageFormat::ARGB => true,
            ImageFormat::ABGR => false,
            ImageFormat::Raw => return, // not produced for pixel-buffer rendering
        };
        if self.in_flight.load(Ordering::Acquire) >= MAX_FRAMES_IN_FLIGHT {
            return; // webview behind: drop this frame
        }
        let (w, h) = (rgba.w, rgba.h);
        if h == 0 {
            return;
        }
        let stride = rgba.raw.len() / h;
        // One allocation per frame: Channel::send takes ownership of the bytes, so the
        // packet buffer cannot be reused across frames.
        let Some(packet) = build_frame_packet(display as u32, w, h, stride, &rgba.raw, bgra)
        else {
            return;
        };
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        if ch.send(InvokeResponseBody::Raw(packet)).is_err() {
            self.frame_ack();
        }
    }

    fn set_displays(&self, displays: &Vec<DisplayInfo>) {
        self.emit_displays(displays);
    }

    fn set_peer_info(&self, peer_info: &PeerInfo) {
        self.emit_displays(&peer_info.displays);
    }

    fn on_connected(&self, _conn_type: ConnType) {
        self.emit("session:connected", ());
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, _link: &str, _retry: bool) {
        self.emit(
            "session:msgbox",
            MsgBox {
                kind: msgtype,
                title,
                text,
            },
        );
    }

    fn update_quality_status(&self, qs: QualityStatus) {
        // Only changed fields are set; the frontend merges them.
        self.emit(
            "session:quality",
            Quality {
                fps: qs.fps.values().next().copied(),
                delay: qs.delay,
                speed: qs.speed,
            },
        );
    }

    fn set_permission(&self, name: &str, value: bool) {
        self.emit(
            "session:permission",
            PermissionEvent { name: name.to_owned(), value },
        );
    }

    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64) {
        self.emit("session:job", JobEvent::progress(id, file_num, speed, finished_size));
    }

    fn job_done(&self, id: i32, file_num: i32) {
        self.emit("session:job", JobEvent::done(id, file_num));
    }

    fn job_error(&self, id: i32, err: String, file_num: i32) {
        self.emit("session:job", JobEvent::error(id, file_num, err));
    }

    fn clear_all_jobs(&self) {
        self.emit("session:jobs_cleared", serde_json::json!({}));
    }

    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    ) {
        self.note_folder(&path, is_local, only_count);
        self.emit(
            "session:folder",
            FolderEvent { id, path, is_local, only_count, count: entries.len() },
        );
    }

    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        to: String,
        is_upload: bool,
        is_identical: bool,
    ) {
        self.emit(
            "session:override",
            OverrideEvent { id, file_num, to, is_upload, is_identical },
        );
    }

    fn confirm_delete_files(&self, id: i32, i: i32, name: String) {
        self.emit("session:confirm_delete", ConfirmDeleteEvent { id, file_num: i, name });
    }

    fn update_block_input_state(&self, on: bool) {
        self.emit("session:block_input", OnEvent { on });
    }

    fn update_record_status(&self, start: bool) {
        self.emit("session:record", OnEvent { on: start });
    }

    fn new_message(&self, msg: String) {
        self.emit("session:chat", ChatEvent { text: msg, at: unix_ms() });
    }

    fn on_voice_call_waiting(&self) {
        self.emit("session:voice", VoiceEvent { state: "waiting", reason: None });
    }

    fn on_voice_call_started(&self) {
        self.emit("session:voice", VoiceEvent { state: "on", reason: None });
    }

    fn on_voice_call_closed(&self, reason: &str) {
        self.emit(
            "session:voice",
            VoiceEvent { state: "off", reason: Some(reason.to_owned()) },
        );
    }

    fn on_voice_call_incoming(&self) {
        self.emit("session:voice", VoiceEvent { state: "incoming", reason: None });
    }

    // Not used by this UI (Flutter-only persisted transfer list, cursor, etc.).
    fn update_transfer_list(&self) {}
    fn load_last_job(&self, _cnt: i32, _job_json: &str, _auto_start: bool) {}
    fn set_cursor_data(&self, _cd: CursorData) {}
    fn set_cursor_id(&self, _id: String) {}
    fn set_cursor_position(&self, _cp: CursorPosition) {}
    fn set_display(&self, _x: i32, _y: i32, _w: i32, _h: i32, _cursor_embedded: bool, _scale: f64) {
    }
    fn switch_display(&self, _display: &SwitchDisplay) {}
    fn set_platform_additions(&self, _data: &str) {}
    fn update_privacy_mode(&self) {}
    fn close_success(&self) {}
    fn set_connection_type(&self, _is_secured: bool, _direct: bool, _stream_type: &str) {}
    fn set_fingerprint(&self, _fingerprint: String) {}
    fn adapt_size(&self) {}
    fn cancel_msgbox(&self, _tag: &str) {}
    fn switch_back(&self, _id: &str) {}
    fn portable_service_running(&self, _running: bool) {}
    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }
    fn next_rgba(&self, _display: usize) {}
    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
    fn set_current_display(&self, _disp_idx: i32) {}
    fn printer_request(&self, _id: i32, _path: String) {}
    fn handle_screenshot_resp(&self, _sid: String, _msg: String) {}
    fn handle_terminal_response(&self, _response: TerminalResponse) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superseded_generation_is_stale() {
        let old = next_gen();
        assert!(is_current_gen(old));
        let h = TauriSessionHandler { gen: old, ..Default::default() };
        assert!(h.is_current());
        let new = next_gen(); // reconnect or explicit close
        assert!(!is_current_gen(old));
        assert!(!h.is_current());
        assert!(is_current_gen(new));
        assert!(!TauriSessionHandler::default().is_current()); // gen 0 never current
    }

    #[test]
    fn frame_header_is_le_u32s() {
        assert_eq!(
            encode_frame_header(1, 1920, 1080, 7680),
            [1, 0, 0, 0, 0x80, 0x07, 0, 0, 0x38, 0x04, 0, 0, 0, 0x1e, 0, 0]
        );
        assert_eq!(encode_frame_header(0, 0, 0, 0), [0u8; 16]);
    }

    #[test]
    fn packet_swizzles_bgra_and_drops_padding() {
        // 2x2 image, source stride 12 (4 bytes of padding per row).
        let src = [
            1, 2, 3, 4, 5, 6, 7, 8, 0xEE, 0xEE, 0xEE, 0xEE, //
            9, 10, 11, 12, 13, 14, 15, 16, 0xEE, 0xEE, 0xEE, 0xEE,
        ];
        let p = build_frame_packet(3, 2, 2, 12, &src, true).unwrap();
        assert_eq!(&p[..16], &encode_frame_header(3, 2, 2, 8));
        assert_eq!(
            &p[16..],
            &[3, 2, 1, 4, 7, 6, 5, 8, 11, 10, 9, 12, 15, 14, 13, 16]
        );
        let p = build_frame_packet(0, 2, 2, 12, &src, false).unwrap();
        assert_eq!(&p[16..], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    }

    #[test]
    fn packet_rejects_short_buffers() {
        assert!(build_frame_packet(0, 2, 2, 8, &[0u8; 15], true).is_none());
        assert!(build_frame_packet(0, 0, 2, 8, &[0u8; 16], true).is_none());
        assert!(build_frame_packet(0, 2, 2, 4, &[0u8; 16], true).is_none());
    }

    fn json<S: Serialize>(v: S) -> serde_json::Value {
        serde_json::to_value(v).unwrap()
    }

    #[test]
    fn job_payloads() {
        use serde_json::json;
        assert_eq!(
            json(JobEvent::progress(3, 1, 1024.5, 2048.0)),
            json!({"id":3,"file_num":1,"kind":"progress","speed":1024.5,"finished":2048.0,"err":null})
        );
        assert_eq!(
            json(JobEvent::done(3, 2)),
            json!({"id":3,"file_num":2,"kind":"done","speed":0.0,"finished":0.0,"err":null})
        );
        assert_eq!(
            json(JobEvent::error(4, 0, "sem permissão".into())),
            json!({"id":4,"file_num":0,"kind":"error","speed":0.0,"finished":0.0,"err":"sem permissão"})
        );
    }

    #[test]
    fn override_folder_permission_voice_payloads() {
        use serde_json::json;
        assert_eq!(
            json(OverrideEvent { id: 1, file_num: 2, to: "/tmp/a".into(), is_upload: true, is_identical: false }),
            json!({"id":1,"file_num":2,"to":"/tmp/a","is_upload":true,"is_identical":false})
        );
        assert_eq!(
            json(FolderEvent { id: 0, path: "C:\\Users\\x".into(), is_local: false, only_count: false, count: 7 }),
            json!({"id":0,"path":"C:\\Users\\x","is_local":false,"only_count":false,"count":7})
        );
        assert_eq!(
            json(PermissionEvent { name: "file".into(), value: false }),
            json!({"name":"file","value":false})
        );
        assert_eq!(
            json(VoiceEvent { state: "off", reason: Some("recusado".into()) }),
            json!({"state":"off","reason":"recusado"})
        );
        assert_eq!(
            json(VoiceEvent { state: "waiting", reason: None }),
            json!({"state":"waiting","reason":null})
        );
        assert_eq!(json(OnEvent { on: true }), json!({"on":true}));
        assert_eq!(
            json(ChatEvent { text: "oi".into(), at: 5 }),
            json!({"text":"oi","at":5})
        );
        assert_eq!(
            json(ConfirmDeleteEvent { id: 1, file_num: 0, name: "a.txt".into() }),
            json!({"id":1,"file_num":0,"name":"a.txt"})
        );
    }

    #[test]
    fn remote_home_set_once_from_first_remote_listing() {
        // Stale (gen 0) handler: emits are dropped, but state is still tracked.
        let h = TauriSessionHandler::default();
        assert_eq!(h.remote_home(), None);
        h.update_folder_files(0, &vec![], "/local/home".into(), true, false);
        assert_eq!(h.remote_home(), None); // local listings never set it
        h.update_folder_files(0, &vec![FileEntry::default()], "/Users/host".into(), false, false);
        assert_eq!(h.remote_home().as_deref(), Some("/Users/host"));
        h.update_folder_files(0, &vec![], "/Users/host/Docs".into(), false, false);
        assert_eq!(h.remote_home().as_deref(), Some("/Users/host")); // first wins
        let clone = h.clone(); // the session's handler clones share the state
        assert_eq!(clone.remote_home().as_deref(), Some("/Users/host"));
    }

    #[test]
    fn only_count_folder_does_not_set_remote_home() {
        let h = TauriSessionHandler::default();
        h.update_folder_files(0, &vec![], "/Users/host/Docs/sub".into(), false, true);
        assert_eq!(h.remote_home(), None); // count-only listing of a sub-dir
        h.update_folder_files(0, &vec![], String::new(), false, false);
        assert_eq!(h.remote_home(), None); // empty path
        h.update_folder_files(0, &vec![], "/Users/host".into(), false, false);
        assert_eq!(h.remote_home().as_deref(), Some("/Users/host"));
    }

    #[test]
    fn new_handler_resets_remote_home() {
        let old = TauriSessionHandler::default();
        old.update_folder_files(0, &vec![], "/Users/host".into(), false, false);
        assert!(old.remote_home().is_some());
        // A new connection builds a fresh handler: nothing carried over.
        assert_eq!(TauriSessionHandler::default().remote_home(), None);
    }

    #[test]
    fn unix_ms_is_plausible() {
        assert!(unix_ms() > 1_700_000_000_000);
    }
}
