"""Unit tests for entity resolution and phone number canonicalization."""

import pytest
from app.modules.correlation.resolution import (
    deduplicate_identifiers,
    normalize_phone_number,
)
from app.schemas.extraction import ExtractedIdentifier


def test_explicit_international_e164_normalization():
    """Verify normalization of international numbers with leading '+'."""
    res1 = normalize_phone_number("+919811002233")
    assert res1.is_normalized is True
    assert res1.normalized_e164 == "+919811002233"
    assert res1.resolution_status == "EXPLICIT_E164_PLUS"

    # With formatting spaces and dashes
    res2 = normalize_phone_number("+91 98110-02233")
    assert res2.is_normalized is True
    assert res2.normalized_e164 == "+919811002233"


def test_international_trunk_00_normalization():
    """Verify normalization replacing trunk '00' with '+'."""
    res = normalize_phone_number("00919811002233")
    assert res.is_normalized is True
    assert res.normalized_e164 == "+919811002233"
    assert res.resolution_status == "INTERNATIONAL_TRUNK_00"


def test_domestic_number_with_configured_default_prefix():
    """Verify normalization of domestic numbers when a default country prefix is configured."""
    # Domestic trunk 0 (e.g. 09811002233)
    res1 = normalize_phone_number("09811002233", default_country_prefix="91")
    assert res1.is_normalized is True
    assert res1.normalized_e164 == "+919811002233"
    assert res1.country_prefix_applied == "91"

    # 10-digit national number
    res2 = normalize_phone_number("9811002233", default_country_prefix="91")
    assert res2.is_normalized is True
    assert res2.normalized_e164 == "+919811002233"


def test_domestic_number_without_default_prefix_remains_unresolved():
    """Verify that ambiguous domestic numbers are NOT normalized when no default prefix is provided."""
    res = normalize_phone_number("09811002233", default_country_prefix=None)
    assert res.is_normalized is False
    assert res.normalized_e164 is None
    assert res.resolution_status == "AMBIGUOUS_COUNTRY_CODE"


def test_invalid_and_out_of_bounds_phone_numbers():
    """Verify rejection of invalid phone numbers."""
    # Too short (< 7 digits)
    res_short = normalize_phone_number("12345")
    assert res_short.is_normalized is False
    assert res_short.resolution_status == "INSUFFICIENT_DIGITS"

    # Too long (> 15 digits)
    res_long = normalize_phone_number("+911234567890123456")
    assert res_long.is_normalized is False
    assert res_long.resolution_status == "EXCESSIVE_DIGITS"

    # Empty
    res_empty = normalize_phone_number("")
    assert res_empty.is_normalized is False


def test_deterministic_deduplication():
    """Verify grouping varied raw mentions of the same number into a single candidate."""
    idents = [
        ExtractedIdentifier(
            identifier_type="PHONE_NUMBER",
            raw_value="+919811002233",
            source_evidence_id="ev-01",
            source_record_id="cdr-01",
        ),
        ExtractedIdentifier(
            identifier_type="PHONE_NUMBER",
            raw_value="09811002233",
            source_evidence_id="ev-02",
            source_record_id="chat-01",
        ),
        ExtractedIdentifier(
            identifier_type="PHONE_NUMBER",
            raw_value="+91 98110 02233",
            source_evidence_id="ev-02",
            source_record_id="chat-03",
        ),
        ExtractedIdentifier(
            identifier_type="PHONE_NUMBER",
            raw_value="+919899887766",
            source_evidence_id="ev-01",
            source_record_id="cdr-02",
        ),
        ExtractedIdentifier(
            identifier_type="EMAIL",
            raw_value="Alpha.Lead@Synthetic-Forensics.Internal",
            source_evidence_id="ev-02",
            source_record_id="chat-01",
        ),
    ]

    candidates = deduplicate_identifiers(idents, default_country_prefix="91")

    # Should produce 3 resolved candidates:
    # 1. +919811002233 (merged 3 mentions)
    # 2. +919899887766 (1 mention)
    # 3. alpha.lead@synthetic-forensics.internal (1 mention)
    assert len(candidates) == 3

    target_cand = next(c for c in candidates if c.canonical_identifier == "+919811002233")
    assert len(target_cand.raw_mentions) == 3
    assert "+919811002233" in target_cand.raw_mentions
    assert "09811002233" in target_cand.raw_mentions
    assert "+91 98110 02233" in target_cand.raw_mentions

    assert len(target_cand.source_evidence_ids) == 2  # ev-01 and ev-02
    assert "ev-01" in target_cand.source_evidence_ids
    assert "ev-02" in target_cand.source_evidence_ids

    # Verify forensic disclaimer is present
    assert "NOT constitute conclusive legal proof of human identity" in target_cand.disclaimer
