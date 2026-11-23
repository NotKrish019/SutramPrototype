//! The local YUKTI server bundled with installed builds.
//!
//! Installed archives ship two programs side by side: `yukti` and `yukti-server` (the Rust API,
//! `backend-rust`, renamed). When a command needs the API at a loopback address and nothing answers
//! there, `yukti` starts `yukti-server` in the background. It runs in filesystem mode, with data
//! under the YUKTI home directory, and `yukti` waits until `/health` answers. The CLI still talks to
//! it over HTTP only; this module only manages the process.
//!
//! * Home: `YUKTI_HOME`, else `%LOCALAPPDATA%\yukti` on Windows, else `~/.yukti`.
//! * Server binary: `YUKTI_SERVER_BIN`, else `yukti-server[.exe]` next to the `yukti` executable.
//! * `YUKTI_NO_AUTOSTART=1` turns automatic starting off.
//!
//! Evidence written by the bundled server lives in `<home>/data/evidence/{raw,derived}`; the CLI
//! never touches those files itself.

use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::cli::ServerCommand;
use crate::client::ApiClient;
use crate::config::Config;
use crate::error::{exit, CliError};
use crate::output::{render_error_with_notes, to_json, Rendered};
use crate::ui::{Badge, Tone, Ui, MARGIN};

pub const ENV_HOME: &str = "YUKTI_HOME";
pub const ENV_SERVER_BIN: &str = "YUKTI_SERVER_BIN";
pub const ENV_NO_AUTOSTART: &str = "YUKTI_NO_AUTOSTART";
pub const SERVER_STATUS_SCHEMA: &str = "yukti.cli.server/v1";

/// File name of the bundled server next to `yukti`.
pub const SERVER_BIN_NAME: &str = if cfg!(windows) {
    "yukti-server.exe"
} else {
    "yukti-server"
};

/// How long to wait for a freshly started server to answer `/health`.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Where the bundled server keeps its files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Paths {
    pub home: PathBuf,
    pub evidence_raw: PathBuf,
    pub evidence_derived: PathBuf,
    pub log: PathBuf,
    pub pid: PathBuf,
}

impl Paths {
    pub fn under(home: PathBuf) -> Self {
        Self {
            evidence_raw: home.join("data").join("evidence").join("raw"),
            evidence_derived: home.join("data").join("evidence").join("derived"),
            log: home.join("server.log"),
            pid: home.join("server.pid"),
            home,
        }
    }

    /// The YUKTI home for this user, or `None` when no home directory can be found.
    pub fn resolve() -> Option<Self> {
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
        let home = if let Some(h) = var(ENV_HOME) {
            PathBuf::from(h)
        } else if cfg!(windows) {
            PathBuf::from(var("LOCALAPPDATA").or_else(|| var("USERPROFILE"))?).join("yukti")
        } else {
            PathBuf::from(var("HOME")?).join(".yukti")
        };
        Some(Self::under(home))
    }
}

/// The bundled server binary, if this installation has one.
pub fn find_server_binary() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os(ENV_SERVER_BIN).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(SERVER_BIN_NAME);
    candidate.is_file().then_some(candidate)
}

/// True when the API URL points at this machine, so a local server can serve it.
pub fn is_loopback(config: &Config) -> bool {
    matches!(
        config.api_url.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    )
}

/// Port the local server should listen on (from the API URL).
pub fn port(config: &Config) -> u16 {
    config.api_url.port_or_known_default().unwrap_or(8080)
}

/// `true` when `GET /health` answers with a 2xx.
pub async fn is_healthy(client: &ApiClient) -> bool {
    matches!(client.health().await, Ok(raw) if raw.is_success())
}

/// Outcome of [`ensure_running`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ensure {
    AlreadyRunning,
    Started { pid: u32, waited: Duration },
}

/// Starts the bundled server unless the API already answers.
pub async fn ensure_running(
    config: &Config,
    client: &ApiClient,
    binary: &Path,
    paths: &Paths,
) -> Result<Ensure, CliError> {
    if is_healthy(client).await {
        return Ok(Ensure::AlreadyRunning);
    }
    if !is_loopback(config) {
        return Err(CliError::Config(format!(
            "the API URL {} is not on this machine, so the bundled server cannot serve it",
            config.base_url()
        )));
    }
    let mut child = spawn(binary, paths, port(config))?;
    let pid = child.id();
    let started = Instant::now();
    while started.elapsed() < STARTUP_TIMEOUT {
        if is_healthy(client).await {
            let _ = std::fs::write(&paths.pid, pid.to_string());
            return Ok(Ensure::Started {
                pid,
                waited: started.elapsed(),
            });
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(CliError::Internal(format!(
                "the YUKTI server exited during startup ({status}). Last log lines:\n{}",
                tail(&paths.log, 12)
            )));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(CliError::Timeout {
        url: client.url_for(crate::client::HEALTH_PATH),
        timeout: STARTUP_TIMEOUT,
    })
}

/// Starts `binary` detached from this terminal, logging to `paths.log`.
fn spawn(binary: &Path, paths: &Paths, port: u16) -> Result<std::process::Child, CliError> {
    let io = |what: &str, e: std::io::Error| {
        CliError::Internal(format!("cannot {what} for the YUKTI server: {e}"))
    };
    for dir in [&paths.home, &paths.evidence_raw, &paths.evidence_derived] {
        std::fs::create_dir_all(dir).map_err(|e| io(&format!("create {}", dir.display()), e))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .map_err(|e| io("open the log file", e))?;
    let log_err = log.try_clone().map_err(|e| io("open the log file", e))?;

    let mut cmd = Process::new(binary);
    cmd.current_dir(&paths.home)
        .env("HOST", "127.0.0.1")
        .env("PORT", port.to_string())
        .env("EVIDENCE_STORAGE_PATH", &paths.evidence_raw)
        .env("DERIVED_STORAGE_PATH", &paths.evidence_derived)
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        )
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);
    detach(&mut cmd);
    cmd.spawn()
        .map_err(|e| io(&format!("start {}", binary.display()), e))
}

#[cfg(unix)]
fn detach(cmd: &mut Process) {
    use std::os::unix::process::CommandExt;
    // Own process group: Ctrl-C in this terminal does not stop the server.
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Process) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(not(any(unix, windows)))]
fn detach(_cmd: &mut Process) {}

/// Last `lines` lines of a text file (empty when it cannot be read).
pub fn tail(path: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

fn read_pid(paths: &Paths) -> Option<u32> {
    std::fs::read_to_string(&paths.pid)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn kill(pid: u32) -> Result<(), CliError> {
    let status = if cfg!(windows) {
        Process::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        Process::new("kill")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(CliError::Internal(format!(
            "could not stop process {pid} ({s}); it may already have exited"
        ))),
        Err(e) => Err(CliError::Internal(format!(
            "could not stop process {pid}: {e}"
        ))),
    }
}

/* ------------------------------------------------------------------------ yukti server */

#[derive(Serialize)]
struct ServerDocument<'a> {
    schema: &'static str,
    action: &'static str,
    api_url: String,
    running: bool,
    /// PID recorded by `yukti` when it started the server, or `null`.
    pid: Option<u32>,
    bundled_server: Option<String>,
    paths: Option<&'a Paths>,
}

/// `yukti server start | stop | status | logs`.
pub async fn run(
    command: &ServerCommand,
    config: &Config,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Rendered {
    let paths = Paths::resolve();
    let binary = find_server_binary();
    let fail = |e: CliError, notes: &[String]| render_error_with_notes(&e, json, notes);

    let action = match command {
        ServerCommand::Start => "start",
        ServerCommand::Stop => "stop",
        ServerCommand::Status => "status",
        ServerCommand::Logs { .. } => "logs",
    };

    match command {
        ServerCommand::Logs { lines } => {
            let Some(p) = &paths else {
                return fail(CliError::Config("no home directory found".into()), &[]);
            };
            let text = tail(&p.log, *lines);
            if text.is_empty() {
                return Rendered::stdout(
                    format!(
                        "{MARGIN}{}\n",
                        ui.paint(Tone::Muted, &format!("No log at {}", p.log.display()))
                    ),
                    exit::SUCCESS,
                );
            }
            return Rendered::stdout(format!("{text}\n"), exit::SUCCESS);
        }
        ServerCommand::Start => {
            let (Some(bin), Some(p)) = (&binary, &paths) else {
                return fail(
                    CliError::Config(format!(
                        "no bundled server: {SERVER_BIN_NAME} is not next to yukti and {ENV_SERVER_BIN} is not set"
                    )),
                    &["Install YUKTI with the installer (it ships yukti-server), or start backend-rust yourself: cargo run --bin sutram-backend-rust".to_string()],
                );
            };
            if let Err(e) = ensure_running(config, client, bin, p).await {
                return fail(e, &[format!("Log: {}", p.log.display())]);
            }
        }
        ServerCommand::Stop => {
            let Some(p) = &paths else {
                return fail(CliError::Config("no home directory found".into()), &[]);
            };
            match read_pid(p) {
                None if is_healthy(client).await => {
                    return fail(
                        CliError::InvalidArgument(format!(
                            "a server answers at {} but was not started by yukti; stop it where it was started",
                            config.base_url()
                        )),
                        &[],
                    )
                }
                None => {}
                Some(pid) => {
                    if let Err(e) = kill(pid) {
                        let _ = std::fs::remove_file(&p.pid);
                        return fail(e, &[]);
                    }
                    let _ = std::fs::remove_file(&p.pid);
                    // Give it a moment to release the port.
                    for _ in 0..20 {
                        if !is_healthy(client).await {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(150)).await;
                    }
                }
            }
        }
        ServerCommand::Status => {}
    }

    let running = is_healthy(client).await;
    let pid = paths.as_ref().and_then(read_pid);
    if json {
        let doc = ServerDocument {
            schema: SERVER_STATUS_SCHEMA,
            action,
            api_url: config.base_url(),
            running,
            pid,
            bundled_server: binary.as_ref().map(|b| b.display().to_string()),
            paths: paths.as_ref(),
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let mut out = ui.header(&["server", action], &config.base_url());
    out.push('\n');
    out.push_str(&format!(
        "{MARGIN}{}\n",
        if running {
            ui.badge(Badge::Ok, "running")
        } else {
            ui.badge(Badge::Off, "stopped")
        }
    ));
    out.push_str(&ui.section("Local server", ""));
    out.push_str(&ui.kv("API URL", Tone::Text, &config.base_url()));
    out.push_str(&ui.kv(
        "Started by yukti",
        Tone::Text,
        &pid.map_or("no".to_string(), |p| format!("yes, pid {p}")),
    ));
    out.push_str(
        &ui.kv(
            "Bundled server",
            Tone::Muted,
            &binary
                .as_ref()
                .map_or("not installed".to_string(), |b| b.display().to_string()),
        ),
    );
    if let Some(p) = &paths {
        out.push_str(&ui.kv(
            "Data",
            Tone::Muted,
            &p.home.join("data").display().to_string(),
        ));
        out.push_str(&ui.kv("Log", Tone::Muted, &p.log.display().to_string()));
    }
    out.push('\n');
    if running {
        out.push_str(&ui.next("use it", "yukti"));
        out.push_str(&ui.next("stop", "yukti server stop"));
    } else {
        out.push_str(&ui.next("start", "yukti server start"));
    }
    Rendered::stdout(out, exit::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_live_under_the_home() {
        let p = Paths::under(PathBuf::from("/h"));
        assert_eq!(p.evidence_raw, PathBuf::from("/h/data/evidence/raw"));
        assert_eq!(
            p.evidence_derived,
            PathBuf::from("/h/data/evidence/derived")
        );
        assert_eq!(p.log, PathBuf::from("/h/server.log"));
    }

    #[test]
    fn loopback_and_port_come_from_the_api_url() {
        let env = crate::config::EnvVars::default();
        let local = Config::resolve(Some("http://localhost:9000"), None, &env).unwrap();
        assert!(is_loopback(&local));
        assert_eq!(port(&local), 9000);
        let default = Config::resolve(None, None, &env).unwrap();
        assert!(is_loopback(&default));
        assert_eq!(port(&default), 8080);
        let remote = Config::resolve(Some("http://10.0.0.5:8080"), None, &env).unwrap();
        assert!(!is_loopback(&remote));
    }

    #[test]
    fn tail_returns_the_last_lines() {
        let dir = std::env::temp_dir().join(format!("yukti-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("log");
        std::fs::write(&f, "a\nb\nc\n").unwrap();
        assert_eq!(tail(&f, 2), "b\nc");
        assert_eq!(tail(&dir.join("missing"), 2), "");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
