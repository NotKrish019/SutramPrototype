# Documentation

Start with the [system overview](architecture/system-overview.md), then the [setup guide](development/setup-guide.md).

**New here?** Read the 📘 [**project documentation**](PROJECT_DOCUMENTATION.md) — the whole project in one document, from the problem to the results — and 📦 [**requirements**](../REQUIREMENTS.md) for what to install.

## Architecture

| Document | About |
|---|---|
| [System overview](architecture/system-overview.md) | Components, data flow and forensic principles |
| [Folder structure](architecture/folder-structure.md) | What lives where in the monorepo |
| [Rust backend migration](architecture/rust-backend-migration.md) | Why the application backend moved to Rust (ADR 001) |
| [Reconstruction adapters](architecture/reconstruction-integration-adapters.md) | How extracted events feed timeline, spatial and contradiction reconstruction |
| [Reconstruction assumptions](architecture/rust-reconstruction-assumptions.md) | Labelled assumptions and the judicial export safeguard |
| [Video frame handoff](architecture/rust-video-frame-handoff.md) | Live CCTV frame handoff to the Rust API |
| [Frontend: public site](architecture/frontend-public-site.md) | The landing site at `/` |
| [Frontend: workspace](architecture/frontend-workspace.md) | The investigator workspace at `/workspace` |
| [Frontend: API client](architecture/frontend-api-client.md) | How the web app talks to the API |
| [Rust build portability](architecture/rust-build-portability-audit.md) | Toolchain and linker notes for Windows builds |
| [USP and gap analysis](architecture/usp-gap-analysis.md) | What sets YUKTI apart, and what's still missing |
| [MVP demonstration runbook](architecture/mvp-demonstration-runbook.md) | Step-by-step end-to-end demo |

## API

- [Reconstruction API](api/reconstruction-api.md): request and response formats, status values and the forensic report endpoint.
- [OpenAPI specification](api/openapi.json)

## Decisions

Architecture decision records:

- [0001: Frontend client-side routing](decisions/0001-frontend-client-side-routing.md)
- [0002: Standalone CLI crate](decisions/0002-standalone-cli-crate.md)

## Development

- [Setup guide](development/setup-guide.md)
- [Git workflow](development/git-workflow.md): single `main` branch policy.
- [Team onboarding](development/team-onboarding.md): the four-member responsibility matrix.
- [CLI getting started](../cli/GETTING_STARTED.md)

## Demo

- [Forensic report of the synthetic warehouse burglary](demo/README.md)

## Contract decisions

[`member-4/`](member-4/) holds the reconstruction contract decisions (DEC-001 … DEC-007) and their approval record. Code and API output refer to these files by path, so they stay where they are.

## Archive

[`archive/`](archive/README.md) holds the phase-by-phase development record: completion reports, findings registers and per-member worklogs. It's kept for history and isn't needed to understand or run the project.
