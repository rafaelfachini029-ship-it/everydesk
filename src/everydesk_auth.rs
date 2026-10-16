//! EveryDesk requester envelope for punch/relay requests to hbbs.
//!
//! `licence_key = "<server_key>;v1;<my_id>;<unix_ts>;<base64 sig>"`, where `sig` is the
//! ed25519 detached signature of `ed-punch-v1|<my_id>|<target_id>|<ts>` made with this
//! machine's key pair (the pk it registers with hbbs). The server side lives in
//! everydesk-server `src/requester_auth.rs`; the format is in its docs/protocolo-everydesk.md.

use hbb_common::{
    anyhow::anyhow,
    base64::{engine::general_purpose::STANDARD, Engine as _},
    bail,
    config::{Config, CONNECT_TIMEOUT, READ_TIMEOUT},
    log,
    rendezvous_proto::{rendezvous_message, OnlineRequest, RendezvousMessage},
    socket_client::connect_tcp,
    sodiumoxide::crypto::sign,
    ResultType,
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
    signed_envelope(server_key, my_id, ts, &msg, sk)
}

/// `<server_key>;v1;<my_id>;<ts>;<base64 sig of msg>`; just the server key when the
/// local key pair is unusable (hbbs will deny and log it; nothing better to send).
fn signed_envelope(server_key: &str, my_id: &str, ts: i64, msg: &str, sk: &[u8]) -> String {
    let Some(sig) = sign_detached(sk, msg.as_bytes()) else {
        log::error!("everydesk auth: invalid local key pair, sending no envelope");
        return server_key.to_owned();
    };
    format!("{server_key};v1;{my_id};{ts};{}", STANDARD.encode(sig))
}

const ONLINE_CONTEXT: &str = "ed-online-v1";

/// The `id` of an `OnlineRequest` (presence query): the same envelope, signed over
/// `ed-online-v1|<my_id>|<ts>`. hbbs answers at most one per 5 s per id (and 10 per
/// 5 s per IP).
pub fn online_envelope(server_key: &str) -> String {
    let my_id = Config::get_id();
    let (sk, _) = Config::get_key_pair();
    online_envelope_with(server_key, &my_id, next_ts(), &sk)
}

fn online_envelope_with(server_key: &str, my_id: &str, ts: i64, sk: &[u8]) -> String {
    let msg = format!("{ONLINE_CONTEXT}|{my_id}|{ts}");
    signed_envelope(server_key, my_id, ts, &msg, sk)
}

/// `(id, online)` for each of `ids` from hbbs' bitmap (one bit per id, left to
/// right); `None` when the bitmap is too short.
pub fn decode_online_states(ids: &[String], states: &[u8]) -> Option<Vec<(String, bool)>> {
    if states.len() < ids.len().div_ceil(8) {
        return None;
    }
    Some(
        ids.iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), states[i / 8] & (0x80 >> (i % 8)) != 0))
            .collect(),
    )
}

/// Presence of `ids` (at most 200) with a signed `OnlineRequest` to hbbs' port-1.
/// Mirrors `client::peer_online::query_online_states_`, which sends the bare id.
pub async fn query_online_signed(ids: &[String]) -> ResultType<Vec<(String, bool)>> {
    let (server, _, _) = crate::get_rendezvous_server(READ_TIMEOUT).await;
    let (host, port) = server
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("invalid server address: {server}"))?;
    let port: u16 = port.parse()?;
    if port == 0 {
        bail!("invalid server address: {server}");
    }
    let mut socket = connect_tcp(format!("{host}:{}", port - 1), CONNECT_TIMEOUT).await?;
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_online_request(OnlineRequest {
        id: online_envelope(&crate::get_key(true).await),
        peers: ids.to_vec(),
        ..Default::default()
    });
    socket.send(&msg_out).await?;
    // hbbs closes without an answer when it denies (unsigned, over 200, rate limited).
    match crate::get_next_nonkeyexchange_msg(&mut socket, Some(3_000)).await {
        Some(RendezvousMessage {
            union: Some(rendezvous_message::Union::OnlineResponse(r)),
            ..
        }) => decode_online_states(ids, &r.states)
            .ok_or_else(|| anyhow!("short online response")),
        _ => bail!("no online response (denied or rate limited?)"),
    }
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
    fn online_envelope_format() {
        let (pk, sk) = sign::gen_keypair();
        let env = online_envelope_with("SRVKEY=", "123456789", 1_800_000_000, &sk.0);
        let parts: Vec<&str> = env.split(';').collect();
        assert_eq!(&parts[..4], &["SRVKEY=", "v1", "123456789", "1800000000"]);
        let sig = STANDARD.decode(parts[4]).unwrap();
        let sig = sign::Signature::from_bytes(&sig).unwrap();
        // the server's `verify_online` message: its own domain, no target
        let msg = "ed-online-v1|123456789|1800000000";
        assert!(sign::verify_detached(&sig, msg.as_bytes(), &pk));
        // never valid as a punch envelope
        assert!(!server_accepts(&env, "", &pk));
    }

    #[test]
    fn online_states_bitmap() {
        let ids: Vec<String> = (0..10).map(|i| format!("id{i}")).collect();
        // bits left to right: id0, id2 online; id9 online (second byte, top bit... index 9 -> bit 6)
        let states = [0b1010_0000u8, 0b0100_0000];
        let out = decode_online_states(&ids, &states).unwrap();
        let online: Vec<&str> = out.iter().filter(|(_, o)| *o).map(|(i, _)| i.as_str()).collect();
        assert_eq!(online, ["id0", "id2", "id9"]);
        assert_eq!(out.len(), 10);
        assert!(decode_online_states(&ids, &states[..1]).is_none());
        assert_eq!(decode_online_states(&[], &[]).unwrap(), vec![]);
    }

    #[test]
    fn timestamps_strictly_increase() {
        let a = next_ts();
        let b = next_ts();
        assert!(b > a);
    }
}
