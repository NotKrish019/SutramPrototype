//! `yukti custody verify` and `yukti certificate`.
//!
//! * `custody verify` shows `GET /api/v1/cases/:case_id/custody/verify`
//!   (`backend-rust/src/custody_audit.rs`): every stored exhibit re-hashed, ledger links checked,
//!   and the case seal.
//! * `certificate` drafts the certificate for electronic records under Section 63 of the
//!   Bharatiya Sakshya Adhiniyam, 2023 from recorded data only: case, exhibits (file, type, size,
//!   SHA-256, ingestion time) and the audit. Facts YUKTI cannot know (device make/model/serial,
//!   who operated it, the signatories) are left as blanks to complete. The draft is laid out after
//!   the Schedule's two parts (Part A by the party, Part B by the expert) and must be checked
//!   against the official text before use. It is never presented as a signed certificate.

use std::path::Path;

use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;
use serde::Serialize;
use serde_json::Value;

use super::{require_case, CaseViewDocument, RequestDoc};
use crate::cli::{CaseArgs, CertificateArgs, CustodyCommand};
use crate::client::{case_custody_path, case_evidence_path, ApiClient};
use crate::error::{exit, CliError};
use crate::output::{render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{human_bytes, short_id, Badge, Cell, Column, Glyph, Tone, Ui, MARGIN};

pub const CUSTODY_SCHEMA: &str = "yukti.cli.custody_verify/v1";
pub const CERTIFICATE_SCHEMA: &str = "yukti.cli.certificate/v1";

pub async fn run(command: &CustodyCommand, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    match command {
        CustodyCommand::Verify(args) => verify(args, client, json, ui).await,
    }
}

/// Text encoded in the seal QR code.
pub fn seal_payload(case_id: &str, seal: &str, exhibits: usize) -> String {
    format!("YUKTI-SEAL v1 case={case_id} exhibits={exhibits} sha256={seal}")
}

/// The QR code as Unicode half blocks, one line per two module rows.
///
/// Block characters are drawn in the text colour. `for_dark_background` draws the QR's light
/// modules as blocks so the code reads correctly on a dark terminal; files viewed on a white page
/// use the normal orientation.
pub fn seal_qr(payload: &str, for_dark_background: bool) -> Option<String> {
    let code = QrCode::new(payload.as_bytes()).ok()?;
    let mut r = code.render::<Dense1x2>();
    if for_dark_background {
        r.dark_color(Dense1x2::Light).light_color(Dense1x2::Dark);
    }
    Some(r.quiet_zone(true).build())
}

fn integrity_badge(status: &str) -> (Badge, Glyph, Tone) {
    match status {
        "VERIFIED_MATCH" => (Badge::Ok, Glyph::Check, Tone::Ok),
        "UNKNOWN_EVIDENCE" => (Badge::Warn, Glyph::Half, Tone::Warn),
        _ => (Badge::Flag, Glyph::Cross, Tone::Flag),
    }
}

async fn fetch_audit(
    raw_case: &str,
    client: &ApiClient,
    json: bool,
    action: &str,
) -> Result<(crate::dto::CaseId, crate::dto::CaseRecord, String, Value), Rendered> {
    let (case_id, case) = require_case(raw_case, client, json, action).await?;
    let path = case_custody_path(&case_id);
    let audit = client.get_json(&path).await.map_err(|err| {
        let notes = if matches!(err, CliError::Http { status: 404, .. }) {
            vec![
                "This API version has no custody audit endpoint; update the YUKTI server."
                    .to_string(),
            ]
        } else {
            Vec::new()
        };
        render_error_with_notes(&err, json, &notes)
    })?;
    Ok((case_id, case, path, audit))
}

async fn verify(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, case, path, audit) =
        match fetch_audit(&args.case_id, client, json, "audited").await {
            Ok(v) => v,
            Err(r) => return r,
        };
    let warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    let status = audit["status"].as_str().unwrap_or("UNKNOWN");
    let exit_code = if status == "FAILED" {
        exit::VERIFICATION_FAILED
    } else {
        exit::SUCCESS
    };
    if json {
        let doc = CaseViewDocument {
            schema: CUSTODY_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            request: RequestDoc {
                method: "GET",
                path: &path,
                body: None,
            },
            response: &audit,
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit_code);
    }
    let mut out = ui.header(&["custody", "verify"], case_id.as_str());
    out.push('\n');
    let (badge, text) = match status {
        "VERIFIED" => (
            Badge::Ok,
            "Every stored exhibit is byte-identical to what was ingested.",
        ),
        "EMPTY" => (Badge::Off, "The case has no exhibits."),
        _ => (Badge::Flag, "At least one exhibit FAILED: see below."),
    };
    out.push_str(&format!(
        "{MARGIN}{}  {}\n\n",
        ui.badge(badge, status),
        ui.paint(Tone::Text, text)
    ));
    let n = |k: &str| audit[k].as_u64().unwrap_or(0);
    out.push_str(&ui.stats(&[
        (n("exhibit_count").to_string(), "exhibits", Tone::Strong),
        (n("verified_count").to_string(), "verified", Tone::Ok),
        (
            n("failed_count").to_string(),
            "failed",
            if n("failed_count") > 0 {
                Tone::Flag
            } else {
                Tone::Muted
            },
        ),
    ]));
    let mode = audit["ledger_mode"].as_str().unwrap_or("");
    out.push_str(&ui.kv(
        "Ledger",
        Tone::Muted,
        if mode == "POSTGRES_LEDGER" {
            "PostgreSQL hash-chained custody ledger"
        } else {
            "none: the API runs without PostgreSQL, so custody rests on the stored-file hashes only"
        },
    ));
    out.push_str(&ui.section("Exhibits", "stored file re-hashed now"));
    let rows: Vec<Vec<Cell>> = audit["exhibits"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|e| {
            let st = e["stored_file"].as_str().unwrap_or("?");
            let (_, glyph, tone) = integrity_badge(st);
            let ledger = &e["ledger"];
            let ledger_text = if ledger["available"].as_bool() != Some(true) {
                "no ledger".to_string()
            } else {
                format!(
                    "{} entr{}, links {}",
                    ledger["entry_count"].as_u64().unwrap_or(0),
                    if ledger["entry_count"].as_u64() == Some(1) {
                        "y"
                    } else {
                        "ies"
                    },
                    if ledger["links_intact"].as_bool() == Some(true) {
                        "intact"
                    } else {
                        "BROKEN"
                    }
                )
            };
            vec![
                Cell::new(st, Tone::Text).marked(glyph, tone),
                Cell::new(e["original_filename"].as_str().unwrap_or("?"), Tone::Strong),
                Cell::new(
                    short_id(e["evidence_id"].as_str().unwrap_or(""), ui),
                    Tone::Subtle,
                ),
                Cell::new(ledger_text, Tone::Muted),
            ]
        })
        .collect();
    out.push_str(&ui.table(
        &[
            Column::left("STORED FILE"),
            Column::left("EXHIBIT"),
            Column::left("ID"),
            Column::left("LEDGER"),
        ],
        &rows,
    ));
    out.push_str(&render_seal(&audit, case_id.as_str(), ui));
    out.push_str(&ui.footnote(audit["disclaimer"].as_str().unwrap_or("")));
    out.push_str(&ui.next(
        "certificate",
        &format!(
            "yukti certificate --case {} --out certificate.md",
            case_id.as_str()
        ),
    ));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code,
    }
}

fn render_seal(audit: &Value, case_id: &str, ui: &Ui) -> String {
    let seal = audit["case_seal_sha256"].as_str().unwrap_or("");
    let count = audit["exhibit_count"].as_u64().unwrap_or(0) as usize;
    let mut out = ui.section("Case seal", "SHA-256");
    out.push_str(&ui.kv_painted(
        "Seal",
        &format!(
            "{} {}",
            ui.mark(Glyph::Observed, Tone::Brand),
            ui.bold(Tone::Strong, seal)
        ),
    ));
    out.push_str(&ui.kv(
        "Recipe",
        Tone::Subtle,
        audit["seal_recipe"].as_str().unwrap_or(""),
    ));
    if ui.unicode {
        if let Some(qr) = seal_qr(&seal_payload(case_id, seal, count), true) {
            out.push('\n');
            for line in qr.lines() {
                out.push_str(&format!("{MARGIN}{line}\n"));
            }
            out.push_str(
                &ui.hint("scan to read the seal; recompute it from the exhibit list to check"),
            );
        }
    }
    out
}

/* ------------------------------------------------------------------------- certificate */

fn blank(v: &Option<String>) -> String {
    v.as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "______________________________".to_string())
}

/// The draft certificate as markdown. Pure: built only from the API records given.
pub fn certificate_markdown(
    args: &CertificateArgs,
    case: &Value,
    evidence: &[Value],
    audit: &Value,
    generated_at: &str,
) -> String {
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let case_id = s(case, "case_id");
    let seal = s(audit, "case_seal_sha256");
    let mut md = String::new();
    md.push_str(
        "# CERTIFICATE UNDER SECTION 63(4)(c) OF THE BHARATIYA SAKSHYA ADHINIYAM, 2023\n\n",
    );
    md.push_str("> **DRAFT prepared by YUKTI from recorded data.** It is not a certificate until every blank is completed, the contents are checked against the source devices and the official form in the Schedule to the Adhiniyam, and it is signed by the persons required. YUKTI does not sign or attest anything.\n\n");
    md.push_str(&format!(
        "**Case:** {} &nbsp; **Case ID:** `{}` &nbsp; **FIR:** {}\n\n",
        s(case, "title"),
        case_id,
        case["fir_number"].as_str().unwrap_or("______")
    ));
    md.push_str(&format!("**Draft generated:** {generated_at}\n\n"));

    md.push_str("## Schedule of electronic records\n\n");
    md.push_str("| # | File | Type | Size | SHA-256 (recorded at ingestion) | Ingested (UTC) | Stored file re-hashed |\n|---|---|---|---|---|---|---|\n");
    let audit_by_id = |id: &str| {
        audit["exhibits"]
            .as_array()
            .and_then(|a| a.iter().find(|e| e["evidence_id"].as_str() == Some(id)))
            .map(|e| s(e, "stored_file"))
            .unwrap_or_else(|| "NOT AUDITED".to_string())
    };
    for (i, e) in evidence.iter().enumerate() {
        md.push_str(&format!(
            "| {} | {} | {} | {} bytes | `{}` | {} | {} |\n",
            i + 1,
            s(e, "original_filename"),
            s(e, "source_type"),
            e["file_size_bytes"].as_u64().unwrap_or(0),
            s(e, "sha256_hash"),
            s(e, "created_at"),
            audit_by_id(&s(e, "evidence_id")),
        ));
    }
    md.push('\n');

    md.push_str("## Part A (to be completed by the party producing the records)\n\n");
    md.push_str(&format!(
        "I, **{}**, {} of {}, do hereby solemnly affirm and sincerely state as follows:\n\n",
        blank(&args.deponent),
        blank(&args.designation),
        blank(&args.organisation)
    ));
    md.push_str("1. I have produced the electronic records listed in the Schedule above, obtained from the following computer / digital device(s) or communication device(s):\n\n");
    md.push_str("   | Make & model | Serial no. | IMEI / UIN / UID / MAC / Cloud ID | Other identifying details |\n   |---|---|---|---|\n   | ______ | ______ | ______ | ______ |\n\n");
    md.push_str("2. The device(s) were (tick as appropriate): ☐ owned ☐ maintained ☐ managed ☐ operated by me / by ______ .\n");
    md.push_str("3. The records were produced by the device(s) during the period over which they were regularly used to store or process information in the ordinary course of the activities concerned.\n");
    md.push_str("4. Throughout that period the device(s) were operating properly, or any period in which they were not operating properly did not affect the electronic records or their accuracy: ______ .\n");
    md.push_str("5. The information in the records reproduces or is derived from information supplied to the device(s) in the ordinary course of those activities.\n");
    md.push_str("6. The hash values of the records, computed with **SHA-256**, are those listed in the Schedule above.\n\n");
    md.push_str("Place: ______ &nbsp; Date: ______ &nbsp; Signature: ______________________\n\n");

    md.push_str("## Part B (to be completed by the expert)\n\n");
    md.push_str("I, ______________________, ______________________ (designation / expertise), state that:\n\n");
    md.push_str("1. The electronic records listed in the Schedule were obtained from the device(s) described in Part A.\n");
    md.push_str("2. Their hash values, computed with **SHA-256**, are as listed in the Schedule. YUKTI recomputed the hash of each stored file on the date of this draft with the results shown in the last column; the expert should recompute them independently.\n");
    md.push_str(&format!(
        "3. The custody audit status at drafting was **{}** ({} of {} exhibits verified; ledger: {}).\n",
        s(audit, "status"),
        audit["verified_count"].as_u64().unwrap_or(0),
        audit["exhibit_count"].as_u64().unwrap_or(0),
        if s(audit, "ledger_mode") == "POSTGRES_LEDGER" {
            "PostgreSQL hash-chained ledger"
        } else {
            "no database ledger; file hashes only"
        }
    ));
    md.push_str("\nPlace: ______ &nbsp; Date: ______ &nbsp; Signature: ______________________\n\n");

    md.push_str("## Case seal\n\n");
    md.push_str(&format!("`{seal}`\n\n{}\n\n", s(audit, "seal_recipe")));
    if let Some(qr) = seal_qr(
        &seal_payload(
            &case_id,
            &seal,
            audit["exhibit_count"].as_u64().unwrap_or(0) as usize,
        ),
        false,
    ) {
        md.push_str("```text\n");
        md.push_str(&qr);
        md.push_str("\n```\n\n");
    }
    md.push_str("*Contradiction leads, inferences and simulations produced by YUKTI are investigative leads, not facts, and are not certified by this document.*\n");
    md
}

#[derive(Serialize)]
struct CertificateDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    draft: bool,
    case_seal_sha256: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    written: Option<Written>,
    markdown: &'a str,
}

#[derive(Serialize)]
struct Written {
    path: String,
    sha256: String,
    size_bytes: usize,
}

pub async fn certificate(
    args: &CertificateArgs,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Rendered {
    let (case_id, case, _path, audit) =
        match fetch_audit(&args.case_id, client, json, "drafted").await {
            Ok(v) => v,
            Err(r) => return r,
        };
    let evidence = match client.get_json(&case_evidence_path(&case_id)).await {
        Ok(Value::Array(items)) => items,
        Ok(_) => Vec::new(),
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let generated = chrono_like_now();
    let md = certificate_markdown(args, &case.raw, &evidence, &audit, &generated);
    let mut warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    if audit["status"].as_str() == Some("FAILED") {
        warnings.push("The custody audit FAILED for at least one exhibit; the draft records this and must not be signed until it is resolved.".to_string());
    }
    if evidence.is_empty() {
        warnings.push("The case has no exhibits; the schedule is empty.".to_string());
    }

    let written = match &args.out {
        None => None,
        Some(path) => {
            if let Err(err) =
                super::analysis::write_export(Path::new(path), md.as_bytes(), args.force)
            {
                return render_error_with_notes(&err, json, &[]);
            }
            Some(Written {
                path: path.display().to_string(),
                sha256: super::evidence::sha256_hex(md.as_bytes()),
                size_bytes: md.len(),
            })
        }
    };
    let seal = audit["case_seal_sha256"].as_str().unwrap_or("");
    if json {
        let doc = CertificateDocument {
            schema: CERTIFICATE_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            draft: true,
            case_seal_sha256: seal,
            written,
            markdown: &md,
        };
        return Rendered {
            stdout: to_json(&doc),
            stderr: String::new(),
            exit_code: exit::SUCCESS,
        };
    }
    let mut out = ui.header(&["certificate"], case_id.as_str());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.badge(Badge::Warn, "draft"),
        ui.bold(
            Tone::Strong,
            "Section 63, Bharatiya Sakshya Adhiniyam, 2023"
        )
    ));
    out.push_str(&ui.para(
        MARGIN.len(),
        Tone::Muted,
        "Built from recorded data only. Device details and signatures are left blank: complete them, check against the official Schedule form, and have Part A and Part B signed.",
    ));
    out.push_str(&ui.section("Schedule", &format!("{} exhibit(s)", evidence.len())));
    for e in &evidence {
        out.push_str(&format!(
            "{MARGIN}{} {}  {}\n{MARGIN}  {}\n",
            ui.mark(Glyph::Observed, Tone::Text),
            ui.bold(Tone::Strong, e["original_filename"].as_str().unwrap_or("?")),
            ui.paint(
                Tone::Muted,
                &format!(
                    "{} {} {}",
                    e["source_type"].as_str().unwrap_or(""),
                    ui.g(Glyph::Dot),
                    human_bytes(e["file_size_bytes"].as_u64().unwrap_or(0))
                )
            ),
            ui.paint(
                Tone::Subtle,
                &format!("SHA-256 {}", e["sha256_hash"].as_str().unwrap_or(""))
            )
        ));
    }
    out.push_str(&render_seal(&audit, case_id.as_str(), ui));
    match &written {
        Some(w) => {
            out.push_str(&ui.section("Written", "markdown"));
            out.push_str(&ui.kv("File", Tone::Strong, &w.path));
            out.push_str(&ui.kv("SHA-256", Tone::Text, &w.sha256));
        }
        None => out.push_str(&ui.next(
            "save it",
            &format!(
                "yukti certificate --case {} --out certificate.md",
                case_id.as_str()
            ),
        )),
    }
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/// Current UTC time as RFC 3339 without pulling in a date crate.
fn chrono_like_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn certificate_lists_every_exhibit_and_never_claims_to_be_signed() {
        let args = CertificateArgs {
            case_id: "c".into(),
            out: None,
            force: false,
            deponent: Some("Insp. A. Kumar".into()),
            designation: None,
            organisation: Some("PS Saket".into()),
        };
        let case = json!({"case_id": "c-1", "title": "Demo", "fir_number": "FIR-7/2026"});
        let ev = vec![
            json!({"evidence_id": "e1", "original_filename": "cdr.csv", "source_type": "TABULAR_CSV", "file_size_bytes": 10, "sha256_hash": "ab".repeat(32), "created_at": "2026-11-23T10:00:00Z"}),
        ];
        let audit = json!({"status": "VERIFIED", "verified_count": 1, "exhibit_count": 1, "ledger_mode": "FILESYSTEM_NO_LEDGER", "case_seal_sha256": "cd".repeat(32), "seal_recipe": "recipe", "exhibits": [{"evidence_id": "e1", "stored_file": "VERIFIED_MATCH"}]});
        let md = certificate_markdown(&args, &case, &ev, &audit, "2026-11-24T00:00:00Z");
        assert!(md.contains("SECTION 63(4)(c) OF THE BHARATIYA SAKSHYA ADHINIYAM, 2023"));
        assert!(md.contains("DRAFT prepared by YUKTI"));
        assert!(md.contains("| 1 | cdr.csv | TABULAR_CSV | 10 bytes |"));
        assert!(md.contains(&"ab".repeat(32)));
        assert!(md.contains("VERIFIED_MATCH"));
        assert!(md.contains("Insp. A. Kumar"));
        assert!(md.contains("PS Saket"));
        assert!(
            md.contains("______________________________"),
            "missing designation stays blank"
        );
        assert!(md.contains(&"cd".repeat(32)));
        assert!(md.contains("Part B"));
    }

    #[test]
    fn seal_qr_encodes_the_payload() {
        let qr = seal_qr(&seal_payload("c", &"ab".repeat(32), 3), true).unwrap();
        assert_ne!(
            qr,
            seal_qr(&seal_payload("c", &"ab".repeat(32), 3), false).unwrap()
        );
        assert!(qr.lines().count() > 10);
        assert!(qr.contains('█') || qr.contains('▀') || qr.contains('▄'));
    }

    #[test]
    fn now_is_rfc3339_utc() {
        let t = chrono_like_now();
        assert_eq!(t.len(), 20);
        assert!(t.ends_with('Z') && t.as_bytes()[10] == b'T');
    }
}
