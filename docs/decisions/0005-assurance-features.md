# 0005 — Assurance features: breaking-point robustness, custody audit, court package, WhatsApp exports

- **Status:** Proposed (2026-11-24). Pending team review. The backend parts touch Member 1
  (extraction), Member 2 (custody, API) and Member 4 (reconstruction) areas.
- **Scope:** new backend modules and routes, new CLI commands, synthetic demo data. No change to
  existing endpoints, schemas, stored formats or `shared/`.

## Context

The gap analysis (`docs/architecture/usp-gap-analysis.md`) lists, among the missing pieces:
- sensitivity and robustness of leads;
- Rust-side provenance and audit;
- WhatsApp `.txt` chat exports.

Judges and investigators ask the same thing a defence lawyer will: *how sure are you, and can
you show the evidence was not altered?*

## Decision

### Backend (`backend-rust/`)

| Addition | What it does |
|---|---|
| `reconstruction/robustness.rs`, `GET /api/v1/cases/:case_id/robustness` | For each contradiction lead, solves for the clock shift, location error and speed threshold at which the lead stops firing. It re-runs the same functions that raised the lead just inside and just outside each margin (`boundary_verified`). Leads from one exhibit share one clock, so the clock dimension is reported as not applicable. A fixed band rule classes each lead ROBUST / SENSITIVE / FRAGILE; the bands are published in the response. No probabilities and no weights (DEC-004). |
| `custody_audit.rs`, `GET /api/v1/cases/:case_id/custody/verify` | Re-hashes every stored exhibit against its recorded SHA-256. With PostgreSQL it also checks the custody ledger's hash links. It computes a case seal from a published recipe. A ledger entry whose hash cannot be reproduced (the stored timestamp has lower precision than the one hashed) is reported as *not reproducible*, never as verified. |
| `extraction/whatsapp.rs` | Parses Android and iOS WhatsApp `.txt` exports into the existing chat pipeline. Ingestion classifies them as `CHAT_EXPORT`. Details below. |
| Tower table | Adds Mumbai BKC to the tower centroid lookup, for the demo. |

WhatsApp parsing rules:
- Minute timestamps get `MINUTE` precision.
- Day/month order is proven from the data when possible; otherwise DD/MM is used and labelled as assumed.
- Recipients are only set for one-to-one chats.
- Every message keeps its source line and its timestamp exactly as written.

The two routes live in their own module, `api/routes/assurance.rs`, and read only.

### CLI (`cli/`)

All new commands are HTTP clients of the API. The backend computes the results; the CLI displays them.

| Command | What it does |
|---|---|
| `robustness` | Shows the margins. |
| `custody verify` | Shows the audit and the seal as a QR code. |
| `certificate` | Drafts a Section 63 BSA certificate from recorded data only. Anything YUKTI cannot know is left blank, and it is always marked DRAFT. |
| `challenge` | Ranks the weaknesses the API already reports, by a fixed published rule. |
| `ask` | Answers a small set of question shapes by fixed rules, citing every fact. It refuses when the evidence does not answer. No language model is used. |
| `replay` | Animates the timeline. |
| `demo` | Runs all of the above on the embedded synthetic case. |

## Consequences

- Each lead now has a quantified answer to "how wrong would you have to be?". The demo confirms it
  independently: a what-if beyond the computed margin removes exactly the leads marked fragile.
- Custody verification depends on the raw store. In filesystem mode there is no ledger, and the
  audit says so plainly.
- The certificate is a drafting aid, not legal advice. Its layout follows the Schedule's Part A and
  Part B and must be checked against the official text.
- Follow-up for the backend owners:
  - Store the exact hashed timestamp text in the custody ledger, so every entry hash can be recomputed.
  - Make the reconstruction use the active reprocessed extraction run's declared clock offset.
  - Fix the existing rustfmt and clippy findings that fail backend CI. They predate this change.
