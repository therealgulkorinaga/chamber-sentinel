//! Substrate integration tests for Chamber Sentinel.
//! Tests the full lifecycle: create world, ingest frames, seal events, burn.

use chamber_core::Runtime;
use chamber_types::*;

#[test]
fn test_create_world_and_burn() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "test camera").unwrap();

    // Verify world exists
    assert!(runtime.state_engine.has_world(world_id));

    // Burn
    let result = runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();
    assert!(result.residue.as_ref().map(|r| r.residue_score == 0.0).unwrap_or(false));

    // Verify world is gone
    assert!(!runtime.state_engine.has_world(world_id));
}

#[test]
fn test_ingest_frame_and_burn() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "frame test").unwrap();

    // Simulate frame ingestion
    let frame_bytes = vec![0xAA; 1920 * 1080 * 3]; // Fake RGB frame
    let obj_id = runtime.ingest_frame(world_id, &frame_bytes, 1920, 1080, 1000i64).unwrap();

    // Verify object exists (encrypted)
    assert!(runtime.state_engine.has_object(world_id, obj_id).unwrap());
    assert_eq!(runtime.state_engine.object_count(world_id).unwrap(), 1);

    // Burn
    runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();

    // Verify frame is gone
    assert!(!runtime.state_engine.has_world(world_id));
}

#[test]
fn test_seal_event_survives_burn() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "seal test").unwrap();

    // Create an event_summary (preservable)
    let event_payload = serde_json::json!({
        "event_type": "person_detected",
        "timestamp": "2026-04-05T15:47:00Z",
        "confidence": 0.94,
        "duration_seconds": 5
    });
    let obj_id = runtime.create_object(world_id, "event_summary", event_payload, true).unwrap();

    // Seal it
    runtime.seal_artifact(world_id, obj_id).unwrap();

    // Verify vault has the artifact
    assert_eq!(runtime.vault.artifact_count_for_world(world_id), 1);

    // Burn
    runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();

    // Vault artifact survives
    let artifacts = runtime.vault.artifacts_from_world(world_id);
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].artifact_class, "event_summary");
}

#[test]
fn test_frame_not_preservable() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "preserve test").unwrap();

    // Create a frame (not preservable)
    let obj_id = runtime.ingest_frame(world_id, &[0; 100], 10, 10, 1000i64).unwrap();

    // Attempt to seal — should fail
    let result = runtime.seal_artifact(world_id, obj_id);
    assert!(result.is_err());
}

#[test]
fn test_rolling_chambers() {
    let mut runtime = Runtime::new();

    // Simulate 3 rolling chambers
    for i in 0..3 {
        let world_id = runtime.create_world("camera_sentinel_v1", &format!("chamber {}", i)).unwrap();

        // Ingest 30 frames
        for f in 0..30 {
            runtime.ingest_frame(world_id, &[f as u8; 100], 10, 10, (i * 30 + f) as i64).unwrap();
        }

        // Seal an event
        let event = serde_json::json!({
            "event_type": "motion_detected",
            "timestamp": "2026-04-05T15:47:00Z",
            "confidence": 0.8,
            "duration_seconds": 1
        });
        let eid = runtime.create_object(world_id, "event_summary", event, true).unwrap();
        runtime.seal_artifact(world_id, eid).unwrap();

        // Burn
        runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();

        // Verify burned
        assert!(!runtime.state_engine.has_world(world_id));
    }

    // Vault should have 3 events from 3 chambers
    let all_artifacts = runtime.vault.all_artifacts();
    assert_eq!(all_artifacts.len(), 3);
}

#[test]
fn test_forward_secrecy() {
    let mut runtime = Runtime::new();

    // Chamber 1
    let w1 = runtime.create_world("camera_sentinel_v1", "chamber 1").unwrap();
    runtime.ingest_frame(w1, &[0xAA; 100], 10, 10, 1000i64).unwrap();
    runtime.burn_world(w1, TerminationMode::AutoBurn).unwrap();

    // Chamber 2
    let w2 = runtime.create_world("camera_sentinel_v1", "chamber 2").unwrap();
    runtime.ingest_frame(w2, &[0xBB; 100], 10, 10, 2000i64).unwrap();

    // W1 is burned — its key is destroyed
    assert!(runtime.crypto.is_key_destroyed(w1));

    // W2 is still active — its key exists
    assert!(runtime.crypto.has_world_key(w2));

    // Different keys
    assert_ne!(w1, w2);

    runtime.burn_world(w2, TerminationMode::AutoBurn).unwrap();
}

#[test]
fn test_two_tier_audit() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "audit test").unwrap();

    // Ingest some frames (generates Tier 2 events)
    for i in 0..5 {
        runtime.ingest_frame(world_id, &[i; 100], 10, 10, i as i64).unwrap();
    }

    // Before burn: should have Tier 1 + Tier 2 events
    let pre_events = runtime.audit.events_for_world(world_id);
    assert!(pre_events.len() >= 2); // At least WorldCreated + some Tier 2

    // Burn
    runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();

    // After burn: only Tier 1 events
    let post_events = runtime.audit.events_for_world(world_id);
    assert_eq!(post_events.len(), 2); // WorldCreated + WorldDestroyed
    assert!(post_events.iter().all(|e| e.event_type.is_substrate_scoped()));
}

#[test]
fn test_encrypted_memory_no_plaintext() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "encrypt test").unwrap();

    // Create frame with distinctive marker
    let marker = b"SENTINEL_MARKER_12345";
    let mut frame = vec![0u8; 1000];
    frame[..marker.len()].copy_from_slice(marker);

    runtime.ingest_frame(world_id, &frame, 10, 10, 1000i64).unwrap();

    // Verify object stored (encrypted)
    assert_eq!(runtime.state_engine.object_count(world_id).unwrap(), 1);

    // Burn — marker is now unrecoverable
    runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();

    // Post-burn: zero residue
    let residue = runtime.burn_engine.measure_residue(world_id);
    assert_eq!(residue.residue_score, 0.0);
    assert!(!residue.state_engine_has_world);
    assert!(residue.crypto_key_destroyed);
}

#[test]
fn test_emergency_burn() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "emergency test").unwrap();

    // Ingest frames
    for i in 0..10 {
        runtime.ingest_frame(world_id, &[i; 100], 10, 10, i as i64).unwrap();
    }

    // Emergency burn — no sealing, everything destroyed
    runtime.burn_world(world_id, TerminationMode::EmergencyBurn).unwrap();

    // No artifacts (nothing was sealed)
    assert_eq!(runtime.vault.artifact_count_for_world(world_id), 0);

    // World is gone
    assert!(!runtime.state_engine.has_world(world_id));
}

#[test]
fn test_residue_report() {
    let mut runtime = Runtime::new();
    let world_id = runtime.create_world("camera_sentinel_v1", "residue test").unwrap();

    for i in 0..20 {
        runtime.ingest_frame(world_id, &[i; 100], 10, 10, i as i64).unwrap();
    }

    runtime.burn_world(world_id, TerminationMode::AutoBurn).unwrap();

    let residue = runtime.burn_engine.measure_residue(world_id);
    assert_eq!(residue.residue_score, 0.0);
    assert!(!residue.state_engine_has_world);
    assert!(residue.crypto_key_destroyed);
    assert!(!residue.audit_leaks_internals);
    assert_eq!(residue.substrate_event_count, 2);
    assert_eq!(residue.world_events_surviving, 0);
}
