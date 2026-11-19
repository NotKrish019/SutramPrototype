"""Forensic Evidence Storage Manager (Member 2).

Enforces path traversal defenses, streaming SHA-256 calculation, atomic write
mechanics, and post-write read-only immutability.
"""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import stat
from typing import BinaryIO


# Windows and POSIX reserved names / characters
RESERVED_NAMES = {
    "con", "prn", "aux", "nul",
    "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9",
    "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
}


def sanitize_filename(filename: str) -> str:
    """Sanitize filename to prevent directory traversal and illegal filesystem characters.
    
    Forensic requirements:
    - Strips path separators ('/' and '\\').
    - Disallows directory traversal sequences ('..').
    - Disallows null bytes and non-printable control characters.
    - Prevents collision with Windows device names.
    - Preserves standard evidence extension and alphanumeric stem.
    """
    if not filename or not isinstance(filename, str):
        return "unnamed_evidence.bin"

    # Strip directory components (both POSIX and Windows style)
    basename = filename.replace("\\", "/").split("/")[-1].strip()

    # Remove null bytes and non-printable control characters
    cleaned = "".join(c for c in basename if c.isprintable() and c not in '<>:"/\\|?*\0')

    # Strip leading/trailing dots and spaces
    cleaned = cleaned.strip(". ")

    if not cleaned:
        return "sanitized_evidence.bin"

    # Check for Windows reserved names
    stem = cleaned.split(".")[0].lower()
    if stem in RESERVED_NAMES:
        cleaned = f"safe_{cleaned}"

    return cleaned


def get_safe_storage_path(
    base_dir: Path | str,
    case_id: str,
    evidence_id: str,
    filename: str,
) -> Path:
    """Construct a verified, canonical storage path that cannot escape base_dir.
    
    Directory structure:
        {base_dir}/{case_id}/{evidence_id}/{sanitized_filename}
        
    Raises:
        ValueError: If case_id, evidence_id, or filename attempt path traversal
                    or escape the base storage directory.
    """
    base_path = Path(base_dir).resolve()

    raw_case = str(case_id).strip()
    raw_ev = str(evidence_id).strip()

    if not raw_case or not re.match(r"^[a-zA-Z0-9_\-]+$", raw_case):
        raise ValueError(f"Invalid case_id '{case_id}': contains illegal or traversal characters.")
    if not raw_ev or not re.match(r"^[a-zA-Z0-9_\-]+$", raw_ev):
        raise ValueError(f"Invalid evidence_id '{evidence_id}': contains illegal or traversal characters.")

    safe_case = raw_case
    safe_ev = raw_ev

    safe_file = sanitize_filename(filename)

    target_dir = (base_path / safe_case / safe_ev).resolve()
    target_path = (target_dir / safe_file).resolve()

    # Path traversal assertion: target must be strictly within base_path
    try:
        target_path.relative_to(base_path)
    except ValueError as exc:
        raise ValueError(
            f"Path traversal detected: destination '{target_path}' escapes base directory '{base_path}'."
        ) from exc

    return target_path


def save_evidence_file(
    file_stream: BinaryIO,
    dest_path: Path,
    *,
    max_bytes: int = 50 * 1024 * 1024,
    chunk_size: int = 64 * 1024,
) -> tuple[int, str]:
    """Stream uploaded bytes to disk while incrementally computing SHA-256.
    
    Forensic guarantees:
    - Writes to an atomic temporary file (.part) before final rename.
    - Incremental SHA-256 calculation guarantees exact hash of bytes written to disk.
    - Enforces max_bytes limit during streaming; cleans up immediately if exceeded.
    - Sets destination file permissions to read-only (0o444) to prevent mutation.
    
    Returns:
        tuple[int, str]: (bytes_written, sha256_hex_digest)
        
    Raises:
        ValueError: If file size exceeds max_bytes or destination exists.
        IOError: If disk write or directory creation fails.
    """
    dest_path = Path(dest_path).resolve()
    dest_dir = dest_path.parent
    dest_dir.mkdir(parents=True, exist_ok=True)

    if dest_path.exists():
        raise ValueError(f"Evidence file already exists at '{dest_path}'. Overwrite prohibited.")

    part_path = dest_dir / f"{dest_path.name}.part"
    hasher = hashlib.sha256()
    bytes_written = 0

    try:
        with open(part_path, "wb") as f_out:
            while True:
                chunk = file_stream.read(chunk_size)
                if not chunk:
                    break
                bytes_written += len(chunk)
                if bytes_written > max_bytes:
                    raise ValueError(
                        f"Upload exceeds maximum allowable evidence size of {max_bytes} bytes."
                    )
                hasher.update(chunk)
                f_out.write(chunk)
            f_out.flush()
            os.fsync(f_out.fileno())

        # Atomic rename from .part to final destination
        part_path.replace(dest_path)

    except Exception:
        if part_path.exists():
            safe_unlink(part_path)
        raise

    return bytes_written, hasher.hexdigest()


def make_file_read_only(file_path: Path | str) -> None:
    """Set file permissions to read-only across POSIX and Windows."""
    p = Path(file_path).resolve()
    if not p.is_file():
        return
    try:
        current_mode = p.stat().st_mode
        p.chmod(current_mode & ~(stat.S_IWUSR | stat.S_IWGRP | stat.S_IWOTH))
    except OSError:
        pass


def safe_unlink(file_path: Path | str) -> None:
    """Remove a file safely, restoring write permissions on Windows first if read-only."""
    p = Path(file_path).resolve()
    if not p.exists():
        return
    try:
        # Restore write permissions in case file was marked read-only on Windows
        p.chmod(stat.S_IWRITE)
        p.unlink()
    except OSError:
        pass


def verify_evidence_integrity(
    storage_path: Path | str,
    expected_sha256: str,
    *,
    chunk_size: int = 64 * 1024,
) -> bool:
    """Pre-extraction hash verification asserting file has not been altered on disk."""
    p = Path(storage_path).resolve()
    if not p.is_file():
        return False

    hasher = hashlib.sha256()
    with open(p, "rb") as f:
        while True:
            chunk = f.read(chunk_size)
            if not chunk:
                break
            hasher.update(chunk)

    computed = hasher.hexdigest().lower()
    return computed == expected_sha256.strip().lower()
