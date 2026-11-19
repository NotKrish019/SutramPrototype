"""Unit tests for Chat / Instant Messaging export extraction engine."""

import hashlib
from pathlib import Path
import pytest

from app.modules.extraction.chat import parse_chat_json

SAMPLE_CHAT_PATH = Path(__file__).resolve().parents[3] / "data" / "synthetic" / "sample_chat_export.json"


def test_valid_synthetic_chat_export_parsing():
    """Verify parsing of valid synthetic chat JSON export."""
    assert SAMPLE_CHAT_PATH.is_file(), f"Synthetic fixture missing: {SAMPLE_CHAT_PATH}"

    result = parse_chat_json(
        SAMPLE_CHAT_PATH,
        source_evidence_id="ev-chat-001",
        default_country_prefix="91",
    )

    assert result.source_evidence_id == "ev-chat-001"
    assert result.chat_id == "chat-grp-synthetic-901"
    assert result.total_messages_processed == 3
    assert len(result.valid_messages) == 3
    assert len(result.rejected_messages) == 0

    first = result.valid_messages[0]
    assert first.message_id == "msg-001"
    assert first.sender_raw_id == "+919811002233"
    assert first.sender_name == "Synthetic Target Alpha"
    assert first.recipients_raw == ["+919899887766"]
    assert first.timestamp_raw == "2026-11-10T20:15:30+05:30"
    assert "Consignment dispatched" in first.content_raw

    # Verify event candidates
    assert len(result.event_candidates) == 3
    evt = result.event_candidates[0]
    assert evt.candidate_id == "evt-chat-ev-chat-001-msg-001"
    assert evt.event_type == "MESSAGING_EXCHANGE"
    assert evt.raw_timestamp == "2026-11-10T20:15:30+05:30"
    assert evt.duration_seconds is None  # Instantaneous
    assert "+919811002233" in evt.involved_entity_raw_ids
    assert "+919899887766" in evt.involved_entity_raw_ids

    # Verify relationships
    # msg-001 has 1 recipient, msg-002 has 1 recipient, msg-003 has 2 recipients -> total 4 relationships
    assert len(result.relationship_candidates) == 4
    rel0 = result.relationship_candidates[0]
    assert rel0.relation_type == "MESSAGE_COMMUNICATION"
    assert rel0.is_observed is True
    assert rel0.source_raw_id == "+919811002233"
    assert rel0.target_raw_id == "+919899887766"
    assert rel0.evidence_basis == ["ev-chat-001"]

    # Verify free-text embedded identifier extraction in msg-001 content:
    # "Call me at 09811002233 or email alpha.lead@synthetic-forensics.internal. Device IMEI 861234567890123."
    extracted_types = [ident.identifier_type for ident in result.identifiers]
    assert "EMAIL" in extracted_types
    assert "IMEI" in extracted_types
    assert "IMSI" in extracted_types  # from msg-002 ("SIM IMSI 404450123456789")

    email_ident = next(i for i in result.identifiers if i.identifier_type == "EMAIL")
    assert email_ident.raw_value == "alpha.lead@synthetic-forensics.internal"

    imei_ident = next(i for i in result.identifiers if i.identifier_type == "IMEI")
    assert imei_ident.raw_value == "861234567890123"

    imsi_ident = next(i for i in result.identifiers if i.identifier_type == "IMSI")
    assert imsi_ident.raw_value == "404450123456789"


def test_malformed_json_handling():
    """Verify clean rejection when JSON syntax is malformed."""
    malformed_json = '{"chat_id": "test", "messages": [{'
    result = parse_chat_json(malformed_json, source_evidence_id="ev-malformed")

    assert len(result.valid_messages) == 0
    assert len(result.rejected_messages) == 1
    assert result.rejected_messages[0].issue_type == "JSON_DECODE_ERROR"


def test_missing_mandatory_fields():
    """Verify non-destructive message-level rejection on missing keys."""
    invalid_data = {
        "chat_id": "test-chat",
        "messages": [
            {"message_id": "m1", "sender_id": "s1", "timestamp": "t1", "content": "hello"},  # valid
            {"message_id": "m2", "timestamp": "t2", "content": "no sender"},  # missing sender_id
            {"sender_id": "s3", "timestamp": "t3", "content": "no message_id"},  # missing message_id
            {"message_id": "m4", "sender_id": "s4", "content": "no timestamp"},  # missing timestamp
            {"message_id": "m5", "sender_id": "s5", "timestamp": "t5", "content": 12345},  # invalid content type
        ],
    }

    result = parse_chat_json(invalid_data, source_evidence_id="ev-missing-fields")
    assert result.total_messages_processed == 5
    assert len(result.valid_messages) == 1
    assert result.valid_messages[0].message_id == "m1"
    assert len(result.rejected_messages) == 4


def test_chat_evidence_immutability():
    """Verify that source chat file SHA-256 remains completely untouched."""
    sha_before = hashlib.sha256(SAMPLE_CHAT_PATH.read_bytes()).hexdigest()
    _ = parse_chat_json(SAMPLE_CHAT_PATH, source_evidence_id="ev-chat-001")
    sha_after = hashlib.sha256(SAMPLE_CHAT_PATH.read_bytes()).hexdigest()
    assert sha_before == sha_after, "Forensic violation: source chat file was mutated!"
