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
            None => log::warn!(
                "pairings: {} is not readable with this machine's key, starting empty",
                path.display()
            ),
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

    pub fn get(&self, peer_id: &str) -> Option<&Pairing> {
        self.pairings.get(peer_id)
    }

    pub fn upsert(&mut self, p: Pairing) {
        self.pairings.insert(p.peer_id.clone(), p);
    }

    pub fn remove(&mut self, peer_id: &str) {
        self.pairings.remove(peer_id);
    }

    /// The stored policy only when `peer_id` is paired AND presents the pk it paired with;
    /// anything else (unknown id, or a known id with another key) is `AskAlways`.
    pub fn policy_for(&self, peer_id: &str, pk: &[u8; 32]) -> Policy {
        match self.pairings.get(peer_id) {
            Some(p) if pk_eq(&p.pk, pk) => p.policy,
            _ => Policy::AskAlways,
        }
    }
}

const FILE_NAME: &str = "pairings.json";
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

fn pk_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    hbb_common::sodiumoxide::utils::memcmp(a, b)
}

pub fn book() -> &'static Mutex<Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    BOOK.get_or_init(|| Mutex::new(Book::load()))
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
}
