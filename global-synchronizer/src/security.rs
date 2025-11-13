use std::collections::HashSet;
use ed25519_dalek::{SigningKey, Signer, PublicKey};
use hex;

/// Key provider abstraction for validator/node keys.
/// Implementations may use environment, KMS, Vault, or HSM.
pub trait KeyProvider: Send + Sync {
    fn public_key_ed25519(&self) -> Option<Vec<u8>>;
    fn sign_ed25519(&self, message: &[u8]) -> Option<Vec<u8>>;
}

/// Environment-based key provider (no private keys on disk requirement can be met via env injection).
pub struct EnvKeyProvider;

impl KeyProvider for EnvKeyProvider {
    fn public_key_ed25519(&self) -> Option<Vec<u8>> {
        let pk_hex = std::env::var("SYNC_NODE_ED25519_PK_HEX").ok()?;
        let bytes = hex::decode(pk_hex).ok()?;
        Some(bytes)
    }

    fn sign_ed25519(&self, message: &[u8]) -> Option<Vec<u8>> {
        let sk_hex = std::env::var("SYNC_NODE_ED25519_SK_HEX").ok()?;
        let sk_bytes = hex::decode(sk_hex).ok()?;
        let arr: [u8; 32] = sk_bytes.try_into().ok()?;
        let sk = SigningKey::from_bytes(&arr);
        let sig = sk.sign(message);
        Some(sig.to_bytes().to_vec())
    }
}

/// Certificate revocation list manager (placeholder for mTLS revocation integration).
pub struct RevocationList {
    revoked_serials: HashSet<String>,
}

impl RevocationList {
    pub fn new() -> Self { Self { revoked_serials: HashSet::new() } }
    pub fn revoke(&mut self, serial: String) { self.revoked_serials.insert(serial); }
    pub fn is_revoked(&self, serial: &str) -> bool { self.revoked_serials.contains(serial) }
}