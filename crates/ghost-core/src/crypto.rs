//! Chiffrement AES-256-GCM par blocs (1 Mio) avec cle derivee par Argon2id.
//!
//! Format : MAGIC(9) | m_kib(4) | t(4) | p(4) | sel(16) | prefixe_nonce(8) | blocs…
//! Bloc   : longueur(4, BE) | chiffre+tag. Nonce = prefixe || compteur(4).
//! Donnees authentifiees (AAD) = en-tete || compteur || drapeau « dernier bloc »,
//! ce qui detecte reordonnancement, troncature et alteration.
//!
//! Les donnees claires sont emises bloc par bloc : l'appelant doit ecrire dans un
//! fichier temporaire et ne le valider que si `decrypt_stream` retourne `Ok`.

use crate::{GhostError, Result};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::{rngs::OsRng, RngCore};
use std::io::{Read, Write};
use zeroize::Zeroizing;

const MAGIC: &[u8; 9] = b"GHOSTENC1";
const HEADER_LEN: usize = 9 + 4 + 4 + 4 + 16 + 8;
pub const CHUNK: usize = 1 << 20;
const TAG: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl Default for KdfParams {
    /// 64 Mio, 3 iterations, 1 voie : recommandation OWASP minimale pour Argon2id.
    fn default() -> Self {
        KdfParams { m_kib: 64 * 1024, t: 3, p: 1 }
    }
}

impl KdfParams {
    /// Parametres reduits, reserves aux tests.
    pub const fn light() -> Self {
        KdfParams { m_kib: 8, t: 1, p: 1 }
    }
    fn validate(&self) -> Result<()> {
        if self.m_kib < 8 || self.m_kib > 1 << 20 || self.t == 0 || self.t > 10 || self.p == 0 || self.p > 8 {
            return Err(GhostError::Crypto("paramètres de dérivation invalides".into()));
        }
        Ok(())
    }
}

pub fn derive_key(password: &str, salt: &[u8], params: KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    params.validate()?;
    let p = Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|e| GhostError::Crypto(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, &mut *out)
        .map_err(|e| GhostError::Crypto(e.to_string()))?;
    Ok(out)
}

fn nonce(prefix: &[u8], counter: u32) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(prefix);
    n[8..].copy_from_slice(&counter.to_be_bytes());
    n
}

fn aad(header: &[u8], counter: u32, last: bool) -> Vec<u8> {
    let mut a = Vec::with_capacity(header.len() + 5);
    a.extend_from_slice(header);
    a.extend_from_slice(&counter.to_be_bytes());
    a.push(last as u8);
    a
}

fn read_chunk<R: Read>(r: &mut R) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; CHUNK];
    let mut filled = 0;
    while filled < CHUNK {
        let n = r.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    buf.truncate(filled);
    Ok(buf)
}

pub fn encrypt_stream<R: Read, W: Write>(
    mut r: R,
    mut w: W,
    password: &str,
    params: KdfParams,
) -> Result<()> {
    if password.is_empty() {
        return Err(GhostError::Crypto("mot de passe vide".into()));
    }
    let mut salt = [0u8; 16];
    let mut prefix = [0u8; 8];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut prefix);
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&params.m_kib.to_be_bytes());
    header.extend_from_slice(&params.t.to_be_bytes());
    header.extend_from_slice(&params.p.to_be_bytes());
    header.extend_from_slice(&salt);
    header.extend_from_slice(&prefix);
    w.write_all(&header)?;

    let key = derive_key(password, &salt, params)?;
    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|e| GhostError::Crypto(e.to_string()))?;
    let mut counter: u32 = 0;
    let mut cur = read_chunk(&mut r)?;
    loop {
        let next = read_chunk(&mut r)?;
        let last = next.is_empty();
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&nonce(&prefix, counter)),
                Payload { msg: &cur, aad: &aad(&header, counter, last) },
            )
            .map_err(|_| GhostError::Crypto("échec du chiffrement".into()))?;
        w.write_all(&(ct.len() as u32).to_be_bytes())?;
        w.write_all(&ct)?;
        if last {
            break;
        }
        counter = counter
            .checked_add(1)
            .ok_or_else(|| GhostError::Crypto("fichier trop volumineux".into()))?;
        cur = next;
    }
    w.flush()?;
    Ok(())
}

pub fn decrypt_stream<R: Read, W: Write>(mut r: R, mut w: W, password: &str) -> Result<()> {
    let mut header = [0u8; HEADER_LEN];
    r.read_exact(&mut header)
        .map_err(|_| GhostError::Crypto("fichier chiffré invalide ou tronqué".into()))?;
    if &header[..9] != MAGIC {
        return Err(GhostError::Crypto("format non reconnu".into()));
    }
    let u32at = |o: usize| u32::from_be_bytes(header[o..o + 4].try_into().unwrap());
    let params = KdfParams { m_kib: u32at(9), t: u32at(13), p: u32at(17) };
    let salt = &header[21..37];
    let prefix = &header[37..45];
    let key = derive_key(password, salt, params)?;
    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|e| GhostError::Crypto(e.to_string()))?;
    let mut counter: u32 = 0;
    loop {
        let mut len_buf = [0u8; 4];
        r.read_exact(&mut len_buf)
            .map_err(|_| GhostError::Crypto("fichier tronqué".into()))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if !(TAG..=CHUNK + TAG).contains(&len) {
            return Err(GhostError::Crypto("bloc de taille invalide".into()));
        }
        let mut ct = vec![0u8; len];
        r.read_exact(&mut ct)
            .map_err(|_| GhostError::Crypto("fichier tronqué".into()))?;
        let n = nonce(prefix, counter);
        let try_dec = |last: bool| {
            cipher.decrypt(Nonce::from_slice(&n), Payload { msg: &ct, aad: &aad(&header, counter, last) })
        };
        if let Ok(pt) = try_dec(false) {
            w.write_all(&pt)?;
            counter = counter
                .checked_add(1)
                .ok_or_else(|| GhostError::Crypto("fichier trop volumineux".into()))?;
            continue;
        }
        match try_dec(true) {
            Ok(pt) => {
                w.write_all(&pt)?;
                let mut extra = [0u8; 1];
                if r.read(&mut extra)? != 0 {
                    return Err(GhostError::Crypto("données inattendues après le dernier bloc".into()));
                }
                w.flush()?;
                return Ok(());
            }
            Err(_) => {
                return Err(GhostError::Crypto(
                    "mot de passe incorrect ou fichier altéré".into(),
                ))
            }
        }
    }
}

pub fn encrypt_bytes(data: &[u8], password: &str, params: KdfParams) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    encrypt_stream(data, &mut out, password, params)?;
    Ok(out)
}

pub fn decrypt_bytes(data: &[u8], password: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    decrypt_stream(data, &mut out, password)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const L: KdfParams = KdfParams::light();

    #[test]
    fn roundtrip_small_empty_and_multichunk() {
        for size in [0usize, 1, 100, CHUNK, CHUNK + 1, 2 * CHUNK + 17] {
            let data: Vec<u8> = (0..size).map(|i| (i * 31 % 251) as u8).collect();
            let enc = encrypt_bytes(&data, "mdp", L).unwrap();
            assert_eq!(decrypt_bytes(&enc, "mdp").unwrap(), data, "taille {size}");
        }
    }

    #[test]
    fn wrong_password_fails() {
        let enc = encrypt_bytes(b"secret", "bon", L).unwrap();
        assert!(decrypt_bytes(&enc, "mauvais").is_err());
    }

    #[test]
    fn tampering_is_detected() {
        let mut enc = encrypt_bytes(&vec![7u8; 5000], "mdp", L).unwrap();
        let i = enc.len() - 20;
        enc[i] ^= 1;
        assert!(decrypt_bytes(&enc, "mdp").is_err());
    }

    #[test]
    fn header_tampering_is_detected() {
        let mut enc = encrypt_bytes(b"abc", "mdp", L).unwrap();
        enc[13] ^= 0x01; // parametre t
        assert!(decrypt_bytes(&enc, "mdp").is_err());
    }

    #[test]
    fn truncation_is_detected() {
        let data = vec![1u8; 2 * CHUNK + 5];
        let enc = encrypt_bytes(&data, "mdp", L).unwrap();
        // retire le dernier bloc entier
        let last_len = 4 + 5 + TAG;
        let cut = &enc[..enc.len() - last_len];
        assert!(decrypt_bytes(cut, "mdp").is_err());
        assert!(decrypt_bytes(&enc[..enc.len() - 1], "mdp").is_err());
    }

    #[test]
    fn trailing_data_is_rejected() {
        let mut enc = encrypt_bytes(b"abc", "mdp", L).unwrap();
        enc.push(0);
        assert!(decrypt_bytes(&enc, "mdp").is_err());
    }

    #[test]
    fn ciphertexts_differ_each_time() {
        let a = encrypt_bytes(b"same", "mdp", L).unwrap();
        let b = encrypt_bytes(b"same", "mdp", L).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn empty_password_and_hostile_params_rejected() {
        assert!(encrypt_bytes(b"x", "", L).is_err());
        let mut enc = encrypt_bytes(b"x", "mdp", L).unwrap();
        enc[9..13].copy_from_slice(&u32::MAX.to_be_bytes()); // m_kib gigantesque
        assert!(decrypt_bytes(&enc, "mdp").is_err());
    }

    #[test]
    fn default_params_work() {
        let enc = encrypt_bytes(b"x", "mdp", KdfParams::default()).unwrap();
        assert_eq!(decrypt_bytes(&enc, "mdp").unwrap(), b"x");
    }
}
