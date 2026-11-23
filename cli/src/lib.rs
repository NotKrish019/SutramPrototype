//! YUKTI command-line client.
//!
//! A standalone HTTP client for the YUKTI Rust API (`backend-rust/`). It shares no code with
//! the backend and never connects to PostgreSQL or Neo4j directly.

pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod dto;
pub mod error;
pub mod output;
pub mod server;
pub mod shell;
pub mod ui;

use cli::{Cli, Command};
use client::ApiClient;
use config::{Config, EnvVars};
use error::CliError;
use output::Rendered;
use ui::Ui;

/// Resolves configuration, runs the parsed command and returns its rendered output.
///
/// Human output is styled for the terminal described by `env.terminal`; the default
/// `EnvVars` describes a pipe, so output is uncoloured.
pub async fn execute(cli: &Cli, env: &EnvVars) -> Rendered {
    let ui = Ui::resolve(cli.color, cli.ascii, &env.terminal);
    let mut rendered = execute_with_ui(cli, env, &ui).await;
    rendered.stderr = ui.for_stderr().style_diagnostics(&rendered.stderr);
    rendered
}

async fn execute_with_ui(cli: &Cli, env: &EnvVars, ui: &Ui) -> Rendered {
    match &cli.command {
        // Doctor reports an invalid configuration as a failed check, so it resolves it itself.
        Command::Doctor => {
            return commands::doctor::run(cli.api_url.as_deref(), cli.timeout, env, cli.json).await
        }
        Command::Completions(args) => return commands::completions::run(args),
        Command::Shell | Command::Demo(_) => {
            let err = CliError::InvalidArgument(
                "`yukti shell` and `yukti demo` are interactive and run only from the yukti binary"
                    .to_string(),
            );
            return output::render_error(&err, cli.json);
        }
        _ => {}
    }
    let config = match Config::resolve(cli.api_url.as_deref(), cli.timeout, env) {
        Ok(config) => config,
        Err(err) => return output::render_error(&err, cli.json),
    };
    if cli.command == Command::Config {
        return commands::config::run(&config, env, cli, ui);
    }
    let client = match ApiClient::new(&config) {
        Ok(client) => client,
        Err(err) => return output::render_error(&err, cli.json),
    };
    if let Command::Server(command) = &cli.command {
        return server::run(command, &config, &client, cli.json, ui).await;
    }
    commands::dispatch(&cli.command, &client, cli.json, ui).await
}

/// Resolves the configuration and builds an API client (for commands that stream output).
pub fn client_for(cli: &Cli, env: &EnvVars) -> Result<ApiClient, CliError> {
    let config = Config::resolve(cli.api_url.as_deref(), cli.timeout, env)?;
    ApiClient::new(&config)
}
