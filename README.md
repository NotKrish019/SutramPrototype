<div align="center">

# YUKTI

**AI-powered digital evidence correlation and crime reconstruction**

Built for **SUTRAM 2026**, the ISEA–IGDTUW National Hackathon.

[![Continuous Integration](https://github.com/NotKrish019/SutramPrototype/actions/workflows/ci.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/ci.yml)
[![Rust Backend CI](https://github.com/NotKrish019/SutramPrototype/actions/workflows/backend-rust.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/backend-rust.yml)
[![CLI CI](https://github.com/NotKrish019/SutramPrototype/actions/workflows/cli.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/cli.yml)

</div>

---

Investigations today run on fragmented evidence: call detail records (CDRs), chat exports, CCTV footage and seized devices, each analysed on its own. Lining up thousands of timestamps, phone numbers and cell towers by hand is slow, and it's easy to miss the one contradiction that matters.

**YUKTI** ingests evidence from many sources and fingerprints every file with SHA-256. It extracts events and entities, links them across sources, and rebuilds what happened as a **timeline**, a **spatial trajectory** and a **set of contradiction leads**. Every reconstructed finding traces back to the exact evidence it came from. Every value the system inferred rather than observed is labelled as an assumption.

> **Forensic stance.** YUKTI separates what was *observed* from what was *inferred*. It never presents a similarity score as proof of identity. It blocks judicial export while a reconstruction still rests on unverified assumptions.

## Features

| | Capability | Status |
|---|---|---|
| 📥 | **Evidence ingestion.** Content-validated upload, SHA-256 fingerprinting, immutable raw store, and declared timezone and clock offset per exhibit | ✅ Implemented |
| 🔎 | **Extraction.** CDR and chat-export parsing into normalised events, with phone number, IMEI and IMSI identifiers | ✅ Implemented |
| 🔗 | **Correlation.** Entity resolution and cross-source relationships, projected into a Neo4j knowledge graph | ✅ Implemented |
| 🕒 | **Temporal reconstruction.** A strictly ordered timeline with time windows, clock-offset correction, and uncertain or concurrent events flagged | ✅ Implemented |
| 🗺️ | **Spatial reconstruction.** Trajectories and movement segments from cell towers, checked against speed thresholds | ✅ Implemented |
| ⚠️ | **Contradiction detection.** Spatio-temporal conflicts raised as *leads*, never verdicts, each with a neutral explanation and its limitations | ✅ Implemented |
| 🧾 | **Provenance and gaps.** Finding-to-evidence traceability, evidence-gap analysis, and what-if simulation | ✅ Implemented |
| ⚖️ | **Judicial report.** A forensic dossier with a disclaimer, gated by an export safeguard | ✅ Implemented |
| 🖥️ | **Investigator workspace.** A React dashboard with case management, the evidence graph, the timeline and contradiction review | ✅ Implemented |
| ⌨️ | **`yukti` CLI.** Cases, evidence, reconstruction and a `doctor` setup check from the terminal | ✅ Implemented |
| 🎥 | **CCTV and video intelligence.** A frame handoff interface is in place; full video analysis is planned | 🟡 Partial |
| 🧠 | **OCR, NER and on-device decision support** (PaddleOCR, spaCy, Laya / ONNX) | 🔜 Planned |

## Architecture

```mermaid
flowchart LR
    subgraph Clients
        W[Web workspace<br/>React + TypeScript]
        C[yukti CLI<br/>Rust]
    end
    subgraph API[Rust API · Axum · :8080]
        I[Ingestion<br/>SHA-256] --> E[Extraction<br/>CDR · chat]
        E --> R[Correlation]
        R --> T[Reconstruction<br/>timeline · spatial · contradictions]
        T --> P[Report<br/>export safeguard]
    end
    W -->|HTTP| API
    C -->|HTTP| API
    API --> FS[(Evidence store<br/>raw + derived)]
    API -.optional.-> PG[(PostgreSQL 16)]
    API -.optional.-> N4[(Neo4j 5)]
```

- The **Rust API** (`backend-rust/`) is the main backend. It runs with no database at all, keeping cases and evidence on disk. PostgreSQL and Neo4j are optional add-ons.
- The **web workspace** and the **CLI** are clients of that API over HTTP. Neither contains any forensic logic.
- The **Python service** (`backend/`) is the original FastAPI implementation and remains the reference for some algorithms.
- The **decision service** (`decision-service/`) is the Node.js home for future on-device AI inference.

See the [system overview](docs/architecture/system-overview.md) for the full picture.

## Quick start

**Prerequisites:** Git, [Rust](https://rustup.rs) (stable), and [Node.js 22 LTS](https://nodejs.org). Docker is optional, for PostgreSQL and Neo4j.

```bash
git clone https://github.com/NotKrish019/SutramPrototype.git
cd SutramPrototype
```

**1. Start the API** in its own terminal:

```bash
cd backend-rust
cargo run --bin sutram-backend-rust      # http://127.0.0.1:8080
```

**2. Start the web workspace** in a second terminal:

```bash
cd apps/web
npm ci
npm run dev                              # http://localhost:5173
```

**3. Use the CLI** (optional) in a third terminal:

```bash
cd cli
cargo install --path .
yukti doctor                             # checks the whole setup
```

Then walk through a full synthetic case: create it, ingest the sample CDR and chat export, and reconstruct the timeline. The commands are in the [CLI getting-started guide](cli/GETTING_STARTED.md).

<details>
<summary><b>Optional: PostgreSQL and Neo4j</b></summary>

```bash
docker compose -f docker-compose.graph.yml up -d

# The API reads environment variables only (no .env file). In the API's terminal:
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/sutram_forensics
export GRAPH_ENABLED=true NEO4J_URI=http://127.0.0.1:7474 NEO4J_USER=neo4j NEO4J_PASSWORD=forensic_graph_password_2026
cargo run --bin sutram-backend-rust
```

The API runs its migrations on startup. `yukti doctor` should now show the database and graph as connected. The [setup guide](docs/development/setup-guide.md#5-optional-postgresql-and-neo4j) has PowerShell equivalents.

</details>

## Repository layout

```text
.
├── backend-rust/        Rust API (Axum, SQLx): ingestion, extraction, correlation, graph, reconstruction
├── apps/web/            Investigator workspace (React 18, TypeScript, Vite, Cytoscape)
├── cli/                 `yukti` command-line client (Rust, HTTP-only)
├── backend/             Original Python FastAPI service (reference implementation)
├── decision-service/    Node.js service reserved for on-device AI inference
├── shared/              Canonical data contracts: schemas, enums, examples (team-governed)
├── data/synthetic/      Synthetic sample evidence, with SHA-256 checksums
├── docs/                Architecture, API, decision records, development guides
└── docker-compose.graph.yml   PostgreSQL 16 + Neo4j 5 for local development
```

## Testing

Each component has its own test suite, and every suite runs in GitHub Actions CI.

| Component | Command |
|---|---|
| Rust API | `cd backend-rust && cargo test` |
| CLI | `cd cli && cargo test` |
| Web | `cd apps/web && npm test && npm run build` |
| Python service | `cd backend && uv run pytest` |
| Decision service | `cd decision-service && npm test` |

## Documentation

| Start here | |
|---|---|
| [Documentation index](docs/README.md) | Everything, organised by topic |
| [System overview](docs/architecture/system-overview.md) | Architecture and forensic principles |
| [Setup guide](docs/development/setup-guide.md) | A full developer environment |
| [Reconstruction API](docs/api/reconstruction-api.md) | Request and response formats |
| [CLI reference](cli/README.md) | Every command, exit code and JSON format |
| [Demo report](docs/demo/README.md) | A sample forensic dossier from synthetic data |

## Forensic integrity principles

1. **Evidence is immutable.** Originals are never modified. Processing reads from the raw store and writes to a separate derived store.
2. **Cryptographic provenance.** Every file is hashed (SHA-256) on ingestion, and every finding links back to the evidence behind it.
3. **Observed vs. inferred.** Assumed timezones, clock offsets and tower-derived coordinates are labelled, never silently defaulted.
4. **No identity fabrication.** Similarity and heuristic confidence are never presented as proof.
5. **Synthetic data only.** No real victim or case data is ever used in development or tests.

## Team

| Role | Focus |
|---|---|
| **Member 1:** AI/ML and evidence intelligence | Extraction, entity resolution, cross-source similarity |
| **Member 2:** Backend and data engineering | API, PostgreSQL, Neo4j, ingestion, graph persistence |
| **Member 3:** Frontend and visualisation | Investigator workspace, graph and timeline UI |
| **Member 4:** Forensic intelligence and integration | Reconstruction, contradiction detection, video intelligence |

Contributions follow a single-branch workflow on `main`. See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).
