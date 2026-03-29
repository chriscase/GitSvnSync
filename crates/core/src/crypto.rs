//! Cryptographic utilities for password hashing and credential encryption.
//!
//! Uses bcrypt for password hashing and AES-256-GCM for symmetric encryption
//! of stored credentials (SVN passwords, tokens, etc.).

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

use crate::db::Database;
use crate::errors::DatabaseError;

/// Errors from cryptographic operations.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("bcrypt error: {0}")]
    BcryptError(String),

    #[error("encryption error: {0}")]
    EncryptionError(String),

    #[error("decryption error: {0}")]
    DecryptionError(String),

    #[error("invalid key length")]
    InvalidKeyLength,

    #[error("base64 decode error: {0}")]
    Base64Error(String),

    #[error("database error: {0}")]
    DatabaseError(#[from] DatabaseError),
}

/// Encrypt a credential value using AES-256-GCM.
/// Returns `(base64_ciphertext, base64_nonce)`.
pub fn encrypt_credential(plaintext: &str, key: &[u8; 32]) -> Result<(String, String), CryptoError> {
    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| CryptoError::EncryptionError(e.to_string()))?;

    Ok((
        base64_encode(&ciphertext),
        base64_encode(&nonce_bytes),
    ))
}

/// Decrypt a credential value.
pub fn decrypt_credential(
    ciphertext_b64: &str,
    nonce_b64: &str,
    key: &[u8; 32],
) -> Result<String, CryptoError> {
    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);

    let ciphertext = base64_decode(ciphertext_b64)?;
    let nonce_bytes = base64_decode(nonce_b64)?;

    if nonce_bytes.len() != 12 {
        return Err(CryptoError::DecryptionError("invalid nonce length".into()));
    }

    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| CryptoError::DecryptionError(e.to_string()))?;

    String::from_utf8(plaintext)
        .map_err(|e| CryptoError::DecryptionError(format!("invalid UTF-8: {}", e)))
}

/// Hash a password using bcrypt.
pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| CryptoError::BcryptError(e.to_string()))
}

/// Verify a password against a bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, CryptoError> {
    bcrypt::verify(password, hash).map_err(|e| CryptoError::BcryptError(e.to_string()))
}

/// Get or generate the server encryption key.
///
/// First checks the `REPOSYNC_ENCRYPTION_KEY` environment variable (hex-encoded 32 bytes).
/// If not set, reads from `kv_state` table. If not stored yet, generates a new random
/// key and persists it.
pub fn get_or_create_encryption_key(db: &Database) -> Result<[u8; 32], CryptoError> {
    const KV_KEY: &str = "encryption_key";

    // 1. Check env var
    if let Ok(hex_key) = std::env::var("REPOSYNC_ENCRYPTION_KEY") {
        let bytes = hex::decode(hex_key.trim())
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }

    // 2. Check DB
    if let Some(hex_key) = db.get_state(KV_KEY)? {
        let bytes = hex::decode(&hex_key).map_err(|_| CryptoError::InvalidKeyLength)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }

    // 3. Generate and store
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let hex_key = hex::encode(key);
    db.set_state(KV_KEY, &hex_key)?;
    tracing::info!("generated new server encryption key (stored in database)");

    Ok(key)
}

// ---------------------------------------------------------------------------
// Base64 helpers (simple, no external crate needed)
// ---------------------------------------------------------------------------

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> Result<Vec<u8>, CryptoError> {
    let input = input.trim_end_matches('=');
    let mut result = Vec::with_capacity(input.len() * 3 / 4);

    let decode_char = |c: u8| -> Result<u32, CryptoError> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(CryptoError::Base64Error(format!("invalid char: {}", c as char))),
        }
    };

    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let remaining = bytes.len() - i;

        let b0 = decode_char(bytes[i])?;
        let b1 = if i + 1 < bytes.len() { decode_char(bytes[i + 1])? } else { 0 };
        let b2 = if i + 2 < bytes.len() { decode_char(bytes[i + 2])? } else { 0 };
        let b3 = if i + 3 < bytes.len() { decode_char(bytes[i + 3])? } else { 0 };

        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        result.push(((triple >> 16) & 0xFF) as u8);
        if remaining > 2 {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if remaining > 3 {
            result.push((triple & 0xFF) as u8);
        }

        i += 4;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let data = b"hello world";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0x42u8; 32];
        let plaintext = "my-secret-password";
        let (ct, nonce) = encrypt_credential(plaintext, &key).unwrap();
        let decrypted = decrypt_credential(&ct, &nonce, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_password_hash_verify() {
        let password = "test-password-123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn test_get_or_create_encryption_key() {
        let db = Database::in_memory().unwrap();
        db.initialize().unwrap();

        let key1 = get_or_create_encryption_key(&db).unwrap();
        let key2 = get_or_create_encryption_key(&db).unwrap();
        assert_eq!(key1, key2, "should return same key on second call");
    }
}
