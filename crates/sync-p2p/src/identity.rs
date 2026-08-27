//! Device identity for notare's P2P sync.
//!
//! Each device has a persistent **Ed25519** keypair. The public key *is* the
//! device's identity, and it is also iroh's [`EndpointId`] / [`PublicKey`]:
//! iroh's `SecretKey` is itself an Ed25519 key, so we reuse it directly rather
//! than inventing a second identity layer. One key, one identity, one peer id.
//!
//! The keypair is generated on first use and stored under the app data dir in a
//! `sync/` subdir (`<data_dir>/notare/sync/device.key`), with 0600 permissions
//! on unix. The on-disk format is the raw 32-byte secret key (`SecretKey::to_bytes`).
//!
//! The public key is surfaced two ways:
//! - [`Identity::id`] — the [`PublicKey`] / [`EndpointId`] (the binary identity
//!   used by iroh to address and authenticate a peer).
//! - [`Identity::fingerprint`] — a human-readable, grouped base32 string
//!   (Syncthing-style dashed blocks) for display and manual pairing. It
//!   round-trips through [`Fingerprint::parse`] back to the [`PublicKey`].

use std::path::{Path, PathBuf};

use iroh::{PublicKey, SecretKey};
use thiserror::Error;

/// The app name used under the platform data dir (matches notare's existing
/// `dirs::data_dir().join("notare")` convention — see `crates/storage`).
pub(crate) const APP_DIR: &str = "notare";
/// Subdir under the app dir reserved for P2P sync state (key + allowlist).
pub(crate) const SYNC_SUBDIR: &str = "sync";
const KEY_FILE: &str = "device.key";

/// Errors from loading or creating a device identity.
#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("no platform data directory available")]
    NoDataDir,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("stored device key is invalid")]
    InvalidKey,
}

/// A device's persistent P2P identity: an Ed25519 keypair whose public key is
/// the device id (and iroh `EndpointId`).
#[derive(Debug, Clone)]
pub struct Identity {
    secret: SecretKey,
}

impl Identity {
    /// The device id / iroh [`PublicKey`] / [`EndpointId`].
    pub fn id(&self) -> PublicKey {
        self.secret.public()
    }

    /// The underlying iroh [`SecretKey`] (used to construct the iroh endpoint).
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret
    }

    /// A human-readable fingerprint of the device id: base32 grouped into
    /// dashed blocks (Syncthing-style).
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_pubkey(&self.id())
    }

    /// Load the identity from `<data_dir>/notare/sync/device.key`, generating
    /// and persisting a fresh keypair on first use.
    pub fn load_or_create() -> Result<Self, IdentityError> {
        let dir = sync_dir().ok_or(IdentityError::NoDataDir)?;
        Self::load_or_create_in(&dir)
    }

    /// Load-or-create rooted at an explicit dir (for tests / non-standard app dirs).
    pub fn load_or_create_in(dir: &Path) -> Result<Self, IdentityError> {
        std::fs::create_dir_all(dir)?;
        let key_path = dir.join(KEY_FILE);
        let secret = match std::fs::read(&key_path) {
            Ok(bytes) => {
                let arr: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                    // Wrong length — refuse to silently overwrite a corrupt key;
                    // surface it so the operator decides.
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "device key is not 32 bytes")
                })?;
                SecretKey::from_bytes(&arr)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let secret = SecretKey::generate();
                let bytes = secret.to_bytes();
                atomic_write(&key_path, &bytes)?;
                restrict_perms(&key_path);
                secret
            }
            Err(e) => return Err(IdentityError::Io(e)),
        };
        Ok(Self { secret })
    }
}

/// The platform-standard app data dir + `notare/sync/`.
pub(crate) fn sync_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP_DIR).join(SYNC_SUBDIR))
}

/// Write `bytes` to `path` via a temp file + rename so a partial key never
/// replaces a good one (and so a crash mid-write leaves the old key intact).
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = tempfile_in(dir)?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// Build a NamedTempFile equivalent without pulling in `tempfile` as a non-dev
// dep: create a sibling file with a `.tmp` suffix, returned as a path. Renamed
// atomically by the caller. Removed if the caller abandons (best-effort).
fn tempfile_in(dir: &Path) -> std::io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let p = dir.join(format!(".{KEY_FILE}.{pid}.{n}.tmp"));
    // Clean up any stale tmp from a prior crash (best-effort, ignore errors).
    let _ = std::fs::remove_file(&p);
    Ok(p)
}

/// Restrict the key file to owner-only on unix (0600). No-op elsewhere.
#[cfg(unix)]
fn restrict_perms(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn restrict_perms(_path: &Path) {}

impl Identity {
    /// Construct a fresh ephemeral identity for tests (not persisted).
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            secret: SecretKey::generate(),
        }
    }
}

/// A human-readable fingerprint of a device id: z-base-32 (iroh's native
/// `PublicKey` encoding) grouped into dashed blocks for readability.
///
/// The canonical, parseable form is the *ungrouped* z-base-32 string — the
/// dashes are display sugar. [`Fingerprint::parse`] accepts both grouped and
/// ungrouped forms (it strips dashes and whitespace before decoding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    grouped: String,
}

impl Fingerprint {
    /// Build a grouped fingerprint from a [`PublicKey`].
    pub fn from_pubkey(pk: &PublicKey) -> Self {
        let z = pk.to_z32();
        // z-base-32 of a 32-byte key yields 52 chars. Group into blocks of 4
        // (Syncthing-style: short, scannable groups). 52 / 4 = 13 blocks.
        let grouped = z
            .as_bytes()
            .chunks(4)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("-");
        Self { grouped }
    }

    /// The dashed, display form.
    pub fn as_str(&self) -> &str {
        &self.grouped
    }

    /// The compact (ungrouped) form — what actually round-trips through iroh.
    pub fn compact(&self) -> String {
        self.grouped.chars().filter(|c| *c != '-').collect()
    }

    /// Parse a fingerprint (grouped or compact) back into a [`PublicKey`].
    pub fn parse(s: &str) -> Result<PublicKey, FingerprintError> {
        let compact: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
        PublicKey::from_z32(&compact).map_err(FingerprintError::from)
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.grouped)
    }
}

/// Errors from parsing a fingerprint.
#[derive(Debug, Error)]
pub enum FingerprintError {
    #[error("invalid fingerprint")]
    Invalid,
}

impl From<iroh::KeyParsingError> for FingerprintError {
    fn from(_: iroh::KeyParsingError) -> Self {
        FingerprintError::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_round_trips() {
        let sk = SecretKey::generate();
        let pk = sk.public();
        let fp = Fingerprint::from_pubkey(&pk);
        // Display is dashed; compact drops the dashes.
        assert!(fp.as_str().contains('-'));
        let back = Fingerprint::parse(fp.as_str()).unwrap();
        assert_eq!(back, pk, "grouped fingerprint round-trips");
        let back2 = Fingerprint::parse(&fp.compact()).unwrap();
        assert_eq!(back2, pk, "compact fingerprint round-trips");
    }

    #[test]
    fn fingerprint_parse_rejects_garbage() {
        assert!(Fingerprint::parse("not-a-real-fingerprint!!").is_err());
        assert!(Fingerprint::parse("").is_err());
    }

    #[test]
    fn identity_load_or_create_persists_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().to_path_buf();

        let id1 = Identity::load_or_create_in(&dir).unwrap();
        let pk1 = id1.id();
        let fp1 = id1.fingerprint().compact();

        // A second load must read the *same* key, not generate a new one.
        let id2 = Identity::load_or_create_in(&dir).unwrap();
        assert_eq!(id2.id(), pk1, "persisted key survives reload");
        assert_eq!(id2.fingerprint().compact(), fp1);

        // The key file exists and is 32 bytes.
        let bytes = std::fs::read(dir.join(KEY_FILE)).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[cfg(unix)]
    #[test]
    fn identity_key_file_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let _id = Identity::load_or_create_in(dir.path()).unwrap();
        let meta = std::fs::metadata(dir.path().join(KEY_FILE)).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "device key file must be owner-only");
    }
}
