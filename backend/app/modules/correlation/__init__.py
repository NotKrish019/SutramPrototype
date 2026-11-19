"""Cross-Source Evidence Correlation Module (Member 1 ownership)."""

from app.modules.correlation.relationships import (
    generate_cdr_relationship_candidate,
    generate_chat_relationship_candidates,
)
from app.modules.correlation.resolution import (
    deduplicate_identifiers,
    normalize_phone_number,
)

__all__ = [
    "normalize_phone_number",
    "deduplicate_identifiers",
    "generate_cdr_relationship_candidate",
    "generate_chat_relationship_candidates",
]
