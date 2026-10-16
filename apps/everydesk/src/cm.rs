//! Accessed side: the app is RustDesk's connection manager. The in-process host server
//! (`start_server`) reaches it over the `_cm` IPC, so the engine never spawns `--cm`.
//! Events: `cm:incoming` (accept prompt), `cm:connected` (authorized session),
//! `cm:removed` (connection gone, including the 60 s click-approve timeout),
//! `cm:pair_offer {conn_id, peer_id, name, key_changed}` (the pending prompt `conn_id` comes
//! from an authenticated controller that can be remembered; sent right after its
//! `cm:incoming`), `cm:reconfirm {conn_id, ring_secs}` (periodic "still connected?" ring for
//! an authorized session; the session keeps streaming meanwhile).

use hbb_common::config::Config;
use librustdesk::ui_cm_interface::{self, Client, ConnectionManager, InvokeUiCM};
use crate::contacts::PolicyView;
use serde::Serialize;
use std::{collections::BTreeMap, sync::Mutex, time::Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct TauriCM {
    app: AppHandle,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Incoming {
    pub conn_id: i32,
    pub peer_id: String,
    pub name: String,
    pub is_file_transfer: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Connected {
    pub conn_id: i32,
    pub peer_id: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Removed {
    pub conn_id: i32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PairOffer {
    pub conn_id: i32,
    pub peer_id: String,
    pub name: String,
    /// This id is already paired here under another key (reinstalled peer or impostor).
    pub key_changed: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Reconfirm {
    pub conn_id: i32,
    pub ring_secs: u32,
}

/// Which event a `Client` reported by the engine maps to.
#[derive(Debug, PartialEq)]
pub enum CmEvent {
    Incoming(Incoming),
    Connected(Connected),
}

pub fn event_for(client: &Client) -> CmEvent {
    event_from(
        client.id,
        client.authorized,
        &client.peer_id,
        &client.name,
        client.is_file_transfer,
    )
}

/// Pure part of `event_for` (`Client` can't be built outside the engine: private `tx`).
pub fn event_from(
    conn_id: i32,
    authorized: bool,
    peer_id: &str,
    name: &str,
    is_file_transfer: bool,
) -> CmEvent {
    if authorized {
        CmEvent::Connected(Connected {
            conn_id,
            peer_id: peer_id.to_owned(),
            name: name.to_owned(),
        })
    } else {
        CmEvent::Incoming(Incoming {
            conn_id,
            peer_id: peer_id.to_owned(),
            name: name.to_owned(),
            is_file_transfer,
        })
    }
}

/// What the UI shows, kept here too so a webview that mounts after an event can hydrate
/// through `cm_pending` (Tauri events are not replayed).
#[derive(Default)]
pub struct CmState {
    pending: BTreeMap<i32, (Incoming, Instant)>,
    connected: BTreeMap<i32, Connected>,
    offers: BTreeMap<i32, PairOffer>,
    reconfirms: BTreeMap<i32, (Reconfirm, Instant)>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PendingSnapshot {
    #[serde(flatten)]
    pub incoming: Incoming,
    /// How long the prompt has been up, so the UI countdown starts at the right second.
    pub age_ms: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ReconfirmSnapshot {
    #[serde(flatten)]
    pub reconfirm: Reconfirm,
    /// How long the ring has been up.
    pub age_ms: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct CmSnapshot {
    pub pending: Vec<PendingSnapshot>,
    pub connected: Vec<Connected>,
    /// Pair offers of the pending prompts.
    pub offers: Vec<PairOffer>,
    /// Unanswered reconfirmation rings.
    pub reconfirms: Vec<ReconfirmSnapshot>,
}

impl CmState {
    pub fn apply(&mut self, ev: &CmEvent, now: Instant) {
        match ev {
            CmEvent::Incoming(p) => {
                self.connected.remove(&p.conn_id);
                self.pending.insert(p.conn_id, (p.clone(), now));
            }
            CmEvent::Connected(c) => {
                self.pending.remove(&c.conn_id);
                self.connected.insert(c.conn_id, c.clone());
            }
        }
    }

    pub fn offer(&mut self, o: PairOffer) {
        if self.pending.contains_key(&o.conn_id) {
            self.offers.insert(o.conn_id, o);
        }
    }

    pub fn ring(&mut self, r: Reconfirm, now: Instant) {
        self.reconfirms.insert(r.conn_id, (r, now));
    }

    /// The ring for `conn_id` was answered.
    pub fn answered(&mut self, conn_id: i32) {
        self.reconfirms.remove(&conn_id);
    }

    /// The UI accepted `conn_id`: its prompt becomes a connected session.
    pub fn authorize(&mut self, conn_id: i32) {
        self.offers.remove(&conn_id);
        if let Some((p, _)) = self.pending.remove(&conn_id) {
            self.connected.insert(
                conn_id,
                Connected {
                    conn_id,
                    peer_id: p.peer_id,
                    name: p.name,
                },
            );
        }
    }

    pub fn remove(&mut self, conn_id: i32) {
        self.pending.remove(&conn_id);
        self.connected.remove(&conn_id);
        self.offers.remove(&conn_id);
        self.reconfirms.remove(&conn_id);
    }

    pub fn snapshot(&self, now: Instant) -> CmSnapshot {
        CmSnapshot {
            pending: self
                .pending
                .values()
                .map(|(p, at)| PendingSnapshot {
                    incoming: p.clone(),
                    age_ms: now.saturating_duration_since(*at).as_millis() as u64,
                })
                .collect(),
            connected: self.connected.values().cloned().collect(),
            offers: self.offers.values().cloned().collect(),
            reconfirms: self
                .reconfirms
                .values()
                .map(|(r, at)| ReconfirmSnapshot {
                    reconfirm: r.clone(),
                    age_ms: now.saturating_duration_since(*at).as_millis() as u64,
                })
                .collect(),
        }
    }
}

static STATE: Mutex<Option<CmState>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut CmState) -> R) -> R {
    f(STATE.lock().unwrap().get_or_insert_with(CmState::default))
}

impl TauriCM {
    fn emit_client(&self, client: &Client) {
        let ev = event_for(client);
        with_state(|s| s.apply(&ev, Instant::now()));
        let r = match ev {
            CmEvent::Incoming(p) => {
                log::info!("everydesk: incoming #{} from {}", p.conn_id, p.peer_id);
                self.app.emit("cm:incoming", p)
            }
            CmEvent::Connected(p) => {
                log::info!("everydesk: connected #{} from {}", p.conn_id, p.peer_id);
                self.app.emit("cm:connected", p)
            }
        };
        if let Err(e) = r {
            log::error!("everydesk: cm emit failed: {e}");
        }
    }

    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Err(e) = self.app.emit(event, payload) {
            log::error!("everydesk: cm emit {event} failed: {e}");
        }
    }
}

impl InvokeUiCM for TauriCM {
    fn add_connection(&self, client: &Client) {
        self.emit_client(client);
    }

    fn remove_connection(&self, id: i32, _close: bool) {
        log::info!("everydesk: cm removed #{id}");
        with_state(|s| s.remove(id));
        if let Err(e) = self.app.emit("cm:removed", Removed { conn_id: id }) {
            log::error!("everydesk: cm emit failed: {e}");
        }
    }

    // TODO etapa 3/4: chat, theme, language, elevation, voice call, file transfer log.
    fn new_message(&self, _id: i32, _text: String) {}
    fn change_theme(&self, _dark: String) {}
    fn change_language(&self) {}
    fn show_elevation(&self, _show: bool) {}
    fn update_voice_call_state(&self, _client: &Client) {}
    fn file_transfer_log(&self, _action: &str, _log: &str) {}

    fn reconfirm(&self, conn_id: i32, ring_secs: u32) {
        log::info!("everydesk: reconfirm #{conn_id} ({ring_secs} s)");
        let r = Reconfirm { conn_id, ring_secs };
        with_state(|s| s.ring(r.clone(), Instant::now()));
        self.emit("cm:reconfirm", r);
    }

    fn pair_offer(
        &self,
        conn_id: i32,
        peer_id: String,
        name: String,
        _avatar: Option<Vec<u8>>,
        key_changed: bool,
    ) {
        log::info!("everydesk: pair offer #{conn_id} from {peer_id} (key_changed={key_changed})");
        let o = PairOffer {
            conn_id,
            peer_id,
            name,
            key_changed,
        };
        with_state(|s| s.offer(o.clone()));
        self.emit("cm:pair_offer", o);
    }
}

/// EveryDesk always asks: every login waits for the in-app accept prompt. Forced at each start
/// because the config is shared with a stock RustDesk install (same `APP_NAME`), where
/// `password` or both (`password::approve_mode`: anything but `click`/`password`) would let a
/// password login in without a prompt.
pub fn force_click_approve_mode() {
    let cur = Config::get_option("approve-mode");
    if needs_click_override(&cur) {
        log::info!("everydesk: approve-mode {cur:?} -> \"click\"");
        Config::set_option("approve-mode".to_owned(), "click".to_owned());
    }
}

fn needs_click_override(current: &str) -> bool {
    current != "click"
}

/// Serves the `_cm` IPC on its own thread (`start_ipc` blocks on a tokio runtime). Must run
/// before the host server gets its first login, so `try_start_cm` finds it.
pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("everydesk-cm".into())
        .spawn(move || {
            log::info!("everydesk: starting _cm ipc server");
            ui_cm_interface::start_ipc(ConnectionManager {
                ui_handler: TauriCM { app },
            });
            log::error!("everydesk: _cm ipc server stopped");
        })
        .expect("spawn cm thread");
}

/// Accepts a pending prompt. `authorize` does not call back `add_connection`, so the UI
/// moves the card to its "Conectado" indicator itself once this returns.
#[tauri::command]
pub fn cm_authorize(conn_id: i32) {
    log::info!("everydesk: authorize #{conn_id}");
    ui_cm_interface::authorize(conn_id);
    with_state(|s| s.authorize(conn_id));
}

/// Current prompts and sessions, for the UI to hydrate once on mount.
#[tauri::command]
pub fn cm_pending() -> CmSnapshot {
    with_state(|s| s.snapshot(Instant::now()))
}

/// Answer to a `cm:reconfirm` ring: `ok` keeps the session (next ring one period later),
/// `!ok` ends it. Unanswered, the engine ends it after `ring_secs`.
#[tauri::command]
pub fn cm_reconfirm_result(conn_id: i32, ok: bool) {
    log::info!("everydesk: reconfirm #{conn_id} -> {ok}");
    with_state(|s| s.answered(conn_id));
    ui_cm_interface::reconfirm_result(conn_id, ok);
}

/// Accepts the pending prompt `conn_id` (like `cm_authorize`); with `accepted`, also
/// remembers the controller under `policy` (`null` = AskAlways). `replace_key` (default
/// false) is the explicit "Substituir chave guardada" answer for a `key_changed` offer:
/// without it the engine keeps the key already pinned.
#[tauri::command]
pub fn cm_pair_ack(
    conn_id: i32,
    accepted: bool,
    policy: Option<PolicyView>,
    replace_key: Option<bool>,
) {
    let replace_key = replace_key.unwrap_or(false);
    log::info!("everydesk: pair ack #{conn_id} accepted={accepted} policy={policy:?} replace_key={replace_key}");
    ui_cm_interface::pair_ack(conn_id, accepted, policy.map(Into::into), replace_key);
    with_state(|s| s.authorize(conn_id));
}

#[tauri::command]
pub fn cm_close(conn_id: i32) {
    log::info!("everydesk: close #{conn_id}");
    ui_cm_interface::close(conn_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approve_mode_is_forced_to_click() {
        for m in ["", "password", "password-click", "both"] {
            assert!(needs_click_override(m), "{m:?} must be overridden");
        }
        assert!(!needs_click_override("click"));
    }

    #[test]
    fn unauthorized_client_is_an_incoming_prompt() {
        assert_eq!(
            event_from(7, false, "123456789", "pc", true),
            CmEvent::Incoming(Incoming {
                conn_id: 7,
                peer_id: "123456789".into(),
                name: "pc".into(),
                is_file_transfer: true,
            })
        );
    }

    #[test]
    fn authorized_client_is_connected() {
        assert_eq!(
            event_from(7, true, "123456789", "pc", false),
            CmEvent::Connected(Connected {
                conn_id: 7,
                peer_id: "123456789".into(),
                name: "pc".into(),
            })
        );
    }

    #[test]
    fn state_tracks_prompts_and_sessions() {
        let t0 = Instant::now();
        let mut s = CmState::default();
        s.apply(&event_from(1, false, "111", "a", false), t0);
        s.apply(&event_from(2, false, "222", "b", false), t0);
        s.apply(&event_from(3, true, "333", "c", false), t0);
        let snap = s.snapshot(t0 + std::time::Duration::from_millis(1500));
        assert_eq!(snap.pending.len(), 2);
        assert_eq!(snap.pending[0].age_ms, 1500);
        assert_eq!(snap.connected.len(), 1);

        s.authorize(1);
        s.remove(2);
        let snap = s.snapshot(t0);
        assert!(snap.pending.is_empty());
        assert_eq!(
            snap.connected.iter().map(|c| c.conn_id).collect::<Vec<_>>(),
            vec![1, 3]
        );
        s.remove(1);
        s.remove(3);
        assert_eq!(s.snapshot(t0), CmSnapshot::default());
    }

    #[test]
    fn pending_snapshot_serializes_flat() {
        let v = serde_json::to_value(PendingSnapshot {
            incoming: Incoming {
                conn_id: 4,
                peer_id: "9".into(),
                name: "n".into(),
                is_file_transfer: false,
            },
            age_ms: 10,
        })
        .unwrap();
        assert_eq!(v["conn_id"], 4);
        assert_eq!(v["age_ms"], 10);
    }

    #[test]
    fn timeout_message_matches_spec() {
        assert_eq!(
            librustdesk::client::LOGIN_MSG_TIMEOUT,
            "Pedido não respondido em 60 s"
        );
    }

    #[test]
    fn offers_and_rings_are_hydrated_and_cleared() {
        let t0 = Instant::now();
        let mut s = CmState::default();
        let offer = |id| PairOffer {
            conn_id: id,
            peer_id: "abc".into(),
            name: "n".into(),
            key_changed: false,
        };
        // An offer for an unknown conn is dropped.
        s.offer(offer(9));
        s.apply(&event_from(1, false, "abc", "n", false), t0);
        s.offer(offer(1));
        s.ring(Reconfirm { conn_id: 2, ring_secs: 60 }, t0);
        let snap = s.snapshot(t0 + std::time::Duration::from_millis(500));
        assert_eq!(snap.offers, vec![offer(1)]);
        assert_eq!(snap.reconfirms[0].age_ms, 500);
        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(
            json["offers"][0],
            serde_json::json!({"conn_id": 1, "peer_id": "abc", "name": "n", "key_changed": false})
        );
        assert_eq!(
            json["reconfirms"][0],
            serde_json::json!({"conn_id": 2, "ring_secs": 60, "age_ms": 500})
        );
        s.authorize(1);
        s.answered(2);
        let snap = s.snapshot(t0);
        assert!(snap.offers.is_empty() && snap.reconfirms.is_empty());
        s.ring(Reconfirm { conn_id: 1, ring_secs: 60 }, t0);
        s.remove(1);
        assert_eq!(s.snapshot(t0), CmSnapshot::default());
    }
}
