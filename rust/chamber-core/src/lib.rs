//! Chamber Sentinel JNI bridge.
//!
//! This is the cdylib that Android loads via System.loadLibrary("chamber_core").
//! It owns a Runtime that wires together all substrate engines, and exposes
//! JNI functions for Kotlin/Java to call.

use chamber_audit::{AuditEventType, AuditLog};
use chamber_burn::BurnEngine;
use chamber_crypto::CryptoProvider;
use chamber_policy::PolicyEngine;
use chamber_state::StateEngine;
use chamber_types::*;
use chrono::Utc;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jint, jlong, jstring};
use jni::JNIEnv;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Artifact Vault — sole cross-world channel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Artifact {
    pub artifact_id: ArtifactId,
    pub source_world_id: WorldId,
    pub artifact_class: String,
    pub payload: serde_json::Value,
    pub sealed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ArtifactVault {
    artifacts: Arc<Mutex<Vec<Artifact>>>,
}

impl ArtifactVault {
    pub fn new() -> Self {
        Self { artifacts: Arc::new(Mutex::new(Vec::new())) }
    }
    pub fn store(&self, artifact: Artifact) {
        self.artifacts.lock().unwrap().push(artifact);
    }
    pub fn artifact_count_for_world(&self, world_id: WorldId) -> usize {
        self.artifacts.lock().unwrap().iter().filter(|a| a.source_world_id == world_id).count()
    }
    pub fn artifacts_from_world(&self, world_id: WorldId) -> Vec<Artifact> {
        self.artifacts.lock().unwrap().iter().filter(|a| a.source_world_id == world_id).cloned().collect()
    }
    pub fn all_artifacts(&self) -> Vec<Artifact> {
        self.artifacts.lock().unwrap().clone()
    }
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// The Chamber Sentinel substrate runtime.
/// Owns all engines, manages world lifecycle.
pub struct Runtime {
    pub crypto: Arc<CryptoProvider>,
    pub state_engine: Arc<StateEngine>,
    pub policy: Arc<PolicyEngine>,
    pub burn_engine: BurnEngine,
    pub audit: Arc<AuditLog>,
    /// Maps world_id -> (grammar_id, phase)
    pub worlds: HashMap<WorldId, WorldMeta>,
    /// Tracks frame counts per world for residue reporting
    pub frame_counts: HashMap<WorldId, u64>,
    /// Artifact vault — sole cross-world channel
    pub vault: ArtifactVault,
}

struct WorldMeta {
    grammar_id: String,
    phase: LifecyclePhase,
    #[allow(dead_code)]
    objective: String,
}

impl Runtime {
    pub fn new() -> Self {
        // Apply process hardening on init
        chamber_crypto::mem_protect::harden_process();

        let crypto = Arc::new(CryptoProvider::new());
        let audit = Arc::new(AuditLog::new());
        let state = Arc::new(StateEngine::new(crypto.clone()));
        let policy = Arc::new(PolicyEngine::new());

        let burn_engine = BurnEngine::new(crypto.clone(), state.clone(), audit.clone());

        // Load the camera grammar
        let grammar = chamber_policy::camera_sentinel_grammar();
        policy.load_grammar(grammar).expect("failed to load camera grammar");

        Runtime {
            crypto,
            state_engine: state,
            policy,
            burn_engine,
            audit,
            worlds: HashMap::new(),
            frame_counts: HashMap::new(),
            vault: ArtifactVault::new(),
        }
    }

    pub fn create_world(&mut self, grammar_id: &str, objective: &str) -> SubstrateResult<WorldId> {
        // Verify grammar exists
        self.policy.get_grammar(grammar_id)?;

        let world_id = WorldId::new();

        // Generate cryptographic key for this world
        self.crypto
            .generate_world_key(world_id)
            .map_err(|e| SubstrateError::CryptoOperationFailed {
                world_id,
                reason: e.to_string(),
            })?;

        // Initialize encrypted state
        self.state_engine.create_world_state(world_id);

        // Record creation
        self.audit.record(
            world_id,
            AuditEventType::WorldCreated {
                grammar_id: grammar_id.to_string(),
            },
        );

        // Track world metadata
        self.worlds.insert(
            world_id,
            WorldMeta {
                grammar_id: grammar_id.to_string(),
                phase: LifecyclePhase::Active,
                objective: objective.to_string(),
            },
        );
        self.frame_counts.insert(world_id, 0);

        // Advance to Active phase
        self.audit.record(
            world_id,
            AuditEventType::PhaseTransition {
                from: LifecyclePhase::Created,
                to: LifecyclePhase::Active,
            },
        );

        Ok(world_id)
    }

    pub fn submit_create_object(
        &mut self,
        world_id: WorldId,
        object_type: &str,
        payload_json: &str,
    ) -> SubstrateResult<ObjectId> {
        let meta = self
            .worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;

        if meta.phase == LifecyclePhase::Terminated {
            return Err(SubstrateError::WorldTerminated(world_id));
        }

        // Policy checks
        self.policy
            .is_object_type_allowed(&meta.grammar_id, object_type)?
            .then_some(())
            .ok_or_else(|| SubstrateError::UnknownObjectType(object_type.to_string()))?;

        self.policy
            .is_primitive_allowed(&meta.grammar_id, Primitive::CreateObject, meta.phase)?
            .then_some(())
            .ok_or(SubstrateError::OperationNotPermittedInPhase {
                operation: Primitive::CreateObject,
                phase: meta.phase,
            })?;

        // Determine lifecycle class and preservability from grammar
        let grammar = self.policy.get_grammar(&meta.grammar_id)?;
        let type_spec = grammar
            .object_types
            .get(object_type)
            .ok_or_else(|| SubstrateError::UnknownObjectType(object_type.to_string()))?;

        let payload: serde_json::Value =
            serde_json::from_str(payload_json).map_err(|e| SubstrateError::InvalidPayload {
                object_type: object_type.to_string(),
                reason: e.to_string(),
            })?;

        let object_id = ObjectId::new();
        let object = Object {
            object_id,
            world_id,
            object_type: object_type.to_string(),
            lifecycle_class: type_spec.default_lifecycle_class,
            payload,
            preservable: type_spec.can_be_preservable,
            created_at: Utc::now(),
        };

        self.state_engine.add_object(world_id, object)?;

        // Track frame counts
        if object_type == "frame" {
            if let Some(count) = self.frame_counts.get_mut(&world_id) {
                *count += 1;
            }
        }

        Ok(object_id)
    }

    pub fn submit_seal_artifact(
        &self,
        world_id: WorldId,
        object_id: ObjectId,
    ) -> SubstrateResult<ArtifactId> {
        let meta = self
            .worlds
            .get(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;

        if meta.phase == LifecyclePhase::Terminated {
            return Err(SubstrateError::WorldTerminated(world_id));
        }

        // Check preservability
        let is_preservable = self.state_engine.is_preservable(world_id, object_id)?;
        if !is_preservable {
            let obj_type = self
                .state_engine
                .object_type(world_id, object_id)?
                .unwrap_or_else(|| "unknown".to_string());
            return Err(SubstrateError::NotPreservable {
                object_type: obj_type,
            });
        }

        // Check policy
        let obj_type = self
            .state_engine
            .object_type(world_id, object_id)?
            .unwrap_or_else(|| "unknown".to_string());
        let can_preserve = self
            .policy
            .can_preserve_object(&meta.grammar_id, &obj_type)?;
        if !can_preserve {
            return Err(SubstrateError::NotPreservable {
                object_type: obj_type,
            });
        }

        // Get the object to store in vault
        let object = self.state_engine.with_object(world_id, object_id, |o| o.clone())?;

        let artifact_id = ArtifactId::new();
        let artifact = Artifact {
            artifact_id,
            source_world_id: world_id,
            artifact_class: obj_type.clone(),
            payload: object.payload,
            sealed_at: Utc::now(),
        };
        self.vault.store(artifact);

        self.audit.record(
            world_id,
            AuditEventType::ArtifactSealed {
                artifact_class: obj_type,
            },
        );

        Ok(artifact_id)
    }

    pub fn burn_world(
        &mut self,
        world_id: WorldId,
        mode: TerminationMode,
    ) -> SubstrateResult<chamber_burn::BurnResult> {
        let meta = self
            .worlds
            .get_mut(&world_id)
            .ok_or(SubstrateError::WorldNotFound(world_id))?;

        if meta.phase == LifecyclePhase::Terminated {
            return Err(SubstrateError::WorldTerminated(world_id));
        }

        // Validate termination mode against policy
        self.policy
            .validate_termination(&meta.grammar_id, mode, false)?;

        // Mark as terminated
        meta.phase = LifecyclePhase::Terminated;

        // Execute 6-layer burn
        let result = self.burn_engine.burn_world(world_id, mode)?;

        // Remove from tracking
        self.frame_counts.remove(&world_id);

        Ok(result)
    }

    pub fn get_residue_report(&self, world_id: WorldId) -> chamber_burn::SemanticResidueReport {
        self.burn_engine.measure_residue(world_id)
    }

    pub fn ingest_frame(
        &mut self,
        world_id: WorldId,
        _frame_bytes: &[u8],
        width: i32,
        height: i32,
        timestamp: i64,
    ) -> SubstrateResult<ObjectId> {
        let payload = serde_json::json!({
            "width": width,
            "height": height,
            "timestamp_ms": timestamp,
            "byte_count": _frame_bytes.len(),
        });

        let payload_str = serde_json::to_string(&payload).map_err(|e| {
            SubstrateError::InvalidPayload {
                object_type: "frame".to_string(),
                reason: e.to_string(),
            }
        })?;

        self.submit_create_object(world_id, "frame", &payload_str)
    }

    /// Convenience: create an object with a serde_json::Value payload.
    pub fn create_object(
        &mut self,
        world_id: WorldId,
        object_type: &str,
        payload: serde_json::Value,
        _preservable: bool,
    ) -> SubstrateResult<ObjectId> {
        let payload_str = serde_json::to_string(&payload).map_err(|e| {
            SubstrateError::InvalidPayload {
                object_type: object_type.to_string(),
                reason: e.to_string(),
            }
        })?;
        self.submit_create_object(world_id, object_type, &payload_str)
    }

    /// Convenience: seal an artifact (wraps submit_seal_artifact).
    pub fn seal_artifact(
        &self,
        world_id: WorldId,
        object_id: ObjectId,
    ) -> SubstrateResult<ArtifactId> {
        self.submit_seal_artifact(world_id, object_id)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a jlong pointer back to a mutable Runtime reference.
/// The pointer must have been created by Box::into_raw(Box::new(Runtime::new())).
unsafe fn runtime_from_ptr<'a>(ptr: jlong) -> &'a mut Runtime {
    &mut *(ptr as *mut Runtime)
}

/// Helper to parse a world_id string into a WorldId.
fn parse_world_id(env: &mut JNIEnv, world_id_str: &JString) -> Option<WorldId> {
    let id_str: String = env.get_string(world_id_str).ok()?.into();
    let uuid = uuid::Uuid::parse_str(&id_str).ok()?;
    Some(WorldId(uuid))
}

/// Helper to return a JSON error string.
fn error_json(env: &mut JNIEnv, msg: &str) -> jstring {
    let json = serde_json::json!({"error": msg}).to_string();
    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ---------------------------------------------------------------------------
// JNI exports
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_nativeInit(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let runtime = Box::new(Runtime::new());
    Box::into_raw(runtime) as jlong
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe {
            let _ = Box::from_raw(ptr as *mut Runtime);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_nativeVersion<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let mut env = env;
    env.new_string(VERSION)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_createWorld<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    grammar_id: JString<'local>,
    objective: JString<'local>,
) -> jstring {
    let rt = unsafe { runtime_from_ptr(ptr) };

    let grammar_str: String = match env.get_string(&grammar_id) {
        Ok(s) => s.into(),
        Err(_) => return error_json(&mut env, "invalid grammar_id string"),
    };
    let objective_str: String = match env.get_string(&objective) {
        Ok(s) => s.into(),
        Err(_) => return error_json(&mut env, "invalid objective string"),
    };

    match rt.create_world(&grammar_str, &objective_str) {
        Ok(world_id) => {
            let json = serde_json::json!({"world_id": world_id.to_string()}).to_string();
            env.new_string(&json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => error_json(&mut env, &e.to_string()),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_submitCreateObject<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    world_id_str: JString<'local>,
    object_type: JString<'local>,
    payload_json: JString<'local>,
) -> jstring {
    let rt = unsafe { runtime_from_ptr(ptr) };

    let world_id = match parse_world_id(&mut env, &world_id_str) {
        Some(id) => id,
        None => return error_json(&mut env, "invalid world_id"),
    };

    let obj_type: String = match env.get_string(&object_type) {
        Ok(s) => s.into(),
        Err(_) => return error_json(&mut env, "invalid object_type string"),
    };

    let payload: String = match env.get_string(&payload_json) {
        Ok(s) => s.into(),
        Err(_) => return error_json(&mut env, "invalid payload_json string"),
    };

    match rt.submit_create_object(world_id, &obj_type, &payload) {
        Ok(object_id) => {
            let json = serde_json::json!({"object_id": object_id.to_string()}).to_string();
            env.new_string(&json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => error_json(&mut env, &e.to_string()),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_submitSealArtifact<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    world_id_str: JString<'local>,
    object_id_str: JString<'local>,
) -> jstring {
    let rt = unsafe { runtime_from_ptr(ptr) };

    let world_id = match parse_world_id(&mut env, &world_id_str) {
        Some(id) => id,
        None => return error_json(&mut env, "invalid world_id"),
    };

    let obj_id_str: String = match env.get_string(&object_id_str) {
        Ok(s) => s.into(),
        Err(_) => return error_json(&mut env, "invalid object_id string"),
    };

    let obj_uuid = match uuid::Uuid::parse_str(&obj_id_str) {
        Ok(u) => u,
        Err(_) => return error_json(&mut env, "invalid object_id UUID"),
    };
    let object_id = ObjectId(obj_uuid);

    match rt.submit_seal_artifact(world_id, object_id) {
        Ok(artifact_id) => {
            let json =
                serde_json::json!({"artifact_id": artifact_id.to_string()}).to_string();
            env.new_string(&json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => error_json(&mut env, &e.to_string()),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_burn<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    world_id_str: JString<'local>,
    mode_str: JString<'local>,
) -> jstring {
    let rt = unsafe { runtime_from_ptr(ptr) };

    let world_id = match parse_world_id(&mut env, &world_id_str) {
        Some(id) => id,
        None => return error_json(&mut env, "invalid world_id"),
    };

    let mode_string: String = match env.get_string(&mode_str) {
        Ok(s) => s.into(),
        Err(_) => return error_json(&mut env, "invalid mode string"),
    };

    let mode = match mode_string.as_str() {
        "auto" | "AutoBurn" => TerminationMode::AutoBurn,
        "emergency" | "EmergencyBurn" => TerminationMode::EmergencyBurn,
        "manual" | "ManualBurn" => TerminationMode::ManualBurn,
        _ => return error_json(&mut env, "unknown termination mode"),
    };

    match rt.burn_world(world_id, mode) {
        Ok(result) => {
            let json = serde_json::to_string(&result).unwrap_or_else(|_| {
                serde_json::json!({"error": "serialization failed"}).to_string()
            });
            env.new_string(&json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => error_json(&mut env, &e.to_string()),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_getResidueReport<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    world_id_str: JString<'local>,
) -> jstring {
    let rt = unsafe { runtime_from_ptr(ptr) };

    let world_id = match parse_world_id(&mut env, &world_id_str) {
        Some(id) => id,
        None => return error_json(&mut env, "invalid world_id"),
    };

    let report = rt.get_residue_report(world_id);
    let json = serde_json::to_string(&report)
        .unwrap_or_else(|_| serde_json::json!({"error": "serialization failed"}).to_string());
    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_chamber_sentinel_ChamberBridge_ingestFrame<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    ptr: jlong,
    world_id_str: JString<'local>,
    frame_bytes: JByteArray<'local>,
    width: jint,
    height: jint,
    timestamp: jlong,
) -> jstring {
    let rt = unsafe { runtime_from_ptr(ptr) };
    let mut env = env;

    let world_id = match parse_world_id(&mut env, &world_id_str) {
        Some(id) => id,
        None => return error_json(&mut env, "invalid world_id"),
    };

    let bytes = match env.convert_byte_array(frame_bytes) {
        Ok(b) => b,
        Err(_) => return error_json(&mut env, "invalid frame byte array"),
    };

    match rt.ingest_frame(world_id, &bytes, width, height, timestamp) {
        Ok(object_id) => {
            let json = serde_json::json!({
                "object_id": object_id.to_string(),
                "frame_size": bytes.len(),
            })
            .to_string();
            env.new_string(&json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => error_json(&mut env, &e.to_string()),
    }
}
