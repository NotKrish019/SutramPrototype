"""Unit tests for observed communication relationship candidate synthesis."""

import pytest
from app.modules.correlation.relationships import (
    generate_cdr_relationship_candidate,
    generate_chat_relationship_candidates,
)
from app.schemas.extraction import ParsedCDRRecord, ParsedChatMessage


def test_cdr_call_relationship_candidate():
    """Verify observed call communication candidate synthesis from a CDR record."""
    cdr_rec = ParsedCDRRecord(
        source_evidence_id="ev-cdr-99",
        record_id="rec-99",
        caller_phone_raw="09811002233",
        receiver_phone_raw="+919899887766",
        call_timestamp_raw="2026-11-10T21:45:12+05:30",
        duration_seconds=120.5,
        cell_tower_id="TOWER-NORTH-01",
        call_type="VOICE_OUTGOING",
        row_index=5,
    )

    candidate = generate_cdr_relationship_candidate(cdr_rec, default_country_prefix="91")

    assert candidate.candidate_id == "rel-cdr-ev-cdr-99-rec-99"
    assert candidate.source_evidence_id == "ev-cdr-99"
    assert candidate.source_record_id == "rec-99"
    assert candidate.source_raw_id == "09811002233"
    assert candidate.target_raw_id == "+919899887766"
    assert candidate.source_normalized_id == "+919811002233"
    assert candidate.target_normalized_id == "+919899887766"
    assert candidate.relation_type == "CALL_COMMUNICATION"
    assert candidate.is_observed is True
    assert candidate.metadata["call_duration_seconds"] == 120.5
    assert candidate.metadata["cell_tower_id"] == "TOWER-NORTH-01"
    assert candidate.evidence_basis == ["ev-cdr-99"]


def test_chat_multi_recipient_relationship_candidates():
    """Verify observed message communication candidates for messages with multiple recipients."""
    chat_msg = ParsedChatMessage(
        source_evidence_id="ev-chat-88",
        message_id="msg-88",
        chat_id="chat-ops-01",
        sender_raw_id="+919811002233",
        sender_name="Alpha",
        recipients_raw=["+919899887766", "+919876543210"],
        timestamp_raw="2026-11-10T20:00:00Z",
        content_raw="Meeting confirmed for both parties.",
    )

    candidates = generate_chat_relationship_candidates(chat_msg, default_country_prefix="91")

    assert len(candidates) == 2

    first = candidates[0]
    assert first.relation_type == "MESSAGE_COMMUNICATION"
    assert first.is_observed is True
    assert first.source_raw_id == "+919811002233"
    assert first.target_raw_id == "+919899887766"
    assert first.metadata["chat_id"] == "chat-ops-01"
    assert first.evidence_basis == ["ev-chat-88"]

    second = candidates[1]
    assert second.source_raw_id == "+919811002233"
    assert second.target_raw_id == "+919876543210"
    assert second.is_observed is True
