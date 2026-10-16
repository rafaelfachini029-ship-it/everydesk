//! EveryDesk server pinned at runtime through the engine's own config options,
//! so the `libs/hbb_common` submodule (upstream defaults) stays untouched.

use hbb_common::config::Config;

pub const EVERYDESK_SERVER: &str = "46.202.151.211";
pub const EVERYDESK_PUBKEY: &str = "T2zz2EfcoTFA7veldOi+OEqwLPwhMYYwDyS4XvuzJFg=";

/// Must run before the host server starts: `Config::get_rendezvous_server()`
/// reads `custom-rendezvous-server` and `common::get_key()` reads `key`.
pub fn apply_server_config() {
    for (k, v) in [
        ("custom-rendezvous-server", EVERYDESK_SERVER),
        ("key", EVERYDESK_PUBKEY),
    ] {
        if Config::get_option(k) != v {
            Config::set_option(k.to_owned(), v.to_owned());
        }
    }
}
