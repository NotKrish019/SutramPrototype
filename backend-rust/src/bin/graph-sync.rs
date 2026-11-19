//! Graph Synchronization CLI Tool.
//!
//! Synchronizes committed PostgreSQL outbox events into Neo4j.
//!
//! Usage:
//! ```bash
//! cargo run --bin graph-sync -- --once
//! cargo run --bin graph-sync -- --limit 100 --case-id <UUID>
//! cargo run --bin graph-sync -- --retry-failed
//! ```

use std::env;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use sutram_backend_rust::config::AppConfig;
use sutram_backend_rust::graph::{
    initialize_schema, GraphClient, GraphConfig, GraphOutboxRepository, GraphProjectionService,
    Neo4jHttpClient,
};
use sutram_backend_rust::persistence::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    let once_mode = args.iter().any(|a| a == "--once");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let retry_failed = args.iter().any(|a| a == "--retry-failed");

    let limit: i64 = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|idx| args.get(idx + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let filter_case_id: Option<Uuid> = args
        .iter()
        .position(|a| a == "--case-id")
        .and_then(|idx| args.get(idx + 1))
        .and_then(|v| Uuid::parse_str(v).ok());

    info!(
        once = once_mode,
        dry_run = dry_run,
        retry_failed = retry_failed,
        limit = limit,
        case_id = ?filter_case_id,
        "Starting YUKTI Graph Synchronization Utility"
    );

    let app_config = AppConfig::default();
    let graph_config = GraphConfig::default();

    if !graph_config.enabled && !args.iter().any(|a| a == "--force") {
        info!("Graph projection is not enabled (GRAPH_ENABLED is false). Use --force to override.");
        return Ok(());
    }

    let db = match Database::connect(&app_config).await {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to connect to PostgreSQL: {}. Exiting.", e);
            return Ok(());
        }
    };

    let neo4j_client = Arc::new(Neo4jHttpClient::new(graph_config.clone()));
    if let Err(e) = neo4j_client.ping().await {
        error!(
            "Failed to connect to Neo4j at '{}': {}. Exiting.",
            graph_config.uri, e
        );
        return Ok(());
    }

    // Initialize schema
    if let Err(e) = initialize_schema(neo4j_client.clone()).await {
        error!("Failed to initialize Neo4j schema: {}", e);
    }

    let outbox_repo = Arc::new(GraphOutboxRepository::new(db.pool().clone()));
    let projection_service =
        GraphProjectionService::new(neo4j_client, graph_config.identifier_hmac_key);

    loop {
        let events = match outbox_repo.claim_pending_events(limit).await {
            Ok(evs) => evs,
            Err(e) => {
                error!("Error claiming outbox events: {}", e);
                break;
            }
        };

        if events.is_empty() {
            info!("No pending graph projection events found.");
            if once_mode {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            continue;
        }

        info!("Processing {} outbox events...", events.len());

        for event in events {
            if let Some(cid) = filter_case_id {
                if event.case_id != cid {
                    continue;
                }
            }

            if dry_run {
                info!(outbox_id = %event.outbox_id, event_type = %event.event_type, "DRY-RUN: Would project event");
                continue;
            }

            match projection_service.project_event(&event).await {
                Ok(()) => {
                    let _ = outbox_repo.mark_succeeded(event.outbox_id).await;
                }
                Err(e) => {
                    error!(outbox_id = %event.outbox_id, "Projection failed: {}", e);
                    let _ = outbox_repo
                        .mark_failed(event.outbox_id, &e.to_string())
                        .await;
                }
            }
        }

        if once_mode {
            break;
        }
    }

    info!("Graph synchronization completed.");
    Ok(())
}
