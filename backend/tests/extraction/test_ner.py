"""Unit tests for deterministic pattern-based identifier extraction (NER)."""

import pytest
from app.modules.extraction.ner import (
    extract_identifiers_from_text,
    luhn_checksum_valid,
)


def test_email_extraction():
    """Verify regex email extraction and case normalization."""
    text = "Send reports to forensic.analyst@cybercell.gov.in and lead.detective@agency.org."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-01")

    emails = [i for i in idents if i.identifier_type == "EMAIL"]
    assert len(emails) == 2
    assert emails[0].raw_value == "forensic.analyst@cybercell.gov.in"
    assert emails[0].normalized_value == "forensic.analyst@cybercell.gov.in"
    assert "char_offset:16-49" in (emails[0].location_reference or "")
    assert emails[1].raw_value == "lead.detective@agency.org"


def test_phone_number_extraction():
    """Verify extraction of international and domestic phone numbers from text."""
    text = "Call target at +919811002233 or local line 09899887766 immediately."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-02")

    phones = [i for i in idents if i.identifier_type == "PHONE_NUMBER"]
    assert len(phones) == 2
    raw_phones = [p.raw_value for p in phones]
    assert "+919811002233" in raw_phones
    assert "09899887766" in raw_phones


def test_contextual_imei_and_imsi_extraction():
    """Verify that 15-digit numbers are classified as IMEI or IMSI when contextual keywords exist."""
    text = "Seized handset IMEI: 861234567890123. The carrier SIM IMSI is 404450123456789."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-03")

    imeis = [i for i in idents if i.identifier_type == "IMEI"]
    imsis = [i for i in idents if i.identifier_type == "IMSI"]

    assert len(imeis) == 1
    assert imeis[0].raw_value == "861234567890123"
    assert imeis[0].is_ambiguous is False

    assert len(imsis) == 1
    assert imsis[0].raw_value == "404450123456789"
    assert imsis[0].is_ambiguous is False


def test_isolated_15_digit_number_reported_as_ambiguous():
    """Verify that an arbitrary 15-digit number without context is marked ambiguous, NOT guessed."""
    text = "Reference code 998877665544332 noted in ledger."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-04")

    ambiguous = [i for i in idents if i.identifier_type == "AMBIGUOUS_NUMERIC_15"]
    assert len(ambiguous) == 1
    assert ambiguous[0].raw_value == "998877665544332"
    assert ambiguous[0].is_ambiguous is True
    assert "Isolated 14/15-digit" in (ambiguous[0].ambiguity_reason or "")
    assert ambiguous[0].normalized_value is None


def test_conflicting_context_reported_as_ambiguous():
    """Verify that conflicting keywords in proximity do not cause arbitrary classification."""
    text = "Device IMEI or SIM IMSI: 861234567890123."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-05")

    ambiguous = [i for i in idents if i.identifier_type == "AMBIGUOUS_NUMERIC_15"]
    assert len(ambiguous) == 1
    assert ambiguous[0].is_ambiguous is True
    assert "Conflicting" in (ambiguous[0].ambiguity_reason or "")


def test_valid_imei_checksum_telemetry():
    """Verify that a valid IMEI has is_checksum_valid=True."""
    text = "Seized handset device IMEI: 352099001761481."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-imei-valid")

    imeis = [i for i in idents if i.identifier_type == "IMEI"]
    assert len(imeis) == 1
    assert imeis[0].raw_value == "352099001761481"
    assert imeis[0].is_checksum_valid is True
    assert imeis[0].is_ambiguous is False


def test_invalid_imei_checksum_preserved():
    """Verify that an IMEI with an invalid Luhn checksum is preserved with is_checksum_valid=False."""
    text = "Handset terminal IMEI 352099001761482 recovered from scene."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-imei-invalid")

    imeis = [i for i in idents if i.identifier_type == "IMEI"]
    assert len(imeis) == 1
    # Raw value is preserved intact; not rejected or discarded
    assert imeis[0].raw_value == "352099001761482"
    assert imeis[0].is_checksum_valid is False
    assert imeis[0].is_ambiguous is False


def test_imsi_does_not_apply_luhn_checksum():
    """Verify that IMSI candidates have is_checksum_valid=None (Luhn not applied to IMSI)."""
    text = "Carrier SIM IMSI is 404450123456789."
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-01", source_record_id="rec-imsi")

    imsis = [i for i in idents if i.identifier_type == "IMSI"]
    assert len(imsis) == 1
    assert imsis[0].raw_value == "404450123456789"
    assert imsis[0].is_checksum_valid is None
    assert imsis[0].is_ambiguous is False


def test_multiple_identifiers_in_single_message():
    """Verify extraction and checksum telemetry when multiple identifiers exist in one message."""
    text = (
        "Contact investigator at +919811002233 or email lead@agency.gov. "
        "Primary device IMEI: 352099001761481. Backup handset IMEI: 990000862471854. "
        "SIM card IMSI: 404450123456789. Also note unlabelled tag 123456789012345."
    )
    idents = extract_identifiers_from_text(text, source_evidence_id="ev-multi", source_record_id="msg-multi-01")

    types = [i.identifier_type for i in idents]
    assert "PHONE_NUMBER" in types
    assert "EMAIL" in types
    assert "IMEI" in types
    assert "IMSI" in types
    assert "AMBIGUOUS_NUMERIC_15" in types

    imeis = [i for i in idents if i.identifier_type == "IMEI"]
    assert len(imeis) == 2
    # First IMEI is valid Luhn (352099001761481)
    assert imeis[0].raw_value == "352099001761481"
    assert imeis[0].is_checksum_valid is True

    # IMSI candidate has is_checksum_valid=None
    imsi = next(i for i in idents if i.identifier_type == "IMSI")
    assert imsi.is_checksum_valid is None

    # Ambiguous candidate has is_checksum_valid=None
    ambiguous = next(i for i in idents if i.identifier_type == "AMBIGUOUS_NUMERIC_15")
    assert ambiguous.raw_value == "123456789012345"
    assert ambiguous.is_checksum_valid is None
    assert ambiguous.is_ambiguous is True


def test_luhn_checksum_validator():
    """Verify Luhn algorithm validation logic."""
    # Standard valid 15-digit IMEI example
    assert luhn_checksum_valid("352099001761481") is True
    # Invalid check digit
    assert luhn_checksum_valid("352099001761482") is False
    # Non-digit / wrong length
    assert luhn_checksum_valid("not-an-imei") is False
    assert luhn_checksum_valid("12345") is False


def test_checksum_validity_does_not_affect_extraction_confidence():
    """Verify that changing checksum validity does not alter extraction confidence."""
    text_valid = "Device IMEI: 352099001761481"
    text_invalid = "Device IMEI: 352099001761482"

    idents_valid = extract_identifiers_from_text(
        text_valid, source_evidence_id="ev-01", source_record_id="rec-valid"
    )
    idents_invalid = extract_identifiers_from_text(
        text_invalid, source_evidence_id="ev-01", source_record_id="rec-invalid"
    )

    imei_valid = idents_valid[0]
    imei_invalid = idents_invalid[0]

    # Checksum telemetry reflects mathematical reality
    assert imei_valid.is_checksum_valid is True
    assert imei_invalid.is_checksum_valid is False

    # Extraction confidence is completely decoupled: regex matching does not fabricate
    # an arbitrary probabilistic confidence score based on checksum results.
    assert imei_valid.extraction_confidence is None
    assert imei_invalid.extraction_confidence is None
    assert imei_valid.extraction_confidence == imei_invalid.extraction_confidence
    assert imei_valid.confidence is None
    assert imei_invalid.confidence is None


def test_extracted_identifier_confidence_decoupling():
    """Verify ExtractedIdentifier model maintains independent telemetry and confidence."""
    from app.schemas.extraction import ExtractedIdentifier

    # Model supports decoupled statistical confidence even when checksum is invalid
    ident = ExtractedIdentifier(
        identifier_type="IMEI",
        raw_value="352099001761482",
        normalized_value="352099001761482",
        source_evidence_id="ev-01",
        source_record_id="rec-01",
        is_checksum_valid=False,
        extraction_confidence=0.92,
        extraction_method="spacy_ner_model",
    )
    assert ident.is_checksum_valid is False
    assert ident.extraction_confidence == 0.92
    assert ident.confidence == 0.92

