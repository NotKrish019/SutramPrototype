"""Entity Resolution Adapter (Member 4 — Phase 3D).

Resolves entity references carried by Events (``involved_entity_ids``) to canonical
``Entity.entity_id`` values using ONLY explicit, exact registrations:

1. An exact ``entity_id`` match.
2. An exact entry in a shared Entity record's ``aliases`` list.
3. An exact entry in an explicitly supplied alias registration map (alias → entity_id).

Matching is case-sensitive and whitespace-exact (after trimming the reference itself). There is
no fuzzy matching, no case folding, no transliteration and no ``canonical_name`` similarity:
"Ravi Kumar", "Ravi Kumaar" and "ravi kumar" are three different references. A reference that
matches more than one entity is reported as ambiguous and is never guessed. Unresolved
references are preserved verbatim with an explicit status and are withheld from spatial
reconstruction so no trajectory or contradiction lead is attributed to an unregistered identity.
"""

from collections.abc import Mapping, Sequence
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.errors import EntityRegistryError
from app.schemas.reconstruction import extract_field


class EntityReferenceStatus(StrEnum):
    RESOLVED = "RESOLVED"
    UNRESOLVED_UNKNOWN = "UNRESOLVED_UNKNOWN"
    UNRESOLVED_AMBIGUOUS = "UNRESOLVED_AMBIGUOUS"


class EntityResolutionMethod(StrEnum):
    ENTITY_ID = "ENTITY_ID"
    ENTITY_ALIAS = "ENTITY_ALIAS"
    EXPLICIT_REGISTRATION = "EXPLICIT_REGISTRATION"


class RegisteredEntity(BaseModel):
    """Validated, immutable view over a shared Entity record."""

    model_config = ConfigDict(frozen=True)

    entity_id: str
    case_id: str
    entity_type: str
    canonical_name: str
    aliases: list[str] = Field(default_factory=list)
    source_evidence_ids: list[str] = Field(default_factory=list)


class EntityReferenceResolution(BaseModel):
    """Outcome of resolving one reference. ``canonical_entity_id`` is set only when RESOLVED."""

    model_config = ConfigDict(frozen=True)

    reference: str
    status: EntityReferenceStatus
    canonical_entity_id: str | None = None
    method: EntityResolutionMethod | None = None
    candidate_entity_ids: list[str] = Field(default_factory=list)
    note: str


class EventEntityResolution(BaseModel):
    """Resolution of all entity references on one Event."""

    model_config = ConfigDict(frozen=True)

    event_id: str
    resolutions: list[EntityReferenceResolution] = Field(default_factory=list)
    canonical_entity_ids: list[str] = Field(
        default_factory=list, description="Deduplicated canonical IDs in first-seen order."
    )
    unresolved_references: list[str] = Field(default_factory=list)


class EntityRegistry:
    """Immutable exact-match index of a case's entities and explicitly registered aliases."""

    def __init__(
        self,
        entities: Mapping[str, RegisteredEntity],
        alias_index: Mapping[str, Mapping[str, EntityResolutionMethod]],
    ) -> None:
        self._entities: dict[str, RegisteredEntity] = dict(sorted(entities.items()))
        self._alias_index: dict[str, dict[str, EntityResolutionMethod]] = {
            alias: dict(sorted(targets.items())) for alias, targets in sorted(alias_index.items())
        }

    @property
    def entities(self) -> dict[str, RegisteredEntity]:
        return dict(self._entities)

    def ambiguous_aliases(self) -> dict[str, list[str]]:
        """Aliases that map to more than one entity, or collide with another entity's ID."""
        result: dict[str, list[str]] = {}
        for alias, targets in self._alias_index.items():
            candidates = set(targets)
            if alias in self._entities:
                candidates.add(alias)
            if len(candidates) > 1:
                result[alias] = sorted(candidates)
        return result

    def resolve(self, reference: str) -> EntityReferenceResolution:
        if not isinstance(reference, str) or not reference.strip():
            raise EntityRegistryError("Entity reference must be a non-empty string.")
        key = reference.strip()
        alias_targets = self._alias_index.get(key, {})
        candidates: set[str] = set(alias_targets)
        if key in self._entities:
            candidates.add(key)

        if not candidates:
            return EntityReferenceResolution(
                reference=key,
                status=EntityReferenceStatus.UNRESOLVED_UNKNOWN,
                note=(
                    "No registered entity_id or exact alias matches this reference; it is "
                    "preserved but excluded from spatial reconstruction. No similarity matching "
                    "is attempted."
                ),
            )
        if len(candidates) > 1:
            return EntityReferenceResolution(
                reference=key,
                status=EntityReferenceStatus.UNRESOLVED_AMBIGUOUS,
                candidate_entity_ids=sorted(candidates),
                note=(
                    "Reference matches more than one registered entity; it is not resolved and "
                    "requires investigator disambiguation."
                ),
            )

        (target,) = candidates
        if key in self._entities:
            method = EntityResolutionMethod.ENTITY_ID
        elif alias_targets[target] == EntityResolutionMethod.EXPLICIT_REGISTRATION:
            method = EntityResolutionMethod.EXPLICIT_REGISTRATION
        else:
            method = EntityResolutionMethod.ENTITY_ALIAS
        return EntityReferenceResolution(
            reference=key,
            status=EntityReferenceStatus.RESOLVED,
            canonical_entity_id=target,
            method=method,
            candidate_entity_ids=[target],
            note=f"Resolved by exact {method.value} match.",
        )

    def resolve_event(self, event_id: str, references: Sequence[str]) -> EventEntityResolution:
        resolutions = [self.resolve(ref) for ref in references]
        canonical: list[str] = []
        unresolved: list[str] = []
        for res in resolutions:
            if res.canonical_entity_id is not None:
                if res.canonical_entity_id not in canonical:
                    canonical.append(res.canonical_entity_id)
            elif res.reference not in unresolved:
                unresolved.append(res.reference)
        return EventEntityResolution(
            event_id=event_id,
            resolutions=resolutions,
            canonical_entity_ids=canonical,
            unresolved_references=unresolved,
        )


def _parse_entity(record: Mapping[str, Any] | BaseModel | Any, case_id: str) -> RegisteredEntity:
    if record is None:
        raise EntityRegistryError("Entity record is None.")
    entity_id = extract_field(record, "entity_id")
    if not isinstance(entity_id, str) or not entity_id.strip():
        raise EntityRegistryError("Entity record is missing a valid string 'entity_id'.")
    entity_id = entity_id.strip()

    def _str_field(name: str) -> str:
        value = extract_field(record, name)
        if not isinstance(value, str) or not value.strip():
            raise EntityRegistryError(
                f"Entity '{entity_id}' is missing a valid string '{name}'.", subject_id=entity_id
            )
        return value.strip()

    record_case_id = _str_field("case_id")
    if record_case_id != case_id:
        raise EntityRegistryError(
            f"Entity '{entity_id}' belongs to case '{record_case_id}', expected '{case_id}'.",
            subject_id=entity_id,
        )

    def _str_list(name: str) -> list[str]:
        value = extract_field(record, name)
        if value is None:
            return []
        if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
            raise EntityRegistryError(
                f"Entity '{entity_id}' field '{name}' must be a list of strings.",
                subject_id=entity_id,
            )
        items: list[str] = []
        for item in value:
            if not isinstance(item, str) or not item.strip():
                raise EntityRegistryError(
                    f"Entity '{entity_id}' field '{name}' contains an empty or non-string value.",
                    subject_id=entity_id,
                )
            items.append(item.strip())
        return items

    return RegisteredEntity(
        entity_id=entity_id,
        case_id=record_case_id,
        entity_type=_str_field("entity_type"),
        canonical_name=_str_field("canonical_name"),
        aliases=_str_list("aliases"),
        source_evidence_ids=_str_list("source_evidence_ids"),
    )


def build_entity_registry(
    entities: Sequence[Mapping[str, Any] | BaseModel | Any],
    *,
    case_id: str,
    explicit_aliases: Mapping[str, str] | None = None,
) -> EntityRegistry:
    """Validate shared Entity records and explicit alias registrations into an EntityRegistry."""
    if not isinstance(entities, Sequence) or isinstance(entities, (str, bytes)):
        raise EntityRegistryError("Entities input must be a sequence of Entity records.")

    registered: dict[str, RegisteredEntity] = {}
    for record in entities:
        entity = _parse_entity(record, case_id)
        if entity.entity_id in registered:
            raise EntityRegistryError(
                f"Duplicate entity_id '{entity.entity_id}'.", subject_id=entity.entity_id
            )
        registered[entity.entity_id] = entity

    alias_index: dict[str, dict[str, EntityResolutionMethod]] = {}
    for entity in registered.values():
        for alias in entity.aliases:
            if alias == entity.entity_id:
                continue
            alias_index.setdefault(alias, {})[entity.entity_id] = (
                EntityResolutionMethod.ENTITY_ALIAS
            )

    for alias, target in sorted((explicit_aliases or {}).items()):
        if not isinstance(alias, str) or not alias.strip():
            raise EntityRegistryError("Explicit alias registrations must use non-empty strings.")
        if not isinstance(target, str) or target.strip() not in registered:
            raise EntityRegistryError(
                f"Explicit alias '{alias}' targets unregistered entity_id {target!r}.",
                subject_id=alias.strip(),
            )
        clean_alias, clean_target = alias.strip(), target.strip()
        if clean_alias in registered and clean_alias != clean_target:
            raise EntityRegistryError(
                f"Explicit alias '{clean_alias}' would redefine the existing entity_id "
                f"'{clean_alias}' as '{clean_target}'; merging entities is not permitted here.",
                subject_id=clean_alias,
            )
        if clean_alias == clean_target:
            continue
        alias_index.setdefault(clean_alias, {})[clean_target] = (
            EntityResolutionMethod.EXPLICIT_REGISTRATION
        )

    return EntityRegistry(registered, alias_index)
