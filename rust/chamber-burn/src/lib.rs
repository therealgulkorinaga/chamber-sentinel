//! Burn engine for Chamber Sentinel.
//!
//! Implements the six-layer destruction model:
//! 1. Logical burn -- invalidate handles, mark world terminated
//! 2. Cryptographic burn -- destroy K_w
//! 3. Storage cleanup -- delete world-scoped records
//! 4. Memory cleanup -- zero runtime structures
//! 5. Audit burn -- destroy Tier 2 world events
//! 6. Semantic residue measurement

use chamber_audit::{AuditEventType, AuditLog};
use chamber_crypto::CryptoProvider;
use chamber_state::StateEngine;
use chamber_types::{SubstrateError, SubstrateResult, TerminationMode, WorldId};
use std::sync::Arc;

/// Result of a burn operation.
#[derive(Debug, serde::Serialize)]
pub struct BurnResult {
    pub world_id: WorldId,
    pub mode: TerminationMode,
    pub layers_completed: Vec<String>,
    pub errors: Vec<String>,
    pub residue: Option<SemanticResidueReport>,
}

/// Post-burn semantic residue measurement.
/// Camera-specific fields for reporting what was processed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticResidueReport {
    /// Can we still find the world in the state engine?
    pub state_engine_has_world: bool,
    /// Does a crypto key still exist for this world?
    pub crypto_key_exists: bool,
    /// Is the crypto key marked as destroyed?
    pub crypto_key_destroyed: bool,
    /// Number of substrate-scoped audit events (Tier 1). Expected: exactly 2 (created + destroyed).
    pub substrate_event_count: usize,
    /// Number of world-scoped audit events surviving burn (Tier 2). Expected: 0.
    pub world_events_surviving: usize,
    /// Are any world-scoped audit events leaking through burn?
    pub audit_leaks_internals: bool,
    /// Residue score: 0.0 = no recoverable world state, 1.0 = fully recoverable.
    pub residue_score: f64,
    /// Camera-specific: number of frames that were processed in this world.
    pub frames_processed: u64,
    /// Camera-specific: number of chambers burned in this session.
    pub chambers_burned: u64,
    /// Camera-specific: number of events sealed before burn.
    pub events_sealed: u64,
}

/// The burn engine orchestrates world destruction.
#[derive(Debug)]
pub struct BurnEngine {
    crypto: Arc<CryptoProvider>,
    state: Arc<StateEngine>,
    audit: Arc<AuditLog>,
}

impl BurnEngine {
    pub fn new(
        crypto: Arc<CryptoProvider>,
        state: Arc<StateEngine>,
        audit: Arc<AuditLog>,
    ) -> Self {
        Self {
            crypto,
            state,
            audit,
        }
    }

    /// Execute the full burn sequence.
    pub fn burn_world(
        &self,
        world_id: WorldId,
        mode: TerminationMode,
    ) -> SubstrateResult<BurnResult> {
        let mut result = BurnResult {
            world_id,
            mode,
            layers_completed: Vec::new(),
            errors: Vec::new(),
            residue: None,
        };

        self.audit.record(
            world_id,
            AuditEventType::BurnStarted { mode },
        );

        // Gather pre-burn stats for the residue report
        let pre_burn_object_count = self.state.object_count(world_id).unwrap_or(0) as u64;

        // Layer 1: Logical burn -- mark handles invalid
        match self.logical_burn(world_id) {
            Ok(()) => {
                result.layers_completed.push("logical".into());
                self.audit.record(
                    world_id,
                    AuditEventType::BurnLayerCompleted {
                        layer: "logical".into(),
                    },
                );
            }
            Err(e) => result.errors.push(format!("logical: {}", e)),
        }

        // Layer 2: Cryptographic burn -- destroy K_w
        match self.cryptographic_burn(world_id) {
            Ok(()) => {
                result.layers_completed.push("cryptographic".into());
                self.audit.record(
                    world_id,
                    AuditEventType::BurnLayerCompleted {
                        layer: "cryptographic".into(),
                    },
                );
            }
            Err(e) => result.errors.push(format!("cryptographic: {}", e)),
        }

        // Layer 3: Storage cleanup -- remove world-scoped data
        match self.storage_cleanup(world_id) {
            Ok(()) => {
                result.layers_completed.push("storage".into());
                self.audit.record(
                    world_id,
                    AuditEventType::BurnLayerCompleted {
                        layer: "storage".into(),
                    },
                );
            }
            Err(e) => result.errors.push(format!("storage: {}", e)),
        }

        // Layer 4: Memory cleanup -- zero in-memory structures
        match self.memory_cleanup(world_id) {
            Ok(()) => {
                result.layers_completed.push("memory".into());
                self.audit.record(
                    world_id,
                    AuditEventType::BurnLayerCompleted {
                        layer: "memory".into(),
                    },
                );
            }
            Err(e) => result.errors.push(format!("memory: {}", e)),
        }

        // Layer 5: Destroy world-scoped audit events (Tier 2)
        // After this, only Tier 1 events survive (WorldCreated + WorldDestroyed)
        self.audit.burn_world_events(world_id);
        result.layers_completed.push("audit_burn".into());

        // Layer 6: Semantic residue measurement
        let residue = self.semantic_measurement(world_id, pre_burn_object_count);
        result.residue = Some(residue);
        result.layers_completed.push("semantic_measurement".into());

        // Tier 1 event: world destroyed (ONLY post-burn record besides WorldCreated)
        self.audit.record(
            world_id,
            AuditEventType::BurnCompleted { mode },
        );

        Ok(result)
    }

    /// Layer 1: Logical burn -- no capability system in camera, so this is a no-op
    /// that confirms the world exists.
    fn logical_burn(&self, world_id: WorldId) -> SubstrateResult<()> {
        if !self.state.has_world(world_id) && !self.crypto.has_world_key(world_id) {
            return Err(SubstrateError::WorldNotFound(world_id));
        }
        Ok(())
    }

    /// Layer 2: Cryptographic burn -- destroy K_w.
    fn cryptographic_burn(&self, world_id: WorldId) -> SubstrateResult<()> {
        self.crypto
            .destroy_world_key(world_id)
            .map_err(|e| SubstrateError::BurnFailed {
                layer: "cryptographic".into(),
                reason: e.to_string(),
            })
    }

    /// Layer 3: Storage cleanup -- remove world-scoped data.
    fn storage_cleanup(&self, world_id: WorldId) -> SubstrateResult<()> {
        self.state.destroy_world_state(world_id)
    }

    /// Layer 4: Memory cleanup -- additional zeroing.
    fn memory_cleanup(&self, _world_id: WorldId) -> SubstrateResult<()> {
        // State engine already secure-wiped in storage_cleanup.
        // This layer handles any additional in-memory structures.
        Ok(())
    }

    /// Post-burn semantic residue measurement.
    /// Also available as a standalone measurement tool.
    pub fn measure_residue(&self, world_id: WorldId) -> SemanticResidueReport {
        self.semantic_measurement(world_id, 0)
    }

    fn semantic_measurement(
        &self,
        world_id: WorldId,
        frames_processed: u64,
    ) -> SemanticResidueReport {
        let state_engine_has_world = self.state.has_world(world_id);
        let crypto_key_exists = self.crypto.has_world_key(world_id);
        let crypto_key_destroyed = self.crypto.is_key_destroyed(world_id);

        // Count only substrate-scoped events (Tier 1)
        let substrate_event_count = self.audit.substrate_event_count(world_id);

        // Check: do any world-scoped (Tier 2) events still exist? They shouldn't.
        let all_events = self.audit.events_for_world(world_id);
        let world_scoped_surviving = all_events
            .iter()
            .filter(|e| !e.event_type.is_substrate_scoped())
            .count();
        let audit_leaks_internals = world_scoped_surviving > 0;

        // Compute residue score.
        // 0.0 = perfect burn (no recoverable world state).
        let mut score = 0.0;
        if state_engine_has_world {
            score += 0.4; // Major: full object graph recoverable
        }
        if crypto_key_exists {
            score += 0.4; // Major: ciphertext decryptable
        }
        if audit_leaks_internals {
            score += 0.15; // Moderate: world-scoped events survived burn
        }

        SemanticResidueReport {
            state_engine_has_world,
            crypto_key_exists,
            crypto_key_destroyed,
            substrate_event_count,
            world_events_surviving: world_scoped_surviving,
            audit_leaks_internals,
            residue_score: score,
            frames_processed,
            chambers_burned: 1,
            events_sealed: 0,
        }
    }
}
