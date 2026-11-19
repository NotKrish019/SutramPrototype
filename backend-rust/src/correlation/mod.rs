//! Correlation and Entity Resolution Subsystem (Member 1).

pub mod relationships;
pub mod resolution;

pub use relationships::{
    generate_cdr_relationship_candidate, generate_chat_relationship_candidates,
};
pub use resolution::{deduplicate_identifiers, normalize_phone_number, PhoneNormalizationResult};
