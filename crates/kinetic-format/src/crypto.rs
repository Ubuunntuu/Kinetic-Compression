use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::Argon2;
use crate::FormatError;

pub const SALT_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;

pub fn derive_key_argon2id(password: &str, salt: &[u8; SALT_LEN]) -> Result<[u8; 32], FormatError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| FormatError::CryptoError(format!("Argon2id KDF failed: {}", e)))?;
    Ok(key)
}

pub fn generate_salt_and_nonce(seed_material: &[u8]) -> ([u8; SALT_LEN], [u8; NONCE_LEN]) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"KINETIC_V1_CSPRNG_SALT_AND_NONCE");
    hasher.update(&nanos.to_le_bytes());
    hasher.update(seed_material);

    let mut reader = hasher.finalize_xof();
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    reader.fill(&mut salt);
    reader.fill(&mut nonce);
    (salt, nonce)
}

fn block_nonce(base_nonce: &[u8; NONCE_LEN], block_id: u32) -> [u8; NONCE_LEN] {
    let mut n = *base_nonce;
    let id_bytes = block_id.to_le_bytes();
    for i in 0..4 {
        n[i] ^= id_bytes[i];
    }
    n
}

pub fn encrypt_block_aes256_gcm(
    key_bytes: &[u8; 32],
    base_nonce: &[u8; NONCE_LEN],
    block_id: u32,
    plaintext: &[u8],
) -> Result<Vec<u8>, FormatError> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let derived_nonce = block_nonce(base_nonce, block_id);
    let nonce = Nonce::from_slice(&derived_nonce);

    cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| FormatError::CryptoError("AES-256-GCM encryption failed".to_string()))
}

pub fn decrypt_block_aes256_gcm(
    key_bytes: &[u8; 32],
    base_nonce: &[u8; NONCE_LEN],
    block_id: u32,
    ciphertext: &[u8],
) -> Result<Vec<u8>, FormatError> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let derived_nonce = block_nonce(base_nonce, block_id);
    let nonce = Nonce::from_slice(&derived_nonce);

    cipher.decrypt(nonce, ciphertext).map_err(|_| {
        FormatError::CryptoError(
            "AES-256-GCM authentication failed: invalid password or tampered archive".to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes256_gcm_argon2id_roundtrip() {
        let (salt, base_nonce) = generate_salt_and_nonce(b"test_seed");
        let key = derive_key_argon2id("KineticMasterPassword!2026", &salt).unwrap();
        let secret = b"Confidential .kinetic archive payload block";

        let enc = encrypt_block_aes256_gcm(&key, &base_nonce, 7, secret).unwrap();
        assert_ne!(enc, secret);

        let dec = decrypt_block_aes256_gcm(&key, &base_nonce, 7, &enc).unwrap();
        assert_eq!(dec, secret);

        assert!(decrypt_block_aes256_gcm(&key, &base_nonce, 8, &enc).is_err());
    }
}