//! Contact presence: which paired ids are online, via a signed `OnlineRequest` to hbbs
//! (`librustdesk::everydesk_auth::query_online_signed`).
//!
//! TEMPORARY SEAM (Task 5 replaces it): until the pairing book lands, the ids come from
//! `contacts-presence-ids.json` in the config dir (a JSON array of id strings); no file,
//! or an unreadable one, means no contacts. Task 5 should swap `paired_ids` for the
//! pairing book and delete the file read.

use hbb_common::{config::Config, log, tokio};
use serde::Serialize;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const IDS_FILE: &str = "contacts-presence-ids.json";
/// hbbs answers at most 200 peers per request.
const MAX_IDS: usize = 200;
/// hbbs answers one request per 5 s per id; a newer answer is reused instead.
const MIN_QUERY_INTERVAL: Duration = Duration::from_secs(6);
const REFRESH_EVERY: Duration = Duration::from_secs(15);

#[derive(Serialize, Clone, Debug, PartialEq)]
struct Presence {
    peer_id: String,
    online: bool,
}

struct Last {
    at: Instant,
    ids: Vec<String>,
    states: Vec<(String, bool)>,
}

/// Held for the whole query, so the command and the refresher never race into hbbs'
/// rate limit.
static LAST: Mutex<Option<Last>> = Mutex::new(None);

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

/// Blocking: presence of the paired ids, reusing an answer younger than 6 s for the
/// same ids. A failed query (unreachable, denied) reports everyone offline.
fn presence_now() -> Vec<(String, bool)> {
    let ids = paired_ids();
    if ids.is_empty() {
        return vec![];
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(l) = last.as_ref() {
        if l.ids == ids && l.at.elapsed() < MIN_QUERY_INTERVAL {
            return l.states.clone();
        }
    }
    let res = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.into())
        .and_then(|rt| rt.block_on(librustdesk::everydesk_auth::query_online_signed(&ids)));
    let states = match res {
        Ok(s) => s,
        Err(e) => {
            log::debug!("everydesk: presence query failed: {e}");
            ids.iter().map(|id| (id.clone(), false)).collect()
        }
    };
    *last = Some(Last {
        at: Instant::now(),
        ids,
        states: states.clone(),
    });
    states
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

#[tauri::command]
pub async fn contacts_presence() -> Vec<(String, bool)> {
    tauri::async_runtime::spawn_blocking(presence_now)
        .await
        .unwrap_or_default()
}

/// Emits `contacts:presence` `[{peer_id, online}]` every 15 s.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        let payload = to_payload(&presence_now());
        if let Err(e) = app.emit("contacts:presence", payload) {
            log::warn!("everydesk: emit contacts:presence: {e}");
        }
        std::thread::sleep(REFRESH_EVERY);
    });
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
