//! EveryDesk server pinned at runtime through the engine's own config options,
//! so the `libs/hbb_common` submodule (upstream defaults) stays untouched.

use hbb_common::config::Config;

pub const EVERYDESK_SERVER: &str = "46.202.151.211";
pub const EVERYDESK_PUBKEY: &str = "T2zz2EfcoTFA7veldOi+OEqwLPwhMYYwDyS4XvuzJFg=";

const SERVER_PINS: [(&str, &str); 2] = [
    ("custom-rendezvous-server", EVERYDESK_SERVER),
    ("key", EVERYDESK_PUBKEY),
];

/// What the host must always run with: server pin + click approval.
static HOST_PINS: [(&str, &str); 3] = [SERVER_PINS[0], SERVER_PINS[1], crate::cm::APPROVE_CLICK];

/// Must run before the host server starts: `Config::get_rendezvous_server()`
/// reads `custom-rendezvous-server` and `common::get_key()` reads `key`.
///
/// `main` runs this in every process: the service's `--server` pins its own config at startup.
/// The app window also pins the service over IPC (`ensure_host_config_on_service`), so the
/// pin is written in BOTH configs: this user's (controller side) and the service's (host).
pub fn apply_server_config() {
    for (k, v) in SERVER_PINS {
        if Config::get_option(k) != v {
            Config::set_option(k.to_owned(), v.to_owned());
        }
    }
}

/// Service mode (app window only): the same pins on the service's config, over IPC, retried in
/// the background until its `--server` answers. No-op in portable mode (local is the host).
pub fn ensure_host_config_on_service() {
    crate::settings::ensure_on_service(&HOST_PINS);
}
