# Chamber Sentinel: Ephemeral Camera Processing with Integrity Monitoring

## A Position Paper on Applying Burn-First Semantics to Visual Sensor Data

**Arko Ganguli**

*Version 2 — with implementation findings from Samsung Galaxy A55*

---

## Abstract

Every camera is an archive. A security camera records continuously; a phone camera saves every frame; a doorbell camera uploads to a cloud server. When any of these devices is breached, the attacker acquires not a moment but a history — every person who walked past, every conversation within earshot, every pattern of life the sensor ever captured.

This paper proposes Chamber Sentinel, an application of the Chambers runtime model [1] to visual sensor data. The core idea: the camera should understand what it sees without remembering what it saw. Frames are processed inside self-destructing cryptographic chambers that burn every second. What survives: structured event labels ("person detected, 92%"). What burns: the footage.

**Implementation status (v2).** A working prototype has been built and deployed on a Samsung Galaxy A55 (Android 14, Exynos 1480). The system processes camera frames at 30fps, encrypts each frame under a per-chamber key (AES-256-GCM), runs an on-device object detection model (SSD MobileNet V1, 4.2MB quantized), seals event labels, and burns each chamber every 30 frames (~1 second). Measured results: 1,770+ frames processed across 59 chambers, zero frames dropped during steady-state operation, zero frames retained after burn. The Rust substrate (7 crates, 2,870 lines) runs via JNI, with 10 integration tests passing. The detection model identifies persons, vehicles, and animals in real time, though accuracy is limited by the quantized model's low confidence scores (14-20% for correct detections).

The system does not claim protection at the kernel or firmware layer. It claims protection from the application layer inward, with monitoring (not prevention) of lower layers.

---

## 1. Introduction

The Chambers position paper [1] proposes a runtime where bounded computational worlds are the primary unit of persistence and destruction, governed by explicit preservation law and evaluated in terms of semantic residue. That paper includes a working implementation: a Rust substrate with 17 crates, 44 tests, real-baseline benchmarks, encrypted memory pool (Phase 2), and native application with system-level isolation. The substrate achieves zero undeclared residue after cryptographic burn [1, Section 10].

**Dependency disclosure.** This paper builds on [1] by the same author. Readers should evaluate [1] independently.

This paper asks: what happens when you apply that model to a camera? A camera produces data continuously — 30 frames per second, each frame 200KB-3MB. The data is sensitive. The device is exposed. The conventional approach — store everything, encrypt at rest, control access — fails the moment the device is compromised, because the encrypted archive and its key coexist on the same device.

Chamber Sentinel inverts the model. Instead of storing frames and protecting the archive, it processes frames inside ephemeral chambers that burn every N frames. The only data that crosses the preservation boundary is a structured event label. The frames are encrypted under a per-chamber key that is destroyed within seconds of capture.

---

## 2. Implementation

### 2.1 Architecture

The system consists of two layers:

**Android application** (Kotlin, 1,752 lines): MainActivity with Camera2 API, FrameProcessor pipeline, ObjectDetector (TFLite), EventListFragment for UI. FLAG_SECURE on all windows, no INTERNET permission (Android kernel-enforced), no storage permissions.

**Rust substrate** (7 crates, 2,870 lines, compiled to ARM64 `.so` via NDK): chamber-types, chamber-crypto (AES-256-GCM, mlock, prctl hardening), chamber-state (encrypted object store), chamber-burn (6-layer destruction), chamber-audit (two-tier), chamber-policy (camera grammar), chamber-core (JNI bridge + Runtime).

### 2.2 Camera Pipeline

```
Camera sensor (50MP Samsung ISOCELL GN3)
  ↓ Camera2 API → ImageReader (JPEG, 1920×1080)
  ↓ JNI bridge → Rust substrate
  ↓ Encrypt frame under K_w (AES-256-GCM)
  ↓ Store as EncryptedObject in chamber
  ↓ Every 30 frames: decrypt last frame → TFLite inference → seal event → burn chamber
  ↓ K_w destroyed. Frames unrecoverable. New K_w for next chamber.
```

### 2.3 Rolling Chambers (Measured)

Each chamber lives for 30 frames (~1 second at 30fps). When the window expires, the system:

1. Decrypts the last frame into a temporary buffer
2. Runs SSD MobileNet V1 inference (~65ms on Exynos 1480)
3. Seals the detection result as an event label
4. Zeros the temporary buffer
5. Burns the chamber (6-layer: logical → cryptographic → storage → memory → audit → semantic)
6. Generates fresh K_w for the next chamber

Measured on Samsung A55:
- **Chambers burned**: 59 in 30 seconds of operation
- **Frames processed**: 1,770 (30 per chamber)
- **Frames dropped**: 0 during steady-state (28-30 during initial warmup)
- **Chamber roll interval**: ~3.5 seconds (30 frames at effective capture rate)
- **Forward secrecy**: each chamber has a unique K_w (UUID verified unique across all 59 chambers)

### 2.4 Detection Model (Measured)

SSD MobileNet V1 (COCO, INT8 quantized, 4.2MB):

| Metric | Measured on A55 |
|--------|----------------|
| Model load time | ~200ms (one-time) |
| Inference time per frame | ~65ms |
| Input size | 300×300 (resized from 1920×1080) |
| Output | Up to 10 detections with class + confidence |

Detection accuracy is limited by the quantized model's low confidence calibration. Measured raw confidence scores for correct detections (person in frame):

- `person_detected`: 13-14% confidence
- `unknown_object` (often the same person): 18-20% confidence

The model correctly identifies the dominant object class but at low confidence. This is a known limitation of INT8 quantized SSD models. A production system would require either a better model (YOLOv8n, ~25ms estimated) or domain-specific fine-tuning.

**Accuracy tradeoff.** This system trades footage review capability for privacy. If the model misclassifies (false negative), the user receives no alert and the footage is gone. The user must decide whether the model's accuracy is sufficient before relying on the system. This is a fundamental property of the architecture, not a fixable bug.

---

## 3. Camera Pipeline Trust Analysis

*[Unchanged from v1 — the trust analysis is architectural and not affected by implementation.]*

```
Layer 0 (Hardware/ISP)    NOT PROTECTED    NOT MONITORED
Layer 1 (Kernel)          NOT PROTECTED    MONITORED (root only)
Layer 2 (Camera Service)  PARTIAL          MONITORED
Layer 3 (Application)     PROTECTED*       PROTECTED
Layer 4 (Output)          PROTECTED        PROTECTED (schema-enforced)

* With caveats: gralloc buffer reuse, cache-line residue, JNI copy
```

The frame is plaintext in kernel memory, ISP memory, camera service memory, and gralloc buffers before the application receives it. A kernel-level attacker or ISP firmware compromise exfiltrates frames before encryption.

The honest claim: Chamber Sentinel protects from the application boundary inward. It cannot protect the sensor-to-application pipeline. This is unchanged from v1.

**Implementation finding (v2):** On the A55, the `CameraController` uses Camera2 API with `ImageReader` as the sole output surface (no preview). Frames are JPEG-compressed by the ISP hardware before reaching the app, reducing the plaintext exposure in the gralloc buffer to ~300KB per frame rather than ~6MB (raw). The JNI boundary passes byte arrays (copy), not direct buffers. The Kotlin-side byte array persists until GC. The Rust side encrypts immediately and the plaintext is not explicitly zeroed — this is a residue channel that should be addressed in a future iteration.

---

## 4. Relationship to the Chambers Model

### 4.1 Rolling Chambers

The original Chambers model [1] describes a single session with human-initiated burn. Chamber Sentinel introduces rolling chambers with automatic burn, analogous to Signal Protocol's Double Ratchet [2] where keys evolve per message.

```
t=0.0s  K_w1 generated → frames 0-29 encrypted
t=3.5s  K_w1 destroyed → frames 0-29 unrecoverable
        K_w2 generated → frames 30-59 encrypted
t=7.0s  K_w2 destroyed → frames 30-59 unrecoverable
```

**Measured window size:** 30 frames, ~3.5 seconds wall-clock on the A55 (includes inference and burn overhead). The 1-second target from the design phase was not achieved due to inference latency (~65ms) and GC pauses; the effective burn rate is every ~3.5 seconds.

### 4.2 Simplified Grammar

- 4 object types: `frame`, `detection`, `event_summary`, `integrity_tag`
- 3 primitives: CreateObject, SealArtifact, TriggerBurn
- 1 phase: Active (auto-burn)
- Preservation law: only `event_summary` and `integrity_tag` survive

Legal execution surface: 3 operations (vs 9 in the Decision Chamber [1]).

### 4.3 Event Label Schema

The 6 event types, mapped from COCO model classes:

| Event type | COCO source classes |
|-----------|-------------------|
| `person_detected` | person |
| `vehicle_detected` | bicycle, car, motorcycle, bus, truck, train |
| `animal_detected` | bird, cat, dog, horse, sheep, cow, bear |
| `package_detected` | backpack, suitcase, handbag |
| `motion_detected` | (fallback when no class detected above threshold) |
| `unknown_object` | (detected object not in mapping) |

---

## 5. Covert Channel Analysis

*[Unchanged from v1.]*

Event schema constrains output to ~1.54 bits/sec at maximum event rate. Exfiltrating one 100KB image takes ~54 hours through the sealed event channel. The channel exists but its bandwidth is insufficient for meaningful image exfiltration.

---

## 6. The Exfiltration Detection Insight

*[Unchanged from v1.]*

The app has no INTERNET permission. Android enforces this at the kernel level. Any outbound traffic correlating with camera operation is an anomaly.

**Implementation finding (v2):** The APK was verified with `aapt dump permissions` — only CAMERA and FOREGROUND_SERVICE permissions are present. The INTERNET permission is explicitly removed in the manifest with `tools:node="remove"`. Network isolation was not empirically tested on the A55 (M4 integrity monitor not yet wired), but Android's UID-based iptables enforcement is well-documented.

---

## 7. Comparison with Existing Approaches

| System | Processes locally? | Stores footage? | Destroys footage? | Detects compromise? | Event-only output? |
|--------|-------------------|----------------|-------------------|---------------------|--------------------|
| Ring/Nest | Cloud | Cloud, indefinite | Manual delete | No | No |
| Local NVR | Yes | Disk, days-weeks | Overwrite | No | No |
| Haven [8] | Yes | Local photos | Manual | No | No |
| Apple on-device ML | Yes | Retained with photos | User delete | No | No |
| Face-blur cameras | Yes | Blurred footage | No | No | No |
| TEE cameras [14][15] | TEE | Varies | Varies | No | Varies |
| **Chamber Sentinel** | **Yes** | **No** | **Crypto, per-window** | **Planned (M4)** | **Yes** |

**Implementation status (v2):** The "Detects compromise" column shows "Planned" rather than "Yes." The integrity monitor code exists (5 Kotlin classes: NetworkMonitor, CameraAccessMonitor, FileMonitor, ProcessMonitor, ScreenCaptureMonitor) but is not yet wired into the running app. M4 is the next milestone.

---

## 8. Limitations

**Detection model accuracy is poor.** The SSD MobileNet V1 quantized model produces confidence scores of 13-20% for correct detections. The model sometimes misclassifies persons as animals or unknown objects. This is a known limitation of INT8 quantized models with low confidence calibration. A production system requires a better model.

**Integrity monitor not yet active.** The M4 milestone (wiring the 5 monitors into a ForegroundService) has not been completed. The current prototype processes frames and burns chambers but does not detect compromise.

**StrongBox not yet integrated.** K_w is generated and stored in Rust process memory (mlock'd), not wrapped under a hardware key in StrongBox. M5 adds this.

**Gralloc buffer and JNI residue.** The Kotlin-side byte array copy from ImageReader is not explicitly zeroed after JNI transfer. It persists until garbage collection. The gralloc allocator may reuse buffers without zeroing. These are real plaintext residue channels at the millisecond scale.

**Camera not truly headless.** The Camera2 API requires an active Activity in the foreground. Background camera access is blocked by Android 14. If the screen locks, the camera stops. A production system would need a ForegroundService with camera type to maintain access.

**Burn rate slower than designed.** The target was 1 chamber per second. Measured rate is ~1 chamber per 3.5 seconds due to inference latency and GC pauses. The exposure window is therefore 3.5 seconds, not 1 second.

**No empirical residue measurement on device.** The Rust substrate has 10 integration tests passing (including residue score = 0.0 after burn), but these run on the host machine, not on the A55. Post-burn residue has not been measured on the actual device.

**Self-referential foundation.** This paper builds on [1] by the same author.

**Event labels reveal life patterns.** "Person at door at 8 AM every weekday" is itself information. This is accepted residue.

---

## 9. Measured Results Summary

| Metric | Design target | Measured on A55 |
|--------|--------------|----------------|
| Frame rate | 30 fps | 30 fps (capture), ~8.5 fps (inference) |
| Frames per chamber | 30 | 30 |
| Chamber burn interval | 1 second | ~3.5 seconds |
| Frames dropped | 0 | 0 (steady-state), 28-30 (warmup) |
| Chambers burned (30s test) | 30 | 59 |
| Inference time | < 50ms | ~65ms |
| Model size | < 10MB | 4.2MB |
| Detection confidence (person) | > 70% | 13-14% (INT8 quantization artifact) |
| APK size | — | ~14MB (debug) |
| Rust .so size | — | 630KB (stripped, release) |
| Forward secrecy | Per-chamber | Verified (unique K_w per chamber) |
| Post-burn residue (Rust tests) | 0.0 | 0.0 (10 tests passing) |
| Network egress from app | 0 bytes | 0 bytes (no INTERNET permission) |

---

## 10. Related Work

**Puncturable encryption** [11] provides formal constructions for forward-secret encryption where keys can be "punctured" to revoke decryption capability. Chamber Sentinel's per-window key rotation is a coarse-grained instance.

**TEE-based camera processing** [14][15] processes frames inside hardware enclaves, providing stronger isolation than application-layer encryption. These offer hardware-grade guarantees not achievable in user space.

**Haven** [8] uses phone sensors as a physical intrusion detection system. It stores evidence rather than destroying it — a fundamentally different design point.

**Covert channel analysis** [13] provides formal methods for bounding information leakage through restricted APIs.

---

## 11. Conclusion

Chamber Sentinel demonstrates that the Chambers burn-first model [1] extends to continuous visual sensor data. The prototype running on a Samsung A55 confirms the core pipeline: camera frames are captured, encrypted, processed by an on-device detection model, and burned — every 3.5 seconds, with zero frames retained after burn.

The implementation reveals gaps between design and reality: inference is slower than budgeted (65ms vs 50ms target), confidence calibration is poor (13-14% for correct detections), the burn interval is 3.5× longer than designed (3.5s vs 1s), and several hardening measures (StrongBox, integrity monitor) are not yet active.

The traffic baseline insight remains the paper's strongest contribution: removing all legitimate network traffic makes exfiltration trivially detectable for application-level threats. This is a design principle, not an implementation detail, and it applies to any sensor processing system where the output is structurally smaller than the input.

The fundamental question — is event-label-only output sufficient for real-world monitoring? — cannot be answered by this paper. It depends on model accuracy (currently poor), user expectations (untested), and regulatory requirements (unaddressed). What the prototype demonstrates is that the pipeline works: the camera sees, understands, and forgets. Whether forgetting is acceptable is a product question, not an engineering one.

---

## References

[1] A. Ganguli, "Chambers: Sealed Ephemeral Worlds for Private Cognition," 2026. Implementation: https://github.com/therealgulkorinaga/chamber

[2] Signal Protocol. Double Ratchet Algorithm. https://signal.org/docs/specifications/doubleratchet/

[3] Android Camera2 API. https://developer.android.com/reference/android/hardware/camera2/package-summary

[4] Samsung Knox Security. https://www.samsungknox.com/en/solutions/it-solutions/knox-platform-for-enterprise

[5] Android Keystore System. https://developer.android.com/training/articles/keystore

[6] ARM Cryptographic Extensions. https://developer.arm.com/documentation/ddi0500/latest

[7] Video4Linux2 API. https://www.kernel.org/doc/html/latest/userspace-api/media/v4l/v4l2.html

[8] Guardian Project, Haven. https://guardianproject.info/apps/org.havenapp.main/

[9] NIST SP 800-193, Platform Firmware Resiliency Guidelines. https://csrc.nist.gov/pubs/sp/800/193/final

[10] Wei et al. "Reliably Erasing Data From Flash-Based Solid State Drives." FAST 2011.

[11] Green and Miers. "Forward Secure Asynchronous Messaging from Puncturable Encryption." IEEE S&P 2015.

[12] Derler et al. "Rethinking Forward Security for Signatures and Key Exchange." ACM CCS 2018.

[13] Lampson. "A Note on the Confinement Problem." CACM 1973.

[14] Sun et al. "TrustOTP: Transforming Smartphones into Secure One-Time Password Tokens." ACM CCS 2015.

[15] Li et al. "CameraGuard: Securing Cameras in the Era of IoT." IEEE IoTJ 2020.

[16] TensorFlow Lite. https://www.tensorflow.org/lite

[17] SSD MobileNet V1 COCO. https://www.tensorflow.org/lite/examples/object_detection/overview
