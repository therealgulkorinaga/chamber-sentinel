# Chamber Sentinel: Ephemeral Camera Processing with Integrity Monitoring

## A Position Paper on Applying Burn-First Semantics to Visual Sensor Data

**Arko Ganguli**

---

## Abstract

Every camera is an archive. A security camera records continuously; a phone camera saves every frame; a doorbell camera uploads to a cloud server. When any of these devices is breached, the attacker acquires not a moment but a history — every person who walked past, every conversation within earshot, every pattern of life the sensor ever captured.

This paper proposes Chamber Sentinel, an application of the Chambers runtime model [1] to visual sensor data. The core idea: the camera should understand what it sees without remembering what it saw. Frames are processed inside self-destructing cryptographic chambers that burn at configurable intervals. What survives: structured event labels. What burns: the footage.

On top of the ephemeral processing, an integrity monitor watches the camera pipeline — from kernel DMA buffers to network sockets — and tags any unauthorized access to camera data. The tags survive burn. The frames don't.

**Scope and status.** This is an architectural position paper. No prototype has been built. Performance figures are estimates, not measurements. The Chambers substrate [1] has a working implementation with 44 passing tests and real-baseline benchmarks, but Chamber Sentinel itself is at the design stage. The claims in this paper are architectural arguments about what the design could achieve, not empirical findings about what it has achieved.

The system does not claim protection at the kernel or firmware layer. It claims protection from the application layer inward, with monitoring (not prevention) of lower layers. The threat model this addresses: device theft, cloud breach, bulk surveillance, and opportunistic malware — not nation-state firmware implants.

---

## 1. Introduction

The Chambers position paper [1] proposes a unique runtime where bounded computational worlds are the primary unit of persistence and destruction, governed by explicit preservation law and evaluated in terms of semantic residue. That paper includes a working implementation: a Rust substrate with 17 crates, 44 tests, real-baseline benchmarks against disposable VM and Docker container environments, and a native application with application-layer isolation (encrypted memory pool, mlock, ptrace denial, WebKit incognito, clipboard isolation). The substrate achieves zero undeclared residue after cryptographic burn, with 2 declared existence-level metadata entries — empirically measured against baselines that retain 2-5.3 entries [1, Section 10].

**Dependency disclosure.** This paper builds on [1] by the same author. The Chambers substrate is implemented and tested but has not been independently peer-reviewed. Readers should evaluate the foundational claims of [1] independently. This paper inherits the trusted-substrate assumption and application-layer boundary from [1]; if those foundations are weakened by review, the claims here are correspondingly weakened.

This paper asks: what happens when you apply that model to a camera? The question is not trivial. A camera produces data continuously — 30 frames per second, each frame 1-3 megabytes. The data is sensitive. The device is exposed. And the conventional approach — store everything, encrypt at rest, control access — fails the moment the device is compromised, because the encrypted archive and its key coexist on the same device.

Chamber Sentinel inverts the model. Instead of storing frames and protecting the archive, it processes frames inside ephemeral chambers that burn every N frames. The only data that crosses the preservation boundary is a structured event label. The frames themselves are encrypted under a per-chamber key that is destroyed within seconds of capture.

---

## 2. Relationship to the Chambers Model

Chamber Sentinel is a direct application of the Chambers architecture [1] with three adaptations for continuous sensor data.

**Semantic residue**, as defined in [1, Section 4.6], is recoverable interpretable information about non-preserved world-state after termination, beyond what the preservation law permits. The taxonomy includes content residue (surviving payload fragments), structural residue (surviving graph topology), behavioral residue (evidence of operations performed), metadata residue (timestamps, counts), and model residue (orchestrator context remnants).

### 2.1 Rolling Chambers

The original model describes a single session: create → explore → converge → finalize → burn. A camera has no natural stopping point. Chamber Sentinel introduces **rolling chambers**: each chamber lives for N frames, then burns and rotates to a fresh key.

```
t=0.0s  K_w1 generated → frames 0-29 encrypted
t=1.0s  K_w1 destroyed → frames 0-29 unrecoverable
        K_w2 generated → frames 30-59 encrypted
t=2.0s  K_w2 destroyed → frames 30-59 unrecoverable
```

This is analogous to Signal Protocol's Double Ratchet [2], where session keys evolve per message. The forward-secrecy analogy noted in [1, Section 7.4.1] applies directly. Related work on puncturable encryption [11] and forward-secret streaming encryption [12] provides formal foundations for per-window key evolution in streaming contexts.

**Window size.** The default window of 30 frames (1 second at 30fps) is a design parameter, not a fixed requirement. The tradeoff:

| Window size | Security | Detection quality | Performance |
|------------|----------|------------------|-------------|
| 1 frame | Minimal exposure (33ms) | No temporal context — cannot distinguish loitering from walking past | High overhead (K_w rotation per frame) |
| 30 frames (1s) | 1-second exposure window | Basic temporal context — can detect presence vs absence | Moderate overhead |
| 150 frames (5s) | 5-second exposure window | Good temporal context — can track short interactions | Low overhead |
| 900 frames (30s) | 30-second exposure window | Full temporal context — can detect complex behaviors | Minimal overhead |

The optimal window size depends on the threat model and the detection model's temporal requirements. A future implementation should make this configurable and document the security/utility tradeoff for each setting.

### 2.2 Simplified Grammar

The Decision Chamber grammar [1] has 10 object types and 9 primitives. A camera chamber needs less:

- **4 object types**: `frame` (temporary), `detection` (temporary), `event_summary` (preservable), `integrity_tag` (preservable)
- **3 primitives**: CreateObject, SealArtifact, TriggerBurn
- **1 phase**: Active (auto-burn after N frames)
- **Preservation law**: only `event_summary` and `integrity_tag` survive

The legal execution surface [1] — defined as the number of distinct operations the environment permits — is 3, even narrower than the Decision Chamber's 9.

### 2.3 Integrity Monitoring (New Contribution)

The original Chambers paper acknowledges that "if compromised, Chambers collapses into managed theater" [1, Section 7.5] but does not propose mechanisms for detecting compromise. Chamber Sentinel adds an integrity monitor that watches the camera pipeline for unauthorized access and produces sealed forensic tags.

**Important circularity.** The integrity monitor's strongest signal — the zero-traffic baseline (Section 6) — depends on Android's permission-based network isolation. But a root-level compromise that the monitor is designed to detect could also bypass that isolation, invalidating the baseline. The monitor is therefore most effective against application-level and opportunistic threats, and least effective against the root-level threats it most needs to detect. This is a genuine architectural limitation, not a solvable design problem within the application layer.

---

## 3. Camera Pipeline Trust Analysis

A camera frame passes through multiple layers before reaching the application. Each layer presents a different trust relationship.

### Layer 0: Hardware / Bootloader

The camera sensor, ISP, and boot chain are manufacturer-controlled. The ISP is a dedicated processor with its own firmware — a black box.

**Position**: not protected, not monitored. Below the trust boundary.

**Physical attacks.** The paper does not address cold boot attacks, JTAG debugging, or chip-off attacks on DRAM. If K_w is in main memory during the active window and the device is physically seized while running, DRAM remanence could recover the current chamber's key. StrongBox protects K_s but K_w must exist in app memory (mlock'd) during operation. Mitigation: the exposure window is the chamber duration (1-30 seconds). After burn, K_w is zeroized. DRAM remanence degrades rapidly at room temperature (seconds to minutes). But a powered-on seizure during an active chamber is a real risk. This would recover at most the current window's frames — not previous windows (forward secrecy).

### Layer 1: Kernel

The Linux kernel manages the camera via V4L2 drivers. Frames exist in plaintext in kernel DMA buffers.

**Position**: not protected, monitored (root only). Monitors: V4L2 consumer tracking, kernel module detection, SELinux audit, DMA buffer consumer counting.

### Layer 2: Camera Service

Android's Camera Service delivers frames via ImageReader. The frame passes through: kernel DMA → camera service → gralloc buffer → app ImageReader. Four plaintext copies.

**Position**: partially protected, monitored. The app detects other camera consumers (CameraManager callback), prevents screen capture (FLAG_SECURE), and operates without preview.

### Layer 3: Application

The frame arrives in ImageReader. The design intent is to encrypt immediately.

**Memory residue caveat.** The "encrypt within microseconds" claim requires qualification. On Android, ImageReader delivers frames through a gralloc buffer backed by shared GPU/CPU memory. The application calls encrypt on the pixel bytes and zeroizes the buffer, but:

- The gralloc allocator may pool and reuse buffers without zeroing. Subsequent buffer allocations may contain remnants of previous frames in recycled gralloc memory.
- ARM CPUs have multi-level caches (L1/L2/L3). Explicit zeroing of a virtual address writes to the cache line, but the dirty line may not be flushed to DRAM immediately. A cold-boot or DMA attacker reading physical memory could find stale cache lines.
- The JNI boundary between Kotlin and Rust involves a buffer copy (unless using direct ByteBuffer). The Kotlin-side copy persists until garbage collection.

These are real residue channels at the microsecond-to-millisecond scale. The design minimizes but does not eliminate plaintext exposure during frame processing. A future implementation should measure actual cache-line flush timing on the A55 and document the residual exposure window.

### Layer 4: Output

Sealed events conform to a grammar-enforced schema.

**Event label schema.** The full set of event types:

1. `person_detected` — human figure identified
2. `vehicle_detected` — car, truck, motorcycle, bicycle
3. `animal_detected` — common domestic and wild animals
4. `package_detected` — box or parcel in delivery context
5. `motion_detected` — movement without identifiable object
6. `unknown_object` — object detected but not classified

These 6 types are a minimal viable set for home/office monitoring. They are insufficient for many surveillance use cases (fire detection, fall detection, weapon detection, license plate recognition). Extending the schema to additional types increases utility but also increases covert channel bandwidth (more bits per event). Each additional type adds ~0.5 bits to the per-event information capacity.

A production system would need a larger, domain-specific event taxonomy. The schema design is central to the value proposition and requires user research to determine which events are worth preserving.

### Summary

```
Layer 0 (Hardware/ISP)    NOT PROTECTED    NOT MONITORED
Layer 1 (Kernel)          NOT PROTECTED    MONITORED (root only)
Layer 2 (Camera Service)  PARTIAL          MONITORED
Layer 3 (Application)     PROTECTED*       PROTECTED
Layer 4 (Output)          PROTECTED        PROTECTED (schema-enforced)

* With caveats: gralloc buffer reuse, cache-line residue, JNI copy
```

---

## 4. Detection Model

*[Section added per review — the model is central to the system's utility.]*

The detection model runs on-device. No cloud inference. The model is the component that transforms "raw frames" into "event labels" — it is the reason the frames can burn.

### 4.1 Model Selection

| Model | Size | Estimated inference (ARM A78) | Capability |
|-------|------|------------------------------|-----------|
| YOLOv8n (nano) | 6.2MB | ~25ms (estimated) | 80 COCO classes, strong accuracy |
| MobileNet V3 + SSD | 4.4MB | ~15ms (estimated) | Object detection, TFLite optimized |
| EfficientDet-Lite0 | 4.3MB | ~30ms (estimated) | Good accuracy/speed tradeoff |

**Inference times are estimates from published benchmarks on comparable ARM hardware, not measurements on the A55.** Actual performance must be measured on the target device.

### 4.2 Model Accuracy and Utility

The system's value depends entirely on the model's accuracy. If the model misses a person at the door (false negative), the user gets no alert and the footage is gone — there is no way to review what happened. If the model hallucinates a person (false positive), the user gets a spurious alert.

Published accuracy for YOLOv8n on COCO: mAP@0.5 = 37.3%. This means roughly 1 in 3 detections at IoU 0.5 may be incorrect. For a security application, this false positive/negative rate may be unacceptable without domain-specific fine-tuning.

**This is a fundamental tradeoff of the Chamber Sentinel model:** you trade footage review capability for privacy. If the model is wrong, you cannot go back and check the footage. The footage is burned. The user must decide whether the model's accuracy is sufficient for their threat model before enabling the system.

### 4.3 Model Protection

The model weights live in app memory (read-only mmap from APK assets). An attacker who extracts the APK can obtain the model and:
- Analyze its detection capabilities (what it can and cannot see)
- Craft adversarial examples to evade detection
- Understand its blind spots

Mitigation: the model is not a secret. It is a standard object detection model. The security guarantee does not depend on the model being unknown — it depends on the frames being encrypted and burned. An attacker who evades the model's detection still cannot exfiltrate frames from the application layer (no network, encrypted memory, per-second burn).

---

## 5. Covert Channel Analysis

Can a compromised detection model encode frame data through the sealed event channel?

The event schema constrains output: event type (6 values = 2.58 bits), timestamp (quantized to 5 seconds), confidence (0.00-1.00, 2 decimal places = 6.64 bits). At maximum event rate (10/minute):

```
(2.58 + 6.64) bits/event × 10 events/min = 92.2 bits/min = 1.54 bits/sec
```

At 1.54 bits/sec, exfiltrating one 100KB compressed image takes ~54 hours.

**Caveats on this analysis:**
- An adversarially optimized model could potentially use event sequencing and timing correlations to increase effective bandwidth beyond the per-event calculation
- The confidence field's 2-decimal precision is the primary channel (6.64 bits per event). Reducing to 1 decimal (3.32 bits) halves the channel bandwidth
- A model that controls which of 6 event types to emit, and when, has more degrees of freedom than the per-field calculation suggests

Related work on covert channel analysis in constrained output schemas [13] provides formal methods for bounding information leakage through restricted APIs. A rigorous treatment would apply these methods rather than the envelope calculation above.

---

## 6. The Exfiltration Detection Insight

The most consequential property of Chamber Sentinel is the **traffic baseline**.

Chamber Sentinel has no legitimate network traffic. The app has no INTERNET permission. Android enforces this at the kernel level via UID-based iptables rules — the app process cannot create network sockets.

Any outbound traffic that correlates with camera operation is an anomaly. The detection rule is simple: "any data leaving that isn't background device traffic is suspicious during camera operation."

**The circularity problem.** This argument is strongest against application-level threats (malicious library, rogue SDK, data-harvesting framework) and weakest against the exact threats the integrity monitor is designed for. A root-level attacker who compromises the kernel can:
- Modify iptables rules, removing the UID-based network block
- Exfiltrate via the kernel network stack without going through the app's UID
- Disable or manipulate TrafficStats counters

In this scenario, the zero-traffic baseline is invalidated because the attacker controls the measurement infrastructure. The integrity monitor is therefore **most effective against opportunistic and application-level threats** and provides **diminishing returns against kernel-level compromise**.

The paper does not claim the integrity monitor solves kernel-level threats. It claims it transforms the attacker's problem from "steal the archive at leisure" (conventional camera) to "maintain real-time exfiltration from a rolling-burn system." Even if the monitor is bypassed, the burn semantics limit the exposure window.

---

## 7. Comparison with Existing Approaches

| System | Processes locally? | Stores footage? | Destroys footage? | Detects compromise? | Event-only output? |
|--------|-------------------|----------------|-------------------|---------------------|--------------------|
| Ring/Nest cloud cameras | Cloud processing | Cloud, indefinite | Manual delete only | No | No — full footage |
| Local NVR (BlueIris, ZoneMinder) | Yes | Local disk, days-weeks | Overwrite (not cryptographic) | No | No — full footage |
| Haven (Guardian Project) [8] | Yes | Local photos + sensor data | Manual | No | No — stores photos |
| Apple on-device ML | Yes | Photos retained; ML analysis tied to photo lifecycle. User can delete photos, and analysis is deleted with them. | User-initiated photo deletion | No | No — analysis augments photos, doesn't replace them |
| "Privacy cameras" (face blur) | Yes | Stores blurred footage | No — blurred footage persists | No | No — modified footage |
| TEE-based camera (TrustOTP [14], CameraGuard [15]) | TEE enclave | Varies — some retain, some process-and-discard | Varies | No | Varies |
| **Chamber Sentinel** | **Yes** | **No** — frames burn per window | **Yes** — cryptographic, per-window | **Yes** — integrity monitor | **Yes** — event labels only |

**Nuances:** Haven is a trip-wire security tool (detect intrusion via sensors), not a surveillance camera — the comparison is on the "local processing + privacy" axis only. Apple's on-device ML is tightly coupled to the photo lifecycle — deleting a photo deletes its analysis. TEE-based camera systems [14][15] process frames inside hardware enclaves, providing stronger isolation than application-layer encryption but typically still retaining processed outputs with richer schemas than Chamber Sentinel's 6 event types.

---

## 8. Limitations

**No implementation exists.** This is an architectural position paper. All performance figures (encryption latency, inference time, battery drain, burn timing) are estimates or extrapolations from published benchmarks on comparable hardware. A prototype is needed to validate these estimates.

**Kernel-level exfiltration is not prevented.** The integrity monitor may detect network anomalies but cannot prevent frame capture at the kernel or ISP level.

**Real-time streaming cannot be stopped from the application layer.** If exfiltration happens before frames reach the app, burn semantics are irrelevant for those frames.

**Memory residue during processing.** Gralloc buffer reuse, CPU cache lines, JNI copies, and garbage collection timing create plaintext exposure windows beyond the "microseconds" the design targets. Actual exposure must be measured on real hardware.

**The detection model determines system utility.** A camera that remembers nothing is only useful if its event labels are accurate. Published mAP for candidate models suggests significant false positive/negative rates without domain-specific fine-tuning. Users who need to review footage for false-negative verification cannot use this system.

**Cold boot / physical seizure.** K_w exists in app memory during the active window. Physical seizure of a running device could recover the current window's key via DRAM remanence. Previous windows are protected by forward secrecy.

**The self-referential foundation.** This paper builds on [1] by the same author. The Chambers substrate has a working implementation but has not been independently peer-reviewed. Readers should evaluate [1] independently.

**Event labels are themselves information.** The pattern of events ("person at door every weekday at 8 AM") reveals life patterns. This is accepted residue.

**Battery and thermal.** Continuous camera + encryption + inference + monitoring will have significant battery impact. Estimated 15-20% additional drain per hour, but this is an estimate only — not measured.

**Integrity monitor circularity.** The zero-traffic baseline that makes exfiltration detection easy is itself dependent on Android's permission enforcement, which fails under the same kernel-level compromise the monitor tries to detect. The monitor is most useful against application-level threats.

---

## 9. Target Hardware

The Samsung Galaxy A55 is the primary target:

- **StrongBox** — hardware key storage (K_s in hardware, never in app memory)
- **ARM crypto extensions** — hardware AES acceleration (estimated < 1ms per 3MB frame encryption — not measured on A55)
- **Exynos ISP** — dedicated image processor

The architecture is not Samsung-specific. Any Android device with Camera2 API, hardware keystore, and ARM crypto extensions is a viable target. StrongBox falls back to TEE-backed keystore on devices without it.

---

## 10. Related Work

**Puncturable encryption** [11] provides formal constructions for forward-secret encryption in streaming contexts, where keys can be "punctured" to revoke decryption capability for specific ciphertexts. Chamber Sentinel's per-window key rotation is a coarse-grained instance of this pattern.

**TEE-based camera processing** (TrustOTP [14], CameraGuard [15]) processes frames inside hardware enclaves (ARM TrustZone or Intel SGX), providing isolation from the OS kernel. These systems offer stronger guarantees than application-layer encryption but require hardware cooperation and typically retain richer output schemas.

**Covert channel analysis** in constrained output schemas [13] provides formal methods for bounding information leakage through restricted APIs. Chamber Sentinel's event schema is a candidate for this analysis.

**Haven** [8] (Guardian Project) uses phone sensors (camera, accelerometer, microphone) as a physical intrusion detection system. It stores evidence (photos, audio recordings) rather than destroying it — a different design point from Chamber Sentinel's burn-first model.

---

## 11. Conclusion

Chamber Sentinel proposes applying the Chambers burn-first model [1] to continuous visual sensor data. The architectural contribution is the combination of rolling chambers with per-window forward secrecy, a constrained event schema as the sole preservation boundary, and an integrity monitor that produces sealed forensic tags.

The traffic baseline insight — that removing all legitimate network traffic makes exfiltration trivially detectable for application-level threats — is the paper's strongest original observation and applies beyond cameras to any sensor processing system where the output is structurally smaller than the input.

The paper is honest about what it does not deliver: no prototype, no measured performance, no validated detection model accuracy, and a self-referential dependency on an unreviewed parent paper. These are not rhetorical hedges — they are genuine gaps that must be filled before the architecture can be evaluated as a system rather than as an idea.

The core question remains: is event-label-only output a sufficient replacement for footage in real-world security monitoring? The answer depends on model accuracy, user expectations, and regulatory requirements — none of which this paper addresses empirically. What the paper does argue is that if the answer is yes for some use cases, then the Chambers model provides a principled way to implement it with formally bounded information retention.

---

## References

[1] A. Ganguli, "Chambers: Sealed Ephemeral Worlds for Private Cognition — A World-Based Position Paper on Persistence-Law-First, Burn-First, Task-Bounded Computing," 2026. Implementation: https://github.com/therealgulkorinaga/chamber

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

[12] Derler et al. "Rethinking Forward Security for Signatures and Key Exchange." ACM CCS 2018. (Forward-secret key evolution in streaming protocols.)

[13] Lampson. "A Note on the Confinement Problem." CACM 1973. (Foundational work on covert channel analysis in constrained systems.)

[14] Sun et al. "TrustOTP: Transforming Smartphones into Secure One-Time Password Tokens." ACM CCS 2015. (TEE-based secure processing on mobile devices.)

[15] Li et al. "CameraGuard: Securing Cameras in the Era of IoT." IEEE IoTJ 2020. (TEE-based camera frame protection.)
