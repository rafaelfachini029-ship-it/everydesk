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
use std::{collections::HashMap, sync::Mutex};

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

/// Controller side: the host's ephemeral box pk from the last keyed handshake with each peer,
/// recorded in `Client::secure_connection` and read when the `LoginRequest` is built. Keyed by
/// peer id; two concurrent sessions to the same peer may see each other's value, in which case
/// the signature fails on the host and the controller is merely unauthenticated (fails closed).
static HOST_PK_B: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);

pub fn set_host_pk_b(peer_id: &str, host_pk_b: &[u8]) {
    HOST_PK_B
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(peer_id.to_owned(), host_pk_b.to_vec());
}

pub fn host_pk_b(peer_id: &str) -> Vec<u8> {
    HOST_PK_B
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(peer_id).cloned())
        .unwrap_or_default()
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
}
