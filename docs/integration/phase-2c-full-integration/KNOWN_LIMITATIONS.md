# YUKTI — Known Limitations and Environmental Constraints

**Project:** YUKTI (SUTRAM 2026)  
**Document ID:** LIM-P2C-011  
**Version:** 1.0.0  
**Date:** 2026-11-20  

---

## 1. Transparency & Forensic Integrity Statement

In adherence to Section 4 of the YUKTI Engineering Directives, digital forensics platforms must never fabricate results, mask failures, or claim live database/container connectivity when running under mock or filesystem fallbacks. This document explicitly catalogs all active limitations, environmental constraints, and architectural workarounds present during the Phase 2C integration.

---

## 2. Infrastructure Limitations

### 2.1 Docker Desktop Daemon Status
- **Observed Limitation:** On the current Windows development machine, the Docker Desktop service (`com.docker.service`) is stopped. Interrogating the daemon yields:
  ```
  open //./pipe/dockerDesktopLinuxEngine: The system cannot find the file specified.
  ```
- **Consequence:** The multi-container stack defined in `docker-compose.graph.yml` (`yukti-postgres` and `yukti-neo4j`) was not running during automated testing.
- **System Defense & Mitigation:**
  - `backend-rust` does not crash or panic when databases are offline.
  - Relational case metadata seamlessly persists via the filesystem repository (`case.json` and `manifest.json`).
  - Graph projection tests run against the verified in-memory graph repository (`tests/graph_tests.rs`).
- **Resolution Step:**
  1. Launch Docker Desktop from the Windows Start menu or run as Administrator:
     ```powershell
     Start-Service -Name "com.docker.service"
     ```
  2. Boot the persistent storage containers:
     ```bash
     docker compose -f docker-compose.graph.yml up -d
     ```

### 2.2 Live Neo4j Connection in Local Mode
- **Observed Limitation:** Graph queries triggered from the frontend in local development without Docker utilize the mock graph projection adapter.
- **Consequence:** Visual graph rendering in the frontend operates on either the pre-loaded synthetic case dataset or in-memory graph snapshots rather than live Neo4j Bolt transactions.
- **Resolution Step:** Once the Neo4j container is up (`neo4j://localhost:7687`), the Rust backend automatically connects via the configured `NEO4J_URI` and `NEO4J_PASSWORD` environment variables.

---

## 3. Evidence Extraction & Processing Constraints

### 3.1 Supported Evidence MIME Types
- **Currently Supported for Deep Extraction:**
  - Call Detail Records (CSV format with standard telecom column headers: `caller_phone`, `receiver_phone`, `call_timestamp`, `duration_seconds`, `cell_tower_id`).
  - WhatsApp Chat Export (JSON format with `messages` array, timestamps, sender IDs).
- **Partially Supported (Ingestion Only):**
  - CCTV Video files (MP4, AVI, MKV) undergo cryptographic SHA-256 calculation and raw exhibit storage, but automated deep frame extraction (OpenCV/YOLO object detection) requires external Python worker services or hardware GPU acceleration.
- **Rejected Formats:**
  - Arbitrary binary executables (`.exe`), compressed archives (`.zip`), or proprietary database dumps are safely rejected during ingestion with an HTTP 422 or 415 error code, preserving the raw store from contamination.

### 3.2 Video Processing Fallback in Rust
- **Observed Limitation:** Deep neural network video analysis (face recognition and license plate OCR) is currently implemented in the Python reference service (`backend/app/modules/video/`).
- **Rust Implementation Status:** Rust handles cryptographic SHA-256 ingestion, frame index provenance verification (DEC-005), and timestamp synchronization. High-speed ML inference will be orchestrated via the Node.js / Laya ONNX decision service in Phase 3.

---

## 4. Synthetic Case Data Protection

### 4.1 Benchmark Case `CASE-026` Immutability
- **Constraint:** The demonstration case `CASE-026` ("Cyber Incident ALPHA / Logistics Dispatch") contains calibrated synthetic exhibits, timestamps, and contradiction injection points.
- **UI Guardrail:** In `apps/web/src/features/workspace/pages/EvidenceUploadPanel.tsx`, direct file uploads to `CASE-026` display an informational notice advising investigators that this case is a calibrated baseline. Uploads are fully enabled for all newly created cases (`POST /api/v1/cases`).
