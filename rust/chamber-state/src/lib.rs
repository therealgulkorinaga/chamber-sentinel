//! State engine for Chamber Sentinel.
//!
//! Holds the encrypted object graph, links, and lifecycle phase per world.
//! All object/link data is encrypted at rest under K_w via EncryptedWorldState.

use chamber_crypto::encrypted_store::EncryptedWorldState;
use chamber_crypto::CryptoProvider;
use chamber_types::{Object, ObjectId, ObjectLink, SubstrateError, SubstrateResult, WorldId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Bundle of encrypted world data.
#[derive(Debug)]
pub struct EncryptedWorldStateBundle {
    pub encrypted: EncryptedWorldState,
}

/// The state engine manages world states across all active worlds.
/// All object/link data is encrypted via CryptoProvider.
#[derive(Debug)]
pub struct StateEngine {
    worlds: Arc<Mutex<HashMap<WorldId, EncryptedWorldStateBundle>>>,
    crypto: Arc<CryptoProvider>,
}

impl StateEngine {
    pub fn new(crypto: Arc<CryptoProvider>) -> Self {
        Self {
            worlds: Arc::new(Mutex::new(HashMap::new())),
            crypto,
        }
    }

    pub fn create_world_state(&self, world_id: WorldId) {
        self.worlds.lock().unwrap().insert(
            world_id,
            EncryptedWorldStateBundle {
                encrypted: EncryptedWorldState::new(),
            },
        );
    }

    /// Remove all state for a world (used during burn).
    /// Securely wipes all ciphertext content before dropping.
    pub fn destroy_world_state(&self, world_id: WorldId) -> SubstrateResult<()> {
        if let Some(mut bundle) = self.worlds.lock().unwrap().remove(&world_id) {
            bundle.encrypted.secure_wipe();
        }
        Ok(())
    }

    pub fn has_world(&self, world_id: WorldId) -> bool {
        self.worlds.lock().unwrap().contains_key(&world_id)
    }

    // --- Object operations (encrypt/decrypt via CryptoProvider) ---

    pub fn add_object(&self, world_id: WorldId, object: Object) -> SubstrateResult<()> {
        let mut worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get_mut(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.add_object(&object, key)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })?
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e,
            })
    }

    pub fn with_object<F, R>(
        &self,
        world_id: WorldId,
        object_id: ObjectId,
        f: F,
    ) -> SubstrateResult<R>
    where
        F: FnOnce(&Object) -> R,
    {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.with_object(object_id, key, f)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })?
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e,
            })
    }

    pub fn with_object_mut<F>(
        &self,
        world_id: WorldId,
        object_id: ObjectId,
        f: F,
    ) -> SubstrateResult<()>
    where
        F: FnOnce(&mut Object),
    {
        let mut worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get_mut(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.with_object_mut(object_id, key, f)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })?
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e,
            })
    }

    pub fn has_object(&self, world_id: WorldId, object_id: ObjectId) -> SubstrateResult<bool> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        Ok(bundle.encrypted.has_object(object_id))
    }

    pub fn object_type(
        &self,
        world_id: WorldId,
        object_id: ObjectId,
    ) -> SubstrateResult<Option<String>> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        Ok(bundle
            .encrypted
            .object_type(object_id)
            .map(|s| s.to_string()))
    }

    pub fn is_preservable(
        &self,
        world_id: WorldId,
        object_id: ObjectId,
    ) -> SubstrateResult<bool> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        Ok(bundle.encrypted.is_preservable(object_id))
    }

    // --- Link operations ---

    pub fn add_link(&self, world_id: WorldId, link: ObjectLink) -> SubstrateResult<()> {
        let mut worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get_mut(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.add_link(&link, key)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })?
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e,
            })
    }

    pub fn link_exists(
        &self,
        world_id: WorldId,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> SubstrateResult<bool> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.link_exists(source_id, target_id, key)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })
    }

    // --- Bulk read (for views -- decrypts one at a time) ---

    pub fn all_objects_decrypted(&self, world_id: WorldId) -> SubstrateResult<Vec<Object>> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.all_objects_decrypted(key)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })
    }

    pub fn all_links_decrypted(&self, world_id: WorldId) -> SubstrateResult<Vec<ObjectLink>> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        self.crypto
            .with_world_key(world_id, |key| {
                bundle.encrypted.all_links_decrypted(key)
            })
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })
    }

    // --- Counts (no decryption) ---

    pub fn object_count(&self, world_id: WorldId) -> SubstrateResult<usize> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        Ok(bundle.encrypted.object_count())
    }

    pub fn link_count(&self, world_id: WorldId) -> SubstrateResult<usize> {
        let worlds = self.worlds.lock().unwrap();
        let bundle = worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;
        Ok(bundle.encrypted.link_count())
    }
}
