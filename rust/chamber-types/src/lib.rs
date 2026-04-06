//! Core types for the Chamber Sentinel substrate runtime.
//!
//! Ported from the Chambers substrate for Android camera use.
//! Simplified lifecycle (no Convergence/Finalization phases),
//! 3 primitives, 4 object types for camera pipeline.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Identity types
// ---------------------------------------------------------------------------

/// Unique, non-reusable identifier for a world (camera session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldId(pub Uuid);

impl WorldId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorldId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for an object within a world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub Uuid);

impl ObjectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a sealed artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub Uuid);

impl ArtifactId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ArtifactId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Lifecycle phases for a camera world.
/// Simplified from the full Chambers model: no Convergence/Finalization.
/// Created -> Active -> Terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifecyclePhase {
    Created,
    Active,
    Terminated,
}

impl LifecyclePhase {
    /// Returns whether transitioning from `self` to `target` is legal.
    pub fn can_transition_to(&self, target: LifecyclePhase) -> bool {
        use LifecyclePhase::*;
        matches!(
            (self, target),
            (Created, Active)
                | (Active, Terminated)
                | (Created, Terminated) // abort path
        )
    }
}

/// How a world terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminationMode {
    /// Automatic burn at session end (normal camera lifecycle).
    AutoBurn,
    /// Emergency burn triggered by tamper detection or policy violation.
    EmergencyBurn,
    /// Manual burn triggered by user action.
    ManualBurn,
}

/// Lifecycle class determines what happens to an object at burn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifecycleClass {
    /// Destroyed at burn. Frames, detections, working state.
    Temporary,
    /// May survive burn if sealed. Event summaries, integrity tags.
    Preservable,
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

/// The closed, finite set of primitive operations for camera worlds.
/// Only 3 primitives for the camera pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Primitive {
    CreateObject,
    SealArtifact,
    TriggerBurn,
}

impl Primitive {
    pub const ALL: &'static [Primitive] = &[
        Primitive::CreateObject,
        Primitive::SealArtifact,
        Primitive::TriggerBurn,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Primitive::CreateObject => "create_object",
            Primitive::SealArtifact => "seal_artifact",
            Primitive::TriggerBurn => "trigger_burn",
        }
    }
}

impl std::fmt::Display for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// The specific operation requested, with typed parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionOperation {
    CreateObject {
        object_type: String,
        payload: serde_json::Value,
        lifecycle_class: LifecycleClass,
        preservable: bool,
    },
    SealArtifact {
        target_id: ObjectId,
        authorization: SealAuthorization,
    },
    TriggerBurn {
        mode: TerminationMode,
    },
}

impl TransitionOperation {
    /// Returns which primitive this operation corresponds to.
    pub fn primitive(&self) -> Primitive {
        match self {
            TransitionOperation::CreateObject { .. } => Primitive::CreateObject,
            TransitionOperation::SealArtifact { .. } => Primitive::SealArtifact,
            TransitionOperation::TriggerBurn { .. } => Primitive::TriggerBurn,
        }
    }
}

/// A transition request submitted to the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRequest {
    pub world_id: WorldId,
    pub operation: TransitionOperation,
}

/// Authorization for sealing an artifact.
/// Camera events are auto-authorized (no human confirmation needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SealAuthorization {
    /// Auto-authorized for camera events.
    AutoAuthorized,
    /// Policy engine approved.
    PolicyApproved { policy_rule: String },
}

// ---------------------------------------------------------------------------
// Objects
// ---------------------------------------------------------------------------

/// A typed object within a world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub object_id: ObjectId,
    pub world_id: WorldId,
    /// The grammar-defined type (e.g., "frame", "detection", "event_summary", "integrity_tag").
    pub object_type: String,
    pub lifecycle_class: LifecycleClass,
    /// Structured payload.
    pub payload: serde_json::Value,
    /// Whether this object can be sealed into an artifact.
    pub preservable: bool,
    pub created_at: DateTime<Utc>,
}

/// A directed edge between two objects in the same world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectLink {
    pub source_id: ObjectId,
    pub target_id: ObjectId,
    pub link_type: String,
    pub world_id: WorldId,
}

// ---------------------------------------------------------------------------
// Artifacts
// ---------------------------------------------------------------------------

/// A sealed artifact that survived world termination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub artifact_id: ArtifactId,
    pub source_world_id: WorldId,
    pub artifact_class: String,
    pub payload: serde_json::Value,
    pub sealed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from the substrate runtime.
#[derive(Debug, thiserror::Error)]
pub enum SubstrateError {
    #[error("world not found: {0}")]
    WorldNotFound(WorldId),

    #[error("world already terminated: {0}")]
    WorldTerminated(WorldId),

    #[error("world ID reuse attempted: {0}")]
    WorldIdReuse(WorldId),

    #[error("invalid lifecycle transition from {from:?} to {to:?}")]
    InvalidLifecycleTransition {
        from: LifecyclePhase,
        to: LifecyclePhase,
    },

    #[error("object not found: {object_id} in world {world_id}")]
    ObjectNotFound {
        object_id: ObjectId,
        world_id: WorldId,
    },

    #[error("unknown object type: {0}")]
    UnknownObjectType(String),

    #[error("invalid payload for type {object_type}: {reason}")]
    InvalidPayload {
        object_type: String,
        reason: String,
    },

    #[error("operation {operation} not permitted in phase {phase:?}")]
    OperationNotPermittedInPhase {
        operation: Primitive,
        phase: LifecyclePhase,
    },

    #[error("object type {object_type} is not preservable under grammar preservation law")]
    NotPreservable { object_type: String },

    #[error("no artifact exists for preservation")]
    NoArtifactForPreservation,

    #[error("burn failed at layer {layer}: {reason}")]
    BurnFailed { layer: String, reason: String },

    #[error("grammar not found: {0}")]
    GrammarNotFound(String),

    #[error("policy violation: {0}")]
    PolicyViolation(String),

    #[error("crypto operation failed for world {world_id}: {reason}")]
    CryptoOperationFailed { world_id: WorldId, reason: String },
}

pub type SubstrateResult<T> = Result<T, SubstrateError>;

// ---------------------------------------------------------------------------
// Grammar
// ---------------------------------------------------------------------------

/// Key for phase-specific primitive permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifecyclePhaseKey {
    Created,
    Active,
}

impl From<LifecyclePhase> for LifecyclePhaseKey {
    fn from(phase: LifecyclePhase) -> Self {
        match phase {
            LifecyclePhase::Created => LifecyclePhaseKey::Created,
            LifecyclePhase::Active => LifecyclePhaseKey::Active,
            LifecyclePhase::Terminated => LifecyclePhaseKey::Active, // no ops in terminated
        }
    }
}

/// Specification of an object type within a grammar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectTypeSpec {
    pub type_name: String,
    pub payload_schema: serde_json::Value,
    pub max_payload_bytes: usize,
    pub transform_set: Vec<Primitive>,
    pub default_lifecycle_class: LifecycleClass,
    pub can_be_preservable: bool,
}

/// A legal termination mode for this grammar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationLaw {
    pub mode: TerminationMode,
    pub description: String,
    pub requires_artifact: bool,
}

/// Specification of a permitted link between object types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSpec {
    pub link_type: String,
    pub source_types: Vec<String>,
    pub target_types: Vec<String>,
}

/// A chamber grammar definition for the camera pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamberGrammar {
    pub grammar_id: String,
    pub name: String,
    pub description: String,
    pub objective_class: String,
    pub object_types: HashMap<String, ObjectTypeSpec>,
    pub phase_primitives: HashMap<LifecyclePhaseKey, Vec<Primitive>>,
    pub preservable_classes: Vec<String>,
    pub termination_modes: Vec<TerminationLaw>,
    pub permitted_links: Vec<LinkSpec>,
}
