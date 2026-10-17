//! Accessed side: the app is RustDesk's connection manager. The in-process host server
//! (`start_server`) reaches it over the `_cm` IPC, so the engine never spawns `--cm`.
//! Events: `cm:incoming` (accept prompt), `cm:connected` (authorized session),
//! `cm:removed` (connection gone, including the 60 s click-approve timeout),
//! `cm:pair_offer {conn_id, peer_id, name, key_changed}` (the pending prompt `conn_id` comes
//! from an authenticated controller that can be remembered; sent right after its
//! `cm:incoming`), `cm:reconfirm {conn_id, ring_secs}` (periodic "still connected?" ring for
//! an authorized session; the session keeps streaming meanwhile),
//! `cm:identity {conn_id, state: "verified"|"unverified"|"key_changed", known}` (who is
//! asking, sent for every prompt right after its `cm:incoming`; `known`: the claimed id is in
//! one of the books, so an unverified or re-keyed claim is a loud warning),
//! `cm:elevation {conn_id, state: "ask"|"auto"|"done"}` (Windows portable run: the controller
//! needs administrator rights; "ask" shows the card once per episode, "auto" means the
//! remembered choice already asked Windows for it, "done" means the elevated helper runs or
//! is no longer needed).
//! `cm:chat {conn_id, text, at, mine: false}` (a chat message from the controller `conn_id`;
//! `at` = unix ms on receipt). The last `CHAT_KEEP` messages of each connection, both ways,
//! come back in `cm_pending` (`chats`); `cm_send_chat` answers.

use hbb_common::config::Config;
use librustdesk::ui_cm_interface::{self, Client, ConnectionManager, InvokeUiCM};
use crate::contacts::PolicyView;
use librustdesk::everydesk::pairings::{IdentityState, Policy};
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
    time::Instant,
};
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
pub struct Identity {
    pub conn_id: i32,
    pub state: IdentityState,
    /// The claimed id is in the host or the contacts book.
    pub known: bool,
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
    identities: BTreeMap<i32, Identity>,
    reconfirms: BTreeMap<i32, (Reconfirm, Instant)>,
    elevation: Elevation,
    chats: BTreeMap<i32, VecDeque<CmChat>>,
}

/// Chat messages kept per connection for `cm_pending` hydration.
pub const CHAT_KEEP: usize = 50;

/// `cm:chat`, and one entry of `CmSnapshot::chats`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CmChat {
    pub conn_id: i32,
    pub text: String,
    /// Unix ms.
    pub at: u64,
    /// Sent from this machine (a reply), not received.
    pub mine: bool,
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
    /// Identity of the pending prompts.
    pub identities: Vec<Identity>,
    /// Unanswered reconfirmation rings.
    pub reconfirms: Vec<ReconfirmSnapshot>,
    /// Recent chat messages (both ways), per connection in order, `CHAT_KEEP` at most each.
    pub chats: Vec<CmChat>,
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

    pub fn identity(&mut self, i: Identity) {
        if self.pending.contains_key(&i.conn_id) {
            self.identities.insert(i.conn_id, i);
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
        self.identities.remove(&conn_id);
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
        self.elevation.remove(conn_id);
        self.pending.remove(&conn_id);
        self.connected.remove(&conn_id);
        self.offers.remove(&conn_id);
        self.identities.remove(&conn_id);
        self.reconfirms.remove(&conn_id);
        self.chats.remove(&conn_id);
    }

    /// Keeps a chat message (the oldest of the connection goes past `CHAT_KEEP`).
    pub fn chat(&mut self, m: CmChat) {
        let q = self.chats.entry(m.conn_id).or_default();
        q.push_back(m);
        while q.len() > CHAT_KEEP {
            q.pop_front();
        }
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
            identities: self.identities.values().cloned().collect(),
            reconfirms: self
                .reconfirms
                .values()
                .map(|(r, at)| ReconfirmSnapshot {
                    reconfirm: r.clone(),
                    age_ms: now.saturating_duration_since(*at).as_millis() as u64,
                })
                .collect(),
            chats: self.chats.values().flatten().cloned().collect(),
        }
    }
}

/// Config option holding the remembered answer to the elevation card: "Y" elevate without
/// asking, "N" never ask, anything else ask.
pub const AUTO_ELEVATE_OPTION: &str = "everydesk-auto-elevate";

/// What to do when the engine says administrator rights are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Remembered "Permitir sempre": request the elevated helper without asking.
    Auto,
    /// Show the card.
    Ask,
    /// Nothing: not needed, not possible here (installed, or not Windows), or "não perguntar
    /// mais".
    Skip,
}

pub fn elevation_decision(remembered: &str, show: bool, can: bool) -> Decision {
    if !show || !can {
        return Decision::Skip;
    }
    match remembered {
        "Y" => Decision::Auto,
        "N" => Decision::Skip,
        _ => Decision::Ask,
    }
}

/// The remembered answer as the UI sees it: "Y", "N" or "" (ask).
pub fn normalize_setting(v: &str) -> &'static str {
    match v {
        "Y" => "Y",
        "N" => "N",
        _ => "",
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ElevationState {
    Ask,
    Auto,
    Done,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ElevationEvent {
    pub conn_id: i32,
    pub state: ElevationState,
}

/// One "needs elevation" episode. The engine repeats `show_elevation(true)` every second per
/// connection (`portable_check`) until the helper runs, so the card (or the automatic
/// request) happens once per episode; the episode ends on `show == false` or when its
/// connection goes. A dismissed card stays dismissed for the episode.
#[derive(Default, Debug)]
struct Elevation {
    /// Connection the episode was decided for.
    latched: Option<i32>,
    /// The card is up and unanswered.
    asking: bool,
}

impl Elevation {
    /// `target`: the session the request is routed through (`show_elevation` carries no id;
    /// the helper is machine-wide, so any authorized session does). Before any session is
    /// authorized nothing latches, so the first tick after the accept decides.
    fn step(&mut self, show: bool, target: Option<i32>, d: Decision) -> Option<ElevationEvent> {
        if !show {
            let conn_id = self.latched.take()?;
            self.asking = false;
            return Some(ElevationEvent {
                conn_id,
                state: ElevationState::Done,
            });
        }
        if self.latched.is_some() {
            return None;
        }
        let conn_id = target?;
        self.latched = Some(conn_id);
        let state = match d {
            Decision::Auto => ElevationState::Auto,
            Decision::Ask => {
                self.asking = true;
                ElevationState::Ask
            }
            Decision::Skip => return None,
        };
        Some(ElevationEvent { conn_id, state })
    }

    fn answered(&mut self) {
        self.asking = false;
    }

    fn remove(&mut self, conn_id: i32) {
        if self.latched == Some(conn_id) {
            *self = Self::default();
        }
    }

    fn pending(&self) -> Option<i32> {
        self.latched.filter(|_| self.asking)
    }
}

impl CmState {
    /// Newest authorized session, the one an elevation request is routed through.
    fn elevation_target(&self) -> Option<i32> {
        self.connected.keys().next_back().copied()
    }

    fn elevation_step(&mut self, show: bool, d: Decision) -> Option<ElevationEvent> {
        let target = self.elevation_target();
        self.elevation.step(show, target, d)
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

    // TODO etapa 3/4: theme, language, voice call, file transfer log.
    fn new_message(&self, id: i32, text: String) {
        let m = CmChat {
            conn_id: id,
            text,
            at: crate::session::unix_ms(),
            mine: false,
        };
        with_state(|s| s.chat(m.clone()));
        self.emit("cm:chat", m);
    }
    fn change_theme(&self, _dark: String) {}
    fn change_language(&self) {}
    fn show_elevation(&self, show: bool) {
        let remembered = Config::get_option(AUTO_ELEVATE_OPTION);
        let d = elevation_decision(&remembered, show, ui_cm_interface::can_elevate());
        let Some(ev) = with_state(|s| s.elevation_step(show, d)) else {
            return;
        };
        log::info!("everydesk: elevation #{} {:?}", ev.conn_id, ev.state);
        if ev.state == ElevationState::Auto {
            ui_cm_interface::elevate_portable(ev.conn_id);
        }
        self.emit("cm:elevation", ev);
    }
    fn update_voice_call_state(&self, _client: &Client) {}
    fn file_transfer_log(&self, _action: &str, _log: &str) {}

    fn reconfirm(&self, conn_id: i32, ring_secs: u32) {
        log::info!("everydesk: reconfirm #{conn_id} ({ring_secs} s)");
        let r = Reconfirm { conn_id, ring_secs };
        with_state(|s| s.ring(r.clone(), Instant::now()));
        self.emit("cm:reconfirm", r);
    }

    fn identity(&self, conn_id: i32, state: IdentityState, known: bool) {
        log::info!("everydesk: identity #{conn_id} {state:?} (known={known})");
        let i = Identity {
            conn_id,
            state,
            known,
        };
        with_state(|s| s.identity(i.clone()));
        self.emit("cm:identity", i);
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
/// so that no stored `approve-mode` (`password` or both; `password::approve_mode`: anything
/// but `click`/`password`) can ever let a password login in without a prompt. The config is
/// EveryDesk's own (`APP_NAME` "EveryDesk", see docs/caminhos.md), not RustDesk's.
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
/// remembers the controller under `policy` (`null` = AskAlways; a Reconfirm period outside
/// 1..=1440 min is rejected before anything reaches the engine). `replace_key` (default
/// false) is the explicit "Substituir chave guardada" answer for a `key_changed` offer:
/// without it the engine keeps the key already pinned.
#[tauri::command]
pub fn cm_pair_ack(
    conn_id: i32,
    accepted: bool,
    policy: Option<PolicyView>,
    replace_key: Option<bool>,
) -> Result<(), String> {
    let policy = policy.map(Policy::try_from).transpose()?;
    let replace_key = replace_key.unwrap_or(false);
    log::info!("everydesk: pair ack #{conn_id} accepted={accepted} policy={policy:?} replace_key={replace_key}");
    ui_cm_interface::pair_ack(conn_id, accepted, policy, replace_key);
    with_state(|s| s.authorize(conn_id));
    Ok(())
}

/// "Permitir sempre" (`remember`) / "Só desta vez" on the elevation card: Windows shows its
/// UAC prompt on this machine; `cm:elevation done` follows once the helper runs.
#[tauri::command]
pub fn cm_elevate(conn_id: i32, remember: bool) {
    log::info!("everydesk: elevate #{conn_id} (remember={remember})");
    if remember {
        Config::set_option(AUTO_ELEVATE_OPTION.to_owned(), "Y".to_owned());
    }
    with_state(|s| s.elevation.answered());
    ui_cm_interface::elevate_portable(conn_id);
}

/// "Agora não" (this episode only) / "não perguntar mais" (`remember_no`: never ask until
/// changed in the settings).
#[tauri::command]
pub fn cm_elevation_dismiss(remember_no: bool) {
    log::info!("everydesk: elevation dismissed (remember_no={remember_no})");
    if remember_no {
        Config::set_option(AUTO_ELEVATE_OPTION.to_owned(), "N".to_owned());
    }
    with_state(|s| s.elevation.answered());
}

/// The remembered answer: "Y" (elevate without asking), "N" (never ask) or "" (ask).
#[tauri::command]
pub fn cm_elevation_setting() -> String {
    normalize_setting(&Config::get_option(AUTO_ELEVATE_OPTION)).to_owned()
}

/// Changes the remembered answer (for the settings toggle); "" goes back to asking.
#[tauri::command]
pub fn cm_elevation_set_setting(value: String) -> Result<(), String> {
    if normalize_setting(&value) != value {
        return Err(format!("invalid elevation setting {value:?}"));
    }
    Config::set_option(AUTO_ELEVATE_OPTION.to_owned(), value);
    Ok(())
}

/// The connection whose elevation card is up and unanswered, for the UI to hydrate on mount.
#[tauri::command]
pub fn cm_elevation_pending() -> Option<i32> {
    with_state(|s| s.elevation.pending())
}

#[tauri::command]
pub fn cm_close(conn_id: i32) {
    log::info!("everydesk: close #{conn_id}");
    ui_cm_interface::close(conn_id);
}

/// Answers the controller `conn_id` in chat. Kept for `cm_pending` as `mine`.
#[tauri::command]
pub fn cm_send_chat(conn_id: i32, text: String) -> Result<(), String> {
    let text = crate::commands::chat_text(&text)?;
    if !with_state(|s| s.connected.contains_key(&conn_id)) {
        return Err("Essa conexão já terminou".to_owned());
    }
    ui_cm_interface::send_chat(conn_id, text.clone());
    with_state(|s| {
        s.chat(CmChat {
            conn_id,
            text,
            at: crate::session::unix_ms(),
            mine: true,
        })
    });
    Ok(())
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
    fn elevation_decision_table() {
        use Decision::*;
        let cases = [
            ("Y", true, true, Auto),
            ("N", true, true, Skip),
            ("", true, true, Ask),
            ("garbage", true, true, Ask),
            ("Y", true, false, Skip),
            ("", true, false, Skip),
            ("Y", false, true, Skip),
            ("", false, true, Skip),
        ];
        for (rem, show, can, want) in cases {
            assert_eq!(elevation_decision(rem, show, can), want, "{rem:?} show={show} can={can}");
        }
        assert_eq!(normalize_setting("Y"), "Y");
        assert_eq!(normalize_setting("N"), "N");
        assert_eq!(normalize_setting("yes"), "");
    }

    #[test]
    fn elevation_happens_once_per_episode() {
        let t0 = Instant::now();
        let mut s = CmState::default();
        let ev = |conn_id, state| Some(ElevationEvent { conn_id, state });
        // Before any authorized session: nothing latches.
        s.apply(&event_from(1, false, "a", "a", false), t0);
        assert_eq!(s.elevation_step(true, Decision::Ask), None);
        s.authorize(1);
        assert_eq!(s.elevation_step(true, Decision::Ask), ev(1, ElevationState::Ask));
        assert_eq!(s.elevation.pending(), Some(1));
        // Repeated ticks while up: silent.
        assert_eq!(s.elevation_step(true, Decision::Ask), None);
        // Answered ("Agora não"): stays quiet for the episode.
        s.elevation.answered();
        assert_eq!(s.elevation.pending(), None);
        assert_eq!(s.elevation_step(true, Decision::Ask), None);
        // Helper runs: done once, then a new episode may ask again.
        assert_eq!(s.elevation_step(false, Decision::Skip), ev(1, ElevationState::Done));
        assert_eq!(s.elevation_step(false, Decision::Skip), None);
        assert_eq!(s.elevation_step(true, Decision::Auto), ev(1, ElevationState::Auto));
        assert_eq!(s.elevation_step(true, Decision::Auto), None);
        // The session goes: the episode ends with it; the newest session is the target.
        s.apply(&event_from(2, true, "b", "b", false), t0);
        s.apply(&event_from(3, true, "c", "c", false), t0);
        s.remove(1);
        assert_eq!(s.elevation_step(true, Decision::Ask), ev(3, ElevationState::Ask));
        s.remove(3);
        assert_eq!(s.elevation.pending(), None);
        // "não perguntar mais": latched without an event, so no card on every tick.
        assert_eq!(s.elevation_step(true, Decision::Skip), None);
        assert_eq!(s.elevation.latched, Some(2));
        assert_eq!(s.elevation_step(false, Decision::Skip), ev(2, ElevationState::Done));
    }

    #[test]
    fn elevation_event_serializes_for_the_ui() {
        let v = serde_json::to_value(ElevationEvent {
            conn_id: 5,
            state: ElevationState::Ask,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"conn_id": 5, "state": "ask"}));
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
        let id = Identity {
            conn_id: 1,
            state: IdentityState::Unverified,
            known: true,
        };
        s.identity(id.clone());
        s.identity(Identity { conn_id: 9, ..id.clone() });
        let snap = s.snapshot(t0);
        assert_eq!(snap.identities, vec![id]);
        assert_eq!(
            serde_json::to_value(&snap).unwrap()["identities"][0],
            serde_json::json!({"conn_id": 1, "state": "unverified", "known": true})
        );
        s.authorize(1);
        s.answered(2);
        let snap = s.snapshot(t0);
        assert!(snap.offers.is_empty() && snap.reconfirms.is_empty() && snap.identities.is_empty());
        s.ring(Reconfirm { conn_id: 1, ring_secs: 60 }, t0);
        s.remove(1);
        assert_eq!(s.snapshot(t0), CmSnapshot::default());
    }

    #[test]
    fn chats_are_kept_per_connection_and_capped() {
        let mut s = CmState::default();
        let m = |conn_id, i: u64, mine| CmChat {
            conn_id,
            text: format!("m{i}"),
            at: i,
            mine,
        };
        for i in 0..(CHAT_KEEP as u64 + 5) {
            s.chat(m(1, i, i % 2 == 0));
        }
        s.chat(m(2, 0, false));
        let snap = s.snapshot(Instant::now());
        assert_eq!(snap.chats.len(), CHAT_KEEP + 1);
        assert_eq!(snap.chats[0].text, "m5");
        assert_eq!(snap.chats[CHAT_KEEP - 1].text, format!("m{}", CHAT_KEEP + 4));
        s.remove(1);
        let snap = s.snapshot(Instant::now());
        assert_eq!(snap.chats.len(), 1);
        assert_eq!(snap.chats[0].conn_id, 2);
    }
}
