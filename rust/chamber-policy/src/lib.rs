//! Policy engine for Chamber Sentinel.
//!
//! Loads grammar definitions and enforces:
//! - permitted object types
//! - permitted primitive calls per phase
//! - preservation-law checks
//! - termination-law checks
//!
//! Includes a hardcoded camera grammar:
//! 4 object types: frame, detection, event_summary, integrity_tag
//! 3 primitives: CreateObject, SealArtifact, TriggerBurn
//! Preservation law: only event_summary and integrity_tag survive

use chamber_types::*;
use std::collections::HashMap;
use std::sync::RwLock;

/// The policy engine -- grammar-driven rule enforcement.
#[derive(Debug)]
pub struct PolicyEngine {
    grammars: RwLock<HashMap<String, ChamberGrammar>>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            grammars: RwLock::new(HashMap::new()),
        }
    }

    /// Load a grammar definition.
    pub fn load_grammar(&self, grammar: ChamberGrammar) -> SubstrateResult<()> {
        let id = grammar.grammar_id.clone();
        self.grammars.write().unwrap().insert(id, grammar);
        Ok(())
    }

    /// Check if an object type is allowed in a grammar.
    pub fn is_object_type_allowed(
        &self,
        grammar_id: &str,
        object_type: &str,
    ) -> SubstrateResult<bool> {
        let grammars = self.grammars.read().unwrap();
        let grammar = grammars
            .get(grammar_id)
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))?;
        Ok(grammar.object_types.contains_key(object_type))
    }

    /// Check if a primitive is allowed in the current lifecycle phase.
    pub fn is_primitive_allowed(
        &self,
        grammar_id: &str,
        primitive: Primitive,
        phase: LifecyclePhase,
    ) -> SubstrateResult<bool> {
        let grammars = self.grammars.read().unwrap();
        let grammar = grammars
            .get(grammar_id)
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))?;
        let phase_key = LifecyclePhaseKey::from(phase);

        if let Some(allowed) = grammar.phase_primitives.get(&phase_key) {
            Ok(allowed.contains(&primitive))
        } else {
            Ok(false)
        }
    }

    /// Check if an object type is preservable under the grammar's preservation law.
    pub fn can_preserve_object(
        &self,
        grammar_id: &str,
        object_type: &str,
    ) -> SubstrateResult<bool> {
        let grammars = self.grammars.read().unwrap();
        let grammar = grammars
            .get(grammar_id)
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))?;
        Ok(grammar
            .preservable_classes
            .contains(&object_type.to_string()))
    }

    /// Validate a termination mode against the grammar's termination law.
    pub fn validate_termination(
        &self,
        grammar_id: &str,
        mode: TerminationMode,
        has_artifact: bool,
    ) -> SubstrateResult<()> {
        let grammars = self.grammars.read().unwrap();
        let grammar = grammars
            .get(grammar_id)
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))?;

        let law = grammar
            .termination_modes
            .iter()
            .find(|t| t.mode == mode)
            .ok_or_else(|| {
                SubstrateError::PolicyViolation(format!(
                    "termination mode {:?} not permitted by grammar",
                    mode
                ))
            })?;

        if law.requires_artifact && !has_artifact {
            return Err(SubstrateError::NoArtifactForPreservation);
        }

        Ok(())
    }

    /// Check if a lifecycle transition is legal.
    pub fn is_transition_legal(
        &self,
        _grammar_id: &str,
        current: LifecyclePhase,
        target: LifecyclePhase,
    ) -> SubstrateResult<bool> {
        Ok(current.can_transition_to(target))
    }

    /// Get preservable classes for a grammar.
    pub fn get_preservable_classes(&self, grammar_id: &str) -> SubstrateResult<Vec<String>> {
        let grammars = self.grammars.read().unwrap();
        let grammar = grammars
            .get(grammar_id)
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))?;
        Ok(grammar.preservable_classes.clone())
    }

    /// Get object types for a grammar.
    pub fn get_object_types(&self, grammar_id: &str) -> SubstrateResult<Vec<String>> {
        let grammars = self.grammars.read().unwrap();
        let grammar = grammars
            .get(grammar_id)
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))?;
        Ok(grammar.object_types.keys().cloned().collect())
    }

    /// Get the grammar (cloned).
    pub fn get_grammar(&self, grammar_id: &str) -> SubstrateResult<ChamberGrammar> {
        let grammars = self.grammars.read().unwrap();
        grammars
            .get(grammar_id)
            .cloned()
            .ok_or_else(|| SubstrateError::GrammarNotFound(grammar_id.to_string()))
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// Camera grammar builder
// =========================================================================

/// Build the camera sentinel grammar programmatically.
/// 4 object types, 3 primitives, preservation for event_summary + integrity_tag.
pub fn camera_sentinel_grammar() -> ChamberGrammar {
    let mut object_types = HashMap::new();

    // frame: raw camera frame metadata (payload is metadata, not pixels)
    object_types.insert(
        "frame".to_string(),
        ObjectTypeSpec {
            type_name: "frame".to_string(),
            payload_schema: serde_json::json!({"type": "object"}),
            max_payload_bytes: 1_000_000, // 1 MB for frame metadata
            transform_set: vec![],
            default_lifecycle_class: LifecycleClass::Temporary,
            can_be_preservable: false,
        },
    );

    // detection: result of running detection model on a frame
    object_types.insert(
        "detection".to_string(),
        ObjectTypeSpec {
            type_name: "detection".to_string(),
            payload_schema: serde_json::json!({"type": "object"}),
            max_payload_bytes: 100_000,
            transform_set: vec![],
            default_lifecycle_class: LifecycleClass::Temporary,
            can_be_preservable: false,
        },
    );

    // event_summary: aggregated event summary (preservable)
    object_types.insert(
        "event_summary".to_string(),
        ObjectTypeSpec {
            type_name: "event_summary".to_string(),
            payload_schema: serde_json::json!({"type": "object"}),
            max_payload_bytes: 50_000,
            transform_set: vec![Primitive::SealArtifact],
            default_lifecycle_class: LifecycleClass::Preservable,
            can_be_preservable: true,
        },
    );

    // integrity_tag: cryptographic integrity proof (preservable)
    object_types.insert(
        "integrity_tag".to_string(),
        ObjectTypeSpec {
            type_name: "integrity_tag".to_string(),
            payload_schema: serde_json::json!({"type": "object"}),
            max_payload_bytes: 10_000,
            transform_set: vec![Primitive::SealArtifact],
            default_lifecycle_class: LifecycleClass::Preservable,
            can_be_preservable: true,
        },
    );

    let mut phase_primitives = HashMap::new();
    phase_primitives.insert(
        LifecyclePhaseKey::Created,
        vec![Primitive::CreateObject],
    );
    phase_primitives.insert(
        LifecyclePhaseKey::Active,
        vec![
            Primitive::CreateObject,
            Primitive::SealArtifact,
            Primitive::TriggerBurn,
        ],
    );

    ChamberGrammar {
        grammar_id: "camera_sentinel_v1".to_string(),
        name: "Camera Sentinel".to_string(),
        description: "Camera surveillance chamber. Frames and detections are temporary. Only event summaries and integrity tags may survive burn.".to_string(),
        objective_class: "surveillance_session".to_string(),
        object_types,
        phase_primitives,
        preservable_classes: vec![
            "event_summary".to_string(),
            "integrity_tag".to_string(),
        ],
        termination_modes: vec![
            TerminationLaw {
                mode: TerminationMode::AutoBurn,
                description: "Normal session end. Burn all temporary objects.".to_string(),
                requires_artifact: false,
            },
            TerminationLaw {
                mode: TerminationMode::EmergencyBurn,
                description: "Emergency burn triggered by tamper detection.".to_string(),
                requires_artifact: false,
            },
            TerminationLaw {
                mode: TerminationMode::ManualBurn,
                description: "Manual burn triggered by user.".to_string(),
                requires_artifact: false,
            },
        ],
        permitted_links: vec![
            LinkSpec {
                link_type: "detected_in".to_string(),
                source_types: vec!["detection".to_string()],
                target_types: vec!["frame".to_string()],
            },
            LinkSpec {
                link_type: "summarizes".to_string(),
                source_types: vec!["event_summary".to_string()],
                target_types: vec!["detection".to_string()],
            },
            LinkSpec {
                link_type: "attests".to_string(),
                source_types: vec!["integrity_tag".to_string()],
                target_types: vec!["event_summary".to_string()],
            },
        ],
    }
}
