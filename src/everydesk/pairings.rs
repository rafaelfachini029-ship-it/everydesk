//! Local pairing book: peers this machine has paired with, keyed by peer id and pinned to
//! the peer's ed25519 pk. Stored in `pairings.json` in the config dir, encrypted with
//! `secretbox` under a key derived from this machine's signing sk.

use hbb_common::{
    base64::{engine::general_purpose::STANDARD, Engine as _},
    config::Config,
    log,
    sodiumoxide::crypto::{hash::sha256, secretbox},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Policy {
    #[default]
    AskAlways,
    StayConnected,
    Reconfirm {
        every_min: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pairing {
    pub peer_id: String,
    pub pk: [u8; 32],
    pub name: String,
    #[serde(default)]
    pub avatar: Option<Vec<u8>>,
    #[serde(default)]
    pub policy: Policy,
    pub created_at: i64,
    /// The owner denied (or let time out) a reconfirmation: the next login goes through the
    /// accept card whatever the policy, so reconnecting cannot undo the deny. Cleared when
    /// the owner accepts a card for this peer.
    #[serde(default)]
    pub ask_next: bool,
}

/// What the accept card may say about who is asking (`ipc::Data::IdentityInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityState {
    /// The controller signed the login with a key bound to this connection, and no book
    /// pins its id to another key.
    Verified,
    /// No (valid) signature: the id and name are only what the peer claims.
    Unverified,
    /// Signed, but one of the books pins this id to another key.
    KeyChanged,
}

/// The identity of a pending login: `controller_pk` is the verified controller key (`None`
/// when unauthenticated). The bool is true when the claimed id is in either book (an
/// unverified or re-keyed claim of a known id is the loud "a chave não confere" case).
pub fn identity_of(
    controller_pk: Option<&[u8; 32]>,
    peer_id: &str,
    host: &Book,
    contacts: &Book,
) -> (IdentityState, bool) {
    let known = host.get(peer_id).is_some() || contacts.get(peer_id).is_some();
    let state = match controller_pk {
        None => IdentityState::Unverified,
        Some(pk) if host.key_changed(peer_id, pk) || contacts.key_changed(peer_id, pk) => {
            IdentityState::KeyChanged
        }
        Some(_) => IdentityState::Verified,
    };
    (state, known)
}

// No Debug: it holds the machine sk.
#[derive(Default)]
pub struct Book {
    pairings: HashMap<String, Pairing>,
    path: PathBuf,
    sk: Vec<u8>,
}

impl Book {
    /// This machine's book; an empty one if the file is missing or unreadable.
    pub fn load() -> Book {
        let (sk, _) = Config::get_key_pair();
        Self::load_from(&Config::path(FILE_NAME), &sk)
    }

    fn load_from(path: &Path, sk: &[u8]) -> Book {
        let mut book = Book {
            pairings: HashMap::new(),
            path: path.to_owned(),
            sk: sk.to_vec(),
        };
        let raw = match std::fs::read(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return book,
            Err(e) => {
                log::warn!("pairings: cannot read {}: {}", path.display(), e);
                return book;
            }
        };
        match decrypt(&raw, sk)
            .and_then(|plain| serde_json::from_slice::<Vec<Pairing>>(&plain).ok())
        {
            Some(list) => {
                book.pairings = list.into_iter().map(|p| (p.peer_id.clone(), p)).collect();
            }
            None => {
                // Keep the old file (e.g. the machine key was regenerated): a later save must
                // not silently overwrite what may still be recoverable.
                let bak = backup_path(path, hbb_common::get_time());
                match std::fs::rename(path, &bak) {
                    Ok(()) => log::warn!(
                        "pairings: {} is not readable with this machine's key; moved to {}, starting empty",
                        path.display(),
                        bak.display()
                    ),
                    Err(e) => log::warn!(
                        "pairings: {} is not readable with this machine's key and cannot be moved to {}: {}; starting empty",
                        path.display(),
                        bak.display(),
                        e
                    ),
                }
            }
        }
        book
    }

    pub fn save(&self) -> ResultType<()> {
        let mut list: Vec<&Pairing> = self.pairings.values().collect();
        list.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
        let plain = serde_json::to_vec(&list)?;
        let data = encrypt(&plain, &self.sk)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Write then rename, so a crash never leaves a truncated book.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// A book of this machine's at `path` (same encryption as `pairings.json`).
    pub fn load_at(path: &Path) -> Book {
        let (sk, _) = Config::get_key_pair();
        Self::load_from(path, &sk)
    }

    /// `peer_id` is paired, but under another key than `pk`.
    pub fn key_changed(&self, peer_id: &str, pk: &[u8; 32]) -> bool {
        matches!(self.pairings.get(peer_id), Some(p) if !pk_eq(&p.pk, pk))
    }

    /// Stores `p`, except that a peer already pinned to another key keeps its pin unless
    /// `replace_key` (the owner explicitly confirmed the key change). True when stored.
    pub fn remember(&mut self, p: Pairing, replace_key: bool) -> bool {
        if self.key_changed(&p.peer_id, &p.pk) && !replace_key {
            return false;
        }
        self.upsert(p);
        true
    }

    pub fn get(&self, peer_id: &str) -> Option<&Pairing> {
        self.pairings.get(peer_id)
    }

    pub fn upsert(&mut self, p: Pairing) {
        self.pairings.insert(p.peer_id.clone(), p);
    }

    pub fn remove(&mut self, peer_id: &str) {
        self.pairings.remove(peer_id);
    }

    /// The stored policy only when `peer_id` is paired AND presents the pk it paired with
    /// AND no deny is pending (`ask_next`); anything else is `AskAlways`.
    pub fn policy_for(&self, peer_id: &str, pk: &[u8; 32]) -> Policy {
        match self.pairings.get(peer_id) {
            Some(p) if pk_eq(&p.pk, pk) && !p.ask_next => p.policy,
            _ => Policy::AskAlways,
        }
    }

    /// Sets `peer_id`'s `ask_next`. With `pk`, only when the pairing is pinned to that key
    /// (an impostor's accepted card must not lift a deny). True when it changed.
    pub fn set_ask_next(&mut self, peer_id: &str, pk: Option<&[u8; 32]>, on: bool) -> bool {
        match self.pairings.get_mut(peer_id) {
            Some(p) if p.ask_next != on && pk.map_or(true, |k| pk_eq(&p.pk, k)) => {
                p.ask_next = on;
                true
            }
            _ => false,
        }
    }

    /// Every pairing, sorted by peer id (stable output for the contacts list).
    pub fn list(&self) -> Vec<&Pairing> {
        let mut list: Vec<&Pairing> = self.pairings.values().collect();
        list.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
        list
    }
}

const FILE_NAME: &str = "pairings.json";
/// Controller side: the hosts that remembered us, for the contacts list only. Kept apart from
/// `pairings.json` so nothing a peer sends can change the owner's trust decisions.
const CONTACTS_FILE_NAME: &str = "contacts.json";
const KEY_CONTEXT: &[u8] = b"everydesk-pairings-v1|";

#[derive(Serialize, Deserialize)]
struct Sealed {
    v: u32,
    nonce: String,
    data: String,
}

fn book_key(sk: &[u8]) -> ResultType<secretbox::Key> {
    if sk.is_empty() {
        hbb_common::bail!("no local key pair");
    }
    let mut input = KEY_CONTEXT.to_vec();
    input.extend_from_slice(sk);
    let digest = sha256::hash(&input);
    Ok(secretbox::Key(digest.0))
}

fn encrypt(plain: &[u8], sk: &[u8]) -> ResultType<Vec<u8>> {
    let key = book_key(sk)?;
    let nonce = secretbox::gen_nonce();
    let sealed = Sealed {
        v: 1,
        nonce: STANDARD.encode(nonce.0),
        data: STANDARD.encode(secretbox::seal(plain, &nonce, &key)),
    };
    Ok(serde_json::to_vec(&sealed)?)
}

fn decrypt(raw: &[u8], sk: &[u8]) -> Option<Vec<u8>> {
    let sealed: Sealed = serde_json::from_slice(raw).ok()?;
    if sealed.v != 1 {
        return None;
    }
    let key = book_key(sk).ok()?;
    let nonce = secretbox::Nonce::from_slice(&STANDARD.decode(sealed.nonce).ok()?)?;
    secretbox::open(&STANDARD.decode(sealed.data).ok()?, &nonce, &key).ok()
}

/// `<file name>.bak-<ts>` next to `path`.
fn backup_path(path: &Path, ts: i64) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE_NAME.to_owned());
    path.with_file_name(format!("{name}.bak-{ts}"))
}

fn pk_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    hbb_common::sodiumoxide::utils::memcmp(a, b)
}

/// This machine's trust book (`pairings.json`): the owner's decisions about who may connect.
/// Written only by the host on the owner's answers (accept card, revoke).
pub fn book() -> &'static Mutex<Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    BOOK.get_or_init(|| Mutex::new(Book::load()))
}

/// The hosts that remembered this machine (`contacts.json`), fed by wire messages from those
/// hosts; for the contacts list only, never consulted for authorization.
pub fn contacts_book() -> &'static Mutex<Book> {
    static CONTACTS: OnceLock<Mutex<Book>> = OnceLock::new();
    CONTACTS.get_or_init(|| Mutex::new(Book::load_at(&Config::path(CONTACTS_FILE_NAME))))
}

/// What a host told us over the wire (`Misc.everydesk`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireEvent {
    /// The host remembered us; `sign_pk` is the host key this session verified.
    PairAccepted {
        sign_pk: [u8; 32],
        name: String,
    },
    Revoked,
}

/// Controller side: applies a host's wire message to the contacts book. True when changed.
pub fn apply_wire_to_contacts(contacts: &mut Book, peer_id: &str, ev: WireEvent, now: i64) -> bool {
    match ev {
        WireEvent::PairAccepted { sign_pk, name } => {
            let existing = contacts.get(peer_id).cloned();
            contacts.upsert(Pairing {
                peer_id: peer_id.to_owned(),
                pk: sign_pk,
                name,
                avatar: existing.as_ref().and_then(|p| p.avatar.clone()),
                // Placeholder: the contacts book only lists contacts.
                policy: Policy::AskAlways,
                created_at: existing.map(|p| p.created_at).unwrap_or(now),
                ask_next: false,
            });
            true
        }
        WireEvent::Revoked => {
            if contacts.get(peer_id).is_none() {
                return false;
            }
            contacts.remove(peer_id);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::sodiumoxide::crypto::sign;

    fn pairing(peer_id: &str, pk: [u8; 32], policy: Policy) -> Pairing {
        Pairing {
            peer_id: peer_id.to_owned(),
            pk,
            name: "Notebook da Marta".to_owned(),
            avatar: Some(vec![1, 2, 3]),
            policy,
            created_at: 1_800_000_000,
            ask_next: false,
        }
    }

    fn temp_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "everydesk-pairings-{tag}-{}-{}",
            std::process::id(),
            hbb_common::get_time()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("pairings.json")
    }

    #[test]
    fn policy_for_requires_matching_pk() {
        let (_, sk) = sign::gen_keypair();
        let mut book = Book::load_from(&temp_path("policy"), &sk.0);
        let a = [0xAA; 32];
        let b = [0xBB; 32];
        assert_eq!(book.policy_for("123456789", &a), Policy::AskAlways);
        book.upsert(pairing("123456789", a, Policy::StayConnected));
        assert_eq!(book.policy_for("123456789", &a), Policy::StayConnected);
        // Same id, different key: an impostor (or a reinstalled peer) is unknown.
        assert_eq!(book.policy_for("123456789", &b), Policy::AskAlways);
        assert_eq!(book.policy_for("987654321", &a), Policy::AskAlways);
        book.remove("123456789");
        assert_eq!(book.policy_for("123456789", &a), Policy::AskAlways);
        assert!(book.get("123456789").is_none());
    }

    #[test]
    fn remember_refuses_key_change_without_flag() {
        let (_, sk) = sign::gen_keypair();
        let mut book = Book::load_from(&temp_path("keychange"), &sk.0);
        let a = [0xAA; 32];
        let b = [0xBB; 32];
        assert!(!book.key_changed("123456789", &a));
        assert!(book.remember(pairing("123456789", a, Policy::StayConnected), false));
        assert!(!book.key_changed("123456789", &a));
        assert!(book.key_changed("123456789", &b));
        // An impostor accepted once from the card must not take over the pin.
        assert!(!book.remember(pairing("123456789", b, Policy::StayConnected), false));
        assert_eq!(book.get("123456789").unwrap().pk, a);
        assert_eq!(book.policy_for("123456789", &b), Policy::AskAlways);
        // Same key: an update (policy change) is fine.
        assert!(book.remember(pairing("123456789", a, Policy::AskAlways), false));
        assert_eq!(book.policy_for("123456789", &a), Policy::AskAlways);
    }

    #[test]
    fn remember_replaces_key_with_flag() {
        let (_, sk) = sign::gen_keypair();
        let mut book = Book::load_from(&temp_path("keyreplace"), &sk.0);
        let a = [0xAA; 32];
        let b = [0xBB; 32];
        book.upsert(pairing("123456789", a, Policy::StayConnected));
        assert!(book.remember(pairing("123456789", b, Policy::StayConnected), true));
        assert_eq!(book.policy_for("123456789", &b), Policy::StayConnected);
        assert_eq!(book.policy_for("123456789", &a), Policy::AskAlways);
    }

    #[test]
    fn wire_pairack_does_not_touch_host_book() {
        assert_ne!(FILE_NAME, CONTACTS_FILE_NAME);
        let (_, sk) = sign::gen_keypair();
        let host_path = temp_path("wire-host");
        let contacts_path = temp_path("wire-contacts");
        let mut host = Book::load_from(&host_path, &sk.0);
        let owner = pairing("123456789", [0xAA; 32], Policy::StayConnected);
        host.upsert(owner.clone());
        host.save().unwrap();

        let mut contacts = Book::load_from(&contacts_path, &sk.0);
        let ack = WireEvent::PairAccepted {
            sign_pk: [0xCC; 32],
            name: "Servidor".to_owned(),
        };
        assert!(apply_wire_to_contacts(&mut contacts, "123456789", ack, 7));
        assert_eq!(contacts.get("123456789").unwrap().pk, [0xCC; 32]);
        assert_eq!(contacts.get("123456789").unwrap().policy, Policy::AskAlways);
        contacts.save().unwrap();
        assert!(apply_wire_to_contacts(
            &mut contacts,
            "123456789",
            WireEvent::Revoked,
            8
        ));
        assert!(!apply_wire_to_contacts(
            &mut contacts,
            "123456789",
            WireEvent::Revoked,
            9
        ));
        contacts.save().unwrap();

        // The owner's decision about 123456789 is untouched on disk.
        let host = Book::load_from(&host_path, &sk.0);
        assert_eq!(host.get("123456789"), Some(&owner));
        assert_eq!(
            host.policy_for("123456789", &[0xAA; 32]),
            Policy::StayConnected
        );
    }

    #[test]
    fn list_is_sorted_and_tracks_changes() {
        let (_, sk) = sign::gen_keypair();
        let mut book = Book::load_from(&temp_path("list"), &sk.0);
        assert!(book.list().is_empty());
        book.upsert(pairing("ccc", [1; 32], Policy::AskAlways));
        book.upsert(pairing("aaa", [2; 32], Policy::StayConnected));
        book.upsert(pairing("bbb", [3; 32], Policy::AskAlways));
        let ids: Vec<&str> = book.list().iter().map(|p| p.peer_id.as_str()).collect();
        assert_eq!(ids, ["aaa", "bbb", "ccc"]);
        book.remove("bbb");
        let ids: Vec<&str> = book.list().iter().map(|p| p.peer_id.as_str()).collect();
        assert_eq!(ids, ["aaa", "ccc"]);
        assert_eq!(book.list()[0].policy, Policy::StayConnected);
    }

    #[test]
    fn book_roundtrip_encrypted() {
        let (_, sk) = sign::gen_keypair();
        let path = temp_path("roundtrip");
        let mut book = Book::load_from(&path, &sk.0);
        let p = pairing("123456789", [0xAA; 32], Policy::Reconfirm { every_min: 30 });
        book.upsert(p.clone());
        book.save().unwrap();

        let raw = std::fs::read(&path).unwrap();
        let needle = b"Notebook da Marta";
        assert!(!raw.windows(needle.len()).any(|w| w == needle));
        assert!(!raw.windows(9).any(|w| w == b"123456789"));

        let loaded = Book::load_from(&path, &sk.0);
        assert_eq!(loaded.get("123456789"), Some(&p));

        // Another machine's key cannot read it: an empty book, not an error.
        let (_, other) = sign::gen_keypair();
        assert!(Book::load_from(&path, &other.0).get("123456789").is_none());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn unreadable_book_is_moved_aside() {
        let (_, sk) = sign::gen_keypair();
        let path = temp_path("bak");
        let mut book = Book::load_from(&path, &sk.0);
        book.upsert(pairing("123456789", [0xAA; 32], Policy::StayConnected));
        book.save().unwrap();
        let original = std::fs::read(&path).unwrap();

        // The machine key was regenerated: the book is unreadable, starts empty, and the old
        // file is kept as `pairings.json.bak-<ts>` instead of being overwritten by a save.
        let (_, other) = sign::gen_keypair();
        let mut fresh = Book::load_from(&path, &other.0);
        assert!(fresh.get("123456789").is_none());
        assert!(!path.exists());
        let dir = path.parent().unwrap();
        let baks: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("pairings.json.bak-")
            })
            .collect();
        assert_eq!(baks.len(), 1);
        assert_eq!(std::fs::read(&baks[0]).unwrap(), original);
        fresh.upsert(pairing("987654321", [0xBB; 32], Policy::AskAlways));
        fresh.save().unwrap();
        assert_eq!(std::fs::read(&baks[0]).unwrap(), original);
        // The original key still reads the backup.
        let restored = Book::load_from(&baks[0], &sk.0);
        assert!(restored.get("123456789").is_some());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn deny_forces_card_on_next_login() {
        let (_, sk) = sign::gen_keypair();
        let path = temp_path("asknext");
        let mut book = Book::load_from(&path, &sk.0);
        let a = [0xAA; 32];
        let b = [0xBB; 32];
        book.upsert(pairing("123456789", a, Policy::Reconfirm { every_min: 20 }));
        // Reconfirmation denied (or timed out): flagged and persisted.
        assert!(book.set_ask_next("123456789", None, true));
        book.save().unwrap();
        let mut book = Book::load_from(&path, &sk.0);
        // Reconnecting does not bring a fresh period: the card decides.
        assert_eq!(book.policy_for("123456789", &a), Policy::AskAlways);
        assert!(book.get("123456789").unwrap().ask_next);
        // Another key accepted from a card does not lift the deny.
        assert!(!book.set_ask_next("123456789", Some(&b), false));
        assert_eq!(book.policy_for("123456789", &a), Policy::AskAlways);
        // The owner accepted the card for the pinned key: the policy applies again.
        assert!(book.set_ask_next("123456789", Some(&a), false));
        assert_eq!(
            book.policy_for("123456789", &a),
            Policy::Reconfirm { every_min: 20 }
        );
        // Unknown id: nothing to flag.
        assert!(!book.set_ask_next("000", None, true));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn identity_of_flags_unverified_and_changed_keys() {
        let (_, sk) = sign::gen_keypair();
        let mut host = Book::load_from(&temp_path("id-host"), &sk.0);
        let mut contacts = Book::load_from(&temp_path("id-contacts"), &sk.0);
        let a = [0xAA; 32];
        let b = [0xBB; 32];
        host.upsert(pairing("111", a, Policy::StayConnected));
        contacts.upsert(pairing("222", a, Policy::AskAlways));
        use IdentityState::*;
        assert_eq!(identity_of(None, "999", &host, &contacts), (Unverified, false));
        assert_eq!(identity_of(None, "111", &host, &contacts), (Unverified, true));
        assert_eq!(identity_of(None, "222", &host, &contacts), (Unverified, true));
        assert_eq!(identity_of(Some(&a), "111", &host, &contacts), (Verified, true));
        assert_eq!(identity_of(Some(&b), "999", &host, &contacts), (Verified, false));
        assert_eq!(identity_of(Some(&b), "111", &host, &contacts), (KeyChanged, true));
        assert_eq!(identity_of(Some(&b), "222", &host, &contacts), (KeyChanged, true));
        assert_eq!(
            serde_json::to_string(&KeyChanged).unwrap(),
            "\"key_changed\""
        );
    }
}
