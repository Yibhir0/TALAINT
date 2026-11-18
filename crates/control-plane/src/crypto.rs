use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub const KEK_ENV: &str = "TALAINT_KEK_BASE64";
pub const NONCE_LEN: usize = 12;

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn normalize_secret_name_to_env(name: &str) -> String {
    let mut out = String::from("TALAINT_SECRET_");
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

pub fn load_kek_from_env() -> Result<[u8; 32]> {
    let b64 = std::env::var(KEK_ENV)
        .map_err(|_| anyhow!("{KEK_ENV} is not set (base64-encoded 32-byte key required)"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .context("invalid base64 in TALAINT_KEK_BASE64")?;
    if bytes.len() != 32 {
        return Err(anyhow!(
            "TALAINT_KEK_BASE64 must decode to 32 bytes, got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn cipher_from_key(key: &[u8; 32]) -> Result<Aes256Gcm> {
    Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("invalid AES-256 key: {e}"))
}

fn random_nonce() -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut n);
    n
}

fn random_dek() -> [u8; 32] {
    let mut k = [0u8; 32];
    OsRng.fill_bytes(&mut k);
    k
}

/// Returns (wrapped_dek_bytes, key_version). wrapped_dek_bytes = nonce || ciphertext+tag
pub fn generate_wrapped_dek(kek: &[u8; 32]) -> Result<(Vec<u8>, i32)> {
    let dek = random_dek();
    let cipher = cipher_from_key(kek)?;
    let nonce_bytes = random_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, dek.as_ref())
        .map_err(|e| anyhow!("encrypt dek: {e}"))?;
    let mut wrapped = Vec::with_capacity(NONCE_LEN + ct.len());
    wrapped.extend_from_slice(&nonce_bytes);
    wrapped.extend_from_slice(&ct);
    Ok((wrapped, 1))
}

pub fn unwrap_dek(kek: &[u8; 32], wrapped: &[u8]) -> Result<[u8; 32]> {
    if wrapped.len() < NONCE_LEN + 16 {
        return Err(anyhow!("wrapped DEK too short"));
    }
    let (nonce_bytes, ct) = wrapped.split_at(NONCE_LEN);
    let cipher = cipher_from_key(kek)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let pt = cipher
        .decrypt(nonce, ct)
        .map_err(|e| anyhow!("decrypt dek: {e}"))?;
    if pt.len() != 32 {
        return Err(anyhow!("invalid DEK length {}", pt.len()));
    }
    let mut dek = [0u8; 32];
    dek.copy_from_slice(&pt);
    Ok(dek)
}

/// Returns (nonce_bytes, ciphertext_bytes)
pub fn encrypt_secret(dek: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = cipher_from_key(dek)?;
    let nonce_bytes = random_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow!("encrypt secret: {e}"))?;
    Ok((nonce_bytes.to_vec(), ct))
}

pub fn decrypt_secret(dek: &[u8; 32], nonce_bytes: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = cipher_from_key(dek)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("decrypt secret: {e}"))
}
