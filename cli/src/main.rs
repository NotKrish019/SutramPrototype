use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use yukti_cli::cli::Cli;
use yukti_cli::config::EnvVars;
use yukti_cli::error::exit;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
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

    let rendered = runtime.block_on(yukti_cli::execute(&cli, &EnvVars::from_process()));

    // Write errors (for example a closed pipe) must not turn into a panic.
    let _ = std::io::stdout().write_all(rendered.stdout.as_bytes());
    let _ = std::io::stderr().write_all(rendered.stderr.as_bytes());
    ExitCode::from(rendered.exit_code)
}
