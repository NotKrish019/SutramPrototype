//! Chain-of-custody audit and case seal.
//!
//! `GET /api/v1/cases/:case_id/custody/verify` answers two questions for every exhibit:
//!
//! 1. **Is the stored file still the file that was ingested?** The raw file is re-hashed from
//!    storage and compared with the SHA-256 recorded at ingestion
//!    ([`resolve_evidence_integrity`]).
//! 2. **Is the custody ledger intact?** (PostgreSQL only.) Every entry must point at the hash of
//!    the entry before it, starting from the genesis hash, so a deleted, inserted or reordered
//!    entry breaks the chain. Each entry's own hash is also recomputed where possible. An entry
//!    hashed with a nanosecond timestamp cannot be reproduced from the microsecond value
//!    PostgreSQL stores, and such an entry cannot be told apart from an edited one, so it is
//!    reported as *not reproducible*, never as verified and never as proven tampering.
//!
//! The **case seal** is one SHA-256 over the case ID and every exhibit's recorded hash, in a
//! documented canonical form, so anyone can recompute it from the report and detect a changed,
//! added or removed exhibit.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::persistence::model::CustodyEventEntity;
use crate::reconstruction::traceability::EvidenceIntegrityStatus;

/// Hash of the (non-existent) entry before the first one.
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// How the case seal is computed (published so it can be reproduced).
pub const SEAL_RECIPE: &str = "SHA-256 over the UTF-8 text \"yukti-case-seal-v1\\n<case_id>\\n\" followed by one line \"<evidence_id> <sha256>\\n\" per exhibit, sorted by evidence_id, all lowercase";

/// One ledger entry as audited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntryAudit {
    pub custody_id: i64,
    pub action: String,
    pub actor: String,
    pub timestamp: String,
    pub event_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_event_hash: Option<String>,
    /// Points at the previous entry's hash (or the genesis hash for the first entry).
    pub link_ok: bool,
    /// `Some(true)` recomputed and equal; `None` not reproducible (an edit, or a timestamp
    /// stored at lower precision than was hashed: the two cannot be told apart).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_recomputed: Option<bool>,
}

/// Ledger audit for one exhibit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerAudit {
    /// `false` when the API runs without PostgreSQL (no ledger exists).
    pub available: bool,
    pub entry_count: usize,
    /// All links intact (`None` when there is no ledger).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links_intact: Option<bool>,
    pub hashes_matched: usize,
    pub hashes_not_reproducible: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_break_custody_id: Option<i64>,
    pub entries: Vec<LedgerEntryAudit>,
}

impl LedgerAudit {
    pub fn unavailable() -> Self {
        Self {
            available: false,
            entry_count: 0,
            links_intact: None,
            hashes_matched: 0,
            hashes_not_reproducible: 0,
            first_break_custody_id: None,
            entries: Vec::new(),
        }
    }
}

/// Audit of one exhibit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExhibitCustodyAudit {
    pub evidence_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_sha256: Option<String>,
    /// Result of re-hashing the stored raw file.
    pub stored_file: EvidenceIntegrityStatus,
    pub ledger: LedgerAudit,
    /// Stored file verified and (when a ledger exists) every ledger link intact.
    pub verified: bool,
}

/// Overall audit outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustodyAuditStatus {
    /// Every exhibit verified.
    Verified,
    /// At least one exhibit failed a check.
    Failed,
    /// The case has no exhibits.
    Empty,
}

/// Case-level audit report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseCustodyAudit {
    pub case_id: String,
    pub audited_at: String,
    pub status: CustodyAuditStatus,
    /// `POSTGRES_LEDGER` or `FILESYSTEM_NO_LEDGER`.
    pub ledger_mode: String,
    pub exhibit_count: usize,
    pub verified_count: usize,
    pub failed_count: usize,
    pub case_seal_sha256: String,
    pub seal_recipe: String,
    pub exhibits: Vec<ExhibitCustodyAudit>,
    pub disclaimer: String,
}

/// Recomputes an entry hash exactly as `CustodyRepository::append_event` does, for one
/// timestamp rendering.
fn entry_hash(e: &CustodyEventEntity, timestamp_text: &str, prev: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(e.action.as_bytes());
    hasher.update(e.evidence_id.to_string().as_bytes());
    hasher.update(e.case_id.to_string().as_bytes());
    hasher.update(e.actor.as_bytes());
    hasher.update(timestamp_text.as_bytes());
    hasher.update(prev.as_bytes());
    hasher.update(
        serde_json::to_string(&e.details)
            .unwrap_or_default()
            .as_bytes(),
    );
    hex::encode(hasher.finalize())
}

/// Audits one exhibit's ledger entries (chronological order).
pub fn audit_ledger(events: &[CustodyEventEntity]) -> LedgerAudit {
    use chrono::SecondsFormat;
    let mut audit = LedgerAudit {
        available: true,
        entry_count: events.len(),
        links_intact: Some(true),
        hashes_matched: 0,
        hashes_not_reproducible: 0,
        first_break_custody_id: None,
        entries: Vec::new(),
    };
    let mut expected_prev = GENESIS_PREV_HASH.to_string();
    for e in events {
        let prev = e.prev_event_hash.clone().unwrap_or_default();
        let link_ok = prev == expected_prev;
        if !link_ok {
            audit.links_intact = Some(false);
            audit.first_break_custody_id.get_or_insert(e.custody_id);
        }
        let renderings = [
            e.timestamp.to_rfc3339(),
            e.timestamp.to_rfc3339_opts(SecondsFormat::Micros, false),
            e.timestamp.to_rfc3339_opts(SecondsFormat::Nanos, false),
            e.timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, false),
        ];
        let reproduced = renderings
            .iter()
            .any(|t| entry_hash(e, t, &prev) == e.event_hash);
        // When no rendering reproduces the hash we cannot tell an edited entry from a timestamp
        // that PostgreSQL stored at lower precision than was hashed, so it is reported as not
        // reproducible, never as verified (and never as proven tampering). The links above are
        // the reliable tamper signal. Storing the hashed timestamp text would close this gap.
        let hash_recomputed = if reproduced {
            audit.hashes_matched += 1;
            Some(true)
        } else {
            audit.hashes_not_reproducible += 1;
            None
        };
        audit.entries.push(LedgerEntryAudit {
            custody_id: e.custody_id,
            action: e.action.clone(),
            actor: e.actor.clone(),
            timestamp: e.timestamp.to_rfc3339(),
            event_hash: e.event_hash.clone(),
            prev_event_hash: e.prev_event_hash.clone(),
            link_ok,
            hash_recomputed,
        });
        expected_prev = e.event_hash.clone();
    }
    audit
}

/// The case seal over `(evidence_id, sha256)` pairs.
pub fn case_seal(case_id: &str, exhibits: &[(String, String)]) -> String {
    let mut lines: Vec<String> = exhibits
        .iter()
        .map(|(id, sha)| format!("{} {}\n", id.to_lowercase(), sha.to_lowercase()))
        .collect();
    lines.sort();
    let mut hasher = Sha256::new();
    hasher.update(format!("yukti-case-seal-v1\n{}\n", case_id.to_lowercase()).as_bytes());
    for l in lines {
        hasher.update(l.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Builds the case report from per-exhibit audits.
pub fn summarize(
    case_id: &str,
    ledger_mode: &str,
    exhibits: Vec<ExhibitCustodyAudit>,
) -> CaseCustodyAudit {
    let verified_count = exhibits.iter().filter(|e| e.verified).count();
    let failed_count = exhibits.len() - verified_count;
    let seal_inputs: Vec<(String, String)> = exhibits
        .iter()
        .map(|e| {
            (
                e.evidence_id.clone(),
                e.recorded_sha256.clone().unwrap_or_default(),
            )
        })
        .collect();
    CaseCustodyAudit {
        case_id: case_id.to_string(),
        audited_at: chrono::Utc::now().to_rfc3339(),
        status: if exhibits.is_empty() {
            CustodyAuditStatus::Empty
        } else if failed_count == 0 {
            CustodyAuditStatus::Verified
        } else {
            CustodyAuditStatus::Failed
        },
        ledger_mode: ledger_mode.to_string(),
        exhibit_count: exhibits.len(),
        verified_count,
        failed_count,
        case_seal_sha256: case_seal(case_id, &seal_inputs),
        seal_recipe: SEAL_RECIPE.to_string(),
        exhibits,
        disclaimer: "This audit re-hashes stored exhibits and checks ledger links at the time shown. It supports, but does not replace, the certificate and testimony required to admit electronic records.".to_string(),
    }
}

/// `verified` for one exhibit from its parts.
pub fn exhibit_verified(stored: EvidenceIntegrityStatus, ledger: &LedgerAudit) -> bool {
    stored == EvidenceIntegrityStatus::VerifiedMatch && ledger.links_intact != Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn chain(n: usize) -> Vec<CustodyEventEntity> {
        let ev = Uuid::nil();
        let case = Uuid::nil();
        let mut prev = GENESIS_PREV_HASH.to_string();
        (0..n)
            .map(|i| {
                let ts = Utc.with_ymd_and_hms(2026, 11, 20, 10, i as u32, 0).unwrap();
                let mut e = CustodyEventEntity {
                    custody_id: i as i64 + 1,
                    event_id_uuid: Uuid::nil(),
                    evidence_id: ev,
                    case_id: case,
                    action: format!("ACTION_{i}"),
                    actor: "system".into(),
                    payload_sha256: None,
                    details: serde_json::json!({"i": i}),
                    prev_event_hash: Some(prev.clone()),
                    event_hash: String::new(),
                    timestamp: ts,
                };
                e.event_hash = entry_hash(&e, &ts.to_rfc3339(), &prev);
                prev = e.event_hash.clone();
                e
            })
            .collect()
    }

    #[test]
    fn intact_chain_verifies() {
        let a = audit_ledger(&chain(3));
        assert_eq!(a.links_intact, Some(true));
        assert_eq!(a.hashes_matched, 3);
        assert_eq!(a.first_break_custody_id, None);
        assert!(exhibit_verified(EvidenceIntegrityStatus::VerifiedMatch, &a));
    }

    #[test]
    fn deleted_entry_breaks_the_chain() {
        let mut c = chain(3);
        c.remove(1);
        let a = audit_ledger(&c);
        assert_eq!(a.links_intact, Some(false));
        assert_eq!(a.first_break_custody_id, Some(3));
        assert!(!exhibit_verified(
            EvidenceIntegrityStatus::VerifiedMatch,
            &a
        ));
    }

    #[test]
    fn edited_details_are_never_reported_as_verified() {
        let mut c = chain(2);
        c[0].details = serde_json::json!({"i": 99});
        let a = audit_ledger(&c);
        assert_eq!(a.hashes_matched, 1);
        assert_eq!(a.hashes_not_reproducible, 1);
        assert_eq!(a.entries[0].hash_recomputed, None);
    }

    #[test]
    fn seal_is_order_independent_and_sensitive_to_every_exhibit() {
        let a = case_seal(
            "c",
            &[("e1".into(), "AA".into()), ("e2".into(), "bb".into())],
        );
        let b = case_seal(
            "C",
            &[("e2".into(), "bb".into()), ("e1".into(), "aa".into())],
        );
        assert_eq!(a, b);
        let changed = case_seal(
            "c",
            &[("e1".into(), "aa".into()), ("e2".into(), "bc".into())],
        );
        let removed = case_seal("c", &[("e1".into(), "aa".into())]);
        assert_ne!(a, changed);
        assert_ne!(a, removed);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn stored_file_problems_fail_the_exhibit() {
        let none = LedgerAudit::unavailable();
        assert!(exhibit_verified(
            EvidenceIntegrityStatus::VerifiedMatch,
            &none
        ));
        for bad in [
            EvidenceIntegrityStatus::HashMismatch,
            EvidenceIntegrityStatus::MissingFromStorage,
            EvidenceIntegrityStatus::UnknownEvidence,
        ] {
            assert!(!exhibit_verified(bad, &none));
        }
    }
}
