use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;
use yukti_cli::cli::{Cli, Command};
use yukti_cli::client::ApiClient;
use yukti_cli::config::{Config, EnvVars};
use yukti_cli::error::exit;
use yukti_cli::server::{self, Ensure, Paths, ENV_NO_AUTOSTART};
use yukti_cli::ui::{Glyph, Tone, Ui, MARGIN};

fn main() -> ExitCode {
    // Plain `yukti` in a terminal opens the interactive session, like other assistant CLIs.
    // Without a terminal (scripts, pipes) it keeps printing usage and exiting 2.
    let bare = std::env::args_os().len() == 1;
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let parsed = if bare && interactive {
        Cli::try_parse_from(["yukti", "shell"])
    } else {
        Cli::try_parse()
    };
    let cli = match parsed {
        Ok(cli) => cli,
        Err(err) => {
            // clap exits 0 for --help/--version and 2 for usage errors.
            let code = u8::try_from(err.exit_code()).unwrap_or(exit::USAGE);
            let _ = err.print();
            return ExitCode::from(code);
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: internal CLI error: failed to start the async runtime: {err}");
            return ExitCode::from(exit::INTERNAL);
        }
    };

    let env = EnvVars::from_process();
    if wants_local_server(&cli.command) {
        autostart(&cli, &env, &runtime);
    }
    if cli.command == Command::Shell {
        return yukti_cli::shell::run_interactive(&cli, &env, &runtime);
    }

    let rendered = runtime.block_on(yukti_cli::execute(&cli, &env));

    // Write errors (for example a closed pipe) must not turn into a panic.
    let _ = std::io::stdout().write_all(rendered.stdout.as_bytes());
    let _ = std::io::stderr().write_all(rendered.stderr.as_bytes());
    ExitCode::from(rendered.exit_code)
}

/// Commands that need the API and may start the bundled server. `status` and `doctor` report
/// the server as it is; `server`, `config` and `completions` manage it or need no API.
fn wants_local_server(command: &Command) -> bool {
    !matches!(
        command,
        Command::Status
            | Command::Doctor
            | Command::Config
            | Command::Completions(_)
            | Command::Server(_)
    )
}

/// Starts the bundled `yukti-server` when the API URL is local and nothing answers there.
/// Failure is reported and the command then runs as usual (and reports the API unreachable).
fn autostart(cli: &Cli, env: &EnvVars, runtime: &tokio::runtime::Runtime) {
    if std::env::var(ENV_NO_AUTOSTART).is_ok_and(|v| !v.is_empty() && v != "0") {
        return;
    }
    let Some(binary) = server::find_server_binary() else {
        return;
    };
    let Ok(config) = Config::resolve(cli.api_url.as_deref(), cli.timeout, env) else {
        return;
    };
    if !server::is_loopback(&config) {
        return;
    }
    let (Some(paths), Ok(client)) = (Paths::resolve(), ApiClient::new(&config)) else {
        return;
    };
    if runtime.block_on(server::is_healthy(&client)) {
        return;
    }
    let ui = Ui::resolve(cli.color, cli.ascii, &env.terminal).for_stderr();
    let say = |text: String| {
        if !cli.json {
            let _ = std::io::stderr().write_all(text.as_bytes());
        }
    };
    say(format!(
        "{MARGIN}{} {}\n",
        ui.paint(Tone::Brand, ui.g(Glyph::Bar)),
        ui.paint(Tone::Muted, "Starting the local YUKTI server…")
    ));
    match runtime.block_on(server::ensure_running(&config, &client, &binary, &paths)) {
        Ok(Ensure::Started { pid, waited }) => say(format!(
            "{MARGIN}{} {}\n\n",
            ui.mark(Glyph::Check, Tone::Ok),
            ui.paint(
                Tone::Muted,
                &format!(
                    "Server ready at {} in {:.1}s (pid {pid}, data in {}). Stop it with `yukti server stop`.",
                    config.base_url(),
                    waited.as_secs_f64(),
                    paths.home.display()
                )
            )
        )),
        Ok(Ensure::AlreadyRunning) => {}
        Err(err) => say(ui.style_diagnostics(&format!(
            "warning: the local YUKTI server could not be started: {err}\nnote: see `yukti server logs`; set {ENV_NO_AUTOSTART}=1 to stop trying.\n"
        ))),
    }
}
