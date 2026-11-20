# Rust reconstruction: labelled assumptions and export gate (Member 4, Phase 3E.1)

**Status:** implemented (uncommitted). **Owner:** Member 4. **Date:** 2026-11-20
**Scope:** `backend-rust/src/reconstruction/`, plus two small fixes in `backend-rust/src/api/routes/reconstruction.rs`

## Why

The real producers (Rust extraction and ingestion, `ed1861c`) do not supply everything the
reconstruction needs. Before this change, the reconstruction filled the gaps silently:
precision defaulted to `EXACT_SECOND`, timezone and offset to `UTC` / `0`, and coordinates were
looked up from tower-name keywords. The output looked like observed fact. Decision (project
owner, 2026-11-20): keep the flow working, but **label every substituted value and block
judicial export** while any finding rests on one.

## What changed

- `model.rs`: `AssumptionKind` (`PRECISION_DEFAULTED`, `TIMEZONE_NOT_DECLARED_ON_EVENT`,
  `CLOCK_OFFSET_NOT_DECLARED_ON_EVENT`, `COORDINATES_INFERRED_FROM_TOWER_ID`),
  `ReconstructionAssumption { kind, field, assumed_value, basis }`, and
  `ExportSafeguard { status: CLEAR | BLOCKED, reasons }`. `TimelineEntry` gains `assumptions` and
  `raw_timestamp` (the producer's string, verbatim).
- `adapters.rs`: records an assumption whenever it uses a default precision, a timezone or offset
  not declared on the event (exhibit-level or configured fallback), or a tower-centroid position.
- `temporal.rs`: carries assumptions onto each entry, and labels engine-level timezone/offset
  defaults.
- `service.rs`: `FullCaseReconstructionResult.export_safeguard`, computed by
  `evaluate_export_safeguard`. It is BLOCKED when any entry has assumptions or any contradiction
  lead exists. Leads whose events rest on assumptions get a limitation saying so. Stored results
  without the field deserialize as BLOCKED ("not evaluated").
- `api/routes/reconstruction.rs`:
  1. **Bug fix.** The filesystem loader used request defaults (`UTC` / `0`) instead of the
     exhibit's declared timezone/offset, so a clock offset declared at upload was silently
     dropped. It now uses `unified_extraction_to_timeline_events`, which applies the run's
     recorded values exactly once.
  2. Events reloaded from the database are labelled with the request-default timezone/offset
     they receive.

All new fields are additive JSON. Existing clients keep working.

## Integration with 23a915d / 52953c3 (rebased onto, 2026-11-20)

- Member 1 now declares `timestamp_precision` (`EXACT_SECOND` for CDR and chat) and a
  `description` on each candidate. The adapter treats the typed field (or `metadata`) as
  producer-supplied, so `PRECISION_DEFAULTED` appears only when neither is present.
- Member 2 no longer writes `UTC` / `0.0` when nothing is declared. An undeclared timezone or
  offset now falls to the engine default and is labelled `*_NOT_DECLARED_ON_EVENT`.
- Per-event exclusion (`timeline.excluded_events`) is counted in `export_safeguard`, and the
  judicial report (`report.rs`) now also returns `SHIP_EXPORT_BLOCKED` when `export_safeguard`
  is BLOCKED, so the two gates cannot disagree.

## Still open

- No frame-consumer interface exists in the Rust backend.
- Contract decisions DEC-001 … DEC-007 have no recorded team approval. The team's Phase 2C docs
  reuse the DEC numbers with different meanings.
- Contradiction leads carry a wall-clock `generated_at`, so repeated runs differ in that field.

## Tests

`backend-rust/tests/reconstruction_real_producers.rs` (8 tests). It covers a real multipart upload
→ real CDR extraction → `POST /api/v1/cases/:id/reconstruction` (byte and SHA-256 integrity,
metadata and raw timestamp preserved, every assumption labelled, export BLOCKED); a declared
offset applied exactly once; deterministic repeated runs; a fully declared event yielding no
assumptions and a CLEAR export; inferred tower coordinates; unsupported precision and invalid
timezone excluded per event (rest of the case still reconstructs); and a contradiction blocking export.
