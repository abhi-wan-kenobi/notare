//! Peer allowlist — the set of devices this device will sync with.
//!
//! This is the security boundary that closes the SSRF finding from the S1
//! audit (§12 of `docs/internal/sync-p2p.md`): because the CloudSync endpoint
//! URL derives from a SQL-supplied `address`, the extension's network layer
//! will otherwise dial **any** host:port (or, over iroh, any node id) it is
//! handed. The allowlist is enforced at **both** connect (outbound dial) and
//! accept (inbound connection) time — a node id not on the list is refused in
//! either direction. See [`crate::agent`] for the enforcement call sites.
//!
//! ## Storage: a local JSON file, deliberately outside SQLite
//!
//! The allowlist is persisted as `<data_dir>/notare/sync/peers.json` — a plain
//! local file, **not** a SQLite table and **never** registered with
//! `cloudsync_init`. This is load-bearing:
//!
//! CRDT sync replicates every table registered with `cloudsync_init` to every
//! peer. If the allowlist were a synced table, a device that has been revoked
//! (removed from the list) could simply re-add itself by replicating its own
//! still-present local row to the peer that revoked it — revocation would be
//! undone by the very sync it is meant to gate. Keeping the allowlist in a
//! local file that the CRDT never sees makes revocation *structurally*
//! irreversible from a peer's perspective: only the local operator can edit
//! `peers.json`, and no sync round can touch it.
//!
//! (A SQLite table would also be viable *if* it were a LOCAL-only table never
//! passed to `cloudsync_init`; we use a JSON file instead because the list is
//! tiny, read on every dial/accept, and needs no schema/migration machinery —
//! and because a separate file makes the "not synced" invariant visually
//! obvious to anyone auditing the sync setup.)

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use iroh::PublicKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const PEERS_FILE: &str = "peers.json";

/// Errors from loading or mutating the peer allowlist.
#[derive(Debug, Error)]
pub enum PeersError {
    #[error("no platform data directory available")]
    NoDataDir,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// A single paired peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    /// The peer's device id (iroh `PublicKey` / `EndpointId`).
    #[serde(with = "pubkey_serde")]
    pub node_id: PublicKey,
    /// Human-readable label (e.g. "MacBook", "Pixel 8"). Free-form.
    #[serde(default)]
    pub label: String,
    /// Unix epoch seconds when the peer was added.
    pub added_at: i64,
    /// Unix epoch seconds of the last successful connection, or 0 if never.
    #[serde(default)]
    pub last_seen: i64,
}

/// The on-disk shape of `peers.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PeersFile {
    peers: Vec<Peer>,
}

/// An in-memory peer allowlist, backed by a JSON file on disk.
///
/// Cheap to clone (one `Arc`); safe to share across the dial/accept paths.
/// All mutations go through the file (load → mutate → store) under the
/// internal lock, so concurrent callers see a consistent list.
#[derive(Debug, Clone)]
pub struct PeerStore {
    inner: Arc<RwLock<PeerStoreInner>>,
}

#[derive(Debug)]
struct PeerStoreInner {
    path: PathBuf,
    /// The allowed node ids, held in memory for fast `is_allowed` checks.
    allowed: HashSet<[u8; 32]>,
    /// Full peer records (for `list_peers` / labels / last_seen).
    peers: Vec<Peer>,
}

impl PeerStore {
    /// Open the allowlist at `<data_dir>/notare/sync/peers.json`, creating an
    /// empty store on first use.
    pub fn load_or_create() -> Result<Self, PeersError> {
        let dir = crate::identity::sync_dir().ok_or(PeersError::NoDataDir)?;
        Self::load_or_create_in(&dir)
    }

    /// Open rooted at an explicit dir (for tests / non-standard app dirs).
    pub fn load_or_create_in(dir: &Path) -> Result<Self, PeersError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(PEERS_FILE);
        Self::open_path(&path)
    }

    /// Open a specific file path.
    fn open_path(path: &Path) -> Result<Self, PeersError> {
        let file = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<PeersFile>(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => PeersFile::default(),
            Err(e) => return Err(PeersError::Io(e)),
        };
        let allowed = file.peers.iter().map(|p| *p.node_id.as_bytes()).collect();
        Ok(Self {
            inner: Arc::new(RwLock::new(PeerStoreInner {
                path: path.to_path_buf(),
                allowed,
                peers: file.peers,
            })),
        })
    }

    /// Is `node_id` on the allowlist? This is the hot path, called on every
    /// dial and every accepted connection.
    pub fn is_allowed(&self, node_id: &PublicKey) -> bool {
        // A poisoned lock means a writer panicked mid-mutation; for a security
        // gate the safe behavior is to fail hard rather than silently allow.
        self.inner
            .read()
            .expect("peer store lock poisoned")
            .allowed
            .contains(node_id.as_bytes())
    }

    /// All paired peers, in insertion order.
    pub fn list_peers(&self) -> Vec<Peer> {
        self.inner
            .read()
            .expect("peer store lock poisoned")
            .peers
            .clone()
    }

    /// Add (or, if already present, update the label of) a peer. Returns the
    /// peer as stored.
    pub fn add_peer(&self, node_id: PublicKey, label: impl Into<String>) -> Result<Peer, PeersError> {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        let label = label.into();
        let added_at = now_secs();
        // Update-in-place vs. insert: decide and apply first, releasing the
        // mutable borrow of `g.peers` before we touch `g` again to persist.
        let stored = if let Some(existing) = g.peers.iter_mut().find(|p| p.node_id == node_id) {
            existing.label = label;
            existing.clone()
        } else {
            let peer = Peer {
                node_id,
                label,
                added_at,
                last_seen: 0,
            };
            g.peers.push(peer.clone());
            g.allowed.insert(*node_id.as_bytes());
            peer
        };
        let snapshot = PeersFile { peers: g.peers.clone() };
        g.persist(&snapshot)?;
        Ok(stored)
    }

    /// Revoke a peer: remove it from the allowlist. Future dials to and
    /// accepts from this node id are refused. The peer cannot re-add itself
    /// — see the module docs on why the allowlist is not CRDT-synced.
    pub fn remove_peer(&self, node_id: &PublicKey) -> Result<bool, PeersError> {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        let before = g.peers.len();
        g.peers.retain(|p| &p.node_id != node_id);
        g.allowed.remove(node_id.as_bytes());
        let removed = g.peers.len() < before;
        if removed {
            let snapshot = PeersFile { peers: g.peers.clone() };
            g.persist(&snapshot)?;
        }
        Ok(removed)
    }

    /// Record that `node_id` was just seen on a successful connection.
    pub fn touch_last_seen(&self, node_id: &PublicKey) {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        let Some(peer) = g.peers.iter_mut().find(|p| &p.node_id == node_id) else {
            return;
        };
        peer.last_seen = now_secs();
        let snapshot = PeersFile { peers: g.peers.clone() };
        // Best-effort persist; a failed last_seen write must not tear down a
        // working connection.
        let _ = g.persist(&snapshot);
    }
}

impl PeerStoreInner {
    fn persist(&self, file: &PeersFile) -> Result<(), PeersError> {
        let bytes = serde_json::to_vec_pretty(file)?;
        // Temp file + rename so a crash mid-write can't corrupt the allowlist.
        let dir = self
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let tmp = dir.join(format!(
            ".{PEERS_FILE}.{}.{}.tmp",
            std::process::id(),
            tmp_counter()
        ));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn tmp_counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    C.fetch_add(1, Ordering::Relaxed)
}

/// Best-effort monotonic clock for timestamps. Uses `SystemTime` directly —
/// the allowlist only needs wall-clock seconds for display/audit, not a
/// monotonic ordering source.
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// serde adapter for `PublicKey`: store as the dashed fingerprint (human-auditable
/// in the JSON) but accept both grouped and compact forms on load.
mod pubkey_serde {
    use iroh::PublicKey;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::identity::Fingerprint;

    pub fn serialize<S: Serializer>(pk: &PublicKey, s: S) -> Result<S::Ok, S::Error> {
        Fingerprint::from_pubkey(pk).as_str().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PublicKey, D::Error> {
        let s = String::deserialize(d)?;
        Fingerprint::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    fn pk() -> PublicKey {
        SecretKey::generate().public()
    }

    #[test]
    fn add_list_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();

        let a = pk();
        let b = pk();
        store.add_peer(a, "Alice").unwrap();
        store.add_peer(b, "Bob").unwrap();

        assert!(store.is_allowed(&a));
        assert!(store.is_allowed(&b));
        assert!(!store.is_allowed(&pk()), "random node not allowed");

        let listed = store.list_peers();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].label, "Alice");

        // Reload from disk — both peers persisted.
        let store2 = PeerStore::load_or_create_in(dir.path()).unwrap();
        assert!(store2.is_allowed(&a));
        assert!(store2.is_allowed(&b));
        assert_eq!(store2.list_peers().len(), 2);

        // Revoke Alice.
        assert!(store2.remove_peer(&a).unwrap());
        assert!(!store2.is_allowed(&a), "revoked peer no longer allowed");
        assert!(store2.is_allowed(&b));
        assert!(!store2.remove_peer(&a).unwrap(), "second revoke is a no-op");

        // Reload again — revocation persisted.
        let store3 = PeerStore::load_or_create_in(dir.path()).unwrap();
        assert!(!store3.is_allowed(&a));
        assert!(store3.is_allowed(&b));
    }

    #[test]
    fn add_peer_updates_label_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();
        let a = pk();
        store.add_peer(a, "old").unwrap();
        store.add_peer(a, "new label").unwrap();
        let peers = store.list_peers();
        assert_eq!(peers.len(), 1, "re-adding the same node id does not duplicate");
        assert_eq!(peers[0].label, "new label");
    }

    #[test]
    fn peers_json_stores_fingerprint_not_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();
        let a = pk();
        store.add_peer(a, "Alice").unwrap();
        let raw = std::fs::read_to_string(dir.path().join(PEERS_FILE)).unwrap();
        assert!(
            raw.contains('-'),
            "node id is stored as a dashed fingerprint, not raw bytes"
        );
        // And the stored fingerprint round-trips.
        let store2 = PeerStore::load_or_create_in(dir.path()).unwrap();
        assert!(store2.is_allowed(&a));
    }

    #[test]
    fn touch_last_seen_records_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();
        let a = pk();
        store.add_peer(a, "Alice").unwrap();
        assert_eq!(store.list_peers()[0].last_seen, 0);
        store.touch_last_seen(&a);
        assert!(store.list_peers()[0].last_seen > 0);
    }
}
