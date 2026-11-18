# Developer Environment Setup Guide

Follow this guide to set up the complete development environment on your local machine.

---

## 1. Prerequisites

Ensure your machine has the following tools installed:
- **Git** (version 2.40+)
- **Python** (version 3.12)
- **uv** (fast Python package manager: `curl -LsSf https://astral.sh/uv/install.sh` or `powershell -c "irm https://astral.sh/uv/install.ps1 | iex"`)
- **Node.js** (version 22 LTS) and **npm** (version 10+)
- **PostgreSQL 16** (optional for Phase 0, required for Phase 1+)
- **Neo4j 5** (optional for Phase 0, required for Phase 3+)

---

## 2. Environment Configuration

1. In the repository root, copy `.env.example` to create your local `.env`:
   ```bash
   cp .env.example .env
   # On Windows PowerShell:
   Copy-Item .env.example .env
   ```
2. Verify that values inside `.env` match your local development environment.

---

## 3. Backend Setup (`backend/`)

1. Change directory to `backend/`:
   ```bash
   cd backend
   ```
2. Synchronize dependencies using `uv`:
   ```bash
   uv sync
   ```
3. Run the automated tests:
   ```bash
   uv run pytest
   ```
4. Start the backend development server:
   ```bash
   uv run uvicorn app.main:app --reload --port 8000
   ```
5. Verify health:
   - Health Endpoint: `http://localhost:8000/health`
   - Interactive OpenAPI Docs: `http://localhost:8000/docs`

---

## 4. Frontend Setup (`apps/web/`)

1. Change directory to `apps/web/`:
   ```bash
   cd apps/web
   ```
2. Install dependencies:
   ```bash
   npm ci
   ```
3. (Optional) Configure the backend URL. Vite reads `.env` files from `apps/web/` only — the
   repository-root `.env` is **not** loaded by the frontend:
   ```bash
   cp .env.example .env.local   # then edit VITE_API_URL if the backend is not on http://localhost:8000
   ```
   See `docs/architecture/frontend-api-client.md` for how requests and errors are handled.
4. Run unit tests and type checks:
   ```bash
   npm run test
   npm run build
   ```
5. Start the Vite development server:
   ```bash
   npm run dev
   ```
6. Open your browser at `http://localhost:5173`.

---

## 5. Decision Service Setup (`decision-service/`)

1. Change directory to `decision-service/`:
   ```bash
   cd decision-service
   ```
2. Install dependencies:
   ```bash
   npm ci
   ```
3. Run unit tests:
   ```bash
   npm run test
   ```
4. Start the decision service:
   ```bash
   npm run dev
   ```
5. Verify health:
   - Health Endpoint: `http://localhost:3001/health`
