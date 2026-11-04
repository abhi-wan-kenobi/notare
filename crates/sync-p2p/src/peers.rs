//! Peer allowlist — the set of devices this device will sync with.
//!
//! This is the security boundary that closes the SSRF finding from the S1
//! audit (§12 of `docs/internal/sync-p2p.md`): without it the agent would
//! dial **any** node id it is handed and accept any that connects. The
//! allowlist is enforced at **both** connect (outbound dial) and accept
//! (inbound connection) time — a node id not on the list is refused in
//! either direction. See [`crate::agent`] for the enforcement call sites.
//!
//! ## Storage: a local JSON file, deliberately outside SQLite
//!
//! The allowlist is persisted as `<data_dir>/notare/sync/peers.json` — a plain
//! local file, **not** a SQLite table and **never** a cr-sqlite CRR. This is
//! load-bearing:
//!
//! CRDT sync replicates every CRR to every
//! peer. If the allowlist were a synced table, a device that has been revoked
//! (removed from the list) could simply re-add itself by replicating its own
//! still-present local row to the peer that revoked it — revocation would be
//! undone by the very sync it is meant to gate. Keeping the allowlist in a
//! local file that the CRDT never sees makes revocation *structurally*
//! irreversible from a peer's perspective: only the local operator can edit
//! `peers.json`, and no sync round can touch it.
//!
//! (A SQLite table would also be viable *if* it were a LOCAL-only table never
//! made a CRR; we use a JSON file instead because the list is
//! tiny, read on every dial/accept, and needs no schema/migration machinery —
//! and because a separate file makes the "not synced" invariant visually
//! obvious to anyone auditing the sync setup.)

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use iroh::PublicKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identity::Fingerprint;

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
    #[error("device has been revoked")]
    Revoked,
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
    /// Device ids that have been explicitly revoked. A revoked id cannot be
    /// re-added via `add_peer` — `revoke` is permanent until the operator
    /// edits the file by hand.
    #[serde(default)]
    revoked: HashSet<String>,
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
    /// z-base-32 compact ids of revoked devices. Persisted in `peers.json`.
    revoked: HashSet<String>,
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
    ///
    /// Revocation wins over the allowlist: a peer listed in both (a hand edit,
    /// or a file written by an older build) is dropped from the allowlist.
    fn open_path(path: &Path) -> Result<Self, PeersError> {
        let file = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<PeersFile>(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => PeersFile::default(),
            Err(e) => return Err(PeersError::Io(e)),
        };
        // Normalize hand-entered dashed/spaced ids to the compact form lookups
        // use; keep anything unparseable verbatim rather than lose an entry.
        let revoked: HashSet<String> = file
            .revoked
            .into_iter()
            .map(|s| Fingerprint::parse(&s).map(|pk| pk.to_z32()).unwrap_or(s))
            .collect();
        let peers: Vec<Peer> = file
            .peers
            .into_iter()
            .filter(|p| !revoked.contains(&p.node_id.to_z32()))
            .collect();
        let allowed = peers.iter().map(|p| *p.node_id.as_bytes()).collect();
        Ok(Self {
            inner: Arc::new(RwLock::new(PeerStoreInner {
                path: path.to_path_buf(),
                allowed,
                peers,
                revoked,
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
    /// peer as stored. Rejects peers whose id is in the revoked set.
    ///
    /// Every mutation is written to disk before it is applied in memory, so a
    /// failed write leaves both unchanged.
    pub fn add_peer(
        &self,
        node_id: PublicKey,
        label: impl Into<String>,
    ) -> Result<Peer, PeersError> {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        if g.revoked.contains(&node_id.to_z32()) {
            return Err(PeersError::Revoked);
        }
        let label = label.into();
        let mut peers = g.peers.clone();
        let stored = if let Some(existing) = peers.iter_mut().find(|p| p.node_id == node_id) {
            existing.label = label;
            existing.clone()
        } else {
            let peer = Peer {
                node_id,
                label,
                added_at: now_secs(),
                last_seen: 0,
            };
            peers.push(peer.clone());
            peer
        };
        persist(&g.path, &peers, &g.revoked)?;
        g.peers = peers;
        g.allowed.insert(*node_id.as_bytes());
        Ok(stored)
    }

    /// Remove a peer from the allowlist without recording it as revoked.
    /// The peer can be re-added later. For permanent removal, use
    /// [`Self::revoke`].
    pub fn remove_peer(&self, node_id: &PublicKey) -> Result<bool, PeersError> {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        if !g.peers.iter().any(|p| &p.node_id == node_id) {
            return Ok(false);
        }
        let peers: Vec<Peer> = g
            .peers
            .iter()
            .filter(|p| &p.node_id != node_id)
            .cloned()
            .collect();
        persist(&g.path, &peers, &g.revoked)?;
        g.peers = peers;
        g.allowed.remove(node_id.as_bytes());
        Ok(true)
    }

    /// Revoke a peer: remove it from the allowlist AND record it in the
    /// revoked set so it cannot be re-added (even from a stale pairing
    /// offer). Returns whether the peer was in the allowlist.
    pub fn revoke(&self, node_id: &PublicKey) -> Result<bool, PeersError> {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        let removed = g.peers.iter().any(|p| &p.node_id == node_id);
        let peers: Vec<Peer> = g
            .peers
            .iter()
            .filter(|p| &p.node_id != node_id)
            .cloned()
            .collect();
        let mut revoked = g.revoked.clone();
        revoked.insert(node_id.to_z32());
        // Always persist — even if the peer wasn't in the list, we need
        // the revoked entry written.
        persist(&g.path, &peers, &revoked)?;
        g.peers = peers;
        g.revoked = revoked;
        g.allowed.remove(node_id.as_bytes());
        Ok(removed)
    }

    /// Whether `node_id` is in the revoked set.
    pub fn is_revoked(&self, node_id: &PublicKey) -> bool {
        self.inner
            .read()
            .expect("peer store lock poisoned")
            .revoked
            .contains(&node_id.to_z32())
    }

    /// Record that `node_id` was just seen on a successful connection.
    pub fn touch_last_seen(&self, node_id: &PublicKey) {
        let mut g = self.inner.write().expect("peer store lock poisoned");
        let Some(peer) = g.peers.iter_mut().find(|p| &p.node_id == node_id) else {
            return;
        };
        peer.last_seen = now_secs();
        // Best-effort persist; a failed last_seen write must not tear down a
        // working connection.
        let _ = persist(&g.path, &g.peers, &g.revoked);
    }
}

/// Write `peers` + `revoked` to `path` (temp file + rename, so a crash
/// mid-write can't corrupt the allowlist).
fn persist(path: &Path, peers: &[Peer], revoked: &HashSet<String>) -> Result<(), PeersError> {
    #[derive(Serialize)]
    struct PeersFileRef<'a> {
        peers: &'a [Peer],
        revoked: &'a HashSet<String>,
    }
    let bytes = serde_json::to_vec_pretty(&PeersFileRef { peers, revoked })?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!(
        ".{PEERS_FILE}.{}.{}.tmp",
        std::process::id(),
        tmp_counter()
    ));
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
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

    use super::Fingerprint;

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
        assert_eq!(
            peers.len(),
            1,
            "re-adding the same node id does not duplicate"
        );
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

    #[test]
    fn revoke_blocks_re_add() {
        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();

        let a = pk();
        store.add_peer(a, "Alice").unwrap();
        assert!(store.is_allowed(&a));

        // Revoke Alice — she's removed AND recorded.
        assert!(store.revoke(&a).unwrap());
        assert!(!store.is_allowed(&a));
        assert!(store.is_revoked(&a));

        // Cannot re-add a revoked peer.
        let result = store.add_peer(a, "Alice again");
        assert!(matches!(result, Err(PeersError::Revoked)));
        assert!(!store.is_allowed(&a));

        // Revoked set persists across reload.
        let store2 = PeerStore::load_or_create_in(dir.path()).unwrap();
        assert!(store2.is_revoked(&a));
        assert!(matches!(
            store2.add_peer(a, "retry"),
            Err(PeersError::Revoked)
        ));
    }

    #[test]
    fn revoke_nonexistent_peer_still_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();
        let a = pk();

        // Revoking a peer that was never added is valid — it prevents
        // future addition from a stale offer.
        assert!(!store.revoke(&a).unwrap());
        assert!(store.is_revoked(&a));
        assert!(matches!(
            store.add_peer(a, "nope"),
            Err(PeersError::Revoked)
        ));
    }

    #[test]
    fn revoked_wins_over_allowed_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let a = pk();
        let b = pk();
        let dashed_a = Fingerprint::from_pubkey(&a).as_str().to_string();
        let json = serde_json::json!({
            "peers": [
                { "node_id": dashed_a, "label": "Alice", "added_at": 1 },
                { "node_id": b.to_z32(), "label": "Bob", "added_at": 1 },
            ],
            // Hand-written in the dashed display form.
            "revoked": [dashed_a],
        });
        std::fs::write(dir.path().join(PEERS_FILE), json.to_string()).unwrap();

        let store = PeerStore::load_or_create_in(dir.path()).unwrap();
        assert!(!store.is_allowed(&a), "revoked peer must not be dialable");
        assert!(store.is_revoked(&a));
        assert!(store.is_allowed(&b));
        assert_eq!(store.list_peers().len(), 1);
        assert!(matches!(store.add_peer(a, "x"), Err(PeersError::Revoked)));
    }

    #[cfg(unix)]
    #[test]
    fn failed_persist_leaves_memory_unchanged() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let store = PeerStore::load_or_create_in(dir.path()).unwrap();
        let a = pk();
        let b = pk();
        store.add_peer(a, "Alice").unwrap();

        let set_mode = |mode| {
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap()
        };
        set_mode(0o555);
        let added = store.add_peer(b, "Bob");
        let revoked = store.revoke(&a);
        let removed = store.remove_peer(&a);
        set_mode(0o755);

        assert!(matches!(added, Err(PeersError::Io(_))), "{added:?}");
        assert!(!store.is_allowed(&b), "unpersisted add must not allow");
        assert!(matches!(revoked, Err(PeersError::Io(_))), "{revoked:?}");
        assert!(matches!(removed, Err(PeersError::Io(_))), "{removed:?}");
        assert!(store.is_allowed(&a));
        assert!(
            !store.is_revoked(&a),
            "memory matches disk after a failed revoke"
        );
        assert_eq!(store.list_peers().len(), 1);
    }
}
