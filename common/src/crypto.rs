use crate::types::{EncryptedData, Signature};
use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit};
use aes_gcm::aead::{Aead, OsRng, AeadCore};
use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signer, Verifier};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, SharedSecret};
use ring::digest::{Context, SHA256};
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};

/// Ed25519 key pair for digital signatures
#[derive(Debug, Clone)]
pub struct SigningKeyPair {
    pub keypair: Keypair,
}

/// X25519 key pair for key exchange
#[derive(Debug)]
pub struct EncryptionKeyPair {
    pub secret: EphemeralSecret,
    pub public: X25519PublicKey,
}

/// Cryptographic service for the blockchain
pub struct CryptoService {
    signing_keypair: SigningKeyPair,
}

impl SigningKeyPair {
    /// Generate a new Ed25519 key pair
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        Self { keypair }
    }

    /// Create from existing secret key bytes
    pub fn from_bytes(secret_bytes: &[u8]) -> Result<Self> {
        if secret_bytes.len() != 32 {
            return Err(anyhow!("Invalid secret key length"));
        }
        
        let secret = SecretKey::from_bytes(secret_bytes)?;
        let public = PublicKey::from(&secret);
        let keypair = Keypair { secret, public };
        
        Ok(Self { keypair })
    }

    /// Get public key bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.keypair.public.to_bytes().to_vec()
    }

    /// Get secret key bytes
    pub fn secret_key_bytes(&self) -> Vec<u8> {
        self.keypair.secret.to_bytes().to_vec()
    }

    /// Sign data
    pub fn sign(&self, data: &[u8]) -> Signature {
        let signature = self.keypair.sign(data);
        Signature {
            algorithm: "Ed25519".to_string(),
            signature: signature.to_bytes().to_vec(),
            public_key: self.public_key_bytes(),
        }
    }

    /// Verify signature
    pub fn verify(&self, data: &[u8], signature: &Signature) -> Result<bool> {
        if signature.algorithm != "Ed25519" {
            return Err(anyhow!("Unsupported signature algorithm"));
        }

        let public_key = PublicKey::from_bytes(&signature.public_key)?;
        let sig = ed25519_dalek::Signature::from_bytes(&signature.signature)?;
        
        match public_key.verify(data, &sig) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

impl EncryptionKeyPair {
    /// Generate a new X25519 key pair
    pub fn generate() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = X25519PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Get public key bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public.to_bytes().to_vec()
    }

    /// Perform key exchange
    pub fn key_exchange(self, peer_public: &[u8]) -> Result<SharedSecret> {
        if peer_public.len() != 32 {
            return Err(anyhow!("Invalid peer public key length"));
        }
        
        let peer_public_key = X25519PublicKey::from(*array_ref::array_ref!(peer_public, 0, 32));
        Ok(self.secret.diffie_hellman(&peer_public_key))
    }
}

impl CryptoService {
    /// Create new crypto service with generated keys
    pub fn new() -> Self {
        Self {
            signing_keypair: SigningKeyPair::generate(),
        }
    }

    /// Create from existing secret key
    pub fn from_secret_key(secret_bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            signing_keypair: SigningKeyPair::from_bytes(secret_bytes)?,
        })
    }

    /// Get public key for this service
    pub fn public_key(&self) -> Vec<u8> {
        self.signing_keypair.public_key_bytes()
    }

    /// Sign data
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_keypair.sign(data)
    }

    /// Verify signature
    pub fn verify(&self, data: &[u8], signature: &Signature) -> Result<bool> {
        self.signing_keypair.verify(data, signature)
    }

    /// Encrypt data using AES-256-GCM with a shared secret
    pub fn encrypt(&self, data: &[u8], shared_secret: &SharedSecret) -> Result<EncryptedData> {
        let key = Key::<Aes256Gcm>::from_slice(shared_secret.as_bytes());
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        let ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        Ok(EncryptedData {
            ciphertext,
            nonce: nonce.to_vec(),
            algorithm: "AES-256-GCM".to_string(),
        })
    }

    /// Decrypt data using AES-256-GCM with a shared secret
    pub fn decrypt(&self, encrypted: &EncryptedData, shared_secret: &SharedSecret) -> Result<Vec<u8>> {
        if encrypted.algorithm != "AES-256-GCM" {
            return Err(anyhow!("Unsupported encryption algorithm"));
        }

        let key = Key::<Aes256Gcm>::from_slice(shared_secret.as_bytes());
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&encrypted.nonce);

        let plaintext = cipher.decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Hash data using SHA-256
    pub fn hash(&self, data: &[u8]) -> Vec<u8> {
        let mut context = Context::new(&SHA256);
        context.update(data);
        context.finish().as_ref().to_vec()
    }

    /// Hash multiple pieces of data together
    pub fn hash_multiple(&self, data_pieces: &[&[u8]]) -> Vec<u8> {
        let mut context = Context::new(&SHA256);
        for piece in data_pieces {
            context.update(piece);
        }
        context.finish().as_ref().to_vec()
    }

    /// Create a deterministic hash from serializable data
    pub fn hash_object<T: Serialize>(&self, obj: &T) -> Result<Vec<u8>> {
        let serialized = bincode::serialize(obj)?;
        Ok(self.hash(&serialized))
    }
}

/// Utility functions for cryptographic operations
pub mod utils {
    use super::*;

    /// Generate a random 32-byte key
    pub fn generate_random_key() -> Vec<u8> {
        let mut key = vec![0u8; 32];
        ring::rand::fill(&ring::rand::SystemRandom::new(), &mut key).unwrap();
        key
    }

    /// Derive a key from a password using PBKDF2
    pub fn derive_key_from_password(password: &str, salt: &[u8], iterations: u32) -> Vec<u8> {
        use ring::pbkdf2;
        let mut key = vec![0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(iterations).unwrap(),
            salt,
            password.as_bytes(),
            &mut key,
        );
        key
    }

    /// Constant-time comparison of byte arrays
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        ring::constant_time::verify_slices_are_equal(a, b).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_and_verification() {
        let keypair = SigningKeyPair::generate();
        let data = b"test message";
        let signature = keypair.sign(data);
        
        assert!(keypair.verify(data, &signature).unwrap());
        assert!(!keypair.verify(b"different message", &signature).unwrap());
    }

    #[test]
    fn test_key_exchange_and_encryption() {
        let alice_keys = EncryptionKeyPair::generate();
        let bob_keys = EncryptionKeyPair::generate();
        
        let alice_shared = alice_keys.key_exchange(&bob_keys.public_key_bytes()).unwrap();
        let bob_shared = bob_keys.key_exchange(&alice_keys.public_key_bytes()).unwrap();
        
        // Shared secrets should be the same
        assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
        
        let crypto = CryptoService::new();
        let data = b"secret message";
        
        let encrypted = crypto.encrypt(data, &alice_shared).unwrap();
        let decrypted = crypto.decrypt(&encrypted, &bob_shared).unwrap();
        
        assert_eq!(data, decrypted.as_slice());
    }
}