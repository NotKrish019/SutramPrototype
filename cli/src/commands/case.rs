//! `yukti case list | create | show | overview`.
//!
//! API contracts (`backend-rust/src/api/routes/cases.rs`, registered in `backend-rust/src/app.rs`):
//! * `GET  /api/v1/cases` returns `Vec<CaseDto>`.
//! * `POST /api/v1/cases` with `CreateCaseRequest` returns `201` + `CaseDto`.
//! * `GET  /api/v1/cases/:case_id` returns `CaseDto` or `404`.
//!
//! Storage caveat: without PostgreSQL the API lists evidence-storage folders. A folder without
//! `case.json` is returned as a placeholder whose title and timestamps the API generates at
//! request time (marked `metadata.storage = "filesystem"`). The CLI shows records exactly as
//! returned and warns about this instead of hiding or "fixing" them.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::cli::{CaseArgs, CaseCommand, CreateCaseArgs, ListCasesArgs, Severity, ShowCaseArgs};
use crate::client::ApiClient;
use crate::dto::{
    is_hyphenated_uuid, CaseDto, CaseId, CaseRecord, CreateCaseRequest, ReadinessResponse,
};
use crate::error::{exit, CliError};
use crate::output::{compact_json, render_error, render_warnings, to_json, Rendered};
use crate::ui::{short_id, status_badge, Badge, Cell, Column, Glyph, Tone, Ui, MARGIN};

/// Schema of the `case list` JSON document.
pub const CASE_LIST_SCHEMA: &str = "yukti.cli.case_list/v1";
/// Schema of the `case create` / `case show` JSON document.
pub const CASE_SCHEMA: &str = "yukti.cli.case/v1";
/// Schema of the `case overview` JSON document.
pub const CASE_OVERVIEW_SCHEMA: &str = "yukti.cli.case_overview/v1";

/// Runs a `case` subcommand.
pub async fn run(command: &CaseCommand, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let result = match command {
        CaseCommand::List(args) => list(args, client, json, ui).await,
        CaseCommand::Create(args) => create(args, client, json, ui).await,
        CaseCommand::Show(args) => show(args, client, json, ui).await,
        CaseCommand::Overview(args) => return overview(args, client, json, ui).await,
    };
    result.unwrap_or_else(|err| render_error(&err, json))
}

/* ------------------------------------------------------------------------------------ list */

/// Storage mode as reported by `GET /health/ready`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorageInfo {
    /// Readiness `status` (`ready` / `degraded` / `not_ready`), `null` if unknown.
    pub readiness_status: Option<String>,
    /// Readiness `database` value exactly as reported, `null` if unknown.
    pub database: Option<String>,
    /// Why the storage mode could not be determined, if it could not.
    pub error: Option<String>,
}

/// Client-side filters applied to `case list`, echoed in the JSON document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CaseFilters {
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
}

impl CaseFilters {
    pub fn from_args(args: &ListCasesArgs) -> Result<Self, CliError> {
        let blank = |v: &Option<String>, o: &str| match v {
            Some(x) if x.trim().is_empty() => Err(CliError::InvalidArgument(format!(
                "{o} must not be blank; omit the option instead"
            ))),
            other => Ok(other.as_ref().map(|x| x.trim().to_string())),
        };
        if args.limit == Some(0) {
            return Err(CliError::InvalidArgument(
                "--limit must be at least 1".to_string(),
            ));
        }
        Ok(Self {
            status: blank(&args.status, "--status")?,
            search: blank(&args.search, "--search")?,
            limit: args.limit,
        })
    }

    fn active(&self) -> bool {
        self.status.is_some() || self.search.is_some() || self.limit.is_some()
    }

    /// The matching records, in the API's order.
    pub fn apply<'a>(&self, cases: &'a [CaseRecord]) -> Vec<&'a CaseRecord> {
        let needle = self.search.as_ref().map(|s| s.to_lowercase());
        cases
            .iter()
            .filter(|c| {
                self.status
                    .as_ref()
                    .is_none_or(|s| c.dto.status.eq_ignore_ascii_case(s))
            })
            .filter(|c| {
                needle.as_ref().is_none_or(|n| {
                    [
                        Some(&c.dto.title),
                        c.dto.fir_number.as_ref(),
                        c.dto.description.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|field| field.to_lowercase().contains(n))
                })
            })
            .take(self.limit.unwrap_or(usize::MAX))
            .collect()
    }
}

#[derive(Serialize)]
struct CaseListDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    /// Records shown (after filters).
    count: usize,
    /// Records returned by the API (before filters).
    total_count: usize,
    filters: &'a CaseFilters,
    /// Records exactly as returned, in the API's order.
    cases: Vec<&'a Value>,
    storage: &'a StorageInfo,
    /// IDs of records carrying the filesystem-placeholder marker.
    placeholder_case_ids: Vec<&'a str>,
    warnings: &'a [String],
}

async fn list(
    args: &ListCasesArgs,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Result<Rendered, CliError> {
    let filters = CaseFilters::from_args(args)?;
    let all = client.list_cases().await?;
    let shown = filters.apply(&all);
    let storage = storage_info(client).await;

    let mut warnings = Vec::new();
    warnings.extend(storage_warning(&storage));
    warnings.extend(record_warnings(&all));

    if json {
        let doc = CaseListDocument {
            schema: CASE_LIST_SCHEMA,
            api_url: client.base_url(),
            count: shown.len(),
            total_count: all.len(),
            filters: &filters,
            cases: shown.iter().map(|c| &c.raw).collect(),
            storage: &storage,
            placeholder_case_ids: shown
                .iter()
                .filter(|c| c.dto.has_filesystem_placeholder_marker())
                .map(|c| c.dto.case_id.as_str())
                .collect(),
            warnings: &warnings,
        };
        return Ok(Rendered::stdout(to_json(&doc), exit::SUCCESS));
    }

    let mut stdout = ui.header(&["case", "list"], client.base_url());
    stdout.push_str(&render_case_table(&shown, all.len(), &filters, ui));
    Ok(Rendered {
        stdout,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    })
}

/// Reads the storage mode from the readiness probe. Failure here never fails `case list`.
pub async fn storage_info(client: &ApiClient) -> StorageInfo {
    let unknown = |error: String| StorageInfo {
        readiness_status: None,
        database: None,
        error: Some(error),
    };
    match client.readiness().await {
        Err(err) => unknown(err.to_string()),
        Ok(raw) => match serde_json::from_str::<ReadinessResponse>(&raw.body) {
            Ok(body) => StorageInfo {
                readiness_status: Some(body.status),
                database: Some(body.database),
                error: None,
            },
            Err(_) => unknown(format!(
                "{} returned HTTP {} without a ReadinessResponse body",
                raw.url, raw.status
            )),
        },
    }
}

/// Warning about where the case list comes from, or `None` when the API reports PostgreSQL
/// as connected.
pub fn storage_warning(storage: &StorageInfo) -> Option<String> {
    match storage.database.as_deref() {
        Some("connected") => None,
        Some("unconfigured_filesystem_mode") => Some(
            "The API is running without PostgreSQL (filesystem mode). Cases are read from evidence-storage folders, not a database; folders without case.json appear as placeholder records whose title and timestamps the API generates when listing.".to_string(),
        ),
        Some(other) => Some(format!(
            "The API reports its database as {other:?}. It falls back to listing evidence-storage folders, so this list may be incomplete or include placeholder records."
        )),
        None => Some(format!(
            "Could not determine the API's storage mode ({}). This list may come from the filesystem fallback and include placeholder records.",
            storage.error.as_deref().unwrap_or("unknown reason")
        )),
    }
}

fn record_warnings(cases: &[CaseRecord]) -> Vec<String> {
    let mut warnings = Vec::new();
    let placeholders: Vec<&str> = cases
        .iter()
        .filter(|c| c.dto.has_filesystem_placeholder_marker())
        .map(|c| c.dto.case_id.as_str())
        .collect();
    if !placeholders.is_empty() {
        warnings.push(format!(
            "{} record(s) carry metadata.storage = \"filesystem\", the marker the API puts on storage folders without case.json; their titles and timestamps were generated by the API, not recorded: {}",
            placeholders.len(),
            placeholders.join(", ")
        ));
    }
    let non_uuid: Vec<&str> = cases
        .iter()
        .filter(|c| !is_hyphenated_uuid(&c.dto.case_id))
        .map(|c| c.dto.case_id.as_str())
        .collect();
    if !non_uuid.is_empty() {
        warnings.push(format!(
            "{} record(s) have a case_id that is not a UUID (likely a storage folder name); `yukti case show` only accepts UUIDs: {}",
            non_uuid.len(),
            non_uuid.join(", ")
        ));
    }
    warnings
}

/// Themed table of cases in the given order, with status counts.
pub fn render_case_table(
    cases: &[&CaseRecord],
    total: usize,
    filters: &CaseFilters,
    ui: &Ui,
) -> String {
    if cases.is_empty() {
        let text = if total == 0 {
            "No cases returned by the API.".to_string()
        } else {
            format!("No cases match the filters ({total} returned by the API).")
        };
        let mut out = format!("\n{MARGIN}{}\n", ui.paint(Tone::Muted, &text));
        out.push('\n');
        out.push_str(&ui.next("create one", "yukti case create --title \"...\""));
        return out;
    }
    // Status distribution, in first-seen order.
    let mut by_status: Vec<(String, usize)> = Vec::new();
    for c in cases {
        match by_status.iter_mut().find(|(s, _)| *s == c.dto.status) {
            Some((_, n)) => *n += 1,
            None => by_status.push((c.dto.status.clone(), 1)),
        }
    }
    let mut stats: Vec<(String, &str, Tone)> =
        vec![(cases.len().to_string(), "cases", Tone::Strong)];
    let labels: Vec<String> = by_status.iter().map(|(s, _)| s.to_lowercase()).collect();
    for ((_, n), label) in by_status.iter().zip(labels.iter()) {
        stats.push((n.to_string(), label.as_str(), Tone::Muted));
    }
    let placeholders = cases
        .iter()
        .filter(|c| c.dto.has_filesystem_placeholder_marker())
        .count();
    let mut out = String::from("\n");
    out.push_str(&ui.stats(&stats));
    out.push('\n');

    let rows: Vec<Vec<Cell>> = cases
        .iter()
        .map(|c| {
            let (glyph, tone) = badge_mark(status_badge(&c.dto.status));
            let mut title = c.dto.title.clone();
            let mut title_cell_tone = Tone::Text;
            if c.dto.has_filesystem_placeholder_marker() {
                title.push_str("  [filesystem placeholder]");
                title_cell_tone = Tone::Supplied;
            }
            vec![
                Cell::new(c.dto.case_id.clone(), Tone::Subtle),
                Cell::new(c.dto.status.clone(), Tone::Text).marked(glyph, tone),
                Cell::new(c.dto.created_at.clone(), Tone::Muted),
                Cell::new(c.dto.fir_number.clone().unwrap_or_default(), Tone::Muted),
                Cell::new(title, title_cell_tone),
            ]
        })
        .collect();
    out.push_str(&ui.table(
        &[
            Column::left("CASE ID"),
            Column::left("STATUS"),
            Column::left("CREATED"),
            Column::left("FIR").max(18),
            Column::left("TITLE"),
        ],
        &rows,
    ));
    let mut note = if filters.active() {
        format!(
            "{} of {total} case(s) match the filters, in the order returned by the API.",
            cases.len()
        )
    } else {
        format!("{} case(s), in the order returned by the API.", cases.len())
    };
    if placeholders > 0 {
        note.push_str(&format!(
            " {placeholders} filesystem placeholder(s): titles and timestamps generated by the API."
        ));
    }
    out.push_str(&ui.footnote(&note));
    out.push_str(&ui.next("brief", "yukti case overview --case <uuid>"));
    out
}

/// Glyph and tone for a badge, for table cells.
pub fn badge_mark(badge: Badge) -> (Glyph, Tone) {
    match badge {
        Badge::Ok => (Glyph::Check, Tone::Ok),
        Badge::Warn => (Glyph::Half, Tone::Warn),
        Badge::Flag => (Glyph::Cross, Tone::Flag),
        Badge::Info => (Glyph::Observed, Tone::Inferred),
        Badge::Off => (Glyph::Observed, Tone::Subtle),
    }
}

/* ---------------------------------------------------------------------------- create, show */

#[derive(Serialize)]
struct CaseDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    /// `create` or `show`.
    action: &'static str,
    /// The case exactly as returned by the API.
    case: &'a Value,
    warnings: &'a [String],
}

/// Validates `case create` arguments into the request body, before any network call.
pub fn build_create_request(args: &CreateCaseArgs) -> Result<CreateCaseRequest, CliError> {
    if args.title.trim().is_empty() {
        return Err(CliError::InvalidArgument(
            "--title must not be blank".to_string(),
        ));
    }
    let optional = |value: &Option<String>, option: &str| -> Result<Option<String>, CliError> {
        match value {
            Some(v) if v.trim().is_empty() => Err(CliError::InvalidArgument(format!(
                "{option} must not be blank; omit the option instead"
            ))),
            other => Ok(other.clone()),
        }
    };
    let metadata = match &args.metadata {
        None => None,
        Some(raw) => Some(parse_metadata(raw)?),
    };
    let case_id = match &args.case_id {
        None => None,
        Some(raw) => Some(CaseId::parse(raw, "--case-id")?.as_str().to_string()),
    };
    Ok(CreateCaseRequest {
        title: args.title.clone(),
        description: optional(&args.description, "--description")?,
        fir_number: optional(&args.fir_number, "--fir-number")?,
        investigator_id: optional(&args.investigator_id, "--investigator-id")?,
        metadata,
        case_id,
    })
}

/// Parses `--metadata`, which must be a JSON object.
pub fn parse_metadata(raw: &str) -> Result<Map<String, Value>, CliError> {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(other) => Err(CliError::InvalidArgument(format!(
            "--metadata must be a JSON object such as '{{\"station\":\"Saket\"}}', got {}",
            match other {
                Value::Array(_) => "an array",
                Value::String(_) => "a string",
                Value::Number(_) => "a number",
                Value::Bool(_) => "a boolean",
                _ => "null",
            }
        ))),
        Err(e) => Err(CliError::InvalidArgument(format!(
            "--metadata is not valid JSON ({e})"
        ))),
    }
}

async fn create(
    args: &CreateCaseArgs,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Result<Rendered, CliError> {
    let request = build_create_request(args)?;
    let record = client.create_case(&request).await?;

    let mut warnings = Vec::new();
    if let Some(requested) = &request.case_id {
        if !record.dto.case_id.eq_ignore_ascii_case(requested) {
            warnings.push(format!(
                "The API returned case_id {} instead of the requested {requested}.",
                record.dto.case_id
            ));
        }
    }
    if !is_hyphenated_uuid(&record.dto.case_id) {
        warnings.push(format!(
            "The API returned a case_id that is not a UUID: {:?}.",
            record.dto.case_id
        ));
    }

    render_case(client, "create", &record, &warnings, json, ui)
}

async fn show(
    args: &ShowCaseArgs,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Result<Rendered, CliError> {
    let case_id = CaseId::parse(&args.case_id, "--case")?;
    let record = client.get_case(&case_id).await?;

    let mut warnings = Vec::new();
    if !record.dto.case_id.eq_ignore_ascii_case(case_id.as_str()) {
        warnings.push(format!(
            "The API returned case_id {} for the requested {}.",
            record.dto.case_id,
            case_id.as_str()
        ));
    }
    if record.dto.has_filesystem_placeholder_marker() {
        warnings.push(
            "This record carries metadata.storage = \"filesystem\", the marker the API puts on storage folders without case.json; its title and timestamps were generated by the API, not recorded.".to_string(),
        );
    }

    render_case(client, "show", &record, &warnings, json, ui)
}

fn render_case(
    client: &ApiClient,
    action: &'static str,
    record: &CaseRecord,
    warnings: &[String],
    json: bool,
    ui: &Ui,
) -> Result<Rendered, CliError> {
    if json {
        let doc = CaseDocument {
            schema: CASE_SCHEMA,
            api_url: client.base_url(),
            action,
            case: &record.raw,
            warnings,
        };
        return Ok(Rendered::stdout(to_json(&doc), exit::SUCCESS));
    }
    let mut stdout = ui.header(&["case", action], client.base_url());
    stdout.push('\n');
    if action == "create" {
        stdout.push_str(&format!(
            "{MARGIN}{}  {}\n",
            ui.badge(Badge::Ok, "created"),
            ui.paint(Tone::Text, &format!("Created case {}", record.dto.case_id))
        ));
    }
    stdout.push_str(&render_case_details(&record.dto, ui));
    stdout.push('\n');
    let id = &record.dto.case_id;
    stdout.push_str(&ui.next("set working", &format!("export YUKTI_CASE={id}")));
    stdout.push_str(&ui.next(
        "add evidence",
        &format!("yukti evidence ingest <file> --case {id}"),
    ));
    stdout.push_str(&ui.next("brief", &format!("yukti case overview --case {id}")));
    Ok(Rendered {
        stdout,
        stderr: render_warnings(warnings),
        exit_code: exit::SUCCESS,
    })
}

/// Title line, status badge and the field list of one case. Absent values read `(none)`.
pub fn render_case_details(case: &CaseDto, ui: &Ui) -> String {
    let none = || "(none)".to_string();
    let mut out = format!(
        "{MARGIN}{}  {}\n",
        ui.bold(Tone::Strong, &case.title),
        ui.badge(status_badge(&case.status), &case.status)
    );
    if case.has_filesystem_placeholder_marker() {
        out.push_str(&format!(
            "{MARGIN}{} {}\n",
            ui.mark(Glyph::Supplied, Tone::Supplied),
            ui.paint(
                Tone::Supplied,
                "filesystem placeholder: title and timestamps generated by the API"
            )
        ));
    }
    out.push_str(&ui.section("Case record", ""));
    let rows: [(&str, Tone, String); 9] = [
        ("Case ID", Tone::Text, case.case_id.clone()),
        ("Title", Tone::Text, case.title.clone()),
        ("Status", Tone::Text, case.status.clone()),
        (
            "Description",
            Tone::Text,
            case.description.clone().unwrap_or_else(none),
        ),
        (
            "FIR number",
            Tone::Text,
            case.fir_number.clone().unwrap_or_else(none),
        ),
        (
            "Investigator",
            Tone::Text,
            case.investigator_id.clone().unwrap_or_else(none),
        ),
        ("Created", Tone::Muted, case.created_at.clone()),
        ("Updated", Tone::Muted, case.updated_at.clone()),
        ("Metadata", Tone::Subtle, compact_json(&case.metadata)),
    ];
    for (label, tone, value) in rows {
        let tone = if value == "(none)" {
            Tone::Subtle
        } else {
            tone
        };
        out.push_str(&ui.kv(label, tone, &value));
    }
    out
}

/* ------------------------------------------------------------------------------ overview */

/// One section of the overview: the response exactly as returned, or why it is missing.
#[derive(Serialize)]
struct OverviewSection {
    path: String,
    response: Option<Value>,
    error: Option<crate::error::ErrorInfo>,
}

impl OverviewSection {
    fn of(path: String, result: &Result<Value, CliError>) -> Self {
        Self {
            path,
            response: result.as_ref().ok().cloned(),
            error: result.as_ref().err().map(crate::error::ErrorInfo::from),
        }
    }
}

#[derive(Serialize)]
struct OverviewDocument<'a> {
    schema: &'static str,
    api_url: &'a str,
    case_id: &'a str,
    /// `false` when any section could not be retrieved (the exit code is then non-zero).
    complete: bool,
    case: &'a Value,
    evidence: OverviewSection,
    reconstruction: OverviewSection,
    gaps: OverviewSection,
    graph: OverviewSection,
    warnings: &'a [String],
}

async fn overview(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    use crate::client::{case_evidence_path, case_reconstruction_path, case_view_path};
    let (case_id, case) = match super::require_case(&args.case_id, client, json, "requested").await
    {
        Ok(found) => found,
        Err(rendered) => return rendered,
    };
    let evidence_path = case_evidence_path(&case_id);
    let reconstruction_path = case_reconstruction_path(&case_id);
    let gaps_path = case_view_path(&case_id, "gaps");
    let graph_path = case_view_path(&case_id, "graph/summary");
    let (evidence, reconstruction, gaps, graph) = tokio::join!(
        client.get_json(&evidence_path),
        client.get_json(&reconstruction_path),
        client.get_json(&gaps_path),
        client.get_json(&graph_path),
    );

    let mut warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    let failures: Vec<(&str, &CliError)> = [
        ("evidence", &evidence),
        ("reconstruction", &reconstruction),
        ("gaps", &gaps),
        ("graph summary", &graph),
    ]
    .into_iter()
    .filter_map(|(name, r)| r.as_ref().err().map(|e| (name, e)))
    .collect();
    for (name, err) in &failures {
        warnings.push(format!("The {name} section is unavailable: {err}"));
    }
    let exit_code = failures
        .first()
        .map_or(exit::SUCCESS, |(_, e)| e.exit_code());

    if json {
        let doc = OverviewDocument {
            schema: CASE_OVERVIEW_SCHEMA,
            api_url: client.base_url(),
            case_id: case_id.as_str(),
            complete: failures.is_empty(),
            case: &case.raw,
            evidence: OverviewSection::of(evidence_path, &evidence),
            reconstruction: OverviewSection::of(reconstruction_path, &reconstruction),
            gaps: OverviewSection::of(gaps_path, &gaps),
            graph: OverviewSection::of(graph_path, &graph),
            warnings: &warnings,
        };
        return Rendered::stdout(to_json(&doc), exit_code);
    }

    let mut out = ui.header(&["case", "overview"], client.base_url());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        ui.bold(Tone::Strong, &case.dto.title),
        ui.badge(status_badge(&case.dto.status), &case.dto.status)
    ));
    let mut meta = vec![case_id.as_str().to_string()];
    if let Some(fir) = &case.dto.fir_number {
        meta.push(format!("FIR {fir}"));
    }
    if let Some(inv) = &case.dto.investigator_id {
        meta.push(format!("investigator {inv}"));
    }
    meta.push(format!("created {}", case.dto.created_at));
    out.push_str(&ui.para(
        MARGIN.len(),
        Tone::Subtle,
        &meta.join(&format!(" {} ", ui.g(Glyph::Dot))),
    ));

    // Evidence.
    out.push_str(&ui.section("Evidence", ""));
    match &evidence {
        Err(err) => out.push_str(&ui.kv("Unavailable", Tone::Flag, &err.to_string())),
        Ok(Value::Array(items)) => {
            let mut by_type: Vec<(String, usize)> = Vec::new();
            let mut bytes: u64 = 0;
            let mut by_status: Vec<(String, usize)> = Vec::new();
            for item in items {
                let t = item["source_type"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                match by_type.iter_mut().find(|(k, _)| *k == t) {
                    Some((_, n)) => *n += 1,
                    None => by_type.push((t, 1)),
                }
                let st = item["status"].as_str().unwrap_or("unknown").to_string();
                match by_status.iter_mut().find(|(k, _)| *k == st) {
                    Some((_, n)) => *n += 1,
                    None => by_status.push((st, 1)),
                }
                bytes += item["file_size_bytes"].as_u64().unwrap_or(0);
            }
            out.push_str(
                &ui.stats(&[
                    (items.len().to_string(), "exhibits", Tone::Strong),
                    (
                        crate::ui::human_bytes(bytes)
                            .split(" (")
                            .next()
                            .unwrap_or("")
                            .to_string(),
                        "stored",
                        Tone::Muted,
                    ),
                ]),
            );
            if !by_type.is_empty() {
                let types = by_type
                    .iter()
                    .map(|(t, n)| format!("{t} {n}"))
                    .collect::<Vec<_>>()
                    .join(&format!(" {} ", ui.g(Glyph::Dot)));
                out.push_str(&ui.kv("Source types", Tone::Text, &types));
                let statuses = by_status
                    .iter()
                    .map(|(t, n)| format!("{t} {n}"))
                    .collect::<Vec<_>>()
                    .join(&format!(" {} ", ui.g(Glyph::Dot)));
                out.push_str(&ui.kv("Ingestion", Tone::Text, &statuses));
            }
        }
        Ok(_) => out.push_str(&ui.kv("Unavailable", Tone::Flag, "unexpected response shape")),
    }

    // Reconstruction.
    out.push_str(&ui.section("Reconstruction", "GET, stored or computed by the API"));
    match &reconstruction {
        Err(err) => out.push_str(&ui.kv("Unavailable", Tone::Flag, &err.to_string())),
        Ok(raw) => {
            match crate::dto::ReconstructionRecord::from_value(&reconstruction_path, raw.clone()) {
                Err(err) => out.push_str(&ui.kv("Unavailable", Tone::Flag, &err.to_string())),
                Ok(record) => {
                    let r = &record.dto;
                    let s = super::reconstruction::ReconstructionSummary::of(r);
                    out.push_str(&ui.stats(&[
                        (s.timeline_total_events.to_string(), "events", Tone::Strong),
                        (
                            s.timeline_excluded_events.to_string(),
                            "excluded",
                            Tone::Muted,
                        ),
                        (
                            s.assumptions.to_string(),
                            "assumptions",
                            if s.assumptions > 0 {
                                Tone::Warn
                            } else {
                                Tone::Muted
                            },
                        ),
                        (
                            s.contradiction_leads.to_string(),
                            "leads",
                            if s.contradiction_leads > 0 {
                                Tone::Flag
                            } else {
                                Tone::Muted
                            },
                        ),
                    ]));
                    let total = s.timeline_total_events.max(1) as f64;
                    out.push_str(&ui.kv_painted(
                        "Ordering",
                        &format!(
                            "{}  {}",
                            ui.meter(s.timeline_strictly_ordered as f64 / total, 20, Tone::Ok),
                            ui.paint(
                                Tone::Muted,
                                &format!(
                                    "{} strictly ordered {} {} concurrent/uncertain",
                                    s.timeline_strictly_ordered,
                                    ui.g(Glyph::Dot),
                                    s.timeline_concurrent_uncertain
                                )
                            )
                        ),
                    ));
                    out.push_str(&ui.kv(
                        "Movement",
                        Tone::Text,
                        &format!(
                            "{} trajectory(ies), {} segment(s), {} over the speed threshold",
                            s.spatial_trajectories,
                            s.spatial_movement_segments,
                            s.spatial_segments_exceeding_speed_threshold
                        ),
                    ));
                    out.push_str(&super::reconstruction::render_export_gate(r, ui));
                    for lead in r.contradictions.leads.iter().take(3) {
                        out.push_str(&format!(
                            "{MARGIN}{} {}  {}\n",
                            ui.mark(Glyph::Flag, Tone::Flag),
                            ui.bold(Tone::Flag, &lead.lead_type),
                            ui.paint(
                                Tone::Muted,
                                &format!(
                                    "{} {} {}  entity {}",
                                    lead.event_a_id,
                                    ui.g(Glyph::Arrow),
                                    lead.event_b_id,
                                    short_id(&lead.entity_id, ui)
                                )
                            )
                        ));
                    }
                    if r.contradictions.leads.len() > 3 {
                        out.push_str(&ui.hint(&format!(
                            "{} more lead(s): yukti contradictions --case {}",
                            r.contradictions.leads.len() - 3,
                            case_id.as_str()
                        )));
                    }
                }
            }
        }
    }

    // Gaps.
    out.push_str(&ui.section("Evidence gaps", ""));
    match &gaps {
        Err(err) => out.push_str(&ui.kv("Unavailable", Tone::Flag, &err.to_string())),
        Ok(raw) => {
            let total = raw["total_gaps_count"].as_u64().unwrap_or(0);
            let sev = |k: &str| raw["gaps_by_severity"][k].as_u64().unwrap_or(0);
            out.push_str(&ui.stats(&[
                (total.to_string(), "gaps", Tone::Strong),
                (
                    sev("CRITICAL").to_string(),
                    "critical",
                    if sev("CRITICAL") > 0 {
                        Tone::Flag
                    } else {
                        Tone::Muted
                    },
                ),
                (
                    sev("HIGH").to_string(),
                    "high",
                    if sev("HIGH") > 0 {
                        Tone::Flag
                    } else {
                        Tone::Muted
                    },
                ),
                (
                    sev("MEDIUM").to_string(),
                    "medium",
                    if sev("MEDIUM") > 0 {
                        Tone::Warn
                    } else {
                        Tone::Muted
                    },
                ),
                (sev("LOW").to_string(), "low", Tone::Muted),
            ]));
            if let Some(items) = raw["gaps"].as_array() {
                let mut sorted: Vec<&Value> = items.iter().collect();
                sorted.sort_by_key(|g| {
                    std::cmp::Reverse(Severity::rank(g["severity"].as_str().unwrap_or("")))
                });
                for g in sorted.iter().take(3) {
                    let sevr = g["severity"].as_str().unwrap_or("?");
                    let tone = match Severity::rank(sevr) {
                        2 | 3 => Tone::Flag,
                        1 => Tone::Warn,
                        _ => Tone::Muted,
                    };
                    let glyph = if Severity::rank(sevr) >= 2 {
                        Glyph::Flag
                    } else {
                        Glyph::Assumed
                    };
                    out.push_str(&format!(
                        "{MARGIN}{} {}  {}\n",
                        ui.mark(glyph, tone),
                        ui.bold(tone, &crate::ui::pad(sevr, 8)),
                        ui.paint(
                            Tone::Text,
                            &crate::ui::truncate(
                                g["title"].as_str().unwrap_or(""),
                                ui.width.saturating_sub(16),
                                ui.g(Glyph::Ellipsis)
                            )
                        )
                    ));
                }
            }
        }
    }

    // Graph.
    out.push_str(&ui.section("Knowledge graph", ""));
    match &graph {
        Err(err) => out.push_str(&ui.kv("Unavailable", Tone::Flag, &err.to_string())),
        Ok(raw) => {
            let n = |k: &str| raw[k].as_i64().unwrap_or(0).to_string();
            out.push_str(&ui.stats(&[
                (n("entity_candidate_count"), "entities", Tone::Strong),
                (n("event_candidate_count"), "events", Tone::Muted),
                (
                    n("relationship_assertion_count"),
                    "relationships",
                    Tone::Inferred,
                ),
            ]));
            let status = raw["graph_synchronization_status"]
                .as_str()
                .unwrap_or("unknown");
            out.push_str(
                &ui.kv_painted("Sync", &ui.badge(super::graph::sync_badge(status), status)),
            );
        }
    }

    out.push('\n');
    out.push_str(&ui.legend());
    out.push('\n');
    let id = case_id.as_str();
    out.push_str(&ui.next("timeline", &format!("yukti timeline --case {id}")));
    out.push_str(&ui.next("leads", &format!("yukti contradictions --case {id}")));
    out.push_str(&ui.next("gaps", &format!("yukti gaps --case {id}")));
    out.push_str(&ui.next(
        "dossier",
        &format!("yukti report --case {id} --out dossier.md"),
    ));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(title: &str) -> CreateCaseArgs {
        CreateCaseArgs {
            title: title.to_string(),
            description: None,
            fir_number: None,
            investigator_id: None,
            metadata: None,
            case_id: None,
        }
    }

    #[test]
    fn blank_title_and_blank_options_are_rejected() {
        for title in ["", "   ", "\t\n"] {
            let err = build_create_request(&args(title)).unwrap_err();
            assert_eq!(err.exit_code(), 2);
            assert!(err.to_string().contains("--title must not be blank"));
        }
        let mut a = args("T");
        a.description = Some("  ".into());
        assert!(build_create_request(&a)
            .unwrap_err()
            .to_string()
            .contains("--description"));
    }

    #[test]
    fn metadata_must_be_a_json_object() {
        assert!(parse_metadata(r#"{"a":1,"b":{"c":[1,2]}}"#).is_ok());
        assert!(parse_metadata("{}").is_ok());
        for (raw, expected) in [
            ("[1,2]", "an array"),
            ("\"x\"", "a string"),
            ("42", "a number"),
            ("null", "null"),
            ("{not json", "not valid JSON"),
            ("", "not valid JSON"),
        ] {
            let err = parse_metadata(raw).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{raw}");
            assert!(err.to_string().contains(expected), "{raw}: {err}");
        }
    }

    #[test]
    fn case_id_option_is_validated_and_canonicalized() {
        let mut a = args("T");
        a.case_id = Some("CASE-026".into());
        assert!(build_create_request(&a)
            .unwrap_err()
            .to_string()
            .contains("--case-id"));
        a.case_id = Some("3F2B8C1E-7A4D-4E1B-9C3A-5D6E7F8A9B0C".into());
        assert_eq!(
            build_create_request(&a).unwrap().case_id.as_deref(),
            Some("3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c")
        );
    }

    #[test]
    fn storage_warning_follows_the_reported_database() {
        let info = |db: Option<&str>| StorageInfo {
            readiness_status: Some("degraded".into()),
            database: db.map(str::to_string),
            error: None,
        };
        assert_eq!(storage_warning(&info(Some("connected"))), None);
        assert!(storage_warning(&info(Some("unconfigured_filesystem_mode")))
            .unwrap()
            .contains("filesystem mode"));
        assert!(storage_warning(&info(Some("disconnected")))
            .unwrap()
            .contains("\"disconnected\""));
        assert!(storage_warning(&info(None))
            .unwrap()
            .contains("Could not determine"));
    }
}
