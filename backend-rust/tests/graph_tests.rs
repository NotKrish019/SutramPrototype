//! Integration and Unit Tests for Knowledge Graph Projection & Query Engine.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use sutram_backend_rust::error::AppError;
use sutram_backend_rust::graph::{
    client::GraphClient,
    model::{CypherStatement, OutboxEvent, OutboxMetrics, OutboxStatus},
    privacy::compute_identifier_fingerprint,
    projection::GraphProjectionService,
    query::GraphQueryService,
    schema::SCHEMA_STATEMENTS,
};

/// Thread-safe in-memory mock implementation of `GraphClient` for offline testing.
#[derive(Default, Clone)]
struct MockGraphClient {
    executed_statements: Arc<Mutex<Vec<CypherStatement>>>,
    query_response: Arc<Mutex<Option<serde_json::Value>>>,
}

impl MockGraphClient {
    fn new() -> Self {
        Self::default()
    }

    fn recorded_statements(&self) -> Vec<CypherStatement> {
        self.executed_statements.lock().unwrap().clone()
    }

    fn set_query_response(&self, response: serde_json::Value) {
        *self.query_response.lock().unwrap() = Some(response);
    }
}

#[async_trait]
impl GraphClient for MockGraphClient {
    async fn execute_query(
        &self,
        statement: &str,
        parameters: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        self.executed_statements
            .lock()
            .unwrap()
            .push(CypherStatement::new(statement, parameters));
        let resp = self
            .query_response
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| json!({ "data": [] }));
        Ok(resp)
    }

    async fn execute_batch(&self, statements: Vec<CypherStatement>) -> Result<(), AppError> {
        self.executed_statements.lock().unwrap().extend(statements);
        Ok(())
    }

    async fn ping(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[test]
fn test_schema_constraints_and_indexes() {
    assert_eq!(
        SCHEMA_STATEMENTS.len(),
        13,
        "Expected exactly 13 idempotent schema constraint/index statements"
    );

    let labels = [
        "Case",
        "Evidence",
        "ExtractionRun",
        "EntityCandidate",
        "EventCandidate",
        "RelationshipAssertion",
        "ReconstructionRun",
        "ContradictionLead",
    ];

    for label in &labels {
        let found = SCHEMA_STATEMENTS
            .iter()
            .any(|s| s.contains(&format!(":{}", label)));
        assert!(
            found,
            "Label '{}' must have a schema constraint or index defined",
            label
        );
    }

    // Verify all statements are idempotent
    for stmt in SCHEMA_STATEMENTS {
        assert!(
            stmt.contains("IF NOT EXISTS"),
            "Statement must be idempotent (IF NOT EXISTS): {}",
            stmt
        );
    }
}

#[test]
fn test_identifier_privacy_scoping_and_determinism() {
    let case_a = Uuid::new_v4();
    let case_b = Uuid::new_v4();
    let key = "test-hmac-secret-key-32-chars-long";

    // 1. Determinism
    let fp1 = compute_identifier_fingerprint(&case_a, "PHONE", "+919876543210", Some(key));
    let fp2 = compute_identifier_fingerprint(&case_a, "PHONE", "+919876543210", Some(key));
    assert_eq!(fp1, fp2);
    assert_eq!(fp1.len(), 64);

    // 2. Case isolation
    let fp_b = compute_identifier_fingerprint(&case_b, "PHONE", "+919876543210", Some(key));
    assert_ne!(
        fp1, fp_b,
        "Distinct cases must produce distinct fingerprints for identical identifiers"
    );

    // 3. Type isolation
    let fp_imei = compute_identifier_fingerprint(&case_a, "IMEI", "+919876543210", Some(key));
    assert_ne!(
        fp1, fp_imei,
        "Distinct identifier types must produce distinct fingerprints"
    );

    // 4. Secret isolation
    let fp_alt_key = compute_identifier_fingerprint(
        &case_a,
        "PHONE",
        "+919876543210",
        Some("alternate-hmac-key"),
    );
    assert_ne!(fp1, fp_alt_key);

    // 5. Unkeyed fallback behavior
    let fp_unkeyed = compute_identifier_fingerprint(&case_a, "PHONE", "+919876543210", None);
    assert_eq!(fp_unkeyed.len(), 64);
    assert_ne!(fp1, fp_unkeyed);
}

#[test]
fn test_outbox_status_and_metrics() {
    assert_eq!(OutboxStatus::Pending.to_string(), "PENDING");
    assert_eq!(OutboxStatus::Processing.to_string(), "PROCESSING");
    assert_eq!(OutboxStatus::Succeeded.to_string(), "SUCCEEDED");
    assert_eq!(OutboxStatus::Failed.to_string(), "FAILED");
    assert_eq!(OutboxStatus::RetryWait.to_string(), "RETRY_WAIT");
    assert_eq!(OutboxStatus::DeadLetter.to_string(), "DEAD_LETTER");

    let metrics = OutboxMetrics::default();
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.processing, 0);
    assert_eq!(metrics.succeeded, 0);
    assert_eq!(metrics.failed, 0);
}

#[test]
fn test_projection_case_created_cypher_generation() {
    let mock_client = Arc::new(MockGraphClient::new());
    let service = GraphProjectionService::new(mock_client, Some("test-key".into()));

    let case_id = Uuid::new_v4();
    let event = OutboxEvent {
        outbox_id: Uuid::new_v4(),
        case_id,
        aggregate_type: "CASE".to_string(),
        aggregate_id: case_id,
        aggregate_version: 1,
        event_type: "CASE_CREATED".to_string(),
        payload: json!({
            "case_id": case_id.to_string(),
            "title": "Operation Cyber Sentinel",
            "status": "OPEN",
            "created_at": Utc::now().to_rfc3339()
        }),
        status: "PENDING".to_string(),
        attempt_count: 0,
        max_attempts: 5,
        next_attempt_at: Utc::now(),
        last_error: None,
        created_at: Utc::now(),
        processed_at: None,
    };

    let statements = service
        .generate_cypher_statements(&event)
        .expect("Statements should generate successfully");
    assert_eq!(statements.len(), 1);

    let stmt = &statements[0];
    assert!(stmt
        .statement
        .contains("MERGE (c:Case {case_id: $case_id})"));
    assert_eq!(
        stmt.parameters.get("case_id").unwrap().as_str().unwrap(),
        case_id.to_string()
    );
    assert_eq!(
        stmt.parameters.get("title").unwrap().as_str().unwrap(),
        "Operation Cyber Sentinel"
    );
}

#[test]
fn test_projection_evidence_ingested_cypher_generation() {
    let mock_client = Arc::new(MockGraphClient::new());
    let service = GraphProjectionService::new(mock_client, Some("test-key".into()));

    let case_id = Uuid::new_v4();
    let evidence_id = Uuid::new_v4();
    let event = OutboxEvent {
        outbox_id: Uuid::new_v4(),
        case_id,
        aggregate_type: "EVIDENCE".to_string(),
        aggregate_id: evidence_id,
        aggregate_version: 1,
        event_type: "EVIDENCE_INGESTED".to_string(),
        payload: json!({
            "sha256_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "source_type": "CDR",
            "mime_type": "text/csv",
            "file_size_bytes": 1024,
            "storage_uri": "forensic://evidence/cdr.csv",
            "created_at": Utc::now().to_rfc3339()
        }),
        status: "PENDING".to_string(),
        attempt_count: 0,
        max_attempts: 5,
        next_attempt_at: Utc::now(),
        last_error: None,
        created_at: Utc::now(),
        processed_at: None,
    };

    let statements = service
        .generate_cypher_statements(&event)
        .expect("Statements should generate successfully");
    assert_eq!(statements.len(), 1);

    let stmt = &statements[0];
    assert!(stmt
        .statement
        .contains("MERGE (c:Case {case_id: $case_id})"));
    assert!(stmt
        .statement
        .contains("MERGE (e:Evidence {evidence_id: $evidence_id})"));
    assert!(stmt.statement.contains("MERGE (c)-[:HAS_EVIDENCE]->(e)"));
    assert_eq!(
        stmt.parameters
            .get("sha256_hash")
            .unwrap()
            .as_str()
            .unwrap(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_projection_extraction_completed_cypher_generation() {
    let mock_client = Arc::new(MockGraphClient::new());
    let service = GraphProjectionService::new(mock_client, Some("secret-test-key".into()));

    let case_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let evidence_id = Uuid::new_v4();

    let event = OutboxEvent {
        outbox_id: Uuid::new_v4(),
        case_id,
        aggregate_type: "EXTRACTION_RUN".to_string(),
        aggregate_id: run_id,
        aggregate_version: 1,
        event_type: "EXTRACTION_COMPLETED".to_string(),
        payload: json!({
            "extraction_run_id": run_id.to_string(),
            "evidence_id": evidence_id.to_string(),
            "extractor_name": "unified_forensic_extractor",
            "extractor_version": "1.0.0",
            "entities": [
                {
                    "entity_id": "ent-1",
                    "entity_type": "PHONE",
                    "canonical_name": "+919876543210",
                    "confidence": 0.95
                }
            ],
            "events": [
                {
                    "event_id": "ev-1",
                    "event_type": "CALL",
                    "nominal_timestamp": "2026-03-01T10:00:00Z",
                    "confidence": 0.90
                }
            ],
            "relationships": [
                {
                    "assertion_id": "rel-1",
                    "source_entity_id": "ent-1",
                    "target_entity_id": "ent-2",
                    "relationship_type": "COMMUNICATED_WITH",
                    "confidence": 0.88
                }
            ]
        }),
        status: "PENDING".to_string(),
        attempt_count: 0,
        max_attempts: 5,
        next_attempt_at: Utc::now(),
        last_error: None,
        created_at: Utc::now(),
        processed_at: None,
    };

    let statements = service
        .generate_cypher_statements(&event)
        .expect("Extraction completed statements should generate");

    // We expect: 1 Run statement, 1 Entity statement, 1 Event statement, 1 Relationship statement = 4 statements
    assert_eq!(statements.len(), 4);

    // 1. Run statement
    assert!(statements[0]
        .statement
        .contains("MERGE (r:ExtractionRun {extraction_run_id: $run_id})"));
    assert!(statements[0]
        .statement
        .contains("MERGE (e)-[:PRODUCED_EXTRACTION_RUN]->(r)"));

    // 2. Entity statement
    assert!(statements[1]
        .statement
        .contains("MERGE (ent:EntityCandidate {graph_entity_id: $graph_entity_id})"));
    assert!(statements[1]
        .statement
        .contains("MERGE (r)-[:EXTRACTED_ENTITY]->(ent)"));
    // Ensure privacy fingerprint is computed
    let ent_fp = statements[1]
        .parameters
        .get("fingerprint")
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(ent_fp.len(), 64);

    // 3. Event statement
    assert!(statements[2]
        .statement
        .contains("MERGE (ev:EventCandidate {event_id: $event_id})"));
    assert!(statements[2]
        .statement
        .contains("MERGE (r)-[:EXTRACTED_EVENT]->(ev)"));

    // 4. Relationship statement
    assert!(statements[3]
        .statement
        .contains("MERGE (rel:RelationshipAssertion {assertion_id: $assertion_id})"));
    assert!(statements[3]
        .statement
        .contains("MERGE (r)-[:PRODUCED_ASSERTION]->(rel)"));
}

#[test]
fn test_projection_reconstruction_completed_cypher_generation() {
    let mock_client = Arc::new(MockGraphClient::new());
    let service = GraphProjectionService::new(mock_client, None);

    let case_id = Uuid::new_v4();
    let reconstruction_id = Uuid::new_v4();

    let event = OutboxEvent {
        outbox_id: Uuid::new_v4(),
        case_id,
        aggregate_type: "RECONSTRUCTION_RUN".to_string(),
        aggregate_id: reconstruction_id,
        aggregate_version: 1,
        event_type: "RECONSTRUCTION_COMPLETED".to_string(),
        payload: json!({
            "reconstruction_run_id": reconstruction_id.to_string(),
            "contradictions": [
                {
                    "contradiction_id": "contra-1",
                    "conflict_type": "SPATIAL_ALIBI_VIOLATION",
                    "severity": "CRITICAL",
                    "confidence": 0.99,
                    "description": "Target phone observed at Location A while CCTV observed at Location B simultaneously"
                }
            ]
        }),
        status: "PENDING".to_string(),
        attempt_count: 0,
        max_attempts: 5,
        next_attempt_at: Utc::now(),
        last_error: None,
        created_at: Utc::now(),
        processed_at: None,
    };

    let statements = service
        .generate_cypher_statements(&event)
        .expect("Reconstruction statements should generate");

    // 1 Run statement + 1 Contradiction statement = 2 statements
    assert_eq!(statements.len(), 2);
    assert!(statements[0]
        .statement
        .contains("MERGE (rec:ReconstructionRun {reconstruction_run_id: $recon_id})"));
    assert!(statements[1]
        .statement
        .contains("MERGE (lead:ContradictionLead {contradiction_id: $contradiction_id})"));
    assert!(statements[1]
        .statement
        .contains("MERGE (rec)-[:IDENTIFIED_CONTRADICTION]->(lead)"));
}

#[tokio::test]
async fn test_mock_client_full_event_projection() {
    let mock_client = Arc::new(MockGraphClient::new());
    let service = GraphProjectionService::new(mock_client.clone(), Some("key".into()));

    let case_id = Uuid::new_v4();
    let event = OutboxEvent {
        outbox_id: Uuid::new_v4(),
        case_id,
        aggregate_type: "CASE".to_string(),
        aggregate_id: case_id,
        aggregate_version: 1,
        event_type: "CASE_CREATED".to_string(),
        payload: json!({
            "title": "Integration Test Case",
            "status": "OPEN",
            "created_at": Utc::now().to_rfc3339()
        }),
        status: "PENDING".to_string(),
        attempt_count: 0,
        max_attempts: 5,
        next_attempt_at: Utc::now(),
        last_error: None,
        created_at: Utc::now(),
        processed_at: None,
    };

    service
        .project_event(&event)
        .await
        .expect("Projection execution should succeed");

    let executed = mock_client.recorded_statements();
    assert_eq!(executed.len(), 1);
    assert!(executed[0]
        .statement
        .contains("MERGE (c:Case {case_id: $case_id})"));
}

#[tokio::test]
async fn test_query_service_depth_and_limit_clamping() {
    let mock_client = Arc::new(MockGraphClient::new());
    mock_client.set_query_response(json!({
        "data": [
            {
                "neighbors": [
                    { "id": "ent-2", "label": "EntityCandidate", "name": "Associate A" }
                ]
            }
        ]
    }));

    let query_service = GraphQueryService::new(mock_client.clone(), None, Some("test-key".into()));
    let case_id = Uuid::new_v4();

    // Pass depth 99 (should clamp to 3) and limit 500 (should clamp to 100)
    let res = query_service
        .get_entity_neighbors(case_id, "ent-1", 99, 500)
        .await
        .expect("Query should succeed");

    assert_eq!(res.depth, 3, "Depth must be clamped to 3");
    assert_eq!(res.nodes.len(), 2); // Root + 1 neighbor

    let recorded = mock_client.recorded_statements();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0]
            .parameters
            .get("limit")
            .unwrap()
            .as_i64()
            .unwrap(),
        100,
        "Limit must be clamped to 100"
    );
}

#[tokio::test]
async fn test_query_service_path_finding() {
    let mock_client = Arc::new(MockGraphClient::new());
    mock_client.set_query_response(json!({
        "data": [
            { "p": { "nodes": [], "relationships": [] } }
        ]
    }));

    let query_service = GraphQueryService::new(mock_client.clone(), None, None);
    let case_id = Uuid::new_v4();

    let res = query_service
        .find_path(case_id, "ent-1", "ent-2", 10)
        .await
        .expect("Path query should succeed");

    assert!(res.found);
    assert_eq!(res.from_entity_id, "ent-1");
    assert_eq!(res.to_entity_id, "ent-2");

    let recorded = mock_client.recorded_statements();
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].statement.contains("shortestPath"));
}
