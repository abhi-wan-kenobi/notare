use base64::Engine;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

/// RFC 7636 S256 code challenge from a verifier.
pub fn pkce_challenge_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Generates a PKCE verifier + challenge pair.
pub fn generate_pkce() -> PkcePair {
    // Two v4 UUIDs = 64 hex chars, ~244 bits of entropy, all within the
    // allowed PKCE verifier charset.
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = pkce_challenge_s256(&verifier);
    PkcePair {
        verifier,
        challenge,
    }
}

/// Verifies that a PKCE verifier matches the given challenge.
pub fn verify_pkce(verifier: &str, challenge: &str) -> bool {
    pkce_challenge_s256(verifier) == challenge
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636_appendix_b() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge_s256(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert!(verify_pkce(
            verifier,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        ));
        assert!(!verify_pkce(verifier, "invalid_challenge"));
    }

    #[test]
    fn generated_pkce_is_valid() {
        let pkce = generate_pkce();
        assert_eq!(pkce.verifier.len(), 64);
        assert!(pkce.verifier.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(pkce.challenge, pkce_challenge_s256(&pkce.verifier));
        assert!(verify_pkce(&pkce.verifier, &pkce.challenge));
        assert_ne!(generate_pkce().verifier, pkce.verifier);
    }
}
