//! Command-line interface definition.
//!
//! Every command group is a variant of [`Command`] with its own argument structs and a
//! handler under `src/commands/`. Global options apply to every command.

use std::path::PathBuf;

use clap::builder::styling::{Ansi256Color, AnsiColor, Color, Style, Styles};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::ui::{palette, ColorChoice};

/// Environment variable that supplies `--case` when the option is omitted.
pub const ENV_CASE: &str = "YUKTI_CASE";

fn rgb((r, g, b): palette::Rgb) -> Option<Color> {
    // 256-colour codes render everywhere, including terminals without truecolor.
    Some(Color::Ansi256(Ansi256Color(crate::ui::rgb_to_256(r, g, b))))
}

/// Help colours that match the YUKTI theme (clap disables them for pipes and NO_COLOR).
pub fn styles() -> Styles {
    Styles::styled()
        .header(Style::new().bold().fg_color(rgb(palette::CRIMSON)))
        .usage(Style::new().bold().fg_color(rgb(palette::CRIMSON)))
        .literal(Style::new().bold())
        .placeholder(Style::new().fg_color(rgb(palette::STEEL)))
        .valid(Style::new().fg_color(rgb(palette::OK)))
        .invalid(Style::new().bold().fg_color(rgb(palette::WARN)))
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
}

const BEFORE_HELP: &str = "\
\x1b[1;38;5;161m▌ YUKTI\x1b[0m  \x1b[38;5;246mDigital Evidence Correlation & Crime Reconstruction\x1b[0m";

const AFTER_HELP: &str = "\
\x1b[1;38;5;161mWorkflow:\x1b[0m
  yukti                                         interactive session (starts the local server)
  yukti doctor                                  check the API, storage and graph
  yukti case create --title \"Warehouse burglary\" register a case
  export YUKTI_CASE=<uuid>                      set the working case for every command
  yukti evidence ingest cdr.csv --auto-extract  ingest and extract (SHA-256 checked)
  yukti reconstruction run                      reconstruct timeline, movement, leads
  yukti case overview                           one-screen investigative brief
  yukti timeline | contradictions | gaps        drill into the findings
  yukti whatif --clock-offset 120               test an alternative assumption
  yukti report --out dossier.md                 export the forensic dossier
  yukti open --view theatre                     watch the case replay in the web workspace
  yukti shell                                   interactive session

\x1b[38;5;242m● observed  ○ inferred  ◇ supplied  ◆ assumed  ▲ flagged for review
Leads are investigative leads, never conclusions. --json prints one versioned document.\x1b[0m";

/// Command-line client for the YUKTI forensic API.
#[derive(Debug, Parser)]
#[command(
    name = "yukti",
    version,
    propagate_version = true,
    styles = styles(),
    before_help = BEFORE_HELP,
    after_help = AFTER_HELP,
    max_term_width = 110
)]
pub struct Cli {
    /// Base URL of the YUKTI Rust API [env: YUKTI_API_URL] [default: http://127.0.0.1:8080]
    #[arg(long, global = true, value_name = "URL")]
    pub api_url: Option<String>,

    /// Request timeout in whole seconds, 1-3600 [env: YUKTI_TIMEOUT_SECS] [default: 10]
    #[arg(long, global = true, value_name = "SECONDS")]
    pub timeout: Option<u64>,

    /// Print one machine-readable JSON document on stdout instead of human-readable text
    #[arg(long, global = true)]
    pub json: bool,

    /// When to use colour in human-readable output (NO_COLOR is honoured in auto mode)
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto, value_name = "WHEN")]
    pub color: ColorChoice,

    /// Use 7-bit ASCII glyphs instead of Unicode box drawing and symbols
    #[arg(long, global = true)]
    pub ascii: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Check that the API is reachable and report database and graph readiness
    Status,

    /// Check that yukti is set up: configuration, API, readiness, database, graph and case API (read-only)
    Doctor,

    /// Show the resolved configuration and where each value came from
    Config,

    /// Manage investigation cases
    #[command(subcommand)]
    Case(CaseCommand),

    /// Ingest, inspect, extract and verify evidence
    #[command(subcommand)]
    Evidence(EvidenceCommand),

    /// Run and explain a case's timeline, spatial and contradiction reconstruction
    #[command(subcommand)]
    Reconstruction(ReconstructionCommand),

    /// Show the reconstructed timeline with uncertainty windows and assumptions
    Timeline(TimelineArgs),

    /// Show trajectories, movement segments and speed-threshold evaluations
    Spatial(CaseArgs),

    /// Show contradiction leads (investigative leads, not findings)
    Contradictions(CaseArgs),

    /// Show evidence gaps that limit the reconstruction, with suggested corroboration
    Gaps(GapsArgs),

    /// Trace findings back to source evidence, hashes and extraction runs
    Provenance(ProvenanceArgs),

    /// Show or export the judicial forensic report and markdown dossier
    Report(ReportArgs),

    /// Explore the case knowledge graph
    #[command(subcommand)]
    Graph(GraphCommand),

    /// Simulate a hypothetical scenario against the baseline reconstruction (nothing is stored)
    #[command(name = "whatif", visible_alias = "what-if")]
    WhatIf(WhatIfArgs),

    /// Print a shell completion script (bash, zsh, fish, elvish, powershell)
    Completions(CompletionsArgs),

    /// Start an interactive session that remembers the working case (also: plain `yukti`)
    Shell,

    /// Start, stop and inspect the local YUKTI server bundled with installed builds
    #[command(subcommand)]
    Server(ServerCommand),

    /// Breaking-point analysis: how wrong an assumption must be before each lead dissolves
    #[command(visible_alias = "breaking-point")]
    Robustness(CaseArgs),

    /// Chain-of-custody checks: re-hash stored exhibits, check the ledger, compute the case seal
    #[command(subcommand)]
    Custody(CustodyCommand),

    /// Draft the Section 63 (Bharatiya Sakshya Adhiniyam, 2023) certificate for the case's exhibits
    Certificate(CertificateArgs),

    /// Defence-counsel view: the weakest points of the case first, with how to close each one
    Challenge(CaseArgs),

    /// Ask a question in plain English; answers cite evidence and refuse when there is none
    Ask(AskArgs),

    /// Replay the reconstructed timeline as an animation with a movement map
    Replay(ReplayArgs),

    /// Run a guided demonstration on a synthetic case (starts the local server if needed)
    Demo(DemoArgs),

    /// Open the web workspace at a case and view (the browser reads the same API) [env: YUKTI_WEB_URL]
    #[command(visible_alias = "web")]
    Open(OpenArgs),
}

/// Arguments of `yukti open`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct OpenArgs {
    /// Case UUID; omit to open the workspace home [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: Option<String>,

    /// Workspace view to open
    #[arg(long, value_enum, default_value_t = crate::commands::open::WebView::Overview)]
    pub view: crate::commands::open::WebView,

    /// Address of the web workspace [env: YUKTI_WEB_URL] [default: http://127.0.0.1:5173]
    #[arg(long, value_name = "URL")]
    pub web_url: Option<String>,

    /// Print the link without launching a browser
    #[arg(long)]
    pub print: bool,
}

/// `yukti custody ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum CustodyCommand {
    /// Re-hash every stored exhibit, check the custody ledger and print the case seal
    Verify(CaseArgs),
}

/// Arguments of `yukti certificate`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CertificateArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Write the draft certificate (markdown) to this file
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,

    /// Replace --out if it already exists
    #[arg(long)]
    pub force: bool,

    /// Name of the person giving Part A (left blank to fill by hand)
    #[arg(long, value_name = "NAME")]
    pub deponent: Option<String>,

    /// Designation of that person
    #[arg(long, value_name = "TEXT")]
    pub designation: Option<String>,

    /// Organisation or police station
    #[arg(long, value_name = "TEXT")]
    pub organisation: Option<String>,
}

/// Arguments of `yukti ask`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AskArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// The question, e.g. "where was 9811002233 at 9 pm"
    #[arg(value_name = "QUESTION", required = true, num_args = 1..)]
    pub question: Vec<String>,
}

/// Arguments of `yukti replay`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReplayArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Milliseconds between events in the animation (0-5000)
    #[arg(long, value_name = "MS", default_value_t = 700, value_parser = clap::value_parser!(u64).range(0..=5000))]
    pub delay: u64,

    /// Print the whole replay at once instead of animating
    #[arg(long)]
    pub no_animate: bool,
}

/// Arguments of `yukti demo`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct DemoArgs {
    /// Do not pause between steps
    #[arg(long)]
    pub fast: bool,
}

/// `yukti server ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ServerCommand {
    /// Start the bundled server in the background (no-op when the API already answers)
    Start,
    /// Stop the server that yukti started
    Stop,
    /// Show whether the server is running, and where its data and log live
    Status,
    /// Print the end of the server log
    Logs {
        /// Number of lines
        #[arg(long, default_value_t = 40)]
        lines: usize,
    },
}

/// A required case, from `--case` or `YUKTI_CASE`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CaseArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,
}

/// `yukti reconstruction ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ReconstructionCommand {
    /// Ask the API to reconstruct a case from the events it holds, with its default options
    Run(ReconstructionArgs),
    /// Show the API's current reconstruction of a case and its judicial report
    Explain(ReconstructionArgs),
    /// Compare reconstruction scenarios (not yet available in the API; exits 69; see `whatif`)
    Compare(ReconstructionArgs),
}

/// Arguments shared by the `yukti reconstruction` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReconstructionArgs {
    /// Case UUID (checked with the API before anything else is requested) [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,
}

/// `yukti evidence ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum EvidenceCommand {
    /// Upload one evidence file to a case (the file is only read, never modified)
    Ingest(IngestEvidenceArgs),
    /// List the evidence receipts of a case, in the API's order
    List(ListEvidenceArgs),
    /// Show one evidence receipt and its extraction history
    Show(EvidenceArgs),
    /// Start a new extraction run with the API's default parameters
    Extract(EvidenceArgs),
    /// Re-run extraction with a declared timezone, clock offset or country code
    Reprocess(ReprocessArgs),
    /// Show the active extraction result of an evidence item
    Extraction(EvidenceArgs),
    /// List every extraction run of an evidence item, oldest first
    Runs(EvidenceArgs),
    /// Verify a local file against the SHA-256 the API recorded at ingestion (exit 7 on mismatch)
    Verify(VerifyEvidenceArgs),
}

/// One evidence item.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct EvidenceArgs {
    /// Evidence UUID
    #[arg(long = "evidence", value_name = "ID")]
    pub evidence_id: String,
}

/// Arguments of `yukti evidence reprocess`. Only values you pass are sent.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReprocessArgs {
    /// Evidence UUID
    #[arg(long = "evidence", value_name = "ID")]
    pub evidence_id: String,

    /// Timezone the source recorded its timestamps in, e.g. Asia/Kolkata
    #[arg(long, value_name = "TIMEZONE")]
    pub source_timezone: Option<String>,

    /// Known clock offset of the source device in seconds, e.g. -30 or 12.5
    #[arg(long, value_name = "SECONDS", allow_hyphen_values = true)]
    pub clock_offset_seconds: Option<String>,

    /// Default telephone country code (digits only) for numbers without one, e.g. 91
    #[arg(long, value_name = "DIGITS")]
    pub default_country_code: Option<String>,

    /// Extra metadata as a JSON object
    #[arg(long, value_name = "JSON")]
    pub metadata: Option<String>,
}

/// Arguments of `yukti evidence verify`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct VerifyEvidenceArgs {
    /// Evidence UUID whose recorded hash is compared
    #[arg(long = "evidence", value_name = "ID")]
    pub evidence_id: String,

    /// Local copy of the evidence file (only read)
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
}

/// Arguments of `yukti evidence ingest`. Optional values you omit are not sent, so an
/// undeclared timezone or clock offset stays undeclared.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct IngestEvidenceArgs {
    /// Case UUID the evidence belongs to (checked with the API before uploading) [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Evidence file to upload
    #[arg(value_name = "FILE")]
    pub file: std::path::PathBuf,

    /// Source type to record instead of the type the API detects from the content
    #[arg(long, value_name = "TYPE")]
    pub source_type: Option<String>,

    /// Timezone the source recorded its timestamps in, e.g. Asia/Kolkata (sent as declared)
    #[arg(long, value_name = "TIMEZONE")]
    pub source_timezone: Option<String>,

    /// Known clock offset of the source device in seconds, e.g. -30 or 12.5 (sent as declared)
    #[arg(long, value_name = "SECONDS", allow_hyphen_values = true)]
    pub clock_offset_seconds: Option<String>,

    /// Extra acquisition metadata as a JSON object
    #[arg(long, value_name = "JSON")]
    pub metadata: Option<String>,

    /// Ask the API to run extraction immediately after ingestion
    #[arg(long)]
    pub auto_extract: bool,
}

/// Arguments of `yukti evidence list`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ListEvidenceArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,
}

/// `yukti case ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum CaseCommand {
    /// List the cases the API returns, in the API's order
    List(ListCasesArgs),
    /// Register a new case and print the UUID the API assigned
    Create(CreateCaseArgs),
    /// Show one case by its UUID
    Show(ShowCaseArgs),
    /// One-screen investigative brief: evidence, reconstruction, leads, gaps, graph, export
    Overview(CaseArgs),
}

/// Arguments of `yukti case list`. Filters apply to the API's list on the client side.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct ListCasesArgs {
    /// Only cases whose status equals this value (case-insensitive)
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,

    /// Only cases whose title, FIR number or description contains this text (case-insensitive)
    #[arg(long, value_name = "TEXT")]
    pub search: Option<String>,

    /// Show at most this many cases
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
}

/// Arguments of `yukti case create`. Only options you pass are sent to the API.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CreateCaseArgs {
    /// Case title (required, must not be blank)
    #[arg(long)]
    pub title: String,

    /// Free-text case description
    #[arg(long)]
    pub description: Option<String>,

    /// FIR / reference number
    #[arg(long, value_name = "FIR")]
    pub fir_number: Option<String>,

    /// Lead investigator identifier
    #[arg(long, value_name = "ID")]
    pub investigator_id: Option<String>,

    /// Extra case metadata as a JSON object, e.g. '{"station":"Saket"}'
    #[arg(long, value_name = "JSON")]
    pub metadata: Option<String>,

    /// Use this UUID for the new case instead of letting the API assign one
    #[arg(long, value_name = "UUID")]
    pub case_id: Option<String>,
}

/// Arguments of `yukti case show`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ShowCaseArgs {
    /// Case UUID (names such as CASE-026 are not accepted) [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,
}

/// Arguments of `yukti timeline`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct TimelineArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Show at most this many entries (the counts still cover the whole timeline)
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,

    /// Only entries that carry at least one labelled assumption
    #[arg(long)]
    pub assumed_only: bool,

    /// Only entries whose ordering is CONCURRENT_UNCERTAIN
    #[arg(long)]
    pub uncertain_only: bool,
}

/// Minimum gap severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Ranks an API severity string; unknown values rank lowest.
    pub fn rank(value: &str) -> u8 {
        match value.to_ascii_uppercase().as_str() {
            "CRITICAL" => 3,
            "HIGH" => 2,
            "MEDIUM" => 1,
            _ => 0,
        }
    }

    pub fn as_rank(self) -> u8 {
        self as u8
    }
}

/// Arguments of `yukti gaps`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct GapsArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Only gaps at or above this severity
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub min_severity: Option<Severity>,
}

/// Arguments of `yukti provenance`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ProvenanceArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Trace only this finding (lead ID, segment ID or timeline event ID)
    #[arg(long, value_name = "ID")]
    pub finding: Option<String>,
}

/// Dossier export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormat {
    /// The markdown dossier exactly as returned by the API
    Markdown,
    /// The complete report JSON exactly as returned by the API
    Json,
}

/// Arguments of `yukti report`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReportArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Write the report to this file instead of printing the dossier
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,

    /// Export format for --out
    #[arg(long, value_enum, default_value_t = ReportFormat::Markdown)]
    pub format: ReportFormat,

    /// Replace --out if it already exists
    #[arg(long)]
    pub force: bool,
}

/// `yukti graph ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum GraphCommand {
    /// Counts of the projected graph and its synchronisation status
    Summary(CaseArgs),
    /// Entities and relationships around one entity
    Neighbors(NeighborsArgs),
    /// Shortest relationship path between two entities
    Path(PathArgs),
}

/// Arguments of `yukti graph neighbors`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct NeighborsArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Graph entity ID or fingerprint
    #[arg(long, value_name = "ID")]
    pub entity: String,

    /// Hops to expand, 1-3 (the API clamps to this range)
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=3))]
    pub depth: Option<u32>,

    /// Maximum neighbours, 1-100 (the API clamps to this range)
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(i64).range(1..=100))]
    pub limit: Option<i64>,
}

/// Arguments of `yukti graph path`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct PathArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Start entity ID or fingerprint
    #[arg(long, value_name = "ID")]
    pub from: String,

    /// End entity ID or fingerprint
    #[arg(long, value_name = "ID")]
    pub to: String,

    /// Maximum path length in hops
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=10))]
    pub max_hops: Option<u32>,
}

/// Arguments of `yukti whatif`. Every option is an alternative assumption; nothing you omit
/// is sent, so the baseline value applies. Numbers are validated as finite before sending.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct WhatIfArgs {
    /// Case UUID [env: YUKTI_CASE]
    #[arg(long = "case", value_name = "UUID", env = ENV_CASE, hide_env = true)]
    pub case_id: String,

    /// Name of the hypothesis being tested
    #[arg(long, value_name = "TEXT")]
    pub name: Option<String>,

    /// Default clock offset in seconds for all exhibits
    #[arg(long, value_name = "SECONDS", allow_hyphen_values = true)]
    pub clock_offset: Option<String>,

    /// Clock offset for one exhibit, EVIDENCE_ID=SECONDS (repeatable)
    #[arg(long, value_name = "ID=SECONDS", allow_hyphen_values = true)]
    pub exhibit_offset: Vec<String>,

    /// Default IANA timezone, e.g. Asia/Kolkata
    #[arg(long, value_name = "TIMEZONE")]
    pub timezone: Option<String>,

    /// Speed threshold in km/h for transit-impossibility leads
    #[arg(long, value_name = "KMH")]
    pub speed_threshold: Option<String>,

    /// Timestamp uncertainty half-width in seconds
    #[arg(long, value_name = "SECONDS")]
    pub half_width: Option<String>,

    /// Leave one evidence exhibit out of the scenario (repeatable)
    #[arg(long, value_name = "ID")]
    pub exclude_evidence: Vec<String>,

    /// Leave one event out of the scenario (repeatable)
    #[arg(long, value_name = "ID")]
    pub exclude_event: Vec<String>,

    /// Full ScenarioSimulationRequest JSON file; flags above override its assumptions
    #[arg(long, value_name = "FILE")]
    pub scenario_file: Option<PathBuf>,
}

/// Arguments of `yukti completions`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct CompletionsArgs {
    /// Target shell
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_status_with_global_options_in_any_position() {
        let cli = Cli::try_parse_from(["yukti", "status"]).unwrap();
        assert_eq!(cli.command, Command::Status);
        assert!(!cli.json);
        assert_eq!(cli.api_url, None);
        assert_eq!(cli.color, ColorChoice::Auto);
        assert!(!cli.ascii);

        let cli =
            Cli::try_parse_from(["yukti", "--api-url", "http://h:1", "--json", "status"]).unwrap();
        assert_eq!(cli.api_url.as_deref(), Some("http://h:1"));
        assert!(cli.json);

        let cli = Cli::try_parse_from([
            "yukti",
            "status",
            "--json",
            "--timeout",
            "3",
            "--color",
            "never",
            "--ascii",
        ])
        .unwrap();
        assert!(cli.json);
        assert_eq!(cli.timeout, Some(3));
        assert_eq!(cli.color, ColorChoice::Never);
        assert!(cli.ascii);
    }

    #[test]
    fn parses_case_commands() {
        let cli = Cli::try_parse_from(["yukti", "case", "list", "--json"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Case(CaseCommand::List(ListCasesArgs::default()))
        );
        assert!(cli.json);

        let cli = Cli::try_parse_from([
            "yukti", "case", "list", "--status", "open", "--search", "atm", "--limit", "2",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Command::Case(CaseCommand::List(ListCasesArgs {
                status: Some("open".into()),
                search: Some("atm".into()),
                limit: Some(2),
            }))
        );

        let cli = Cli::try_parse_from([
            "yukti",
            "case",
            "create",
            "--title",
            "Warehouse burglary",
            "--fir-number",
            "FIR-12/2026",
            "--metadata",
            r#"{"station":"Saket"}"#,
        ])
        .unwrap();
        match cli.command {
            Command::Case(CaseCommand::Create(args)) => {
                assert_eq!(args.title, "Warehouse burglary");
                assert_eq!(args.fir_number.as_deref(), Some("FIR-12/2026"));
                assert_eq!(args.metadata.as_deref(), Some(r#"{"station":"Saket"}"#));
                assert_eq!(args.description, None);
                assert_eq!(args.case_id, None);
            }
            other => panic!("unexpected {other:?}"),
        }

        let cli = Cli::try_parse_from(["yukti", "case", "show", "--case", "abc"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Case(CaseCommand::Show(ShowCaseArgs {
                case_id: "abc".into()
            }))
        );
        let cli = Cli::try_parse_from(["yukti", "case", "overview", "--case", "abc"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Case(CaseCommand::Overview(CaseArgs {
                case_id: "abc".into()
            }))
        );
    }

    #[test]
    fn case_commands_require_their_options() {
        for args in [
            vec!["yukti", "case"],
            vec!["yukti", "case", "create"],
            vec!["yukti", "case", "show"],
            vec!["yukti", "case", "overview"],
            vec!["yukti", "case", "delete"],
        ] {
            let err = Cli::try_parse_from(&args).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{args:?}");
        }
    }

    #[test]
    fn parses_reconstruction_commands() {
        for (name, expected) in [
            ("run", ReconstructionCommand::Run as fn(_) -> _),
            ("explain", ReconstructionCommand::Explain),
            ("compare", ReconstructionCommand::Compare),
        ] {
            let cli =
                Cli::try_parse_from(["yukti", "reconstruction", name, "--case", "abc"]).unwrap();
            assert_eq!(
                cli.command,
                Command::Reconstruction(expected(ReconstructionArgs {
                    case_id: "abc".into()
                }))
            );
        }
        for args in [
            vec!["yukti", "reconstruction"],
            vec!["yukti", "reconstruction", "run"],
            vec!["yukti", "reconstruction", "explain"],
            vec!["yukti", "reconstruction", "compare"],
            vec!["yukti", "reconstruction", "delete", "--case", "abc"],
        ] {
            let err = Cli::try_parse_from(&args).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{args:?}");
        }
    }

    #[test]
    fn parses_analysis_commands() {
        let cli = Cli::try_parse_from([
            "yukti",
            "timeline",
            "--case",
            "c",
            "--limit",
            "5",
            "--assumed-only",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Command::Timeline(TimelineArgs {
                case_id: "c".into(),
                limit: Some(5),
                assumed_only: true,
                uncertain_only: false,
            })
        );
        let cli = Cli::try_parse_from(["yukti", "gaps", "--case", "c", "--min-severity", "high"])
            .unwrap();
        assert_eq!(
            cli.command,
            Command::Gaps(GapsArgs {
                case_id: "c".into(),
                min_severity: Some(Severity::High)
            })
        );
        let cli = Cli::try_parse_from(["yukti", "report", "--case", "c", "--out", "d.md"]).unwrap();
        match cli.command {
            Command::Report(r) => {
                assert_eq!(r.out, Some(PathBuf::from("d.md")));
                assert_eq!(r.format, ReportFormat::Markdown);
                assert!(!r.force);
            }
            other => panic!("unexpected {other:?}"),
        }
        let cli = Cli::try_parse_from([
            "yukti",
            "graph",
            "neighbors",
            "--case",
            "c",
            "--entity",
            "e1",
            "--depth",
            "2",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Graph(GraphCommand::Neighbors(_))
        ));
        assert!(Cli::try_parse_from([
            "yukti",
            "graph",
            "neighbors",
            "--case",
            "c",
            "--entity",
            "e",
            "--depth",
            "9"
        ])
        .is_err());
    }

    #[test]
    fn parses_whatif_with_repeatable_options() {
        let cli = Cli::try_parse_from([
            "yukti",
            "what-if",
            "--case",
            "c",
            "--clock-offset",
            "-120",
            "--exhibit-offset",
            "ev1=30",
            "--exhibit-offset",
            "ev2=-5",
            "--exclude-event",
            "evt-9",
        ])
        .unwrap();
        match cli.command {
            Command::WhatIf(w) => {
                assert_eq!(w.clock_offset.as_deref(), Some("-120"));
                assert_eq!(w.exhibit_offset, vec!["ev1=30", "ev2=-5"]);
                assert_eq!(w.exclude_event, vec!["evt-9"]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_usage() {
        use clap::error::ErrorKind;
        let missing = Cli::try_parse_from(["yukti"]).unwrap_err();
        assert_eq!(missing.exit_code(), 2);
        let unknown = Cli::try_parse_from(["yukti", "frobnicate"]).unwrap_err();
        assert_eq!(unknown.kind(), ErrorKind::InvalidSubcommand);
        assert_eq!(unknown.exit_code(), 2);
        let bad_timeout =
            Cli::try_parse_from(["yukti", "--timeout", "soon", "status"]).unwrap_err();
        assert_eq!(bad_timeout.exit_code(), 2);
        let bad_color = Cli::try_parse_from(["yukti", "--color", "red", "status"]).unwrap_err();
        assert_eq!(bad_color.exit_code(), 2);
        let help = Cli::try_parse_from(["yukti", "--help"]).unwrap_err();
        assert_eq!(help.exit_code(), 0);
    }

    #[test]
    fn parses_server_commands() {
        let cli = Cli::try_parse_from(["yukti", "server", "start"]).unwrap();
        assert_eq!(cli.command, Command::Server(ServerCommand::Start));
        let cli = Cli::try_parse_from(["yukti", "server", "logs", "--lines", "5"]).unwrap();
        assert_eq!(
            cli.command,
            Command::Server(ServerCommand::Logs { lines: 5 })
        );
        assert!(Cli::try_parse_from(["yukti", "server"]).is_err());
    }

    #[test]
    fn severity_ranks_follow_the_api_scale() {
        assert!(Severity::rank("CRITICAL") > Severity::rank("HIGH"));
        assert_eq!(Severity::rank("LOW"), Severity::Low.as_rank());
        assert_eq!(Severity::rank("HIGH"), Severity::High.as_rank());
        assert_eq!(Severity::rank("weird"), 0);
    }
}
