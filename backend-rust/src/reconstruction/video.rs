//! Forensic Video Evidence Processing Engine (Member 4).
//!
//! Provides secure, read-only extraction of technical video metadata and keyframes
//! using FFprobe and FFmpeg subprocess CLI (DEC-005).
//!
//! Key forensic guarantees:
//! 1. Immutability of Original Evidence: Source files are read strictly in binary read-only
//!    mode. SHA-256 is verified both before and after processing to guarantee immutability.
//! 2. Safe Subprocess Execution: Subprocess calls use explicit argument vectors (never shell
//!    interpolation) and strict timeout enforcement.
//! 3. Cryptographic Provenance: Every extracted frame artifact is hashed with SHA-256 and
//!    bound to the parent evidence exhibit SHA-256.
//! 4. Non-Destructive Storage: Derived frames are stored strictly under derived storage roots.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

use super::model::{ExtractedFrameArtifact, VideoMetadata, VideoStreamMetadata};
use crate::error::AppError;

/// Computes streaming SHA-256 of a target file.
pub fn compute_file_sha256(path: &Path) -> Result<String, AppError> {
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "Evidence file not found: '{}'",
            path.display()
        )));
    }

    let mut file = File::open(path).map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Failed to open file for hashing: {}", e))
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let count = file.read(&mut buffer).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Error reading file for hashing: {}", e))
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Technical video probe using FFprobe with argument vectors and timeout.
pub async fn probe_video_metadata(
    video_path: &Path,
    timeout_duration: Duration,
) -> Result<VideoMetadata, AppError> {
    if !video_path.exists() {
        return Err(AppError::NotFound(format!(
            "Video file not found: '{}'",
            video_path.display()
        )));
    }

    let pre_hash = compute_file_sha256(video_path)?;

    let mut cmd = Command::new("ffprobe");
    cmd.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(video_path)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "FFprobe is not installed or not found on system PATH: {}",
            e
        ))
    })?;

    let output = match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(res) => res.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("FFprobe process execution error: {}", e))
        })?,
        Err(_) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "FFprobe execution timed out after {} seconds.",
                timeout_duration.as_secs()
            )));
        }
    };

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::BadRequest(format!(
            "FFprobe failed to inspect media container: {}",
            err_msg.trim()
        )));
    }

    let parsed_json: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to parse FFprobe JSON output: {}",
            e
        ))
    })?;

    // Parse streams
    let mut streams = Vec::new();
    if let Some(stream_array) = parsed_json.get("streams").and_then(|v| v.as_array()) {
        for (idx, stream) in stream_array.iter().enumerate() {
            let codec_type = stream
                .get("codec_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if codec_type != "video" {
                continue;
            }

            let codec_name = stream
                .get("codec_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let codec_long_name = stream
                .get("codec_long_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let width = stream
                .get("width")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            let height = stream
                .get("height")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            let r_frame_rate = stream
                .get("r_frame_rate")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let avg_frame_rate = stream
                .get("avg_frame_rate")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let duration_seconds = stream
                .get("duration")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok());
            let nb_frames = stream
                .get("nb_frames")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok());

            streams.push(VideoStreamMetadata {
                index: idx,
                codec_name,
                codec_long_name,
                width,
                height,
                r_frame_rate,
                avg_frame_rate,
                duration_seconds,
                nb_frames,
            });
        }
    }

    if streams.is_empty() {
        return Err(AppError::BadRequest(
            "Media container contains no valid video streams.".to_string(),
        ));
    }

    let format_obj = parsed_json.get("format").ok_or_else(|| {
        AppError::BadRequest("FFprobe output missing format descriptor.".to_string())
    })?;

    let format_name = format_obj
        .get("format_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let duration_seconds = format_obj
        .get("duration")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let size_bytes = format_obj
        .get("size")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let bit_rate = format_obj
        .get("bit_rate")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok());

    // Post-operation immutability check
    let post_hash = compute_file_sha256(video_path)?;
    if pre_hash != post_hash {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Forensic integrity violation: source video was modified during FFprobe execution."
        )));
    }

    Ok(VideoMetadata {
        format_name,
        duration_seconds,
        size_bytes,
        bit_rate,
        streams,
        probe_sha256: pre_hash,
    })
}

/// Deterministic keyframe extraction with resource ceilings and SHA-256 verification (DEC-005).
///
/// - `output_dir` must be absent or empty. Derived artifacts are never overwritten or mixed with
///   earlier output, and FFmpeg runs with `-n` (never overwrite).
/// - `pts_seconds` is the presentation timestamp FFmpeg reports for each emitted frame (showinfo
///   `pts_time`). If the reported PTS cannot be matched one-to-one with the written frames, the
///   extraction fails instead of guessing.
/// - `relative_path` is the frame file name relative to `output_dir` (never a host path).
/// - `estimated_utc` is left unset. UTC anchoring needs evidence calibration and is done by the
///   caller (see `video_handoff`).
pub async fn extract_keyframes(
    video_path: &Path,
    output_dir: &Path,
    max_frames: usize,
    timeout_duration: Duration,
) -> Result<Vec<ExtractedFrameArtifact>, AppError> {
    if !video_path.exists() {
        return Err(AppError::NotFound(format!(
            "Video file not found: '{}'",
            video_path.display()
        )));
    }
    if max_frames == 0 {
        return Err(AppError::BadRequest(
            "max_frames must be at least 1.".to_string(),
        ));
    }

    let pre_hash = compute_file_sha256(video_path)?;

    if output_dir.exists() {
        let mut existing = fs::read_dir(output_dir).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to inspect frame output directory: {}",
                e
            ))
        })?;
        if let Ok(Some(_)) = existing.next_entry().await {
            return Err(AppError::BadRequest(format!(
                "Frame output directory '{}' is not empty; derived frame artifacts are never \
                 overwritten or merged with earlier output.",
                output_dir.display()
            )));
        }
    }
    fs::create_dir_all(output_dir).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to create frame output directory: {}",
            e
        ))
    })?;

    let frame_pattern = output_dir.join("frame_%04d.jpg");
    let frame_pattern_str = frame_pattern.to_string_lossy();

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "info", "-n", "-i"])
        .arg(video_path)
        .args([
            "-vf",
            "select='eq(pict_type,I)',showinfo",
            "-fps_mode",
            "vfr",
            "-frames:v",
        ])
        .arg(max_frames.to_string())
        .arg(&*frame_pattern_str)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "FFmpeg is not installed or not found on system PATH: {}",
            e
        ))
    })?;

    let output = match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(res) => res.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("FFmpeg process execution error: {}", e))
        })?,
        Err(_) => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "FFmpeg extraction timed out after {} seconds.",
                timeout_duration.as_secs()
            )));
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        return Err(AppError::BadRequest(format!(
            "FFmpeg frame extraction failed: {}",
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        )));
    }
    let pts_values = parse_showinfo_pts_times(&stderr);

    // Read generated frames and compute cryptographic hashes
    let mut dir_entries = fs::read_dir(output_dir).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to read frame output directory: {}",
            e
        ))
    })?;

    let mut frame_files = Vec::new();
    while let Ok(Some(entry)) = dir_entries.next_entry().await {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .map(|ext| ext == "jpg" || ext == "jpeg")
                .unwrap_or(false)
        {
            frame_files.push(path);
        }
    }
    frame_files.sort();

    if frame_files.is_empty() {
        return Err(AppError::BadRequest(
            "FFmpeg produced no keyframes for this video.".to_string(),
        ));
    }
    if pts_values.len() != frame_files.len() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "FFmpeg reported {} frame PTS values for {} written frames; refusing to guess \
             frame timestamps.",
            pts_values.len(),
            frame_files.len()
        )));
    }

    let mut frame_artifacts = Vec::with_capacity(frame_files.len());
    for (idx, (frame_path, pts)) in frame_files.iter().zip(pts_values).enumerate() {
        let frame_hash = compute_file_sha256(frame_path)?;
        let file_name = frame_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        frame_artifacts.push(ExtractedFrameArtifact {
            frame_index: idx + 1,
            pts_seconds: pts,
            sha256_hash: frame_hash,
            relative_path: file_name,
            estimated_utc: None,
        });
    }

    // Post-operation immutability check
    let post_hash = compute_file_sha256(video_path)?;
    if pre_hash != post_hash {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Forensic integrity violation: source video was modified during FFmpeg frame extraction."
        )));
    }

    Ok(frame_artifacts)
}

/// Extracts `pts_time` values, in output order, from FFmpeg `showinfo` filter log lines.
pub fn parse_showinfo_pts_times(stderr: &str) -> Vec<f64> {
    stderr
        .lines()
        .filter(|line| line.contains("Parsed_showinfo") && line.contains(" n:"))
        .filter_map(|line| {
            let rest = &line[line.find("pts_time:")? + "pts_time:".len()..];
            let token = rest.split_whitespace().next()?;
            token.parse::<f64>().ok().filter(|v| v.is_finite())
        })
        .collect()
}
