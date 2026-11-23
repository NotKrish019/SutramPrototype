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

use cli::{Cli, Command};
use client::ApiClient;
use config::{Config, EnvVars};
use output::Rendered;

/// Resolves configuration, runs the parsed command and returns its rendered output.
pub async fn execute(cli: &Cli, env: &EnvVars) -> Rendered {
    // Doctor reports an invalid configuration as a failed check, so it resolves it itself.
    if cli.command == Command::Doctor {
        return commands::doctor::run(cli.api_url.as_deref(), cli.timeout, env, cli.json).await;
    }
    let config = match Config::resolve(cli.api_url.as_deref(), cli.timeout, env) {
        Ok(config) => config,
        Err(err) => return output::render_error(&err, cli.json),
    };
    let client = match ApiClient::new(&config) {
        Ok(client) => client,
        Err(err) => return output::render_error(&err, cli.json),
    };
    commands::dispatch(&cli.command, &client, cli.json).await
}
