//! Application State & Axum Router Assembly.

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::{
    api::routes::{
        cases::{create_case, get_case, list_cases},
        evidence::{
            get_active_extraction, get_evidence_item, list_case_evidence, list_extraction_runs,
            reprocess_evidence, trigger_extraction, upload_evidence,
        },
        graph::{find_entity_path, get_case_graph_summary, get_entity_neighbors},
        health::{health_check, readiness_check},
        reconstruction::{
            get_case_contradictions, get_case_reconstruction, get_case_report, get_case_spatial,
            get_case_timeline, trigger_case_reconstruction,
        },
    },
    config::AppConfig,
    extraction::EvidenceExtractionService,
    graph::{GraphClient, GraphConfig, GraphOutboxRepository, GraphQueryService, Neo4jHttpClient},
    ingestion::{EvidenceIngestionService, StorageManager},
    persistence::{
        CaseRepository, CustodyRepository, Database, EvidenceRepository, ExtractionRunRepository,
        ReconciliationService, ReconstructionRunRepository,
    },
    reconstruction::service::ForensicReconstructionService,
};

/// Global application state shared across handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub ingestion_service: EvidenceIngestionService,
    pub extraction_service: EvidenceExtractionService,
    pub reconstruction_service: Arc<ForensicReconstructionService>,
    pub database: Option<Arc<Database>>,
    pub case_repo: Option<Arc<CaseRepository>>,
    pub evidence_repo: Option<Arc<EvidenceRepository>>,
    pub extraction_repo: Option<Arc<ExtractionRunRepository>>,
    pub custody_repo: Option<Arc<CustodyRepository>>,
    pub reconstruction_repo: Option<Arc<ReconstructionRunRepository>>,
    pub graph_client: Option<Arc<dyn GraphClient>>,
    pub graph_query_service: Option<Arc<GraphQueryService>>,
    pub graph_outbox_repo: Option<Arc<GraphOutboxRepository>>,
}

/// Asynchronously builds the application state with optional database and reconciliation.
pub async fn build_app_state(config: AppConfig) -> AppState {
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let ingestion_service =
        EvidenceIngestionService::new(storage.clone(), config.max_upload_size_bytes);
    let extraction_service = EvidenceExtractionService::new(storage.clone());

    let (database, case_repo, evidence_repo, extraction_repo, custody_repo, reconstruction_repo) =
        if let Some(ref db_url) = config.database_url {
            match Database::connect(&config).await {
                Ok(db) => {
                    info!("Successfully connected to PostgreSQL database.");
                    if let Err(e) = db.run_migrations().await {
                        warn!("Failed to apply database migrations: {}", e);
                    } else {
                        info!("PostgreSQL migrations applied successfully.");
                    }
                    let pool = db.pool().clone();
                    let case_r = Arc::new(CaseRepository::new(pool.clone()));
                    let ev_r = Arc::new(EvidenceRepository::new(pool.clone()));
                    let ext_r = Arc::new(ExtractionRunRepository::new(pool.clone()));
                    let cust_r = Arc::new(CustodyRepository::new(pool.clone()));
                    let recon_r = Arc::new(ReconstructionRunRepository::new(pool));

                    // Execute initial reconciliation
                    let recon = ReconciliationService::new(
                        storage.clone(),
                        case_r.clone(),
                        ev_r.clone(),
                        ext_r.clone(),
                        cust_r.clone(),
                    );
                    if let Ok(rep) = recon.reconcile_all().await {
                        info!("Initial filesystem reconciliation completed: {:?}", rep);
                    }

                    (
                        Some(Arc::new(db)),
                        Some(case_r),
                        Some(ev_r),
                        Some(ext_r),
                        Some(cust_r),
                        Some(recon_r),
                    )
                }
                Err(e) => {
                    warn!(
                        "Failed to connect to database at '{}': {}. Operating in filesystem mode.",
                        db_url, e
                    );
                    (None, None, None, None, None, None)
                }
            }
        } else {
            (None, None, None, None, None, None)
        };

    type GraphComponents = (
        Option<Arc<dyn GraphClient>>,
        Option<Arc<GraphOutboxRepository>>,
        Option<Arc<GraphQueryService>>,
    );

    let graph_config = GraphConfig::default();
    let (graph_client, graph_outbox_repo, graph_query_service): GraphComponents =
        if graph_config.enabled {
            if let Err(e) = graph_config.validate() {
                warn!("{}", e);
            }
            let client: Arc<dyn GraphClient> = Arc::new(Neo4jHttpClient::new(graph_config.clone()));
            let outbox = database
                .as_ref()
                .map(|db| Arc::new(GraphOutboxRepository::new(db.pool().clone())));
            let query_svc = Arc::new(GraphQueryService::new(
                client.clone(),
                outbox.clone(),
                graph_config.identifier_hmac_key,
            ));
            (Some(client), outbox, Some(query_svc))
        } else {
            (None, None, None)
        };

    let reconstruction_service = Arc::new(ForensicReconstructionService::new());

    AppState {
        config,
        ingestion_service,
        extraction_service,
        reconstruction_service,
        database,
        case_repo,
        evidence_repo,
        extraction_repo,
        custody_repo,
        reconstruction_repo,
        graph_client,
        graph_query_service,
        graph_outbox_repo,
    }
}

/// Asynchronously builds the Axum router with full database connectivity and migrations.
pub async fn create_router_async(config: AppConfig) -> Router {
    let state = build_app_state(config).await;
    create_router_with_state(state)
}

/// Synchronous constructor preserving backward compatibility for unit tests and local CLI.
pub fn create_router(config: AppConfig) -> Router {
    let storage = StorageManager::with_derived(
        config.evidence_storage_path.clone(),
        config.derived_storage_path.clone(),
    );
    let ingestion_service =
        EvidenceIngestionService::new(storage.clone(), config.max_upload_size_bytes);
    let extraction_service = EvidenceExtractionService::new(storage.clone());
    let reconstruction_service = Arc::new(ForensicReconstructionService::new());

    // Recover any interrupted runs on startup if a Tokio runtime is available
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let storage_clone = storage.clone();
        handle.spawn(async move {
            if let Ok(count) = storage_clone.recover_interrupted_runs().await {
                if count > 0 {
                    tracing::info!(
                        "Recovered {} interrupted extraction runs on startup.",
                        count
                    );
                }
            }
        });
    }

    let state = AppState {
        config,
        ingestion_service,
        extraction_service,
        reconstruction_service,
        database: None,
        case_repo: None,
        evidence_repo: None,
        extraction_repo: None,
        custody_repo: None,
        reconstruction_repo: None,
        graph_client: None,
        graph_query_service: None,
        graph_outbox_repo: None,
    };

    create_router_with_state(state)
}

/// Router constructor taking explicitly initialized `AppState`.
pub fn create_router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/health/ready", get(readiness_check))
        .route("/api/v1/cases", post(create_case).get(list_cases))
        .route("/api/v1/cases/:case_id", get(get_case))
        .route("/api/v1/cases/:case_id/evidence", get(list_case_evidence))
        .route("/api/v1/evidence/upload", post(upload_evidence))
        .route("/api/v1/evidence/:evidence_id", get(get_evidence_item))
        .route(
            "/api/v1/evidence/:evidence_id/extract",
            post(trigger_extraction),
        )
        .route(
            "/api/v1/evidence/:evidence_id/reprocess",
            post(reprocess_evidence),
        )
        .route(
            "/api/v1/evidence/:evidence_id/extraction",
            get(get_active_extraction),
        )
        .route(
            "/api/v1/evidence/:evidence_id/extraction/runs",
            get(list_extraction_runs),
        )
        .route(
            "/api/v1/cases/:case_id/graph/summary",
            get(get_case_graph_summary),
        )
        .route(
            "/api/v1/cases/:case_id/graph/entities/:entity_id/neighbors",
            get(get_entity_neighbors),
        )
        .route("/api/v1/cases/:case_id/graph/path", get(find_entity_path))
        .route(
            "/api/v1/cases/:case_id/reconstruction",
            post(trigger_case_reconstruction).get(get_case_reconstruction),
        )
        .route("/api/v1/cases/:case_id/timeline", get(get_case_timeline))
        .route("/api/v1/cases/:case_id/spatial", get(get_case_spatial))
        .route(
            "/api/v1/cases/:case_id/contradictions",
            get(get_case_contradictions),
        )
        .route(
            "/api/v1/cases/:case_id/report",
            get(get_case_report).post(get_case_report),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
