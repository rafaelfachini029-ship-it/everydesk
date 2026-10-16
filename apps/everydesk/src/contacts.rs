//! Contact presence: which paired ids are online, via a signed `OnlineRequest` to hbbs
//! (`librustdesk::everydesk_auth::query_online_signed`).
//!
//! TEMPORARY SEAM (Task 5 replaces it): until the pairing book lands, the ids come from
//! `contacts-presence-ids.json` in the config dir (a JSON array of id strings); no file,
//! or an unreadable one, means no contacts. Task 5 should swap `paired_ids` for the
//! pairing book and delete the file read.
//!
//! Only a successful reply changes what the app shows: a failed query (timeout, network,
//! hbbs' rate limit) keeps the last known state. Queries are at least 6 s apart (hbbs
//! allows one per 5 s per id); a request inside that window is answered from the last
//! known state and the refresher is woken to query as soon as the window opens.

use hbb_common::{config::Config, log, tokio};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const IDS_FILE: &str = "contacts-presence-ids.json";
/// hbbs answers at most 200 peers per request.
const MAX_IDS: usize = 200;
/// hbbs answers one request per 5 s per id; one second of margin.
const MIN_QUERY_INTERVAL: Duration = Duration::from_secs(6);
const REFRESH_EVERY: Duration = Duration::from_secs(15);

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

/// Ids from the JSON array, deduplicated, empty strings dropped, at most `MAX_IDS`.
fn parse_ids(json: &str) -> Vec<String> {
    let Ok(list) = serde_json::from_str::<Vec<String>>(json) else {
        log::warn!("everydesk: {IDS_FILE} is not a JSON array of strings");
        return vec![];
    };
    let mut out: Vec<String> = Vec::new();
    for id in list.into_iter().map(|s| s.trim().to_owned()) {
        if !id.is_empty() && !out.contains(&id) {
            out.push(id);
        }
    }
    if out.len() > MAX_IDS {
        log::warn!("everydesk: {} contact ids, querying the first {MAX_IDS}", out.len());
        out.truncate(MAX_IDS);
    }
    out
}

fn paired_ids() -> Vec<String> {
    match std::fs::read_to_string(Config::path(IDS_FILE)) {
        Ok(s) => parse_ids(&s),
        Err(_) => vec![],
    }
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

    #[test]
    fn ids_file_parsing() {
        assert_eq!(parse_ids(r#"["a-b-c", " d ", "", "a-b-c"]"#), ["a-b-c", "d"]);
        assert!(parse_ids("not json").is_empty());
        assert!(parse_ids(r#"{"ids": ["x"]}"#).is_empty());
        let many: Vec<String> = (0..250).map(|i| format!("id{i}")).collect();
        let parsed = parse_ids(&serde_json::to_string(&many).unwrap());
        assert_eq!(parsed.len(), MAX_IDS);
        assert_eq!(parsed[0], "id0");
    }

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
}
