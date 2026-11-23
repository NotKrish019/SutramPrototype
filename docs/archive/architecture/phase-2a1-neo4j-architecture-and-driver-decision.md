# ADR: Neo4j Connectivity Architecture and Client Selection (Phase 2A.1)

**Document:** `docs/architecture/phase-2a1-neo4j-architecture-and-driver-decision.md`  
**Date:** November 19, 2026  
**Status:** ACCEPTED  
**Scope:** YUKTI Backend Graph Database Subsystem  

---

## 1. Context & Architectural Requirements

Phase 2A introduces the **YUKTI Digital Forensic Knowledge Graph** to project forensic records (cases, evidence exhibits, extraction runs, technical identifiers, resolved entities, events, relationships, trajectories, video artifacts, and contradiction leads) into Neo4j for graph-based investigative traversal and visualization.

The connectivity solution must satisfy:
1. **Toolchain Portability:** 100% pure-Rust TLS without external native C OpenSSL dependencies, fully compatible with Windows LLVM-MinGW (`x86_64-pc-windows-gnullvm`) and Ubuntu Linux CI.
2. **Parameterized Safety:** Strict enforcement of parameterized Cypher statements; zero string interpolation of untrusted input.
3. **Transaction & Idempotency Support:** Atomic execution of Cypher statements with structured error handling for constraint violations, deadlocks, and network failures.
4. **LTS Server Compatibility:** Full compatibility with Neo4j 5.26 LTS (Community and Enterprise editions).
5. **Architectural Decoupling:** Domain and API layers must interact with an abstract `GraphClient` / `GraphRepository` trait, ensuring graph queries remain transport-agnostic.

---

## 2. Evaluation of Rust Connectivity Options

### Option A: `neo4rs` Community Bolt Driver
* **Description:** An asynchronous community-authored Rust implementation of the Neo4j Bolt binary protocol (Bolt v4/v5 over TCP, typically port 7687).
* **Advantages:**
  * Native Bolt binary protocol framing.
  * Direct socket connection with low per-statement serialization overhead.
* **Disadvantages & Risks:**
  * Community maintained; lacks official Neo4j backing and formal SLA.
  * Version desynchronization: Bolt protocol revisions in newer Neo4j 5.x releases (such as notification format updates and telemetry changes) have caused deserialization panics in community drivers.
  * More complex TLS configuration across divergent MinGW and MSVC toolchains.
  * Direct binary protocol debugging is difficult compared to structured JSON logs.

### Option B: Official Neo4j HTTP Query API (`/db/{database}/query/v2`) via `reqwest`
* **Description:** The official HTTP Query API introduced in Neo4j 5.13+ and stabilized in Neo4j 5.26 LTS, providing REST-based execution of parameterized Cypher over HTTPS (port 7474 / 7473).
* **Advantages:**
  * **Officially Supported:** Native Neo4j HTTP API endpoint maintained and documented directly by Neo4j.
  * **Pure-Rust TLS & Portability:** Powered by `reqwest` with `rustls-tls`, completely eliminating C OpenSSL runtime linkage and guaranteeing seamless operation on Windows `gnullvm` and Linux CI.
  * **Connection Pooling:** High-performance connection pooling and HTTP keep-alive managed transparently by `hyper`.
  * **Robust Structured Errors:** Neo4j returns standard JSON error objects (`code: "Neo.ClientError.Schema.ConstraintValidationFailed"`, `message: "..."`), allowing structured error categorization in Rust without regex parsing.
  * **Firewall & Proxy Compatibility:** Operates over standard HTTP/HTTPS ports without requiring specialized Bolt firewall traversal.
* **Disadvantages:**
  * Slightly higher payload serialization overhead than binary Bolt; negligible for outbox-batched forensic projections and bounded investigative graph queries.

---

## 3. Decision & Rationale

**Decision:** We select **Option B: Neo4j Official HTTP Query API (`/db/{database}/query/v2`) via `reqwest` (with pure-Rust TLS)** as the foundational graph transport for YUKTI.

### Key Architectural Safeguards:
1. **Transport Abstraction:**
   All graph operations are placed behind a trait:
   ```rust
   #[async_trait::async_trait]
   pub trait GraphClient: Send + Sync {
       async fn execute_query(&self, statement: &str, params: serde_json::Value) -> Result<GraphQueryResult, AppError>;
       async fn ping(&self) -> Result<(), AppError>;
   }
   ```
   No application handler, outbox worker, or service ever depends on `reqwest` types directly. If Bolt is ever desired in the future, it can be swapped in as a drop-in implementation of `GraphClient`.
2. **Server Version Baseline:** Neo4j 5.26 LTS (`neo4j:5.26-community` container).
3. **Authentication:** Standard HTTP Basic authentication configured via `NEO4J_USER` and `NEO4J_PASSWORD`.
4. **Parameterized Cypher Only:** All Cypher executions require an explicit `serde_json::Value` parameter map. Dynamic string formatting into Cypher statement text is strictly disallowed.
