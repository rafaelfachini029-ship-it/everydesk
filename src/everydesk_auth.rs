//! EveryDesk requester envelope for punch/relay requests to hbbs.
//!
//! `licence_key = "<server_key>;v1;<my_id>;<unix_ts>;<base64 sig>"`, where `sig` is the
//! ed25519 detached signature of `ed-punch-v1|<my_id>|<target_id>|<ts>` made with this
//! machine's key pair (the pk it registers with hbbs). The server side lives in
//! everydesk-server `src/requester_auth.rs`; the format is in its docs/protocolo-everydesk.md.

use hbb_common::{
    base64::{engine::general_purpose::STANDARD, Engine as _},
    config::Config,
    log,
    sodiumoxide::crypto::sign,
};
use std::sync::Mutex;

const SIGN_CONTEXT: &str = "ed-punch-v1";

/// Last timestamp handed out. hbbs rejects a repeated (my_id, ts) for 60 s, so two
/// requests from this process in the same second get ts and ts + 1.
static LAST_TS: Mutex<i64> = Mutex::new(0);

fn next_ts() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let mut last = LAST_TS.lock().unwrap();
    *last = now.max(*last + 1);
    *last
}

/// The `licence_key` for a punch/relay request to `target_id`.
pub fn punch_envelope(server_key: &str, target_id: &str) -> String {
    let my_id = Config::get_id();
    let (sk, _) = Config::get_key_pair();
    envelope_with(server_key, &my_id, target_id, next_ts(), &sk)
}

fn envelope_with(server_key: &str, my_id: &str, target_id: &str, ts: i64, sk: &[u8]) -> String {
    let msg = format!("{SIGN_CONTEXT}|{my_id}|{target_id}|{ts}");
    let Some(sig) = sign_detached(sk, msg.as_bytes()) else {
        // hbbs will deny this and log it; there is nothing better to send.
        log::error!("everydesk auth: invalid local key pair, sending no envelope");
        return server_key.to_owned();
    };
    format!("{server_key};v1;{my_id};{ts};{}", STANDARD.encode(sig))
}

/// Ed25519 detached signature of `msg` with the raw secret key `sk`; `None` if `sk` is malformed.
pub(crate) fn sign_detached(sk: &[u8], msg: &[u8]) -> Option<[u8; sign::SIGNATUREBYTES]> {
    let sk = sign::SecretKey::from_slice(sk)?;
    Some(sign::sign_detached(msg, &sk).to_bytes())
}

/// Checks an ed25519 detached signature; false for any malformed input.
pub(crate) fn verify_detached(pk: &[u8], sig: &[u8], msg: &[u8]) -> bool {
    let (Some(pk), Ok(sig)) = (
        sign::PublicKey::from_slice(pk),
        sign::Signature::from_bytes(sig),
    ) else {
        return false;
    };
    sign::verify_detached(&sig, msg, &pk)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirror of the server's `parse_envelope` + signature check.
    fn server_accepts(licence_key: &str, target_id: &str, pk: &sign::PublicKey) -> bool {
        let parts: Vec<&str> = licence_key.split(';').collect();
        if parts.len() != 5 || parts[1] != "v1" || parts[2].is_empty() {
            return false;
        }
        let Ok(ts) = parts[3].parse::<i64>() else {
            return false;
        };
        let Ok(sig) = STANDARD.decode(parts[4]) else {
            return false;
        };
        let Ok(sig) = sign::Signature::from_bytes(&sig) else {
            return false;
        };
        let msg = format!("ed-punch-v1|{}|{}|{}", parts[2], target_id, ts);
        sign::verify_detached(&sig, msg.as_bytes(), pk)
    }

    #[test]
    fn punch_envelope_format() {
        let (pk, sk) = sign::gen_keypair();
        let env = envelope_with("SRVKEY=", "123456789", "987654321", 1_800_000_000, &sk.0);
        let parts: Vec<&str> = env.split(';').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(&parts[..4], &["SRVKEY=", "v1", "123456789", "1800000000"]);
        assert_eq!(
            STANDARD.decode(parts[4]).unwrap().len(),
            sign::SIGNATUREBYTES
        );
        assert!(server_accepts(&env, "987654321", &pk));
        assert!(!server_accepts(&env, "111111111", &pk));
    }

    #[test]
    fn timestamps_strictly_increase() {
        let a = next_ts();
        let b = next_ts();
        assert!(b > a);
    }
}
