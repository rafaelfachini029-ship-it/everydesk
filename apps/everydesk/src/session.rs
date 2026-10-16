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
    atomic::{AtomicUsize, Ordering},
    Arc,
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

#[derive(Clone, Default)]
pub struct TauriSessionHandler {
    app: Option<AppHandle>,
    frames: Option<Channel<InvokeResponseBody>>,
    in_flight: Arc<AtomicUsize>,
}

impl TauriSessionHandler {
    pub fn new(app: AppHandle, frames: Channel<InvokeResponseBody>) -> Self {
        Self {
            app: Some(app),
            frames: Some(frames),
            in_flight: Default::default(),
        }
    }

    pub fn frame_ack(&self) {
        let _ = self
            .in_flight
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
    }

    pub fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Some(app) = &self.app {
            if let Err(e) = app.emit(event, payload) {
                log::warn!("everydesk: emit {event} failed: {e}");
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

    // TODO etapa 3/4: everything below.
    fn set_cursor_data(&self, _cd: CursorData) {}
    fn set_cursor_id(&self, _id: String) {}
    fn set_cursor_position(&self, _cp: CursorPosition) {}
    fn set_display(&self, _x: i32, _y: i32, _w: i32, _h: i32, _cursor_embedded: bool, _scale: f64) {
    }
    fn switch_display(&self, _display: &SwitchDisplay) {}
    fn set_platform_additions(&self, _data: &str) {}
    fn update_privacy_mode(&self) {}
    fn set_permission(&self, _name: &str, _value: bool) {}
    fn close_success(&self) {}
    fn set_connection_type(&self, _is_secured: bool, _direct: bool, _stream_type: &str) {}
    fn set_fingerprint(&self, _fingerprint: String) {}
    fn job_error(&self, _id: i32, _err: String, _file_num: i32) {}
    fn job_done(&self, _id: i32, _file_num: i32) {}
    fn clear_all_jobs(&self) {}
    fn new_message(&self, _msg: String) {}
    fn update_transfer_list(&self) {}
    fn load_last_job(&self, _cnt: i32, _job_json: &str, _auto_start: bool) {}
    fn update_folder_files(
        &self,
        _id: i32,
        _entries: &Vec<FileEntry>,
        _path: String,
        _is_local: bool,
        _only_count: bool,
    ) {
    }
    fn confirm_delete_files(&self, _id: i32, _i: i32, _name: String) {}
    fn override_file_confirm(
        &self,
        _id: i32,
        _file_num: i32,
        _to: String,
        _is_upload: bool,
        _is_identical: bool,
    ) {
    }
    fn update_block_input_state(&self, _on: bool) {}
    fn job_progress(&self, _id: i32, _file_num: i32, _speed: f64, _finished_size: f64) {}
    fn adapt_size(&self) {}
    fn cancel_msgbox(&self, _tag: &str) {}
    fn switch_back(&self, _id: &str) {}
    fn portable_service_running(&self, _running: bool) {}
    fn on_voice_call_started(&self) {}
    fn on_voice_call_closed(&self, _reason: &str) {}
    fn on_voice_call_waiting(&self) {}
    fn on_voice_call_incoming(&self) {}
    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }
    fn next_rgba(&self, _display: usize) {}
    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
    fn set_current_display(&self, _disp_idx: i32) {}
    fn update_record_status(&self, _start: bool) {}
    fn printer_request(&self, _id: i32, _path: String) {}
    fn handle_screenshot_resp(&self, _sid: String, _msg: String) {}
    fn handle_terminal_response(&self, _response: TerminalResponse) {}
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
