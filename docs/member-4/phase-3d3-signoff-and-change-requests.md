# Phase 3D.3: contract sign-off form and upstream change requests

**Prepared by:** Member 4 · **Date:** 2026-11-20 · **Status:** awaiting responses. Nothing is approved.

This file holds the requests Member 4 needs answered before the producer boundaries can be
closed. As of the date above, no team sign-off, Member 1 change or Member 2 change has been
received (`origin/main` = `ed1861c`, no other branches). Every decision stays withheld in code
(`APPROVED_CONTRACT_DECISIONS = frozenset()`).

---

## A. Contract decisions: sign-off form

Full text of each decision: [`phase-3c-contract-decisions.md`](phase-3c-contract-decisions.md).
Proposed shared-schema wording: [`proposed-shared-schema-changes.md`](proposed-shared-schema-changes.md).

For each decision, every affected owner records **APPROVE** or **REJECT**, with the date and a
verifiable reference: a PR review link, a commit, or a link to a meeting-notes / chat message.
"Approved in a task brief" is not a reference. Once a row is fully approved, Member 4 records it
in [`team-contract-approvals-2026-11-19.md`](team-contract-approvals-2026-11-19.md) and adds the
ID to `APPROVED_CONTRACT_DECISIONS` in the same change. Only then is the matching part of
`shared/schemas/README.md` updated.

| Decision | Summary | Must approve | M1 | M2 | M3 | M4 | Reference | Result |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| DEC-001 | `true_utc = localized_device_time + clock_offset_seconds` (fast clock → negative offset) | M2, M4 | n/a | PENDING | n/a | proposer | none | **WITHHELD** |
| DEC-002 | Precision `EXACT_SECOND`/`MINUTE`/`HOUR`/`DAY`/`ESTIMATED`; half-open UTC window | M1, M2, M3, M4 | PENDING | PENDING | PENDING | proposer | none | **WITHHELD** |
| DEC-003 | Relation types add `CONTRADICTS`, `CO_OCCURRED_WITH`; `CONTRADICTS` always inferred | M1, M2, M3, M4 | PENDING | PENDING | PENDING | proposer | none | **WITHHELD** |
| DEC-004 | `Relationship.weight` nullable; null never shown as 0.0 | M2, M3, M4 | n/a | PENDING | PENDING | proposer | none | **WITHHELD** |
| DEC-005 | Optional `duration_seconds`, `uncertainty_window_start/end` (both or neither, start <= nominal < end) | M1, M2, M4 | PENDING | PENDING | n/a | proposer | none | **WITHHELD** |
| DEC-006 | `Evidence.metadata`: `source_timezone`, `clock_offset_seconds`, `recording_start_timestamp`, `device_coordinates`; absent stays absent | M2, M4 | n/a | PENDING | n/a | proposer | none | **WITHHELD** |
| DEC-007 | Extraction keeps `raw_timestamp` verbatim; only the M4 temporal engine produces UTC, applying correction once | M1, M2, M4 | PENDING | PENDING | n/a | proposer | none | **WITHHELD** |

Rejections are recorded the same way. A rejected decision stays withheld, and Member 4 revises
the proposal.

---

## B. Request to Member 1 (extraction)

Details: [`member1-candidate-contract-gap.md`](member1-candidate-contract-gap.md).

1. **`ExtractedEventCandidate.timestamp_precision`**: required. The value the source format
   actually supports, using the DEC-002 vocabulary (needs DEC-002 approval). Document per parser
   (CDR, chat) how precision is determined. It must not be guessed from string formatting
   alone.
2. **`ExtractedEventCandidate.description`**: required. A short summary of the source record.
3. **Keep** `location_name` and `metadata`. Member 4 preserves them verbatim
   (`source_fields_preserved`).
4. **Frame consumer**: implement Member 4's `ExtractedFrameConsumer` protocol
   (`backend/app/modules/reconstruction/adapters/frame_handoff.py`):
   `submit_frames(handoff: FrameHandoff) -> None`. It receives the derived frames of one CCTV
   item, bound to `source_video_sha256` with per-frame `artifact_id` / `sha256_hash`. It must not
   modify the frame files. Events derived from frames come back as ordinary event candidates
   whose `source_evidence_id` is the video, with the frame `artifact_id` kept in `metadata`.
   Please confirm, or propose changes to, this interface.

**Acceptance (Member 4 will run):** `tests/integration/reconstruction/test_real_producers.py`
with the real `extract_evidence` output. The candidates must produce timeline events, not
`PRODUCER_CONTRACT_GAP`, and keep `location_name`/`metadata`. The test
`test_member1_real_extraction_output_is_blocked_as_contract_gap` is then replaced by a passing
real-path test.

---

## C. Request to Member 2 (ingestion)

Details and diff: [`member2-ingestion-clock-defaults-patch.md`](member2-ingestion-clock-defaults-patch.md).

1. Remove the `"UTC"` / `0.0` defaults for `source_timezone` / `clock_offset_seconds` (service
   and HTTP form). Write the keys only when declared.
2. Emit `source_type` from the committed `EvidenceType` enum (`CDR`, `CHAT_EXPORT`, `CCTV`, …),
   or agree a documented mapping with the team.
3. Expose `storage_uri` (canonical key `data/evidence/raw/{case}/{evidence}/{evidence}.raw`, or
   `urn:yukti:evidence:{evidence}:storage`). Never an absolute host path.
4. Agree one storage layout. Today Member 2 writes `{base}/{case}/{evidence}/{filename}` and
   Member 4 reads `{root}/data/evidence/raw/{case}/{evidence}/{evidence}.raw`. Either side may
   move, but it must be documented in `docs/architecture/`.
5. Resolve `created_at` vs shared `ingested_at`.

**Acceptance (Member 4 will run):** a real `EvidenceRecord` from `ingest_evidence` is accepted
with no bridging, its bytes resolve from the agreed location, and SHA-256 and size verify. An
undeclared IST naive timestamp is rejected (`SOURCE_TIMEZONE_UNKNOWN`), and a declared
`Asia/Kolkata` / `-300` is applied exactly once. Only after that does Member 4 remove
`MEMBER2_UNDECLARED_TIME_DEFAULTS` and the `CLOCK_METADATA_INGESTION_DEFAULT_IGNORED` safeguard.

---

## D. Response log

| Date | From | Item | Reference | Outcome |
| :--- | :--- | :--- | :--- | :--- |
| none yet | | | | |
