//! AES-256-GCM encryption of a credentials file.
//!
//! The key is 32 random bytes, written as 64 hex characters. An encrypted file
//! is one line, `base64(nonce)--base64(ciphertext+tag)`, with a fresh random
//! nonce on every write.

use std::fmt;

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};

const SEPARATOR: &str = "--";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

/// The key a credentials file is encrypted with. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct MasterKey(Key<Aes256Gcm>);

impl MasterKey {
    /// A new random key.
    #[must_use]
    pub fn generate() -> Self {
        Self(Aes256Gcm::generate_key(OsRng))
    }

    /// Parses 64 hex characters; surrounding whitespace (a trailing newline
    /// in a key file) is ignored. `None` for anything else.
    #[must_use]
    pub fn parse(hex_key: &str) -> Option<Self> {
        let bytes: [u8; KEY_LEN] = hex::decode(hex_key.trim()).ok()?.try_into().ok()?;
        Some(Self(Key::<Aes256Gcm>::from(bytes)))
    }

    /// The key as 64 lowercase hex characters, as written to a key file.
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(..)")
    }
}

/// Why a file could not be decrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecryptError {
    /// Not `base64--base64`, or the plaintext is not UTF-8.
    Format,
    /// Authentication failed: the wrong key, or the file was altered.
    Mismatch,
}

/// Encrypts `plaintext` under a fresh random nonce.
#[must_use]
pub fn encrypt(key: &MasterKey, plaintext: &str) -> String {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = Aes256Gcm::new(&key.0)
        .encrypt(&nonce, plaintext.as_bytes())
        // Only fails for plaintexts beyond GCM's ~64 GiB limit.
        .expect("credentials plaintext exceeds the AES-GCM size limit");
    format!(
        "{}{SEPARATOR}{}",
        STANDARD.encode(nonce),
        STANDARD.encode(ciphertext)
    )
}

/// Decrypts what [`encrypt`] produced.
///
/// # Errors
///
/// [`DecryptError::Format`] when `contents` is not an encrypted credentials
/// file, [`DecryptError::Mismatch`] when `key` does not open it.
pub fn decrypt(key: &MasterKey, contents: &str) -> Result<String, DecryptError> {
    let (nonce, ciphertext) = contents
        .trim()
        .split_once(SEPARATOR)
        .ok_or(DecryptError::Format)?;
    let nonce = STANDARD.decode(nonce).map_err(|_| DecryptError::Format)?;
    let ciphertext = STANDARD
        .decode(ciphertext)
        .map_err(|_| DecryptError::Format)?;
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| DecryptError::Format)?;
    let plaintext = Aes256Gcm::new(&key.0)
        .decrypt(&Nonce::from(nonce), ciphertext.as_slice())
        .map_err(|_| DecryptError::Mismatch)?;
    String::from_utf8(plaintext).map_err(|_| DecryptError::Format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let key = MasterKey::generate();
        let sealed = encrypt(&key, "jwt:\n  secret: s3cret\n");
        assert_eq!(decrypt(&key, &sealed).unwrap(), "jwt:\n  secret: s3cret\n");
    }

    #[test]
    fn wrong_key_is_a_mismatch() {
        let sealed = encrypt(&MasterKey::generate(), "a: 1");
        assert_eq!(
            decrypt(&MasterKey::generate(), &sealed),
            Err(DecryptError::Mismatch)
        );
    }

    #[test]
    fn a_flipped_byte_is_a_mismatch() {
        let key = MasterKey::generate();
        let sealed = encrypt(&key, "a: 1");
        let (nonce, ciphertext) = sealed.split_once(SEPARATOR).unwrap();
        let mut bytes = STANDARD.decode(ciphertext).unwrap();
        bytes[0] ^= 1;
        let tampered = format!("{nonce}{SEPARATOR}{}", STANDARD.encode(bytes));
        assert_eq!(decrypt(&key, &tampered), Err(DecryptError::Mismatch));
    }

    #[test]
    fn every_write_uses_a_fresh_nonce() {
        let key = MasterKey::generate();
        assert_ne!(encrypt(&key, "a: 1"), encrypt(&key, "a: 1"));
    }

    #[test]
    fn rejects_what_is_not_a_credentials_file() {
        let key = MasterKey::generate();
        for contents in ["", "plain: yaml", "!!!--!!!", "AAAA--AAAA"] {
            assert_eq!(
                decrypt(&key, contents),
                Err(DecryptError::Format),
                "{contents}"
            );
        }
    }

    #[test]
    fn keys_are_64_hex_characters() {
        let key = MasterKey::generate();
        let hex_key = key.to_hex();
        assert_eq!(hex_key.len(), 64);
        assert_eq!(MasterKey::parse(&format!("{hex_key}\n")), Some(key));
        assert_eq!(MasterKey::parse(&hex_key[..62]), None);
        assert_eq!(MasterKey::parse(&"z".repeat(64)), None);
    }

    #[test]
    fn debug_hides_the_key() {
        assert_eq!(format!("{:?}", MasterKey::generate()), "MasterKey(..)");
    }
}
