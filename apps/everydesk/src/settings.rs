//! Host-side options (permissions, approve-mode, elevation answer, server pin) must reach the
//! process that hosts. Portable: that is this process, so `Config` is the truth. Service mode
//! (`service::is_service_mode`): the SYSTEM service's `--server` hosts with its OWN config
//! (`C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\EveryDesk\config\EveryDesk.toml` on
//! Windows), and `Config::set_option` here only writes this user's copy, which the host never
//! reads. So in service mode writes go over the main IPC (`Data::Options`, what
//! `ipc::set_options` sends) and reads come from `ui_interface::get_option`, the cache that
//! `ui_interface::start_option_status_sync` refreshes from the host every second.
//!
//! Engine modes (`--server`, `--service`, ...) run `lib.rs::main`'s startup fixes on their own
//! config with plain `Config` writes; only the app window routes through here.

use hbb_common::config::Config;
use librustdesk::ipc::{self, Data};
use std::collections::HashMap;
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Over IPC to the service's `--server` (plus a local copy).
    Ipc,
    /// This process's own `Config`.
    Local,
}

/// Pure routing seam.
pub fn route(service_mode: bool) -> Route {
    if service_mode {
        Route::Ipc
    } else {
        Route::Local
    }
}

fn current_route() -> Route {
    route(crate::service::is_service_mode())
}

const IPC_TIMEOUT: Duration = Duration::from_secs(3);
/// The status sync refreshes its cache every second: a value just written wins over it
/// for a little longer than that, so a read right after a toggle sees the toggle.
const OVERLAY_TTL: Duration = Duration::from_millis(2500);

static RECENT: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);

fn remember(key: &str, value: &str) {
    let mut g = RECENT.lock().unwrap();
    g.get_or_insert_with(HashMap::new)
        .insert(key.to_owned(), (value.to_owned(), Instant::now()));
}

fn recent(key: &str) -> Option<String> {
    let g = RECENT.lock().unwrap();
    let (v, at) = g.as_ref()?.get(key)?;
    (at.elapsed() < OVERLAY_TTL).then(|| v.clone())
}

/// Same map edit as `ipc::set_option`: "" removes the key (the engine's default).
pub fn apply(map: &mut HashMap<String, String>, key: &str, value: &str) {
    if value.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_owned(), value.to_owned());
    }
}

/// Read-modify-write of the host's map on one connection. Unlike `ipc::set_option` it fails
/// instead of falling back to this user's map (so the user's whole map is never pushed onto the
/// service) and it reports whether the host acknowledged.
async fn ipc_set(pairs: Vec<(String, String)>) -> hbb_common::ResultType<()> {
    let mut c = ipc::connect(1000, "").await?;
    c.send(&Data::Options(None)).await?;
    let mut map = match c.next_timeout(1000).await? {
        Some(Data::Options(Some(m))) => m,
        _ => hbb_common::bail!("resposta inesperada do serviço"),
    };
    let before = map.clone();
    for (k, v) in &pairs {
        apply(&mut map, k, v);
    }
    if map == before {
        return Ok(());
    }
    c.send(&Data::Options(Some(map))).await?;
    match c.next_timeout(1000).await? {
        Some(Data::Options(None)) => Ok(()),
        _ => hbb_common::bail!("o serviço não confirmou"),
    }
}

/// Runs the IPC write on its own thread and runtime (callers may already sit inside a tokio
/// runtime, e.g. the `_cm` thread) and gives up after `IPC_TIMEOUT`.
fn ipc_set_blocking(pairs: Vec<(String, String)>) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("everydesk-settings".into())
        .spawn(move || {
            let res = hbb_common::tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
                .and_then(|rt| rt.block_on(ipc_set(pairs)).map_err(|e| e.to_string()));
            let _ = tx.send(res);
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(IPC_TIMEOUT)
        .map_err(|_| "tempo esgotado".to_owned())?
}

const SERVICE_UNREACHABLE: &str =
    "O serviço do EveryDesk não respondeu; a mudança ficou salva só neste usuário e vale quando o serviço voltar a ler a configuração. Tente de novo.";

/// Writes a host-side option where the host reads it. Service mode: over IPC (and locally,
/// so an in-process fallback host sees the same value); on IPC failure the local write still
/// happens and the error says so.
pub fn set_option(key: &str, value: &str) -> Result<(), String> {
    Config::set_option(key.to_owned(), value.to_owned());
    match current_route() {
        Route::Local => Ok(()),
        Route::Ipc => match ipc_set_blocking(vec![(key.to_owned(), value.to_owned())]) {
            Ok(()) => {
                remember(key, value);
                Ok(())
            }
            Err(e) => {
                log::warn!("everydesk: option {key} not sent to the service: {e}");
                Err(SERVICE_UNREACHABLE.to_owned())
            }
        },
    }
}

/// Reads a host-side option from where the host reads it.
pub fn get_option(key: &str) -> String {
    match current_route() {
        Route::Local => Config::get_option(key),
        Route::Ipc => recent(key).unwrap_or_else(|| librustdesk::ui_interface::get_option(key)),
    }
}

/// The id the host registers with: in service mode the service's (the status sync reads it
/// every second), falling back to this user's copy (`start_server` syncs it at startup).
pub fn host_id() -> String {
    if current_route() == Route::Ipc {
        let id = librustdesk::ui_interface::get_connect_status().id;
        if !id.is_empty() {
            return id;
        }
    }
    Config::get_id()
}

/// Service mode only: makes sure the service's config holds `pairs` (server pin, click
/// approval), retrying in the background until its `--server` answers (~2 min). The service's
/// `--server` already applies the same fixes to its own config at its startup (`lib.rs::main`
/// runs for every mode); this covers a service whose config drifted since.
pub fn ensure_on_service(pairs: &'static [(&'static str, &'static str)]) {
    if current_route() != Route::Ipc {
        return;
    }
    std::thread::Builder::new()
        .name("everydesk-ensure".into())
        .spawn(move || {
            let pairs: Vec<(String, String)> =
                pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
            for attempt in 1..=60 {
                match ipc_set_blocking(pairs.clone()) {
                    Ok(()) => {
                        log::info!("everydesk: service config ensured (attempt {attempt})");
                        return;
                    }
                    Err(e) => log::info!("everydesk: service config not ensured yet: {e}"),
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            log::warn!("everydesk: gave up ensuring the service config");
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_writes_route_to_ipc_in_service_mode() {
        assert_eq!(route(true), Route::Ipc);
        assert_eq!(route(false), Route::Local);
        if !cfg!(windows) {
            // Off Windows there is no service mode: everything stays local.
            assert_eq!(current_route(), Route::Local);
        }
    }

    #[test]
    fn apply_matches_the_engine_map_edit() {
        let mut m = HashMap::new();
        apply(&mut m, "enable-audio", "N");
        assert_eq!(m.get("enable-audio").map(String::as_str), Some("N"));
        // On is stored as "" = key removed (`cm::option_value(true)`).
        apply(&mut m, "enable-audio", "");
        assert!(!m.contains_key("enable-audio"));
    }

    #[test]
    fn recent_write_overlays_the_cache_briefly() {
        remember("everydesk-test-key", "Y");
        assert_eq!(recent("everydesk-test-key").as_deref(), Some("Y"));
        assert_eq!(recent("everydesk-other-key"), None);
    }
}
