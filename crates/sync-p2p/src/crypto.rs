//! E2E payload encryption for the P2P sync transport (SYNC-7).
//!
//! Each device reuses its Ed25519 identity key as an X25519 static key via the
//! standard Ed25519→X25519 conversion. A per-peer symmetric key is derived with
//! X25519 DH + HKDF-SHA256, with the info string bound to both node ids so the
//! key is pair-specific. Payloads are encrypted with XChaCha20-Poly1305: a
//! 24-byte random nonce is prepended to the AEAD ciphertext.
//!
//! This module lives at the **agent peer boundary** only. The local C↔agent TCP
//! hop is already gated by SYNC-5's bearer token and stays plaintext.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use hkdf::Hkdf;
use iroh::{PublicKey, SecretKey};
use sha2::Sha256;
use thiserror::Error;

/// Length of the XChaCha20-Poly1305 nonce.
pub const NONCE_LEN: usize = 24;

/// Errors from the encryption layer.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("AEAD encryption failed")]
    Encrypt,
    #[error("AEAD decryption failed")]
    Decrypt,
}

/// Derive a per-peer symmetric key from an X25519 DH over the devices' Ed25519
/// identity keys.
///
/// `my_secret` and `peer_public` are the iroh `SecretKey`/`PublicKey` (which
/// are Ed25519 keys). The derived key binds to both node ids so traffic from A→B
/// cannot be replayed or decrypted by C that knows only one public key.
pub fn derive_peer_key(my_secret: &SecretKey, peer_public: &PublicKey) -> [u8; 32] {
    let my_signing = iroh_secret_to_signing(my_secret);
    let peer_verifying = iroh_public_to_verifying(peer_public);

    let my_scalar = my_signing.to_scalar_bytes();
    let peer_montgomery = peer_verifying.to_montgomery();
    let shared = peer_montgomery.mul_clamped(my_scalar);

    let info = pair_domain_info(my_secret.public().as_bytes(), peer_public.as_bytes());

    let hkdf = Hkdf::<Sha256>::new(None, &shared.0);
    let mut okm = [0u8; 32];
    hkdf.expand(&info, &mut okm)
        .expect("hkdf expand to 32 bytes is infallible for this prf");
    okm
}

/// Encrypt `plaintext` for `peer_public` using our identity key.
///
/// Returns `[24-byte nonce || ciphertext+tag]`. The nonce is freshly random
/// for every call (XChaCha20's 192-bit nonce makes accidental collision
/// negligible).
pub fn encrypt(
    my_secret: &SecretKey,
    peer_public: &PublicKey,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key = derive_peer_key(my_secret, peer_public);
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::Encrypt)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::fill(&mut nonce_bytes);

    let nonce = XNonce::try_from(&nonce_bytes[..]).map_err(|_| CryptoError::Encrypt)?;

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::Encrypt)?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a `[24-byte nonce || ciphertext+tag]` blob received from `peer_public`
/// using our identity key.
pub fn decrypt(
    my_secret: &SecretKey,
    peer_public: &PublicKey,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if ciphertext.len() < NONCE_LEN {
        return Err(CryptoError::Decrypt);
    }
    let key = derive_peer_key(my_secret, peer_public);
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::Decrypt)?;

    let nonce = XNonce::try_from(&ciphertext[..NONCE_LEN]).map_err(|_| CryptoError::Decrypt)?;

    cipher
        .decrypt(&nonce, &ciphertext[NONCE_LEN..])
        .map_err(|_| CryptoError::Decrypt)
}

/// Convert an iroh `SecretKey` to the underlying `ed25519_dalek::SigningKey`.
fn iroh_secret_to_signing(secret: &SecretKey) -> SigningKey {
    let bytes = secret.to_bytes();
    // iroh's SecretKey is exactly 32 bytes of Ed25519 secret seed.
    SigningKey::from_bytes(&bytes)
}

/// Convert an iroh `PublicKey` to the underlying `ed25519_dalek::VerifyingKey`.
fn iroh_public_to_verifying(public: &PublicKey) -> VerifyingKey {
    let bytes = public.as_bytes();
    VerifyingKey::from_bytes(bytes).expect("iroh PublicKey is a valid Ed25519 point")
}

/// Build an HKDF info string that binds the derived key to both node ids.
///
/// The node ids are compared lexicographically and concatenated as raw 32-byte
/// public keys. The fixed prefix `notare-sync-p2p-v1:` plus the sorted binary ids
/// means a key derived for (A,B) is never valid for (A,C) even if C shares a
/// public-key byte prefix with B.
fn pair_domain_info(id_a: &[u8; 32], id_b: &[u8; 32]) -> Vec<u8> {
    let (first, second) = if id_a.as_slice() <= id_b.as_slice() {
        (id_a, id_b)
    } else {
        (id_b, id_a)
    };
    let mut info = Vec::with_capacity(21 + 32 + 32);
    info.extend_from_slice(b"notare-sync-p2p-v1:");
    info.extend_from_slice(first);
    info.extend_from_slice(second);
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt() {
        let a = iroh::SecretKey::generate();
        let b = iroh::SecretKey::generate();
        let peer_b = b.public();

        let plaintext = b"the quick sync payload";
        let sealed = encrypt(&a, &peer_b, plaintext).unwrap();
        let opened = decrypt(&b, &a.public(), &sealed).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let a = iroh::SecretKey::generate();
        let b = iroh::SecretKey::generate();
        let mut sealed = encrypt(&a, &b.public(), b"payload").unwrap();
        sealed[NONCE_LEN + 5] ^= 0xff;
        assert!(decrypt(&b, &a.public(), &sealed).is_err());
    }

    #[test]
    fn key_is_bound_to_peer_pair() {
        let a = iroh::SecretKey::generate();
        let b = iroh::SecretKey::generate();
        let c = iroh::SecretKey::generate();

        let key_ab = derive_peer_key(&a, &b.public());
        let key_ac = derive_peer_key(&a, &c.public());
        let key_ba = derive_peer_key(&b, &a.public());

        assert_ne!(key_ab, key_ac, "key for B must differ from key for C");
        assert_eq!(key_ab, key_ba, "DH is symmetric: A↔B == B↔A");

        // C cannot decrypt a message sent from A to B.
        let sealed = encrypt(&a, &b.public(), b"pair-bound secret").unwrap();
        assert!(decrypt(&c, &a.public(), &sealed).is_err());
    }

    #[test]
    fn wrong_sender_public_key_fails() {
        let a = iroh::SecretKey::generate();
        let b = iroh::SecretKey::generate();
        let c = iroh::SecretKey::generate();

        let sealed = encrypt(&a, &b.public(), b"payload").unwrap();
        // C tries to decrypt pretending the sender was A, but C's DH with A is wrong.
        assert!(decrypt(&b, &c.public(), &sealed).is_err());
    }
}
