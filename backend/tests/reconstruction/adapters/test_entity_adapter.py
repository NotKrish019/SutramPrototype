"""Phase 3D — Entity Resolution Adapter tests (exact registrations only, no fuzzy merging)."""

from collections.abc import Callable
from copy import deepcopy
from typing import Any

import pytest
from tests.reconstruction.adapters.synthetic_burglary import CASE_ID, SyntheticCase

from app.modules.reconstruction.adapters.entity_adapter import (
    EntityReferenceStatus,
    EntityResolutionMethod,
    build_entity_registry,
)
from app.modules.reconstruction.adapters.errors import EntityRegistryError


def test_exact_entity_id_resolves(burglary_case: SyntheticCase) -> None:
    registry = build_entity_registry(burglary_case.entities, case_id=CASE_ID)
    res = registry.resolve("ent-person-ravi")
    assert res.status == EntityReferenceStatus.RESOLVED
    assert res.canonical_entity_id == "ent-person-ravi"
    assert res.method == EntityResolutionMethod.ENTITY_ID


def test_registered_alias_resolves_to_canonical_id(burglary_case: SyntheticCase) -> None:
    registry = build_entity_registry(burglary_case.entities, case_id=CASE_ID)
    res = registry.resolve("Handset A")
    assert res.status == EntityReferenceStatus.RESOLVED
    assert res.canonical_entity_id == "ent-phone-01"
    assert res.method == EntityResolutionMethod.ENTITY_ALIAS


def test_explicit_registration_resolves(burglary_case: SyntheticCase) -> None:
    registry = build_entity_registry(
        burglary_case.entities,
        case_id=CASE_ID,
        explicit_aliases={"m1-person-0007": "ent-person-ravi"},
    )
    res = registry.resolve("m1-person-0007")
    assert res.canonical_entity_id == "ent-person-ravi"
    assert res.method == EntityResolutionMethod.EXPLICIT_REGISTRATION


@pytest.mark.parametrize(
    "reference",
    ["Ravi Kumar", "Ravi Kumaar", "ravi kumar", "R Kumar", "r. kumar", "ent-person-rav", "ENT-PERSON-RAVI"],
)
def test_similar_names_and_ids_never_merged(burglary_case: SyntheticCase, reference: str) -> None:
    registry = build_entity_registry(burglary_case.entities, case_id=CASE_ID)
    res = registry.resolve(reference)
    # canonical_name is never used for matching, and near-misses never resolve.
    assert res.status == EntityReferenceStatus.UNRESOLVED_UNKNOWN
    assert res.canonical_entity_id is None


def test_similar_named_entities_remain_distinct(burglary_case: SyntheticCase) -> None:
    registry = build_entity_registry(burglary_case.entities, case_id=CASE_ID)
    resolution = registry.resolve_event("evt-x", ["ent-person-ravi", "ent-person-ravi2", "R. Kumar"])
    # "R. Kumar" is an exact alias of ent-person-ravi → deduplicated; ravi2 stays separate.
    assert resolution.canonical_entity_ids == ["ent-person-ravi", "ent-person-ravi2"]
    assert resolution.unresolved_references == []


def test_unknown_reference_preserved_with_diagnostic(burglary_case: SyntheticCase) -> None:
    registry = build_entity_registry(burglary_case.entities, case_id=CASE_ID)
    resolution = registry.resolve_event("evt-04", ["ent-person-ravi", "ent-unknown-77"])
    assert resolution.canonical_entity_ids == ["ent-person-ravi"]
    assert resolution.unresolved_references == ["ent-unknown-77"]
    unknown = resolution.resolutions[1]
    assert unknown.status == EntityReferenceStatus.UNRESOLVED_UNKNOWN
    assert "No registered entity_id" in unknown.note


def test_alias_shared_by_two_entities_is_ambiguous(burglary_case: SyntheticCase) -> None:
    entities = deepcopy(burglary_case.entities)
    entities[0]["aliases"].append("Ravi")
    entities[1]["aliases"].append("Ravi")
    registry = build_entity_registry(entities, case_id=CASE_ID)
    res = registry.resolve("Ravi")
    assert res.status == EntityReferenceStatus.UNRESOLVED_AMBIGUOUS
    assert res.canonical_entity_id is None
    assert res.candidate_entity_ids == ["ent-person-ravi", "ent-person-ravi2"]
    assert registry.ambiguous_aliases() == {"Ravi": ["ent-person-ravi", "ent-person-ravi2"]}


def test_alias_colliding_with_other_entity_id_is_ambiguous(burglary_case: SyntheticCase) -> None:
    entities = deepcopy(burglary_case.entities)
    entities[1]["aliases"].append("ent-person-ravi")
    registry = build_entity_registry(entities, case_id=CASE_ID)
    assert registry.resolve("ent-person-ravi").status == EntityReferenceStatus.UNRESOLVED_AMBIGUOUS


def test_explicit_alias_cannot_merge_existing_entities(burglary_case: SyntheticCase) -> None:
    with pytest.raises(EntityRegistryError, match="merging entities is not permitted"):
        build_entity_registry(
            burglary_case.entities,
            case_id=CASE_ID,
            explicit_aliases={"ent-person-ravi2": "ent-person-ravi"},
        )


def test_explicit_alias_must_target_registered_entity(burglary_case: SyntheticCase) -> None:
    with pytest.raises(EntityRegistryError, match="unregistered"):
        build_entity_registry(
            burglary_case.entities, case_id=CASE_ID, explicit_aliases={"x": "ent-missing"}
        )


@pytest.mark.parametrize(
    ("mutate", "match"),
    [
        (lambda es: es.append(deepcopy(es[0])), "Duplicate entity_id"),
        (lambda es: es[0].update(case_id="case-other"), "belongs to case"),
        (lambda es: es[0].update(aliases="R. Kumar"), "list of strings"),
        (lambda es: es[0].update(entity_type=""), "entity_type"),
    ],
)
def test_invalid_entity_records_rejected(
    burglary_case: SyntheticCase, mutate: Callable[[list[dict[str, Any]]], None], match: str
) -> None:
    entities = deepcopy(burglary_case.entities)
    mutate(entities)
    with pytest.raises(EntityRegistryError, match=match):
        build_entity_registry(entities, case_id=CASE_ID)


def test_registry_does_not_mutate_inputs(burglary_case: SyntheticCase) -> None:
    original = deepcopy(burglary_case.entities)
    build_entity_registry(burglary_case.entities, case_id=CASE_ID).resolve("R. Kumar")
    assert burglary_case.entities == original
