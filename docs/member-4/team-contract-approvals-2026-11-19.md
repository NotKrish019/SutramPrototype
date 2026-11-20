# Contract Decision Approvals: NOT VERIFIED

**Status (Phase 3D.2, 2026-11-20): no verified team sign-off exists for DEC-001 … DEC-007.**
**Recorded by:** Member 4 (Forensic Intelligence & System Integration)
**Applies to:** [`phase-3c-contract-decisions.md`](phase-3c-contract-decisions.md) (DEC-001 … DEC-006), plus DEC-007
**Code effect:** `backend/app/modules/reconstruction/adapters/contracts.py` →
`APPROVED_CONTRACT_DECISIONS = frozenset()` (fail closed). The full proposal is
`PROPOSED_CONTRACT_DECISIONS`, which must be passed explicitly. Results then list those decisions
under `unverified_contract_decisions`.

> **Provenance.** The block below is the "APPROVED DECISIONS" section of the Phase 3E task brief,
> copied verbatim on 2026-11-19. It is **not** a team sign-off. It names no approvers, gives no
> date or channel of agreement, and no original signed document has been found. The Phase 3D.1
> audit found it unverifiable, so Phase 3D.2 withdrew the approvals.
>
> **To record a genuine approval**, add for each decision: the approving members (1, 2, 3, 4),
> the date, and a link to the original record (meeting notes, PR review, chat message). Then add
> the decision IDs to `APPROVED_CONTRACT_DECISIONS` in the same change.

## Sign-off record

| Decision | Member 1 | Member 2 | Member 3 | Member 4 | Evidence link |
| :--- | :--- | :--- | :--- | :--- | :--- |
| DEC-001 … DEC-007 | not recorded | not recorded | not recorded | proposer | none |

## Copied brief text (proposal content only, not approval evidence)

---

```text
APPROVED DECISIONS (all now approved, enable them in the gate)
DEC-001: TrueUTC = DeviceTimeLocalizedToUTC + clock_offset_seconds. Fast clock gives a negative offset (5 minutes fast = -300).
DEC-002: precision values are EXACT_SECOND, MINUTE, HOUR, DAY, ESTIMATED. Keep local calendar flooring before UTC conversion. Uncertainty intervals are half open: [start_utc, end_utc), start <= nominal < end.
DEC-003: relationship taxonomy includes CONTRADICTS and CO_OCCURRED_WITH, observed vs inferred distinguished.
DEC-004: weight is nullable, never 0.0 as a substitute.
DEC-005: optional duration_seconds, uncertainty_window_start, uncertainty_window_end, kept separate. Validation: duration finite and non negative; window start and end both present or both absent; start < end; start <= nominal < end. Missing values stay null.
DEC-006: evidence metadata fields source_timezone (IANA), clock_offset_seconds, recording_start_timestamp, device_coordinates.
DEC-007 (reverses the old assumption): extraction preserves raw_timestamp unchanged. Only the Member 4 temporal engine produces corrected UTC (Event.timestamp / timestamp_utc). Apply localization and clock offset exactly ONCE. If an incoming timestamp already has Z or a numeric offset, parse it correctly and do not localize it again. Never overwrite raw_timestamp. Output must carry raw_timestamp, timestamp_utc, source_timezone, clock_offset_seconds, precision and the UTC uncertainty window.
Unknown timezone or unverified clock offset must not be presented as verified calibration. Preserve that uncertainty in the result.
```
