# YUKTI — System Runbook & Operator Guide

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** RUN-P2C-012  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. System Requirements & Prerequisites

To run the complete YUKTI forensic intelligence platform locally, ensure the following runtimes and tools are installed:

- **Operating System:** Windows 10/11, Ubuntu 22.04 LTS+, or macOS Sonoma+
- **Rust Toolchain:** Rust 1.82+ (`rustc`, `cargo`, `rustfmt`, `clippy`)
- **Node.js Environment:** Node.js 20+ or 22 LTS, `npm` 10+
- **Python (Reference Tests):** Python 3.12+ (with `pytest`)
- **Container Infrastructure (Optional for Live DB):** Docker Desktop 4.25+ with Docker Compose v2

---

## 2. Quick-Start Guide (Zero-Docker Development Mode)

YUKTI is engineered to run in a resilient zero-infrastructure mode using filesystem storage and in-memory graph models.

### Step 1: Start the Rust Production Backend
```powershell
# Open Terminal 1
cd C:\Users\krish\Desktop\SutramPrototype\backend-rust

# Run in debug mode (or --release for maximum throughput)
cargo run
```
- **Port:** `8080`
- **Health Check:** `http://localhost:8080/health` (Returns `{"status": "UP", "version": "0.1.0"}`)
- **Evidence Storage:** Automatically writes to `data/raw/` and `data/derived/`

### Step 2: Start the React Investigator Frontend
```powershell
# Open Terminal 2
cd C:\Users\krish\Desktop\SutramPrototype\apps\web

# Install dependencies (if not already installed)
npm install

# Start Vite development server
npm run dev
```
- **Port:** `5173`
- **Application URL:** `http://localhost:5173`
- **Investigator Workspace:** Navigate to `http://localhost:5173/workspace`

---

## 3. Production Stack Guide (With PostgreSQL & Neo4j Containers)

When Docker Desktop is running, launch the persistent database tier:

### Step 1: Start Database Containers
```powershell
cd C:\Users\krish\Desktop\SutramPrototype

# Start PostgreSQL 16 and Neo4j 5.26
docker compose -f docker-compose.graph.yml up -d
```
- **PostgreSQL:** Port `5432` (User: `sutram_admin`, Database: `sutram_forensics`)
- **Neo4j Bolt:** Port `7687` (User: `neo4j`, Password: `sutram_forensics_2026`)
- **Neo4j Browser:** `http://localhost:7474`

### Step 2: Configure Backend Environment Variables
Create or verify `.env` in `backend-rust/`:
```ini
DATABASE_URL=postgres://sutram_admin:sutram_secret_2026@localhost:5432/sutram_forensics
NEO4J_URI=bolt://localhost:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=sutram_forensics_2026
PORT=8080
RUST_LOG=info
```

### Step 3: Run Database Migrations (Automated on Startup)
When `DATABASE_URL` is detected, `sutram-backend-rust` automatically executes pending migrations in `backend-rust/migrations/` on startup.

---

## 4. Executing the Quality & Verification Suites

### 4.1 Rust Backend Checks
```powershell
cd C:\Users\krish\Desktop\SutramPrototype\backend-rust

# Check formatting
cargo fmt --all -- --check

# Run Clippy linter with strict warning denials
cargo clippy --all-targets -- -D warnings

# Execute all 117 unit and integration tests
cargo test --locked

# Build optimized release binary
cargo build --release --locked
```

### 4.2 Web Frontend Checks
```powershell
cd C:\Users\krish\Desktop\SutramPrototype\apps\web

# Execute 264 Vitest tests
npm test

# Run TypeScript compilation and production bundle build
npm run build
```

### 4.3 Python Reference Checks
```powershell
cd C:\Users\krish\Desktop\SutramPrototype
python -m pytest backend/tests
```

---

## 5. Live Demonstration Script for Hackathon Judges

1. **Open Frontend:** Browse to `http://localhost:5173`.
2. **Examine Cinematic Landing:** Point out the problem statement (multimodal digital evidence correlation) and team identity.
3. **Open Workspace:** Click "Launch Investigator Workspace" (`/workspace`).
4. **Examine Default Benchmark Case (`CASE-026`):**
   - Click **Knowledge Graph Tab**: Show Cytoscape graph, color-coded node taxonomy, and solid vs dashed edges (observed vs inferred).
   - Click **Timeline Tab**: Demonstrate chronological event sequencing and half-open uncertainty intervals.
   - Click **Contradictions Tab**: Highlight the detected impossible speed contradiction ($7000+\text{ km/h}$) and neutral explanatory text (DEC-006).
   - Click **Reports Tab**: Showcase the amber **`SHIP_EXPORT_BLOCKED`** badge guarding court submission.
5. **Demonstrate Real Evidence Upload:**
   - Click **Upload Evidence Tab**.
   - Select a new test CSV or chat JSON file.
   - Click "Upload & Ingest Evidence".
   - Show streaming progress indicator, resulting green success badge, verified SHA-256 hash, and assigned run ID.
   - Click "View in Knowledge Graph" to observe the updated graph.

---

## 6. Troubleshooting Common Issues

| Symptom | Probable Cause | Corrective Action |
|---|---|---|
| Frontend displays network error on upload | Backend not running on port 8080 | Verify Terminal 1 running `cargo run` in `backend-rust`. Check `http://localhost:8080/health`. |
| Upload fails with `413 Payload Too Large` | Exhibit exceeds 100 MB default limit | Configure `MAX_UPLOAD_BYTES` in backend config or split file. |
| Docker daemon connection error | Docker Desktop service stopped | Start Docker Desktop application from Windows Start Menu. |
| Port 8080 already in use | Another process running on port 8080 | Identify via `netstat -ano \| findstr 8080` and terminate PID, or set `PORT=8081` and update Vite proxy. |
