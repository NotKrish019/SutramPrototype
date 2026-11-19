"""Observed Communication Relationship Synthesis (Member 1).

Generates provisional, directly observed communication relationship candidates
strictly grounded in source CDR and messaging records. Does NOT generate speculative
association, guilt, or inferential relationship links.
"""

from __future__ import annotations

from typing import Optional

from app.modules.correlation.resolution import normalize_phone_number
from app.schemas.extraction import (
    ParsedCDRRecord,
    ParsedChatMessage,
    ProvisionalRelationshipCandidate,
)


def generate_cdr_relationship_candidate(
    record: ParsedCDRRecord,
    *,
    default_country_prefix: Optional[str] = None,
) -> ProvisionalRelationshipCandidate:
    """Generate an observed telephony call communication relationship candidate from a CDR row.

    Strictly preserves observed evidence provenance and marks is_observed=True.
    """
    caller_norm = normalize_phone_number(
        record.caller_phone_raw, default_country_prefix=default_country_prefix
    )
    callee_norm = normalize_phone_number(
        record.receiver_phone_raw, default_country_prefix=default_country_prefix
    )

    metadata = {
        "call_duration_seconds": record.duration_seconds,
        "call_type": record.call_type,
        "raw_timestamp": record.call_timestamp_raw,
    }
    if record.cell_tower_id:
        metadata["cell_tower_id"] = record.cell_tower_id

    return ProvisionalRelationshipCandidate(
        candidate_id=f"rel-cdr-{record.source_evidence_id}-{record.record_id}",
        source_evidence_id=record.source_evidence_id,
        source_record_id=record.record_id,
        source_raw_id=record.caller_phone_raw,
        target_raw_id=record.receiver_phone_raw,
        source_normalized_id=caller_norm.normalized_e164 if caller_norm.is_normalized else None,
        target_normalized_id=callee_norm.normalized_e164 if callee_norm.is_normalized else None,
        relation_type="CALL_COMMUNICATION",
        is_observed=True,
        metadata=metadata,
        evidence_basis=[record.source_evidence_id],
    )


def generate_chat_relationship_candidates(
    message: ParsedChatMessage,
    *,
    default_country_prefix: Optional[str] = None,
) -> list[ProvisionalRelationshipCandidate]:
    """Generate observed message communication relationship candidate(s) from a chat message.

    One candidate is created per recipient in 1-on-1 or multi-party messages.
    """
    candidates: list[ProvisionalRelationshipCandidate] = []
    sender_norm = normalize_phone_number(
        message.sender_raw_id, default_country_prefix=default_country_prefix
    )

    for idx, recipient_raw in enumerate(message.recipients_raw):
        recipient_norm = normalize_phone_number(
            recipient_raw, default_country_prefix=default_country_prefix
        )

        metadata = {
            "raw_timestamp": message.timestamp_raw,
        }
        if message.chat_id:
            metadata["chat_id"] = message.chat_id
        if message.sender_name:
            metadata["sender_display_name"] = message.sender_name

        candidates.append(
            ProvisionalRelationshipCandidate(
                candidate_id=f"rel-chat-{message.source_evidence_id}-{message.message_id}-{idx}",
                source_evidence_id=message.source_evidence_id,
                source_record_id=message.message_id,
                source_raw_id=message.sender_raw_id,
                target_raw_id=recipient_raw,
                source_normalized_id=sender_norm.normalized_e164 if sender_norm.is_normalized else None,
                target_normalized_id=recipient_norm.normalized_e164 if recipient_norm.is_normalized else None,
                relation_type="MESSAGE_COMMUNICATION",
                is_observed=True,
                metadata=metadata,
                evidence_basis=[message.source_evidence_id],
            )
        )

    return candidates
