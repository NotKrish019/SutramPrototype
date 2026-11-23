//! Forensic Content Sniffing and Format Validation (Member 2).
//!
//! Inspects file magic bytes, headers, and internal structures to verify evidence integrity.
//! Prevents file extension spoofing and classifies formats forensically:
//! - Generic CSV is classified as `TABULAR_CSV` (NOT prematurely as `CALL_DETAIL_RECORD`).
//! - JSON is inspected for chat structures before classifying as `CHAT_EXPORT`; otherwise `GENERIC_JSON`.
//! - Binary files are verified via magic bytes (PDF, PNG, JPEG, MP4/MOV).

use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::error::AppError;

/// Result of content sniffing and format validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentInspection {
    /// Verified MIME type.
    pub mime_type: String,
    /// Categorical source modality.
    pub source_type: String,
    /// Forensic diagnostic explaining why the format was recognized.
    pub diagnostic: String,
}

/// Inspects the header and contents of a stored raw evidence file.
///
/// Returns `Ok(ContentInspection)` if format is recognized and structurally valid.
/// Returns `Err(AppError)` if file is empty, malformed, or of an unsupported type.
pub async fn sniff_and_validate_content(
    file_path: &Path,
    declared_filename: &str,
) -> Result<ContentInspection, AppError> {
    let mut file = File::open(file_path).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to open evidence file for inspection: {}",
            e
        ))
    })?;

    let metadata = file.metadata().await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Failed to read evidence metadata: {}", e))
    })?;

    let file_size = metadata.len();
    if file_size == 0 {
        return Err(AppError::BadRequest(
            "Uploaded evidence file is empty (0 bytes).".to_string(),
        ));
    }

    // Read the first 4096 bytes for magic byte inspection
    let mut header = vec![0u8; 4096.min(file_size as usize)];
    file.read_exact(&mut header).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to read evidence header bytes: {}",
            e
        ))
    })?;

    let ext = Path::new(declared_filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    // 1. PDF Magic Bytes: '%PDF-'
    if header.starts_with(b"%PDF-") {
        return Ok(ContentInspection {
            mime_type: "application/pdf".to_string(),
            source_type: "DOCUMENT_PDF".to_string(),
            diagnostic: "Valid PDF document magic bytes (%PDF-) verified.".to_string(),
        });
    }

    // 2. PNG Magic Bytes: '\x89PNG\r\n\x1a\n'
    if header.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(ContentInspection {
            mime_type: "image/png".to_string(),
            source_type: "FORENSIC_IMAGE".to_string(),
            diagnostic: "Valid PNG image magic bytes verified.".to_string(),
        });
    }

    // 3. JPEG Magic Bytes: '\xff\xd8\xff'
    if header.starts_with(b"\xff\xd8\xff") {
        return Ok(ContentInspection {
            mime_type: "image/jpeg".to_string(),
            source_type: "FORENSIC_IMAGE".to_string(),
            diagnostic: "Valid JPEG image magic bytes verified.".to_string(),
        });
    }

    // 4. MP4 / MOV Video: 'ftyp' box in first 32 bytes
    if header.len() >= 12
        && header[..header.len().min(32)]
            .windows(4)
            .any(|w| w == b"ftyp")
    {
        return Ok(ContentInspection {
            mime_type: "video/mp4".to_string(),
            source_type: "CCTV_VIDEO".to_string(),
            diagnostic: "Valid ISO Base Media / MP4 ftyp box verified.".to_string(),
        });
    }

    // 5. JSON Structural Inspection
    let is_json_candidate = ext == "json"
        || header.trim_ascii_start().starts_with(b"{")
        || header.trim_ascii_start().starts_with(b"[");
    if is_json_candidate {
        // Read full file for JSON structural parse (bounded by MAX_UPLOAD_SIZE)
        let full_content = tokio::fs::read(file_path).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to read JSON evidence payload: {}",
                e
            ))
        })?;

        let parsed: serde_json::Value = serde_json::from_slice(&full_content)
            .map_err(|e| AppError::UnprocessableEntity(format!("Malformed JSON content: {}", e)))?;

        // Inspect structure to distinguish chat exports from generic JSON
        let is_chat = match &parsed {
            serde_json::Value::Object(map) => {
                map.contains_key("messages")
                    || map.contains_key("chats")
                    || (map.contains_key("conversation") && map.contains_key("participants"))
            }
            serde_json::Value::Array(arr) => {
                // If array, inspect first element for chat fields (sender, timestamp, text/content)
                if let Some(serde_json::Value::Object(first)) = arr.first() {
                    (first.contains_key("sender")
                        || first.contains_key("from")
                        || first.contains_key("author"))
                        && (first.contains_key("text")
                            || first.contains_key("message")
                            || first.contains_key("content"))
                } else {
                    false
                }
            }
            _ => false,
        };

        let source_type = if is_chat {
            "CHAT_EXPORT".to_string()
        } else {
            "GENERIC_JSON".to_string()
        };

        return Ok(ContentInspection {
            mime_type: "application/json".to_string(),
            source_type,
            diagnostic: "Valid JSON document structure verified.".to_string(),
        });
    }

    // 5b. WhatsApp plain-text chat export ("Export chat" -> .txt). Checked before CSV because
    // its "date, time - sender: text" lines contain commas and would otherwise read as CSV.
    if ext == "txt"
        && crate::extraction::whatsapp::looks_like_whatsapp_txt(&String::from_utf8_lossy(&header))
    {
        return Ok(ContentInspection {
            mime_type: "text/plain".to_string(),
            source_type: "CHAT_EXPORT".to_string(),
            diagnostic: "WhatsApp plain-text chat export layout verified (dated message lines)."
                .to_string(),
        });
    }

    // 6. CSV / Delimited Text Inspection
    // Crucial requirement: Generic CSV must NOT be classified as CALL_DETAIL_RECORD.
    // It must initially be classified as TABULAR_CSV until parsed in Phase 1H.
    let is_csv_candidate = ext == "csv" || ext == "tsv" || ext == "txt";
    if is_csv_candidate {
        if let Ok(text) = std::str::from_utf8(&header) {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            if lines.is_empty() {
                return Err(AppError::BadRequest(
                    "Delimited text evidence contains only whitespace.".to_string(),
                ));
            }

            // Check for delimiter consistency across first 2+ lines
            let candidate_delimiters = [',', ';', '\t', '|'];
            let mut detected_delim = None;

            for &delim in &candidate_delimiters {
                let first_count = lines[0].matches(delim).count();
                if first_count >= 1 {
                    if lines.len() > 1 {
                        let second_count = lines[1].matches(delim).count();
                        if first_count == second_count {
                            detected_delim = Some(delim);
                            break;
                        }
                    } else {
                        detected_delim = Some(delim);
                        break;
                    }
                }
            }

            if let Some(delim) = detected_delim {
                return Ok(ContentInspection {
                    mime_type: "text/csv".to_string(),
                    source_type: "TABULAR_CSV".to_string(),
                    diagnostic: format!("Valid delimited text structure verified (delimiter='{}'). Classified as TABULAR_CSV.", delim),
                });
            }

            // Fallback for plain text document
            if ext == "txt" {
                return Ok(ContentInspection {
                    mime_type: "text/plain".to_string(),
                    source_type: "DOCUMENT_TEXT".to_string(),
                    diagnostic: "Valid plain text document verified.".to_string(),
                });
            }
        }
    }

    // Unrecognized or unsupported file format
    Err(AppError::UnprocessableEntity(format!(
        "Content validation rejected evidence '{}': Unsupported or unrecognized evidence format.",
        declared_filename
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_sniff_empty_file_rejected() {
        let temp = NamedTempFile::new().unwrap();
        let err = sniff_and_validate_content(temp.path(), "empty.bin")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[tokio::test]
    async fn test_sniff_valid_pdf() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n")
            .unwrap();
        temp.flush().unwrap();

        let inspection = sniff_and_validate_content(temp.path(), "report.pdf")
            .await
            .unwrap();
        assert_eq!(inspection.mime_type, "application/pdf");
        assert_eq!(inspection.source_type, "DOCUMENT_PDF");
    }

    #[tokio::test]
    async fn test_sniff_valid_png() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
            .unwrap();
        temp.flush().unwrap();

        let inspection = sniff_and_validate_content(temp.path(), "screenshot.png")
            .await
            .unwrap();
        assert_eq!(inspection.mime_type, "image/png");
        assert_eq!(inspection.source_type, "FORENSIC_IMAGE");
    }

    #[tokio::test]
    async fn test_sniff_csv_not_classified_as_cdr() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"colA,colB,colC\n1,2,3\n4,5,6\n").unwrap();
        temp.flush().unwrap();

        let inspection = sniff_and_validate_content(temp.path(), "data.csv")
            .await
            .unwrap();
        assert_eq!(inspection.mime_type, "text/csv");
        // Must be TABULAR_CSV, NOT CALL_DETAIL_RECORD
        assert_eq!(inspection.source_type, "TABULAR_CSV");
    }

    #[tokio::test]
    async fn test_sniff_generic_json_not_chat() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"{\"system\": \"settings\", \"version\": 1}")
            .unwrap();
        temp.flush().unwrap();

        let inspection = sniff_and_validate_content(temp.path(), "config.json")
            .await
            .unwrap();
        assert_eq!(inspection.mime_type, "application/json");
        // Must be GENERIC_JSON, NOT CHAT_EXPORT
        assert_eq!(inspection.source_type, "GENERIC_JSON");
    }

    #[tokio::test]
    async fn test_sniff_chat_json_identified() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"{\"messages\": [{\"sender\": \"Alice\", \"text\": \"hello\"}]}")
            .unwrap();
        temp.flush().unwrap();

        let inspection = sniff_and_validate_content(temp.path(), "export.json")
            .await
            .unwrap();
        assert_eq!(inspection.mime_type, "application/json");
        assert_eq!(inspection.source_type, "CHAT_EXPORT");
    }

    #[tokio::test]
    async fn test_sniff_malformed_json_rejected() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"{ malformed json string").unwrap();
        temp.flush().unwrap();

        let err = sniff_and_validate_content(temp.path(), "bad.json")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::UnprocessableEntity(_)));
    }
}
