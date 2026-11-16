# Monorepo Folder Structure Guide

This repository is organized as a clean, multi-package monorepo designed for 4 collaborating developers.

```
sutram-digital-forensics/
│
├── .agents/
│   └── rules/
│       └── project-standards.md      # Absolute rules governing all AI and developer work
│
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.md             # Standardized bug reporting form
│   │   └── feature_request.md        # Standardized feature proposal form
│   ├── PULL_REQUEST_TEMPLATE.md      # PR checklist template
│   └── workflows/
│       └── ci.yml                    # Automated GitHub Actions CI workflow for main branch
│
├── docs/
│   ├── architecture/
│   │   ├── system-overview.md        # High-level architecture, data flows & phase status
│   │   └── folder-structure.md       # Directory layout documentation (this file)
│   ├── api/                          # OpenAPI schemas and route documentation
│   ├── development/
│   │   ├── setup-guide.md            # Comprehensive developer environment setup guide
│   │   ├── git-workflow.md           # Single main branch Git synchronization guide
│   │   └── team-onboarding.md        # Roles, responsibilities & workflows for 4 team members
│   └── decisions/                    # Architectural Decision Records (ADRs)
│
├── apps/
│   └── web/                          # React + TypeScript + Vite frontend
│       ├── public/                   # Static assets
│       ├── src/
│       │   ├── assets/               # Local icons and imagery
│       │   ├── components/           # Reusable UI primitives (Buttons, Modals, Cards)
│       │   ├── features/             # Feature-specific components (GraphViewer, Timeline)
│       │   ├── hooks/                # Custom React hooks
│       │   ├── layouts/              # Top-level page layout templates
│       │   ├── lib/                  # Utility libraries and client configurations
│       │   ├── pages/                # Route views (HomePage, CasePage, EvidencePage)
│       │   ├── services/             # HTTP clients and API integration layer
│       │   ├── types/                # TypeScript interface and type declarations
│       │   ├── App.tsx               # Main application component
│       │   └── main.tsx              # DOM mounting entrypoint
│       ├── index.html                # HTML document entrypoint
│       ├── package.json              # Web dependencies and scripts
│       ├── tsconfig.json             # TypeScript compiler settings (strict mode)
│       └── vite.config.ts            # Vite bundler configuration
│
├── backend/                          # Python 3.12 + FastAPI backend
│   ├── app/
│   │   ├── api/
│   │   │   └── routes/               # API route endpoints (health, cases, evidence)
│   │   ├── core/                     # Application configuration and settings
│   │   ├── models/                   # Database ORM models (SQLAlchemy / Neo4j)
│   │   ├── schemas/                  # Pydantic validation schemas
│   │   ├── services/                 # Business logic and coordination services
│   │   ├── modules/                  # Specialized forensic processing pipelines
│   │   │   ├── ingestion/            # File parsing, verification, SHA-256 computation
│   │   │   ├── extraction/           # OCR, NER, transcription, frame analysis
│   │   │   ├── correlation/          # Cross-evidence linkage and entity resolution
│   │   │   ├── reconstruction/       # Timeline sequencing and spatial mapping
│   │   │   └── reporting/            # Investigative report generator
│   │   └── main.py                   # FastAPI app factory and router registration
│   ├── tests/                        # Pytest automated test suite
│   ├── pyproject.toml                # uv Python package specification
│   └── uv.lock                       # uv locked dependency graph
│
├── decision-service/                 # Node.js 22 LTS + TypeScript decision support microservice
│   ├── src/
│   │   └── index.ts                  # Microservice entrypoint and HTTP health router
│   ├── tests/                        # Decision service test suite
│   ├── package.json                  # Node dependencies and scripts
│   └── tsconfig.json                 # TypeScript compiler configuration
│
├── shared/                           # Canonical cross-stack contracts and models
│   ├── schemas/                      # Universal JSON schema definitions
│   ├── enums/                        # Shared enumerations and taxonomies
│   └── examples/                     # Canonical sample payloads
│
├── data/                             # Data and evidentiary fixtures
│   ├── synthetic/                    # Lightweight mock evidentiary samples for tests
│   └── README.md                     # Forensic data handling policies
│
├── scripts/                          # Developer automation and database migration scripts
├── infra/
│   └── docker/                       # Docker Compose and Dockerfile specifications
│
├── .env.example                      # Safe template for local environment variables
├── .gitignore                        # Git exclusion rules protecting secrets and evidence
├── AGENTS.md                         # Project-wide operational directives for AI & team
├── README.md                         # Main repository landing page and documentation
└── CONTRIBUTING.md                   # Collaboration and Git hygiene guide for the 4 members
```
