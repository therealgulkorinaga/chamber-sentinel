# PRD — Chamber Sentinel

## Camera Integrity Monitor with Ephemeral Frame Processing

---

## 1. What this is

Chamber Sentinel is a camera application that processes video frames inside self-destructing chambers. The camera sees, understands, and forgets. What survives: event labels ("person at door, 3:47 PM"). What burns: the footage.

On top of the ephemeral processing, an integrity monitor watches the entire camera pipeline — from kernel DMA buffers to network sockets — and tags any unauthorized access to the camera data. The tags survive burn. The frames don't. The attacker gets caught. The footage is gone.

---

## 2. Core thesis

**The camera should understand what it sees without remembering what it saw.**

Today every camera is an archive. A breach exposes the entire history. A subpoena retrieves every frame. A stolen device contains every moment.

Chamber Sentinel makes a different trade: you get the understanding (events, detections, alerts) without the evidence (the footage). The AI sees it so you don't have to store it.

When the camera pipeline is compromised — at any layer — the integrity monitor detects the anomaly, tags it as a sealed forensic record, and triggers emergency burn. The frames the attacker tried to steal are already gone.

---

## 3. Target device

**Samsung Galaxy A55** (primary)

| Component | Spec | How we use it |
|-----------|------|--------------|
| SoC | Exynos 1480 (ARM Cortex-A78 + A55) | Runs the Rust substrate via NDK |
| RAM | 8GB | Substrate + quantized detection model |
| Camera | 50MP main (Samsung ISOCELL GN3) | Frame source |
| Security | Samsung Knox, ARM TrustZone, StrongBox | K_s in hardware (StrongBox), tamper detection (Knox) |
| Connectivity | WiFi, 5G, BLE | Traffic monitoring target |
| OS | Android 14 (One UI 6) | Camera2 API, TrafficStats, FileObserver |

Secondary targets: any Android device with Camera2 API + hardware keystore. The substrate is device-agnostic; the integrity monitor uses standard Android APIs.

---

## 4. Architecture — Layer by layer

### Layer 0: Hardware / Bootloader (NOT PROTECTED)

Samsung's domain. Secure Boot, boot ROM, TrustZone initialization. If compromised, everything above collapses. Chamber Sentinel accepts this as a trust dependency and does not claim protection at this layer.

**Accepted risk:** Supply chain attack, bootloader exploit, ISP firmware compromise.

### Layer 1: Kernel (MONITORED, NOT PROTECTED)

The Linux kernel manages camera hardware via V4L2 driver. The Exynos ISP outputs processed frames into kernel DMA buffers. Frames are plaintext in kernel memory.

Chamber Sentinel cannot prevent kernel-level access to frames. But it can **monitor** kernel behavior and detect anomalies:

- DMA buffer consumer tracking (with root)
- Kernel module load detection
- SELinux audit log monitoring
- System call tracing on camera service

**Accepted risk:** A kernel-level attacker sees frames before encryption. The integrity monitor detects and tags the anomaly.

### Layer 2: Camera Service (MONITORED, PARTIALLY PROTECTED)

Android's Camera Service delivers frames to apps via ImageReader. The frame passes through: kernel DMA → camera service → gralloc buffer → app ImageReader. Four plaintext copies.

Chamber Sentinel monitors this layer:

- Camera availability callbacks (detect other apps accessing camera)
- No camera preview displayed (no framebuffer exposure)
- FLAG_SECURE on app window (block screenshots/screen recording)

**Accepted risk:** Camera service, gralloc buffer, and GPU driver see plaintext frames.

### Layer 3: Application — The Chamber (PROTECTED)

This is Chamber Sentinel's domain. The frame arrives in ImageReader, is immediately encrypted under K_w, and the plaintext is zeroed. From this point, Chambers' full protection applies:

- Encrypted memory pool (AES-256-GCM under K_w)
- Guard buffer (mlock'd, zeroed after inference)
- K_w in StrongBox (hardware, never in app memory)
- No file I/O, no gallery, no cloud, no thumbnails
- Burn destroys K_w — all encrypted frames unrecoverable

### Layer 4: Output — Sealed Events (PROTECTED)

Only sealed events cross the preservation boundary. Grammar-enforced schema: event type, timestamp, confidence. No pixel data, no embeddings, no image descriptors.

### Layer 5: Integrity Monitor (NEW — crosses all layers)

A background thread that continuously watches the camera pipeline and tags anomalies. Tags are sealed artifacts — they survive burn.

---

## 5. Integrity monitor specification

### 5.1 Non-root observables (works on stock Android)

| Check | API | Frequency | Baseline | Anomaly |
|-------|-----|-----------|----------|---------|
| Camera consumers | `CameraManager.AvailabilityCallback` | Event-driven | Only this app | Any other app accessing camera |
| Network egress (app) | `TrafficStats.getUidTxBytes(myUid)` | Every 500ms | < 1 KB/sec (sealed events) | > 10 KB/sec |
| Network egress (device) | `TrafficStats.getTotalTxBytes()` | Every 500ms | Baseline ± 20% | Spike > 2x during frame processing |
| New files | `FileObserver` on DCIM, media, tmp dirs | Event-driven | 0 new image files during operation | Any image file created |
| Process list | `ActivityManager.getRunningAppProcesses()` | Every 2 sec | Stable set | New unknown process during operation |
| Memory pressure | `ActivityManager.MemoryInfo` | Every 2 sec | Stable | Unusual consumption (another process buffering frames) |
| App permissions | `PackageManager` | On start | Known set | New app with camera permission installed |

### 5.2 Root observables (research prototype / custom ROM)

| Check | Method | What it reveals |
|-------|--------|----------------|
| V4L2 consumers | `inotify` on `/dev/video*` | Which PIDs open the camera device |
| DMA buffer refs | `/sys/kernel/debug/dma-buf/bufinfo` | Which processes hold camera DMA buffers |
| Kernel modules | `inotify` on `/sys/module/` | New module loaded during operation = rootkit |
| SELinux denials | `dmesg` / `logcat -b events` | Something trying to bypass camera policy |
| Socket creation | `/proc/net/tcp` + `/proc/net/tcp6` | New connections during operation |
| Syscall trace | `ftrace` on camera service PID | Unexpected write() to unknown fd |
| eBPF probes | Attach to `v4l2_ioctl`, `sendto` | High-performance kernel-level monitoring |

### 5.3 Anomaly response

When any check produces an anomaly:

1. **Tag** — create an integrity event with timestamp, check type, measured value, expected value, process details
2. **Seal** — the tag becomes a sealed artifact (survives burn)
3. **Assess severity** — critical (unauthorized camera access, network spike) vs warning (new process, memory pressure)
4. **On critical:** emergency burn — destroy all encrypted frames immediately, then continue monitoring
5. **Alert user** — display integrity warning

### 5.4 Tag schema

```json
{
  "tag_type": "anomalous_exfiltration",
  "timestamp": "2026-04-05T15:47:03Z",
  "check": "network_egress_app",
  "expected": "< 1 KB/sec",
  "measured": "4.7 MB/sec",
  "duration_ms": 500,
  "process": { "pid": 4847, "name": "com.unknown.service" },
  "action_taken": "emergency_burn",
  "frames_at_risk": 12,
  "chamber_id": "019d5..."
}
```

---

## 6. Detection model

### 6.1 Requirements

- Runs on-device (no cloud inference — no network)
- Quantized for mobile (INT8 or Q4)
- Inference time < 100ms per frame (for real-time detection)
- Output: event labels only (no embeddings, no feature vectors)

### 6.2 Model options

| Model | Size | Inference time (A55) | Capability |
|-------|------|---------------------|-----------|
| MobileNet V3 + SSD | ~5MB | ~15ms | Object detection (person, car, package, animal) |
| YOLOv8n (nano) | ~6MB | ~25ms | Object detection, better accuracy |
| MediaPipe Object Detection | ~4MB | ~20ms | Google's on-device detection, optimized for ARM |
| Custom trained (transfer) | ~5MB | ~20ms | Domain-specific (front door, package, vehicle) |

### 6.3 Inference pipeline

```
Encrypted frame in EncryptedWorldState
  ↓ decrypt into guard buffer (8KB for metadata, frame stays encrypted)
  ↓ actually: decrypt frame into a larger mlock'd inference buffer
  ↓ resize to model input (e.g., 320x320)
  ↓ run model inference (15-25ms)
  ↓ output: [{ class: "person", confidence: 0.94, bbox: [x,y,w,h] }]
  ↓ zero the inference buffer
  ↓ bounding box info is discarded (not sealed — only event label survives)
  ↓ seal: { event: "person_detected", time: "3:47 PM", confidence: 0.94 }
```

---

## 7. Chamber lifecycle for camera

### 7.1 Continuous chamber mode

Unlike the Decision Chamber (one session, one burn), the camera runs in **rolling chambers**:

```
Chamber 1: frames 0-30    (1 second at 30fps)
  → detect → seal events → burn
Chamber 2: frames 31-60
  → detect → seal events → burn
Chamber 3: frames 61-90
  → detect → seal events → burn
  ...
```

Each chamber lives for N frames (configurable: 1 second, 5 seconds, 30 seconds). K_w rotates per chamber. If the device is compromised mid-chamber, at most N frames are at risk. All previous chambers are already burned.

### 7.2 Grammar — Camera Chamber

```
Objective class: camera_monitoring

Object types:
  - frame (temporary — burns)
  - detection (temporary — burns)
  - event_summary (preservable — survives)

Preservation law: only event_summary survives

Termination: auto-burn after N frames
```

### 7.3 Rolling key schedule

```
t=0.0s  K_w1 generated → encrypt frames 0-30
t=1.0s  K_w1 destroyed → frames 0-30 unrecoverable
        K_w2 generated → encrypt frames 31-60
t=2.0s  K_w2 destroyed → frames 31-60 unrecoverable
        K_w3 generated → ...
```

Forward secrecy per chamber. Compromise of K_w3 reveals only the current 1-second window. All previous windows are gone.

---

## 8. What the user sees

### 8.1 Main screen

A live view showing only sealed events (not camera preview):

```
┌─────────────────────────────────┐
│  CHAMBER SENTINEL               │
│  ● Monitoring (active)          │
│  Chamber #847 | 0 anomalies     │
├─────────────────────────────────┤
│  Today                          │
│  3:47 PM  Person at door        │
│  3:52 PM  Package delivered     │
│  6:12 PM  Person at door        │
│  6:13 PM  Person left           │
│  8:30 PM  Animal (cat)          │
│                                 │
│  No footage stored.             │
│  847 chambers burned today.     │
├─────────────────────────────────┤
│  Integrity: ✓ Clean             │
│  Network: 0.3 KB/sec            │
│  Camera consumers: 1 (this app) │
└─────────────────────────────────┘
```

No photos. No thumbnails. No video playback. Just event labels.

### 8.2 Integrity alert

```
┌─────────────────────────────────┐
│  ⚠ INTEGRITY ALERT              │
│                                  │
│  Anomalous network egress        │
│  detected at 3:47:03 PM         │
│                                  │
│  Expected: < 1 KB/sec           │
│  Measured: 4.7 MB/sec           │
│  Process: com.unknown.service    │
│                                  │
│  Action: Emergency burn executed │
│  Frames at risk: 12             │
│  Frames recovered by attacker:  │
│  Unknown — data burned.          │
│                                  │
│  [View Integrity Log]            │
└─────────────────────────────────┘
```

### 8.3 Integrity log (sealed, survives all burns)

```
┌─────────────────────────────────┐
│  INTEGRITY LOG                   │
│                                  │
│  2026-04-05 15:47:03             │
│  TAG: anomalous_exfiltration     │
│  Network: 4.7 MB/s (expected <1) │
│  Process: PID 4847               │
│  Action: emergency_burn          │
│                                  │
│  2026-04-05 09:12:44             │
│  TAG: unauthorized_camera_access │
│  Process: com.social.app         │
│  Action: alert_user              │
│                                  │
│  Total integrity events: 2      │
│  Total chambers burned: 847     │
│  Uptime: 14h 23m                │
└─────────────────────────────────┘
```

---

## 9. Privacy claims (honest)

### What Chamber Sentinel protects

| Threat | Protection |
|--------|-----------|
| Phone stolen — attacker looks for footage | No footage exists. Encrypted frames have no key. |
| Cloud breach — attacker accesses backup | No footage in backup. Only sealed events. |
| Subpoena for camera footage | No footage to produce. Integrity log and events available. |
| App-level malware reading camera data | Frames encrypted in app memory within microseconds |
| Insider at phone manufacturer | Sealed events contain no images, no pixels, no embeddings |

### What Chamber Sentinel does NOT protect

| Threat | Why |
|--------|-----|
| Kernel-level rootkit reading ISP buffers | Below trust boundary — frames plaintext in kernel |
| Compromised ISP firmware | Samsung's black box — we can't see inside |
| Real-time live streaming by compromised firmware | If exfiltration happens before the frame reaches the app, Chambers never sees it |
| Bootloader/supply chain attack | Below trust boundary |
| Physical camera observation (someone looks over your shoulder) | Not a digital threat |
| Covert channel through event timing | Partially mitigated by timestamp quantization |

### Honest summary

Chamber Sentinel protects from the application layer inward. It monitors the kernel layer for anomalies. It cannot protect the kernel layer. A nation-state attacker with firmware-level access can exfiltrate frames before they reach the app — but the integrity monitor will likely detect the network anomaly and tag it.

The threat model this product addresses: **device theft, cloud breach, bulk surveillance, and opportunistic malware.** Not: nation-state firmware implants.

---

## 10. Non-goals

- Not a CCTV replacement (no footage review, no playback)
- Not a doorbell product (no two-way audio, no cloud service)
- Not a general camera app (no photos, no gallery)
- Not an anonymization tool (no face blurring — faces are never stored)
- Not cross-platform in v1 (Android only, Samsung A55 primary)

---

## 11. Technical stack

| Component | Technology |
|-----------|-----------|
| App shell | Kotlin (Android) |
| Substrate runtime | Rust (via Android NDK + JNI) |
| Crypto | AES-256-GCM (ARM hardware acceleration) |
| Key storage | Android Keystore / StrongBox |
| Detection model | TFLite or ONNX Runtime (quantized) |
| Camera | Camera2 API + ImageReader |
| Network monitoring | TrafficStats API |
| File monitoring | FileObserver API |
| Build | Gradle (Android) + Cargo (Rust) |

---

## 12. Milestones

### M1 — Substrate on Android (2 weeks)

Port the Rust substrate to Android NDK. JNI bridge between Kotlin and Rust. Verify: create chamber, add objects, burn — all via JNI. StrongBox key storage for K_w.

**Exit criteria:** chamber lifecycle runs on A55, K_w in StrongBox.

### M2 — Camera ingestion (2 weeks)

Camera2 API integration. Frames arrive in ImageReader, immediately encrypted, stored in EncryptedWorldState. No preview, no file, no gallery. FLAG_SECURE on window.

**Exit criteria:** camera frames enter and exit chambers. No plaintext in app storage after burn.

### M3 — Detection model (2 weeks)

Integrate quantized object detection model. Inference inside guard buffer. Output: event labels only. Seal events as artifacts. Rolling chamber mode (1-second windows).

**Exit criteria:** real-time detection at 30fps. Events sealed. Frames burned every second.

### M4 — Integrity monitor (2 weeks)

All non-root observables implemented. Anomaly detection, tagging, sealed integrity log. Emergency burn on critical anomalies. User-facing integrity dashboard.

**Exit criteria:** network spike triggers emergency burn + sealed tag. Camera access by another app triggers alert.

### M5 — Hardening (1 week)

ptrace deny, core dump disable, mlock, WebView incognito (if any WebView used), clipboard isolation. Traffic baseline calibration. Timestamp quantization for events.

**Exit criteria:** Phase 2 equivalent hardening on Android.

### M6 — Root-level monitoring (research, optional) (2 weeks)

DMA buffer tracking, eBPF probes, kernel module detection. Requires rooted A55 or custom ROM. Not for consumer release — research prototype only.

**Exit criteria:** eBPF probe detects unauthorized DMA buffer consumer.

---

## 13. Success metrics

| Metric | Target |
|--------|--------|
| Frame-to-encryption latency | < 5ms (frame encrypted within 5ms of ImageReader callback) |
| Inference time per frame | < 100ms |
| Rolling chamber burn rate | 1 chamber/second (30 frames each) |
| Network baseline during operation | < 1 KB/sec |
| Anomaly detection latency | < 1 second from exfiltration start to emergency burn |
| Sealed event size | < 200 bytes per event |
| Battery impact | < 15% additional drain vs stock camera |
| Storage used | < 10 MB (sealed events only, no footage) |
| Integrity false positive rate | < 1 per day |

---

## 14. One-line outcome

If this works, you have a camera that sees everything, remembers nothing, and catches anyone who tries to steal what it saw.
