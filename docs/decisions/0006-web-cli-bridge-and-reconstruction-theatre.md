# 0006 · Web ↔ CLI bridge and the Reconstruction Theatre

- **Status:** Accepted
- **Date:** 2026-11-25
- **Scope:** `apps/web`, `cli`, one middleware layer in `backend-rust/src/app.rs`

## Context

The web workspace and the `yukti` CLI are both clients of the Rust API, but in practice they were
disconnected:

1. The API sent no CORS headers, so the browser workspace (served from another port) could not call it.
2. The web health parser expected the old Python response (`project`), while the Rust API answers
   `{ status, service }`. The workspace therefore always reported the API as offline.
3. The case list in the web was the synthetic fixture only, so cases created from the CLI never appeared.

The workspace also lacked one place that explains the whole investigation end to end, which is what
reviewers and judges look for first.

## Decision

**API.** A loopback-only CORS layer (`localhost`, `127.0.0.1`, `[::1]`, any port; GET/POST/OPTIONS).
Pages on other hosts still cannot read case data through a user's browser. Covered by a unit test.

**Web data layer.**
- The health validator accepts `service` as well as `project`.
- `liveRepository` lists `GET /api/v1/cases` ahead of the labelled synthetic cases and loads live detail
  from the reconstruction and evidence endpoints. The workspace switches to it automatically once the
  case endpoint answers (health alone is not enough), and back to synthetic fixtures when it does not.
- Synthetic-ness is decided per case (`WorkspaceCase.source`), so demo cases keep their artwork and
  disclaimers beside live cases.
- `ReviewItem.transit` keeps the structured movement basis (distance, required speed, threshold) so
  views can show the arithmetic instead of only prose.

**Bridge.** While live, the workspace polls the case list every 5 s (paused when the tab is hidden) and
announces cases that appear, typically from `yukti case create` or `yukti demo`. Every case view has a
terminal dock listing the CLI commands that reproduce it. `yukti open --case … --view …` opens the web at
the same case and view; `yukti demo` ends with that step and `yukti case overview` suggests it.

**Reconstruction Theatre** (`/workspace/cases/:id/theatre`). A six-chapter, narrated playback built from
the loaded `CaseDetail`: seal (hashing and custody ledger) → one clock (raw values glide to normalised
time while the axis zooms in) → places (route drawn in event order, inferred locations as areas) → links
(graph with observed / inferred / contradiction styling) → conflict (records side by side with the gap or
speed arithmetic) → brief. Each scene is a pure function of the model and a 0..1 progress value, so
playback is deterministic and scrubbable. Under `prefers-reduced-motion` it starts paused with each
chapter fully revealed.

## Consequences

- No new runtime dependencies in any package.
- The theatre reads only fields already in `CaseDetail`; it adds no API surface and does not touch
  `shared/schemas/`.
- Forensic principles are kept visible in the animation: inferred places are areas, inferred links are
  dashed, contradictions are labelled as leads, and every chapter states what YUKTI did and its limits.
- Local `apps/web/.env.local` files that still point `VITE_API_URL` at the retired Python API (port 8000)
  must be updated to `http://localhost:8080` for live mode.
