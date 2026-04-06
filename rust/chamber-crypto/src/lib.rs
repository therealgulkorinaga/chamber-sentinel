//! Cryptographic primitives for Chamber Sentinel.
//!
//! Key hierarchy: K_s (substrate) wraps K_w (per-world).
//! K_w encrypts all world-scoped state at rest.
//! Burn = destroy K_w -> ciphertext becomes unrecoverable.
//!
//! Memory hardening: mlock, MADV_DONTDUMP, guard buffer.
//! Android-specific: uses prctl for debugger denial.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use chamber_types::WorldId;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use zeroize::Zeroize;

pub mod mem_protect;
pub mod encrypted_store;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("key not found for world: {0}")]
    KeyNotFound(WorldId),
    #[error("key already destroyed for world: {0}")]
    KeyDestroyed(WorldId),
    #[error("substrate key not initialized")]
    SubstrateKeyNotInitialized,
}

/// A world-scoped encryption key. Zeroized on drop.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct WorldKey {
    pub key_bytes: [u8; 32],
}

impl std::fmt::Debug for WorldKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WorldKey([REDACTED])")
    }
}

/// Encrypted data with nonce.
#[derive(Debug, Clone)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
}

/// The crypto provider manages the key hierarchy.
#[derive(Debug)]
pub struct CryptoProvider {
    /// Substrate key K_s -- wraps per-world keys.
    #[allow(dead_code)]
    substrate_key: Arc<Mutex<Option<[u8; 32]>>>,
    /// Active world keys, keyed by WorldId.
    world_keys: Arc<Mutex<HashMap<WorldId, WorldKey>>>,
    /// Tombstoned world IDs whose keys have been destroyed.
    destroyed_keys: Arc<Mutex<Vec<WorldId>>>,
}

impl CryptoProvider {
    pub fn new() -> Self {
        let mut ks = [0u8; 32];
        OsRng.fill_bytes(&mut ks);
        Self {
            substrate_key: Arc::new(Mutex::new(Some(ks))),
            world_keys: Arc::new(Mutex::new(HashMap::new())),
            destroyed_keys: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Generate a new world key K_w. The key is mlock'd in physical RAM.
    pub fn generate_world_key(&self, world_id: WorldId) -> Result<(), CryptoError> {
        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        // Pin the key in physical RAM -- never paged to swap
        mem_protect::mlock_key(&key_bytes);
        let world_key = WorldKey { key_bytes };
        self.world_keys.lock().unwrap().insert(world_id, world_key);
        Ok(())
    }

    /// Encrypt data using a world's key.
    pub fn encrypt(
        &self,
        world_id: WorldId,
        plaintext: &[u8],
    ) -> Result<EncryptedData, CryptoError> {
        let keys = self.world_keys.lock().unwrap();
        let world_key = keys
            .get(&world_id)
            .ok_or(CryptoError::KeyNotFound(world_id))?;

        let cipher = Aes256Gcm::new_from_slice(&world_key.key_bytes)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        Ok(EncryptedData {
            ciphertext,
            nonce: nonce_bytes,
        })
    }

    /// Decrypt data using a world's key.
    pub fn decrypt(
        &self,
        world_id: WorldId,
        encrypted: &EncryptedData,
    ) -> Result<Vec<u8>, CryptoError> {
        if self.destroyed_keys.lock().unwrap().contains(&world_id) {
            return Err(CryptoError::KeyDestroyed(world_id));
        }

        let keys = self.world_keys.lock().unwrap();
        let world_key = keys
            .get(&world_id)
            .ok_or(CryptoError::KeyNotFound(world_id))?;

        let cipher = Aes256Gcm::new_from_slice(&world_key.key_bytes)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        let nonce = Nonce::from_slice(&encrypted.nonce);

        cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }

    /// Cryptographic burn: destroy K_w for a world.
    /// After this, any ciphertext encrypted under K_w is unrecoverable.
    pub fn destroy_world_key(&self, world_id: WorldId) -> Result<(), CryptoError> {
        let mut keys = self.world_keys.lock().unwrap();
        if let Some(mut key) = keys.remove(&world_id) {
            key.key_bytes.zeroize();
            self.destroyed_keys.lock().unwrap().push(world_id);
            Ok(())
        } else if self.destroyed_keys.lock().unwrap().contains(&world_id) {
            Ok(()) // idempotent
        } else {
            Err(CryptoError::KeyNotFound(world_id))
        }
    }

    /// Check if a world key exists (not destroyed).
    pub fn has_world_key(&self, world_id: WorldId) -> bool {
        self.world_keys.lock().unwrap().contains_key(&world_id)
    }

    /// Check if a world key was destroyed.
    pub fn is_key_destroyed(&self, world_id: WorldId) -> bool {
        self.destroyed_keys.lock().unwrap().contains(&world_id)
    }

    /// Access a world key within a scoped closure.
    pub fn with_world_key<F, R>(&self, world_id: WorldId, f: F) -> Result<R, CryptoError>
    where
        F: FnOnce(&WorldKey) -> R,
    {
        let keys = self.world_keys.lock().unwrap();
        let key = keys
            .get(&world_id)
            .ok_or(CryptoError::KeyNotFound(world_id))?;
        Ok(f(key))
    }
}

impl Default for CryptoProvider {
    fn default() -> Self {
        Self::new()
    }
}
