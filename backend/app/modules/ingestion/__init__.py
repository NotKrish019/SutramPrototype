"""Evidence Ingestion & Hashing Module (Member 2 ownership).

Provides safe evidence intake, streaming cryptographic verification, content sniffing,
and non-destructive versioned extraction run management.
"""

from app.modules.ingestion.service import (
    EvidenceIngestionService,
    IngestionRepository,
    default_registry,
)
from app.modules.ingestion.storage import (
    get_safe_storage_path,
    sanitize_filename,
    save_evidence_file,
    verify_evidence_integrity,
)
from app.modules.ingestion.validation import sniff_file_content

__all__ = [
    "EvidenceIngestionService",
    "IngestionRepository",
    "default_registry",
    "get_safe_storage_path",
    "sanitize_filename",
    "save_evidence_file",
    "sniff_file_content",
    "verify_evidence_integrity",
]
