//! Encrypted in-memory object store.
//!
//! Provides EncryptedWorldState -- the encrypted object/link graph for one world.
//! Objects and links are encrypted at rest under the world key K_w.
//! During burn, secure_wipe zeroizes all ciphertext.

use crate::{CryptoError, EncryptedData, WorldKey};
use chamber_types::{Object, ObjectId, ObjectLink};
use std::collections::HashMap;
use zeroize::Zeroize;

/// An encrypted object entry: stores encrypted serialized Object + metadata.
#[derive(Debug)]
struct EncryptedObjectEntry {
    object_id: ObjectId,
    object_type: String,
    preservable: bool,
    ciphertext: Vec<u8>,
    nonce: [u8; 12],
}

/// An encrypted link entry.
#[derive(Debug)]
struct EncryptedLinkEntry {
    ciphertext: Vec<u8>,
    nonce: [u8; 12],
    source_id: ObjectId,
    target_id: ObjectId,
}

/// Encrypted state for a single world.
/// Holds encrypted objects and links, keyed by ObjectId.
#[derive(Debug)]
pub struct EncryptedWorldState {
    objects: HashMap<ObjectId, EncryptedObjectEntry>,
    links: Vec<EncryptedLinkEntry>,
}

impl EncryptedWorldState {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            links: Vec::new(),
        }
    }

    /// Encrypt and store an object.
    pub fn add_object(&mut self, object: &Object, key: &WorldKey) -> Result<(), String> {
        let json = serde_json::to_vec(object).map_err(|e| e.to_string())?;
        let encrypted = encrypt_bytes(&json, key).map_err(|e| e.to_string())?;

        self.objects.insert(
            object.object_id,
            EncryptedObjectEntry {
                object_id: object.object_id,
                object_type: object.object_type.clone(),
                preservable: object.preservable,
                ciphertext: encrypted.ciphertext,
                nonce: encrypted.nonce,
            },
        );
        Ok(())
    }

    /// Decrypt and access an object within a closure.
    pub fn with_object<F, R>(
        &self,
        object_id: ObjectId,
        key: &WorldKey,
        f: F,
    ) -> Result<R, String>
    where
        F: FnOnce(&Object) -> R,
    {
        let entry = self
            .objects
            .get(&object_id)
            .ok_or_else(|| format!("object not found: {}", object_id))?;

        let encrypted = EncryptedData {
            ciphertext: entry.ciphertext.clone(),
            nonce: entry.nonce,
        };
        let plaintext = decrypt_bytes(&encrypted, key).map_err(|e| e.to_string())?;
        let object: Object = serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
        Ok(f(&object))
    }

    /// Decrypt, mutate, and re-encrypt an object.
    pub fn with_object_mut<F>(
        &mut self,
        object_id: ObjectId,
        key: &WorldKey,
        f: F,
    ) -> Result<(), String>
    where
        F: FnOnce(&mut Object),
    {
        let entry = self
            .objects
            .get(&object_id)
            .ok_or_else(|| format!("object not found: {}", object_id))?;

        let encrypted = EncryptedData {
            ciphertext: entry.ciphertext.clone(),
            nonce: entry.nonce,
        };
        let plaintext = decrypt_bytes(&encrypted, key).map_err(|e| e.to_string())?;
        let mut object: Object = serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
        f(&mut object);

        // Re-encrypt
        self.add_object(&object, key)
    }

    /// Check if an object exists.
    pub fn has_object(&self, object_id: ObjectId) -> bool {
        self.objects.contains_key(&object_id)
    }

    /// Get the object type (stored in plaintext metadata for policy checks).
    pub fn object_type(&self, object_id: ObjectId) -> Option<&str> {
        self.objects.get(&object_id).map(|e| e.object_type.as_str())
    }

    /// Check if an object is preservable.
    pub fn is_preservable(&self, object_id: ObjectId) -> bool {
        self.objects
            .get(&object_id)
            .map(|e| e.preservable)
            .unwrap_or(false)
    }

    /// Encrypt and store a link.
    pub fn add_link(&mut self, link: &ObjectLink, key: &WorldKey) -> Result<(), String> {
        let json = serde_json::to_vec(link).map_err(|e| e.to_string())?;
        let encrypted = encrypt_bytes(&json, key).map_err(|e| e.to_string())?;

        self.links.push(EncryptedLinkEntry {
            ciphertext: encrypted.ciphertext,
            nonce: encrypted.nonce,
            source_id: link.source_id,
            target_id: link.target_id,
        });
        Ok(())
    }

    /// Check if a link exists between two objects.
    pub fn link_exists(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        _key: &WorldKey,
    ) -> bool {
        self.links
            .iter()
            .any(|l| l.source_id == source_id && l.target_id == target_id)
    }

    /// Decrypt all objects (for views).
    pub fn all_objects_decrypted(&self, key: &WorldKey) -> Vec<Object> {
        let mut result = Vec::new();
        for entry in self.objects.values() {
            let encrypted = EncryptedData {
                ciphertext: entry.ciphertext.clone(),
                nonce: entry.nonce,
            };
            if let Ok(plaintext) = decrypt_bytes(&encrypted, key) {
                if let Ok(obj) = serde_json::from_slice::<Object>(&plaintext) {
                    result.push(obj);
                }
            }
        }
        result
    }

    /// Decrypt all links.
    pub fn all_links_decrypted(&self, key: &WorldKey) -> Vec<ObjectLink> {
        let mut result = Vec::new();
        for entry in &self.links {
            let encrypted = EncryptedData {
                ciphertext: entry.ciphertext.clone(),
                nonce: entry.nonce,
            };
            if let Ok(plaintext) = decrypt_bytes(&encrypted, key) {
                if let Ok(link) = serde_json::from_slice::<ObjectLink>(&plaintext) {
                    result.push(link);
                }
            }
        }
        result
    }

    /// Number of objects stored.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Number of links stored.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Securely wipe all encrypted data. Called during burn.
    pub fn secure_wipe(&mut self) {
        for entry in self.objects.values_mut() {
            entry.ciphertext.zeroize();
            entry.nonce.zeroize();
        }
        self.objects.clear();

        for entry in &mut self.links {
            entry.ciphertext.zeroize();
            entry.nonce.zeroize();
        }
        self.links.clear();
    }
}

impl Default for EncryptedWorldState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal encrypt/decrypt helpers using WorldKey directly
// ---------------------------------------------------------------------------

fn encrypt_bytes(plaintext: &[u8], key: &WorldKey) -> Result<EncryptedData, CryptoError> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::{Aes256Gcm, Nonce};
    use rand::RngCore;

    let cipher = Aes256Gcm::new_from_slice(&key.key_bytes)
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

fn decrypt_bytes(encrypted: &EncryptedData, key: &WorldKey) -> Result<Vec<u8>, CryptoError> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let cipher = Aes256Gcm::new_from_slice(&key.key_bytes)
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

    let nonce = Nonce::from_slice(&encrypted.nonce);

    cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
}
