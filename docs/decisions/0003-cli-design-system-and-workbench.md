# 0003 — CLI design system and investigator workbench commands

- **Status:** Proposed (2026-11-24). Pending team review.
- **Scope:** `cli/` only. No changes to `backend-rust/`, `apps/web/`, `shared/` or any database schema.
- **Builds on:** [0002](0002-standalone-cli-crate.md). The CLI stays a standalone, HTTP-only crate with
  no forensic logic.

## Context

The first CLI release covered `status`, `case`, `evidence ingest|list` and `reconstruction`. Its output
was plain, uncoloured ASCII. Meanwhile the Rust API already serves the endpoints the workspace UI is built
on: timeline, spatial, contradictions, gaps, provenance, report, graph and what-if simulation, as well as
evidence show, extract, reprocess and extraction runs. In the CLI → API → UI demonstration, the CLI needs to look like
part of the same product and to reach the same capabilities as the UI.

## Decision

1. **One design system in `cli/src/ui.rs`, taken from the web tokens** (`apps/web/tailwind.config.js`):
   crimson for brand, active state and anything flagged, plus the semantic evidence palette (observed,
   inferred, supplied, assumed, flagged, verified). Each state has both a glyph and words, so colour is
   never the only signal. Body text keeps the terminal's foreground colour, for light themes.
2. **Colour is a property of the stream, not of the command.** It is used only on a terminal or with
   `--color always`. `NO_COLOR`, `TERM=dumb` and `--color never` disable it, and `--ascii` replaces the
   glyphs. Terminal facts are captured once in `EnvVars.terminal`, so tests stay deterministic. The
   layout is identical with and without colour, and a test checks this. JSON output is unchanged and never coloured.
3. **Workbench commands map one-to-one onto existing endpoints**: `timeline`, `spatial`, `contradictions`,
   `gaps`, `provenance`, `report`, `graph summary|neighbors|path`, `whatif`, `evidence
   show|extract|reprocess|extraction|runs|verify`, `case overview`. As before, each checks the case first,
   sends only what the user gave, and passes the complete response through in its versioned JSON document.
4. **Local hashing is the only computation the CLI adds**, and it is a check, never a replacement:
   - `evidence ingest` compares the SHA-256 of the bytes it sent with the API's recorded hash.
   - `evidence verify` compares a local copy with that recorded hash.
   - `report --out` prints the SHA-256 of the file it wrote.

   A mismatch exits with the new code `7`.
5. **Convenience without hidden state:** `YUKTI_CASE` supplies `--case`. `yukti shell` re-runs each
   line through the normal parser and executor, and only adds `--case` where a command accepts it.
   `doctor`, `config` and `completions` are diagnostics; `config` and `completions` make no API call.

## Consequences

- Human output changed, so scripts must use `--json`, whose documents are versioned and unchanged for
  existing commands apart from additive fields. Two new dependencies, `clap_complete` and
  `terminal_size`, are not shared with `backend-rust`, so offline builds need them in the cargo cache.
- `reconstruction compare` still exits `69`. Stateless comparison is available through `whatif`.
  Persisted comparison waits for a backend contract.
- The test suite grew from 113 to 153 tests. It now covers every new command, the colour and ASCII modes,
  and exit code `7`.
