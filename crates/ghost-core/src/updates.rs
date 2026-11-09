//! Verification de paquets de mise a jour : SHA-256 + signature Ed25519.
//! Rien n'est installe par ce module : il decide seulement si un paquet est digne de confiance.

use crate::{hashing::sha256_file, GhostError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::path::Path;

pub fn verify_package(path: &Path, expected_sha256: &str, signature_b64: &str, public_key_b64: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(GhostError::Integrity("empreinte SHA-256 du paquet différente de celle annoncée".into()));
    }
    let pk: [u8; 32] = STANDARD
        .decode(public_key_b64.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| GhostError::Crypto("clé publique invalide".into()))?;
    let sig: [u8; 64] = STANDARD
        .decode(signature_b64.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| GhostError::Crypto("signature invalide".into()))?;
    let key = VerifyingKey::from_bytes(&pk).map_err(|e| GhostError::Crypto(e.to_string()))?;
    // La signature porte sur l'empreinte (32 octets) : elle lie le paquet exact.
    let digest = hex::decode(&actual).map_err(|e| GhostError::Crypto(e.to_string()))?;
    key.verify(&digest, &Signature::from_bytes(&sig))
        .map_err(|_| GhostError::Integrity("signature de mise à jour invalide : paquet refusé".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, String, String, String) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("update.bin");
        std::fs::write(&p, b"contenu de la mise a jour").unwrap();
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let hash = sha256_file(&p).unwrap();
        let sig = sk.sign(&hex::decode(&hash).unwrap());
        (
            d,
            p,
            hash,
            STANDARD.encode(sig.to_bytes()),
            STANDARD.encode(sk.verifying_key().to_bytes()),
        )
    }

    #[test]
    fn valid_package_is_accepted() {
        let (_d, p, h, s, k) = fixture();
        verify_package(&p, &h, &s, &k).unwrap();
    }

    #[test]
    fn tampered_package_is_rejected() {
        let (_d, p, h, s, k) = fixture();
        std::fs::write(&p, b"contenu piege").unwrap();
        assert!(verify_package(&p, &h, &s, &k).is_err());
    }

    #[test]
    fn wrong_key_or_signature_is_rejected() {
        let (_d, p, h, s, _k) = fixture();
        let other = STANDARD.encode(SigningKey::from_bytes(&[9u8; 32]).verifying_key().to_bytes());
        assert!(verify_package(&p, &h, &s, &other).is_err());
        assert!(verify_package(&p, &h, "AAAA", &other).is_err());
        // empreinte annoncee falsifiee
        assert!(verify_package(&p, &"0".repeat(64), &s, &other).is_err());
    }
}
