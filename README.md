<div align="center">

<img src="docs/assets/yukti-logo.png" alt="YUKTI logo" width="100%"/>

<br/>

[![Continuous Integration](https://github.com/NotKrish019/SutramPrototype/actions/workflows/ci.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/ci.yml)
[![Rust Backend CI](https://github.com/NotKrish019/SutramPrototype/actions/workflows/backend-rust.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/backend-rust.yml)
[![CLI CI](https://github.com/NotKrish019/SutramPrototype/actions/workflows/cli.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/cli.yml)

**[Quick start](#-quick-start)** · **[How it works](#-how-it-works)** · **[Web workspace](#-using-the-web-workspace)** · **[CLI](#-using-the-yukti-cli)** · **[API](#-using-the-api-directly)** · **[Docs](#-documentation)**

📦 **[Requirements](REQUIREMENTS.md)** · 📘 **[Full project documentation](docs/PROJECT_DOCUMENTATION.md)**

</div>

---

## 📌 What is YUKTI?

Investigations today run on fragmented evidence: call detail records (CDRs), chat exports, CCTV footage and seized devices, each analysed on its own. Lining up thousands of timestamps, phone numbers and cell towers by hand is slow, and it's easy to miss the one contradiction that matters.

**YUKTI** turns that evidence into one reconstructed account of events:

1. **Ingest** each file and fingerprint it with SHA-256. The original is never modified.
2. **Extract** events and identifiers (phone numbers, IMEI, IMSI, towers, timestamps).
3. **Correlate** the same people, devices and places across sources into a knowledge graph.
4. **Reconstruct** what happened as a corrected timeline, a spatial trajectory and a list of contradiction leads.
5. **Report** a judicial dossier where every finding traces back to the exact evidence behind it.

> **Forensic stance.** YUKTI separates what was **observed** from what was **inferred**. An assumed timezone, an undeclared clock offset or a location estimated from a cell tower is always labelled. A similarity score is never presented as proof of identity. Judicial export is **blocked** while any finding still rests on such assumptions.

Built for **SUTRAM 2026**, the ISEA–IGDTUW National Hackathon. All bundled data is synthetic.

## ✨ Features

| | Capability | What it gives the investigator | Status |
|---|---|---|---|
| 📥 | **Evidence ingestion** | Content-validated upload, SHA-256 fingerprint, immutable raw store, declared timezone and clock offset per exhibit | ✅ |
| 🔎 | **Extraction** | CDR and chat-export parsing into normalised events and identifiers | ✅ |
| 🔗 | **Correlation** | Entity resolution across sources; a Neo4j knowledge graph of people, devices and places | ✅ |
| 🕒 | **Temporal reconstruction** | Every source on one corrected clock, with time windows and uncertain events flagged | ✅ |
| 🗺️ | **Spatial reconstruction** | Trajectories and movement segments, checked against speed thresholds | ✅ |
| ⚠️ | **Contradiction detection** | Conflicts raised as *leads*, never verdicts, each with a neutral explanation and its limitations | ✅ |
| 🧾 | **Provenance, gaps, what-if** | Trace any finding to its evidence, find missing evidence, test alternative assumptions | ✅ |
| ⚖️ | **Judicial report** | A forensic dossier with a disclaimer and an export safeguard | ✅ |
| 🖥️ | **Investigator workspace** | A web dashboard covering the whole workflow | ✅ |
| ⌨️ | **`yukti` CLI** | The same workflow from the terminal, plus a `doctor` setup check | ✅ |
| 🎥 | **Video intelligence** | Keyframe handoff and frame provenance; full CCTV analysis is planned | 🟡 Partial |
| 🧠 | **OCR, NER, on-device decision support** | PaddleOCR, spaCy, Laya / ONNX | 🔜 Planned |

---

## 🏗️ How it works

### System architecture

The **Rust API** does all the forensic work. The **web workspace** and the **`yukti` CLI** are two ways of driving it over HTTP; neither contains any forensic logic. The databases are optional: without them, the API keeps everything on disk.

```mermaid
flowchart LR
    subgraph Clients["You use YUKTI through"]
        W["🖥️ Web workspace<br/>apps/web · :5173"]
        C["⌨️ yukti CLI<br/>cli/"]
        H["🔌 Any HTTP client<br/>curl, scripts"]
    end
    subgraph API["Rust API · backend-rust/ · :8080"]
        direction TB
        I["Ingestion"] --> E["Extraction"] --> R["Correlation"] --> T["Reconstruction"] --> P["Report"]
    end
    W -- HTTP --> API
    C -- HTTP --> API
    H -- HTTP --> API
    API --> FS[("📁 Evidence store<br/>raw · derived")]
    API -. optional .-> PG[("🐘 PostgreSQL 16")]
    API -. optional .-> N4[("🕸️ Neo4j 5")]
    PG -. graph-sync .-> N4
```

### The evidence pipeline

Each stage reads the previous stage's output and writes new records. Nothing goes back and modifies the original evidence.

```mermaid
flowchart TD
    A["📄 Evidence file<br/>CDR .csv · chat .json"] --> B{"Content valid?"}
    B -- no --> X["❌ Rejected<br/>HTTP 422, nothing stored"]
    B -- yes --> C["🔒 Ingestion<br/>SHA-256 computed<br/>original stored read-only<br/>timezone + clock offset recorded"]
    C --> D["🔎 Extraction<br/>events + identifiers<br/>phone · IMEI · IMSI · tower"]
    D --> E["🔗 Correlation<br/>same entity across sources<br/>→ knowledge graph"]
    E --> F["🕒 Temporal<br/>one corrected clock<br/>ordered timeline"]
    E --> G["🗺️ Spatial<br/>trajectories<br/>speed checks"]
    F --> H["⚠️ Contradictions<br/>physically impossible<br/>combinations → leads"]
    G --> H
    F --> I["🏷️ Assumptions<br/>every inferred value labelled"]
    G --> I
    H --> J["⚖️ Judicial report<br/>+ export safeguard"]
    I --> J
```

### The export safeguard

The judicial report says whether the reconstruction is fit to export. Export is **blocked** until a person has resolved anything the system can't vouch for:

```mermaid
flowchart TD
    S(["Generate report"]) --> Q1{"Unresolved<br/>contradiction leads?"}
    Q1 -- yes --> B1["🔴 BLOCKED<br/>investigator must resolve each lead"]
    Q1 -- no --> Q2{"Events excluded?<br/>bad timestamp or unknown timezone"}
    Q2 -- yes --> B2["🔴 BLOCKED<br/>fix or declare the source's timezone"]
    Q2 -- no --> Q3{"Findings rest on<br/>unobserved values?"}
    Q3 -- yes --> B3["🔴 BLOCKED<br/>each assumption is listed"]
    Q3 -- no --> OK["🟢 CLEARED for judicial export"]
```

With the bundled sample data the report is **BLOCKED**, and that's intended. The samples don't declare a clock offset, and their locations come from cell towers, so YUKTI says so instead of guessing.

---

## 🚀 Quick start

**You need:** Git, [Rust](https://rustup.rs) (stable, 1.82+) and [Node.js 22 LTS](https://nodejs.org). Docker is optional. The full list of software, libraries and hardware is in **[REQUIREMENTS.md](REQUIREMENTS.md)**.

```mermaid
flowchart LR
    A["1 · Clone"] --> B["2 · Start the API<br/>:8080"] --> C["3 · Start the web app<br/>:5173"] --> D["4 · Install the CLI"] --> E["5 · yukti doctor"]
    E -- all PASS / WARN --> F(["✅ Ready"])
    E -- any FAIL --> G["Follow the hint<br/>on the FAIL line"] --> E
```

```bash
# 1. Clone
git clone https://github.com/NotKrish019/SutramPrototype.git
cd SutramPrototype

# 2. Start the API (terminal 1, keep it running)
cd backend-rust
cargo run --bin sutram-backend-rust          # → http://127.0.0.1:8080

# 3. Start the web workspace (terminal 2)
cd apps/web
npm ci
npm run dev                                  # → http://localhost:5173

# 4. Install the CLI (terminal 3)
cd cli
cargo install --path .

# 5. Check everything is connected
yukti doctor
```

`yukti doctor` should finish with **`OK with warnings`**. The warnings only mean PostgreSQL and Neo4j aren't running, which is fine: YUKTI works fully without them.

<details>
<summary><b>Optional: add PostgreSQL and Neo4j</b></summary>

```bash
docker compose -f docker-compose.graph.yml up -d

# The API reads environment variables only (it does not load a .env file).
# In the API's terminal, before starting it:
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/sutram_forensics
export GRAPH_ENABLED=true NEO4J_URI=http://127.0.0.1:7474 NEO4J_USER=neo4j NEO4J_PASSWORD=forensic_graph_password_2026
cargo run --bin sutram-backend-rust
```

- The API runs its database migrations on startup.
- `yukti doctor` now shows the database and graph as `connected`.
- To copy records into the graph, run `cargo run --bin graph-sync -- --once` in `backend-rust/`.
- The [setup guide](docs/development/setup-guide.md#5-optional-postgresql-and-neo4j) has PowerShell equivalents.

</details>

---

## 🕵️ Your first investigation, end to end

The commands below take a synthetic case from nothing to a judicial report. Run them from the repository root, with the API running.

```mermaid
sequenceDiagram
    autonumber
    actor You
    participant CLI as yukti CLI
    participant API as Rust API
    participant Store as Evidence store
    You->>CLI: yukti case create --title "..."
    CLI->>API: POST /api/v1/cases
    API-->>You: case UUID
    loop for each exhibit (CDR .csv, chat .json)
        You->>CLI: yukti evidence ingest --case UUID FILE --auto-extract
        CLI->>API: GET case (exists?) then POST /evidence/upload
        API->>Store: write original read-only, SHA-256
        API-->>You: receipt + hash + records extracted (5 CDR, 3 chat)
    end
    You->>CLI: yukti reconstruction run --case UUID
    CLI->>API: POST /cases/UUID/reconstruction
    API->>Store: load events, rebuild timeline, spatial, leads
    API-->>You: 8 events · 5 trajectories · export BLOCKED (with reasons)
    You->>CLI: yukti reconstruction explain --case UUID
    CLI->>API: GET reconstruction + GET report
    API-->>You: full timeline + judicial dossier
```

```bash
# 1. Open a case. Copy the UUID it prints.
yukti case create --title "Warehouse burglary" --fir-number "FIR-112/2026"
CASE=<paste-uuid>                                # PowerShell: $CASE = "<paste-uuid>"

# 2. Ingest the two synthetic exhibits, declaring their timezone
yukti evidence ingest --case $CASE data/synthetic/sample_cdr.csv         --source-timezone Asia/Kolkata --auto-extract
yukti evidence ingest --case $CASE data/synthetic/sample_chat_export.json --source-timezone Asia/Kolkata --auto-extract
yukti evidence list --case $CASE

# 3. Reconstruct, then read the full detail and the report
yukti reconstruction run     --case $CASE
yukti reconstruction explain --case $CASE
```

Then open **http://localhost:5173/workspace** to see the same case in the investigator UI.

---

## 🖥️ Using the web workspace

Open **http://localhost:5173**. The public site is at `/`, and the investigator workspace is at **`/workspace`**.

- **Live data.** When the API is reachable, the workspace shows live data from it.
- **Demo mode.** When the API is not reachable, the workspace uses built-in synthetic demo cases, so the UI can always be explored.
- **Status.** A status indicator shows which of the two modes you're in.

Inside a case, the sidebar follows the investigator's workflow:

```mermaid
flowchart LR
    O["📂 Overview"] --> S1["1 · 📥 Evidence Intake"] --> S2["2 · 🕒 Temporal"] --> S3["3 · 🗺️ Spatial"] --> S4["4 · ⚠️ Contradictions"] --> S5["5 · 🎥 Video"] --> S6["6 · ⚖️ Judicial Report"]
    S2 -.-> G["🕸️ Knowledge Graph"]
    S2 -.-> K["🔗 Evidence Correlation"]
```

| Section | URL (`/workspace/cases/<id>/…`) | What it does | How to use it |
|---|---|---|---|
| **Investigation Overview** | *(case root)* | What happened, what conflicts, what to do next | Start here. It summarises the case and links to whatever needs attention. |
| **1 · Evidence Intake** | `evidence` | Exhibits, hashes and chain of custody | Drag in CDR or chat files. Each is uploaded, hashed by the API and extracted. Check that the SHA-256 matches your seizure record. |
| **2 · Temporal Reconstruction** | `timeline` | Every source on one corrected clock | Read events in order. Badges mark uncertain or concurrent events and any assumed timezone or offset. |
| **3 · Spatial Reconstruction** | `spatial` | Where evidence places each event | Follow each trajectory. Segments faster than the speed threshold are flagged. |
| **4 · Contradiction Intelligence** | `contradictions` | Claims the evidence can't reconcile | Review each lead with its explanation and limitations. These are leads for a person to resolve, not conclusions. |
| **5 · Video Intelligence** | `video` | Keyframes, hashes and frame provenance | Inspect handed-off frames and their hashes. |
| **6 · Judicial Report** | `reports` | Evidence to conclusion, with every limit stated | Read the dossier and the export status. If it's BLOCKED, the reasons tell you what to resolve. |
| **Knowledge Graph** | `graph` | People, devices and places as connected records | Click a node to see its details and neighbours, and trace paths between entities. |
| **Evidence Correlation** | `correlation` | Which exhibits support each other | See where independent sources agree or disagree. |

**Configuration.** By default the web app calls the API at `http://localhost:8080`. To change that, copy `apps/web/.env.example` to `apps/web/.env.local` and set `VITE_API_URL`.

---

## ⌨️ Using the `yukti` CLI

`yukti` drives the API from a terminal, using the same endpoints as the web app. Install it on macOS or Linux with
`curl -fsSL https://raw.githubusercontent.com/NotKrish019/SutramPrototype/main/cli/install/install.sh | sh`, or on Windows with
`irm https://raw.githubusercontent.com/NotKrish019/SutramPrototype/main/cli/install/install.ps1 | iex`. Then type `yukti`: it starts the bundled local engine and opens an interactive session. The step-by-step guide for Mac and Windows users is [`docs/guides/YUKTI-Installation-and-Usage-Guide.pdf`](docs/guides/YUKTI-Installation-and-Usage-Guide.pdf).

**Try it in one command:** `yukti demo` runs a guided tour on a synthetic case. It covers ingest, reconstruction, breaking-point analysis, a what-if, cross-examination, a cited question and the court package.

```mermaid
flowchart TD
    Y(["yukti"]) --> SETUP["status · doctor · config · server"]
    Y --> CA["case<br/>list · create · show · overview"]
    Y --> EV["evidence<br/>ingest · list · show · verify<br/>extract · reprocess · extraction · runs"]
    Y --> AN["reconstruction · timeline · spatial<br/>contradictions · gaps · provenance · graph"]
    Y --> AS["robustness · challenge · whatif<br/>ask · replay"]
    Y --> CT["custody verify · certificate · report"]
    Y --> TOUR["demo · shell"]
```

### Command reference

Commands that take `--case` also read `YUKTI_CASE`, so `export YUKTI_CASE=<uuid>` sets a working case.

| Command | What it does |
|---|---|
| `yukti` / `yukti shell` | Interactive session; `use <uuid>` sets the working case. Starts the bundled local engine if needed. |
| `yukti demo` | Guided demonstration on the synthetic case in `data/synthetic/demo/`. |
| `yukti doctor` · `status` · `config` · `server start\|stop\|status\|logs` | Setup checklist, health, settings and the local engine. |
| `yukti case list\|create\|show\|overview` | Cases; `overview` is a one-screen investigative brief. |
| `yukti evidence ingest FILE` | Uploads one exhibit unmodified, then compares the API's SHA-256 with the bytes sent (exit `7` on mismatch). Reads CDR CSV, chat JSON and **WhatsApp `.txt` exports**. |
| `yukti evidence list\|show\|verify\|extract\|reprocess\|extraction\|runs` | Exhibits, chain of custody, local-copy verification and versioned extraction. |
| `yukti reconstruction run\|explain` · `timeline` · `spatial` · `contradictions` · `gaps` · `provenance` | The reconstruction and each of its parts. |
| `yukti robustness` (alias `breaking-point`) | **How wrong an assumption must be before each lead dissolves**: clock, location and threshold margins, each verified by recomputation, classed ROBUST / SENSITIVE / FRAGILE. |
| `yukti challenge` | **The case as defence counsel would attack it**: weakest points first, each with what would close it. |
| `yukti whatif` | Hypothetical scenario (clock offsets, timezone, threshold, excluded evidence) compared with the baseline; nothing is stored. |
| `yukti ask "…"` | Plain-English questions answered only from the evidence, citing file, record and the timestamp as written; refuses when the evidence does not answer. No language model. |
| `yukti replay` | The timeline played back event by event on a terminal map, with leads flagged. |
| `yukti custody verify` | Re-hashes every stored exhibit, checks the custody ledger (PostgreSQL) and prints the **case seal** with a QR code. |
| `yukti certificate` | Drafts the **Section 63 certificate (Bharatiya Sakshya Adhiniyam, 2023)** for the case's exhibits, with hashes, audit result and seal, for an officer and an expert to complete and sign. |
| `yukti report --out FILE` | Exports the judicial dossier and prints its SHA-256. |
| `yukti graph summary\|neighbors\|path` | Knowledge-graph queries. |

**`evidence ingest` options**

| Option | Purpose |
|---|---|
| `--source-timezone <TZ>` | Declares the exhibit's timezone, for example `Asia/Kolkata`. If you leave it out, it's recorded as *not declared*, never silently set to UTC. |
| `--clock-offset-seconds <N>` | Declares how far the source device's clock was off. |
| `--auto-extract` | Extracts events immediately and shows the record counts. |
| `--source-type <TYPE>` | Overrides the type the API detects from the content. |
| `--metadata '{json}'` | Extra case notes, for example `'{"seized_by":"INV-7"}'`. |

**Global options.** These work with every command.

| Option | Environment variable | Default |
|---|---|---|
| `--api-url <URL>` | `YUKTI_API_URL` | `http://127.0.0.1:8080` |
| `--timeout <SECONDS>` | `YUKTI_TIMEOUT_SECS` | `10` |
| `--json` | n/a | off: prints one versioned JSON document, for scripting |

**Exit codes.** Use them in scripts and CI.

| Code | Meaning | Typical fix |
|---|---|---|
| `0` | Success | n/a |
| `2` | Invalid input or configuration; nothing was sent | Check the arguments, for example that `--case` is a UUID |
| `3` | API unreachable | Start the API |
| `4` | The API answered 4xx (for example, case not found) | Check the ID |
| `5` | The API answered 5xx, or a dependency isn't ready | Check the API's log, PostgreSQL and Neo4j |
| `6` | Unexpected response | Check that `--api-url` points at the YUKTI API |
| `7` | Integrity check failed: a SHA-256 does not match the recorded one (`evidence ingest`, `evidence verify`, `custody verify`) | Re-acquire the exhibit and compare with the seizure record |
| `69` | Feature not available in the API yet | n/a |

The full reference is in [`cli/README.md`](cli/README.md), and the step-by-step guide is [`cli/GETTING_STARTED.md`](cli/GETTING_STARTED.md).

---

## 🔌 Using the API directly

Everything the web app and CLI do is available over plain HTTP at `http://127.0.0.1:8080`.

| Area | Method and path | What it does |
|---|---|---|
| **Health** | `GET /health` | Liveness: is the API up? |
| | `GET /health/ready` | Readiness of the database and graph (`ready`, `degraded` or `not_ready`) |
| **Cases** | `GET /api/v1/cases` · `POST /api/v1/cases` | List cases, or create one |
| | `GET /api/v1/cases/:case_id` | One case |
| **Evidence** | `POST /api/v1/evidence/upload` | Multipart upload. Validates, hashes and stores the file, and can extract with `auto_extract=true` |
| | `GET /api/v1/cases/:case_id/evidence` · `GET /api/v1/evidence/:evidence_id` | List a case's evidence, or get one receipt |
| **Extraction** | `POST /api/v1/evidence/:evidence_id/extract` | Run extraction on stored evidence |
| | `POST /api/v1/evidence/:evidence_id/reprocess` | Start a new versioned run with different parameters (timezone, offset, country code) |
| | `GET …/extraction` · `GET …/extraction/runs` | The latest extraction, or the run history |
| **Reconstruction** | `POST /api/v1/cases/:case_id/reconstruction` | Run a full reconstruction |
| | `GET /api/v1/cases/:case_id/reconstruction` | The latest reconstruction |
| | `GET …/timeline` · `GET …/spatial` · `GET …/contradictions` | Each part on its own |
| **Report** | `GET /api/v1/cases/:case_id/report` | The judicial report, dossier and export status |
| **Traceability** | `GET …/reconstruction/provenance` · `GET …/findings/:finding_id/provenance` | Trace any finding back to its evidence and SHA-256 |
| **Gaps** | `GET …/reconstruction/gaps` · `GET …/gaps` | Evidence that's missing or would strengthen the case |
| **What-if** | `POST …/reconstruction/simulate` · `POST …/what-if` | Re-run under alternative assumptions without changing any stored result |
| **Robustness** | `GET …/robustness` | Breaking point of every contradiction lead (clock, location, threshold margins), verified by recomputation |
| **Custody** | `GET …/custody/verify` | Re-hash every stored exhibit, check the custody ledger, and return the case seal |
| **Graph** | `GET …/graph/summary` · `GET …/graph/entities/:entity_id/neighbors` · `GET …/graph/path` | Knowledge-graph queries (needs Neo4j) |

Paths starting with `…/` are under `/api/v1/cases/:case_id` or `/api/v1/evidence/:evidence_id`.

```bash
curl http://127.0.0.1:8080/health/ready
curl -X POST http://127.0.0.1:8080/api/v1/cases -H "Content-Type: application/json" -d '{"title":"Demo"}'
curl -F case_id=<UUID> -F source_timezone=Asia/Kolkata -F auto_extract=true \
     -F file=@data/synthetic/sample_cdr.csv http://127.0.0.1:8080/api/v1/evidence/upload
```

Request and response formats are in the [Reconstruction API guide](docs/api/reconstruction-api.md) and the [OpenAPI specification](docs/api/openapi.json).

---

## 📁 Repository layout

```text
.
├── backend-rust/            Rust API (Axum, SQLx): every forensic stage, persistence, HTTP API
├── apps/web/                Investigator workspace (React 18, TypeScript, Vite, Cytoscape)
├── cli/                     `yukti` command-line client (Rust, HTTP-only)
├── backend/                 Original Python FastAPI service (reference implementation)
├── decision-service/        Node.js service reserved for on-device AI inference
├── shared/                  Canonical data contracts: schemas, enums, examples (team-governed)
├── data/synthetic/          Synthetic sample evidence, with SHA-256 checksums
├── docs/                    Architecture, API, decision records, guides (start at docs/README.md)
└── docker-compose.graph.yml PostgreSQL 16 + Neo4j 5 for local development
```

## 🧪 Testing

Every component has its own test suite, and every suite runs in GitHub Actions CI. At the last full run (24 November 2026), **all 1,236 tests passed**.

| Component | Command |
|---|---|
| Rust API | `cd backend-rust && cargo test` |
| CLI | `cd cli && cargo test` |
| Web workspace | `cd apps/web && npm test && npm run build` |
| Python service | `cd backend && uv run pytest` (or `pip install -r requirements.txt`, then `pytest` in `backend/`) |
| Decision service | `cd decision-service && npm test` |

## 📚 Documentation

| | |
|---|---|
| 📘 [**Project documentation**](docs/PROJECT_DOCUMENTATION.md) ([PDF](docs/YUKTI-Project-Documentation.pdf)) | Problem, objectives, solution, architecture, methodology, implementation, results and future scope, in one document |
| 📦 [**Requirements**](REQUIREMENTS.md) | Software, hardware and every library, with what each is for |
| [Documentation index](docs/README.md) | Everything, organised by topic |
| [System overview](docs/architecture/system-overview.md) | Components, pipeline and implementation status |
| [Setup guide](docs/development/setup-guide.md) | Every component, its configuration and the optional databases |
| [Reconstruction assumptions](docs/architecture/rust-reconstruction-assumptions.md) | How inferred values are labelled, and when export is blocked |
| [CLI getting started](cli/GETTING_STARTED.md) | Install, `doctor` and a full walkthrough |
| [Demo report](docs/demo/README.md) | A sample forensic dossier from synthetic data |

## 🛡️ Forensic integrity principles

1. **Evidence is immutable.** Originals are never modified. Processing reads from the raw store and writes to a separate derived store.
2. **Cryptographic provenance.** Every file is hashed with SHA-256 on ingestion, and every finding links back to the evidence behind it.
3. **Observed vs. inferred.** Assumed timezones, clock offsets and tower-derived coordinates are labelled, never silently defaulted.
4. **No identity fabrication.** Similarity and heuristic confidence are never presented as proof.
5. **Synthetic data only.** No real victim or case data is ever used in development or tests.

## 👥 Team

| Role | Focus |
|---|---|
| **Member 1:** AI/ML and evidence intelligence | Extraction, entity resolution, cross-source similarity |
| **Member 2:** Backend and data engineering | API, PostgreSQL, Neo4j, ingestion, graph persistence |
| **Member 3:** Frontend and visualisation | Investigator workspace, graph and timeline UI |
| **Member 4:** Forensic intelligence and integration | Reconstruction, contradiction detection, video intelligence |

Contributions follow a single-branch workflow on `main`. See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).
