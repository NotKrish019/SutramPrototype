//! Command-line interface definition.
//!
//! New command groups (`case`, `evidence`, `reconstruction`, `gaps`, `graph`) are added as
//! variants of [`Command`] with their own argument structs and a handler under
//! `src/commands/`. Global options apply to every command.

use clap::{Args, Parser, Subcommand};

/// Command-line client for the YUKTI forensic API.
#[derive(Debug, Parser)]
#[command(name = "yukti", version, propagate_version = true)]
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

    /// Manage investigation cases
    #[command(subcommand)]
    Case(CaseCommand),

    /// Ingest and list evidence for a case
    #[command(subcommand)]
    Evidence(EvidenceCommand),

    /// Run and explain a case's timeline, spatial and contradiction reconstruction
    #[command(subcommand)]
    Reconstruction(ReconstructionCommand),
}

/// `yukti reconstruction ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ReconstructionCommand {
    /// Ask the API to reconstruct a case from the events it holds, with its default options
    Run(ReconstructionArgs),
    /// Show the API's current reconstruction of a case and its judicial report
    Explain(ReconstructionArgs),
    /// Compare reconstruction scenarios (not yet available in the API; exits 69)
    Compare(ReconstructionArgs),
}

/// Arguments shared by the `yukti reconstruction` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReconstructionArgs {
    /// Case UUID (checked with the API before anything else is requested)
    #[arg(long = "case", value_name = "UUID")]
    pub case_id: String,
}

/// `yukti evidence ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum EvidenceCommand {
    /// Upload one evidence file to a case (the file is only read, never modified)
    Ingest(IngestEvidenceArgs),
    /// List the evidence receipts of a case, in the API's order
    List(ListEvidenceArgs),
}

/// Arguments of `yukti evidence ingest`. Optional values you omit are not sent, so an
/// undeclared timezone or clock offset stays undeclared.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct IngestEvidenceArgs {
    /// Case UUID the evidence belongs to (checked with the API before uploading)
    #[arg(long = "case", value_name = "UUID")]
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
    /// Case UUID
    #[arg(long = "case", value_name = "UUID")]
    pub case_id: String,
}

/// `yukti case ...` subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum CaseCommand {
    /// List the cases the API returns, in the API's order
    List,
    /// Register a new case and print the UUID the API assigned
    Create(CreateCaseArgs),
    /// Show one case by its UUID
    Show(ShowCaseArgs),
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
    /// Case UUID (names such as CASE-026 are not accepted)
    #[arg(long = "case", value_name = "UUID")]
    pub case_id: String,
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

        let cli =
            Cli::try_parse_from(["yukti", "--api-url", "http://h:1", "--json", "status"]).unwrap();
        assert_eq!(cli.api_url.as_deref(), Some("http://h:1"));
        assert!(cli.json);

        let cli = Cli::try_parse_from(["yukti", "status", "--json", "--timeout", "3"]).unwrap();
        assert!(cli.json);
        assert_eq!(cli.timeout, Some(3));
    }

    #[test]
    fn parses_case_commands() {
        let cli = Cli::try_parse_from(["yukti", "case", "list", "--json"]).unwrap();
        assert_eq!(cli.command, Command::Case(CaseCommand::List));
        assert!(cli.json);

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
    }

    #[test]
    fn case_commands_require_their_options() {
        for args in [
            vec!["yukti", "case"],
            vec!["yukti", "case", "create"],
            vec!["yukti", "case", "show"],
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
        let help = Cli::try_parse_from(["yukti", "--help"]).unwrap_err();
        assert_eq!(help.exit_code(), 0);
    }
}
