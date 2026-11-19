//! Chat / Instant Messaging Export Extraction Engine (Member 1).
//!
//! Parses messaging export JSON files (e.g. WhatsApp, Telegram, Signal, SMS)
//! deterministically. Validates message structure, extracts embedded text identifiers,
//! preserves original timestamp and content strings without mutation, and generates
//! observed message communication relationship candidates.

use crate::{
    correlation::{
        relationships::generate_chat_relationship_candidates, resolution::normalize_phone_number,
    },
    extraction::{
        identifiers::extract_identifiers_from_text,
        model::{
            ChatExtractionResult, ExtractedEventCandidate, ExtractedIdentifier,
            ExtractionValidationIssue, ParsedChatMessage,
        },
    },
};
use serde_json::Value;

/// Deterministically parse a messaging export JSON into structured candidates and issues.
///
/// Forensic guarantees:
/// - Never mutates the source file.
/// - Explicitly records missing or malformed message items.
/// - Scans message content for embedded identifiers (emails, phone numbers, IMEIs, IMSIs).
/// - Preserves uncorrected timestamps and raw message bodies verbatim.
/// - Does NOT assert legal identity from phone numbers alone.
pub fn parse_chat_json(
    raw_json: &str,
    source_evidence_id: &str,
    default_country_prefix: Option<&str>,
) -> ChatExtractionResult {
    let data: Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(e) => {
            return ChatExtractionResult {
                source_evidence_id: source_evidence_id.to_string(),
                chat_id: None,
                total_messages_processed: 0,
                valid_messages: Vec::new(),
                rejected_messages: vec![ExtractionValidationIssue {
                    record_id: None,
                    field_name: "json".to_string(),
                    issue_type: "JSON_DECODE_ERROR".to_string(),
                    raw_value: None,
                    message: format!("Malformed JSON file: {}.", e),
                    row_index: None,
                }],
                identifiers: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
            };
        }
    };

    let obj = match data.as_object() {
        Some(o) => o,
        None => {
            return ChatExtractionResult {
                source_evidence_id: source_evidence_id.to_string(),
                chat_id: None,
                total_messages_processed: 0,
                valid_messages: Vec::new(),
                rejected_messages: vec![ExtractionValidationIssue {
                    record_id: None,
                    field_name: "root".to_string(),
                    issue_type: "INVALID_ROOT_OBJECT".to_string(),
                    raw_value: None,
                    message: format!(
                        "Top-level JSON structure must be an object (dict), got {}.",
                        match data {
                            Value::Array(_) => "list",
                            Value::String(_) => "str",
                            Value::Number(_) => "int",
                            Value::Bool(_) => "bool",
                            Value::Null => "NoneType",
                            _ => "unknown",
                        }
                    ),
                    row_index: None,
                }],
                identifiers: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
            };
        }
    };

    let chat_id = obj
        .get("chat_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let chat_title = obj
        .get("chat_title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let raw_messages = obj
        .get("messages")
        .and_then(|v| v.as_array())
        .or_else(|| obj.get("conversations").and_then(|v| v.as_array()));

    let raw_messages = match raw_messages {
        Some(arr) => arr,
        None => {
            return ChatExtractionResult {
                source_evidence_id: source_evidence_id.to_string(),
                chat_id,
                total_messages_processed: 0,
                valid_messages: Vec::new(),
                rejected_messages: vec![ExtractionValidationIssue {
                    record_id: None,
                    field_name: "messages".to_string(),
                    issue_type: "MISSING_MESSAGES_ARRAY".to_string(),
                    raw_value: None,
                    message: "Top-level JSON must contain a 'messages' list.".to_string(),
                    row_index: None,
                }],
                identifiers: Vec::new(),
                event_candidates: Vec::new(),
                relationship_candidates: Vec::new(),
            };
        }
    };

    let mut valid_messages = Vec::new();
    let mut rejected_messages = Vec::new();
    let mut identifiers = Vec::new();
    let mut event_candidates = Vec::new();
    let mut relationship_candidates = Vec::new();

    for (idx_0, msg_val) in raw_messages.iter().enumerate() {
        let idx = idx_0 + 1;
        let msg = match msg_val.as_object() {
            Some(m) => m,
            None => {
                rejected_messages.push(ExtractionValidationIssue {
                    record_id: None,
                    field_name: "message".to_string(),
                    issue_type: "INVALID_MESSAGE_OBJECT".to_string(),
                    raw_value: None,
                    message: format!("Message at index {} is not a dictionary.", idx),
                    row_index: Some(idx),
                });
                continue;
            }
        };

        let msg_id = msg
            .get("message_id")
            .or_else(|| msg.get("id"))
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.trim().to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .unwrap_or_default();

        let sender_id = msg
            .get("sender_id")
            .or_else(|| msg.get("sender"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let sender_name = msg
            .get("sender_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let timestamp_raw = msg
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let content_val = msg.get("content").or_else(|| msg.get("text"));

        let mut has_error = false;
        let fallback_record_id = if msg_id.is_empty() {
            format!("msg-index-{}", idx)
        } else {
            msg_id.clone()
        };

        if msg_id.is_empty() {
            rejected_messages.push(ExtractionValidationIssue {
                record_id: Some(format!("msg-index-{}", idx)),
                field_name: "message_id".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: None,
                message: format!("Message at index {} lacks mandatory 'message_id'.", idx),
                row_index: Some(idx),
            });
            has_error = true;
        }

        if sender_id.is_empty() {
            let label = if msg_id.is_empty() {
                idx.to_string()
            } else {
                msg_id.clone()
            };
            rejected_messages.push(ExtractionValidationIssue {
                record_id: Some(fallback_record_id.clone()),
                field_name: "sender_id".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: None,
                message: format!("Message '{}' lacks mandatory 'sender_id'.", label),
                row_index: Some(idx),
            });
            has_error = true;
        }

        if timestamp_raw.is_empty() {
            let label = if msg_id.is_empty() {
                idx.to_string()
            } else {
                msg_id.clone()
            };
            rejected_messages.push(ExtractionValidationIssue {
                record_id: Some(fallback_record_id.clone()),
                field_name: "timestamp".to_string(),
                issue_type: "MISSING_REQUIRED_FIELD".to_string(),
                raw_value: None,
                message: format!("Message '{}' lacks mandatory 'timestamp'.", label),
                row_index: Some(idx),
            });
            has_error = true;
        }

        let content_raw = match content_val {
            Some(Value::String(s)) => s.clone(),
            Some(other) => {
                let label = if msg_id.is_empty() {
                    idx.to_string()
                } else {
                    msg_id.clone()
                };
                let type_name = match other {
                    Value::Array(_) => "list",
                    Value::Number(_) => "int",
                    Value::Bool(_) => "bool",
                    Value::Null => "NoneType",
                    Value::Object(_) => "dict",
                    _ => "unknown",
                };
                rejected_messages.push(ExtractionValidationIssue {
                    record_id: Some(fallback_record_id.clone()),
                    field_name: "content".to_string(),
                    issue_type: "INVALID_CONTENT_TYPE".to_string(),
                    raw_value: None,
                    message: format!(
                        "Message '{}' has invalid content (expected string, got {}).",
                        label, type_name
                    ),
                    row_index: Some(idx),
                });
                has_error = true;
                String::new()
            }
            None => {
                let label = if msg_id.is_empty() {
                    idx.to_string()
                } else {
                    msg_id.clone()
                };
                rejected_messages.push(ExtractionValidationIssue {
                    record_id: Some(fallback_record_id.clone()),
                    field_name: "content".to_string(),
                    issue_type: "INVALID_CONTENT_TYPE".to_string(),
                    raw_value: None,
                    message: format!(
                        "Message '{}' has invalid content (expected string, got NoneType).",
                        label
                    ),
                    row_index: Some(idx),
                });
                has_error = true;
                String::new()
            }
        };

        if has_error {
            continue;
        }

        // Format recipients
        let mut recipients = Vec::new();
        if let Some(r_val) = msg.get("recipients") {
            if let Some(arr) = r_val.as_array() {
                for r in arr {
                    if let Some(s) = r.as_str() {
                        let trimmed = s.trim().to_string();
                        if !trimmed.is_empty() {
                            recipients.push(trimmed);
                        }
                    }
                }
            } else if let Some(s) = r_val.as_str() {
                let trimmed = s.trim().to_string();
                if !trimmed.is_empty() {
                    recipients.push(trimmed);
                }
            }
        }

        let parsed_msg = ParsedChatMessage {
            source_evidence_id: source_evidence_id.to_string(),
            message_id: msg_id.clone(),
            chat_id: chat_id.clone(),
            sender_raw_id: sender_id.clone(),
            sender_name: sender_name.clone(),
            recipients_raw: recipients.clone(),
            timestamp_raw: timestamp_raw.clone(),
            content_raw: content_raw.clone(),
        };
        valid_messages.push(parsed_msg.clone());

        // 1. Structural Identifiers (Sender & Recipients)
        let sender_norm = normalize_phone_number(&sender_id, default_country_prefix);
        identifiers.push(ExtractedIdentifier {
            identifier_type: "PHONE_NUMBER".to_string(),
            raw_value: sender_id.clone(),
            normalized_value: if sender_norm.is_normalized {
                sender_norm.normalized_e164
            } else {
                None
            },
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: msg_id.clone(),
            location_reference: Some("field:sender_id".to_string()),
            is_ambiguous: false,
            ambiguity_reason: None,
            context_snippet: None,
            is_checksum_valid: None,
            extraction_confidence: None,
            extraction_method: "chat_metadata".to_string(),
        });

        for rec_raw in &recipients {
            let rec_norm = normalize_phone_number(rec_raw, default_country_prefix);
            identifiers.push(ExtractedIdentifier {
                identifier_type: "PHONE_NUMBER".to_string(),
                raw_value: rec_raw.clone(),
                normalized_value: if rec_norm.is_normalized {
                    rec_norm.normalized_e164
                } else {
                    None
                },
                source_evidence_id: source_evidence_id.to_string(),
                source_record_id: msg_id.clone(),
                location_reference: Some("field:recipients".to_string()),
                is_ambiguous: false,
                ambiguity_reason: None,
                context_snippet: None,
                is_checksum_valid: None,
                extraction_confidence: None,
                extraction_method: "chat_metadata".to_string(),
            });
        }

        // 2. Free-text NER Identifier Extraction from content
        let content_identifiers = extract_identifiers_from_text(
            &content_raw,
            source_evidence_id,
            &msg_id,
            default_country_prefix,
        );
        identifiers.extend(content_identifiers);

        // 3. Event Candidate Creation
        let mut involved = vec![sender_id.clone()];
        for r in &recipients {
            if r != &sender_id {
                involved.push(r.clone());
            }
        }

        let mut evt_meta = serde_json::Map::new();
        if let Some(ref cid) = chat_id {
            evt_meta.insert("chat_id".to_string(), serde_json::json!(cid));
        }
        if let Some(ref ctitle) = chat_title {
            evt_meta.insert("chat_title".to_string(), serde_json::json!(ctitle));
        }
        if let Some(ref sname) = sender_name {
            evt_meta.insert("sender_name".to_string(), serde_json::json!(sname));
        }
        evt_meta.insert(
            "message_length".to_string(),
            serde_json::json!(content_raw.chars().count()),
        );

        event_candidates.push(ExtractedEventCandidate {
            candidate_id: format!("evt-chat-{}-{}", source_evidence_id, msg_id),
            source_evidence_id: source_evidence_id.to_string(),
            source_record_id: msg_id,
            event_type: "MESSAGING_EXCHANGE".to_string(),
            raw_timestamp: timestamp_raw,
            duration_seconds: None,
            location_name: None,
            involved_entity_raw_ids: involved,
            metadata: serde_json::Value::Object(evt_meta),
        });

        // 4. Relationship Candidate Creation
        let rel_cands = generate_chat_relationship_candidates(&parsed_msg, default_country_prefix);
        relationship_candidates.extend(rel_cands);
    }

    ChatExtractionResult {
        source_evidence_id: source_evidence_id.to_string(),
        chat_id,
        total_messages_processed: raw_messages.len(),
        valid_messages,
        rejected_messages,
        identifiers,
        event_candidates,
        relationship_candidates,
    }
}
