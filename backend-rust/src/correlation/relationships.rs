//! Observed Communication Relationship Synthesis (Member 1).
//!
//! Generates provisional, directly observed communication relationship candidates
//! strictly grounded in source CDR and messaging records. Does NOT generate speculative
//! association, guilt, or inferential relationship links.

use crate::{
    correlation::resolution::normalize_phone_number,
    extraction::model::{ParsedCDRRecord, ParsedChatMessage, ProvisionalRelationshipCandidate},
};

/// Generate an observed telephony call communication relationship candidate from a CDR row.
///
/// Strictly preserves observed evidence provenance and marks is_observed=true.
pub fn generate_cdr_relationship_candidate(
    record: &ParsedCDRRecord,
    default_country_prefix: Option<&str>,
) -> ProvisionalRelationshipCandidate {
    let caller_norm = normalize_phone_number(&record.caller_phone_raw, default_country_prefix);
    let callee_norm = normalize_phone_number(&record.receiver_phone_raw, default_country_prefix);

    let mut metadata_map = serde_json::Map::new();
    metadata_map.insert(
        "call_duration_seconds".to_string(),
        serde_json::json!(record.duration_seconds),
    );
    metadata_map.insert("call_type".to_string(), serde_json::json!(record.call_type));
    metadata_map.insert(
        "raw_timestamp".to_string(),
        serde_json::json!(record.call_timestamp_raw),
    );
    if let Some(ref tower) = record.cell_tower_id {
        metadata_map.insert("cell_tower_id".to_string(), serde_json::json!(tower));
    }

    ProvisionalRelationshipCandidate {
        candidate_id: format!("rel-cdr-{}-{}", record.source_evidence_id, record.record_id),
        source_evidence_id: record.source_evidence_id.clone(),
        source_record_id: record.record_id.clone(),
        source_raw_id: record.caller_phone_raw.clone(),
        target_raw_id: record.receiver_phone_raw.clone(),
        source_normalized_id: if caller_norm.is_normalized {
            caller_norm.normalized_e164
        } else {
            None
        },
        target_normalized_id: if callee_norm.is_normalized {
            callee_norm.normalized_e164
        } else {
            None
        },
        relation_type: "CALL_COMMUNICATION".to_string(),
        is_observed: true,
        metadata: serde_json::Value::Object(metadata_map),
        evidence_basis: vec![record.source_evidence_id.clone()],
    }
}

/// Generate observed message communication relationship candidate(s) from a chat message.
///
/// One candidate is created per recipient in 1-on-1 or multi-party messages.
pub fn generate_chat_relationship_candidates(
    message: &ParsedChatMessage,
    default_country_prefix: Option<&str>,
) -> Vec<ProvisionalRelationshipCandidate> {
    let mut candidates = Vec::new();
    let sender_norm = normalize_phone_number(&message.sender_raw_id, default_country_prefix);

    for (idx, recipient_raw) in message.recipients_raw.iter().enumerate() {
        let recipient_norm = normalize_phone_number(recipient_raw, default_country_prefix);

        let mut metadata_map = serde_json::Map::new();
        metadata_map.insert(
            "raw_timestamp".to_string(),
            serde_json::json!(message.timestamp_raw),
        );
        if let Some(ref chat_id) = message.chat_id {
            metadata_map.insert("chat_id".to_string(), serde_json::json!(chat_id));
        }
        if let Some(ref sender_name) = message.sender_name {
            metadata_map.insert(
                "sender_display_name".to_string(),
                serde_json::json!(sender_name),
            );
        }

        candidates.push(ProvisionalRelationshipCandidate {
            candidate_id: format!(
                "rel-chat-{}-{}-{}",
                message.source_evidence_id, message.message_id, idx
            ),
            source_evidence_id: message.source_evidence_id.clone(),
            source_record_id: message.message_id.clone(),
            source_raw_id: message.sender_raw_id.clone(),
            target_raw_id: recipient_raw.clone(),
            source_normalized_id: if sender_norm.is_normalized {
                sender_norm.normalized_e164.clone()
            } else {
                None
            },
            target_normalized_id: if recipient_norm.is_normalized {
                recipient_norm.normalized_e164
            } else {
                None
            },
            relation_type: "MESSAGE_COMMUNICATION".to_string(),
            is_observed: true,
            metadata: serde_json::Value::Object(metadata_map),
            evidence_basis: vec![message.source_evidence_id.clone()],
        });
    }

    candidates
}
