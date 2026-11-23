# Monorepo Folder Structure Guide

YUKTI is a multi-package monorepo shared by a four-member team. Each top-level folder is a separate component with its own build, tests and CI job.

```text
SutramPrototype/
│
├── backend-rust/                     # Rust API (Axum + SQLx): the application backend
│   ├── src/
│   │   ├── api/routes/               # HTTP handlers (health, cases, evidence, graph, reconstruction)
│   │   ├── ingestion/                # Validation, SHA-256 hashing, immutable raw storage
│   │   ├── extraction/               # CDR and chat-export parsers, identifier normalisation
│   │   ├── correlation/              # Entity resolution and cross-source relationships
│   │   ├── graph/                    # Neo4j projection, outbox, queries, reconciliation
│   │   ├── persistence/              # PostgreSQL repositories (cases, evidence, custody, runs)
│   │   ├── reconstruction/           # Timeline, spatial, contradictions, gaps, what-if, report
│   │   ├── bin/graph-sync.rs         # Tool: sync the PostgreSQL outbox into Neo4j
│   │   ├── app.rs, config.rs         # App wiring and environment configuration
│   │   └── main.rs                   # API entry point
│   ├── migrations/                   # Versioned SQLx PostgreSQL migrations
│   ├── tests/                        # Integration tests
│   └── .env.example                  # Every API setting and its default
│
├── apps/web/                         # Investigator workspace (React 18 + TypeScript + Vite)
│   ├── src/
│   │   ├── features/
│   │   │   ├── landing/              # Public site at /
│   │   │   ├── workspace/            # Investigator workspace at /workspace
│   │   │   └── graph/                # Cytoscape knowledge-graph components
│   │   ├── components/               # Reusable UI primitives
│   │   ├── services/                 # HTTP client for the Rust API
│   │   ├── router/                   # Client-side routing (ADR 0001)
│   │   ├── hooks/, types/, styles/   # Shared hooks, types and styles
│   │   └── main.tsx                  # Entry point
│   └── .env.example                  # VITE_API_URL template
│
├── cli/                              # `yukti` command-line client (Rust, HTTP-only)
│   ├── src/commands/                 # status, doctor, case, evidence, reconstruction
│   ├── tests/                        # Tests against an in-process mock API
│   ├── README.md                     # Full command reference
│   └── GETTING_STARTED.md            # Install and walkthrough
│
├── backend/                          # Original Python FastAPI service (reference implementation)
│   ├── app/modules/                  # ingestion, extraction, correlation, reconstruction, reporting
│   └── tests/                        # pytest suite
│
├── decision-service/                 # Node.js 22 shell for future on-device inference (Laya / ONNX)
│
├── shared/                           # Canonical contracts: schemas, enums, examples (team-governed)
│
├── data/
│   └── synthetic/                    # Synthetic sample evidence with SHA-256 checksums
│
├── docs/
│   ├── README.md                     # Documentation index
│   ├── architecture/                 # System design (this file, system overview, subsystems)
│   ├── api/                          # Reconstruction API guide, OpenAPI specification
│   ├── decisions/                    # Architecture decision records (ADRs)
│   ├── development/                  # Setup guide, Git workflow, team onboarding
│   ├── demo/                         # Sample forensic report from synthetic data
│   ├── member-4/                     # Reconstruction contract decisions (referenced by code)
│   └── archive/                      # Phase-by-phase development record (historical)
│
├── .github/
│   ├── workflows/                    # CI: ci.yml (Python, web, decision service),
│   │                                 #     backend-rust.yml, cli.yml
│   ├── ISSUE_TEMPLATE/               # Bug and feature templates
│   └── PULL_REQUEST_TEMPLATE.md
│
├── scripts/                          # Reserved for developer automation
├── infra/docker/                     # Reserved for container specifications
├── docker-compose.graph.yml          # PostgreSQL 16 + Neo4j 5 for local development
├── .env.example                      # Environment template for the Python service
├── AGENTS.md                         # Operational rules for AI agents and developers
├── CONTRIBUTING.md                   # Commit, sync and testing procedure
└── README.md                         # Project landing page
```

## Boundaries

- **Business logic lives in the API.** `apps/web/` and `cli/` are HTTP clients only.
- **Route handlers stay thin.** Logic belongs in the module folders (`ingestion/`, `reconstruction/` and so on), not in `api/routes/`.
- **`shared/` changes need team agreement.** See [AGENTS.md](../../AGENTS.md) §6.
- **Evidence never enters Git.** Runtime stores (`data/evidence/`) are ignored. Only `data/synthetic/` is committed.
