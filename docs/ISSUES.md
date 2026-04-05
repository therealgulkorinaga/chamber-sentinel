# Chamber Sentinel — Detailed Issue List

**Total issues: 127**
**Milestones: 6**
**Estimated duration: 11 weeks (single engineer)**

---

## Legend

- **P0**: blocking — must complete before milestone exit
- **P1**: high — important for quality/security
- **P2**: normal — can defer without blocking
- **P3**: optional — nice-to-have
- **Depends**: issues that must complete first
- **Crate/Module**: where the code goes
- **Test**: how to verify it works

---

# Milestone 1 — Substrate on Android (Issues 1–28)

## Epic 1.1: Android project scaffold (Issues 1–6)

### Issue 1: Initialize Android project
**Goal**: Empty Android app that builds and runs on A55.
**Tasks**:
- Create Android project with Kotlin + Gradle
- Set minSdk = 31 (Android 12, for StrongBox guarantees)
- Set targetSdk = 34 (Android 14)
- Add NDK configuration for Rust integration
- Configure ABI filters: arm64-v8a only (A55 is aarch64)
- Create empty MainActivity with fullscreen, no action bar
- Add `android:screenOrientation="portrait"` to manifest
**Deliverable**: APK installs and launches on A55, shows blank screen
**Priority**: P0
**Depends**: none
**Test**: install on A55, app launches without crash

### Issue 2: Rust NDK toolchain setup
**Goal**: Rust compiles to Android aarch64 and links into the APK.
**Tasks**:
- Install `rustup target add aarch64-linux-android`
- Install Android NDK r26+ via SDK Manager
- Create `.cargo/config.toml` with linker path for aarch64-linux-android
- Create `rust/` directory at project root for Rust crates
- Create `chamber-core` crate (lib, cdylib) with a single exported function: `extern "C" fn chamber_hello() -> i32 { 42 }`
- Build `libchamber_core.so` for aarch64
- Verify it links into the APK
**Deliverable**: Rust `.so` is inside the APK
**Priority**: P0
**Depends**: Issue 1
**Test**: `adb shell "ls /data/app/*/lib/arm64/libchamber_core.so"` finds the library

### Issue 3: JNI bridge — Kotlin to Rust
**Goal**: Kotlin can call Rust functions and Rust can call back into Kotlin.
**Tasks**:
- Create `ChamberBridge.kt` with `external fun` declarations
- Create `jni.rs` in chamber-core with `#[no_mangle] pub extern "C" fn Java_...` functions
- Initial bridge functions:
  - `nativeInit() -> Long` (returns substrate pointer)
  - `nativeDestroy(ptr: Long)` (frees substrate)
  - `nativeVersion() -> String` (returns version string)
- Use `jni` crate (0.21+) for JNI helpers
- Test: call `nativeVersion()` from Kotlin, verify return value
**Deliverable**: bidirectional Kotlin ↔ Rust calls work
**Priority**: P0
**Depends**: Issue 2
**Test**: `ChamberBridge.nativeVersion()` returns "0.1.0" on device

### Issue 4: Permissions manifest
**Goal**: App requests exactly the permissions it needs, nothing more.
**Tasks**:
- Add to AndroidManifest.xml:
  - `CAMERA` — for camera access
  - `FOREGROUND_SERVICE` — for background monitoring
  - `FOREGROUND_SERVICE_CAMERA` — Android 14 requirement
- Do NOT add:
  - `INTERNET` — no network access from the app
  - `READ_EXTERNAL_STORAGE` / `WRITE_EXTERNAL_STORAGE` — no file access
  - `RECORD_AUDIO` — no microphone
  - `ACCESS_FINE_LOCATION` — no location
- Explicitly add `<uses-permission android:name="android.permission.INTERNET" tools:node="remove"/>` to prevent libraries from injecting it
- Document: the app has no INTERNET permission. It cannot make network connections. This is enforced by Android at the kernel level.
**Deliverable**: manifest with camera + foreground service only
**Priority**: P0
**Depends**: Issue 1
**Test**: `aapt dump permissions app.apk` shows only CAMERA and FOREGROUND_SERVICE

### Issue 5: FLAG_SECURE on all windows
**Goal**: The app's UI cannot be screenshotted or screen-recorded by other apps.
**Tasks**:
- Set `window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, ...)` in onCreate
- Apply to all Activities and Dialogs
- Verify: Android enforces this at the compositor level
**Deliverable**: screenshots of the app show a black rectangle
**Priority**: P0
**Depends**: Issue 1
**Test**: take screenshot while app is foreground — verify black/blank image

### Issue 6: App lockdown — no implicit intents
**Goal**: The app does not respond to or send any implicit intents that could leak data.
**Tasks**:
- Set `exported="false"` on all activities, services, receivers
- No `<intent-filter>` that would allow other apps to invoke this app
- No `startActivity(intent)` calls to external apps
- No `ContentProvider` exposed
- No share targets, no deep links, no custom URL schemes
**Deliverable**: app is completely self-contained, unreachable from outside
**Priority**: P1
**Depends**: Issue 1
**Test**: `adb shell am start -n com.chamber.sentinel/.SomeActivity` fails with security exception

---

## Epic 1.2: Port substrate core to Android (Issues 7–16)

### Issue 7: Port chambers-types
**Goal**: Core data types compile for aarch64-linux-android.
**Tasks**:
- Copy types from chamber project: World, Object, ObjectId, WorldId, LifecycleClass, LifecyclePhase, TerminationMode, Primitive, TransitionOperation, TransitionRequest, SealAuthorization, ChamberGrammar, SubstrateError
- Verify: `cargo build --target aarch64-linux-android` succeeds
- No platform-specific code in types — should compile unchanged
**Deliverable**: `chamber-types` crate compiles for Android
**Priority**: P0
**Depends**: Issue 2
**Test**: `cargo build --target aarch64-linux-android -p chamber-types` succeeds

### Issue 8: Port chambers-crypto (without libc-specific code)
**Goal**: Crypto primitives work on Android.
**Tasks**:
- Port AES-256-GCM encrypt/decrypt (uses `aes-gcm` crate — pure Rust, cross-platform)
- Port WorldKey with zeroize
- Port key generation (uses `rand` + `OsRng` — works on Android)
- Conditional compilation for mem_protect:
  - mlock: works on Android (Linux kernel)
  - ptrace deny: different API on Android (prctl instead of ptrace)
  - GuardBuffer: mmap works on Android
- Test encrypt/decrypt roundtrip on device
**Deliverable**: crypto operations work on A55
**Priority**: P0
**Depends**: Issue 7
**Test**: encrypt "hello" → decrypt → verify "hello" on device

### Issue 9: Port encrypted_store
**Goal**: EncryptedWorldState works on Android.
**Tasks**:
- Port EncryptedObject, EncryptedLink types
- Port encrypt_object, decrypt_object, encrypt_link, decrypt_link
- Port EncryptedWorldState (add_object, with_object, with_object_mut, all_objects_decrypted, secure_wipe)
- Verify roundtrip on device
**Deliverable**: encrypted object store works on A55
**Priority**: P0
**Depends**: Issue 8
**Test**: create 100 objects, decrypt all, verify content matches

### Issue 10: Port chambers-state
**Goal**: StateEngine with encrypted storage works on Android.
**Tasks**:
- Port EncryptedWorldStateBundle, ConvergenceReviewState, RenderState
- Port StateEngine (all methods from Phase 2)
- Verify: create world state, add objects, query, destroy
**Deliverable**: state engine works on A55
**Priority**: P0
**Depends**: Issue 9
**Test**: full lifecycle (create, add, query, destroy) on device

### Issue 11: Port chambers-audit (two-tier)
**Goal**: Two-tier audit with Tier 2 burn works on Android.
**Tasks**:
- Port SubstrateEvent, WorldEvent, AuditLog
- Port burn_world_events
- Verify: record events, burn, verify only Tier 1 survives
**Deliverable**: two-tier audit works on A55
**Priority**: P0
**Depends**: Issue 7
**Test**: 10 events recorded, burn, verify 2 survive

### Issue 12: Port chambers-burn
**Goal**: 6-layer burn works on Android.
**Tasks**:
- Port BurnEngine (logical, cryptographic, storage, memory, audit, semantic)
- Port SemanticResidueReport
- Port measure_residue
- Verify: post-burn residue score = 0.0
**Deliverable**: burn engine works on A55
**Priority**: P0
**Depends**: Issues 8, 9, 10, 11
**Test**: create world with 50 objects, burn, residue score = 0.0

### Issue 13: Port chambers-policy
**Goal**: Grammar loading and policy enforcement works on Android.
**Tasks**:
- Port PolicyEngine with RwLock-based grammar storage
- Port grammar validation (object types, phase primitives, preservation law)
- Load camera grammar (see Issue 17)
**Deliverable**: policy engine works on A55
**Priority**: P0
**Depends**: Issue 7
**Test**: load camera grammar, verify preservation law (only event_summary survives)

### Issue 14: Port chambers-capability
**Goal**: Epoch-scoped capability system works on Android.
**Tasks**:
- Port CapabilitySystem (issue, check, revoke, invalidate epoch)
- Verify epoch narrowing works
**Deliverable**: capability system works on A55
**Priority**: P0
**Depends**: Issue 7
**Test**: issue token, advance epoch, verify old token rejected

### Issue 15: Port chambers-interpreter
**Goal**: 5-check validation pipeline works on Android.
**Tasks**:
- Port Interpreter (world scope, type compat, capability, lifecycle, preservation)
- Verify: invalid operations rejected, valid operations accepted
**Deliverable**: interpreter works on A55
**Priority**: P0
**Depends**: Issues 10, 13, 14
**Test**: submit invalid TransitionRequest → verify rejection with specific error

### Issue 16: Assemble Runtime on Android
**Goal**: Full Runtime struct works on Android via JNI.
**Tasks**:
- Port Runtime::new() with all engine wiring
- Expose via JNI: createWorld, submit, advancePhase, burn, getResidueReport
- Kotlin-side wrapper class: `ChamberRuntime` with high-level methods
**Deliverable**: full substrate lifecycle callable from Kotlin
**Priority**: P0
**Depends**: Issues 7–15
**Test**: Kotlin test: create world → add 10 objects → advance → burn → verify residue = 0.0

---

## Epic 1.3: StrongBox key storage (Issues 17–21)

### Issue 17: Define camera grammar
**Goal**: Grammar definition for the camera chamber.
**Tasks**:
- Define grammar JSON:
  - grammar_id: "camera_monitor_v1"
  - objective_class: "camera_monitoring"
  - object types:
    - `frame` (temporary, burns) — max_payload: 500KB (compressed frame)
    - `detection` (temporary, burns) — model output
    - `event_summary` (preservable, survives) — sealed event label
    - `integrity_tag` (preservable, survives) — anomaly record
  - phase_primitives:
    - Active: CreateObject, SealArtifact, TriggerBurn
    - (No convergence/finalization for camera — auto-burn after N frames)
  - preservation law: only event_summary and integrity_tag survive
  - termination: auto-burn (rolling chambers)
**Deliverable**: camera grammar file
**Priority**: P0
**Depends**: Issue 13
**Test**: grammar loads in PolicyEngine, preservation law correct

### Issue 18: Android Keystore integration for K_s
**Goal**: Substrate key K_s is stored in hardware (StrongBox), never in app memory.
**Tasks**:
- Use `android.security.keystore2` API
- Generate K_s as AES-256 key in StrongBox:
  ```kotlin
  KeyGenerator.getInstance("AES", "AndroidKeyStore")
    .init(KeyGenParameterSpec.Builder("chamber_ks", PURPOSE_ENCRYPT or PURPOSE_DECRYPT)
      .setKeySize(256)
      .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
      .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
      .setIsStrongBoxBacked(true)  // Hardware requirement
      .setUserAuthenticationRequired(false)  // Always accessible
      .build())
    .generateKey()
  ```
- K_s never leaves the StrongBox hardware
- K_w wrapping: encrypt K_w under K_s using StrongBox. K_w is generated in app memory (mlock'd), but its persistence form is encrypted under K_s
- Expose K_s operations to Rust via JNI: `wrapKey(kw_bytes) -> encrypted_bytes`, `unwrapKey(encrypted_bytes) -> kw_bytes`
**Deliverable**: K_s in StrongBox, K_w wrapping/unwrapping works
**Priority**: P0
**Depends**: Issue 3
**Test**: generate K_s, wrap K_w, unwrap K_w, verify K_w matches. Delete K_s, verify unwrap fails.

### Issue 19: Verify StrongBox on A55
**Goal**: Confirm StrongBox is available and functional on the Samsung A55.
**Tasks**:
- Check `PackageManager.FEATURE_STRONGBOX_KEYSTORE`
- If StrongBox unavailable: fall back to TEE-backed keystore (still hardware, less isolated)
- Log which keystore backend is used
- Test: key generation, wrap, unwrap, delete cycle
**Deliverable**: confirmed hardware key storage on target device
**Priority**: P0
**Depends**: Issue 18
**Test**: app logs "StrongBox: available" or "TEE fallback: active" on A55

### Issue 20: K_w lifecycle tied to StrongBox
**Goal**: K_w is generated in Rust, wrapped under K_s in StrongBox, and unwrapped only when needed.
**Tasks**:
- On chamber creation:
  1. Rust generates K_w (32 bytes, OsRng)
  2. mlock K_w buffer
  3. JNI call: Kotlin wraps K_w under K_s via StrongBox → returns encrypted blob
  4. Rust stores encrypted blob
  5. Rust zeroizes plaintext K_w
- On frame encryption:
  1. JNI call: Kotlin unwraps K_w from encrypted blob via StrongBox → returns plaintext K_w
  2. Rust uses K_w for encrypt/decrypt
  3. Rust zeroizes plaintext K_w after operation
- On burn:
  1. Rust zeroizes any cached K_w
  2. Encrypted blob deleted
  3. StrongBox key (K_s) persists for next chamber
**Deliverable**: K_w never persists in plaintext; always wrapped under hardware key
**Priority**: P1
**Depends**: Issues 18, 8
**Test**: dump process memory — no 32-byte K_w pattern found outside mlock'd region

### Issue 21: K_w rotation per rolling chamber
**Goal**: Each rolling chamber (1-second window) gets a fresh K_w.
**Tasks**:
- On chamber roll:
  1. Generate new K_w
  2. Wrap under K_s
  3. Destroy old K_w
  4. New frames encrypted under new K_w
- Previous chamber's K_w is gone — forward secrecy per window
- Performance: K_w generation + StrongBox wrap must complete in < 10ms (cannot stall frame ingestion)
**Deliverable**: K_w rotates every N frames with forward secrecy
**Priority**: P1
**Depends**: Issue 20
**Test**: process 300 frames (10 chambers), verify 10 different K_w blobs were created and 9 were destroyed

---

## Epic 1.4: Substrate hardening on Android (Issues 22–28)

### Issue 22: Disable core dumps on Android
**Goal**: App process produces no tombstone on crash.
**Tasks**:
- `prctl(PR_SET_DUMPABLE, 0)` at native init (Rust side)
- Verify: SIGABRT produces no tombstone in `/data/tombstones/`
**Deliverable**: no crash dump contains app memory
**Priority**: P0
**Depends**: Issue 3
**Test**: send SIGABRT to app process, verify no tombstone created

### Issue 23: mlock on Android
**Goal**: K_w and guard buffer locked in RAM.
**Tasks**:
- `mlock()` works on Android (Linux kernel) but may be limited by `RLIMIT_MEMLOCK`
- Check `getrlimit(RLIMIT_MEMLOCK)` — default on Android is usually 64KB
- 64KB is enough for K_w (32 bytes) + guard buffer (8KB) + inference buffer (if small)
- If limit is too low: document that rooted devices can raise it
**Deliverable**: K_w and guard buffer mlock'd on A55
**Priority**: P0
**Depends**: Issue 8
**Test**: `mincore()` confirms guard buffer pages are resident

### Issue 24: ptrace deny on Android
**Goal**: No debugger can attach to the app process.
**Tasks**:
- `prctl(PR_SET_DUMPABLE, 0)` — also prevents ptrace attachment on Android
- Alternatively: `prctl(PR_SET_PTRACER, PR_SET_PTRACER_ANY)` set to none
- Verify: `adb shell run-as com.chamber.sentinel` cannot attach
**Deliverable**: process is not debuggable
**Priority**: P0
**Depends**: Issue 3
**Test**: `adb jdwp` does not list the app's PID

### Issue 25: Prevent ADB backup
**Goal**: `adb backup` cannot extract app data.
**Tasks**:
- Set `android:allowBackup="false"` in manifest
- Set `android:fullBackupContent="false"` in manifest
- Set `android:dataExtractionRules` to deny all (Android 12+)
**Deliverable**: no app data extractable via adb backup
**Priority**: P0
**Depends**: Issue 1
**Test**: `adb backup com.chamber.sentinel` produces empty backup

### Issue 26: Prevent recent apps thumbnail
**Goal**: Android's recent apps screen does not show a thumbnail of the app.
**Tasks**:
- `FLAG_SECURE` already handles this (Issue 5)
- Additionally: set `android:excludeFromRecents="true"` if the app should not appear in recents at all
- Decision: appear in recents (but with blank thumbnail) or hide entirely?
**Deliverable**: no visual information leaks via recent apps
**Priority**: P1
**Depends**: Issue 5
**Test**: open app, switch to recent apps, verify blank/no thumbnail

### Issue 27: Network isolation verification
**Goal**: Empirically verify the app cannot make network connections.
**Tasks**:
- The app has no INTERNET permission (Issue 4)
- Test: attempt `URL("https://example.com").openConnection()` from Kotlin — verify SecurityException
- Test: attempt `socket()` from Rust — verify EACCES
- Test: attempt DNS resolution — verify failure
- Document: Android enforces network isolation at the kernel level via UID-based iptables rules. No INTERNET permission = no network syscalls succeed.
**Deliverable**: empirical proof of network isolation
**Priority**: P0
**Depends**: Issue 4
**Test**: all 3 network attempts fail with permission denial

### Issue 28: Substrate on Android integration test
**Goal**: Full substrate lifecycle runs on A55 via JNI.
**Tasks**:
- Android instrumented test (runs on device):
  1. Create Runtime via JNI
  2. Load camera grammar
  3. Create world
  4. Add 50 frame objects (simulated)
  5. Add 5 detection objects
  6. Seal 2 event_summary artifacts
  7. Burn
  8. Verify: residue score = 0.0
  9. Verify: vault contains 2 event_summaries
  10. Verify: K_w is destroyed
  11. Verify: no frame content in process memory (string scan)
**Deliverable**: full lifecycle test passes on A55
**Priority**: P0
**Depends**: Issues 7–27
**Test**: instrumented test passes on device

---

# Milestone 2 — Camera Ingestion (Issues 29–48)

## Epic 2.1: Camera2 API integration (Issues 29–36)

### Issue 29: Camera2 session setup
**Goal**: Open rear camera, configure capture session.
**Tasks**:
- Request CAMERA permission at runtime (Android 6+)
- Open `CameraManager.openCamera()` for rear camera (facing back)
- Create `CaptureSession` with `ImageReader` as output surface
- Configure: 1080p resolution, 30fps, JPEG or YUV_420_888 format
- No preview surface — frames go only to ImageReader (no display)
**Deliverable**: camera session opens, frames arrive in ImageReader
**Priority**: P0
**Depends**: Issue 1
**Test**: logcat shows "Frame received" at 30fps

### Issue 30: Frame format selection
**Goal**: Choose optimal frame format for detection + encryption.
**Tasks**:
- Options:
  - YUV_420_888: native camera format, ~3MB per 1080p frame, efficient for ML
  - JPEG: compressed, ~200KB per frame, requires decode for ML
  - RAW: huge, not needed
- Decision: YUV_420_888 for ML inference, JPEG for storage efficiency
- Actually: use YUV for inference, never store full frames — only the encrypted compressed version if needed for the guard buffer
- For the camera chamber: frames are transient. We may not need to store even the encrypted frame — just process it and discard
**Deliverable**: documented format choice with rationale
**Priority**: P1
**Depends**: Issue 29
**Test**: verify format available on A55 camera

### Issue 31: ImageReader callback → Rust
**Goal**: Each camera frame is passed to Rust immediately via JNI.
**Tasks**:
- `ImageReader.OnImageAvailableListener` callback in Kotlin
- On each frame:
  1. Acquire latest image from ImageReader
  2. Extract pixel bytes (Y plane for grayscale, or full YUV)
  3. JNI call: `nativeIngestFrame(ptr, bytes, width, height, timestamp)`
  4. Close the image (releases buffer back to camera)
- Timing requirement: entire callback must complete in < 33ms (30fps budget)
**Deliverable**: frames flow from camera to Rust at 30fps
**Priority**: P0
**Depends**: Issues 3, 29
**Test**: log frame count — verify 30 frames/sec arriving in Rust

### Issue 32: Zero-copy frame transfer (optimization)
**Goal**: Avoid copying frame bytes between Kotlin and Rust.
**Tasks**:
- Instead of `GetByteArrayElements` (which copies), use `GetDirectByteBuffer` on the ImageReader's plane buffer
- The Image's plane buffer is a direct ByteBuffer — Rust can read the pointer directly
- Caution: the buffer is only valid until `image.close()` — Rust must finish before Kotlin closes the image
**Deliverable**: frame bytes accessed without copy
**Priority**: P2
**Depends**: Issue 31
**Test**: measure JNI call latency — < 1ms without copy vs ~5ms with copy

### Issue 33: Frame encryption on arrival
**Goal**: The moment frame bytes arrive in Rust, encrypt immediately.
**Tasks**:
- In `nativeIngestFrame`:
  1. Generate per-frame nonce (12 bytes)
  2. Encrypt frame bytes under K_w (AES-256-GCM, hardware accelerated on ARM)
  3. Store as EncryptedObject in the current chamber's EncryptedWorldState
  4. Zeroize the plaintext frame bytes in the receive buffer
- Timing: encryption of 3MB YUV frame at AES hardware speed: < 1ms
**Deliverable**: frames encrypted within 2ms of arrival
**Priority**: P0
**Depends**: Issues 9, 31
**Test**: measure encrypt latency — < 2ms per frame on A55

### Issue 34: No preview surface — camera without display
**Goal**: Camera operates without showing any preview to the user.
**Tasks**:
- Do NOT bind camera output to a SurfaceView or TextureView
- Use `ImageReader` as the sole output surface
- The screen shows only sealed events (text), never camera frames
- This prevents framebuffer capture of camera content
**Deliverable**: camera runs with zero display of raw frames
**Priority**: P0
**Depends**: Issue 29
**Test**: screen recording while app is active shows no camera imagery

### Issue 35: Camera lifecycle management
**Goal**: Camera properly opens, runs, and closes without leaks.
**Tasks**:
- Open camera in `onResume`, close in `onPause`
- Handle camera disconnection (another app takes camera)
- Handle camera error callbacks
- Release all ImageReader buffers on close
- Ensure no frame references survive camera close
**Deliverable**: camera lifecycle is leak-free
**Priority**: P0
**Depends**: Issue 29
**Test**: open app, background, foreground, background 10 times — no crash, no memory leak

### Issue 36: Camera access logging
**Goal**: Log every camera open/close for the integrity monitor.
**Tasks**:
- Record timestamp of camera open, camera close, camera error
- These are substrate-scoped events (Tier 1) — survive burn
- Used by integrity monitor to correlate with network anomalies
**Deliverable**: camera lifecycle events in audit log
**Priority**: P1
**Depends**: Issues 11, 29
**Test**: verify audit log contains camera_opened and camera_closed events

---

## Epic 2.2: Rolling chamber mode (Issues 37–43)

### Issue 37: Chamber auto-creation on camera start
**Goal**: Starting the camera automatically creates the first rolling chamber.
**Tasks**:
- On camera open → create world with camera grammar → issue capabilities → set phase to Active
- Chamber ID + K_w generated
- Frame counter initialized to 0
**Deliverable**: camera start = chamber start
**Priority**: P0
**Depends**: Issues 16, 17, 29
**Test**: open camera, verify world created in state engine

### Issue 38: Frame counter and chamber window size
**Goal**: Track frames per chamber, configurable window size.
**Tasks**:
- Default: 30 frames per chamber (1 second at 30fps)
- Configurable: 15 (0.5s), 30 (1s), 150 (5s), 900 (30s)
- Each frame increments counter
- When counter hits window size → trigger roll
**Deliverable**: frame counter controls chamber lifecycle
**Priority**: P0
**Depends**: Issue 37
**Test**: set window=30, send 90 frames, verify 3 chambers created

### Issue 39: Chamber roll — burn and create
**Goal**: When window expires, burn current chamber and create next.
**Tasks**:
- Roll sequence:
  1. Run inference on current chamber's frames (Issue 52+)
  2. Seal any detected events
  3. Trigger burn on current chamber
  4. Generate new K_w for next chamber
  5. Create new world
  6. Continue frame ingestion into new chamber
- Critical: no frame gap during roll. New chamber must be ready before old one burns. Use double-buffering: next chamber created before current burns.
**Deliverable**: seamless chamber rolling at 30fps
**Priority**: P0
**Depends**: Issues 12, 21, 38
**Test**: run camera for 60 seconds, verify 60 chambers created and burned, zero frames dropped

### Issue 40: Double-buffered chamber roll
**Goal**: No frame drops during chamber transition.
**Tasks**:
- Maintain two chamber slots: current + next
- At frame N-5 (5 frames before roll): create next chamber, generate K_w
- At frame N: switch ingestion to next chamber, burn current
- Frames never wait for chamber creation
**Deliverable**: zero-drop chamber transitions
**Priority**: P0
**Depends**: Issue 39
**Test**: run at 30fps for 5 minutes, count frames — verify zero drops (total = 9000)

### Issue 41: Forward secrecy verification
**Goal**: Empirically verify that compromising K_w[n] reveals nothing about chamber[n-1].
**Tasks**:
- Test:
  1. Create chamber 1, encrypt 30 frames, burn (K_w1 destroyed)
  2. Create chamber 2, encrypt 30 frames, capture K_w2 before burn
  3. Attempt to decrypt chamber 1's frames with K_w2 → must fail
  4. Decrypt chamber 2's frames with K_w2 → must succeed
  5. Burn chamber 2
  6. Attempt to decrypt chamber 2's frames with K_w2 → must fail (K_w2 now zeroized)
**Deliverable**: forward secrecy proven per chamber
**Priority**: P0
**Depends**: Issue 39
**Test**: all 4 assertions pass

### Issue 42: Chamber roll metrics
**Goal**: Track and log chamber roll performance.
**Tasks**:
- Metrics per roll:
  - Roll latency (time from trigger to new chamber ready): target < 5ms
  - Burn latency: target < 10ms
  - K_w generation + StrongBox wrap: target < 10ms
  - Total roll overhead: target < 25ms
  - Frame count per chamber
  - Events sealed per chamber
- Log as sealed telemetry (survives burn, no frame content)
**Deliverable**: performance metrics per chamber roll
**Priority**: P1
**Depends**: Issue 39
**Test**: 60 seconds of operation, all rolls < 25ms

### Issue 43: Graceful shutdown — final burn
**Goal**: When camera stops, burn the last chamber.
**Tasks**:
- On camera close or app background:
  1. Run inference on any remaining frames
  2. Seal any final events
  3. Burn the active chamber
  4. Zeroize all buffers
- No frames survive app close
**Deliverable**: app exit = clean burn
**Priority**: P0
**Depends**: Issue 39
**Test**: open camera for 3 seconds, close app, verify state engine empty

---

## Epic 2.3: Burn verification (Issues 44–48)

### Issue 44: Post-burn memory scan
**Goal**: After burn, scan app memory for frame content.
**Tasks**:
- Before burning: create 30 frames with distinctive marker bytes (e.g., 0xDEADBEEF pattern at known offsets)
- After burn: scan the process memory map for the marker pattern
- Expected: marker not found (frames were encrypted, then ciphertext was wiped)
**Deliverable**: empirical proof that frame content is gone after burn
**Priority**: P0
**Depends**: Issue 39
**Test**: marker scan returns 0 hits after burn

### Issue 45: Post-burn storage scan
**Goal**: After burn, verify no files were written to storage.
**Tasks**:
- Scan: `/data/data/com.chamber.sentinel/`, `/sdcard/DCIM/`, `/sdcard/Pictures/`, `/data/media/`, temp directories
- Expected: zero image/video files
- Also check: no SQLite database with frame metadata, no SharedPreferences with frame data
**Deliverable**: zero storage artifacts after burn
**Priority**: P0
**Depends**: Issue 39
**Test**: recursive file scan finds zero image files

### Issue 46: Post-burn network verification
**Goal**: After burn, verify zero bytes were sent to any external endpoint.
**Tasks**:
- Record `TrafficStats.getUidTxBytes(myUid)` before and after a full session
- Expected delta: 0 bytes (app has no INTERNET permission)
**Deliverable**: zero network egress from app process
**Priority**: P0
**Depends**: Issues 4, 27
**Test**: tx byte delta = 0 after 60 seconds of operation

### Issue 47: K_w destruction verification
**Goal**: After burn, verify K_w is unrecoverable.
**Tasks**:
- After burn: attempt to decrypt a saved ciphertext from the burned chamber
- Expected: decryption fails (key not found / destroyed)
- Verify: StrongBox-wrapped K_w blob has been deleted
- Verify: plaintext K_w not in process memory (byte scan for all 32-byte high-entropy sequences)
**Deliverable**: K_w provably gone after burn
**Priority**: P0
**Depends**: Issues 12, 20
**Test**: decrypt attempt fails, blob deleted, memory scan clean

### Issue 48: Residue report for camera chambers
**Goal**: SemanticResidueReport adapted for camera workload.
**Tasks**:
- Add camera-specific fields:
  - frames_processed (total frames across all chambers)
  - chambers_burned (count)
  - events_sealed (count)
  - integrity_tags_sealed (count)
  - max_frame_exposure_ms (longest time a frame was decrypted in guard buffer)
**Deliverable**: camera-aware residue reporting
**Priority**: P1
**Depends**: Issue 12
**Test**: residue report shows correct counts after 60-second session

---

# Milestone 3 — Detection Model (Issues 49–68)

## Epic 3.1: Model integration (Issues 49–55)

### Issue 49: Select and obtain detection model
**Goal**: Choose quantized object detection model for on-device inference.
**Tasks**:
- Evaluate:
  - YOLOv8n (nano): 6MB, strong accuracy, ONNX/TFLite available
  - MobileNet V3 + SSD: 5MB, fast, TFLite optimized
  - MediaPipe Object Detector: 4MB, Google-optimized for Android
  - EfficientDet-Lite0: 4MB, good accuracy/speed tradeoff
- Selection criteria: < 10MB, < 50ms inference on A55, detects: person, vehicle, animal, package
- Download model weights
- Convert to TFLite format if needed (quantized INT8)
**Deliverable**: model file ready for integration
**Priority**: P0
**Depends**: none
**Test**: model file exists, < 10MB, TFLite format

### Issue 50: TFLite runtime integration
**Goal**: TFLite interpreter runs in the Android app.
**Tasks**:
- Add `org.tensorflow:tensorflow-lite:2.14+` dependency
- Add `org.tensorflow:tensorflow-lite-gpu:2.14+` for GPU delegate (Exynos Mali GPU)
- Load model from assets
- Create interpreter with GPU delegate
- Warm up: run inference once on dummy input
**Deliverable**: TFLite interpreter initialized and warm
**Priority**: P0
**Depends**: Issue 49
**Test**: interpreter created without error, dummy inference completes

### Issue 51: Model input pipeline
**Goal**: Camera frames preprocessed for model input.
**Tasks**:
- Model expects: 320x320 (or 640x640) RGB tensor, normalized [0,1] or [-1,1]
- Camera provides: 1080p YUV_420_888
- Pipeline:
  1. Decrypt frame from guard buffer
  2. Convert YUV → RGB (can use Android's built-in YuvImage or libyuv via Rust)
  3. Resize to model input size (bilinear interpolation)
  4. Normalize pixel values
  5. Copy to TFLite input tensor
- All in the guard buffer — no intermediate copies
**Deliverable**: camera frame correctly preprocessed for model
**Priority**: P0
**Depends**: Issues 33, 50
**Test**: feed known test image, verify model input tensor matches expected values

### Issue 52: Run inference in guard buffer
**Goal**: Model inference happens inside the mlock'd buffer, plaintext zeroed after.
**Tasks**:
- Sequence:
  1. Decrypt frame into inference buffer (larger than 8KB — need ~300KB for 320x320 RGB)
  2. Preprocess in-place
  3. Run TFLite interpreter
  4. Extract detections (class, confidence, bbox)
  5. Zero the inference buffer
  6. Frame plaintext existed for ~50ms (inference time)
- The inference buffer must be mlock'd and MADV_DONTDUMP'd
- Adjust guard buffer size from 8KB to 512KB for camera frames
**Deliverable**: inference runs in protected memory
**Priority**: P0
**Depends**: Issues 23, 51
**Test**: inference completes, buffer zeroed, detections extracted

### Issue 53: Detection output → typed objects
**Goal**: Model detections become chamber objects.
**Tasks**:
- For each detection above confidence threshold (e.g., 0.7):
  - Create a `detection` object in the chamber:
    ```json
    {
      "class": "person",
      "confidence": 0.94,
      "frame_index": 15,
      "timestamp": "2026-04-05T15:47:03Z"
    }
    ```
  - Note: NO bounding box coordinates stored (could reveal spatial information about the scene)
  - Note: NO embedding or feature vector stored
- Detections are temporary objects (burn with the chamber)
**Deliverable**: model output stored as encrypted chamber objects
**Priority**: P0
**Depends**: Issue 52
**Test**: camera pointing at person → detection object created with class "person"

### Issue 54: Inference timing and frame skip
**Goal**: If inference is slower than frame rate, skip frames gracefully.
**Tasks**:
- At 30fps, budget is 33ms per frame
- If inference takes 50ms, process every other frame (15fps effective detection rate)
- Non-processed frames are still encrypted and burned — they just don't get model inference
- Configurable: process every Nth frame (1=all, 2=every other, 5=every 5th)
**Deliverable**: graceful degradation under inference load
**Priority**: P1
**Depends**: Issue 52
**Test**: set inference_every=2, verify 15 detections per second at 30fps camera

### Issue 55: Model output schema validation
**Goal**: Model output is validated against grammar before creating objects.
**Tasks**:
- Detection object must conform to `detection` type schema in camera grammar
- Reject any output that contains:
  - Raw pixel data
  - Feature embeddings
  - Bounding box (if configured to exclude)
  - Free-text fields exceeding size bounds
- This prevents the model from being a covert channel for frame data through the sealed event
**Deliverable**: model output validated before storage
**Priority**: P0
**Depends**: Issue 53
**Test**: inject oversized model output → verify rejection

---

## Epic 3.2: Event sealing (Issues 56–62)

### Issue 56: Detection → event aggregation
**Goal**: Aggregate per-frame detections into events.
**Tasks**:
- A "person at door" event is not one frame — it's a sequence of detections across multiple frames
- Aggregation logic:
  - Same class detected in N consecutive frames (e.g., 5) → create event
  - Event starts at first detection, ends when class disappears for M frames (e.g., 10)
  - Event summary: class, start_time, end_time, max_confidence, frame_count
- This happens within a rolling chamber or across chamber boundaries (need to carry detection state)
**Deliverable**: multi-frame detection aggregation
**Priority**: P0
**Depends**: Issue 53
**Test**: 30 frames with person → 1 event (not 30 separate events)

### Issue 57: Cross-chamber detection state
**Goal**: Detection state carries across chamber boundaries.
**Tasks**:
- Problem: chamber burns every 30 frames. A person visible for 90 frames spans 3 chambers.
- Solution: detection state (current tracked objects, frame counts) is separate from frame data
- Detection state is encrypted under a separate key K_d that persists across chamber rolls
- K_d is rotated on a longer cycle (e.g., every 5 minutes) or when no active detections exist
- When K_d rotates, detection state is sealed and burned
**Deliverable**: detections span multiple chambers
**Priority**: P1
**Depends**: Issues 39, 56
**Test**: person visible for 3 seconds (3 chambers) → 1 event, not 3

### Issue 58: Event sealing
**Goal**: Aggregated events become sealed artifacts.
**Tasks**:
- When an event ends (or chamber force-burns):
  1. Create `event_summary` object:
     ```json
     {
       "event_type": "person_detected",
       "start_time": "2026-04-05T15:47:00Z",
       "end_time": "2026-04-05T15:47:12Z",
       "confidence": 0.94,
       "duration_seconds": 12
     }
     ```
  2. Seal as artifact (auto-authorized — no human confirmation needed for camera events)
  3. Store in vault
- No frame data in the event. No bounding boxes. No images.
**Deliverable**: events sealed as vault artifacts
**Priority**: P0
**Depends**: Issue 56
**Test**: person walks past camera → vault contains one event_summary artifact

### Issue 59: Event schema — strict type enforcement
**Goal**: Event schema prevents information leakage through sealed events.
**Tasks**:
- Allowed fields (camera grammar enforcement):
  - event_type: enum ["person_detected", "vehicle_detected", "animal_detected", "package_detected", "motion_detected", "unknown_object"]
  - start_time: ISO 8601 timestamp, quantized to nearest 5 seconds
  - end_time: same
  - confidence: float 0.0–1.0, 2 decimal places
  - duration_seconds: integer
- NOT allowed:
  - Bounding box (spatial information about the scene)
  - Image embedding or feature vector
  - Color information
  - Size/scale information
  - Free text description
  - Frame index or count (reveals processing rate)
- Schema enforced by policy engine before sealing
**Deliverable**: strict event schema with no covert channels
**Priority**: P0
**Depends**: Issue 58
**Test**: attempt to seal event with bbox → rejected. Seal event with only allowed fields → accepted.

### Issue 60: Timestamp quantization
**Goal**: Event timestamps don't reveal precise timing patterns.
**Tasks**:
- Round start_time and end_time to nearest 5 seconds (or configurable: 1s, 5s, 15s, 60s)
- Rationale: precise timestamps could reveal:
  - Exactly when a person arrived (±100ms) — useful for correlation attacks
  - Duration with frame-level precision — reveals detection model behavior
- Quantized timestamps still useful for "person at door around 3:47 PM"
**Deliverable**: configurable timestamp quantization
**Priority**: P1
**Depends**: Issue 58
**Test**: event at 15:47:03.847 → sealed as 15:47:05

### Issue 61: Event rate limiting
**Goal**: Prevent event flooding that could be a covert channel.
**Tasks**:
- Max events per minute: configurable (default: 10)
- Max events per hour: configurable (default: 100)
- If limits exceeded: log integrity warning, continue detection but stop sealing
- Rationale: a compromised model could encode frame data by generating thousands of events with carefully chosen timestamps/types
**Deliverable**: event rate limits enforced
**Priority**: P1
**Depends**: Issue 58
**Test**: generate 20 events in 1 minute → only first 10 sealed, rest logged as rate-limited

### Issue 62: Vault event storage
**Goal**: Sealed events stored efficiently in the vault.
**Tasks**:
- Vault on Android: SQLite database in app's private storage
- Table: `events(id, event_type, start_time, end_time, confidence, duration, sealed_at)`
- No full-text search (prevents content scanning by other apps)
- Database file encrypted with Android's SQLCipher or Jetpack Security EncryptedFile
- Vault survives app updates (data persists in app private dir)
**Deliverable**: persistent event vault on Android
**Priority**: P0
**Depends**: Issue 58
**Test**: seal 50 events, kill app, relaunch, verify 50 events in vault

---

## Epic 3.3: Inference performance (Issues 63–68)

### Issue 63: GPU delegate performance on Exynos
**Goal**: Verify and optimize TFLite GPU inference on A55's Mali GPU.
**Tasks**:
- Test GPU delegate (Mali-G68): measure inference time vs CPU
- Expected: GPU 2-3x faster than CPU for MobileNet/YOLO
- If GPU delegate has issues (compatibility, accuracy): fall back to NNAPI delegate
- Benchmark: 100 inference runs, measure p50/p95/p99 latency
**Deliverable**: optimal inference delegate selected for A55
**Priority**: P1
**Depends**: Issue 50
**Test**: p95 inference latency < 50ms with chosen delegate

### Issue 64: Inference buffer sizing
**Goal**: Correctly sized mlock'd buffer for frame preprocessing + inference.
**Tasks**:
- Required buffer sizes:
  - Frame decrypt: up to 3MB (1080p YUV)
  - Preprocessed input: 320*320*3 = 307KB (float32) or 307KB (uint8)
  - Model output: ~1KB (detection results)
  - Total needed: ~3.5MB
- mlock limit on Android: check RLIMIT_MEMLOCK (typically 64KB)
- Problem: 3.5MB >> 64KB. Cannot mlock the entire inference buffer.
- Solution: decrypt and preprocess in chunks, or accept that the full inference buffer is not mlock'd but IS zeroed immediately after use
- Document the tradeoff: mlock protects against swap, but 3.5MB may exceed the limit
**Deliverable**: documented buffer sizing with tradeoff analysis
**Priority**: P1
**Depends**: Issue 52
**Test**: measure actual RLIMIT_MEMLOCK on A55

### Issue 65: Batch inference (process multiple frames per model call)
**Goal**: Reduce per-frame overhead by batching.
**Tasks**:
- Instead of one model call per frame, batch N frames:
  - Decrypt N frames sequentially into the inference buffer
  - Run model once on batch input (if model supports batching)
  - Extract N sets of detections
  - Zero buffer
- Trade-off: higher latency per batch vs fewer total model calls
- May not be worth it if single-frame inference is already < 33ms
**Deliverable**: optional batch inference mode
**Priority**: P3
**Depends**: Issue 52
**Test**: batch=4, verify 4 detection sets per model call

### Issue 66: Power consumption measurement
**Goal**: Measure battery impact of continuous camera + inference + encryption.
**Tasks**:
- Baseline: A55 battery drain in standby (screen off)
- Test: run Chamber Sentinel for 1 hour, measure battery delta
- Target: < 15% additional drain per hour vs standby
- If too high: reduce frame rate, increase inference skip, reduce chamber window
**Deliverable**: documented power consumption
**Priority**: P1
**Depends**: Issue 52
**Test**: 1-hour test, battery drain documented

### Issue 67: Thermal throttling handling
**Goal**: Gracefully degrade when the phone gets hot.
**Tasks**:
- Monitor: `PowerManager.getThermalStatus()` (Android 10+)
- On THERMAL_STATUS_MODERATE: reduce frame rate to 15fps, inference every 3rd frame
- On THERMAL_STATUS_SEVERE: reduce to 10fps, inference every 5th frame
- On THERMAL_STATUS_CRITICAL: pause camera, log warning
- Resume normal operation when thermal status drops
**Deliverable**: thermal-aware frame processing
**Priority**: P1
**Depends**: Issue 54
**Test**: simulate thermal event → verify frame rate reduction

### Issue 68: End-to-end latency measurement
**Goal**: Measure time from frame capture to sealed event.
**Tasks**:
- Instrument: timestamp at ImageReader callback, at encryption, at decrypt-for-inference, at model output, at event seal
- Target pipeline:
  - Camera → ImageReader: ~5ms (hardware)
  - ImageReader → Rust JNI: < 2ms
  - Encrypt: < 2ms
  - Decrypt for inference: < 2ms
  - Preprocess: < 5ms
  - Inference: < 50ms
  - Event aggregation: < 1ms
  - Seal: < 1ms
  - Total: < 70ms from capture to sealed event
**Deliverable**: end-to-end latency profile
**Priority**: P1
**Depends**: Issue 52
**Test**: p95 end-to-end < 100ms

---

# Milestone 4 — Integrity Monitor (Issues 69–95)

## Epic 4.1: Non-root observables (Issues 69–80)

### Issue 69: Camera availability monitor
**Goal**: Detect when another app accesses the camera.
**Tasks**:
- Register `CameraManager.AvailabilityCallback`
- On `onCameraUnavailable(cameraId)`: if we didn't close it, another app took it
- Tag: `unauthorized_camera_access` with timestamp and camera ID
**Deliverable**: alert when camera accessed by other app
**Priority**: P0
**Depends**: Issue 29
**Test**: open camera in Chamber Sentinel, then open stock camera app → tag generated

### Issue 70: Network egress monitor (app-level)
**Goal**: Monitor this app's network transmission.
**Tasks**:
- Every 500ms: read `TrafficStats.getUidTxBytes(Process.myUid())`
- Compute delta from last reading
- Expected: 0 bytes always (no INTERNET permission)
- If delta > 0: tag `impossible_network_egress` (shouldn't happen — Android prevents it)
- This catches the case where a library or framework somehow bypasses the permission
**Deliverable**: app-level network egress monitoring
**Priority**: P0
**Depends**: Issue 4
**Test**: verify zero bytes transmitted during 60-second session

### Issue 71: Network egress monitor (device-level)
**Goal**: Monitor total device network activity for anomalies during camera operation.
**Tasks**:
- Every 500ms: read `TrafficStats.getTotalTxBytes()`
- Compute delta, compare to baseline (measured during non-camera periods)
- Baseline: establish in first 30 seconds of app launch (before camera starts)
- Anomaly threshold: > 2x baseline sustained for > 5 seconds
- Tag: `anomalous_device_network` with measured rate and baseline rate
**Deliverable**: device-wide network anomaly detection
**Priority**: P0
**Depends**: none
**Test**: during camera operation, download large file on same device → tag generated

### Issue 72: File creation monitor
**Goal**: Detect any image/video files created during camera operation.
**Tasks**:
- `FileObserver` watching:
  - `/sdcard/DCIM/`
  - `/sdcard/Pictures/`
  - `/sdcard/Download/`
  - `/data/media/0/`
  - App's own cache dir
  - Temp directories
- Events: CREATE, MOVED_TO (catches files moved into these dirs)
- Filter: file extension is image/video (.jpg, .png, .mp4, .webm, .heic)
- Tag: `unauthorized_file_write` with filename and path
**Deliverable**: real-time file creation monitoring
**Priority**: P0
**Depends**: none
**Test**: take screenshot during operation → tag generated for the screenshot file

### Issue 73: Process list monitor
**Goal**: Detect new processes spawning during camera operation.
**Tasks**:
- Every 2 seconds: `ActivityManager.getRunningAppProcesses()`
- On first read: store baseline set
- On subsequent reads: diff against baseline
- New process appearing during camera operation: tag `suspicious_process_spawn` with process name and PID
- Whitelist: system processes, launcher, keyboard — these come and go normally
**Deliverable**: process spawn detection
**Priority**: P1
**Depends**: none
**Test**: launch a new app while camera running → tag generated (unless whitelisted)

### Issue 74: Memory pressure monitor
**Goal**: Detect unusual memory consumption suggesting frame buffering by another process.
**Tasks**:
- Every 2 seconds: `ActivityManager.MemoryInfo`
- Track: available memory, total memory, low memory flag
- Anomaly: available memory drops significantly during camera operation (another process consuming camera frames)
- Tag: `unusual_memory_pressure` with measured values
**Deliverable**: memory consumption anomaly detection
**Priority**: P2
**Depends**: none
**Test**: run a memory-consuming process during camera → tag generated if significant drop

### Issue 75: Permission audit on startup
**Goal**: Check if any new apps with camera permission were installed since last run.
**Tasks**:
- On app start: `PackageManager.getInstalledPackages()` with `GET_PERMISSIONS`
- Filter: apps with `CAMERA` permission
- Compare against stored list from previous run
- New app with camera permission: tag `new_camera_app_installed` with package name
- Store current list in encrypted SharedPreferences
**Deliverable**: camera permission audit
**Priority**: P1
**Depends**: none
**Test**: install a camera app → on next Chamber Sentinel launch, tag generated

### Issue 76: Clipboard monitor
**Goal**: Detect if clipboard contains image data during camera operation.
**Tasks**:
- Every 5 seconds: `ClipboardManager.getPrimaryClip()`
- Check: does the clip contain an image URI or image data?
- If yes during camera operation: tag `clipboard_image_during_camera`
- Rationale: a compromised process might copy a camera frame to the clipboard
**Deliverable**: clipboard image detection
**Priority**: P2
**Depends**: none
**Test**: copy an image to clipboard while camera running → tag generated

### Issue 77: Screen recording detection
**Goal**: Detect if screen recording or screen mirroring is active.
**Tasks**:
- Android 11+: `MediaProjectionManager` callbacks
- Check `Display.getFlags()` for `FLAG_SECURE` violation attempts
- Monitor `DisplayManager.getDisplays()` for virtual/mirrored displays (casting)
- Tag: `screen_recording_active` or `display_mirroring_active`
**Deliverable**: screen capture detection
**Priority**: P1
**Depends**: none
**Test**: start screen recording, launch app → tag generated

### Issue 78: Accessibility service detection
**Goal**: Detect if accessibility services are reading the screen.
**Tasks**:
- Check `Settings.Secure.getString(ENABLED_ACCESSIBILITY_SERVICES)`
- If any non-system accessibility service is active: tag `accessibility_service_active`
- Accessibility services can read screen content, including any text displayed
- The app should show no sensitive content on screen, but this is an additional check
**Deliverable**: accessibility service detection
**Priority**: P1
**Depends**: none
**Test**: enable TalkBack or third-party accessibility service → tag generated

### Issue 79: Integrity monitor service
**Goal**: Background service that runs all monitors continuously.
**Tasks**:
- `ForegroundService` with `ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA`
- Runs all monitors (Issues 69–78) in parallel
- Each monitor runs on its own scheduled interval
- Results aggregated into integrity state
- Service survives app backgrounding (foreground notification required)
**Deliverable**: persistent monitoring service
**Priority**: P0
**Depends**: Issues 69–78
**Test**: monitors run continuously for 1 hour without crash

### Issue 80: Integrity dashboard UI
**Goal**: User can see real-time integrity status.
**Tasks**:
- Display:
  - Overall status: "Clean" (green), "Warning" (yellow), "Alert" (red)
  - Per-monitor status: last check time, result, any active tags
  - Network rate: current TX bytes/sec
  - Camera consumers: count and names
  - Active tags: list with timestamps
  - Total chambers burned: count
  - Total events sealed: count
**Deliverable**: real-time integrity UI
**Priority**: P1
**Depends**: Issue 79
**Test**: UI updates live as monitors run

---

## Epic 4.2: Anomaly response (Issues 81–88)

### Issue 81: Tag creation and sealing
**Goal**: Integrity tags are sealed artifacts that survive burn.
**Tasks**:
- Tag structure:
  ```json
  {
    "tag_type": "anomalous_device_network",
    "timestamp": "2026-04-05T15:47:03Z",
    "check": "device_network_egress",
    "expected": "baseline: 12 KB/sec",
    "measured": "4.7 MB/sec",
    "duration_ms": 2500,
    "severity": "critical",
    "process": { "name": "com.unknown.service", "pid": 4847 },
    "action_taken": "emergency_burn",
    "chamber_id": "019d5...",
    "frames_at_risk": 12
  }
  ```
- Tags are `integrity_tag` objects (preservable per camera grammar)
- Automatically sealed — no human authorization needed
- Stored in vault alongside event summaries
**Deliverable**: structured integrity tags
**Priority**: P0
**Depends**: Issue 17
**Test**: trigger network anomaly → tag in vault with correct fields

### Issue 82: Severity classification
**Goal**: Classify anomalies by severity to determine response.
**Tasks**:
- Critical (immediate emergency burn):
  - `unauthorized_camera_access` — another process using camera
  - Network spike > 10x baseline for > 2 seconds
  - `impossible_network_egress` — app sending data (should be impossible)
- Warning (tag + user alert, no burn):
  - `new_camera_app_installed`
  - `accessibility_service_active`
  - `screen_recording_active`
  - Moderate network spike (2-10x baseline)
- Info (tag only):
  - `suspicious_process_spawn` (may be false positive)
  - `unusual_memory_pressure`
  - `clipboard_image_during_camera`
**Deliverable**: severity classification for all tag types
**Priority**: P0
**Depends**: Issue 81
**Test**: each tag type has correct severity

### Issue 83: Emergency burn
**Goal**: On critical anomaly, burn all active chambers immediately.
**Tasks**:
- Emergency burn sequence:
  1. Stop camera ingestion
  2. Skip any pending inference
  3. Burn all active chambers (current + next if double-buffered)
  4. Zeroize all buffers
  5. Seal the integrity tag (happens before burn, so the tag survives)
  6. Resume camera with new chamber after a configurable cooldown (e.g., 5 seconds)
- Emergency burn must complete in < 50ms
**Deliverable**: emergency burn triggered by integrity monitor
**Priority**: P0
**Depends**: Issues 39, 82
**Test**: inject network spike → verify burn completes in < 50ms, tag sealed, camera resumes

### Issue 84: User alert notification
**Goal**: User sees integrity alerts as Android notifications.
**Tasks**:
- On warning or critical tag:
  - Android notification with:
    - Title: "Chamber Sentinel — Integrity Alert"
    - Body: human-readable description (e.g., "Unusual network activity detected")
    - Priority: HIGH for critical, DEFAULT for warning
  - Notification does NOT contain sensitive data (no frame content, no detailed process info)
  - Tap notification opens integrity dashboard
**Deliverable**: user-facing integrity notifications
**Priority**: P1
**Depends**: Issue 82
**Test**: trigger warning → notification appears

### Issue 85: Emergency burn cooldown
**Goal**: After emergency burn, wait before resuming to avoid burn loops.
**Tasks**:
- After emergency burn: wait N seconds (default: 5) before creating new chamber
- During cooldown: camera is off, no frames processed
- If anomaly persists after cooldown: burn again, double the cooldown (exponential backoff)
- Max cooldown: 5 minutes
- Log cooldown events as tags
**Deliverable**: burn → cooldown → resume cycle
**Priority**: P1
**Depends**: Issue 83
**Test**: trigger 3 consecutive anomalies → cooldowns are 5s, 10s, 20s

### Issue 86: False positive mitigation
**Goal**: Reduce false positives from integrity monitors.
**Tasks**:
- Network monitor:
  - Ignore first 10 seconds after chamber start (settling period)
  - Use rolling average over 5 readings, not single spike
  - Whitelist: system UID traffic (phone calls, system services)
- Process monitor:
  - Whitelist: launcher, keyboard, system UI, notification shade
  - Only flag truly unknown processes
- Memory monitor:
  - Ignore gradual changes (app lifecycle, GC)
  - Only flag sudden drops > 100MB
**Deliverable**: tuned anomaly thresholds
**Priority**: P1
**Depends**: Issue 79
**Test**: normal phone usage during camera operation → < 1 false positive per hour

### Issue 87: Integrity log persistence
**Goal**: Integrity tags persist across app restarts and are reviewable.
**Tasks**:
- Tags stored in the same vault database as events (Issue 62)
- Table: `integrity_tags(id, tag_type, timestamp, severity, detail_json, action_taken, chamber_id)`
- Queryable: by severity, by time range, by tag type
- Exportable: JSON export for forensic analysis (events + tags)
**Deliverable**: persistent, queryable integrity log
**Priority**: P0
**Depends**: Issues 62, 81
**Test**: generate 10 tags, kill app, relaunch, verify 10 tags in database

### Issue 88: Integrity log rotation
**Goal**: Old integrity logs are pruned to prevent unbounded storage growth.
**Tasks**:
- Retention policy: keep tags for N days (default: 30)
- Keep events for M days (default: 90)
- On app start: prune expired records
- Max storage: 50MB for vault database
**Deliverable**: bounded integrity log storage
**Priority**: P2
**Depends**: Issue 87
**Test**: insert 10,000 tags with old timestamps → prune reduces to expected count

---

## Epic 4.3: Root-level monitors (Issues 89–95)

### Issue 89: Root detection
**Goal**: Detect whether the device is rooted and enable/disable root-level monitors.
**Tasks**:
- Check for:
  - `su` binary in PATH
  - Magisk files
  - Custom recovery
  - SELinux permissive mode
  - Test `Runtime.exec("su")` response
- If rooted: enable root-level monitors (Issues 90–95)
- If not rooted: skip root monitors, log "root monitors unavailable"
- Do NOT refuse to run on rooted devices — just enable additional monitoring
**Deliverable**: root detection with monitor gating
**Priority**: P1
**Depends**: none
**Test**: test on rooted device → root monitors enabled. Test on stock A55 → root monitors skipped.

### Issue 90: V4L2 device consumer tracking
**Goal**: Track which processes open the camera at the kernel level.
**Tasks**:
- Requires root: read `/proc/*/fd` and check for symlinks to `/dev/video*`
- Every 2 seconds: scan all processes for camera device file descriptors
- Expected consumers: camera service PID + our app PID
- Unexpected consumer: tag `unauthorized_v4l2_consumer` with PID and process name
**Deliverable**: kernel-level camera consumer tracking
**Priority**: P1
**Depends**: Issue 89
**Test**: (rooted device) open camera in two apps → tag for unexpected consumer

### Issue 91: DMA buffer consumer tracking
**Goal**: Track which processes hold references to camera DMA buffers.
**Tasks**:
- Requires root: read `/sys/kernel/debug/dma-buf/bufinfo`
- Parse: buffer size, attached device, exporter, consumer count
- Filter for camera-related buffers (exporter contains "cam" or "isp")
- Expected consumer count: 2 (camera service + our app)
- Extra consumer: tag `unauthorized_dma_consumer`
**Deliverable**: DMA buffer tracking
**Priority**: P2
**Depends**: Issue 89
**Test**: (rooted device) verify expected consumer count during normal operation

### Issue 92: Kernel module monitor
**Goal**: Detect new kernel modules loaded during camera operation.
**Tasks**:
- Requires root: `inotify` on `/sys/module/` or periodic `lsmod` diff
- Baseline: module list at app start
- New module during camera operation: tag `kernel_module_loaded` with module name
- Known-bad modules: any name containing "capture", "hook", "inject", "keylog"
**Deliverable**: kernel module loading detection
**Priority**: P2
**Depends**: Issue 89
**Test**: (rooted device) `insmod` a test module during camera → tag generated

### Issue 93: SELinux audit monitoring
**Goal**: Detect SELinux policy violations related to camera access.
**Tasks**:
- Requires root: read `dmesg` or `/dev/kmsg` filtered for `avc: denied` with camera-related contexts
- Interesting contexts: `camera_device`, `video_device`, `mediaserver`
- A SELinux denial means something tried to access the camera outside its policy
- Tag: `selinux_camera_denial` with the denial message
**Deliverable**: SELinux audit monitoring for camera
**Priority**: P2
**Depends**: Issue 89
**Test**: (rooted device) trigger SELinux denial via `cat /dev/video0` from adb shell → tag generated

### Issue 94: Socket monitoring
**Goal**: Detect new network sockets created during camera operation.
**Tasks**:
- Requires root: read `/proc/net/tcp` and `/proc/net/tcp6`
- Baseline: socket list before camera starts
- New socket during camera operation: tag `new_socket_during_camera` with local/remote address and owning UID
- Filter: ignore sockets from system UIDs (0, 1000, etc.)
**Deliverable**: socket creation monitoring
**Priority**: P2
**Depends**: Issue 89
**Test**: (rooted device) create a TCP connection from adb while camera running → tag generated

### Issue 95: eBPF probe (research only)
**Goal**: High-performance kernel-level monitoring via eBPF.
**Tasks**:
- Requires root + kernel with eBPF support (Android 12+, kernel 5.4+)
- Attach eBPF programs to:
  - `v4l2_ioctl` — monitor every camera ioctl call
  - `sendto` / `send` — monitor every network send syscall
  - `mmap` — monitor memory mapping of camera-related regions
- eBPF provides near-zero overhead kernel tracing
- Output: per-event monitoring data fed to integrity monitor
- This is a research deliverable, not consumer-facing
**Deliverable**: eBPF-based camera pipeline monitor
**Priority**: P3
**Depends**: Issue 89
**Test**: (rooted device with eBPF) attach probe to v4l2_ioctl → log every camera operation

---

# Milestone 5 — Hardening (Issues 96–108)

## Epic 5.1: Application-layer hardening (Issues 96–102)

### Issue 96: Disable all debugging features
**Goal**: Release build has zero debugging capability.
**Tasks**:
- ProGuard/R8: obfuscate code, strip debug info
- Set `android:debuggable="false"` in manifest
- `prctl(PR_SET_DUMPABLE, 0)` in native code
- Strip symbols from `libchamber_core.so`
- Verify: `adb jdwp` does not list our PID
- Verify: `adb shell run-as` fails
**Deliverable**: release build is not debuggable
**Priority**: P0
**Depends**: Issue 3
**Test**: all debugging attachment methods fail

### Issue 97: WebView isolation (if used)
**Goal**: If any WebView is used for UI, it must be incognito.
**Tasks**:
- If using WebView for settings or info pages:
  - Set `setPrivateBrowsingEnabled(true)` (deprecated) or clear data on exit
  - Disable JavaScript if not needed
  - Disable file access: `setAllowFileAccess(false)`
  - Disable content access: `setAllowContentAccess(false)`
- Prefer native Android UI over WebView when possible
**Deliverable**: WebView is isolated or not used
**Priority**: P1
**Depends**: none
**Test**: no WebView cache/data in app private storage

### Issue 98: Clipboard isolation
**Goal**: App never reads from or writes to system clipboard.
**Tasks**:
- Do not call `ClipboardManager.setPrimaryClip()` anywhere
- Do not read clipboard in any code path (except the monitoring in Issue 76)
- Verify: no clipboard-related API calls in the codebase (grep audit)
**Deliverable**: zero clipboard interaction (except monitoring)
**Priority**: P1
**Depends**: none
**Test**: code audit: no setPrimaryClip calls

### Issue 99: Restrict app components for Android 14
**Goal**: All Android 14 security features enabled.
**Tasks**:
- `android:enableOnBackInvokedCallback="true"` (predictable back behavior)
- `android:requestLegacyExternalStorage="false"`
- Photo picker only (no broad storage access)
- Actually: we don't need photo picker either — we never access photos
- `<queries>` element: we don't query any other packages
- Set `android:localeConfig` to prevent locale-based attacks
**Deliverable**: Android 14 security features enabled
**Priority**: P1
**Depends**: Issue 1
**Test**: target SDK validation passes

### Issue 100: Certificate pinning (if any network added later)
**Goal**: Placeholder for future network security.
**Tasks**:
- Currently: no network. No INTERNET permission.
- If network is ever added (e.g., for sealed event sync):
  - Certificate pinning in network_security_config.xml
  - Pinned to specific server certificate
  - No fallback to system trust store
- Document: any network addition must go through security review
**Deliverable**: network security framework ready (unused)
**Priority**: P3
**Depends**: none
**Test**: N/A (no network)

### Issue 101: Tamper detection
**Goal**: Detect if the APK has been modified after signing.
**Tasks**:
- At startup: verify APK signature matches expected value
- Check: `PackageManager.GET_SIGNATURES` → hash → compare to hardcoded value
- If mismatch: refuse to start, display "Tampered" warning
- This catches: repackaged APKs, injected code, Xposed module modifications
**Deliverable**: APK tamper detection
**Priority**: P1
**Depends**: Issue 1
**Test**: modify APK, resign with different key → app refuses to start

### Issue 102: SafetyNet / Play Integrity attestation
**Goal**: Verify device integrity via Google's attestation API.
**Tasks**:
- Call Play Integrity API (or SafetyNet, if Integrity API unavailable)
- Check: MEETS_DEVICE_INTEGRITY (device is not rooted/modified)
- If integrity check fails: warn user but allow operation (root monitors are useful too)
- Log integrity status as a startup tag
**Deliverable**: device integrity attestation
**Priority**: P2
**Depends**: none
**Test**: check on stock A55 → passes. Check on rooted device → fails (warning shown).

---

## Epic 5.2: Traffic baseline calibration (Issues 103–108)

### Issue 103: Baseline measurement protocol
**Goal**: Establish network traffic baseline for the specific device.
**Tasks**:
- On first run, before camera starts:
  1. Measure device TX/RX for 60 seconds in 500ms intervals
  2. Compute: mean, std dev, p95
  3. Store as baseline profile (encrypted SharedPreferences)
- Baseline factors: WiFi connected, cellular connected, bluetooth active
- Separate baselines for WiFi vs cellular vs airplane mode
**Deliverable**: device-specific traffic baseline
**Priority**: P0
**Depends**: Issue 71
**Test**: baseline established, stored, retrievable on next run

### Issue 104: Adaptive baseline update
**Goal**: Baseline evolves as device behavior changes.
**Tasks**:
- After each monitoring session: update baseline with exponential moving average
- Weight: 90% old baseline, 10% new data
- This adapts to changes in background app behavior, system updates, etc.
- Never update baseline during anomaly (would pollute the reference)
**Deliverable**: self-calibrating traffic baseline
**Priority**: P1
**Depends**: Issue 103
**Test**: baseline shifts gradually over 10 sessions

### Issue 105: Per-app traffic attribution
**Goal**: Identify which app is responsible for network anomalies.
**Tasks**:
- `TrafficStats.getUidTxBytes(uid)` for each running UID
- Top-N UIDs by TX bytes during anomaly window
- Include in tag: list of UIDs and their TX byte counts
- Resolve UID → package name via PackageManager
**Deliverable**: traffic anomaly attributed to specific app
**Priority**: P1
**Depends**: Issue 71
**Test**: during anomaly, tag contains the responsible app's package name

### Issue 106: WiFi vs cellular baseline separation
**Goal**: Separate baselines for different network types.
**Tasks**:
- `ConnectivityManager.getActiveNetwork()` → type
- Maintain separate baseline profiles for WiFi, cellular, and no-network
- Anomaly thresholds may differ (cellular has more variable traffic)
**Deliverable**: network-type-aware baselines
**Priority**: P2
**Depends**: Issue 103
**Test**: WiFi baseline and cellular baseline are stored separately

### Issue 107: Baseline export for forensic analysis
**Goal**: Export baseline + anomaly data for offline analysis.
**Tasks**:
- Export as JSON: baseline profile, all tags, all events
- Export only — no import (one-way)
- Export to a user-chosen location (via Android's SAF file picker)
- The export file is NOT encrypted (it contains only events and tags, no frames)
**Deliverable**: forensic data export
**Priority**: P2
**Depends**: Issue 87
**Test**: export file is valid JSON, contains all events and tags

### Issue 108: Baseline visualization
**Goal**: User can see their device's traffic baseline.
**Tasks**:
- Simple chart: normal traffic pattern (green band) with current reading (dot)
- Anomalies highlighted (red dots outside the band)
- Last 24 hours of data
**Deliverable**: traffic baseline visualization
**Priority**: P3
**Depends**: Issue 103
**Test**: chart renders with real baseline data

---

# Milestone 6 — Root-Level Monitoring Research Prototype (Issues 109–127)

*Requires rooted Samsung A55 or custom ROM. Not for consumer release.*

## Epic 6.1: Root environment setup (Issues 109–112)

### Issue 109: Root the A55 (research device only)
**Goal**: Obtain root access on a dedicated research A55.
**Tasks**:
- Unlock bootloader (Samsung: enable OEM unlock in developer options)
- Flash TWRP or equivalent custom recovery
- Install Magisk for systemless root
- Verify: `adb shell su -c id` returns `uid=0(root)`
- WARNING: unlocking bootloader trips Knox, permanently disabling Knox security features. Use a SEPARATE device from the consumer target.
**Deliverable**: rooted A55 for research
**Priority**: P1
**Depends**: none
**Test**: `su` works

### Issue 110: SELinux permissive mode (research only)
**Goal**: Allow root monitors to access kernel debugging interfaces.
**Tasks**:
- `setenforce 0` (permissive mode)
- Verify: `/sys/kernel/debug/` is accessible
- Verify: `/proc/*/fd` is readable for all processes
- WARNING: this disables critical security. Research device only.
**Deliverable**: kernel debug interfaces accessible
**Priority**: P1
**Depends**: Issue 109
**Test**: `ls /sys/kernel/debug/dma-buf/` succeeds

### Issue 111: eBPF availability check
**Goal**: Verify eBPF is available on the rooted A55's kernel.
**Tasks**:
- Check kernel version: `uname -r` (needs 5.4+ for useful eBPF)
- Check `CONFIG_BPF`, `CONFIG_BPF_SYSCALL`, `CONFIG_BPF_JIT` in `/proc/config.gz`
- Test: load a minimal eBPF program
- If unavailable: fall back to ftrace or kprobes
**Deliverable**: eBPF availability confirmed or fallback identified
**Priority**: P2
**Depends**: Issue 109
**Test**: minimal eBPF program loads and runs

### Issue 112: Research monitor app variant
**Goal**: Build variant of Chamber Sentinel with root monitor code enabled.
**Tasks**:
- Gradle build flavor: `researchDebug` vs `consumerRelease`
- Research build: includes root monitors (Issues 90–95), requires root
- Consumer build: excludes root monitors, works on stock device
- Both share the same camera + substrate + non-root monitor code
**Deliverable**: dual build variants
**Priority**: P1
**Depends**: Issue 89
**Test**: research build has root monitors, consumer build does not

---

## Epic 6.2: Kernel-level data collection (Issues 113–120)

### Issue 113: DMA buffer forensics
**Goal**: Collect detailed DMA buffer allocation/deallocation traces during camera operation.
**Tasks**:
- Read `/sys/kernel/debug/dma-buf/bufinfo` every 500ms
- Record: buffer sizes, attachment counts, exporter names
- Correlate with camera frame timing
- Output: timeline of DMA buffer lifecycle during a chamber session
**Deliverable**: DMA buffer trace data
**Priority**: P2
**Depends**: Issue 91
**Test**: 30-second trace with camera active

### Issue 114: ISP firmware behavior profiling
**Goal**: Understand what the Exynos ISP does with camera data.
**Tasks**:
- Trace ISP-related kernel messages: `dmesg | grep -i isp`
- Monitor ISP memory regions (if exposed in sysfs)
- Document: which memory regions the ISP writes to, how long buffers live
- This is observational — we cannot modify ISP behavior
**Deliverable**: ISP behavior documentation for A55
**Priority**: P3
**Depends**: Issue 110
**Test**: ISP trace collected during 30-second camera session

### Issue 115: Camera service syscall trace
**Goal**: Record every system call the camera service makes during a chamber session.
**Tasks**:
- `strace -p $(pidof cameraserver) -e trace=write,sendto,sendmsg,ioctl`
- Filter for interesting calls: writes to unexpected file descriptors, network sends
- Duration: 30-second trace during camera operation
- Output: annotated syscall log
**Deliverable**: camera service syscall trace
**Priority**: P2
**Depends**: Issue 110
**Test**: trace collected, annotated with suspicious calls (if any)

### Issue 116: Gralloc buffer tracking
**Goal**: Track gralloc buffer allocation and sharing for camera frames.
**Tasks**:
- Gralloc buffers are shared GPU/CPU memory used for camera frames
- Read `/proc/$(pidof surfaceflinger)/maps` for gralloc regions
- Track: which processes map the same gralloc buffers
- Expected: only camera service and our app
- Unexpected mapping: evidence of frame interception
**Deliverable**: gralloc sharing analysis
**Priority**: P3
**Depends**: Issue 110
**Test**: list processes with access to camera gralloc buffers

### Issue 117: eBPF v4l2_ioctl probe
**Goal**: Monitor every V4L2 ioctl call at the kernel level.
**Tasks**:
- Write eBPF program that attaches to `v4l2_ioctl` kprobe
- Log: calling PID, ioctl command, buffer address
- Expected callers: camera service only
- Unexpected caller: evidence of unauthorized camera access at kernel level
**Deliverable**: eBPF-based camera ioctl monitoring
**Priority**: P3
**Depends**: Issue 111
**Test**: eBPF probe logs ioctl calls during camera operation

### Issue 118: eBPF network send probe
**Goal**: Monitor every `sendto`/`sendmsg` syscall for potential exfiltration.
**Tasks**:
- eBPF program on `sys_enter_sendto` and `sys_enter_sendmsg`
- Log: calling PID, destination address/port, byte count
- Filter: calls during camera operation window
- Cross-reference: is the calling PID associated with camera data?
**Deliverable**: network exfiltration detection at syscall level
**Priority**: P3
**Depends**: Issue 111
**Test**: send data from adb during camera → eBPF logs the send

### Issue 119: Memory mapping probe
**Goal**: Detect if any process memory-maps camera-related regions.
**Tasks**:
- eBPF program on `sys_enter_mmap`
- Filter: mmap calls targeting camera DMA buffer addresses
- Expected: camera service + our app
- Unexpected: evidence of memory-level frame interception
**Deliverable**: memory mapping detection for camera regions
**Priority**: P3
**Depends**: Issue 111
**Test**: document expected mmap patterns during camera operation

### Issue 120: Kernel monitor integration
**Goal**: Feed all root-level monitor data into the same integrity tagging system.
**Tasks**:
- Root monitors produce the same tag format as non-root monitors
- Tags sealed in the same vault
- Severity classification includes root-level findings
- Root findings have higher confidence (kernel-level evidence) vs non-root (inference)
**Deliverable**: unified tagging from kernel + app monitors
**Priority**: P1
**Depends**: Issues 90–95, 81
**Test**: root monitor tag and non-root monitor tag coexist in vault

---

## Epic 6.3: Research analysis and documentation (Issues 121–127)

### Issue 121: Camera pipeline trust boundary map
**Goal**: Document the exact trust boundary for the A55's camera pipeline.
**Tasks**:
- Diagram: sensor → ISP → kernel driver → DMA → camera service → gralloc → app
- For each stage: who controls it, what can observe it, what can modify it
- Mark: where Chamber Sentinel protects, where it monitors, where it's blind
**Deliverable**: `docs/a55-camera-trust-boundary.md`
**Priority**: P1
**Depends**: Issues 113–119
**Test**: document reviewed for accuracy

### Issue 122: Exfiltration detection accuracy report
**Goal**: Measure the integrity monitor's detection rate.
**Tasks**:
- Create controlled exfiltration scenarios:
  1. Large file download during camera (network spike) → should detect
  2. Small trickle exfiltration (10 KB/sec) → may miss
  3. Camera opened by second app → should detect
  4. Process spawned that reads /dev/video → should detect (root only)
  5. Clipboard copy during operation → should detect
- For each scenario: run 10 times, measure detection rate and time-to-detect
- Report: true positive rate, false positive rate, detection latency
**Deliverable**: `docs/detection-accuracy-report.md`
**Priority**: P1
**Depends**: Issues 79, 89
**Test**: report generated with measured rates

### Issue 123: Battery and performance impact report
**Goal**: Comprehensive performance analysis on A55.
**Tasks**:
- Measure:
  - Battery drain: standby vs camera-only vs camera+inference vs camera+inference+monitors
  - CPU usage per component
  - Memory usage per component
  - Thermal profile over 1 hour
  - Frame-to-event latency histogram
  - Chamber roll latency histogram
- Compare: non-root monitoring overhead vs root monitoring overhead
**Deliverable**: `docs/performance-report.md`
**Priority**: P1
**Depends**: Issues 66, 79
**Test**: 1-hour test on A55 with all measurements

### Issue 124: Covert channel analysis
**Goal**: Analyze potential covert channels through sealed events.
**Tasks**:
- Can an attacker encode frame data through:
  - Event timing patterns? (mitigated by timestamp quantization)
  - Event rate patterns? (mitigated by rate limiting)
  - Confidence value precision? (mitigated by schema: 2 decimal places = 7 bits per event)
  - Event type sequence? (6 types = 2.6 bits per event)
- Calculate: maximum covert channel bandwidth through sealed events
- Example: 10 events/minute * 2.6 bits/event = 26 bits/minute = 0.43 bits/second
  - At 0.43 bps, exfiltrating a single 100KB image takes ~54 hours
  - This is negligibly slow — the "attacker" would need days per frame
**Deliverable**: `docs/covert-channel-analysis.md`
**Priority**: P1
**Depends**: Issues 59, 60, 61
**Test**: calculated channel bandwidth documented

### Issue 125: Comparison with existing camera privacy solutions
**Goal**: Compare Chamber Sentinel against existing approaches.
**Tasks**:
- Compare against:
  - Google's on-device ML (processes locally but stores results with images)
  - Apple's on-device photo analysis (scans but retains photos)
  - Ring/Nest cameras (cloud-processed, stored indefinitely)
  - Haven (Guardian Project — uses sensors for physical security, different model)
  - Standard "privacy camera" apps (blur faces but retain footage)
- For each: residue after deletion, predictability, covert channel risk
**Deliverable**: `docs/competitive-analysis-camera.md`
**Priority**: P2
**Depends**: none
**Test**: document complete

### Issue 126: Research paper draft
**Goal**: Draft a research paper on ephemeral camera processing with integrity monitoring.
**Tasks**:
- Sections: introduction, threat model, architecture, implementation, evaluation, limitations
- Include: A55-specific findings, detection accuracy data, performance data, covert channel analysis
- Target venue: privacy/security workshop or systems conference
**Deliverable**: paper draft in `docs/paper/`
**Priority**: P2
**Depends**: Issues 121–125
**Test**: draft complete, internally reviewed

### Issue 127: Open-source release preparation
**Goal**: Prepare the codebase for public release.
**Tasks**:
- License: choose license (MIT or Apache 2.0)
- README: installation, usage, architecture, limitations
- Remove any hardcoded test credentials or device-specific paths
- CI: GitHub Actions for Android build + Rust cross-compilation
- Contribution guidelines
- Security policy (SECURITY.md)
**Deliverable**: publishable open-source project
**Priority**: P2
**Depends**: all previous
**Test**: fresh clone → build → install → run on A55

---

# Dependency Graph (Simplified)

```
M1: Substrate on Android
  Issues 1-6 (scaffold) → Issues 7-16 (port substrate) → Issues 17-21 (StrongBox) → Issues 22-28 (hardening)

M2: Camera Ingestion
  Issues 29-36 (Camera2) → Issues 37-43 (rolling chambers) → Issues 44-48 (burn verification)
  Depends on: M1 complete

M3: Detection Model
  Issues 49-55 (model integration) → Issues 56-62 (event sealing) → Issues 63-68 (performance)
  Depends on: M2 (camera frames flowing)

M4: Integrity Monitor
  Issues 69-80 (non-root monitors) → Issues 81-88 (anomaly response)
  Issues 89-95 (root monitors) — parallel, independent
  Depends on: M2 (camera running for context)

M5: Hardening
  Issues 96-102 (app hardening) — can start after M1
  Issues 103-108 (traffic baseline) — needs M4 monitors

M6: Research Prototype
  Issues 109-112 (root setup) → Issues 113-120 (kernel data) → Issues 121-127 (analysis)
  Depends on: M4 (integrity framework)
```

---

# Summary

| Milestone | Issues | Duration | Dependency |
|-----------|--------|----------|-----------|
| M1 Substrate on Android | 1–28 | 2 weeks | None |
| M2 Camera Ingestion | 29–48 | 2 weeks | M1 |
| M3 Detection Model | 49–68 | 2 weeks | M2 |
| M4 Integrity Monitor | 69–95 | 2 weeks | M2 |
| M5 Hardening | 96–108 | 1 week | M1, M4 |
| M6 Research Prototype | 109–127 | 2 weeks | M4 |

**Total: 127 issues, 11 weeks estimated.**
