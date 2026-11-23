# Architecture Document: Neo4j Environment, Docker & CI Configuration

**Document ID:** ARCH-2A5  
**Status:** Approved & Implemented  
**Author:** Lead Rust Knowledge Graph Architect, YUKTI Forensics Team  
**Date:** 2026-11-19  

---

## 1. Overview

YUKTI requires a deterministic multi-model persistence environment across:
1. Local developer workstations (Windows 11 with LLVM-MinGW toolchain).
2. Continuous Integration runners (GitHub Actions Linux x86_64 and Windows MSVC).
3. Production / On-Premise deployments (Docker Compose / Kubernetes).

---

## 2. Docker Compose Multi-Model Infrastructure (`docker-compose.graph.yml`)

The root-level `docker-compose.graph.yml` specifies the multi-model stack:

```yaml
services:
  yukti-postgres:
    image: postgres:16-alpine
    container_name: yukti-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: sutram_forensics
    ports:
      - "5432:5432"
    volumes:
      - pg_data_forensic:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres -d sutram_forensics"]
      interval: 5s
      timeout: 5s
      retries: 5

  yukti-neo4j:
    image: neo4j:5.26-community
    container_name: yukti-neo4j
    restart: unless-stopped
    environment:
      NEO4J_AUTH: neo4j/forensic_graph_password_2026
      NEO4J_server_memory_pagecache_size: 512M
      NEO4J_server_memory_heap_initial__size: 512M
      NEO4J_server_memory_heap_max__size: 1024M
    ports:
      - "7474:7474" # HTTP Browser & Query API v2
      - "7687:7687" # Bolt protocol
    volumes:
      - neo4j_data_forensic:/data
      - neo4j_logs_forensic:/logs
    healthcheck:
      test: ["CMD-SHELL", "wget -q --spider http://localhost:7474 || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 10
      start_period: 20s

volumes:
  pg_data_forensic:
    name: yukti_pg_data_forensic
  neo4j_data_forensic:
    name: yukti_neo4j_data_forensic
  neo4j_logs_forensic:
    name: yukti_neo4j_logs_forensic
```

---

## 3. GitHub Actions Continuous Integration Workflow

`.github/workflows/backend-rust.yml` configures service containers for Linux CI:

1. **PostgreSQL 16:** Runs with `pg_isready` health check.
2. **Neo4j 5.26:** Runs with HTTP `/` query health check.
3. **FFmpeg:** Installed via apt for video processing verification.
4. **Environment Variables:**
   - `DATABASE_URL: postgres://yukti_user:yukti_password@localhost:5432/yukti_test`
   - `GRAPH_ENABLED: "true"`
   - `NEO4J_URI: "http://localhost:7474"`
   - `NEO4J_DATABASE: "neo4j"`
   - `NEO4J_USER: "neo4j"`
   - `NEO4J_PASSWORD: "forensic_graph_password_2026"`
   - `GRAPH_IDENTIFIER_HMAC_KEY: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"`

---

## 4. Local Development Fallback

When Docker or background services are unavailable:
- The Rust application defaults gracefully to in-memory/filesystem storage.
- `/health/ready` returns `status: "degraded"`.
- Unit tests run completely offline using `MockGraphClient` without external network calls.
