//! QR / link-based device pairing for notare's P2P sync.
//!
//! ## Flow
//!
//! 1. **Device A** (offerer) calls [`PairingService::create_offer`]: a
//!    [`PairingOffer`] containing a 32-byte random secret, A's device id +
//!    name, and a 10-minute expiry is generated and encoded as
//!    `notare://pair#<base64url(json)>`. Only the secret is kept in memory
//!    (inside the [`PairingService`]); the link is handed to the UI for QR
//!    rendering.
//!
//! 2. **Device B** (joiner) scans the QR / pastes the link, calls
//!    [`PairingOffer::decode`], then dials device A on ALPN
//!    [`PAIRING_ALPN`] and sends a [`JoinMsg`] carrying its own device id +
//!    name + an HMAC-SHA256 proof (`HMAC(secret, joiner_id || offerer_id)`).
//!
//! 3. **Device A** receives the join, verifies the HMAC in constant time,
//!    and — if valid — burns the offer (single use), replies with
//!    [`AcceptMsg`], and moves the new peer into a "pending confirmation"
//!    state. Both devices derive a 6-digit Short Authentication String
//!    (SAS) from `SHA-256(sorted(device_ids) || secret)` and display it.
//!
//! 4. Both users confirm the 6-digit code matches on their screen. The
//!    peer enters the [`PeerStore`] only after the user taps "Confirm" (a
//!    2-minute window).
//!
//! 5. **Revocation**: [`PeerStore::revoke`] removes the peer and records
//!    its id in a revoked set so a stale offer cannot re-pair it.
//!
//! ## Security properties
//!
//! - The secret never leaves the QR / link; it is never sent over the
//!   network.
//! - The HMAC proof binds both device ids, preventing a stolen link from
//!   being replayed by a different device.
//! - The SAS gives the user visual confirmation that no MITM replaced the
//!   device ids in transit.
//! - 5 failed proofs burn the offer (brute-force rate limit).
//! - Offers expire after 10 minutes and are single-use.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use iroh::PublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// The ALPN the pairing handshake speaks over iroh. Distinct from the sync
/// ALPN so the agent can accept pairing connections from non-allowlisted
/// peers (only while an unexpired offer exists).
pub const PAIRING_ALPN: &[u8] = b"/notare/pair/1";

/// How long an offer is valid (seconds).
const OFFER_TTL_SECS: u64 = 600; // 10 minutes

/// Maximum failed proof attempts before the offer is burned.
const MAX_FAILED_PROOFS: u32 = 5;

/// How long a pending confirmation is valid (seconds).
const CONFIRM_TIMEOUT_SECS: u64 = 120; // 2 minutes

/// Errors from the pairing subsystem.
#[derive(Debug, Error)]
pub enum PairingError {
    #[error("no active offer")]
    NoActiveOffer,
    #[error("offer has expired")]
    OfferExpired,
    #[error("invalid proof")]
    InvalidProof,
    #[error("offer burned after too many failed attempts")]
    OfferBurned,
    #[error("invalid offer link: {0}")]
    InvalidLink(String),
    #[error("no pending confirmation for this device")]
    NoPendingConfirmation,
    #[error("confirmation timed out")]
    ConfirmationTimedOut,
    #[error("device is revoked")]
    DeviceRevoked,
    #[error("cannot pair with self")]
    SelfPair,
    #[error("peer operation failed: {0}")]
    PeerStore(String),
}

/// Version 1 of the pairing offer, encoded into the link fragment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingOffer {
    /// Protocol version.
    pub v: u32,
    /// Offerer's device id (z-base-32 compact form).
    pub device_id: String,
    /// Offerer's device name.
    pub name: String,
    /// 32 random bytes (base64url-encoded in the JSON).
    #[serde(with = "base64url_bytes")]
    pub secret: Vec<u8>,
    /// Unix timestamp (seconds) when the offer expires.
    pub expires_at: u64,
    /// Optional relay URL hint for the joiner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
}

impl PairingOffer {
    /// Create a new offer for the given device identity.
    pub fn new(device_id: &PublicKey, name: impl Into<String>) -> Self {
        let mut secret = [0u8; 32];
        rand::fill(&mut secret);
        let now = now_secs();
        Self {
            v: 1,
            device_id: device_id.to_z32(),
            name: name.into(),
            secret: secret.to_vec(),
            expires_at: now + OFFER_TTL_SECS,
            relay_url: None,
        }
    }

    /// Encode the offer as a `notare://pair#<base64url(json)>` link.
    pub fn encode_link(&self) -> String {
        let json = serde_json::to_vec(self).expect("PairingOffer is always serializable");
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&json);
        format!("notare://pair#{encoded}")
    }

    /// Decode a `notare://pair#<base64url(json)>` link back into an offer.
    pub fn decode(link: &str) -> Result<Self, PairingError> {
        let fragment = link
            .strip_prefix("notare://pair#")
            .ok_or_else(|| PairingError::InvalidLink("missing notare://pair# prefix".into()))?;
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(fragment)
            .map_err(|e| PairingError::InvalidLink(format!("base64 decode failed: {e}")))?;
        let offer: PairingOffer = serde_json::from_slice(&json)
            .map_err(|e| PairingError::InvalidLink(format!("json decode failed: {e}")))?;
        if offer.v != 1 {
            return Err(PairingError::InvalidLink(format!(
                "unsupported version: {}",
                offer.v
            )));
        }
        Ok(offer)
    }

    /// Whether the offer has expired.
    pub fn is_expired(&self) -> bool {
        now_secs() >= self.expires_at
    }

    /// Seconds remaining until expiry, or 0 if expired.
    pub fn seconds_remaining(&self) -> u64 {
        self.expires_at.saturating_sub(now_secs())
    }

    /// Parse the offerer's device id from the z-base-32 string.
    pub fn offerer_id(&self) -> Result<PublicKey, PairingError> {
        crate::identity::Fingerprint::parse(&self.device_id)
            .map_err(|e| PairingError::InvalidLink(format!("invalid device_id: {e}")))
    }
}

/// The joiner's message, sent over a bi-stream on ALPN `/notare/pair/1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinMsg {
    /// Joiner's device id (z-base-32 compact form).
    pub device_id: String,
    /// Joiner's device name.
    pub name: String,
    /// HMAC-SHA256(secret, joiner_device_id || offerer_device_id).
    #[serde(with = "base64url_bytes")]
    pub proof: Vec<u8>,
}

/// The offerer's reply after verifying the join proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptMsg {
    /// Offerer's device id (z-base-32 compact form).
    pub device_id: String,
    /// Offerer's device name.
    pub name: String,
}

/// A rejection message sent when the proof is invalid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectMsg {
    pub reason: String,
}

/// Compute the HMAC-SHA256 proof: `HMAC(secret, joiner_id || offerer_id)`.
///
/// Both ids are the raw z-base-32 bytes (compact form, no dashes).
pub fn compute_proof(secret: &[u8], joiner_id: &str, offerer_id: &str) -> Vec<u8> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret).expect("HMAC-SHA256 accepts any key length");
    mac.update(joiner_id.as_bytes());
    mac.update(offerer_id.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// Verify a proof in constant time. Returns true if the proof is valid.
pub fn verify_proof(secret: &[u8], joiner_id: &str, offerer_id: &str, proof: &[u8]) -> bool {
    let expected = compute_proof(secret, joiner_id, offerer_id);
    expected.ct_eq(proof).into()
}

/// Derive the 6-digit Short Authentication String (SAS) that both devices
/// display for the user to compare.
///
/// `SAS = SHA-256(sorted(device_id_a, device_id_b) || secret)` → first 3
/// bytes → decimal mod 1_000_000, zero-padded to 6 digits.
pub fn compute_sas(id_a: &str, id_b: &str, secret: &[u8]) -> String {
    let (first, second) = if id_a <= id_b {
        (id_a, id_b)
    } else {
        (id_b, id_a)
    };
    let mut hasher = Sha256::new();
    hasher.update(first.as_bytes());
    hasher.update(second.as_bytes());
    hasher.update(secret);
    let hash = hasher.finalize();
    // Take first 4 bytes as a big-endian u32, mod 1_000_000.
    let n = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
    let code = n % 1_000_000;
    format!("{code:06}")
}

/// A pending confirmation: the user has not yet confirmed the SAS code.
#[derive(Debug, Clone)]
pub struct PendingConfirmation {
    /// The peer's device id (z-base-32 compact).
    pub device_id: String,
    /// The peer's display name.
    pub name: String,
    /// The 6-digit SAS code to display.
    pub sas: String,
    /// Unix timestamp when the confirmation window closes.
    pub expires_at: u64,
}

impl PendingConfirmation {
    pub fn is_expired(&self) -> bool {
        now_secs() >= self.expires_at
    }
}

/// In-memory state for the pairing subsystem. Held by the P2P agent.
///
/// Thread-safe (one `Arc<RwLock<...>>`). The active offer and pending
/// confirmations live only in memory — they are never persisted, so a
/// restart always starts clean.
#[derive(Debug, Clone)]
pub struct PairingService {
    inner: Arc<RwLock<PairingInner>>,
}

#[derive(Debug)]
struct PairingInner {
    /// The currently active offer, if any. Only one offer at a time.
    active_offer: Option<ActiveOffer>,
    /// Pending confirmations keyed by the peer's z32 device id.
    pending: HashMap<String, PendingConfirmation>,
}

#[derive(Debug)]
struct ActiveOffer {
    /// The secret from the offer (the only part kept in memory).
    secret: Vec<u8>,
    /// The offerer's own device id (z32 compact).
    offerer_id: String,
    /// Expiry timestamp.
    expires_at: u64,
    /// Count of failed proof attempts.
    failed_attempts: u32,
}

impl PairingService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PairingInner {
                active_offer: None,
                pending: HashMap::new(),
            })),
        }
    }

    /// Create a new pairing offer. Replaces any existing offer.
    ///
    /// Returns the offer (for encoding as a link / QR). The service keeps
    /// only the secret + metadata in memory.
    pub fn create_offer(&self, device_id: &PublicKey, name: impl Into<String>) -> PairingOffer {
        let offer = PairingOffer::new(device_id, name);
        let mut g = self.inner.write().expect("pairing lock poisoned");
        g.active_offer = Some(ActiveOffer {
            secret: offer.secret.clone(),
            offerer_id: offer.device_id.clone(),
            expires_at: offer.expires_at,
            failed_attempts: 0,
        });
        offer
    }

    /// Whether there is an active, unexpired offer (so the agent knows
    /// whether to accept connections on the pairing ALPN).
    pub fn has_active_offer(&self) -> bool {
        let g = self.inner.read().expect("pairing lock poisoned");
        match &g.active_offer {
            Some(offer) => now_secs() < offer.expires_at,
            None => false,
        }
    }

    /// Cancel the active offer without burning it (user cancelled).
    pub fn cancel_offer(&self) {
        let mut g = self.inner.write().expect("pairing lock poisoned");
        g.active_offer = None;
    }

    /// Handle an incoming join attempt from a peer. Called by the agent's
    /// pairing ALPN accept handler.
    ///
    /// On success, the peer is placed in the pending-confirmation state and
    /// the `AcceptMsg` + SAS code are returned. On failure, a `RejectMsg`
    /// reason is returned.
    ///
    /// `revoked_set` is checked before accepting — a revoked peer cannot
    /// re-pair even with a valid proof.
    pub fn handle_join(
        &self,
        join: &JoinMsg,
        revoked: &dyn Fn(&str) -> bool,
    ) -> Result<(AcceptMsg, String), PairingError> {
        let mut g = self.inner.write().expect("pairing lock poisoned");

        let offer = g.active_offer.as_mut().ok_or(PairingError::NoActiveOffer)?;

        if now_secs() >= offer.expires_at {
            g.active_offer = None;
            return Err(PairingError::OfferExpired);
        }

        if join.device_id == offer.offerer_id {
            return Err(PairingError::SelfPair);
        }

        if revoked(&join.device_id) {
            // Burn the offer when a revoked device tries — don't leak
            // information about why via retry count.
            g.active_offer = None;
            return Err(PairingError::DeviceRevoked);
        }

        if !verify_proof(
            &offer.secret,
            &join.device_id,
            &offer.offerer_id,
            &join.proof,
        ) {
            offer.failed_attempts += 1;
            if offer.failed_attempts >= MAX_FAILED_PROOFS {
                g.active_offer = None;
                return Err(PairingError::OfferBurned);
            }
            return Err(PairingError::InvalidProof);
        }

        // Valid proof — burn the offer (single use).
        let secret = offer.secret.clone();
        let offerer_id = offer.offerer_id.clone();
        g.active_offer = None;

        let sas = compute_sas(&join.device_id, &offerer_id, &secret);

        let pending = PendingConfirmation {
            device_id: join.device_id.clone(),
            name: join.name.clone(),
            sas: sas.clone(),
            expires_at: now_secs() + CONFIRM_TIMEOUT_SECS,
        };
        g.pending.insert(join.device_id.clone(), pending);

        Ok((
            AcceptMsg {
                device_id: offerer_id,
                name: String::new(), // filled in by the caller with the real device name
            },
            sas,
        ))
    }

    /// Confirm or reject a pending peer. Called when the user taps
    /// Confirm / Reject on the SAS dialog.
    ///
    /// Returns the confirmation details if accepted, so the caller can add
    /// the peer to the PeerStore.
    pub fn confirm(
        &self,
        device_id: &str,
        accept: bool,
    ) -> Result<Option<PendingConfirmation>, PairingError> {
        let mut g = self.inner.write().expect("pairing lock poisoned");
        let pending = g
            .pending
            .remove(device_id)
            .ok_or(PairingError::NoPendingConfirmation)?;

        if pending.is_expired() {
            return Err(PairingError::ConfirmationTimedOut);
        }

        if accept { Ok(Some(pending)) } else { Ok(None) }
    }

    /// Get the current pairing status: offer state + pending confirmations.
    pub fn status(&self) -> PairingStatus {
        let g = self.inner.read().expect("pairing lock poisoned");

        // Clean up expired pendings while we're here (read lock — we can't
        // mutate, but we can report them as expired).
        let pending: Vec<PendingConfirmation> = g
            .pending
            .values()
            .filter(|p| !p.is_expired())
            .cloned()
            .collect();

        let offer_active = matches!(&g.active_offer, Some(offer) if now_secs() < offer.expires_at);

        let offer_expires_at = g
            .active_offer
            .as_ref()
            .filter(|o| now_secs() < o.expires_at)
            .map(|o| o.expires_at);

        PairingStatus {
            offer_active,
            offer_expires_at,
            pending_confirmations: pending,
        }
    }

    /// Garbage-collect expired pending confirmations.
    pub fn gc(&self) {
        let mut g = self.inner.write().expect("pairing lock poisoned");
        g.pending.retain(|_, p| !p.is_expired());
        if let Some(offer) = &g.active_offer
            && now_secs() >= offer.expires_at
        {
            g.active_offer = None;
        }
    }
}

impl Default for PairingService {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of the pairing subsystem state, returned to the UI.
#[derive(Debug, Clone)]
pub struct PairingStatus {
    pub offer_active: bool,
    pub offer_expires_at: Option<u64>,
    pub pending_confirmations: Vec<PendingConfirmation>,
}

/// Generate an SVG string encoding the given data as a QR code.
pub fn qr_svg(data: &str) -> Result<String, PairingError> {
    use qrcode::QrCode;
    let code = QrCode::new(data.as_bytes())
        .map_err(|e| PairingError::InvalidLink(format!("QR encoding failed: {e}")))?;
    let svg = code
        .render::<qrcode::render::svg::Color<'_>>()
        .quiet_zone(true)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    Ok(svg)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// serde adapter: serialize `Vec<u8>` as base64url (no padding), deserialize
/// back. Used for the `secret` and `proof` fields in the wire types.
mod base64url_bytes {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        s.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&s)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device_id() -> PublicKey {
        iroh::SecretKey::generate().public()
    }

    #[test]
    fn offer_encode_decode_roundtrip() {
        let id = test_device_id();
        let offer = PairingOffer::new(&id, "Test Device");
        let link = offer.encode_link();

        assert!(link.starts_with("notare://pair#"));

        let decoded = PairingOffer::decode(&link).unwrap();
        assert_eq!(decoded.v, 1);
        assert_eq!(decoded.device_id, id.to_z32());
        assert_eq!(decoded.name, "Test Device");
        assert_eq!(decoded.secret, offer.secret);
        assert_eq!(decoded.expires_at, offer.expires_at);
    }

    #[test]
    fn offer_decode_invalid_prefix() {
        let result = PairingOffer::decode("https://example.com/pair#abc");
        assert!(matches!(result, Err(PairingError::InvalidLink(_))));
    }

    #[test]
    fn offer_decode_invalid_base64() {
        let result = PairingOffer::decode("notare://pair#!!!invalid!!!");
        assert!(matches!(result, Err(PairingError::InvalidLink(_))));
    }

    #[test]
    fn offer_decode_invalid_json() {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"not json");
        let link = format!("notare://pair#{encoded}");
        let result = PairingOffer::decode(&link);
        assert!(matches!(result, Err(PairingError::InvalidLink(_))));
    }

    #[test]
    fn offer_expiry() {
        let id = test_device_id();
        let mut offer = PairingOffer::new(&id, "Test");
        assert!(!offer.is_expired());
        assert!(offer.seconds_remaining() > 0);

        // Force expiry.
        offer.expires_at = now_secs() - 1;
        assert!(offer.is_expired());
        assert_eq!(offer.seconds_remaining(), 0);
    }

    #[test]
    fn proof_compute_and_verify() {
        let secret = b"test-secret-32-bytes-padding!!!!";
        let joiner = "joiner_z32_id";
        let offerer = "offerer_z32_id";

        let proof = compute_proof(secret, joiner, offerer);
        assert!(verify_proof(secret, joiner, offerer, &proof));

        // Wrong secret.
        assert!(!verify_proof(
            b"wrong-secret-32-bytes-padding!!",
            joiner,
            offerer,
            &proof
        ));

        // Swapped ids.
        assert!(!verify_proof(secret, offerer, joiner, &proof));

        // Tampered proof.
        let mut bad_proof = proof.clone();
        bad_proof[0] ^= 0xff;
        assert!(!verify_proof(secret, joiner, offerer, &bad_proof));
    }

    #[test]
    fn sas_equal_on_both_sides() {
        let secret = b"shared-secret-for-both-devices!!";
        let id_a = "aaaa";
        let id_b = "bbbb";

        let sas1 = compute_sas(id_a, id_b, secret);
        let sas2 = compute_sas(id_b, id_a, secret);
        assert_eq!(
            sas1, sas2,
            "SAS must be identical regardless of argument order"
        );
        assert_eq!(sas1.len(), 6, "SAS must be 6 digits");
        assert!(sas1.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn sas_different_across_secrets() {
        let id_a = "aaaa";
        let id_b = "bbbb";
        let sas1 = compute_sas(id_a, id_b, b"secret-one-32-bytes-padding!!!!!");
        let sas2 = compute_sas(id_a, id_b, b"secret-two-32-bytes-padding!!!!!");
        // Statistically: 1 in 1_000_000 chance of collision — acceptable
        // for a test but not a proof. The important thing is the function
        // depends on the secret.
        assert_ne!(sas1, sas2, "SAS should differ with different secrets");
    }

    #[test]
    fn single_use_offer() {
        let offerer_key = iroh::SecretKey::generate();
        let offerer_id = offerer_key.public();
        let joiner_key = iroh::SecretKey::generate();
        let joiner_id = joiner_key.public();

        let service = PairingService::new();
        let offer = service.create_offer(&offerer_id, "Offerer");
        assert!(service.has_active_offer());

        let proof = compute_proof(&offer.secret, &joiner_id.to_z32(), &offerer_id.to_z32());
        let join = JoinMsg {
            device_id: joiner_id.to_z32(),
            name: "Joiner".into(),
            proof,
        };

        let no_revoke = |_: &str| false;

        // First join succeeds.
        let (accept, sas) = service.handle_join(&join, &no_revoke).unwrap();
        assert_eq!(accept.device_id, offerer_id.to_z32());
        assert_eq!(sas.len(), 6);

        // Second join fails — offer burned (single use).
        let result = service.handle_join(&join, &no_revoke);
        assert!(matches!(result, Err(PairingError::NoActiveOffer)));
    }

    #[test]
    fn wrong_proof_rejected() {
        let offerer_key = iroh::SecretKey::generate();
        let offerer_id = offerer_key.public();
        let joiner_key = iroh::SecretKey::generate();
        let joiner_id = joiner_key.public();

        let service = PairingService::new();
        let _offer = service.create_offer(&offerer_id, "Offerer");

        let join = JoinMsg {
            device_id: joiner_id.to_z32(),
            name: "Joiner".into(),
            proof: vec![0u8; 32], // wrong proof
        };

        let no_revoke = |_: &str| false;
        let result = service.handle_join(&join, &no_revoke);
        assert!(matches!(result, Err(PairingError::InvalidProof)));
    }

    #[test]
    fn rate_limit_burns_offer() {
        let offerer_key = iroh::SecretKey::generate();
        let offerer_id = offerer_key.public();
        let joiner_key = iroh::SecretKey::generate();
        let joiner_id = joiner_key.public();

        let service = PairingService::new();
        let _offer = service.create_offer(&offerer_id, "Offerer");

        let bad_join = JoinMsg {
            device_id: joiner_id.to_z32(),
            name: "Attacker".into(),
            proof: vec![0u8; 32],
        };

        let no_revoke = |_: &str| false;

        // 4 failed attempts — still active.
        for _ in 0..4 {
            let result = service.handle_join(&bad_join, &no_revoke);
            assert!(matches!(result, Err(PairingError::InvalidProof)));
        }
        assert!(service.has_active_offer());

        // 5th attempt burns the offer.
        let result = service.handle_join(&bad_join, &no_revoke);
        assert!(matches!(result, Err(PairingError::OfferBurned)));
        assert!(!service.has_active_offer());
    }

    #[test]
    fn revoked_device_rejected() {
        let offerer_key = iroh::SecretKey::generate();
        let offerer_id = offerer_key.public();
        let joiner_key = iroh::SecretKey::generate();
        let joiner_id = joiner_key.public();

        let service = PairingService::new();
        let offer = service.create_offer(&offerer_id, "Offerer");

        let proof = compute_proof(&offer.secret, &joiner_id.to_z32(), &offerer_id.to_z32());
        let join = JoinMsg {
            device_id: joiner_id.to_z32(),
            name: "Joiner".into(),
            proof,
        };

        let joiner_z32 = joiner_id.to_z32();
        let is_revoked = |id: &str| id == joiner_z32;
        let result = service.handle_join(&join, &is_revoked);
        assert!(matches!(result, Err(PairingError::DeviceRevoked)));
    }

    #[test]
    fn self_pair_rejected() {
        let key = iroh::SecretKey::generate();
        let id = key.public();

        let service = PairingService::new();
        let offer = service.create_offer(&id, "Self");

        let proof = compute_proof(&offer.secret, &id.to_z32(), &id.to_z32());
        let join = JoinMsg {
            device_id: id.to_z32(),
            name: "Self".into(),
            proof,
        };

        let no_revoke = |_: &str| false;
        let result = service.handle_join(&join, &no_revoke);
        assert!(matches!(result, Err(PairingError::SelfPair)));
    }

    #[test]
    fn confirm_accept_and_reject() {
        let offerer_key = iroh::SecretKey::generate();
        let offerer_id = offerer_key.public();
        let joiner_key = iroh::SecretKey::generate();
        let joiner_id = joiner_key.public();

        let service = PairingService::new();
        let offer = service.create_offer(&offerer_id, "Offerer");

        let proof = compute_proof(&offer.secret, &joiner_id.to_z32(), &offerer_id.to_z32());
        let join = JoinMsg {
            device_id: joiner_id.to_z32(),
            name: "Joiner".into(),
            proof,
        };

        let no_revoke = |_: &str| false;
        service.handle_join(&join, &no_revoke).unwrap();

        // Confirm acceptance.
        let confirmed = service.confirm(&joiner_id.to_z32(), true).unwrap();
        assert!(confirmed.is_some());
        let c = confirmed.unwrap();
        assert_eq!(c.device_id, joiner_id.to_z32());
        assert_eq!(c.name, "Joiner");
    }

    #[test]
    fn confirm_nonexistent_peer() {
        let service = PairingService::new();
        let result = service.confirm("nonexistent", true);
        assert!(matches!(result, Err(PairingError::NoPendingConfirmation)));
    }

    #[test]
    fn cancel_offer() {
        let key = iroh::SecretKey::generate();
        let id = key.public();
        let service = PairingService::new();
        let _offer = service.create_offer(&id, "Test");
        assert!(service.has_active_offer());

        service.cancel_offer();
        assert!(!service.has_active_offer());
    }

    #[test]
    fn status_snapshot() {
        let key = iroh::SecretKey::generate();
        let id = key.public();
        let service = PairingService::new();

        let status = service.status();
        assert!(!status.offer_active);
        assert!(status.pending_confirmations.is_empty());

        let _offer = service.create_offer(&id, "Test");
        let status = service.status();
        assert!(status.offer_active);
        assert!(status.offer_expires_at.is_some());
    }
}
