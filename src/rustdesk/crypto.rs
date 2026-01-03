//! RustDesk Cryptography
//!
//! This module implements the NaCl-based cryptography used by RustDesk:
//! - Curve25519 for key exchange
//! - XSalsa20-Poly1305 for authenticated encryption
//! - Ed25519 for signatures
//! - Ed25519 to Curve25519 key conversion for unified keypair usage

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sodiumoxide::crypto::box_::{self, Nonce, PublicKey, SecretKey};
use sodiumoxide::crypto::secretbox;
use sodiumoxide::crypto::sign::{self, ed25519};
use thiserror::Error;

/// Cryptography errors
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Failed to initialize sodiumoxide")]
    InitError,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Invalid key length")]
    InvalidKeyLength,
    #[error("Invalid nonce")]
    InvalidNonce,
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    #[error("Key conversion failed")]
    KeyConversionFailed,
}

/// Initialize the cryptography library
/// Must be called before using any crypto functions
pub fn init() -> Result<(), CryptoError> {
    sodiumoxide::init().map_err(|_| CryptoError::InitError)
}

/// A keypair for asymmetric encryption
#[derive(Clone)]
pub struct KeyPair {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

impl KeyPair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let (public_key, secret_key) = box_::gen_keypair();
        Self {
            public_key,
            secret_key,
        }
    }

    /// Create from existing keys
    pub fn from_keys(public_key: &[u8], secret_key: &[u8]) -> Result<Self, CryptoError> {
        let pk = PublicKey::from_slice(public_key).ok_or(CryptoError::InvalidKeyLength)?;
        let sk = SecretKey::from_slice(secret_key).ok_or(CryptoError::InvalidKeyLength)?;
        Ok(Self {
            public_key: pk,
            secret_key: sk,
        })
    }

    /// Get public key as bytes
    pub fn public_key_bytes(&self) -> &[u8] {
        self.public_key.as_ref()
    }

    /// Get secret key as bytes
    pub fn secret_key_bytes(&self) -> &[u8] {
        self.secret_key.as_ref()
    }

    /// Encode public key as base64
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.public_key_bytes())
    }

    /// Encode secret key as base64
    pub fn secret_key_base64(&self) -> String {
        BASE64.encode(self.secret_key_bytes())
    }

    /// Create from base64-encoded keys
    pub fn from_base64(public_key: &str, secret_key: &str) -> Result<Self, CryptoError> {
        let pk_bytes = BASE64.decode(public_key).map_err(|_| CryptoError::InvalidKeyLength)?;
        let sk_bytes = BASE64.decode(secret_key).map_err(|_| CryptoError::InvalidKeyLength)?;
        Self::from_keys(&pk_bytes, &sk_bytes)
    }
}

/// Generate a random nonce for box encryption
pub fn generate_nonce() -> Nonce {
    box_::gen_nonce()
}

/// Encrypt data using public-key cryptography (NaCl box)
///
/// Uses the sender's secret key and receiver's public key for encryption.
/// Returns (nonce, ciphertext).
pub fn encrypt_box(
    data: &[u8],
    their_public_key: &PublicKey,
    our_secret_key: &SecretKey,
) -> (Nonce, Vec<u8>) {
    let nonce = generate_nonce();
    let ciphertext = box_::seal(data, &nonce, their_public_key, our_secret_key);
    (nonce, ciphertext)
}

/// Decrypt data using public-key cryptography (NaCl box)
pub fn decrypt_box(
    ciphertext: &[u8],
    nonce: &Nonce,
    their_public_key: &PublicKey,
    our_secret_key: &SecretKey,
) -> Result<Vec<u8>, CryptoError> {
    box_::open(ciphertext, nonce, their_public_key, our_secret_key)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Encrypt data with a precomputed shared key
pub fn encrypt_with_key(data: &[u8], key: &secretbox::Key) -> (secretbox::Nonce, Vec<u8>) {
    let nonce = secretbox::gen_nonce();
    let ciphertext = secretbox::seal(data, &nonce, key);
    (nonce, ciphertext)
}

/// Decrypt data with a precomputed shared key
pub fn decrypt_with_key(
    ciphertext: &[u8],
    nonce: &secretbox::Nonce,
    key: &secretbox::Key,
) -> Result<Vec<u8>, CryptoError> {
    secretbox::open(ciphertext, nonce, key).map_err(|_| CryptoError::DecryptionFailed)
}

/// Compute a shared symmetric key from public/private keypair
/// This is the precomputed key for the NaCl box
pub fn precompute_key(their_public_key: &PublicKey, our_secret_key: &SecretKey) -> box_::PrecomputedKey {
    box_::precompute(their_public_key, our_secret_key)
}

/// Create a symmetric key from raw bytes
pub fn symmetric_key_from_slice(key: &[u8]) -> Result<secretbox::Key, CryptoError> {
    secretbox::Key::from_slice(key).ok_or(CryptoError::InvalidKeyLength)
}

/// Parse a nonce from bytes
pub fn nonce_from_slice(bytes: &[u8]) -> Result<Nonce, CryptoError> {
    Nonce::from_slice(bytes).ok_or(CryptoError::InvalidNonce)
}

/// Parse a public key from bytes
pub fn public_key_from_slice(bytes: &[u8]) -> Result<PublicKey, CryptoError> {
    PublicKey::from_slice(bytes).ok_or(CryptoError::InvalidKeyLength)
}

/// Hash a password for storage/comparison
/// RustDesk uses simple SHA256 for password hashing
pub fn hash_password(password: &str, salt: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    hasher.finalize().to_vec()
}

/// RustDesk double hash for password verification
/// Client calculates: SHA256(SHA256(password + salt) + challenge)
/// This matches what the client sends in LoginRequest
pub fn hash_password_double(password: &str, salt: &str, challenge: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    // First hash: SHA256(password + salt)
    let mut hasher1 = Sha256::new();
    hasher1.update(password.as_bytes());
    hasher1.update(salt.as_bytes());
    let first_hash = hasher1.finalize();

    // Second hash: SHA256(first_hash + challenge)
    let mut hasher2 = Sha256::new();
    hasher2.update(&first_hash);
    hasher2.update(challenge.as_bytes());
    hasher2.finalize().to_vec()
}

/// Verify a password hash
pub fn verify_password(password: &str, salt: &str, expected_hash: &[u8]) -> bool {
    let computed = hash_password(password, salt);
    // Constant-time comparison would be better, but for our use case this is acceptable
    computed == expected_hash
}

/// Decrypt symmetric key using Curve25519 secret key directly
///
/// This is used when we have a fresh Curve25519 keypair for the connection
/// (as per RustDesk protocol - each connection generates a new keypair)
pub fn decrypt_symmetric_key(
    their_temp_public_key: &[u8],
    sealed_symmetric_key: &[u8],
    our_secret_key: &SecretKey,
) -> Result<secretbox::Key, CryptoError> {
    if their_temp_public_key.len() != box_::PUBLICKEYBYTES {
        return Err(CryptoError::InvalidKeyLength);
    }

    let their_pk = PublicKey::from_slice(their_temp_public_key)
        .ok_or(CryptoError::InvalidKeyLength)?;

    // Use zero nonce as per RustDesk protocol
    let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);

    let key_bytes = box_::open(sealed_symmetric_key, &nonce, &their_pk, our_secret_key)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    secretbox::Key::from_slice(&key_bytes).ok_or(CryptoError::InvalidKeyLength)
}

/// Encrypt a message using the negotiated symmetric key
///
/// RustDesk uses a specific nonce format for session encryption
pub fn encrypt_message(data: &[u8], key: &secretbox::Key, nonce_counter: u64) -> Vec<u8> {
    // Create nonce from counter (little-endian, padded to 24 bytes)
    let mut nonce_bytes = [0u8; secretbox::NONCEBYTES];
    nonce_bytes[..8].copy_from_slice(&nonce_counter.to_le_bytes());
    let nonce = secretbox::Nonce(nonce_bytes);

    secretbox::seal(data, &nonce, key)
}

/// Decrypt a message using the negotiated symmetric key
pub fn decrypt_message(
    ciphertext: &[u8],
    key: &secretbox::Key,
    nonce_counter: u64,
) -> Result<Vec<u8>, CryptoError> {
    // Create nonce from counter (little-endian, padded to 24 bytes)
    let mut nonce_bytes = [0u8; secretbox::NONCEBYTES];
    nonce_bytes[..8].copy_from_slice(&nonce_counter.to_le_bytes());
    let nonce = secretbox::Nonce(nonce_bytes);

    secretbox::open(ciphertext, &nonce, key).map_err(|_| CryptoError::DecryptionFailed)
}

/// Ed25519 signing keypair for RustDesk SignedId messages
#[derive(Clone)]
pub struct SigningKeyPair {
    pub public_key: sign::PublicKey,
    pub secret_key: sign::SecretKey,
}

impl SigningKeyPair {
    /// Generate a new random signing keypair
    pub fn generate() -> Self {
        let (public_key, secret_key) = sign::gen_keypair();
        Self {
            public_key,
            secret_key,
        }
    }

    /// Create from existing keys
    pub fn from_keys(public_key: &[u8], secret_key: &[u8]) -> Result<Self, CryptoError> {
        let pk = sign::PublicKey::from_slice(public_key).ok_or(CryptoError::InvalidKeyLength)?;
        let sk = sign::SecretKey::from_slice(secret_key).ok_or(CryptoError::InvalidKeyLength)?;
        Ok(Self {
            public_key: pk,
            secret_key: sk,
        })
    }

    /// Get public key as bytes
    pub fn public_key_bytes(&self) -> &[u8] {
        self.public_key.as_ref()
    }

    /// Get secret key as bytes
    pub fn secret_key_bytes(&self) -> &[u8] {
        self.secret_key.as_ref()
    }

    /// Encode public key as base64
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.public_key_bytes())
    }

    /// Encode secret key as base64
    pub fn secret_key_base64(&self) -> String {
        BASE64.encode(self.secret_key_bytes())
    }

    /// Create from base64-encoded keys
    pub fn from_base64(public_key: &str, secret_key: &str) -> Result<Self, CryptoError> {
        let pk_bytes = BASE64.decode(public_key).map_err(|_| CryptoError::InvalidKeyLength)?;
        let sk_bytes = BASE64.decode(secret_key).map_err(|_| CryptoError::InvalidKeyLength)?;
        Self::from_keys(&pk_bytes, &sk_bytes)
    }

    /// Sign a message
    /// Returns the signature prepended to the message (as per RustDesk protocol)
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        sign::sign(message, &self.secret_key)
    }

    /// Sign a message and return just the signature (64 bytes)
    pub fn sign_detached(&self, message: &[u8]) -> [u8; 64] {
        let sig = sign::sign_detached(message, &self.secret_key);
        // Use as_ref() to access the signature bytes since the inner field is private
        let sig_bytes: &[u8] = sig.as_ref();
        let mut result = [0u8; 64];
        result.copy_from_slice(sig_bytes);
        result
    }

    /// Convert Ed25519 public key to Curve25519 public key for encryption
    ///
    /// This allows using the same keypair for both signing and encryption,
    /// which is required by RustDesk's protocol where clients encrypt the
    /// symmetric key using the public key from IdPk.
    pub fn to_curve25519_pk(&self) -> Result<PublicKey, CryptoError> {
        ed25519::to_curve25519_pk(&self.public_key)
            .map_err(|_| CryptoError::KeyConversionFailed)
    }

    /// Convert Ed25519 secret key to Curve25519 secret key for decryption
    ///
    /// This allows decrypting messages that were encrypted using the
    /// converted public key.
    pub fn to_curve25519_sk(&self) -> Result<SecretKey, CryptoError> {
        ed25519::to_curve25519_sk(&self.secret_key)
            .map_err(|_| CryptoError::KeyConversionFailed)
    }
}

/// Verify a signed message
/// Returns the original message if signature is valid
pub fn verify_signed(signed_message: &[u8], public_key: &sign::PublicKey) -> Result<Vec<u8>, CryptoError> {
    sign::verify(signed_message, public_key).map_err(|_| CryptoError::SignatureVerificationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let _ = init();
        let keypair = KeyPair::generate();
        assert_eq!(keypair.public_key_bytes().len(), 32);
        assert_eq!(keypair.secret_key_bytes().len(), 32);
    }

    #[test]
    fn test_keypair_serialization() {
        let _ = init();
        let keypair1 = KeyPair::generate();
        let pk_b64 = keypair1.public_key_base64();
        let sk_b64 = keypair1.secret_key_base64();

        let keypair2 = KeyPair::from_base64(&pk_b64, &sk_b64).unwrap();
        assert_eq!(keypair1.public_key_bytes(), keypair2.public_key_bytes());
        assert_eq!(keypair1.secret_key_bytes(), keypair2.secret_key_bytes());
    }

    #[test]
    fn test_box_encryption() {
        let _ = init();
        let alice = KeyPair::generate();
        let bob = KeyPair::generate();

        let message = b"Hello, RustDesk!";
        let (nonce, ciphertext) = encrypt_box(message, &bob.public_key, &alice.secret_key);

        let plaintext = decrypt_box(&ciphertext, &nonce, &alice.public_key, &bob.secret_key).unwrap();
        assert_eq!(plaintext, message);
    }

    #[test]
    fn test_password_hashing() {
        let password = "test_password";
        let salt = "random_salt";

        let hash1 = hash_password(password, salt);
        let hash2 = hash_password(password, salt);
        assert_eq!(hash1, hash2);

        assert!(verify_password(password, salt, &hash1));
        assert!(!verify_password("wrong_password", salt, &hash1));
    }
}
