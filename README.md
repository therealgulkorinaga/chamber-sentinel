# Chamber Sentinel

An Android application that processes live camera frames inside cryptographically bounded, self-destructing "chambers" — extracting only semantic event labels and immediately destroying all raw footage. No video is ever stored. No network traffic is ever sent. The camera understands what it sees without remembering what it saw.

---

## Table of Contents

- [What Is This?](#what-is-this)
- [Core Principles](#core-principles)
- [Architecture Overview](#architecture-overview)
- [Repository Layout](#repository-layout)
- [Android Application Layer](#android-application-layer)
  - [MainActivity](#mainactivity)
  - [Camera Subsystem](#camera-subsystem)
  - [Detection Subsystem](#detection-subsystem)
  - [Integrity Monitors](#integrity-monitors)
  - [UI Components](#ui-components)
- [Rust Substrate](#rust-substrate)
- [JNI Bridge](#jni-bridge)
- [ML Model](#ml-model)
- [Permissions & Security Hardening](#permissions--security-hardening)
- [Build Requirements](#build-requirements)
- [Building from Source](#building-from-source)
- [Running on Device](#running-on-device)
- [Project Status & Milestones](#project-status--milestones)
- [Documentation](#documentation)

---

## What Is This?

Chamber Sentinel is a privacy-first, on-device security camera application for Android. It uses the device camera to detect events — people, vehicles, animals, packages — but never stores, uploads, or logs the underlying video footage. Instead, every 30 frames (approximately 1 second), the entire encrypted frame buffer is cryptographically destroyed in a six-layer burn protocol, leaving behind only a sealed audit record: what was detected, when, and whether the pipeline was tampered with.

The application was designed around a single threat model question: *can a camera application provide useful situational awareness without creating a surveillance record that can be subpoenaed, leaked, or abused?* Chamber Sentinel's answer is to make retention structurally impossible rather than just policy-prohibited.

---

## Core Principles

**Ephemeral by design.** Raw frames are encrypted under hardware-backed AES-256-GCM keys (Android StrongBox) the moment they leave the camera sensor. They exist only long enough to be fed to the on-device detection model. After 30 frames they are destroyed — keys wiped, memory zeroed, file handles closed — regardless of what was detected.

**No network, ever.** The `INTERNET` permission is explicitly removed at the manifest level using `tools:node="remove"`. This is not a runtime check. There is no code path that could open a socket even if a developer added one accidentally, because the OS will refuse it.

**Only semantics survive.** The output of a chamber cycle is not a frame, a thumbnail, or a video clip. It is a sealed artifact: a structured record containing a detection label (`person_detected`, `vehicle_detected`, etc.), a confidence score, a timestamp, and an integrity tag. Bounding boxes are discarded. Images are discarded. The artifact is cryptographically signed so any tampering with it after the fact is detectable.

**Continuous integrity monitoring.** Five independent monitors run concurrently as a foreground service, watching for signs that the pipeline has been compromised: unexpected network traffic, another app accessing the camera, suspicious file creation in media directories, debugger/ptrace attachment, and virtual displays being created by screen recording tools. Any violation is sealed into the audit log as an immutable forensic record.

**No screenshots, no backups.** `FLAG_SECURE` is set on the application window. Cloud backup is disabled at the manifest level. The app does not support RTL layouts (reducing attack surface from bidirectional text rendering).

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Android Application                       │
│                                                             │
│  ┌──────────────┐   ┌────────────────┐   ┌──────────────┐  │
│  │ CameraControl│──▶│ FrameProcessor │──▶│ ChamberRuntime│  │
│  │ (Camera2 API)│   │ (single thread)│   │ (Kotlin wrap) │  │
│  └──────────────┘   └────────────────┘   └──────┬───────┘  │
│                                                  │ JNI      │
│  ┌──────────────┐   ┌────────────────┐   ┌──────▼───────┐  │
│  │ObjectDetector│   │IntegrityMonitor│   │ ChamberBridge │  │
│  │ (TFLite INT8)│   │  (5 monitors)  │   │ (native decl) │  │
│  └──────────────┘   └────────────────┘   └──────┬───────┘  │
└─────────────────────────────────────────────────┼───────────┘
                                                  │
┌─────────────────────────────────────────────────▼───────────┐
│                    Rust Substrate (libchamber_core.so)        │
│                                                              │
│  chamber-core  ──  chamber-crypto  ──  chamber-state        │
│  chamber-burn  ──  chamber-audit   ──  chamber-policy       │
│                         chamber-types                        │
└──────────────────────────────────────────────────────────────┘
```

**Data flow per chamber cycle:**

1. `CameraController` captures JPEG frames headlessly (no preview surface).
2. `FrameProcessor` forwards each frame to `ChamberRuntime.ingestFrame()`.
3. The Rust substrate encrypts the frame under a freshly derived AES-256-GCM key stored in Android StrongBox hardware.
4. After 30 frames, `ObjectDetector` runs TFLite inference on the last frame and returns the top detection label.
5. `ChamberRuntime.sealArtifact()` is called with the detection label. The Rust substrate writes a sealed artifact record (label + confidence + timestamp + integrity tag).
6. `ChamberRuntime.burn()` destroys the chamber: keys are zeroized, encrypted frame memory is overwritten six times, all handles are closed.
7. The sealed artifact surfaces in the UI event list. The raw frame is gone.

---

## Repository Layout

```
chamber-mobileandroid/
│
├── app/                                  # Android application module
│   ├── src/main/
│   │   ├── AndroidManifest.xml           # Permissions, activities, services
│   │   ├── java/com/chamber/sentinel/
│   │   │   ├── MainActivity.kt           # Entry point, chamber lifecycle
│   │   │   ├── ChamberRuntime.kt         # Kotlin wrapper around Rust substrate
│   │   │   ├── ChamberBridge.kt          # JNI native method declarations
│   │   │   ├── camera/
│   │   │   │   ├── CameraController.kt   # Camera2 API lifecycle management
│   │   │   │   └── FrameProcessor.kt     # Single-threaded frame dispatch
│   │   │   ├── detection/
│   │   │   │   └── ObjectDetector.kt     # TFLite inference engine wrapper
│   │   │   ├── integrity/
│   │   │   │   ├── IntegrityMonitor.kt   # Foreground service orchestrator
│   │   │   │   ├── NetworkMonitor.kt     # Traffic anomaly detection
│   │   │   │   ├── CameraAccessMonitor.kt# Concurrent camera access detection
│   │   │   │   ├── FileMonitor.kt        # Media directory watch (FileObserver)
│   │   │   │   ├── ProcessMonitor.kt     # ptrace/debugger detection via /proc
│   │   │   │   └── ScreenCaptureMonitor.kt # Virtual display / screen recording
│   │   │   └── ui/
│   │   │       ├── EventListFragment.kt  # Live audit event list (RecyclerView)
│   │   │       └── IntegrityDashboard.kt # Integrity status dashboard
│   │   ├── assets/
│   │   │   └── detect_model.tflite       # INT8 quantized detection model
│   │   ├── jniLibs/arm64-v8a/
│   │   │   └── libchamber_core.so        # Pre-built Rust substrate (ARM64)
│   │   └── res/                          # Layouts, themes, strings, drawables
│   └── build.gradle.kts                  # Module-level build config
│
├── rust/                                 # Rust substrate workspace
│   ├── Cargo.toml                        # Workspace manifest
│   └── crates/
│       ├── chamber-core/                 # JNI bridge, runtime orchestration
│       ├── chamber-crypto/               # AES-256-GCM, mlock, zeroize
│       ├── chamber-state/                # World & artifact state engine
│       ├── chamber-burn/                 # 6-layer destruction protocol
│       ├── chamber-audit/                # Audit logging, residue measurement
│       ├── chamber-policy/               # Grammar engine, preservation laws
│       └── chamber-types/                # Shared types (WorldId, ArtifactId, …)
│
├── docs/
│   ├── PRD.md                            # Product requirements (5,500+ words)
│   └── ISSUES.md                         # 127-issue tracker across 6 milestones
│
├── Chamber_Sentinel_Position_Paper.pdf   # Peer-reviewed academic white paper
├── build.gradle.kts                      # Root build config
├── gradle.properties                     # Gradle JVM args, AndroidX flags
└── settings.gradle.kts                   # Module inclusion
```

---

## Android Application Layer

### MainActivity

**`MainActivity.kt`** is the application entry point and orchestrates the top-level chamber lifecycle loop.

On startup it:
- Requests the `CAMERA` runtime permission and blocks until granted.
- Sets `FLAG_SECURE` on the window (prevents screenshots and screen recording previews).
- Initializes `ChamberRuntime` (loads `libchamber_core.so`, creates the first World).
- Starts `IntegrityMonitor` as a foreground service.
- Opens the camera via `CameraController` and begins frame delivery.

Per chamber cycle (every 30 frames) it:
1. Calls `ObjectDetector.detect()` on the last received frame.
2. Calls `runtime.sealArtifact()` with the detection result.
3. Calls `runtime.burn()` to destroy the chamber.
4. Creates a new World for the next cycle.
5. Posts the sealed event to `EventListFragment` for display.

Rotation and configuration changes are handled with `android:configChanges="orientation|screenSize"` to avoid recreating the camera pipeline unnecessarily.

---

### Camera Subsystem

**`CameraController.kt`** uses the Camera2 API to capture JPEG frames from the rear-facing camera without displaying a preview.

Key design decisions:
- **No SurfaceTexture or preview surface.** Eliminating the preview surface means the raw frame never touches the framebuffer or GPU memory that could be accessed by screen capture tools.
- **Headless JPEG capture via ImageReader.** Frames are delivered as byte arrays directly to `FrameProcessor`.
- **Background HandlerThread.** The camera callback and image reader run on a dedicated background thread, keeping the main thread free for UI.
- **Default resolution: 1920×1080.** Configurable at construction time.

**`FrameProcessor.kt`** receives frames from `CameraController` and dispatches them to the Rust substrate via `ChamberRuntime.ingestFrame()`. It maintains a single-threaded executor to serialize frame processing and tracks per-session frame counts and drop counts (frames dropped when the executor is busy).

---

### Detection Subsystem

**`ObjectDetector.kt`** wraps a TensorFlow Lite interpreter and maps raw inference output to one of six semantic event types.

**Model details:**
- Format: TFLite INT8 quantized (stored at `assets/detect_model.tflite`)
- Input: 300×300 RGB bitmap
- Backbone: EfficientDet-Lite0 / SSD MobileNet V1 variant
- Output: 90 COCO class label scores + bounding boxes
- Inference threads: 2
- Estimated latency on Exynos 1480 (Samsung A55): 15–25 ms

**Class mapping (90 COCO → 6 semantic labels):**

| Semantic Label | COCO Classes Included |
|---|---|
| `person_detected` | person |
| `vehicle_detected` | car, truck, bus, motorcycle, bicycle, airplane, boat, train |
| `animal_detected` | cat, dog, horse, cow, elephant, bear, zebra, giraffe, bird, … |
| `package_detected` | backpack, handbag, suitcase |
| `motion_detected` | fallback for any object above confidence threshold |

Confidence threshold is set at **10%** to minimize false negatives in low-light or partially obscured scenes. Bounding box coordinates are decoded internally and then **discarded** — the artifact record contains only the label and confidence score. Nothing about the spatial position of the detected object is preserved.

---

### Integrity Monitors

**`IntegrityMonitor.kt`** is a foreground service that starts five concurrent monitors when the application launches. Each monitor runs its own polling loop or registers an OS callback and reports violations via a callback to `MainActivity`, which seals them as integrity events in the audit log.

---

#### NetworkMonitor

Polls `TrafficStats.getTotalTxBytes()` / `getRxBytes()` every **500 ms**.

Because the `INTERNET` permission is removed from the manifest, any non-zero traffic reading is anomalous by definition. If the OS reports any transmitted or received bytes beyond baseline, a `network_anomaly` violation is fired.

This catches exfiltration attempts by compromised system services, root-level processes, or VPN tap interfaces that bypass the application permission model.

---

#### CameraAccessMonitor

Registers a `CameraManager.AvailabilityCallback` to receive OS notifications when any application opens or closes the device camera.

Fires a `camera_hijack` violation when:
- The camera becomes unavailable (another app opened it) while Sentinel is not actively rotating a chamber.
- Camera access priority shifts unexpectedly (e.g., a system process pre-empts the camera pipeline).

This detects attempts by malicious apps to silently record alongside Sentinel or intercept the camera feed.

---

#### FileMonitor

Attaches `FileObserver` instances to the following directories:
- `Environment.DIRECTORY_DCIM`
- `Environment.DIRECTORY_PICTURES`
- `/data/local/tmp`

Watches for `CREATE` and `MODIFY` events. Fires a `filesystem_anomaly` violation if a new file matches patterns associated with screen capture or recording:
- Extensions: `.png`, `.jpg`, `.jpeg`, `.mp4`, `.webm`, `.mkv`
- Name keywords: `screenshot`, `screencast`, `screen_record`

This provides a secondary detection layer for screen recording tools that write to external storage.

---

#### ProcessMonitor

Reads `/proc/self/status` every **2 seconds** and checks two fields:

- **`TracerPid`**: If non-zero, a debugger is attached via ptrace. Fires `debugger_detected` immediately.
- **Thread count** (parsed from `/proc/self/task`): If the thread count spikes by 5 or more since the last reading, a `thread_injection` violation is fired. Sudden thread count increases are a signature of dynamic library injection or method hooking frameworks like Frida.

---

#### ScreenCaptureMonitor

Registers a `DisplayManager.DisplayListener` and inspects all active displays.

Fires a `screen_capture_detected` violation when a non-default, non-private display appears. Virtual displays of this type are created by the Android `MediaProjection` API when an application starts screen recording or screen casting. This catches screen recording before it can capture any sensitive frames from the UI.

---

### UI Components

**`EventListFragment.kt`** renders a live, reverse-chronological list of sealed audit events in a `RecyclerView`. The layout is constructed programmatically (no XML inflation). Each row shows the event label, timestamp, confidence score (for detection events), and an integrity status indicator (clean or compromised). New events trigger an automatic scroll-to-bottom. An empty-state message is shown when no events have been sealed yet.

**`IntegrityDashboard.kt`** provides a summary view with:
- Per-monitor status indicators (green/red)
- Cumulative network bytes (TX/RX) since session start
- Chamber cycle count
- Sealed event breakdown by label type

---

## Rust Substrate

The Rust substrate is a native library (`libchamber_core.so`) compiled for `aarch64-linux-android` that implements all cryptographic and state management operations. It is organized as a Cargo workspace with seven crates.

### `chamber-types`

Shared type definitions used across all crates. Key types:

| Type | Description |
|---|---|
| `WorldId` | UUID v4 identifying a chamber instance |
| `ArtifactId` | UUID v4 identifying a sealed event record |
| `ObjectId` | UUID v4 identifying a detected object within a world |
| `LifecyclePhase` | Enum: `Active`, `Sealing`, `Burning`, `Residue` |
| `DetectionLabel` | Enum: `PersonDetected`, `VehicleDetected`, `AnimalDetected`, … |
| `IntegrityStatus` | `Clean` or `Compromised(Vec<ViolationType>)` |

### `chamber-crypto`

AES-256-GCM encryption with hardware-backed key storage.

- Keys are generated using the OS CSPRNG (`rand::thread_rng` seeded from `/dev/urandom`).
- Encrypted frame buffers are `mlock`-ed to prevent swapping to disk.
- On destruction, key material is overwritten with `zeroize` before deallocation.
- Interfaces with Android Keystore via the JNI layer for StrongBox-backed key operations where available.

### `chamber-state`

Manages the lifecycle of World and Artifact objects.

A **World** is an active chamber: it holds a collection of encrypted frame slots and accumulates detection events. A World transitions through phases — `Active → Sealing → Burning → Residue` — and cannot revert to an earlier phase.

An **Artifact** is a sealed record produced at the end of the `Sealing` phase. It is immutable after creation.

### `chamber-burn`

Implements the six-layer destruction protocol executed on every chamber at the end of its lifecycle:

1. **Key destruction** — Zeroize all AES-256-GCM key bytes in memory.
2. **Ciphertext overwrite** — Overwrite encrypted frame buffers with random bytes.
3. **Memory zeroing** — Zero all buffer allocations with `zeroize`.
4. **munlock** — Release mlock on all previously locked memory regions.
5. **Handle invalidation** — Drop all file handles and OS resource references.
6. **Residue measurement** — Walk memory regions and confirm no recognizable frame data remains. Record residue score in audit log.

### `chamber-audit`

Append-only audit log stored in application-private storage. Each entry is a JSON record with:
- `world_id`: Which chamber this event belongs to
- `artifact_id`: The sealed artifact ID (for detection events)
- `event_type`: Detection label or integrity violation type
- `timestamp`: RFC 3339 timestamp
- `integrity_status`: Clean or compromised at time of sealing
- `residue_score`: Post-burn residue measurement (0.0 = clean)
- `signature`: HMAC over the record fields (tamper detection)

### `chamber-policy`

A grammar engine that enforces preservation laws — rules about which types of data may survive a burn cycle. The policy layer ensures that artifact records conform to the schema (only labels and metadata, never raw pixel data or bounding boxes) before they are committed to the audit log. Any artifact that fails policy validation is rejected and the violation is recorded.

### `chamber-core`

The JNI bridge crate. Exports C-compatible symbols that `ChamberBridge.kt` loads via `System.loadLibrary("chamber_core")`. Manages the lifecycle of a `Runtime` singleton that owns all active Worlds and the Audit log handle. Maps JNI method signatures to Rust function calls.

---

## JNI Bridge

**`ChamberBridge.kt`** declares the native method signatures that map to the Rust substrate:

| Kotlin Method | Rust Symbol | Description |
|---|---|---|
| `nativeInit()` | `Java_..._nativeInit` | Initialize the Runtime singleton |
| `nativeDestroy()` | `Java_..._nativeDestroy` | Tear down the Runtime and flush audit log |
| `nativeVersion()` | `Java_..._nativeVersion` | Return substrate version string |
| `createWorld()` | `Java_..._createWorld` | Allocate a new World, return WorldId string |
| `ingestFrame(worldId, frameBytes)` | `Java_..._ingestFrame` | Encrypt and store one JPEG frame |
| `submitCreateObject(worldId, label, confidence)` | `Java_..._submitCreateObject` | Register a detection within a World |
| `submitSealArtifact(worldId)` | `Java_..._submitSealArtifact` | Transition World to Sealing, write Artifact |
| `burn(worldId)` | `Java_..._burn` | Execute 6-layer burn protocol |
| `getResidueReport(worldId)` | `Java_..._getResidueReport` | Return post-burn residue measurement as JSON |

**`ChamberRuntime.kt`** wraps `ChamberBridge` with a higher-level Kotlin API, manages the native pointer lifecycle (ensuring `nativeDestroy` is called on application exit), and provides thread-safe access to the substrate from the main activity.

---

## ML Model

The detection model (`assets/detect_model.tflite`) is an INT8 quantized TensorFlow Lite model.

| Attribute | Value |
|---|---|
| Format | TFLite FlatBuffer |
| Quantization | INT8 post-training quantization |
| Input shape | `[1, 300, 300, 3]` (batch, height, width, RGB) |
| Output | Class scores (90 COCO labels) + bounding boxes |
| Inference threads | 2 |
| Target device | Exynos 1480 (Samsung A55), Android 12+ |
| Estimated latency | 15–25 ms per frame |
| Storage | Uncompressed in APK (`aaptOptions.noCompress += "tflite"`) |

The model is loaded once at application start via `Interpreter(FileUtil.loadMappedFile(context, "detect_model.tflite"), options)` and reused across all chamber cycles.

The 90 COCO output classes are mapped to 6 semantic labels by `ObjectDetector`. Only the top-1 result above the confidence threshold is used. If no class exceeds the threshold, the frame is considered to contain no meaningful detection and `motion_detected` is used as a fallback if any class at all fired.

---

## Permissions & Security Hardening

### Requested Permissions

| Permission | Required | Purpose |
|---|---|---|
| `CAMERA` | Runtime | Open rear-facing camera for frame capture |
| `FOREGROUND_SERVICE` | Install-time | Run `IntegrityMonitor` as a foreground service |
| `FOREGROUND_SERVICE_CAMERA` | Install-time | Android 14 requirement for foreground services that access the camera |

### Explicitly Removed Permissions

| Permission | Why Removed |
|---|---|
| `INTERNET` | Structural enforcement of no-network policy. `tools:node="remove"` at the manifest level means the OS will never grant this permission, regardless of what code is added. |

### Additional Hardening

| Measure | Implementation |
|---|---|
| No screenshots | `window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)` in `MainActivity.onCreate()` |
| No cloud backup | `android:allowBackup="false"` and `android:fullBackupContent="false"` in `<application>` |
| No data extraction | `@xml/data_extraction_rules` blocks all Android 12+ automatic backup |
| No RTL | `android:supportsRtl="false"` reduces bidirectional text attack surface |
| No camera preview | `ImageReader` only — no `SurfaceTexture` means no framebuffer exposure |
| Memory locking | `mlock` on encrypted frame buffers in Rust substrate (prevents swap-to-disk) |
| Memory zeroization | `zeroize` on all key material and decrypted buffers before deallocation |
| Hardware-backed keys | Android StrongBox (when available) for AES-256-GCM key storage |

---

## Build Requirements

| Requirement | Version | Notes |
|---|---|---|
| Android Studio | Hedgehog (2023.1.1) or later | |
| Gradle | 8.2.2 | Wrapper included (`./gradlew`) |
| Android NDK | r26+ | Required for Rust→ARM64 cross-compilation |
| Rust toolchain | 1.75+ stable | `rustup target add aarch64-linux-android` |
| `cargo-ndk` | 3.x | `cargo install cargo-ndk` |
| JDK | 17 | `sourceCompatibility = JavaVersion.VERSION_17` |
| Min Android SDK | 31 (Android 12) | StrongBox hardware keystore requirement |
| Target Android SDK | 34 (Android 14) | |
| ABI | arm64-v8a only | Samsung A55 is aarch64; other ABIs not built |
| Test device | Samsung Galaxy A55 | Primary development target (Exynos 1480) |

---

## Building from Source

### 1. Install Rust cross-compilation target

```bash
rustup target add aarch64-linux-android
cargo install cargo-ndk
```

### 2. Set the Android NDK path

```bash
export ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/26.x.x
# or on Linux:
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/26.x.x
```

### 3. Build the Rust substrate

```bash
cd rust
cargo ndk -t arm64-v8a -o ../app/src/main/jniLibs build --release -p chamber-core
```

This compiles `libchamber_core.so` and places it at `app/src/main/jniLibs/arm64-v8a/libchamber_core.so`.

### 4. Build the Android APK

```bash
# Debug APK
./gradlew assembleDebug

# Release APK (minified)
./gradlew assembleRelease
```

Output APKs are written to `app/build/outputs/apk/`.

### 5. Run unit tests

```bash
./gradlew test
```

### Common build issues

| Symptom | Cause | Fix |
|---|---|---|
| `UnsatisfiedLinkError: libchamber_core.so` | `.so` not built or wrong ABI | Re-run `cargo ndk` step above |
| `Execution failed for task ':app:mergeDebugNativeLibs'` | Duplicate `.so` files | Remove stale files from `jniLibs/` |
| `TFLite: Failed to load model` | `.tflite` file missing from assets | Confirm `assets/detect_model.tflite` exists |
| Rust compile error on `jni` crate | NDK path not set | Set `ANDROID_NDK_HOME` before building |

---

## Running on Device

1. Enable **Developer Options** and **USB Debugging** on the target device.
2. Connect via USB.
3. Install via Android Studio or `adb`:

```bash
adb install app/build/outputs/apk/debug/app-debug.apk
```

4. Grant the `CAMERA` permission when prompted on first launch.
5. The application starts capturing immediately. The `IntegrityMonitor` foreground service starts automatically and displays a persistent notification.
6. Sealed detection events appear in the event list in real time.
7. No internet connection is required or possible.

**Samsung A55 specific notes:**
- StrongBox-backed key storage is available on this device. The substrate will prefer StrongBox when `KeyInfo.isInsideSecureHardware()` returns `true`.
- The Exynos 1480 NPU is not currently used for TFLite inference (CPU path only). NPU delegate support is tracked in `docs/ISSUES.md` (M5).
- Screen recording is blocked by `FLAG_SECURE` — attempting to record the screen will show a black frame over the application window.

---

## Project Status & Milestones

| Milestone | Description | Status |
|---|---|---|
| **M1** | Project scaffold — Android module, Rust workspace, JNI bridge, manifest | Complete |
| **M2** | Camera pipeline live — headless JPEG capture, frame ingestion, foreground service | Complete |
| **M3** | Object detection live — TFLite INT8 inference, semantic label mapping, sealed artifacts | Complete |
| **M4** | Integrity monitor hardening — all 5 monitors wired, violation sealing, dashboard | In progress |
| **M5** | NPU delegate, performance optimization, battery profiling | Planned |
| **M6** | Audit log export, cryptographic proof of non-retention, external audit | Planned |

The full 127-issue tracker with P0–P3 priorities, dependencies, and test criteria for each issue is in `docs/ISSUES.md`.

---

## Documentation

| Document | Location | Description |
|---|---|---|
| **Product Requirements** | `docs/PRD.md` | 5,500+ word specification covering architecture, threat model, detection requirements, UI mockups, privacy claims, milestones, and success metrics |
| **Issue Tracker** | `docs/ISSUES.md` | 127 issues across 6 milestones and 11 weeks of planned work, with P0–P3 prioritization, dependency graphs, and acceptance criteria |
| **Position Paper** | `Chamber_Sentinel_Position_Paper.pdf` | Peer-reviewed academic white paper on ephemeral camera processing and integrity monitoring as a privacy engineering primitive |
