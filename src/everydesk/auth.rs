//! Controller identity in `LoginRequest.everydesk_auth`.
//!
//! The controller signs, with its machine ed25519 key, the message
//! `ed-login-v1|<host_id>|<my_id>|<session_id>|<base64(host ephemeral box pk)>` and sends
//! `[pk (32) || sig (64)]`. The host's ephemeral box pk is fresh per connection and signed by
//! the host in its `SignedId`, so it acts as a nonce: a signature cannot be replayed into another
//! connection. Only a keyed (encrypted) connection has it on both sides.

use crate::everydesk_auth::{sign_detached, verify_detached};
use hbb_common::{
    base64::{engine::general_purpose::STANDARD, Engine as _},
    log,
    sodiumoxide::crypto::sign::{self, PUBLICKEYBYTES, SIGNATUREBYTES},
};
use std::sync::{Arc, Mutex};

const SIGN_CONTEXT: &str = "ed-login-v1";

fn login_message(host_id: &str, my_id: &str, session_id: u64, host_pk_b: &[u8]) -> String {
    format!(
        "{SIGN_CONTEXT}|{host_id}|{my_id}|{session_id}|{}",
        STANDARD.encode(host_pk_b)
    )
}

/// `[pk (32) || sig (64)]`, or empty when there is nothing to bind to (no host pk) or the
/// local key pair is malformed; the host then treats the controller as unauthenticated.
pub fn sign_login(
    sk: &[u8],
    host_id: &str,
    my_id: &str,
    session_id: u64,
    host_pk_b: &[u8],
) -> Vec<u8> {
    if host_pk_b.is_empty() || sk.len() != sign::SECRETKEYBYTES {
        return Vec::new();
    }
    let msg = login_message(host_id, my_id, session_id, host_pk_b);
    let Some(sig) = sign_detached(sk, msg.as_bytes()) else {
        log::error!("everydesk auth: invalid local key pair, login unsigned");
        return Vec::new();
    };
    // An ed25519 libsodium sk is seed || pk.
    let mut out = Vec::with_capacity(PUBLICKEYBYTES + SIGNATUREBYTES);
    out.extend_from_slice(&sk[PUBLICKEYBYTES..]);
    out.extend_from_slice(&sig);
    out
}

pub fn verify_login(
    bytes: &[u8],
    host_id: &str,
    my_id: &str,
    session_id: u64,
    host_pk_b: &[u8],
) -> Option<[u8; 32]> {
    if bytes.len() != PUBLICKEYBYTES + SIGNATUREBYTES || host_pk_b.is_empty() {
        return None;
    }
    let (pk, sig) = bytes.split_at(PUBLICKEYBYTES);
    let msg = login_message(host_id, my_id, session_id, host_pk_b);
    if !verify_detached(pk, sig, msg.as_bytes()) {
        return None;
    }
    pk.try_into().ok()
}

/// Controller side: what the last keyed handshake of one `Client::start` proved about the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostKeys {
    /// The host's machine ed25519 pk (rendezvous-signed `SignedId`).
    pub sign_pk: [u8; 32],
    /// The host's fresh box pk for this connection; binds our login signature to it.
    pub box_pk_b: [u8; 32],
}

hbb_common::tokio::task_local! {
    static HOST_KEYS: Arc<Mutex<Option<HostKeys>>>;
}

/// Runs one connection attempt (`Client::_start`) with its own slot for the host keys, and
/// returns them with the attempt's output. The slot belongs to this future alone, so concurrent
/// sessions (even to the same peer) never see each other's keys; the caller stores them in its
/// own session state (`LoginConfigHandler`).
pub async fn capture_host_keys<F: std::future::Future>(f: F) -> (F::Output, Option<HostKeys>) {
    let slot: Arc<Mutex<Option<HostKeys>>> = Default::default();
    let out = HOST_KEYS.scope(slot.clone(), f).await;
    let keys = *slot.lock().unwrap();
    (out, keys)
}

/// Called by `Client::secure_connection` once a keyed handshake succeeded. Outside
/// `capture_host_keys` (or in a spawned task) nothing is recorded: the login goes unsigned and
/// the host treats the controller as unauthenticated (fails closed). Within one attempt the
/// transport races may secure more than one stream; the last success wins, and if that is not
/// the stream finally used the host rejects the signature (again fails closed).
pub fn record_host_keys(sign_pk: &[u8], box_pk_b: &[u8]) {
    let (Ok(sign_pk), Ok(box_pk_b)) = (sign_pk.try_into(), box_pk_b.try_into()) else {
        return;
    };
    let _ = HOST_KEYS.try_with(|slot| {
        *slot.lock().unwrap() = Some(HostKeys { sign_pk, box_pk_b });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_signature_roundtrip() {
        let (pk, sk) = sign::gen_keypair();
        let host_pk_b = [7u8; 32];
        let auth = sign_login(&sk.0, "111222333", "444555666", 42, &host_pk_b);
        assert_eq!(auth.len(), 96);
        assert_eq!(
            verify_login(&auth, "111222333", "444555666", 42, &host_pk_b),
            Some(pk.0)
        );
    }

    #[test]
    fn login_signature_rejects_other_session() {
        let (_, sk) = sign::gen_keypair();
        let host_pk_b = [7u8; 32];
        let auth = sign_login(&sk.0, "111222333", "444555666", 42, &host_pk_b);
        assert_eq!(
            verify_login(&auth, "111222333", "444555666", 43, &host_pk_b),
            None
        );
        assert_eq!(
            verify_login(&auth, "111222334", "444555666", 42, &host_pk_b),
            None
        );
        assert_eq!(
            verify_login(&auth, "111222333", "444555667", 42, &host_pk_b),
            None
        );
    }

    #[test]
    fn login_signature_rejects_other_host_pk() {
        let (_, sk) = sign::gen_keypair();
        let auth = sign_login(&sk.0, "111222333", "444555666", 42, &[7u8; 32]);
        assert_eq!(
            verify_login(&auth, "111222333", "444555666", 42, &[8u8; 32]),
            None
        );
    }

    #[test]
    fn login_signature_rejects_malformed() {
        let (_, sk) = sign::gen_keypair();
        let host_pk_b = [7u8; 32];
        let mut auth = sign_login(&sk.0, "1", "2", 3, &host_pk_b);
        assert_eq!(verify_login(&[], "1", "2", 3, &host_pk_b), None);
        assert_eq!(verify_login(&auth[..95], "1", "2", 3, &host_pk_b), None);
        // No host pk: nothing to bind to, never authenticated.
        assert_eq!(verify_login(&auth, "1", "2", 3, &[]), None);
        // Swapped pk: the signature no longer matches.
        let (other_pk, _) = sign::gen_keypair();
        auth[..32].copy_from_slice(&other_pk.0);
        assert_eq!(verify_login(&auth, "1", "2", 3, &host_pk_b), None);
        // Bad local key: nothing is sent.
        assert!(sign_login(&[1u8; 5], "1", "2", 3, &host_pk_b).is_empty());
    }

    #[test]
    fn host_keys_are_per_attempt() {
        let rt = hbb_common::tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            // Outside any attempt: nothing is recorded anywhere.
            record_host_keys(&[1u8; 32], &[2u8; 32]);
            let a = capture_host_keys(async {
                hbb_common::tokio::task::yield_now().await;
                record_host_keys(&[1u8; 32], &[2u8; 32]);
            });
            let b = capture_host_keys(async {
                record_host_keys(&[3u8; 32], &[4u8; 32]);
                hbb_common::tokio::task::yield_now().await;
            });
            let none = capture_host_keys(async {});
            let bad = capture_host_keys(async { record_host_keys(&[1u8; 31], &[2u8; 32]) });
            let ((_, a), (_, b), (_, none), (_, bad)) = hbb_common::tokio::join!(a, b, none, bad);
            assert_eq!(
                a,
                Some(HostKeys {
                    sign_pk: [1; 32],
                    box_pk_b: [2; 32]
                })
            );
            assert_eq!(
                b,
                Some(HostKeys {
                    sign_pk: [3; 32],
                    box_pk_b: [4; 32]
                })
            );
            assert_eq!(none, None);
            assert_eq!(bad, None);
        });
    }
}
