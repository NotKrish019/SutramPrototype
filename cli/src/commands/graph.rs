//! `yukti graph summary | neighbors | path`.
//!
//! API contracts (`backend-rust/src/api/routes/graph.rs`):
//! * `GET /api/v1/cases/:case_id/graph/summary` returns `GraphSummaryResponse`. When Neo4j is
//!   not configured the API answers with zero counts and status `GRAPH_DISABLED`.
//! * `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors?depth=&limit=` returns
//!   `GraphNeighborResponse { nodes, relationships }`.
//! * `GET /api/v1/cases/:case_id/graph/path?from=&to=&max_hops=` returns `GraphPathResponse`.
//!
//! Relationships in the graph are assertions extracted or inferred from evidence; they are drawn
//! with the inferred glyph and never presented as established identity.

use serde_json::Value;

use super::{require_case, CaseViewDocument, RequestDoc};
use crate::cli::{CaseArgs, GraphCommand, NeighborsArgs, PathArgs};
use crate::client::{case_view_path, graph_neighbors_path, graph_path_path, ApiClient};
use crate::error::{exit, CliError};
use crate::output::{render_error_with_notes, render_warnings, to_json, Rendered};
use crate::ui::{Badge, Cell, Column, Glyph, Tone, Ui, MARGIN};

pub const GRAPH_SUMMARY_SCHEMA: &str = "yukti.cli.graph_summary/v1";
pub const GRAPH_NEIGHBORS_SCHEMA: &str = "yukti.cli.graph_neighbors/v1";
pub const GRAPH_PATH_SCHEMA: &str = "yukti.cli.graph_path/v1";

pub async fn run(command: &GraphCommand, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    match command {
        GraphCommand::Summary(args) => summary(args, client, json, ui).await,
        GraphCommand::Neighbors(args) => neighbors(args, client, json, ui).await,
        GraphCommand::Path(args) => path(args, client, json, ui).await,
    }
}

/// Badge for `graph_synchronization_status`.
pub fn sync_badge(status: &str) -> Badge {
    if status == "SYNCHRONIZED" {
        Badge::Ok
    } else if status.starts_with("SYNCING") {
        Badge::Warn
    } else if status.starts_with("DEGRADED") || status.contains("FAIL") {
        Badge::Flag
    } else if status == "STANDALONE_GRAPH" {
        Badge::Info
    } else {
        Badge::Off
    }
}

/// Best human label of a graph node from its properties.
pub fn node_name(node: &Value) -> String {
    let props = &node["properties"];
    for key in [
        "display_name",
        "name",
        "canonical_identifier",
        "normalized_value",
        "identifier",
        "raw_value",
        "event_type",
        "title",
        "original_filename",
    ] {
        if let Some(v) = props[key].as_str().filter(|v| !v.is_empty()) {
            return v.to_string();
        }
    }
    node["id"].as_str().unwrap_or("?").to_string()
}

fn checked(raw: &str, option: &str) -> Result<String, CliError> {
    let v = raw.trim();
    if v.is_empty() {
        Err(CliError::InvalidArgument(format!(
            "{option} must not be blank"
        )))
    } else {
        Ok(v.to_string())
    }
}

fn document(
    schema: &'static str,
    client: &ApiClient,
    case_id: &str,
    path: &str,
    raw: &Value,
    warnings: &[String],
) -> Rendered {
    let doc = CaseViewDocument {
        schema,
        api_url: client.base_url(),
        case_id,
        request: RequestDoc {
            method: "GET",
            path,
            body: None,
        },
        response: raw,
        warnings,
    };
    Rendered::stdout(to_json(&doc), exit::SUCCESS)
}

/* ------------------------------------------------------------------------------- summary */

async fn summary(args: &CaseArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let (case_id, case) = match require_case(&args.case_id, client, json, "requested").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let path = case_view_path(&case_id, "graph/summary");
    let raw = match client.get_json(&path).await {
        Ok(v) => v,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let mut warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    let status = raw["graph_synchronization_status"]
        .as_str()
        .unwrap_or("UNKNOWN")
        .to_string();
    if status == "GRAPH_DISABLED" {
        warnings.push(
            "The graph is disabled on the API (Neo4j not configured): the counts are placeholders, not measurements.".to_string(),
        );
    }
    if json {
        return document(
            GRAPH_SUMMARY_SCHEMA,
            client,
            case_id.as_str(),
            &path,
            &raw,
            &warnings,
        );
    }
    let n = |k: &str| raw[k].as_i64().unwrap_or(0);
    let mut out = ui.header(&["graph", "summary"], case_id.as_str());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}\n\n",
        ui.badge(sync_badge(&status), &status)
    ));
    out.push_str(&ui.stats(&[
        (
            n("entity_candidate_count").to_string(),
            "entities",
            Tone::Strong,
        ),
        (n("event_candidate_count").to_string(), "events", Tone::Text),
        (
            n("relationship_assertion_count").to_string(),
            "relationships",
            Tone::Inferred,
        ),
    ]));
    out.push_str(&ui.section("Projection", ""));
    let rows: [(&str, i64, Tone); 4] = [
        ("Evidence", n("evidence_count"), Tone::Text),
        ("Extraction runs", n("extraction_run_count"), Tone::Text),
        ("Reconstructions", n("reconstruction_run_count"), Tone::Text),
        (
            "Contradictions",
            n("contradiction_lead_count"),
            if n("contradiction_lead_count") > 0 {
                Tone::Flag
            } else {
                Tone::Text
            },
        ),
    ];
    let max = rows.iter().map(|r| r.1).max().unwrap_or(0).max(1) as f64;
    for (label, value, tone) in rows {
        out.push_str(&ui.kv_painted(
            label,
            &format!(
                "{}  {}",
                ui.meter(
                    value as f64 / max,
                    20,
                    if tone == Tone::Flag {
                        Tone::Flag
                    } else {
                        Tone::Inferred
                    }
                ),
                ui.paint(tone, &value.to_string())
            ),
        ));
    }
    out.push_str(&ui.kv(
        "Last projection",
        Tone::Muted,
        raw["last_projection_time"].as_str().unwrap_or("never"),
    ));
    out.push_str(&ui.kv(
        "Identifier privacy",
        Tone::Muted,
        raw["identifier_privacy"].as_str().unwrap_or("unknown"),
    ));
    out.push('\n');
    out.push_str(&ui.next(
        "explore",
        &format!(
            "yukti graph neighbors --case {} --entity <id>",
            case_id.as_str()
        ),
    ));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/* ----------------------------------------------------------------------------- neighbors */

fn render_relationships(rels: &[Value], names: &dyn Fn(&str) -> String, ui: &Ui) -> String {
    let mut out = String::new();
    for r in rels {
        let s = |k: &str| r[k].as_str().unwrap_or("?").to_string();
        out.push_str(&format!(
            "{MARGIN}{} {}  {}  {}\n",
            ui.mark(Glyph::Inferred, Tone::Inferred),
            ui.paint(Tone::Text, &names(&s("source"))),
            ui.paint(
                Tone::Inferred,
                &format!("{} {}", s("relation_type"), ui.g(Glyph::Arrow))
            ),
            ui.paint(Tone::Text, &names(&s("target")))
        ));
    }
    out
}

async fn neighbors(args: &NeighborsArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let entity = match checked(&args.entity, "--entity") {
        Ok(e) => e,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let (case_id, case) = match require_case(&args.case_id, client, json, "requested").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let path = graph_neighbors_path(&case_id, &entity, args.depth, args.limit);
    let raw = match client.get_json(&path).await {
        Ok(v) => v,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    if json {
        return document(
            GRAPH_NEIGHBORS_SCHEMA,
            client,
            case_id.as_str(),
            &path,
            &raw,
            &warnings,
        );
    }
    let nodes = raw["nodes"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let rels = raw["relationships"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let lookup = |id: &str| {
        nodes
            .iter()
            .find(|n| n["id"].as_str() == Some(id))
            .map_or_else(|| id.to_string(), node_name)
    };
    let mut out = ui.header(&["graph", "neighbors"], case_id.as_str());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{} {}  {}\n\n",
        ui.mark(Glyph::Observed, Tone::Brand),
        ui.bold(
            Tone::Strong,
            raw["root_entity_id"].as_str().unwrap_or(&entity)
        ),
        ui.paint(
            Tone::Subtle,
            &format!("depth {}", raw["depth"].as_u64().unwrap_or(1))
        )
    ));
    out.push_str(&ui.stats(&[
        (nodes.len().to_string(), "nodes", Tone::Strong),
        (rels.len().to_string(), "relationships", Tone::Inferred),
    ]));
    out.push_str(&ui.section("Nodes", ""));
    if nodes.is_empty() {
        out.push_str(&format!(
            "{MARGIN}{}\n",
            ui.paint(Tone::Muted, "No neighbours returned.")
        ));
    } else {
        let rows: Vec<Vec<Cell>> = nodes
            .iter()
            .map(|n| {
                vec![
                    Cell::new(n["label"].as_str().unwrap_or("?"), Tone::Muted),
                    Cell::new(node_name(n), Tone::Text),
                    Cell::new(n["id"].as_str().unwrap_or(""), Tone::Subtle),
                ]
            })
            .collect();
        out.push_str(&ui.table(
            &[
                Column::left("LABEL"),
                Column::left("NAME").max(40),
                Column::left("ID"),
            ],
            &rows,
        ));
    }
    if !rels.is_empty() {
        out.push_str(&ui.section("Relationships", "asserted or inferred from evidence"));
        out.push_str(&render_relationships(rels, &lookup, ui));
    }
    out.push_str(&ui.footnote(
        "Graph links are assertions drawn from evidence and similarity; they are leads, not proof of identity.",
    ));
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

/* ---------------------------------------------------------------------------------- path */

async fn path(args: &PathArgs, client: &ApiClient, json: bool, ui: &Ui) -> Rendered {
    let ends = checked(&args.from, "--from").and_then(|f| Ok((f, checked(&args.to, "--to")?)));
    let (from, to) = match ends {
        Ok(e) => e,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let (case_id, case) = match require_case(&args.case_id, client, json, "requested").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let path = graph_path_path(&case_id, &from, &to, args.max_hops);
    let raw = match client.get_json(&path).await {
        Ok(v) => v,
        Err(err) => return render_error_with_notes(&err, json, &[]),
    };
    let warnings: Vec<String> = super::placeholder_warning(&case_id, &case)
        .into_iter()
        .collect();
    if json {
        return document(
            GRAPH_PATH_SCHEMA,
            client,
            case_id.as_str(),
            &path,
            &raw,
            &warnings,
        );
    }
    let found = raw["found"].as_bool().unwrap_or(false);
    let nodes = raw["nodes"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let rels = raw["relationships"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut out = ui.header(&["graph", "path"], case_id.as_str());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}  {}\n",
        if found {
            ui.badge(Badge::Ok, "path found")
        } else {
            ui.badge(Badge::Off, "no path")
        },
        ui.paint(
            Tone::Muted,
            &format!(
                "{from} {} {to}{}",
                ui.g(Glyph::Arrow),
                if found {
                    format!(" {} {} hop(s)", ui.g(Glyph::Dot), rels.len())
                } else {
                    String::new()
                }
            )
        )
    ));
    if found && !nodes.is_empty() {
        out.push_str(&ui.section("Chain", ""));
        let rail = ui.paint(Tone::Line, ui.g(Glyph::Rail));
        for (i, node) in nodes.iter().enumerate() {
            let tone = if i == 0 || i + 1 == nodes.len() {
                Tone::Brand
            } else {
                Tone::Text
            };
            out.push_str(&format!(
                "{MARGIN}{} {}  {}\n",
                ui.mark(Glyph::Observed, tone),
                ui.bold(Tone::Strong, &node_name(node)),
                ui.paint(
                    Tone::Subtle,
                    &format!(
                        "{} {} {}",
                        node["label"].as_str().unwrap_or("?"),
                        ui.g(Glyph::Dot),
                        node["id"].as_str().unwrap_or("")
                    )
                )
            ));
            if let Some(rel) = rels.get(i).filter(|_| i + 1 < nodes.len()) {
                out.push_str(&format!(
                    "{MARGIN}{rail}  {}\n",
                    ui.paint(
                        Tone::Inferred,
                        &format!(
                            "{} {}",
                            rel["relation_type"].as_str().unwrap_or("?"),
                            ui.g(Glyph::Arrow)
                        )
                    )
                ));
            }
        }
    } else if !found {
        out.push_str(&ui.para(
            MARGIN.len(),
            Tone::Muted,
            "The graph holds no path between these entities within the hop limit. Absence of a link is not evidence that none exists.",
        ));
    }
    Rendered {
        stdout: out,
        stderr: render_warnings(&warnings),
        exit_code: exit::SUCCESS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sync_badges_follow_the_api_statuses() {
        assert_eq!(sync_badge("SYNCHRONIZED"), Badge::Ok);
        assert_eq!(sync_badge("SYNCING (3 pending)"), Badge::Warn);
        assert_eq!(sync_badge("DEGRADED (1 failures)"), Badge::Flag);
        assert_eq!(sync_badge("GRAPH_DISABLED"), Badge::Off);
        assert_eq!(sync_badge("STANDALONE_GRAPH"), Badge::Info);
    }

    #[test]
    fn node_names_prefer_readable_properties() {
        assert_eq!(
            node_name(&json!({"id":"n1","properties":{"canonical_identifier":"+919800000001"}})),
            "+919800000001"
        );
        assert_eq!(node_name(&json!({"id":"n2","properties":{}})), "n2");
    }
}
