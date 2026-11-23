# Developer Environment Setup Guide

How to set up and run every part of YUKTI on your machine.

The minimum to see the product working is the **Rust API** plus the **web workspace** (sections 2 and 3). The other sections are optional.

| Component | Folder | Runs on | Needed for |
|---|---|---|---|
| Rust API | `backend-rust/` | `http://127.0.0.1:8080` | Everything: the web app and CLI talk to it |
| Web workspace | `apps/web/` | `http://localhost:5173` | The investigator UI |
| `yukti` CLI | `cli/` | your terminal | Driving the API from the command line |
| Python service | `backend/` | `http://localhost:8000` | Reference algorithms and their tests |
| Decision service | `decision-service/` | `http://localhost:3001` | Future on-device AI inference |
| PostgreSQL and Neo4j | `docker-compose.graph.yml` | `:5432`, `:7474` | Optional persistence and knowledge graph |

---

## 1. Prerequisites

- **Git** 2.40+
- **Rust** (stable, via [rustup](https://rustup.rs)). On Windows, use the MSVC toolchain; see the [build portability notes](../architecture/rust-build-portability-audit.md) if linking fails.
- **Node.js** 22 LTS and **npm** 10+
- *Optional:* **Docker**, for PostgreSQL 16 and Neo4j 5
- *Optional:* **Python** 3.12 and [**uv**](https://astral.sh/uv), for the Python service

---

## 2. Rust API (`backend-rust/`)

```bash
cd backend-rust
cargo test                                # run the test suite
cargo run --bin sutram-backend-rust       # start the API
```

- Use `--bin sutram-backend-rust`. The crate also contains the `graph-sync` tool, so plain `cargo run` can't choose which program to run.
- Check it's up: `http://127.0.0.1:8080/health` returns `{"status":"ok",...}`.
- Check dependency status: `http://127.0.0.1:8080/health/ready`.
- **No database needed.** Without PostgreSQL the API runs in *filesystem mode*. Cases and evidence are stored under `data/evidence/`, relative to the folder you start the API from (so `backend-rust/data/evidence/`). Readiness reports `degraded`, which is expected.

**Configuration.** The API reads **process environment variables only**. It does not load a `.env` file. Every setting and its default is listed in [`backend-rust/.env.example`](../../backend-rust/.env.example). The common ones:

| Variable | Default | Purpose |
|---|---|---|
| `HOST`, `PORT` | `127.0.0.1`, `8080` | Listen address |
| `EVIDENCE_STORAGE_PATH` | `data/evidence/raw` | Immutable raw evidence store |
| `DERIVED_STORAGE_PATH` | `data/evidence/derived` | Normalised artefacts |
| `DATABASE_URL` | unset (filesystem mode) | PostgreSQL connection string |
| `GRAPH_ENABLED`, `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD` | disabled | Neo4j knowledge graph |

---

## 3. Web workspace (`apps/web/`)

```bash
cd apps/web
npm ci
npm test                 # unit tests
npm run build            # type check and production build
npm run dev              # http://localhost:5173
```

The app calls the Rust API at `http://localhost:8080` by default. To use another address, create `apps/web/.env.local` from `apps/web/.env.example` and set `VITE_API_URL`. Vite reads `.env` files from `apps/web/` only, never the repository root. See the [frontend API client](../architecture/frontend-api-client.md) for details.

---

## 4. `yukti` CLI (`cli/`)

```bash
cd cli
cargo test
cargo install --path .   # puts `yukti` on your PATH
yukti doctor             # checks configuration, API, database, graph
```

The [CLI getting-started guide](../../cli/GETTING_STARTED.md) walks through a full synthetic case.

---

## 5. Optional: PostgreSQL and Neo4j

Start both databases:

```bash
docker compose -f docker-compose.graph.yml up -d
```

Then set the variables in the terminal you start the API from. The values below match `docker-compose.graph.yml`.

**bash:**

```bash
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/sutram_forensics
export GRAPH_ENABLED=true NEO4J_URI=http://127.0.0.1:7474 NEO4J_USER=neo4j NEO4J_PASSWORD=forensic_graph_password_2026
cargo run --bin sutram-backend-rust
```

**PowerShell:**

```powershell
$env:DATABASE_URL = "postgres://postgres:postgres@localhost:5432/sutram_forensics"
$env:GRAPH_ENABLED = "true"; $env:NEO4J_URI = "http://127.0.0.1:7474"
$env:NEO4J_USER = "neo4j"; $env:NEO4J_PASSWORD = "forensic_graph_password_2026"
cargo run --bin sutram-backend-rust
```

The API runs its database migrations on startup. `yukti doctor` should now report the database and graph as `connected`.

To copy committed records into Neo4j, run `cargo run --bin graph-sync -- --once`.

---

## 6. Optional: Python service (`backend/`)

The original FastAPI implementation. It's kept as the reference for some algorithms and is still tested in CI.

```bash
cd backend
uv sync
uv run pytest
uv run uvicorn app.main:app --reload --port 8000   # http://localhost:8000/docs
```

It reads `.env` from the folder it starts in, so `backend/.env`. The repository-root `.env.example` lists its variables. Nothing else connects to this service by default.

---

## 7. Optional: Decision service (`decision-service/`)

A Node.js shell reserved for on-device AI inference (Laya / ONNX).

```bash
cd decision-service
npm ci
npm test
npm run dev              # http://localhost:3001/health
```
