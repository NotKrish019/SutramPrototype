"""Chat / Instant Messaging Export Extraction Engine (Member 1).

Parses messaging export JSON files (e.g. WhatsApp, Telegram, Signal, SMS)
deterministically. Validates message structure, extracts embedded text identifiers,
preserves original timestamp and content strings without mutation, and generates
observed message communication relationship candidates.
"""

from __future__ import annotations

import io
import json
from pathlib import Path
from typing import Any, Optional, Union

from app.modules.correlation.relationships import generate_chat_relationship_candidates
from app.modules.correlation.resolution import normalize_phone_number
from app.modules.extraction.ner import extract_identifiers_from_text
from app.schemas.extraction import (
    ChatExtractionResult,
    ExtractedEventCandidate,
    ExtractedIdentifier,
    ExtractionValidationIssue,
    ParsedChatMessage,
)


def parse_chat_json(
    json_source: Union[str, Path, dict[str, Any], io.TextIOBase],
    source_evidence_id: str,
    *,
    default_country_prefix: Optional[str] = None,
) -> ChatExtractionResult:
    """Deterministically parse a messaging export JSON into structured candidates and issues.

    Forensic guarantees:
    - Never mutates the source file.
    - Explicitly records missing or malformed message items.
    - Scans message content for embedded identifiers (emails, phone numbers, IMEIs, IMSIs).
    - Preserves uncorrected timestamps and raw message bodies verbatim.
    - Does NOT assert legal identity from phone numbers alone.
    """
    data: Any
    if isinstance(json_source, dict):
        data = json_source
    elif isinstance(json_source, Path):
        try:
            with open(json_source, "r", encoding="utf-8-sig", errors="replace") as f:
                data = json.load(f)
        except json.JSONDecodeError as exc:
            return ChatExtractionResult(
                source_evidence_id=source_evidence_id,
                total_messages_processed=0,
                rejected_messages=[
                    ExtractionValidationIssue(
                        field_name="json",
                        issue_type="JSON_DECODE_ERROR",
                        message=f"Malformed JSON file: {exc}.",
                    )
                ],
            )
    elif isinstance(json_source, io.TextIOBase):
        try:
            data = json.load(json_source)
        except json.JSONDecodeError as exc:
            return ChatExtractionResult(
                source_evidence_id=source_evidence_id,
                total_messages_processed=0,
                rejected_messages=[
                    ExtractionValidationIssue(
                        field_name="json",
                        issue_type="JSON_DECODE_ERROR",
                        message=f"Malformed JSON stream: {exc}.",
                    )
                ],
            )
    elif isinstance(json_source, str):
        # Could be JSON string or file path
        stripped = json_source.strip()
        if (stripped.startswith("{") or stripped.startswith("[")) and "\n" in json_source:
            try:
                data = json.loads(json_source)
            except json.JSONDecodeError as exc:
                return ChatExtractionResult(
                    source_evidence_id=source_evidence_id,
                    total_messages_processed=0,
                    rejected_messages=[
                        ExtractionValidationIssue(
                            field_name="json",
                            issue_type="JSON_DECODE_ERROR",
                            message=f"Malformed JSON string: {exc}.",
                        )
                    ],
                )
        elif Path(json_source).is_file():
            try:
                with open(json_source, "r", encoding="utf-8-sig", errors="replace") as f:
                    data = json.load(f)
            except json.JSONDecodeError as exc:
                return ChatExtractionResult(
                    source_evidence_id=source_evidence_id,
                    total_messages_processed=0,
                    rejected_messages=[
                        ExtractionValidationIssue(
                            field_name="json",
                            issue_type="JSON_DECODE_ERROR",
                            message=f"Malformed JSON file: {exc}.",
                        )
                    ],
                )
        else:
            try:
                data = json.loads(json_source)
            except json.JSONDecodeError as exc:
                return ChatExtractionResult(
                    source_evidence_id=source_evidence_id,
                    total_messages_processed=0,
                    rejected_messages=[
                        ExtractionValidationIssue(
                            field_name="json",
                            issue_type="JSON_DECODE_ERROR",
                            message=f"Malformed JSON content or file not found: {exc}.",
                        )
                    ],
                )
    else:
        raise TypeError(f"Unsupported json_source type: {type(json_source).__name__}")

    if not isinstance(data, dict):
        return ChatExtractionResult(
            source_evidence_id=source_evidence_id,
            total_messages_processed=0,
            rejected_messages=[
                ExtractionValidationIssue(
                    field_name="root",
                    issue_type="INVALID_ROOT_OBJECT",
                    message=f"Top-level JSON structure must be an object (dict), got {type(data).__name__}.",
                )
            ],
        )

    chat_id = data.get("chat_id")
    chat_title = data.get("chat_title")
    raw_messages = data.get("messages")

    if raw_messages is None or not isinstance(raw_messages, list):
        return ChatExtractionResult(
            source_evidence_id=source_evidence_id,
            chat_id=chat_id,
            total_messages_processed=0,
            rejected_messages=[
                ExtractionValidationIssue(
                    field_name="messages",
                    issue_type="MISSING_MESSAGES_ARRAY",
                    message="Top-level JSON must contain a 'messages' list.",
                )
            ],
        )

    valid_messages: list[ParsedChatMessage] = []
    rejected_messages: list[ExtractionValidationIssue] = []
    identifiers: list[ExtractedIdentifier] = []
    event_candidates: list[ExtractedEventCandidate] = []
    relationship_candidates = []

    for idx, msg in enumerate(raw_messages, start=1):
        if not isinstance(msg, dict):
            rejected_messages.append(
                ExtractionValidationIssue(
                    field_name="message",
                    issue_type="INVALID_MESSAGE_OBJECT",
                    message=f"Message at index {idx} is not a dictionary.",
                    row_index=idx,
                )
            )
            continue

        msg_id = str(msg.get("message_id") or "").strip()
        sender_id = str(msg.get("sender_id") or "").strip()
        sender_name = msg.get("sender_name")
        timestamp_raw = str(msg.get("timestamp") or "").strip()
        content_raw = msg.get("content")
        raw_recipients = msg.get("recipients", [])

        has_error = False
        if not msg_id:
            rejected_messages.append(
                ExtractionValidationIssue(
                    record_id=f"msg-index-{idx}",
                    field_name="message_id",
                    issue_type="MISSING_REQUIRED_FIELD",
                    message=f"Message at index {idx} lacks mandatory 'message_id'.",
                    row_index=idx,
                )
            )
            has_error = True

        if not sender_id:
            rejected_messages.append(
                ExtractionValidationIssue(
                    record_id=msg_id or f"msg-index-{idx}",
                    field_name="sender_id",
                    issue_type="MISSING_REQUIRED_FIELD",
                    message=f"Message '{msg_id or idx}' lacks mandatory 'sender_id'.",
                    row_index=idx,
                )
            )
            has_error = True

        if not timestamp_raw:
            rejected_messages.append(
                ExtractionValidationIssue(
                    record_id=msg_id or f"msg-index-{idx}",
                    field_name="timestamp",
                    issue_type="MISSING_REQUIRED_FIELD",
                    message=f"Message '{msg_id or idx}' lacks mandatory 'timestamp'.",
                    row_index=idx,
                )
            )
            has_error = True

        if content_raw is None or not isinstance(content_raw, str):
            rejected_messages.append(
                ExtractionValidationIssue(
                    record_id=msg_id or f"msg-index-{idx}",
                    field_name="content",
                    issue_type="INVALID_CONTENT_TYPE",
                    message=f"Message '{msg_id or idx}' has invalid content (expected string, got {type(content_raw).__name__}).",
                    row_index=idx,
                )
            )
            has_error = True

        if has_error:
            continue

        # Format recipients
        if isinstance(raw_recipients, list):
            recipients = [str(r).strip() for r in raw_recipients if str(r).strip()]
        elif isinstance(raw_recipients, str) and raw_recipients.strip():
            recipients = [raw_recipients.strip()]
        else:
            recipients = []

        parsed_msg = ParsedChatMessage(
            source_evidence_id=source_evidence_id,
            message_id=msg_id,
            chat_id=chat_id,
            sender_raw_id=sender_id,
            sender_name=str(sender_name) if sender_name else None,
            recipients_raw=recipients,
            timestamp_raw=timestamp_raw,
            content_raw=content_raw,
        )
        valid_messages.append(parsed_msg)

        # 1. Structural Identifiers (Sender & Recipients)
        sender_norm = normalize_phone_number(sender_id, default_country_prefix=default_country_prefix)
        identifiers.append(
            ExtractedIdentifier(
                identifier_type="PHONE_NUMBER",
                raw_value=sender_id,
                normalized_value=sender_norm.normalized_e164 if sender_norm.is_normalized else None,
                source_evidence_id=source_evidence_id,
                source_record_id=msg_id,
                location_reference="field:sender_id",
                extraction_method="chat_metadata",
            )
        )

        for rec_raw in recipients:
            rec_norm = normalize_phone_number(rec_raw, default_country_prefix=default_country_prefix)
            identifiers.append(
                ExtractedIdentifier(
                    identifier_type="PHONE_NUMBER",
                    raw_value=rec_raw,
                    normalized_value=rec_norm.normalized_e164 if rec_norm.is_normalized else None,
                    source_evidence_id=source_evidence_id,
                    source_record_id=msg_id,
                    location_reference="field:recipients",
                    extraction_method="chat_metadata",
                )
            )

        # 2. Free-text NER Identifier Extraction (content)
        content_identifiers = extract_identifiers_from_text(
            content_raw,
            source_evidence_id=source_evidence_id,
            source_record_id=msg_id,
            default_country_code=default_country_prefix,
        )
        identifiers.extend(content_identifiers)

        # 3. Event Candidate Creation
        involved = [sender_id] + [r for r in recipients if r != sender_id]
        event_candidates.append(
            ExtractedEventCandidate(
                candidate_id=f"evt-chat-{source_evidence_id}-{msg_id}",
                source_evidence_id=source_evidence_id,
                source_record_id=msg_id,
                event_type="MESSAGING_EXCHANGE",
                raw_timestamp=timestamp_raw,
                duration_seconds=None,  # Instantaneous
                location_name=None,
                involved_entity_raw_ids=involved,
                metadata={
                    "chat_id": chat_id,
                    "chat_title": chat_title,
                    "sender_name": sender_name,
                    "message_length": len(content_raw),
                },
            )
        )

        # 4. Relationship Candidate Creation
        rel_candidates = generate_chat_relationship_candidates(
            parsed_msg, default_country_prefix=default_country_prefix
        )
        relationship_candidates.extend(rel_candidates)

    return ChatExtractionResult(
        source_evidence_id=source_evidence_id,
        chat_id=chat_id,
        total_messages_processed=len(raw_messages),
        valid_messages=valid_messages,
        rejected_messages=rejected_messages,
        identifiers=identifiers,
        event_candidates=event_candidates,
        relationship_candidates=relationship_candidates,
    )
