# Project Standards & Engineering Guidelines

This document governs engineering practices, repository structure, and code standards for **SUTRAM Digital Forensics**.

---

## 1. Fixed Monorepo Structure

All development must conform to the agreed layout:
```
sutram-digital-forensics/
├── .agents/rules/
├── .github/
├── docs/ (architecture, api, development, decisions)
├── apps/web/ (React, Vite, Tailwind frontend)
├── backend/ (FastAPI, Python 3.12, modular pipelines)
├── decision-service/ (Node.js 22 LTS, TypeScript)
├── shared/ (schemas, enums, examples)
├── data/ (synthetic datasets only)
├── scripts/ (automation scripts)
└── infra/docker/ (containerization)
```
- Never create a parallel root or alter root folder naming.
- For directories waiting on feature code, commit a `.gitkeep` file rather than placeholder stub logic.

---

## 2. Dependency Management Conventions

- **Python (`backend/`):**
  - Managed exclusively through `uv`.
  - Defined in `backend/pyproject.toml` with pinned/compatible versions.
  - Resolved via `backend/uv.lock`.
  - Only install necessary dependencies for the current milestone. Do not install heavy ML packages prematurely.
- **Frontend (`apps/web/`):**
  - Managed via `npm`.
  - Declared in `apps/web/package.json` and locked with `package-lock.json`.
- **Decision Service (`decision-service/`):**
  - Managed via `npm`.
  - Declared in `decision-service/package.json` and locked with `package-lock.json`.

---

## 3. Naming Conventions

- **Files & Directories:**
  - Python: `snake_case.py` for modules, `tests/test_*.py` for test files.
  - TypeScript/React: `PascalCase.tsx` for components and pages, `camelCase.ts` for hooks, utilities, and services.
  - Documentation: `kebab-case.md`.
- **Code Identifiers:**
  - Classes & Interfaces: `PascalCase`.
  - Functions, methods, and variables: `snake_case` in Python; `camelCase` in TypeScript.
  - Constants & Enums: `UPPER_SNAKE_CASE`.
  - Shared Schema fields: `snake_case` (JSON schema / Pydantic / TypeScript serialization standard).

---

## 4. Shared Schema Governance

- Canonical data models for `Case`, `Evidence`, `Entity`, `Event`, and `Relationship` are maintained in `shared/schemas/`.
- Backend Pydantic models in `backend/app/schemas/` and frontend TypeScript interfaces in `apps/web/src/types/` must mirror these contracts.
- Any modification to a shared schema requires consensus from all four project members.

---

## 5. Error Handling & Logging Standards

- **Backend:**
  - Return standardized HTTP error responses with status codes (`4xx`, `5xx`) and JSON envelopes (`{"error": {"code": "...", "message": "..."}}`).
  - Never allow unhandled exceptions to crash the server or leak raw stack traces to the client.
- **Frontend:**
  - Wrap async network operations in explicit try/catch blocks or react-query error boundaries.
  - Surface actionable, human-friendly error messages to the user interface.

---

## 6. Testing Requirements

- **Backend:** Every service and API route must be covered with `pytest`.
- **Frontend:** Components and client services must have tests run with `vitest`.
- **Decision Service:** Test runner execution verifying baseline health endpoints.
- **CI Gate:** All tests across all three subprojects must pass on every commit to `main`.

---

## 7. Forensic Evidence Handling

- Evidence files are strictly read-only.
- Ingestion pipelines must compute cryptographic hashes (SHA-256) upon arrival.
- Always retain metadata linking extracted entities/events back to the exact source file and timestamp.
- Never place unredacted or real evidentiary materials in git repositories.

---

## 8. Git Synchronization & Branch Rules

- **Single Branch (`main`):** No feature branches or pull requests are used.
- **Routine before starting work:**
  ```bash
  git pull --rebase origin main
  ```
- **Routine before committing & pushing:**
  ```bash
  git status
  git add <files>
  git commit -m "feat(scope): concise description"
  git pull --rebase origin main
  # verify all tests pass
  git push origin main
  ```
- **Strictly Forbidden:** `git push --force`, `git push -f`, `git reset --hard` on tracked commits.

---

## 9. Security Requirements

- All credentials, secrets, tokens, and database passwords must be read from environment variables via `.env`.
- `.env` is ignored by `.gitignore`; `.env.example` serves as the safe template.
- Sanitize all file inputs against path traversal (`../`) and file extension spoofing.
