//! Contacts: the union of the host's trust book (`pairings::book`, `pairings.json`: peers
//! that may connect here, with the owner's policy) and the controller's contacts book
//! (`pairings::contacts_book`, `contacts.json`: hosts that remembered us), plus their presence via a signed `OnlineRequest` to hbbs
//! (`librustdesk::everydesk_auth::query_online_signed`).
//!
//! Commands:
//! - `contacts_list() -> [{peer_id, name, policy: PolicyView | null, online}]` (sorted by
//!   name, then id; `online` is the last known presence; `policy` null = a contact we only
//!   control, not editable),
//! - `contacts_set_policy(peer_id, policy)` (`Err` when the id is not paired),
//! - `contacts_remove(peer_id)` (host side: engine `ui_cm_interface::revoke`, which forgets
//!   the pairing and ends any live session of that peer here, telling it it was revoked;
//!   controller side: dropped from the contacts book),
//! - `contacts_presence() -> [[peer_id, online]]`.
//!
//! `policy` is a `PolicyView`: `{"kind":"AskAlways"}`, `{"kind":"StayConnected"}` or
//! `{"kind":"Reconfirm","every_min":20}` (the engine `Policy` is externally tagged, which
//! is awkward in TS).
//!
//! Only a successful reply changes what the app shows: a failed query (timeout, network,
//! hbbs' rate limit) keeps the last known state. Queries are at least 6 s apart (hbbs
//! allows one per 5 s per id); a request inside that window is answered from the last
//! known state and the refresher is woken to query as soon as the window opens.

use hbb_common::{log, tokio};
use librustdesk::everydesk::pairings::{book, contacts_book, Book, Pairing, Policy};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

/// hbbs answers at most 200 peers per request.
const MAX_IDS: usize = 200;
/// hbbs answers one request per 5 s per id; one second of margin.
const MIN_QUERY_INTERVAL: Duration = Duration::from_secs(6);
const REFRESH_EVERY: Duration = Duration::from_secs(15);

/// Trust policy as the UI sees it (internally tagged by `kind`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum PolicyView {
    AskAlways,
    StayConnected,
    Reconfirm { every_min: u32 },
}

impl From<Policy> for PolicyView {
    fn from(p: Policy) -> Self {
        match p {
            Policy::AskAlways => PolicyView::AskAlways,
            Policy::StayConnected => PolicyView::StayConnected,
            Policy::Reconfirm { every_min } => PolicyView::Reconfirm { every_min },
        }
    }
}

/// Allowed reconfirmation periods (minutes): any value from 1 min to 24 h. The UI offers the
/// presets 20/45/80 plus "Outro…" (spec: "ou livre").
pub const RECONFIRM_PERIOD_RANGE_MIN: std::ops::RangeInclusive<u32> = 1..=1440;

/// Rejects a Reconfirm period outside `RECONFIRM_PERIOD_RANGE_MIN`.
impl TryFrom<PolicyView> for Policy {
    type Error = String;
    fn try_from(p: PolicyView) -> Result<Self, String> {
        Ok(match p {
            PolicyView::AskAlways => Policy::AskAlways,
            PolicyView::StayConnected => Policy::StayConnected,
            PolicyView::Reconfirm { every_min } if RECONFIRM_PERIOD_RANGE_MIN.contains(&every_min) => {
                Policy::Reconfirm { every_min }
            }
            PolicyView::Reconfirm { every_min } => {
                return Err(format!("período de reconfirmação inválido: {every_min} min"))
            }
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ContactView {
    pub peer_id: String,
    pub name: String,
    /// The policy this machine applies when `peer_id` connects here (host-side pairing,
    /// editable); `None` for a contact we only control (shown as "—").
    pub policy: Option<PolicyView>,
    pub online: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
struct Presence {
    peer_id: String,
    online: bool,
}

#[derive(Default)]
struct State {
    /// Last query sent (successful or not: hbbs counts both).
    last_query: Option<Instant>,
    /// Last successful answer per id.
    known: HashMap<String, bool>,
}

/// Held for the whole query, so the command and the refresher never overlap.
static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Wakes the refresher early (a request was debounced).
static WAKE: OnceLock<Mutex<Sender<()>>> = OnceLock::new();

// ---- pairing books ---------------------------------------------------------------------

fn entries(book: &std::sync::Mutex<Book>) -> Vec<Pairing> {
    let book = book.lock().unwrap_or_else(|e| e.into_inner());
    book.list().into_iter().cloned().collect()
}

/// Host-side pairings (policy editable) and controller-side contacts (policy `None`).
fn pairings() -> Vec<(Pairing, bool)> {
    let mut out: Vec<(Pairing, bool)> =
        entries(book()).into_iter().map(|p| (p, true)).collect();
    out.extend(entries(contacts_book()).into_iter().map(|p| (p, false)));
    out
}

/// Ids to ask presence for: deduplicated, at most `MAX_IDS`.
fn cap_ids(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids.dedup();
    if ids.len() > MAX_IDS {
        log::warn!("everydesk: {} contact ids, querying the first {MAX_IDS}", ids.len());
        ids.truncate(MAX_IDS);
    }
    ids
}

fn paired_ids() -> Vec<String> {
    cap_ids(pairings().into_iter().map(|(p, _)| p.peer_id).collect())
}

/// One row per id: a peer in both books shows the host-side entry (its policy is ours).
/// Input order matters: host-side entries first.
fn contact_views(list: Vec<(Pairing, bool)>, known: &HashMap<String, bool>) -> Vec<ContactView> {
    let mut out: Vec<ContactView> = Vec::new();
    for (p, host_side) in list {
        if out.iter().any(|c| c.peer_id == p.peer_id) {
            continue;
        }
        out.push(ContactView {
            online: known.get(&p.peer_id).copied().unwrap_or(false),
            name: p.name,
            policy: host_side.then(|| p.policy.into()),
            peer_id: p.peer_id,
        });
    }
    out.sort_by(|a, b| {
        (a.name.to_lowercase(), &a.peer_id).cmp(&(b.name.to_lowercase(), &b.peer_id))
    });
    out
}

/// Both books' contacts with their last known presence.
#[tauri::command]
pub async fn contacts_list() -> Vec<ContactView> {
    // Off the main thread: STATE is held for a whole presence query (up to ~3 s).
    tauri::async_runtime::spawn_blocking(|| {
        let list = pairings();
        let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let empty = HashMap::new();
        let known = guard.as_ref().map_or(&empty, |st| &st.known);
        contact_views(list, known)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn contacts_set_policy(peer_id: String, policy: PolicyView) -> Result<(), String> {
    let engine = Policy::try_from(policy)?;
    let mut book = book().lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut p) = book.get(&peer_id).cloned() else {
        return Err(format!("contato {peer_id} não encontrado"));
    };
    p.policy = engine;
    book.upsert(p);
    book.save().map_err(|e| {
        log::error!("everydesk: cannot save policy for {peer_id}: {e}");
        format!("não foi possível salvar: {e}")
    })?;
    log::info!("everydesk: contact {peer_id} policy -> {policy:?}");
    Ok(())
}

/// Forgets `peer_id` in both books. Host side goes through the engine revoke (removes it
/// from the book, saves, and ends a live session of that peer here with a revoke notice);
/// controller side is only dropped locally (the host keeps its own decision).
#[tauri::command]
pub fn contacts_remove(peer_id: String) -> Result<(), String> {
    log::info!("everydesk: contact {peer_id} removed");
    let host_side = book().lock().unwrap_or_else(|e| e.into_inner()).get(&peer_id).is_some();
    if host_side {
        librustdesk::ui_cm_interface::revoke(&peer_id);
    }
    let mut contacts = contacts_book().lock().unwrap_or_else(|e| e.into_inner());
    if contacts.get(&peer_id).is_some() {
        contacts.remove(&peer_id);
        contacts.save().map_err(|e| {
            log::error!("everydesk: cannot save contacts after removing {peer_id}: {e}");
            format!("não foi possível salvar: {e}")
        })?;
    }
    Ok(())
}

/// Last known state of `ids`, in list order; an id never answered yet is offline.
fn snapshot(known: &HashMap<String, bool>, ids: &[String]) -> Vec<(String, bool)> {
    ids.iter()
        .map(|id| (id.clone(), known.get(id).copied().unwrap_or(false)))
        .collect()
}

/// A successful answer replaces the state; ids no longer listed are forgotten.
fn record_success(known: &mut HashMap<String, bool>, ids: &[String], fresh: &[(String, bool)]) {
    known.retain(|id, _| ids.contains(id));
    for (id, online) in fresh {
        known.insert(id.clone(), *online);
    }
}

/// How long to wait before the next query may go out.
fn wait_before_query(since_last: Option<Duration>) -> Duration {
    since_last.map_or(Duration::ZERO, |d| MIN_QUERY_INTERVAL.saturating_sub(d))
}

enum Outcome {
    /// Fresh answer from hbbs.
    Fresh(Vec<(String, bool)>),
    /// Debounced: last known state; a query is due in the returned time.
    Debounced(Vec<(String, bool)>, Duration),
    /// The query failed; the state is unchanged.
    Failed,
}

/// Blocking: queries hbbs for `ids` unless a query went out less than 6 s ago.
fn refresh(ids: &[String]) -> Outcome {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(State::default);
    let wait = wait_before_query(st.last_query.map(|t| t.elapsed()));
    if !wait.is_zero() {
        return Outcome::Debounced(snapshot(&st.known, ids), wait);
    }
    st.last_query = Some(Instant::now());
    let res = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.into())
        .and_then(|rt| rt.block_on(librustdesk::everydesk_auth::query_online_signed(ids)));
    match res {
        Ok(fresh) => {
            record_success(&mut st.known, ids, &fresh);
            Outcome::Fresh(fresh)
        }
        Err(e) => {
            log::debug!("everydesk: presence query failed, keeping last state: {e}");
            Outcome::Failed
        }
    }
}

fn last_known(ids: &[String]) -> Vec<(String, bool)> {
    let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map_or_else(|| snapshot(&HashMap::new(), ids), |st| snapshot(&st.known, ids))
}

fn wake_refresher() {
    if let Some(tx) = WAKE.get() {
        tx.lock().unwrap_or_else(|e| e.into_inner()).send(()).ok();
    }
}

fn to_payload(states: &[(String, bool)]) -> Vec<Presence> {
    states
        .iter()
        .map(|(peer_id, online)| Presence {
            peer_id: peer_id.clone(),
            online: *online,
        })
        .collect()
}

/// Presence of the paired ids: a fresh answer when one may be asked, else the last
/// known state (and the refresher queries as soon as hbbs allows).
#[tauri::command]
pub async fn contacts_presence() -> Vec<(String, bool)> {
    tauri::async_runtime::spawn_blocking(|| {
        let ids = paired_ids();
        if ids.is_empty() {
            return vec![];
        }
        match refresh(&ids) {
            Outcome::Fresh(s) => s,
            Outcome::Debounced(s, _) => {
                wake_refresher();
                s
            }
            Outcome::Failed => last_known(&ids),
        }
    })
    .await
    .unwrap_or_default()
}

/// Emits `contacts:presence` `[{peer_id, online}]` after each successful query, every
/// 15 s (sooner when woken, never closer than the debounce allows). Nothing is emitted
/// on failure or when there are no contacts.
pub fn start(app: AppHandle) {
    let (tx, rx) = channel();
    if WAKE.set(Mutex::new(tx)).is_err() {
        return;
    }
    std::thread::spawn(move || refresher(app, rx));
}

fn refresher(app: AppHandle, rx: Receiver<()>) {
    let mut next = Duration::ZERO;
    loop {
        // Woken or timed out: either way it is time to try.
        let _ = rx.recv_timeout(next);
        while rx.try_recv().is_ok() {}
        next = REFRESH_EVERY;
        let ids = paired_ids();
        if ids.is_empty() {
            continue;
        }
        match refresh(&ids) {
            Outcome::Fresh(s) => {
                if let Err(e) = app.emit("contacts:presence", to_payload(&s)) {
                    log::warn!("everydesk: emit contacts:presence: {e}");
                }
            }
            // Too soon (the command just queried, or the list changed): retry when allowed.
            Outcome::Debounced(_, wait) => next = wait,
            Outcome::Failed => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn failure_keeps_last_known_state() {
        let mut known = HashMap::new();
        record_success(&mut known, &ids(&["a", "b"]), &[("a".into(), true), ("b".into(), false)]);
        // a failed query records nothing: the snapshot is still the last success
        assert_eq!(
            snapshot(&known, &ids(&["a", "b"])),
            [("a".to_owned(), true), ("b".to_owned(), false)]
        );
        // a newly added id with no answer yet shows offline; order follows the list
        assert_eq!(
            snapshot(&known, &ids(&["c", "a"])),
            [("c".to_owned(), false), ("a".to_owned(), true)]
        );
        // a success replaces the state and forgets ids no longer listed
        record_success(&mut known, &ids(&["a"]), &[("a".into(), false)]);
        assert_eq!(known.len(), 1);
        assert_eq!(snapshot(&known, &ids(&["a"])), [("a".to_owned(), false)]);
    }

    #[test]
    fn debounce_waits_for_5s_since_last_query() {
        assert_eq!(wait_before_query(None), Duration::ZERO);
        assert_eq!(wait_before_query(Some(Duration::from_secs(1))), Duration::from_secs(5));
        assert_eq!(wait_before_query(Some(Duration::from_secs(6))), Duration::ZERO);
        assert_eq!(wait_before_query(Some(Duration::from_secs(60))), Duration::ZERO);
    }

    #[test]
    fn payload_shape() {
        let p = to_payload(&[("a".into(), true), ("b".into(), false)]);
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                {"peer_id": "a", "online": true},
                {"peer_id": "b", "online": false}
            ])
        );
    }

    #[test]
    fn policy_view_maps_every_variant() {
        let all = [
            (Policy::AskAlways, PolicyView::AskAlways, r#"{"kind":"AskAlways"}"#),
            (Policy::StayConnected, PolicyView::StayConnected, r#"{"kind":"StayConnected"}"#),
            (
                Policy::Reconfirm { every_min: 20 },
                PolicyView::Reconfirm { every_min: 20 },
                r#"{"kind":"Reconfirm","every_min":20}"#,
            ),
        ];
        for (engine, view, json) in all {
            assert_eq!(PolicyView::from(engine), view);
            assert_eq!(Policy::try_from(view), Ok(engine));
            assert_eq!(serde_json::to_string(&view).unwrap(), json);
            assert_eq!(serde_json::from_str::<PolicyView>(json).unwrap(), view);
        }
        for m in [1, 20, 30, 45, 80, 81, 1440] {
            assert_eq!(
                Policy::try_from(PolicyView::Reconfirm { every_min: m }),
                Ok(Policy::Reconfirm { every_min: m })
            );
        }
        for m in [0, 1441, u32::MAX] {
            assert!(Policy::try_from(PolicyView::Reconfirm { every_min: m }).is_err());
        }
        assert!(serde_json::from_str::<PolicyView>(r#"{"kind":"Reconfirm"}"#).is_err());
        assert!(serde_json::from_str::<PolicyView>(r#""AskAlways""#).is_err());
    }

    #[test]
    fn contact_view_roundtrip() {
        let c = ContactView {
            peer_id: "abc-def-ghi".into(),
            name: "Notebook".into(),
            policy: Some(PolicyView::Reconfirm { every_min: 45 }),
            online: true,
        };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "peer_id": "abc-def-ghi",
                "name": "Notebook",
                "policy": {"kind": "Reconfirm", "every_min": 45},
                "online": true
            })
        );
        assert_eq!(serde_json::from_value::<ContactView>(json).unwrap(), c);
        let only_controlled = ContactView { policy: None, online: false, ..c };
        let json = serde_json::to_value(&only_controlled).unwrap();
        assert_eq!(json["policy"], serde_json::Value::Null);
        assert_eq!(serde_json::from_value::<ContactView>(json).unwrap(), only_controlled);
    }

    fn pairing(id: &str, name: &str, policy: Policy) -> Pairing {
        Pairing {
            peer_id: id.into(),
            pk: [7; 32],
            name: name.into(),
            avatar: None,
            policy,
            created_at: 1,
            ask_next: false,
        }
    }

    #[test]
    fn contact_views_sorted_with_presence() {
        let mut known = HashMap::new();
        known.insert("b".to_owned(), true);
        let v = contact_views(
            vec![
                (pairing("b", "zeca", Policy::StayConnected), true),
                (pairing("a", "Ana", Policy::AskAlways), false),
                (pairing("b", "zeca (contato)", Policy::AskAlways), false),
            ],
            &known,
        );
        assert_eq!(v.iter().map(|c| c.peer_id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(!v[0].online && v[1].online);
        // controller-only contact: no policy; in both books: the host-side entry wins
        assert_eq!(v[0].policy, None);
        assert_eq!(v[1].policy, Some(PolicyView::StayConnected));
        assert_eq!(v[1].name, "zeca");
    }

    #[test]
    fn cap_ids_dedups_and_caps() {
        assert_eq!(cap_ids(ids(&["b", "a", "b"])), ["a", "b"]);
        let many: Vec<String> = (0..250).map(|i| format!("id{i:03}")).collect();
        assert_eq!(cap_ids(many).len(), MAX_IDS);
    }
}
