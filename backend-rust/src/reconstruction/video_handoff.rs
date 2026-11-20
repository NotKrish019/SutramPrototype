//! Live video frame handoff (Member 4, Phase 3E.3).
//!
//! Connects CCTV evidence to reconstruction:
//! raw bytes (integrity-checked against the ingestion receipt) → `extract_keyframes` →
//! frame artifacts (persisted once per source SHA-256, never overwritten) → optional UTC
//! anchoring from declared calibration → Member 1's real consumer
//! `extraction::consume_video_frames` → event candidates with provenance.
//!
//! Single correction (DEC-001/DEC-007 as proposed): anchoring only localizes the declared
//! `recording_start_timestamp` to a UTC instant. It does **not** apply the clock offset. The
//! declared offset is carried on each frame candidate, and the temporal engine applies it exactly
//! once. Frames are anchored only when the recording start is declared and its timezone is known
//! (an explicit offset in the value, or a declared `source_timezone`). Otherwise the frame keeps
//! a relative PTS, and the timeline excludes it with a reason instead of guessing a time.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::model::ExtractedFrameArtifact;
use super::temporal::parse_and_normalize_timestamp;
use super::video::{compute_file_sha256, extract_keyframes};
use crate::error::AppError;
use crate::extraction::model::ExtractedEventCandidate;
use crate::extraction::service::consume_video_frames;
use crate::ingestion::{EvidenceManifest, StorageManager};

/// Source types treated as video evidence.
pub const VIDEO_SOURCE_TYPES: &[&str] = &["CCTV_VIDEO", "CCTV", "VIDEO"];
pub const DEFAULT_MAX_FRAMES: usize = 50;
pub const DEFAULT_FRAME_TIMEOUT: Duration = Duration::from_secs(120);
const FRAMES_MANIFEST: &str = "frames.json";
const EXTRACTION_METHOD: &str = "ffmpeg_iframe_select_showinfo_pts";

/// Outcome of handing one video exhibit's frames to the reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VideoHandoffStatus {
    /// Frames were anchored to UTC and handed to the consumer.
    HandedOffAnchored,
    /// Frames were handed to the consumer with relative PTS only (no declared calibration);
    /// the timeline excludes them rather than inventing a time.
    HandedOffUnanchored,
    /// Stored bytes do not match the ingestion receipt; nothing was extracted.
    RejectedIntegrityMismatch,
    /// Frame extraction failed (FFmpeg error, timeout, no keyframes, PTS mismatch).
    ExtractionFailed,
}

/// Per-exhibit handoff record, carried in the reconstruction result and the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoHandoffRecord {
    pub evidence_id: String,
    pub source_video_sha256: String,
    pub status: VideoHandoffStatus,
    pub frames_extracted: usize,
    pub candidates_handed_off: usize,
    /// True when an existing frame set for this exact source hash was reused (no re-extraction).
    pub reused_existing_frames: bool,
    /// Derived-storage relative directory of the frame set (`{case}/{evidence}/frames-…`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_basis: Option<String>,
    pub diagnostics: Vec<String>,
}

/// Persisted once per source video hash; makes repeated runs reuse identical frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrameSetManifest {
    source_video_sha256: String,
    extraction_method: String,
    max_frames: usize,
    frames: Vec<ExtractedFrameArtifact>,
}

struct Calibration {
    anchor_start_utc: Option<DateTime<Utc>>,
    anchor_basis: Option<String>,
    source_timezone: Option<String>,
    clock_offset_seconds: Option<f64>,
    device_coordinates: Option<serde_json::Value>,
    diagnostics: Vec<String>,
}

fn declared_calibration(metadata: &serde_json::Value) -> Calibration {
    let source_timezone = metadata
        .get("source_timezone")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let clock_offset_seconds = metadata
        .get("clock_offset_seconds")
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite());
    let device_coordinates = metadata
        .get("device_coordinates")
        .filter(|v| v.get("lat").is_some() && v.get("lon").is_some())
        .cloned();
    let mut diagnostics = Vec::new();

    let start = metadata
        .get("recording_start_timestamp")
        .and_then(|v| v.as_str());
    let (anchor_start_utc, anchor_basis) = match start {
        None => {
            diagnostics.push(
                "No recording_start_timestamp declared; frames keep relative PTS only.".to_string(),
            );
            (None, None)
        }
        Some(raw) => {
            let has_explicit_offset = raw.ends_with('Z')
                || raw
                    .rfind(['+', '-'])
                    .map(|i| i > 10 && raw[i..].contains(':'))
                    .unwrap_or(false);
            let tz = if has_explicit_offset {
                Some("UTC".to_string())
            } else {
                source_timezone.clone()
            };
            match tz {
                None => {
                    diagnostics.push(format!(
                        "recording_start_timestamp '{raw}' has no UTC offset and no \
                         source_timezone is declared; frames are not anchored."
                    ));
                    (None, None)
                }
                Some(tz) => match parse_and_normalize_timestamp(raw, &tz, 0.0) {
                    Ok(start_utc) => (
                        Some(start_utc),
                        Some(format!(
                            "recording_start_timestamp '{raw}' localized with {} (clock offset \
                             not applied here; applied once by the temporal engine)",
                            if has_explicit_offset {
                                "its explicit UTC offset".to_string()
                            } else {
                                format!("declared source_timezone '{tz}'")
                            }
                        )),
                    ),
                    Err(e) => {
                        diagnostics.push(format!(
                            "recording_start_timestamp '{raw}' could not be anchored: {e}"
                        ));
                        (None, None)
                    }
                },
            }
        }
    };

    Calibration {
        anchor_start_utc,
        anchor_basis,
        source_timezone,
        clock_offset_seconds,
        device_coordinates,
        diagnostics,
    }
}

fn is_video(manifest: &EvidenceManifest) -> bool {
    let t = manifest.receipt.source_type.trim().to_ascii_uppercase();
    VIDEO_SOURCE_TYPES.contains(&t.as_str()) || manifest.receipt.mime_type.starts_with("video/")
}

async fn load_or_extract_frames(
    raw_path: &Path,
    frames_dir: &Path,
    source_sha256: &str,
    max_frames: usize,
    timeout_duration: Duration,
) -> Result<(Vec<ExtractedFrameArtifact>, bool), AppError> {
    let manifest_path = frames_dir.join(FRAMES_MANIFEST);
    if let Ok(bytes) = fs::read(&manifest_path).await {
        let existing: FrameSetManifest = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Corrupt frame-set manifest: {}", e))
        })?;
        if existing.source_video_sha256 != source_sha256 {
            return Err(AppError::Internal(anyhow::anyhow!(
                "Frame-set manifest at '{}' belongs to a different source hash.",
                frames_dir.display()
            )));
        }
        // Re-verify every persisted frame before reuse.
        for f in &existing.frames {
            let p = frames_dir.join(&f.relative_path);
            if compute_file_sha256(&p)? != f.sha256_hash {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "Persisted frame '{}' no longer matches its recorded SHA-256.",
                    f.relative_path
                )));
            }
        }
        return Ok((existing.frames, true));
    }
    if frames_dir.exists() {
        return Err(AppError::BadRequest(format!(
            "Frame directory '{}' exists without a frame-set manifest; refusing to overwrite or \
             merge partial output.",
            frames_dir.display()
        )));
    }

    // Extract into a fresh staging directory, then publish atomically by rename.
    let staging = frames_dir.with_extension(format!("staging-{}", uuid::Uuid::new_v4()));
    let frames = extract_keyframes(raw_path, &staging, max_frames, timeout_duration).await?;
    let manifest = FrameSetManifest {
        source_video_sha256: source_sha256.to_string(),
        extraction_method: EXTRACTION_METHOD.to_string(),
        max_frames,
        frames: frames.clone(),
    };
    let encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Frame manifest encode: {}", e)))?;
    fs::write(staging.join(FRAMES_MANIFEST), encoded)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Frame manifest write: {}", e)))?;
    if let Err(rename_err) = fs::rename(&staging, frames_dir).await {
        // Another run may have published the same frame set first. Reuse it only if its manifest
        // is present and matches this source; never overwrite it. The staging copy is left in
        // place for inspection.
        let published = fs::read(frames_dir.join(FRAMES_MANIFEST))
            .await
            .map_err(|_| {
                AppError::Internal(anyhow::anyhow!(
                    "Could not publish frame set to '{}': {}",
                    frames_dir.display(),
                    rename_err
                ))
            })?;
        let existing: FrameSetManifest = serde_json::from_slice(&published).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Corrupt frame-set manifest: {}", e))
        })?;
        if existing.source_video_sha256 != source_sha256 {
            return Err(AppError::Internal(anyhow::anyhow!(
                "Published frame set belongs to a different source hash."
            )));
        }
        return Ok((existing.frames, true));
    }
    Ok((frames, false))
}

/// Extracts, anchors and hands off the frames of every video exhibit in a case.
///
/// Returns the frame event candidates produced by Member 1's consumer (with Member 4 provenance
/// added to their metadata) and one record per video exhibit. Non-video exhibits are ignored.
pub async fn handoff_case_video_frames(
    storage: &StorageManager,
    case_id: &str,
    manifests: &[EvidenceManifest],
    max_frames: usize,
    timeout_duration: Duration,
) -> (Vec<ExtractedEventCandidate>, Vec<VideoHandoffRecord>) {
    let mut candidates = Vec::new();
    let mut records = Vec::new();

    let mut videos: Vec<&EvidenceManifest> = manifests.iter().filter(|m| is_video(m)).collect();
    videos.sort_by(|a, b| a.receipt.evidence_id.cmp(&b.receipt.evidence_id));

    for manifest in videos {
        let receipt = &manifest.receipt;
        let evidence_id = receipt.evidence_id.clone();
        let mut record = VideoHandoffRecord {
            evidence_id: evidence_id.clone(),
            source_video_sha256: receipt.sha256_hash.clone(),
            status: VideoHandoffStatus::ExtractionFailed,
            frames_extracted: 0,
            candidates_handed_off: 0,
            reused_existing_frames: false,
            frames_reference: None,
            anchor_basis: None,
            diagnostics: Vec::new(),
        };

        // 1. Integrity of the stored bytes against the ingestion receipt.
        let raw_path = match storage.get_raw_file_path(case_id, &evidence_id) {
            Ok(p) => p,
            Err(e) => {
                record
                    .diagnostics
                    .push(format!("Raw evidence path invalid: {e}"));
                records.push(record);
                continue;
            }
        };
        match compute_file_sha256(&raw_path) {
            Ok(h) if h == receipt.sha256_hash => {}
            Ok(h) => {
                record.status = VideoHandoffStatus::RejectedIntegrityMismatch;
                record.diagnostics.push(format!(
                    "Stored bytes hash {h} does not match receipt {}; frames not extracted.",
                    receipt.sha256_hash
                ));
                records.push(record);
                continue;
            }
            Err(e) => {
                record.status = VideoHandoffStatus::RejectedIntegrityMismatch;
                record
                    .diagnostics
                    .push(format!("Stored evidence bytes unreadable: {e}"));
                records.push(record);
                continue;
            }
        }

        // 2–4. Frames: reuse the persisted set for this exact hash, or extract once.
        let short = &receipt.sha256_hash[..receipt.sha256_hash.len().min(16)];
        let frames_dir_name = format!("frames-{short}");
        let frames_dir: PathBuf = match storage.get_derived_evidence_dir(case_id, &evidence_id) {
            Ok(d) => d.join(&frames_dir_name),
            Err(e) => {
                record
                    .diagnostics
                    .push(format!("Derived path invalid: {e}"));
                records.push(record);
                continue;
            }
        };
        if let Some(parent) = frames_dir.parent() {
            let _ = fs::create_dir_all(parent).await;
        }
        let (mut frames, reused) = match load_or_extract_frames(
            &raw_path,
            &frames_dir,
            &receipt.sha256_hash,
            max_frames,
            timeout_duration,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                record
                    .diagnostics
                    .push(format!("Frame extraction failed: {e}"));
                records.push(record);
                continue;
            }
        };
        let frames_reference = format!("{case_id}/{evidence_id}/{frames_dir_name}");
        for f in &mut frames {
            f.relative_path = format!("{frames_reference}/{}", f.relative_path);
        }
        record.frames_extracted = frames.len();
        record.reused_existing_frames = reused;
        record.frames_reference = Some(frames_reference.clone());

        // 5. Anchoring from declared calibration only.
        let calibration = declared_calibration(&receipt.metadata);
        record
            .diagnostics
            .extend(calibration.diagnostics.iter().cloned());
        if let Some(start) = calibration.anchor_start_utc {
            for f in &mut frames {
                f.estimated_utc = Some(
                    start + chrono::Duration::milliseconds((f.pts_seconds * 1000.0).round() as i64),
                );
            }
            record.anchor_basis = calibration.anchor_basis.clone();
        }

        // 6. Handoff to Member 1's real consumer.
        let mut produced = consume_video_frames(&evidence_id, &frames, None);

        // Member 4 provenance; the declared offset rides on the event so it is applied once.
        for (cand, frame) in produced.iter_mut().zip(frames.iter()) {
            if let Some(meta) = cand.metadata.as_object_mut() {
                meta.insert(
                    "source_video_sha256".into(),
                    receipt.sha256_hash.clone().into(),
                );
                meta.insert(
                    "frame_artifact_reference".into(),
                    format!(
                        "urn:yukti:artifact:{}:frame:{}",
                        frames_reference, frame.frame_index
                    )
                    .into(),
                );
                meta.insert("frames_reference".into(), frames_reference.clone().into());
                if let Some(basis) = &calibration.anchor_basis {
                    meta.insert("utc_anchor_basis".into(), basis.clone().into());
                }
                if calibration.anchor_start_utc.is_some() {
                    // The anchored timestamp is already a UTC instant, so it is parsed as UTC.
                    // The declared device timezone is kept for provenance.
                    meta.insert("source_timezone".into(), "UTC".into());
                    if let Some(tz) = &calibration.source_timezone {
                        meta.insert("declared_device_timezone".into(), tz.clone().into());
                    }
                } else if let Some(tz) = &calibration.source_timezone {
                    meta.insert("source_timezone".into(), tz.clone().into());
                }
                if let Some(off) = calibration.clock_offset_seconds {
                    meta.insert("clock_offset_seconds".into(), off.into());
                }
                if let Some(coords) = &calibration.device_coordinates {
                    meta.insert("coordinates".into(), coords.clone());
                }
            }
        }

        record.candidates_handed_off = produced.len();
        record.status = if calibration.anchor_start_utc.is_some() {
            VideoHandoffStatus::HandedOffAnchored
        } else {
            VideoHandoffStatus::HandedOffUnanchored
        };
        candidates.extend(produced);
        records.push(record);
    }

    (candidates, records)
}
