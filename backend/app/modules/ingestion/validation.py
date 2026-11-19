"""File Content & Format Validation Module (Member 2).

Forensically inspects magic bytes and file structures to verify true content
type rather than naively trusting filename extensions or client MIME headers.
"""

from __future__ import annotations

import csv
import io
import json
from pathlib import Path
from typing import Optional, Tuple


SUPPORTED_MIME_TYPES = {
    "text/csv": "CALL_DETAIL_RECORD",
    "application/json": "CHAT_EXPORT",
    "application/pdf": "DOCUMENT_PDF",
    "image/png": "FORENSIC_IMAGE",
    "image/jpeg": "FORENSIC_IMAGE",
    "video/mp4": "CCTV_VIDEO",
    "video/quicktime": "CCTV_VIDEO",
    "text/plain": "DOCUMENT_TEXT",
}


def sniff_file_content(
    file_path: Path | str,
    declared_filename: str,
) -> Tuple[bool, str, str, str]:
    """Inspect file header bytes and structure to verify content fidelity.
    
    Returns:
        Tuple[bool, str, str, str]: (is_valid, detected_mime, source_type, diagnostic_reason)
    """
    path = Path(file_path).resolve()
    if not path.is_file():
        return False, "application/octet-stream", "UNKNOWN", "File does not exist on disk."

    size = path.stat().st_size
    if size == 0:
        return False, "application/octet-stream", "UNKNOWN", "Uploaded evidence file is empty (0 bytes)."

    # Read first 4096 bytes for magic header and structure analysis
    with open(path, "rb") as f:
        header = f.read(4096)

    ext = Path(declared_filename).suffix.lower()

    # 1. PDF Magic Bytes: '%PDF-'
    if header.startswith(b"%PDF-"):
        return True, "application/pdf", "DOCUMENT_PDF", "Valid PDF document magic bytes verified."

    # 2. PNG Magic Bytes: '\x89PNG\r\n\x1a\n'
    if header.startswith(b"\x89PNG\r\n\x1a\n"):
        return True, "image/png", "FORENSIC_IMAGE", "Valid PNG image magic bytes verified."

    # 3. JPEG Magic Bytes: '\xff\xd8\xff'
    if header.startswith(b"\xff\xd8\xff"):
        return True, "image/jpeg", "FORENSIC_IMAGE", "Valid JPEG image magic bytes verified."

    # 4. MP4 / MOV Video: 'ftyp' box in first 32 bytes
    if len(header) >= 12 and b"ftyp" in header[:32]:
        return True, "video/mp4", "CCTV_VIDEO", "Valid ISO Base Media / MP4 ftyp box verified."

    # 5. JSON Parsing Structure
    if ext == ".json" or header.lstrip().startswith((b"{", b"[")):
        try:
            with open(path, "r", encoding="utf-8") as f_json:
                data = json.load(f_json)
                if isinstance(data, (dict, list)):
                    return True, "application/json", "CHAT_EXPORT", "Valid JSON structure verified."
        except UnicodeDecodeError:
            return False, "application/octet-stream", "UNKNOWN", "File declared as JSON contains non-UTF8 binary data."
        except json.JSONDecodeError as exc:
            return False, "application/json", "UNKNOWN", f"Malformed JSON content: {exc.msg} at line {exc.lineno}."

    # 6. CSV Parsing Structure
    if ext == ".csv" or ext in {".txt", ".tsv"}:
        try:
            with open(path, "r", encoding="utf-8", errors="replace") as f_txt:
                sample = f_txt.read(4096)
                if not sample.strip():
                    return False, "text/plain", "UNKNOWN", "Text evidence file contains only whitespace."
                
                # Verify CSV delimiter consistency
                sniffer = csv.Sniffer()
                try:
                    dialect = sniffer.sniff(sample, delimiters=",;\t|")
                    # Check that header row exists
                    f_txt.seek(0)
                    reader = csv.reader(f_txt, dialect)
                    first_row = next(reader, None)
                    if first_row and len(first_row) >= 2:
                        return True, "text/csv", "CALL_DETAIL_RECORD", f"Valid delimited text verified (delimiter='{dialect.delimiter}')."
                except csv.Error:
                    # If sniffer fails, check simple comma separation
                    if "," in sample and "\n" in sample:
                        return True, "text/csv", "CALL_DETAIL_RECORD", "Comma-separated text structure verified."
                    return True, "text/plain", "DOCUMENT_TEXT", "Plain text evidence format verified."
        except Exception as exc:
            return False, "application/octet-stream", "UNKNOWN", f"Text decoding error: {str(exc)}"

    return False, "application/octet-stream", "UNKNOWN", f"Unsupported or unrecognized file signature for '{declared_filename}'."
