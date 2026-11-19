"""Deterministic Entity & Identifier Resolution Module (Member 1).

Implements deterministic phone number canonicalization (E.164-style), audit-safe
equivalence tracking, and provenance-preserving entity candidate deduplication.
"""

from __future__ import annotations

import re
from typing import Optional

from pydantic import BaseModel, ConfigDict, Field

from app.schemas.extraction import ExtractedIdentifier, ResolvedEntityCandidate


class PhoneNormalizationResult(BaseModel):
    """Result of attempting to normalize a phone number string into standard E.164 format."""

    model_config = ConfigDict(frozen=True)

    raw_input: str
    normalized_e164: Optional[str] = None
    is_normalized: bool = False
    resolution_status: str
    reason: str
    country_prefix_applied: Optional[str] = None


def normalize_phone_number(
    raw_phone: str,
    *,
    default_country_prefix: Optional[str] = None,
) -> PhoneNormalizationResult:
    """Deterministically normalize a phone string into E.164 (+<country_code><national_number>).

    Forensic rules:
    - Never guess a country code if not explicit, unless default_country_prefix is configured.
    - Preserves raw input exactly.
    - Explicit international prefix (+) is accepted directly.
    - International trunk '00' is replaced with '+'.
    - Domestic leading trunk '0' is stripped only when default_country_prefix is provided.
    """
    if not raw_phone or not isinstance(raw_phone, str):
        return PhoneNormalizationResult(
            raw_input=str(raw_phone),
            is_normalized=False,
            resolution_status="EMPTY_OR_NON_STRING",
            reason="Input phone number was empty or not a string.",
        )

    cleaned = raw_phone.strip()
    # Check for leading '+'
    has_plus = cleaned.startswith("+")

    # Extract all digits
    digits = re.sub(r"\D", "", cleaned)

    if len(digits) < 7:
        return PhoneNormalizationResult(
            raw_input=cleaned,
            is_normalized=False,
            resolution_status="INSUFFICIENT_DIGITS",
            reason=f"Phone number contains only {len(digits)} digits; minimum valid length is 7.",
        )
    if len(digits) > 15:
        return PhoneNormalizationResult(
            raw_input=cleaned,
            is_normalized=False,
            resolution_status="EXCESSIVE_DIGITS",
            reason=f"Phone number contains {len(digits)} digits; maximum ITU-T E.164 length is 15.",
        )

    # Case 1: Explicit leading '+'
    if has_plus:
        norm = f"+{digits}"
        return PhoneNormalizationResult(
            raw_input=cleaned,
            normalized_e164=norm,
            is_normalized=True,
            resolution_status="EXPLICIT_E164_PLUS",
            reason="Normalized using explicit leading '+' international country code indicator.",
        )

    # Case 2: International trunk '00'
    if cleaned.startswith("00") and len(digits) >= 9:
        norm = f"+{digits[2:]}"
        return PhoneNormalizationResult(
            raw_input=cleaned,
            normalized_e164=norm,
            is_normalized=True,
            resolution_status="INTERNATIONAL_TRUNK_00",
            reason="Normalized by replacing leading international trunk prefix '00' with '+'.",
        )

    # Case 3: Domestic number requiring default country prefix
    if default_country_prefix:
        clean_prefix = re.sub(r"\D", "", default_country_prefix)
        if not clean_prefix:
            return PhoneNormalizationResult(
                raw_input=cleaned,
                is_normalized=False,
                resolution_status="INVALID_DEFAULT_PREFIX",
                reason=f"Provided default_country_prefix '{default_country_prefix}' contains no digits.",
            )

        # If starts with leading domestic trunk '0' (e.g. 09811002233)
        national_digits = digits[1:] if digits.startswith("0") else digits

        # Standard 10-digit national number check for common test environments
        if len(national_digits) == 10:
            norm = f"+{clean_prefix}{national_digits}"
            return PhoneNormalizationResult(
                raw_input=cleaned,
                normalized_e164=norm,
                is_normalized=True,
                resolution_status="CONFIGURED_DEFAULT_COUNTRY_PREFIX",
                reason=(
                    f"Domestic number normalized to '+{clean_prefix}' using configured default country "
                    f"prefix assumption ({'stripped domestic trunk 0' if digits.startswith('0') else '10-digit national number'})."
                ),
                country_prefix_applied=clean_prefix,
            )

    # Case 4: Ambiguous without explicit country code and no default prefix
    return PhoneNormalizationResult(
        raw_input=cleaned,
        is_normalized=False,
        resolution_status="AMBIGUOUS_COUNTRY_CODE",
        reason=(
            "Number lacks explicit international indicator ('+' or '00') and no default country "
            "code was configured. Not normalized to prevent erroneous cross-jurisdiction entity merges."
        ),
    )


def deduplicate_identifiers(
    identifiers: list[ExtractedIdentifier],
    *,
    default_country_prefix: Optional[str] = None,
) -> list[ResolvedEntityCandidate]:
    """Deterministically group and deduplicate extracted identifiers.

    Phone numbers are grouped by normalized E.164 when resolvable.
    Emails are grouped case-insensitively.
    IMEI and IMSIs are grouped by exact numeric sequence.
    Ambiguous identifiers remain isolated under their raw strings.
    """
    grouped: dict[str, dict] = {}

    for ident in identifiers:
        c_id: str
        c_type = ident.identifier_type
        reason: str

        if c_type == "PHONE_NUMBER":
            res = normalize_phone_number(ident.raw_value, default_country_prefix=default_country_prefix)
            if res.is_normalized and res.normalized_e164:
                c_id = res.normalized_e164
                reason = res.reason
            else:
                c_id = ident.raw_value.strip()
                reason = f"Unnormalized raw phone: {res.reason}"
        elif c_type == "EMAIL":
            c_id = ident.raw_value.strip().lower()
            reason = "Case-insensitive exact email match"
        elif c_type in {"IMEI", "IMSI"}:
            c_id = ident.raw_value.strip()
            reason = f"Exact {c_type} hardware/subscriber identifier match"
        else:
            c_id = f"{c_type}:{ident.raw_value.strip()}"
            reason = ident.ambiguity_reason or "Ambiguous raw mention kept isolated"

        if c_id not in grouped:
            grouped[c_id] = {
                "canonical_identifier": c_id,
                "identifier_type": c_type,
                "raw_mentions": set(),
                "equivalence_reasons": set(),
                "source_evidence_ids": set(),
                "source_record_ids": set(),
            }

        grouped[c_id]["raw_mentions"].add(ident.raw_value)
        grouped[c_id]["equivalence_reasons"].add(reason)
        grouped[c_id]["source_evidence_ids"].add(ident.source_evidence_id)
        grouped[c_id]["source_record_ids"].add(ident.source_record_id)

    candidates: list[ResolvedEntityCandidate] = []
    # Deterministic sorting by canonical identifier
    for c_id in sorted(grouped.keys()):
        data = grouped[c_id]
        candidates.append(
            ResolvedEntityCandidate(
                canonical_identifier=c_id,
                identifier_type=data["identifier_type"],
                raw_mentions=sorted(data["raw_mentions"]),
                equivalence_reasons=sorted(data["equivalence_reasons"]),
                source_evidence_ids=sorted(data["source_evidence_ids"]),
                source_record_ids=sorted(data["source_record_ids"]),
            )
        )

    return candidates


__all__ = [
    "PhoneNormalizationResult",
    "ResolvedEntityCandidate",
    "normalize_phone_number",
    "deduplicate_identifiers",
]
