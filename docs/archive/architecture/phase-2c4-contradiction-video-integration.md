# Phase 2C.4: Contradiction Engine and Video Integration Architecture

**Project:** YUKTI (formerly SUTRAM 2026)  
**Author:** Lead Digital Forensics Systems Engineer  
**Date:** 2026-11-19T22:49:30+05:30  
**Phase:** Phase 2C — End-to-End Forensic Reconstruction Integration  

---

## 1. Contradiction Detection Engine (`contradictions.rs`)

### 1.1 Existential Feasibility Over Uncertainty Windows
A contradiction lead is generated **only** when no physically valid realization exists within the half-open uncertainty bounds of the observed records:
- **Condition for Feasibility:** An event $A$ at $(x_A, y_A)$ with duration $D_A$ and uncertainty $[S_A, E_A)$ can precede event $B$ at $(x_B, y_B)$ with uncertainty $[S_B, E_B)$ within speed threshold $v_{\text{threshold}}$ if and only if:
  $$\Delta t_{\text{post max}} = E_B - (S_A + D_A) > 0$$
  $$\frac{d_{\text{Haversine}}(A, B) \cdot 3600.0}{\Delta t_{\text{post max}}} \le v_{\text{threshold}}$$
- If $\Delta t_{\text{post max}} \le 0$ or the minimum required speed exceeds the threshold, the sequence is physically irreconcilable under the recorded metadata.
- **Overlapping Windows Are Not Contradictions:** If two uncertainty windows overlap, but a valid non-contradictory assignment exists, no lead is flagged. Overlapping events are classified merely as `CONCURRENT_UNCERTAIN`.

### 1.2 Neutral, Objective Language & Limitation Caveats (DEC-004, DEC-006)
In strict accordance with judicial forensic standards:
- Contradiction leads do not assert guilt, lying, or conclusive identity.
- Statistical confidence scores (`weight`) are omitted (`None`).
- Every lead includes explicit limitation caveats explaining potential confounding factors:
  1. Uncalibrated device clock drift.
  2. Coarse cellular tower centroids rather than exact GPS pinpoints.
  3. Shared or transferred physical devices between individuals.
  4. Legitimate high-speed transport modes (train, highway, aviation).

---

## 2. Technical Video Forensics Integration (`video.rs`)

### 2.1 Cryptographic Immutability and Stream Probing
Video files are inspected via subprocess invocations of `ffprobe` with strict execution timeouts:
- Extracts container format, duration, bit rate, video stream resolution (width $\times$ height), frame rate (`r_frame_rate`, `avg_frame_rate`), and total frame count.
- Computes SHA-256 digest of probe telemetry to guarantee integrity.

### 2.2 Keyframe Extraction and Frame Hashing
Keyframes are extracted using `ffmpeg` (`-vf select='eq(pict_type\,I)'` or interval sampling):
- Frames are saved as uncompressed PNG images into derived exhibit storage (`data/derived/frames/`).
- Every extracted frame is hashed with SHA-256 immediately upon generation (`frame_sha256`).
- Frame PTS timestamp (Presentation Time Stamp in seconds) is recorded and mapped to nominal UTC time using exhibit clock offset and source timezone.
