# AGENTS.md — Global Operational Directives for AI Agents & Engineers

Welcome to **YUKTI Digital Forensics** (built for SUTRAM 2026 — ISEA-IGDTUW National Hackathon).
All AI agents, coding assistants, and human developers operating within this repository must strictly adhere to the following rules.

---

## 1. PROJECT IDENTITY & MISSION

- **Project:** AI-Powered Digital Evidence Correlation and Crime Reconstruction Platform.
- **Hackathon:** SUTRAM 2026 — ISEA-IGDTUW National Hackathon.
- **Problem Focus:** Automated multimodal digital evidence ingestion, entity/event extraction, cross-source correlation, timeline and spatial reconstruction, contradiction detection, and investigator decision support.
- **Single Source of Truth:** This repository is the sole source of truth for the entire 4-member development team.

---

## 2. ARCHITECTURAL BOUNDARIES

- **Monorepo Separation:**
  - `apps/web/`: React + TypeScript + Vite frontend.
  - `backend/`: Python 3.12 + FastAPI modular service.
  - `decision-service/`: Node.js 22 LTS + TypeScript service (for future Laya / ONNX runtime integration).
  - `shared/`: Canonical data contracts, schemas, enums, and examples.
- **Strict Separation of Concerns:**
  - Keep API route handlers lightweight; business logic must live strictly in `backend/app/services/` or `backend/app/modules/`.
  - Frontend components must be modular and reusable; state and network calls must live in dedicated hooks and service modules.
  - AI and ML inference pipelines must be decoupled from presentation and API transport layers.
  - Never move frontend files into backend directories or create nested alternate roots.

---

## 3. DEVELOPMENT STANDARDS

- **Inspection First:** Always inspect existing code, directory structures, and configurations before proposing or making changes.
- **Type Safety:**
  - Python: Strict type hints (`mypy`-compliant) on all function arguments and returns.
  - TypeScript: Strict mode (`"strict": true`) enforced across `apps/web` and `decision-service`.
- **Quality & Testing:**
  - Core business logic, parsers, and API endpoints must have associated tests (`pytest` for Python, `vitest` for React, Node test runner for decision service).
  - Explicit error handling; never swallow exceptions or fail silently.
- **Documentation:**
  - Document all architectural changes in `docs/architecture/` or `docs/decisions/`.

---

## 4. FORENSIC INTEGRITY PRINCIPLES

1. **Immutability of Evidence:** Original evidence files must **NEVER** be modified in place. All processing pipelines must read from raw stores and write normalized artifacts to derived stores.
2. **Cryptographic Provenance:** In future ingestion phases, every piece of ingested evidence must immediately have its SHA-256 (and SHA-3) cryptographic hash computed and logged.
3. **Chain of Custody:** Preserve source file references, byte offsets, extraction timestamps, and ingestion metadata.
4. **Observed vs. Inferred:** Strictly distinguish observed hard facts (e.g. timestamp from carrier CDR) from inferred semantic links (e.g. cross-source face or entity similarity score).
5. **No Identity Fabrications:** Never present similarity or heuristic confidence as conclusive proof of identity.
6. **Synthetic Development Data:** Real victim or live case data is strictly forbidden in development or test suites. Use synthetic datasets only (`data/synthetic/`).

---

## 5. GIT REPOSITORY & BRANCH POLICY

- **Single Branch Rule:** All development happens directly on `main`.
- **PROHIBITED:** Creating branches (`develop`, `feature/*`, `staging`, `testing`, etc.) is strictly forbidden.
- **PROHIBITED:** Force-pushing (`git push --force`, `git push --force-with-lease`) and history rewriting (`git reset --hard`) are strictly forbidden.
- **Synchronization Routine:**
  ```bash
  git pull --rebase origin main
  ```
  Every developer must synchronize with `origin/main` before beginning work and before pushing.
- **Atomic Commits:** Make descriptive, conventional commits (`feat(scope): ...`, `fix(scope): ...`).
- **Secret Protection:** Never commit `.env`, credentials, API keys, certificates, or proprietary models.

---

## 6. FOUR-MEMBER COLLABORATION MODEL

- **Member 1:** AI/ML & Evidence Intelligence (OCR, extraction, entity/event resolution, cross-source similarity).
- **Member 2:** Backend & Data Engineering (FastAPI, PostgreSQL, Neo4j, ingestion APIs, graph persistence).
- **Member 3:** Frontend & Visualization (React dashboard, evidence upload, knowledge graph UI, timeline UI).
- **Member 4:** Forensic Intelligence & Integration (CCTV/video processing, timestamp normalization, contradiction detection, Laya integration).
- **Shared Schema Governance:** No member or agent may unilaterally modify `shared/schemas/`. Shared contract adjustments require team alignment.

---

## 7. AI AGENT OPERATIONAL PROTOCOL

**Before Implementing:**
1. Inspect the repository state and read project instructions.
2. Identify affected files and modules.
3. Formulate an explicit plan and present it.
4. Request explicit confirmation for architectural changes.

**After Implementing:**
1. Execute unit and integration tests.
2. Inspect `git status` and verify changes.
3. Update relevant documentation.
4. Provide a clear summary of changes and remaining manual verification steps.
