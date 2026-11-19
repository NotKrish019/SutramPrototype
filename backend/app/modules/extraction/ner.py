"""Deterministic Pattern-Based Identifier Extraction (Member 1).

Extracts and validates phone numbers, email addresses, IMEIs, and IMSIs from
unstructured text and structured field values. Preserves exact character offsets,
raw matches, source provenance, and flags ambiguous numeric sequences.
"""

from __future__ import annotations

import re
from typing import Optional

from app.schemas.extraction import ExtractedIdentifier

# Regex Patterns
EMAIL_PATTERN = re.compile(
    r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"
)

# International and common domestic phone number pattern with separators
PHONE_PATTERN = re.compile(
    r"(?:(?:\+|00)[1-9]\d{0,3}[\s.-]?)?(?:(?:\(\d{1,4}\)|\d{1,4})[\s.-]?)?\d{3,4}[\s.-]?\d{3,4}\b"
)

# 14 to 15-digit standalone numeric sequences (candidate IMEI/IMSI)
NUMERIC_14_15_PATTERN = re.compile(r"\b\d{14,15}\b")

# Contextual keyword indicators for IMEI and IMSI
IMEI_KEYWORDS = re.compile(r"\b(?:imei|device|handset|terminal|hardware|tac)\b", re.IGNORECASE)
IMSI_KEYWORDS = re.compile(r"\b(?:imsi|sim|subscriber|iccid|telecom)\b", re.IGNORECASE)


def luhn_checksum_valid(digits: str) -> bool:
    """Validate 15-digit Luhn checksum (standard for GSM/3GPP IMEI)."""
    if not digits.isdigit() or len(digits) != 15:
        return False
    total = 0
    for idx, char in enumerate(reversed(digits)):
        val = int(char)
        if idx % 2 == 1:
            doubled = val * 2
            total += doubled - 9 if doubled > 9 else doubled
        else:
            total += val
    return total % 10 == 0


def _extract_context_window(text: str, start: int, end: int, window_size: int = 40) -> str:
    """Extract a small snippet of surrounding text for forensic audit and contextual validation."""
    ctx_start = max(0, start - window_size)
    ctx_end = min(len(text), end + window_size)
    return text[ctx_start:ctx_end].strip()


def extract_identifiers_from_text(
    text: str,
    source_evidence_id: str,
    source_record_id: str,
    *,
    default_country_code: Optional[str] = None,
) -> list[ExtractedIdentifier]:
    """Deterministically extract email addresses, phone numbers, IMEIs, and IMSIs from text.

    Preserves character offsets, surrounding context, and marks ambiguous numeric strings.
    """
    if not text or not isinstance(text, str):
        return []

    identifiers: list[ExtractedIdentifier] = []
    consumed_spans: set[tuple[int, int]] = set()

    # 1. Email Extraction
    for match in EMAIL_PATTERN.finditer(text):
        start, end = match.span()
        raw_val = match.group(0).strip()
        consumed_spans.add((start, end))
        identifiers.append(
            ExtractedIdentifier(
                identifier_type="EMAIL",
                raw_value=raw_val,
                normalized_value=raw_val.lower(),
                source_evidence_id=source_evidence_id,
                source_record_id=source_record_id,
                location_reference=f"char_offset:{start}-{end}",
                context_snippet=_extract_context_window(text, start, end),
                extraction_method="regex_email",
            )
        )

    # 2. 14-15 Digit Numeric Sequences (IMEI / IMSI / Ambiguous)
    for match in NUMERIC_14_15_PATTERN.finditer(text):
        start, end = match.span()
        raw_val = match.group(0).strip()
        consumed_spans.add((start, end))

        # Check immediate preceding context (e.g., "IMEI: ", "Device IMEI ")
        pre_ctx = text[max(0, start - 30):start]
        post_ctx = text[end:min(len(text), end + 30)]
        context_window = _extract_context_window(text, start, end, window_size=40)

        has_pre_imei = bool(IMEI_KEYWORDS.search(pre_ctx))
        has_pre_imsi = bool(IMSI_KEYWORDS.search(pre_ctx))
        has_post_imei = bool(IMEI_KEYWORDS.search(post_ctx))
        has_post_imsi = bool(IMSI_KEYWORDS.search(post_ctx))

        if has_pre_imei and not has_pre_imsi:
            identifiers.append(
                ExtractedIdentifier(
                    identifier_type="IMEI",
                    raw_value=raw_val,
                    normalized_value=raw_val,
                    is_checksum_valid=luhn_checksum_valid(raw_val),
                    source_evidence_id=source_evidence_id,
                    source_record_id=source_record_id,
                    location_reference=f"char_offset:{start}-{end}",
                    context_snippet=context_window,
                    extraction_method="regex_imei_contextual",
                )
            )
        elif has_pre_imsi and not has_pre_imei:
            identifiers.append(
                ExtractedIdentifier(
                    identifier_type="IMSI",
                    raw_value=raw_val,
                    normalized_value=raw_val,
                    is_checksum_valid=None,
                    source_evidence_id=source_evidence_id,
                    source_record_id=source_record_id,
                    location_reference=f"char_offset:{start}-{end}",
                    context_snippet=context_window,
                    extraction_method="regex_imsi_contextual",
                )
            )
        elif has_post_imei and not has_post_imsi:
            identifiers.append(
                ExtractedIdentifier(
                    identifier_type="IMEI",
                    raw_value=raw_val,
                    normalized_value=raw_val,
                    is_checksum_valid=luhn_checksum_valid(raw_val),
                    source_evidence_id=source_evidence_id,
                    source_record_id=source_record_id,
                    location_reference=f"char_offset:{start}-{end}",
                    context_snippet=context_window,
                    extraction_method="regex_imei_contextual",
                )
            )
        elif has_post_imsi and not has_post_imei:
            identifiers.append(
                ExtractedIdentifier(
                    identifier_type="IMSI",
                    raw_value=raw_val,
                    normalized_value=raw_val,
                    is_checksum_valid=None,
                    source_evidence_id=source_evidence_id,
                    source_record_id=source_record_id,
                    location_reference=f"char_offset:{start}-{end}",
                    context_snippet=context_window,
                    extraction_method="regex_imsi_contextual",
                )
            )
        else:
            # Ambiguous: No keyword or conflicting keywords in immediate proximity
            reason = (
                "Conflicting IMEI and IMSI keywords in immediate context"
                if ((has_pre_imei or has_post_imei) and (has_pre_imsi or has_post_imsi))
                else "Isolated 14/15-digit numeric sequence lacking explicit IMEI or IMSI contextual keywords"
            )
            identifiers.append(
                ExtractedIdentifier(
                    identifier_type="AMBIGUOUS_NUMERIC_15",
                    raw_value=raw_val,
                    normalized_value=None,
                    is_checksum_valid=None,
                    source_evidence_id=source_evidence_id,
                    source_record_id=source_record_id,
                    location_reference=f"char_offset:{start}-{end}",
                    is_ambiguous=True,
                    ambiguity_reason=reason,
                    context_snippet=context_window,
                    extraction_method="regex_numeric_15",
                )
            )

    # 3. Phone Number Extraction
    for match in PHONE_PATTERN.finditer(text):
        start, end = match.span()
        # Avoid overlapping with email domains or previously consumed 15-digit spans
        if any(cs[0] <= start < cs[1] or cs[0] < end <= cs[1] for cs in consumed_spans):
            continue

        raw_val = match.group(0).strip()
        # Strip trailing punctuation sometimes caught by boundary
        clean_raw = raw_val.rstrip(".,;!?)]")
        digits_only = re.sub(r"\D", "", clean_raw)

        # Minimum phone length 7 digits, max 15 digits
        if len(digits_only) < 7 or len(digits_only) > 15:
            continue

        identifiers.append(
            ExtractedIdentifier(
                identifier_type="PHONE_NUMBER",
                raw_value=clean_raw,
                normalized_value=None,  # Normalized via resolution module
                source_evidence_id=source_evidence_id,
                source_record_id=source_record_id,
                location_reference=f"char_offset:{start}-{start+len(clean_raw)}",
                context_snippet=_extract_context_window(text, start, start + len(clean_raw)),
                extraction_method="regex_phone",
            )
        )

    return identifiers
