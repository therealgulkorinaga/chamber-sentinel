# Chamber Sentinel: Ephemeral Camera Processing with Integrity Monitoring

## A Position Paper on Applying Burn-First Semantics to Visual Sensor Data

**Arko Ganguli**

---

## Abstract

Every camera is an archive. A security camera records continuously; a phone camera saves every frame; a doorbell camera uploads to a cloud server. When any of these devices is breached, the attacker acquires not a moment but a history — every person who walked past, every conversation within earshot, every pattern of life the sensor ever captured.

This paper proposes Chamber Sentinel, an application of the Chambers runtime model [1] to visual sensor data. The core idea: the camera should understand what it sees without remembering what it saw. Frames are processed inside self-destructing cryptographic chambers that burn every second. What survives: event labels ("person at door, 3:47 PM"). What burns: the footage.

On top of the ephemeral processing, an integrity monitor watches the camera pipeline — from kernel DMA buffers to network sockets — and tags any unauthorized access to camera data. The tags survive burn. The frames don't. The attacker gets caught. The footage they tried to steal is already gone.

The system does not claim protection at the kernel or firmware layer. It claims protection from the application layer inward, with monitoring (not prevention) of lower layers. The threat model this addresses: device theft, cloud breach, bulk surveillance, and opportunistic malware — not nation-state firmware implants.

---

## 1. Introduction

The Chambers position paper [1] proposes a world-first runtime where bounded computational worlds are the primary unit of persistence and destruction, governed by explicit preservation law and evaluated in terms of semantic residue. The paper demonstrates that a substrate with typed objects, a closed primitive algebra, and cryptographic burn can achieve zero undeclared residue after destruction — outperforming disposable VM and container baselines on metadata retention and reconstruction feasibility.

This paper asks: what happens when you apply that model to a camera?

The question is not trivial. A camera produces data continuously — 30 frames per second, each frame 1-3 megabytes. The data is sensitive (it captures people, spaces, activities). The device is exposed (physically accessible, network-connected, often running commodity firmware). And the conventional approach — store everything, encrypt at rest, control access — fails the moment the device is compromised, because the encrypted archive and its key coexist on the same device.

Chamber Sentinel inverts the model. Instead of storing frames and protecting the archive, it processes frames inside ephemeral chambers that burn every N frames. The only data that crosses the preservation boundary is a structured event label: what happened, when, and at what confidence. The frames themselves — the raw visual evidence — are encrypted under a per-chamber key that is destroyed within seconds of the frames being captured.

---

## 2. Relationship to the Chambers Model

Chamber Sentinel is a direct application of the Chambers architecture [1] with three adaptations for continuous sensor data:

### 2.1 Rolling Chambers

The original Chambers model describes a single chamber session: create → explore → converge → finalize → burn. This works for bounded tasks (decisions, research, negotiations) where the human decides when to burn.

A camera has no natural stopping point. It produces frames continuously. Chamber Sentinel introduces **rolling chambers**: each chamber lives for N frames (default: 30, or 1 second at 30fps). When the window expires, the current chamber burns and a new one takes its place. The key rotates per chamber, providing forward secrecy: compromise of the current key reveals only the current 1-second window. All previous windows are unrecoverable.

```
t=0.0s  K_w1 generated → frames 0-29 encrypted
t=1.0s  K_w1 destroyed → frames 0-29 unrecoverable
        K_w2 generated → frames 30-59 encrypted
t=2.0s  K_w2 destroyed → frames 30-59 unrecoverable
        K_w3 generated → ...
```

This is analogous to Signal Protocol's Double Ratchet [2], where session keys evolve per message. Chamber Sentinel's keys evolve per time window. The forward-secrecy analogy noted in the original Chambers paper [1, Section 7.4.1] applies directly.

### 2.2 Simplified Grammar

The Decision Chamber grammar [1] has 10 object types, 9 primitives, and a multi-phase lifecycle (exploring → reviewing → finalizing). A camera chamber needs far less:

- **3 object types**: `frame` (temporary), `detection` (temporary), `event_summary` (preservable)
- **3 primitives**: CreateObject, SealArtifact, TriggerBurn
- **1 phase**: Active (auto-burn after N frames, no convergence review needed)
- **Preservation law**: only `event_summary` survives

The legal execution surface [1, Section 4] is even narrower than the Decision Chamber: 3 operations instead of 9. Fewer operations means fewer channels through which residue can be created, leaked, or retained.

### 2.3 Integrity Monitoring (New Contribution)

The original Chambers paper focuses on the substrate's own integrity — the trusted-substrate assumption [1, Section 2]. It acknowledges that "if compromised, Chambers collapses into managed theater" but does not propose mechanisms for detecting compromise.

Chamber Sentinel adds an **integrity monitor** that watches the camera pipeline for unauthorized access. The monitor operates at the application layer (no root required) and optionally at the kernel layer (with root). It cannot prevent kernel-level exfiltration, but it can detect it and create sealed forensic evidence.

This is a new architectural component not present in the original Chambers model: a monitoring layer that produces sealed integrity tags as artifacts. The tags survive burn. The frames don't. An attacker may exfiltrate frames from the kernel layer, but the integrity monitor detects the anomaly, and the evidence of the attack survives for forensic review.

---

## 3. Camera Pipeline Trust Analysis

A camera frame passes through multiple layers before it reaches the application. Each layer presents a different trust relationship.

### Layer 0: Hardware / Bootloader

The camera sensor, image signal processor (ISP), and boot chain are controlled by the device manufacturer. The ISP is a dedicated processor with its own firmware — it receives raw Bayer data from the sensor, performs debayering, noise reduction, and white balance, and outputs processed frames into kernel memory. This firmware is a black box.

**Chamber Sentinel's position**: not protected, not monitored. Below the trust boundary. If the ISP firmware is compromised, frames are exfiltrated before any software can intervene.

### Layer 1: Kernel

The Linux kernel manages the camera via V4L2 drivers. Frames exist in plaintext in kernel DMA buffers. Any kernel module or root process can read these buffers.

**Chamber Sentinel's position**: not protected, but monitored. The integrity monitor can detect:
- Unauthorized processes opening `/dev/video*` (V4L2 consumer tracking)
- New kernel modules loaded during camera operation (rootkit detection)
- SELinux policy violations for camera-related contexts
- Unusual DMA buffer consumer counts

These monitors require root access. On stock Android, they are unavailable. On a rooted research device, they provide kernel-level visibility.

### Layer 2: Camera Service

Android's Camera Service delivers frames to applications via ImageReader. The frame passes through kernel DMA → camera service → gralloc buffer → app. Four plaintext copies exist before the application receives the frame.

**Chamber Sentinel's position**: partially protected, monitored. The application can:
- Detect other apps accessing the camera (CameraManager.AvailabilityCallback)
- Prevent screen capture of its own window (FLAG_SECURE)
- Operate without displaying a camera preview (no framebuffer exposure)

### Layer 3: Application

This is Chamber Sentinel's primary domain. The frame arrives in the app's ImageReader and is immediately encrypted under K_w (AES-256-GCM, hardware-accelerated on ARM). From this point:
- The plaintext frame is zeroed within microseconds
- All subsequent access is through the encrypted memory pool [1, Section 7]
- Inference happens in a guarded buffer (mlock'd, zeroed after use)
- Only sealed event labels cross the preservation boundary
- Burn destroys K_w — all encrypted frames become unrecoverable

### Layer 4: Output

Sealed events conform to a strict grammar-enforced schema: event type (enum), timestamp (quantized), confidence (2 decimal places). No pixel data, no embeddings, no bounding boxes, no free text. The schema is designed to minimize covert channel bandwidth through the preservation boundary.

### Summary

```
Layer 0 (Hardware/ISP)    NOT PROTECTED    NOT MONITORED
Layer 1 (Kernel)          NOT PROTECTED    MONITORED (root only)
Layer 2 (Camera Service)  PARTIAL          MONITORED
Layer 3 (Application)     PROTECTED        PROTECTED
Layer 4 (Output)          PROTECTED        PROTECTED (schema-enforced)
```

The honest claim: Chamber Sentinel protects from the application boundary inward and monitors downward. It cannot prevent kernel-level exfiltration. It can detect it.

---

## 4. Integrity Monitor Architecture

The integrity monitor is a background service that continuously watches the camera pipeline and tags anomalies. Tags are sealed artifacts that survive burn.

### 4.1 Non-Root Observables

These work on any stock Android device without special permissions:

| Observable | API | What it detects |
|-----------|-----|----------------|
| Camera consumers | CameraManager.AvailabilityCallback | Another app accessing the camera |
| App network egress | TrafficStats.getUidTxBytes | This app sending data (should be impossible — no INTERNET permission) |
| Device network egress | TrafficStats.getTotalTxBytes | Device-wide traffic spike during camera operation |
| File creation | FileObserver | Image/video files written during operation |
| Process spawning | ActivityManager.getRunningAppProcesses | New unknown processes during operation |
| Memory pressure | ActivityManager.MemoryInfo | Another process buffering frames |
| Screen recording | DisplayManager | Virtual displays or screen capture active |
| Accessibility services | Settings.Secure | Services that can read screen content |

### 4.2 Root Observables (Research Only)

These require a rooted device and are not available on stock Android:

| Observable | Method | What it detects |
|-----------|--------|----------------|
| V4L2 consumers | /proc/*/fd scanning | Which PIDs open the camera device at kernel level |
| DMA buffer consumers | /sys/kernel/debug/dma-buf | Unauthorized processes holding camera DMA buffers |
| Kernel modules | inotify on /sys/module | Rootkit or capture module loaded during operation |
| SELinux denials | dmesg filtering | Something bypassing camera access policy |
| Network sockets | /proc/net/tcp | New connections during operation |
| eBPF probes | Kernel attach points | High-performance monitoring of camera ioctls and network sends |

### 4.3 Anomaly Response

When an anomaly is detected:

1. **Tag**: create a structured integrity event with timestamp, check type, measured value, expected value, responsible process
2. **Seal**: the tag is sealed as an artifact (survives burn)
3. **Classify**: critical (emergency burn) vs warning (alert user) vs info (log only)
4. **Act**: on critical anomalies, trigger emergency burn — destroy all encrypted frames, zero all buffers, then resume with a new chamber after a cooldown period

The emergency burn ensures that even if the integrity monitor detects exfiltration after it has begun, the attacker's window is limited to the current chamber (1 second of frames). All previous chambers are already burned.

### 4.4 Forensic Output

After a session, the integrity log contains:
- All sealed events (person detected, vehicle detected, etc.)
- All integrity tags (anomalies detected, actions taken)
- Chamber statistics (chambers burned, frames processed, events sealed)

The integrity log contains no frame data, no pixel content, no image embeddings. It is a record of what the camera saw (event labels) and whether anyone tried to steal what it saw (integrity tags).

---

## 5. Covert Channel Analysis

A critical question: can a compromised detection model encode frame data through the sealed event channel?

The event schema constrains the output: event type (6 possible values = 2.6 bits), timestamp (quantized to 5 seconds), confidence (0.00-1.00 with 2 decimal places = ~7 bits). At the maximum event rate (10 events per minute), the covert channel bandwidth is approximately:

```
(2.6 + 7) bits/event × 10 events/min = 96 bits/min = 1.6 bits/sec
```

At 1.6 bits per second, exfiltrating a single 100KB compressed image would take approximately 54 hours. Exfiltrating one minute of 30fps video (180MB) would take approximately 2.8 years.

This bandwidth is negligibly slow for visual data exfiltration. The covert channel exists (any output channel is a potential covert channel) but its capacity is insufficient for meaningful image exfiltration. Reducing the event rate or quantizing timestamps further narrows the channel.

---

## 6. The Exfiltration Detection Insight

The most consequential property of Chamber Sentinel is not the frame encryption or the burn semantics — it is the **traffic baseline**.

A conventional camera has high, variable network traffic: streaming to NAS, cloud backup, motion alerts, firmware updates. An attacker's exfiltration traffic hides in this noise.

Chamber Sentinel has no legitimate network traffic. The app has no INTERNET permission. Android enforces this at the kernel level — the app process cannot create network sockets. The expected outbound traffic is exactly zero bytes.

Any outbound traffic from the device that correlates with camera operation is an anomaly. The detection rule is: "any data leaving that isn't background device traffic is suspicious during camera operation." One rule. Simple. Highly sensitive.

This transforms the detection problem from "find the needle in the haystack" (conventional camera) to "detect any straw that appears" (Chamber Sentinel). The chamber model creates an environment where exfiltration is not hidden — it is the only traffic that would exist.

---

## 7. Comparison with Existing Approaches

| System | Processes locally? | Stores footage? | Encrypts at rest? | Destroys footage? | Detects compromise? |
|--------|-------------------|----------------|-------------------|-------------------|-------------------|
| Ring/Nest | Cloud | Cloud (indefinite) | Yes (provider controls key) | Manual delete (not cryptographic) | No |
| Local NVR | Yes | Local disk (days-weeks) | Optional | Overwrite (not cryptographic) | No |
| Haven (Guardian Project) | Yes | Local (photos + sensor data) | Optional | Manual | No |
| Apple on-device ML | Yes | Retains photos + analysis | Yes (device key) | User-initiated | No |
| "Privacy cameras" (face blur) | Yes | Stores blurred footage | Varies | No — footage persists | No |
| **Chamber Sentinel** | **Yes** | **No** — frames burn every second | **Yes** — AES-256-GCM per chamber | **Yes** — cryptographic, per-second | **Yes** — integrity monitor |

No existing camera product combines: on-device processing, zero footage retention, per-second cryptographic destruction, and active compromise detection. Chamber Sentinel occupies a distinct position in the design space.

---

## 8. Limitations

**Kernel-level exfiltration is not prevented.** A compromised kernel, ISP firmware, or camera driver can exfiltrate frames before they reach the application. The integrity monitor may detect the resulting network anomaly but cannot prevent the initial capture.

**Real-time streaming cannot be stopped.** If a compromised component streams frames out continuously at the same rate they're captured, and has sufficient bandwidth, the frames leave before they burn. The integrity monitor detects the bandwidth anomaly, but the data is already gone.

**The detection model is a trust dependency.** If the model is compromised, it could encode frame information into detection outputs. The covert channel bandwidth is low (1.6 bits/sec) but nonzero. A model that is adversarially trained to maximize covert channel throughput could potentially extract more information than the schema analysis suggests.

**Event labels reveal information.** "Person at door at 3:47 PM" is itself information. An attacker who compromises the event vault (not the frames) learns the pattern of life — when people come and go, how often, for how long. This is accepted residue, analogous to the Chambers substrate's 2 existence-level events [1].

**Battery and thermal constraints.** Continuous camera + encryption + inference + integrity monitoring consumes significant power. Preliminary estimates suggest 15-20% additional battery drain per hour on the Samsung A55. Thermal throttling at sustained load may reduce detection accuracy.

**Android permission model is the actual enforcement.** The claim "no INTERNET permission = no network" relies on Android's UID-based iptables enforcement. If Android's permission model is bypassed (root exploit, kernel compromise), the network isolation fails. This is an OS trust dependency.

---

## 9. Target Hardware

The Samsung Galaxy A55 is the primary target for three reasons:

1. **StrongBox** — hardware key storage (Samsung Knox + ARM TrustZone). K_s lives in hardware, never in app memory. This provides Phase 3-level key protection [1] on a $300 consumer device.

2. **ARM crypto extensions** — AES-256-GCM runs in hardware at nanosecond latency. Encrypting a 3MB frame takes < 1ms. This is fast enough for real-time 30fps encryption without stalling the camera pipeline.

3. **Exynos ISP** — a dedicated image signal processor that handles frame preprocessing in hardware, freeing the CPU for encryption and inference.

The architecture is not Samsung-specific. Any Android device with Camera2 API, hardware keystore, and ARM crypto extensions can run Chamber Sentinel. The StrongBox requirement can fall back to TEE-backed keystore on devices without StrongBox.

---

## 10. Conclusion

Chamber Sentinel demonstrates that the Chambers model [1] extends beyond bounded decision tasks to continuous sensor processing. The key architectural insight is that a camera does not need to remember what it saw — it only needs to understand it. The understanding (event labels) is preserved. The evidence (frames) is burned.

The integrity monitor adds a capability the original Chambers model did not address: detecting compromise of the lower platform. While the monitor cannot prevent kernel-level exfiltration, it transforms the attacker's problem from "steal the archive at leisure" to "maintain a continuous high-bandwidth covert channel that is detectable by a simple traffic anomaly rule."

The honest claim is narrow: Chamber Sentinel protects from the application layer inward and monitors downward. It does not replace firewalls, OS hardening, or hardware attestation. It occupies a distinct position: a camera that sees everything, remembers nothing, and catches anyone who tries to steal what it saw.

---

## References

[1] A. Ganguli, "Chambers: Sealed Ephemeral Worlds for Private Cognition — A World-Based Position Paper on Persistence-Law-First, Burn-First, Task-Bounded Computing," 2026.

[2] Signal Protocol. Double Ratchet Algorithm. https://signal.org/docs/specifications/doubleratchet/

[3] Android Camera2 API. https://developer.android.com/reference/android/hardware/camera2/package-summary

[4] Samsung Knox Security. https://www.samsungknox.com/en/solutions/it-solutions/knox-platform-for-enterprise

[5] Android Keystore System. https://developer.android.com/training/articles/keystore

[6] ARM Cryptographic Extensions. https://developer.arm.com/documentation/ddi0500/latest

[7] Video4Linux2 API. https://www.kernel.org/doc/html/latest/userspace-api/media/v4l/v4l2.html

[8] Guardian Project, Haven. https://guardianproject.info/apps/org.havenapp.main/

[9] NIST SP 800-193, Platform Firmware Resiliency Guidelines. https://csrc.nist.gov/pubs/sp/800/193/final

[10] Wei et al. "Reliably Erasing Data From Flash-Based Solid State Drives." FAST 2011.
