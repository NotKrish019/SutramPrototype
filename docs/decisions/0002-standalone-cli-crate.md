# 0002 — Standalone, HTTP-only CLI crate

- **Status:** Proposed (CLI/integration owner, 2026-11-21). Pending team review.
- **Scope:** `cli/` and `.github/workflows/cli.yml` only. No changes to `backend-rust/`, `apps/web/`,
  `shared/` or any database schema.

## Context

YUKTI needs a command-line client (`yukti`) for the CLI → API → UI demonstration. The investigator
workspace (`apps/web/`) already uses the Rust API over HTTP, and backend forensic logic, persistence and
contracts are owned separately and change often.

There were three ways to add the CLI:

1. Another `[[bin]]` inside `backend-rust/`.
2. A root Cargo workspace containing `backend-rust/` and a new CLI crate.
3. A standalone crate at `cli/` that talks to the API over HTTP only.

## Decision

Option 3: `cli/` is an independent crate (`yukti-cli`, binary `yukti`) with its own `Cargo.toml` and
`Cargo.lock`.

- It calls only the HTTP endpoints registered in `backend-rust/src/app.rs`, the same API the React
  workspace uses.
- It has **no dependency on the backend crate** and never connects to PostgreSQL or Neo4j directly.
- It contains **no forensic logic**. Response types are small client-side mirrors (`cli/src/dto.rs`).
  JSON output passes server bodies through unchanged, so fields the CLI does not model are not lost.
- There is **no root Cargo workspace**.
- It has its own CI workflow (`.github/workflows/cli.yml`): fmt, locked check, tests and clippy on Linux,
  plus a locked check and tests with MSVC on Windows. The workflow triggers only on `cli/**` changes.

## Rationale

- **One source of truth for behaviour.** If the CLI linked the backend crate, it could call reconstruction
  or persistence code directly and diverge from what the API and the UI show. Going through HTTP keeps the
  CLI, the UI and the API answering identically.
- **Ownership isolation.** A binary inside `backend-rust/` would share its `Cargo.toml` and `Cargo.lock`.
  Every CLI dependency change would then touch backend-owned files and risk merge conflicts with ongoing
  backend work. A root workspace would move the lockfile and `target/` for the backend, which is an
  architectural change for another team member's crate.
- **Honest output.** Because the CLI only reports what the API returns, it cannot fabricate records,
  identifiers or dependency states. For example, `yukti status` shows PostgreSQL or Neo4j as connected only
  when the API's readiness probe says so, and `yukti case list` flags the API's filesystem placeholder records
  instead of presenting them as database rows.
- **Small and offline-buildable.** Dependencies are `clap`, `reqwest` (rustls), `tokio`, `serde` and
  `serde_json`. All of them are already used by `backend-rust`, so a machine that has built the backend
  can build the CLI offline.

## Consequences

- API contract changes are not caught at compile time. The CLI's DTO mirrors and test fixtures
  (`cli/tests/support/mod.rs`) must be updated when the corresponding handlers change. The CI path filter
  means a backend-only change does not re-run CLI tests.
- CLI commands can only offer what the API exposes. Planned commands that depend on API functionality that
  doesn't exist yet (scenario comparison, finding provenance, evidence gaps) must exit with code `69`
  (unsupported) rather than reimplement that logic client-side.
- The CLI is documented in `cli/README.md`, which also defines its exit codes and versioned JSON output
  schemas as a public contract.
