# SUTRAM Digital Forensics — Backend Service

Python 3.12 + FastAPI backend responsible for evidence ingestion, data normalization, AI pipelines, graph correlation, and reconstruction APIs.

## Quick Start

```bash
# Sync dependencies via uv
uv sync

# Run tests
uv run pytest

# Start development server
uv run uvicorn app.main:app --reload --port 8000
```
- Health Check: `GET http://localhost:8000/health`
- Interactive OpenAPI Docs: `GET http://localhost:8000/docs`
