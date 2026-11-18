# YUKTI Digital Forensics
### AI-Powered Digital Evidence Correlation and Crime Reconstruction Platform

[![CI](https://github.com/NotKrish019/SutramPrototype/actions/workflows/ci.yml/badge.svg)](https://github.com/NotKrish019/SutramPrototype/actions/workflows/ci.yml)
[![Branch: main](https://img.shields.io/badge/branch-main-blue.svg)](https://github.com/NotKrish019/SutramPrototype)
[![Python: 3.12](https://img.shields.io/badge/python-3.12-blue.svg)](https://www.python.org/)
[![Node: 22 LTS](https://img.shields.io/badge/node-22_LTS-green.svg)](https://nodejs.org/)
[![Vite: React+TS](https://img.shields.io/badge/frontend-React_18_+_TypeScript-61dafb.svg)](https://vitejs.dev/)

---

## 1. Project Overview

**YUKTI Digital Forensics** is an intelligent, end-to-end digital investigation platform engineered for the **SUTRAM 2026 — ISEA-IGDTUW National Hackathon**. It empowers cyber forensic investigators, law enforcement analysts, and intelligence teams to ingest, normalize, correlate, and reconstruct complex criminal activities from multi-source digital evidence.

---

## 2. Problem Being Solved

Modern criminal investigations encounter fragmented digital footprints across disparate sources: seized smartphones, mobile chats, Call Detail Records (CDRs), CCTV footage, scanned case documents, and digital disk dumps. 

Investigators face critical challenges:
- **Information Silos:** Evidence from different sources is analyzed in isolation.
- **Cognitive Overload:** Correlating thousands of timestamps, phone numbers, and geo-coordinates manually leads to investigative delays and missed connections.
- **Evidence Contradictions:** Detecting alibi discrepancies and conflicting testimonies across multimodal evidence is labor-intensive and error-prone.
- **Chain of Custody Risks:** Maintaining strict cryptographic integrity (SHA-256) while processing evidence is paramount for legal admissibility.

**YUKTI** solves this by unifying multi-source digital evidence into a centralized, cross-correlated knowledge graph with automated temporal-spatial reconstruction and decision support.

---

## 3. Core Planned Capabilities (Phased Roadmap)

1. **Multimodal Evidence Ingestion:** Ingestion of CCTV video, CDR spreadsheets, chat exports, scanned documents, and disk dumps.
2. **Evidence Normalization & Hashing:** Instantaneous SHA-256 cryptographic hashing upon seizure and ingestion.
3. **Entity and Event Extraction:** OCR (PaddleOCR), Named Entity Recognition (spaCy NER), and event extraction.
4. **Cross-Source Evidence Correlation:** Entity resolution, fuzzy matching, and cross-modality association (Sentence Transformers).
5. **Knowledge Graph Generation:** Property graph construction in Neo4j connecting persons, locations, devices, and occurrences.
6. **Temporal Crime Reconstruction:** Synchronized multi-track chronological timeline reconstruction.
7. **Spatial Crime Reconstruction:** Geographic trajectory mapping and cell-tower triangulation.
8. **Contradiction Detection:** Algorithmic verification of alibis and identification of mutually exclusive claims.
9. **Evidence Provenance & Integrity:** Immutable audit logs and chain-of-custody tracking.
10. **Investigator Decision Support:** Local AI inference (Laya / ONNX runtime) for hypothesis evaluation.
11. **Court-Admissible Reporting:** Automated forensic summary and evidence-linked report export.

---

## 4. Technology Stack

- **Frontend:** React, TypeScript, Vite, Tailwind CSS
- **Backend:** Python 3.12, FastAPI, Pydantic, Uvicorn, SQLAlchemy
- **Databases:** PostgreSQL 16 (Relational Case Data), Neo4j 5 (Knowledge Graph)
- **AI / ML Pipeline (Planned):** PaddleOCR, OpenCV, spaCy, Sentence Transformers, Scikit-learn
- **Decision Service:** Node.js 22 LTS, TypeScript, Laya / ONNX Runtime (future integration)
- **Package Management:** `uv` (Python), `npm` (Frontend & Decision Service)
- **Testing:** `pytest` (Backend), `vitest` (Frontend), Node test runner (Decision Service)
- **CI / DevOps:** GitHub Actions, Docker Compose

---

## 5. Repository Structure

```
sutram-digital-forensics/
├── .agents/                      # AI assistant & project operational rules
├── .github/                      # CI workflows & issue templates
├── docs/                         # Architecture, API, and onboarding documentation
├── apps/web/                     # React + TypeScript + Vite frontend
├── backend/                      # Python 3.12 + FastAPI backend
├── decision-service/             # Node.js 22 LTS microservice (Laya inference)
├── shared/                       # Shared schemas, enums, and JSON examples
├── data/                         # Synthetic evidence fixtures & policies
├── scripts/                      # Developer tooling & utilities
├── infra/                        # Docker & deployment configs
├── .env.example                  # Environment configuration template
├── .gitignore                    # Git ignore specifications
├── AGENTS.md                     # Root instructions for engineers and AI agents
├── README.md                     # Master documentation
└── CONTRIBUTING.md               # Team collaboration & Git workflow standards
```

---

## 6. Prerequisites

Ensure your system has the following tools installed:
- Git 2.40+
- Python 3.12
- `uv` (Python package manager): [https://astral.sh/uv](https://astral.sh/uv)
- Node.js 22 LTS and npm 10+
- (Optional for Phase 0) PostgreSQL 16 and Neo4j 5

---

## 7. Installation Instructions

Clone the repository and prepare the environment:
```bash
git clone https://github.com/NotKrish019/SutramPrototype.git
cd SutramPrototype

# Copy environment configuration
cp .env.example .env
```

---

## 8. Frontend Startup (`apps/web`)

```bash
cd apps/web
npm ci
npm run dev
```
The frontend will start at `http://localhost:5173`.

---

## 9. Backend Startup (`backend`)

```bash
cd backend
uv sync
uv run uvicorn app.main:app --reload --port 8000
```
- API Health Endpoint: `http://localhost:8000/health`
- Interactive OpenAPI Docs: `http://localhost:8000/docs`

---

## 10. Decision Service Startup (`decision-service`)

```bash
cd decision-service
npm ci
npm run dev
```
The decision service health endpoint will run at `http://localhost:3001/health`.

---

## 11. Environment Configuration

All configurable secrets, host addresses, and ports live in `.env` (derived from `.env.example`).
Key variables:
- `APP_ENV`: Application environment (`development` / `production`).
- `DATABASE_URL`: PostgreSQL connection string.
- `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD`: Neo4j graph database credentials.
- `EVIDENCE_STORAGE_PATH`: Directory for local raw evidence storage (ignored by git).
- `DECISION_SERVICE_URL`: URL of the decision service.

---

## 12. Testing Instructions

Run all test suites locally before committing:
```bash
# Backend Tests
cd backend && uv run pytest

# Frontend Tests & Build Check
cd ../apps/web && npm run test && npm run build

# Decision Service Tests
cd ../decision-service && npm run test
```

---

## 13. Git Workflow — Single Main Branch

All four team members commit directly to `main`. No additional branches are permitted.
```bash
# 1. Pull latest work before starting
git pull --rebase origin main

# 2. Stage and commit small, verified chunks
git add <files>
git commit -m "feat(scope): descriptive message"

# 3. Pull rebase again to integrate teammate commits cleanly
git pull --rebase origin main

# 4. Push directly to main
git push origin main
```

---

## 14. Team Responsibilities

- **Member 1 (AI/ML & Evidence Intelligence):** OCR, entity/event extraction, entity resolution, similarity scoring.
- **Member 2 (Backend & Data Engineering):** FastAPI core, PostgreSQL, Neo4j graph storage, file ingestion APIs.
- **Member 3 (Frontend & Visualization):** React UI, evidence upload, knowledge graph viewer, timeline viewer, reporting UI.
- **Member 4 (Forensic Intelligence & Integration):** Video/CCTV frame extraction, timestamp normalization, contradiction detection, Laya integration.

---

## 15. Security & Data-Handling Rules

- **Strict Evidence Protection:** Original evidence is strictly read-only. Cryptographic hashes (SHA-256) are calculated on ingestion.
- **Zero Real Evidence in Git:** Under no circumstances should real victim data, case files, or authentic CCTV footage be committed. Use synthetic fixtures in `data/synthetic/` only.
- **Zero Credentials in Git:** `.env` is excluded via `.gitignore`. Never commit keys, passwords, or tokens.
