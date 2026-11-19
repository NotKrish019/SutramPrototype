# YUKTI — MVP End-to-End Demonstration Runbook

> **CRITICAL FORENSIC NOTICE:**  
> **SYNTHETIC DEMO DATA — NOT REAL FORENSIC EVIDENCE**  
> All records, telephone numbers, cell tower coordinates, IMEI/IMSI numbers, and chat messages used in this demonstration are synthetically generated for technical evaluation purposes. No real persons or live criminal cases are represented.

---

## 1. Overview & Operating Modes

YUKTI is an AI-powered multimodal digital evidence correlation and crime reconstruction platform built for the **SUTRAM 2026 — ISEA-IGDTUW National Hackathon**.

The platform supports two runtime modes:
1. **Standalone Filesystem Mode (Zero Dependencies — Recommended for Instant Demos):**
   - Backend runs completely in-memory and against local immutable storage in `data/raw/` and derived caches in `data/derived/`.
   - Frontend runs in Vite development mode or serves the production build.
   - Requires only: Rust toolchain (or precompiled binary) and Node.js.
2. **Full Database & Knowledge Graph Mode (With Docker):**
   - Relational persistence in PostgreSQL 16 via SQLx.
   - Knowledge graph projections in Neo4j 5.26 via `neo4rs`.
   - Requires: Docker Desktop with Compose.

---

## 2. Prerequisites & Quick Setup

### System Requirements
- **OS:** Windows 10/11, Linux, or macOS
- **Rust:** `cargo 1.80+` (or precompiled `sutram-backend-rust.exe`)
- **Node.js:** `v20+` or `v22+` with `npm`
- **Optional:** Docker Desktop (if demonstrating live Neo4j / PostgreSQL queries)

---

## 3. Demonstration Walkthrough Steps

### Step 1: Start the Backend Service

Open a terminal in the repository root:
```powershell
cd C:\Users\krish\Desktop\SutramPrototype
```

**Option A (Using Precompiled Release Binary):**
```powershell
.\backend-rust\target\release\sutram-backend-rust.exe
```

**Option B (Building & Running from Cargo):**
```powershell
cargo run --release --manifest-path backend-rust/Cargo.toml
```

*Expected Terminal Output:*
```
[INFO] Starting YUKTI Digital Forensics Backend (Rust Axum)...
[INFO] Environment: development
[INFO] Server listening on http://0.0.0.0:8080 (or PORT from environment)
```

Verify backend health in browser or PowerShell:
```powershell
curl http://localhost:8080/health
```
Response: `{"status":"ok","version":"0.1.0","backend":"rust",...}`

---

### Step 2: Start the React Frontend

Open a second terminal:
```powershell
cd C:\Users\krish\Desktop\SutramPrototype\apps\web
npm run dev
```

*Expected Terminal Output:*
```
  VITE v5.4.14  ready in 450 ms

  ➜  Local:   http://localhost:5173/
  ➜  Network: use --host to expose
```

Open your browser to: **`http://localhost:5173`**

---

### Step 3: Explore the Case Dashboard

1. Navigate to the **Cases** view (`http://localhost:5173/workspace/cases`).
2. Select case **`CASE-026`** (or the uploaded investigation case).
3. Notice the clear indicator:
   ```
   SYNTHETIC DEMO DATA — NOT REAL FORENSIC EVIDENCE
   ```
4. Observe the four core analytical modules in the workspace sidebar:
   - **Timeline (Temporal Reconstruction)**
   - **Spatial Map (Geospatial Analysis)**
   - **Contradiction Review (Lead Generation)**
   - **Knowledge Graph (Cross-Source Correlation)**

---

### Step 4: Demonstrate Live Evidence Ingestion & SHA-256 Verification

To demonstrate live evidence upload, run the following commands or use the UI upload dialog:

```powershell
# Upload Synthetic CDR Exhibit
curl.exe -X POST "http://localhost:8080/api/v1/evidence/upload?auto_extract=true" `
  -F "case_id=CASE-DEMO-001" `
  -F "file=@data/synthetic/sample_cdr.csv"

# Upload Synthetic WhatsApp Chat Exhibit
curl.exe -X POST "http://localhost:8080/api/v1/evidence/upload?auto_extract=true" `
  -F "case_id=CASE-DEMO-001" `
  -F "file=@data/synthetic/sample_chat_export.json"
```

**What to verify:**
- Look at the returned JSON: `sha256_hash` is computed immediately upon ingestion.
- Check `data/raw/CASE-DEMO-001/`: raw files are written once, given read-only attributes, and never modified in place.
- Extraction runs automatically when `?auto_extract=true` is passed.

---

### Step 5: Trigger & Inspect Forensic Case Reconstruction

Trigger full cross-source reconstruction for the case:
```powershell
curl.exe -X POST "http://localhost:8080/api/v1/cases/CASE-DEMO-001/reconstruct"
```

The response combines evidence from CDR and Chat exports into:
1. **Normalized Timeline:** Sorted chronologically with half-open $[start, end)$ intervals and UTC normalization (`observed_at + clock_drift_offset`).
2. **Spatial Trajectories:** Evaluates movement between cell towers using WGS84 Haversine equations.
3. **Contradiction Leads:** Analyzes temporal concurrency and impossible travel speeds.

---

### Step 6: Review the Interactive Timeline Workspace

Navigate to `http://localhost:5173/workspace/timeline`:
- **Chronological Sorting:** All events across telecommunication and chat messages appear on a unified axis.
- **Clock Drift Transparency:** Hover over timestamps to see:
  - Raw Observed Timestamp: `2026-11-18 10:15:00`
  - Applied Offset: `+00:00:00` (or carrier skew)
  - Normalized UTC Timestamp: `2026-11-18T04:45:00.000Z`
- **Filter by Entity:** Filter events by suspect phone number (`+919811002233`) to isolate specific activity.

---

### Step 7: Review the Geospatial Map & Synthetic Centroid Markers

Navigate to `http://localhost:5173/workspace/spatial`:
- **Trajectory Visualization:** Displays the path of entities moving across cell towers.
- **Forensic Centroid Disclaimer:** Inspect cell tower markers. They are explicitly rendered as:
  ```
  TOWER-DEL-001 [SYNTHETIC CENTROID — NOT GPS OBSERVATION]
  ```
  This prevents misinterpreting broad cellular coverage areas as high-precision GPS tracking.

---

### Step 8: Review the Contradiction Review Workspace

Navigate to `http://localhost:5173/workspace/contradictions`:
- Observe the detected contradictions generated by the Rust engine:
  1. **`SPATIAL_TRANSIT_IMPOSSIBLE`**:
     - Flagged velocity: `3441.4 km/h` between two simultaneous events.
     - Neutral hypothesis: *"Investigative Lead: Simultaneous presence detected across distant towers. Possible explanation: SIM cloning, dual-SIM device, or misconfigured carrier timestamp."*
  2. **`TEMPORAL_ACTIVITY_INFEASIBLE`**:
     - Overlapping duration: `1800s` of concurrent active voice calls or meetings.
- Note that all explanations are framed as neutral investigative hypotheses in strict compliance with DEC-004.

---

### Step 9: Explore the Knowledge Graph Workspace

Navigate to `http://localhost:5173/workspace/graph`:
- **Visual Graph Rendering:** Cytoscape-driven graph view depicting entities (Person, Phone, Tower, Account) and relationships (`COMMUNICATED_WITH`, `LOCATED_AT`).
- **Forensic Edge Weighting:** Observed evidence edges carry `weight: null` in compliance with DEC-006 (observed evidence is factual and not given arbitrary numeric weights).
- **Interactive Tools:**
  - Select nodes to view provenance and source evidence exhibits.
  - Trace shortest paths between suspects.
  - Filter by relationship type or timeframe.

---

### Step 10: Verify Report Export Safeguards

Navigate to `http://localhost:5173/workspace/reports`:
- Notice the **`SHIP_EXPORT_BLOCKED`** warning badge on automated export controls.
- Review the mandatory judicial warning:
  > **JUDICIAL CAVEAT:** Automated correlation and reconstruction outputs are investigative decision-support artifacts only. They do not constitute conclusive forensic proof of identity or liability. Final evidentiary reports require mandatory supervisory investigator sign-off and formal chain-of-custody verification.

---

## 4. Optional: Starting Docker for Live Neo4j & PostgreSQL

If the evaluation environment has Docker Desktop installed and running:

1. Start the database containers:
   ```powershell
   docker-compose -f docker-compose.graph.yml up -d
   ```
2. Verify services:
   - PostgreSQL 16: `localhost:5432` (database `sutram_forensics`)
   - Neo4j 5.26: `localhost:7474` (browser) / `localhost:7687` (Bolt protocol)
3. Run SQLx migrations:
   ```powershell
   cd backend-rust
   cargo sqlx migrate run
   ```
4. Restart the backend. It will detect the active database connections and automatically persist all runs to PostgreSQL and project graphs to Neo4j.

---

## 5. Troubleshooting & Verification Commands

| Component | Verification Command | Expected Outcome |
| :--- | :--- | :--- |
| **Rust Tests** | `cargo test --locked --manifest-path backend-rust/Cargo.toml` | `115 passed; 0 failed` |
| **Rust Linting** | `cargo clippy --locked --manifest-path backend-rust/Cargo.toml --tests -- -D warnings` | `0 errors; 0 warnings` |
| **Frontend Tests** | `npm --prefix apps/web test` | `261 passed (18 test files)` |
| **Frontend Build** | `npm --prefix apps/web run build` | `dist/ generated in ~13s` |
| **Python Tests** | `python -m pytest backend/tests` | `144 passed; 50 skipped` |
