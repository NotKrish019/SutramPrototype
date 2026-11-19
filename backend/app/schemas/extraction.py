"""Provisional Extraction & Correlation Data Transfer Objects (Member 1).

These schemas represent internal, provisional data structures for the extraction
and entity resolution pipeline. They strictly preserve forensic provenance and
original observed values without pre-empting the unapproved shared domain models
(Case, Evidence, Entity, Event, Relationship).
"""

from enum import Enum
from pathlib import Path
from typing import Any, Optional, Union
from pydantic import BaseModel, ConfigDict, Field


class ExtractionStatus(str, Enum):
    """Execution status of an evidence extraction dispatch."""

    SUCCESS = "SUCCESS"
    PARTIAL_SUCCESS = "PARTIAL_SUCCESS"
    FAILED = "FAILED"


class ResolvedEntityCandidate(BaseModel):
    """Deterministic entity candidate grouping raw mentions by verified canonical identifier."""

    model_config = ConfigDict(frozen=True)

    canonical_identifier: str = Field(
        description="Normalized identifier (e.g. E.164 phone number, lowercased email, or raw unique ID)."
    )
    identifier_type: str = Field(description="PHONE_NUMBER, EMAIL, IMEI, IMSI.")
    raw_mentions: list[str] = Field(
        default_factory=list,
        description="All unique raw string variations observed in source records.",
    )
    equivalence_reasons: list[str] = Field(
        default_factory=list,
        description="Auditable explanations documenting why these mentions were grouped.",
    )
    source_evidence_ids: list[str] = Field(
        default_factory=list,
        description="All origin evidence items where this entity was observed.",
    )
    source_record_ids: list[str] = Field(
        default_factory=list,
        description="All row or message records where this entity was observed.",
    )
    disclaimer: str = Field(
        default=(
            "Normalized identifiers represent technical communication addresses or device "
            "hardware signatures, and do NOT constitute conclusive legal proof of human identity."
        ),
        description="Forensic integrity disclaimer.",
    )


class ExtractionValidationIssue(BaseModel):
    """Details of a parsing, schema, or value validation issue encountered during extraction."""

    model_config = ConfigDict(frozen=True)

    record_id: Optional[str] = Field(
        default=None,
        description="Source row or message identifier where the issue occurred, if available.",
    )
    field_name: str = Field(description="Name of the affected field.")
    issue_type: str = Field(
        description="Classification of issue: MISSING_REQUIRED_FIELD, INVALID_FORMAT, TYPE_ERROR, MALFORMED_ROW, etc."
    )
    raw_value: Optional[str] = Field(
        default=None,
        description="The unparsed/invalid raw string as observed in the source file.",
    )
    message: str = Field(description="Explanatory, non-accusatory diagnostic message.")
    row_index: Optional[int] = Field(
        default=None,
        description="0-indexed or 1-indexed row number in tabular/line-based evidence.",
    )


class ExtractedIdentifier(BaseModel):
    """A discrete identifier (Phone, Email, IMEI, IMSI) extracted from evidence text or structured records."""

    model_config = ConfigDict(frozen=True)

    identifier_type: str = Field(
        description="Type category: PHONE_NUMBER, EMAIL, IMEI, IMSI, AMBIGUOUS_NUMERIC_15."
    )
    raw_value: str = Field(description="Exact substring as observed in the original source evidence.")
    normalized_value: Optional[str] = Field(
        default=None,
        description="Canonicalized form (e.g. E.164 for phone numbers) if unambiguous; otherwise None.",
    )
    source_evidence_id: str = Field(description="Cryptographically bound origin Evidence ID.")
    source_record_id: str = Field(description="Row or message identifier containing this identifier.")
    location_reference: Optional[str] = Field(
        default=None,
        description="Character span (e.g. 'char_offset:42-56') or source column (e.g. 'column:caller_phone').",
    )
    is_ambiguous: bool = Field(
        default=False,
        description="True if format matches multiple categories (e.g. 15-digit number without IMEI/IMSI context).",
    )
    ambiguity_reason: Optional[str] = Field(
        default=None,
        description="Explanation when is_ambiguous is True.",
    )
    context_snippet: Optional[str] = Field(
        default=None,
        description="Surrounding text window preserved for forensic audit.",
    )
    is_checksum_valid: Optional[bool] = Field(
        default=None,
        description=(
            "Luhn algorithm checksum validity for IMEI candidates (True if valid, False if invalid). "
            "Set to None for IMSI, phone numbers, emails, and ambiguous identifiers."
        ),
    )
    extraction_confidence: Optional[float] = Field(
        default=None,
        ge=0.0,
        le=1.0,
        description=(
            "Statistical model or extraction-quality confidence in [0.0, 1.0]. "
            "Set to None for deterministic regex/rule-based matches where no empirical probabilistic "
            "model is used. Completely decoupled from mathematical checksum validity (is_checksum_valid)."
        ),
    )
    extraction_method: str = Field(
        default="regex_pattern",
        description="Extraction method applied (e.g. 'regex_pattern', 'structured_column', 'heuristic').",
    )

    @property
    def confidence(self) -> Optional[float]:
        """Convenience alias for extraction_confidence."""
        return self.extraction_confidence


class ExtractedEventCandidate(BaseModel):
    """Provisional temporal event candidate generated directly from an evidence record."""

    model_config = ConfigDict(frozen=True)

    candidate_id: str = Field(description="Deterministic candidate identifier.")
    source_evidence_id: str = Field(description="Source Evidence ID.")
    source_record_id: str = Field(description="Row or message identifier.")
    event_type: str = Field(description="Event category: TELEPHONY_CALL, MESSAGING_EXCHANGE, etc.")
    raw_timestamp: str = Field(
        description="Original uncorrected timestamp string as recorded in evidence."
    )
    duration_seconds: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Physical activity duration if explicitly recorded (e.g. call duration).",
    )
    location_name: Optional[str] = Field(
        default=None,
        description="Cell tower, GPS tag, or stated location name from source record.",
    )
    involved_entity_raw_ids: list[str] = Field(
        default_factory=list,
        description="Raw phone numbers or usernames directly involved in this event.",
    )
    metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Additional technical attributes extracted from the record.",
    )


class ProvisionalRelationshipCandidate(BaseModel):
    """Directly observed communication or interaction relationship supported by evidence."""

    model_config = ConfigDict(frozen=True)

    candidate_id: str = Field(description="Deterministic candidate identifier.")
    source_evidence_id: str = Field(description="Source Evidence ID backing this relationship.")
    source_record_id: str = Field(description="Originating record/message ID.")
    source_raw_id: str = Field(description="Initiating party raw identifier (e.g. caller, sender).")
    target_raw_id: str = Field(description="Receiving party raw identifier (e.g. callee, recipient).")
    source_normalized_id: Optional[str] = Field(
        default=None,
        description="Resolved canonical identifier for source party, if available.",
    )
    target_normalized_id: Optional[str] = Field(
        default=None,
        description="Resolved canonical identifier for target party, if available.",
    )
    relation_type: str = Field(
        description="Provisional relation type: CALL_COMMUNICATION, MESSAGE_COMMUNICATION."
    )
    is_observed: bool = Field(
        default=True,
        description="True because this candidate is supported by direct observation in the evidence record.",
    )
    metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Observed metadata (e.g. call_duration_seconds, chat_id, call_type).",
    )
    evidence_basis: list[str] = Field(
        default_factory=list,
        description="Evidence ID(s) directly establishing this relationship.",
    )


class ParsedCDRRecord(BaseModel):
    """A validated row extracted from a Call Detail Record (CDR) CSV."""

    model_config = ConfigDict(frozen=True)

    source_evidence_id: str
    record_id: str
    caller_phone_raw: str
    receiver_phone_raw: str
    call_timestamp_raw: str
    duration_seconds: float = Field(ge=0.0)
    cell_tower_id: Optional[str] = None
    call_type: str = "VOICE_CALL"
    row_index: int = Field(ge=1)


class ParsedChatMessage(BaseModel):
    """A validated message extracted from a chat/messaging JSON export."""

    model_config = ConfigDict(frozen=True)

    source_evidence_id: str
    message_id: str
    chat_id: Optional[str] = None
    sender_raw_id: str
    sender_name: Optional[str] = None
    recipients_raw: list[str] = Field(default_factory=list)
    timestamp_raw: str
    content_raw: str


class CDRExtractionResult(BaseModel):
    """Complete structured output from CDR CSV extraction."""

    model_config = ConfigDict(frozen=True)

    source_evidence_id: str
    total_rows_processed: int = Field(ge=0)
    valid_records: list[ParsedCDRRecord] = Field(default_factory=list)
    rejected_records: list[ExtractionValidationIssue] = Field(default_factory=list)
    identifiers: list[ExtractedIdentifier] = Field(default_factory=list)
    event_candidates: list[ExtractedEventCandidate] = Field(default_factory=list)
    relationship_candidates: list[ProvisionalRelationshipCandidate] = Field(default_factory=list)


class ChatExtractionResult(BaseModel):
    """Complete structured output from chat export JSON extraction."""

    model_config = ConfigDict(frozen=True)

    source_evidence_id: str
    chat_id: Optional[str] = None
    total_messages_processed: int = Field(ge=0)
    valid_messages: list[ParsedChatMessage] = Field(default_factory=list)
    rejected_messages: list[ExtractionValidationIssue] = Field(default_factory=list)
    identifiers: list[ExtractedIdentifier] = Field(default_factory=list)
    event_candidates: list[ExtractedEventCandidate] = Field(default_factory=list)
    relationship_candidates: list[ProvisionalRelationshipCandidate] = Field(default_factory=list)


class EvidenceExtractionRequest(BaseModel):
    """Request DTO to dispatch unified evidence extraction across supported modalities."""

    model_config = ConfigDict(frozen=True)

    evidence_id: str = Field(description="Unique identifier of the target evidence item.")
    case_id: str = Field(description="Forensic case identifier.")
    source_type: str = Field(
        description="Evidence modality (e.g. CALL_DETAIL_RECORD, CHAT_EXPORT)."
    )
    file_path: Union[str, Path] = Field(
        description="Trusted internal storage location or filesystem path of the raw evidence artifact."
    )
    expected_sha256: str = Field(
        min_length=64,
        max_length=64,
        description="Cryptographic SHA-256 digest expected for pre-extraction verification.",
    )
    source_timezone: Optional[str] = Field(
        default=None,
        description="Preserved timezone identifier (e.g. 'Asia/Kolkata', 'UTC') if declared.",
    )
    clock_offset_seconds: Optional[float] = Field(
        default=None,
        description="Device or carrier clock drift offset in seconds if known.",
    )
    metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Additional validated ingestion metadata.",
    )
    default_country_prefix: Optional[str] = Field(
        default=None,
        description="Optional default international country prefix (e.g. '91') for phone normalization.",
    )


class UnifiedExtractionResult(BaseModel):
    """Complete, typed result returned by the unified evidence extraction dispatcher."""

    model_config = ConfigDict(frozen=True)

    evidence_id: str = Field(description="Target evidence identifier.")
    case_id: str = Field(description="Forensic case identifier.")
    source_type: str = Field(description="Evidence modality processed.")
    status: ExtractionStatus = Field(description="Outcome status: SUCCESS, PARTIAL_SUCCESS, FAILED.")
    identifiers: list[ExtractedIdentifier] = Field(
        default_factory=list,
        description="Extracted discrete technical identifiers (phones, emails, IMEIs, IMSIs).",
    )
    resolved_entities: list[ResolvedEntityCandidate] = Field(
        default_factory=list,
        description="Deduplicated entity candidates grouped by verified canonical identifier.",
    )
    event_candidates: list[ExtractedEventCandidate] = Field(
        default_factory=list,
        description="Provisional temporal event candidates synthesized from records.",
    )
    relationship_candidates: list[ProvisionalRelationshipCandidate] = Field(
        default_factory=list,
        description="Directly observed communication or interaction relationship candidates.",
    )
    validation_issues: list[ExtractionValidationIssue] = Field(
        default_factory=list,
        description="Structured validation issues, parsing rejections, or integrity failures.",
    )
    total_records_processed: int = Field(
        default=0,
        ge=0,
        description="Total source records or messages inspected.",
    )
    valid_records_count: int = Field(
        default=0,
        ge=0,
        description="Total successfully parsed and validated records.",
    )
    rejected_records_count: int = Field(
        default=0,
        ge=0,
        description="Total rejected or malformed records.",
    )
    processing_time_ms: Optional[float] = Field(
        default=None,
        ge=0.0,
        description="Execution duration in milliseconds.",
    )
    extraction_metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Dispatcher metadata, parser name/version, and configuration details.",
    )
