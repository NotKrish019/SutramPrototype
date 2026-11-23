# Team Onboarding & 4-Member Responsibility Matrix

Welcome to the **YUKTI** engineering team (SUTRAM 2026 — ISEA-IGDTUW National Hackathon). This document specifies member roles, module ownership, and day-to-day operations.

---

## 1. Member Ownership Matrix

```mermaid
graph TD
    M1["Member 1: AI/ML & Evidence Intelligence"] -->|Owns| MOD1["extraction & correlation"]
    M2["Member 2: Backend & Data Engineering"] -->|Owns| MOD2["api, ingestion, persistence, graph"]
    M3["Member 3: Frontend & Visualization"] -->|Owns| MOD3["apps/web/"]
    M4["Member 4: Forensic Intelligence & Integration"] -->|Owns| MOD4["reconstruction & decision-service/"]
```

The application backend is now the Rust API in `backend-rust/src/` ([ADR 001](../architecture/rust-backend-migration.md)). Each backend domain below lists its Rust module first, then the matching module in the original Python service (`backend/app/`), which is kept as a reference.

### Member 1: AI/ML and Evidence Intelligence
- **Primary Domain:** `backend-rust/src/extraction/` and `backend-rust/src/correlation/` (Python: `backend/app/modules/extraction/`, `correlation/`)
- **Responsibilities:**
  - Optical Character Recognition (PaddleOCR) for seized documents and mobile screenshots.
  - Named Entity Recognition (spaCy NER) for identifying persons, organizations, phone numbers, and locations.
  - Event extraction and relationship parsing.
  - Cross-source entity resolution (Sentence Transformers, fuzzy matching, deduplication).
  - Cross-modality similarity scoring.

### Member 2: Backend and Data Engineering
- **Primary Domain:** `backend-rust/src/api/`, `ingestion/`, `persistence/`, `graph/`, `migrations/`, PostgreSQL and Neo4j (Python: `backend/app/api/`, `models/`, `services/`)
- **Responsibilities:**
  - API routing (Axum), authentication, and core application configuration.
  - Relational schema design, migrations, and PostgreSQL persistence for Case and Evidence records.
  - Neo4j graph schema design, Cypher queries, and graph synchronization.
  - Secure file ingestion endpoints, SHA-256 integrity verification, and storage paths.
  - Backend integration testing and API performance tuning.

### Member 3: Frontend and Visualization
- **Primary Domain:** `apps/web/`
- **Responsibilities:**
  - Interactive investigator dashboard (React + TypeScript + Vite + Tailwind CSS).
  - Multi-file evidence drag-and-drop upload interface with progress & checksum indicators.
  - Knowledge graph interactive visualization (Cytoscape / Force Graph / D3).
  - Multi-track chronological timeline visualization.
  - Geospatial mapping interface for movement reconstruction.
  - Investigative report viewing and PDF export rendering.

### Member 4: Forensic Intelligence and Integration
- **Primary Domain:** `backend-rust/src/reconstruction/` and `decision-service/` (Python: `backend/app/modules/reconstruction/`)
- **Responsibilities:**
  - Video and CCTV frame extraction, timestamp OCR, and motion event detection (OpenCV).
  - Timestamp normalization across inconsistent timezones and device clocks.
  - Spatial movement reconstruction and pathing.
  - Contradiction detection engine (alibi discrepancy analysis, mutually exclusive statements).
  - Decision service integration (Laya / ONNX runtime for hypothesis validation).
  - End-to-end integration and system sanity checks.

---

## 2. Onboarding Steps for Every Member

### Step 1: Clone the Single Source of Truth
```bash
git clone https://github.com/NotKrish019/SutramPrototype.git
cd SutramPrototype
```

### Step 2: Toolchain Setup
The [setup guide](setup-guide.md) covers every component in detail, including environment variables and the optional databases.

- **Rust API:**
  ```bash
  cd backend-rust
  cargo test
  cargo run --bin sutram-backend-rust    # http://127.0.0.1:8080
  ```
- **React Frontend:**
  ```bash
  cd apps/web
  npm ci
  npm run test
  ```
- **CLI:**
  ```bash
  cd cli
  cargo install --path .
  yukti doctor                           # checks the whole setup
  ```
- **Python Service** (reference implementation):
  ```bash
  cd backend
  uv sync
  uv run pytest
  ```
- **Decision Service:**
  ```bash
  cd decision-service
  npm ci
  npm run test
  ```

---

## 3. Collaborative Rules

1. **One Monorepo, One Branch:** All 4 members work in the same folder structure on branch `main`. Never initialize a separate project or alternate branch.
2. **Schema Consensus:** If you need to add or adjust a field in `Case`, `Evidence`, `Entity`, `Event`, or `Relationship`, post a proposal in the shared channel and update `shared/schemas/README.md` only after team consensus.
3. **Respect Module Boundaries:** Do not edit another member's module files without checking in.
4. **Clean Commits:**
   ```bash
   git pull --rebase origin main
   git commit -m "feat(module): description"
   git pull --rebase origin main
   git push origin main
   ```
