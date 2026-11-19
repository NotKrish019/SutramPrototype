//! PostgreSQL Persistence Layer via SQLx.
//!
//! Provides connection pooling, versioned database migrations, and typed repositories
//! for cases, evidence, extraction runs, and the immutable custody ledger.

pub mod case_repository;
pub mod custody_repository;
pub mod database;
pub mod evidence_repository;
pub mod extraction_run_repository;
pub mod model;
pub mod reconciliation;
pub mod reconstruction_repository;

pub use case_repository::CaseRepository;
pub use custody_repository::CustodyRepository;
pub use database::Database;
pub use evidence_repository::EvidenceRepository;
pub use extraction_run_repository::ExtractionRunRepository;
pub use model::{
    deterministic_uuid, CaseEntity, ContradictionEntity, CustodyEventEntity, EventEntity,
    EvidenceEntity, ExtractedIdentifierEntity, ExtractionRunEntity, ReconstructionRunEntity,
    ReconstructionRunInputEntity, RelationshipEntity, ResolvedEntityEntity,
    SpatialTrajectoryEntity, ValidationIssueEntity,
};
pub use reconciliation::{ReconciliationReport, ReconciliationService};
pub use reconstruction_repository::ReconstructionRunRepository;
