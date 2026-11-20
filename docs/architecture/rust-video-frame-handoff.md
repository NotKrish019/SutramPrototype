# Rust live video frame handoff (Member 4, Phase 3E.3)

**Status:** implemented (uncommitted). **Owner:** Member 4. **Date:** 2026-11-20
**Consumer:** Member 1's `extraction::consume_video_frames(evidence_id, frames, base_timestamp_utc)` (23a915d), used unchanged.

## Live path

| # | Stage | Function / module | Output |
| :--- | :--- | :--- | :--- |
| 1 | Ingestion | `POST /api/v1/evidence/upload` → `ingestion::validation` (MP4 `ftyp` → `CCTV_VIDEO`) | raw bytes `{raw}/{case}/{ev}/{ev}.raw`, receipt SHA-256 |
| 2 | Integrity | `video_handoff::handoff_case_video_frames` → `video::compute_file_sha256` | bytes must equal the receipt hash, else `REJECTED_INTEGRITY_MISMATCH` |
| 3–4 | Frames | `video::extract_keyframes` (I-frames, FFmpeg `showinfo` PTS, `-n`, empty output dir only) | `Vec<ExtractedFrameArtifact>` (real `pts_seconds`, per-frame SHA-256) |
| 4b | Persist | `frames-{sha16}/` + `frames.json`, staged then renamed; reused on later runs after re-hashing | one frame set per source hash, never overwritten |
| 5 | Anchor | declared `recording_start_timestamp` localized with its explicit offset or the declared `source_timezone`; **no clock offset applied here** | `estimated_utc`, or none (relative PTS) |
| 6 | Handoff | `extraction::consume_video_frames` (Member 1) | `ExtractedEventCandidate` per frame (`CCTV_KEYFRAME`) |
| 6b | Provenance | Member 4 adds `source_video_sha256`, `frame_artifact_reference` (`urn:yukti:artifact:{case}/{ev}/frames-…:frame:{n}`), `frames_reference`, `utc_anchor_basis`, declared `clock_offset_seconds`, `declared_device_timezone`, declared `device_coordinates` | candidate metadata |
| 7–8 | Reconstruction | `api/routes/reconstruction.rs::load_case_events_with_video` → temporal (offset applied **once**), spatial, contradictions | timeline entries; unanchored frames → `excluded_events` with reason |
| 9 | Report | `report::generate_case_report` → `timeline_events` (frame SHA-256, PTS, artifact reference, source hash), `excluded_events`, `video_handoff`, and dossier sections "Reconstructed Timeline & Provenance" / "Video frame handoff" | judicial report |

`FullCaseReconstructionResult.video_handoff` holds one record per video exhibit (status, frames
extracted and handed off, reuse, diagnostics). A failed or integrity-rejected handoff blocks
judicial export.

## Defects fixed in `video::extract_keyframes`

- `pts_seconds` was the frame index (`idx as f64`). It is now FFmpeg's reported `pts_time`. If
  the PTS count does not match the frames written, extraction fails.
- `relative_path` held the absolute host path. It is now relative (the file name, then prefixed
  with the derived-storage reference).
- Pre-existing JPEGs in the output directory were swept into the result, and FFmpeg could prompt
  or overwrite. The directory must now be empty, and FFmpeg runs with `-nostdin -n`.

## Limits

- Database mode: when events come from PostgreSQL, the video handoff is not run (filesystem mode
  only).
- Unanchored frames are excluded, not shown with a time. Declaring `recording_start_timestamp`
  (with an offset or a `source_timezone`) anchors them.
- The frame precision is Member 1's declaration (`EXACT_SECOND`).

## Tests

`backend-rust/tests/video_handoff_live.rs`: real FFmpeg (required; a missing FFmpeg fails the
tests), real upload route, real consumer. It covers calibrated frames → reconstruction → report
with provenance; repeated runs reusing one frame set (no duplicates or overwrites); identical
frames and times across fresh stores; uncalibrated frames excluded with reasons; and tampered
bytes rejected before extraction. It also unit-tests the `showinfo` PTS parser.
