//! `yukti shell`: an interactive session.
//!
//! Every line is parsed as a `yukti` command (the leading `yukti` is optional) and run through
//! the same code path as the binary, with the global options the shell was started with.
//! `use <uuid>` sets a working case that is added as `--case` to any command that accepts it.
//!
//! Built-ins: `use [UUID]`, `unuse`, `help`, `clear`, `banner`, `exit` / `quit` / Ctrl-D.

use std::io::{BufRead, Write};
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::EnvVars;
use crate::dto::CaseId;
use crate::ui::{short_id, Cell, ColorChoice, Column, Glyph, Tone, Ui, MARGIN};

/// State kept between lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub case: Option<String>,
    pub last_exit: u8,
}

/// What to do with one input line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Run this argv (starting with `yukti`).
    Run(Vec<String>),
    /// Print this text (a built-in's output).
    Print(String),
    /// Print to stderr (a usage error).
    Error(String),
    /// Clear the screen.
    Clear,
    Exit,
    Nothing,
}

/// Splits a line into words, honouring single quotes, double quotes and backslash escapes.
pub fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut chars = line.chars();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') | (None, '\\') => match chars.next() {
                Some(n) => {
                    current.push(n);
                    in_word = true;
                }
                None => return Err("line ends with a lone backslash".to_string()),
            },
            (Some(_), c) => current.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            (None, c) => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!("unterminated {q} quote"));
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

/// Global options of the launching command, re-applied to every line.
pub fn global_args(cli: &Cli) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(url) = &cli.api_url {
        args.extend(["--api-url".to_string(), url.clone()]);
    }
    if let Some(t) = cli.timeout {
        args.extend(["--timeout".to_string(), t.to_string()]);
    }
    match cli.color {
        ColorChoice::Auto => {}
        ColorChoice::Always => args.extend(["--color".to_string(), "always".to_string()]),
        ColorChoice::Never => args.extend(["--color".to_string(), "never".to_string()]),
    }
    if cli.ascii {
        args.push("--ascii".to_string());
    }
    args
}

fn help_text(ui: &Ui) -> String {
    let rows = |items: &[(&str, &str)], tone: Tone| -> Vec<Vec<Cell>> {
        items
            .iter()
            .map(|(cmd, what)| vec![Cell::new(*cmd, tone), Cell::new(*what, Tone::Muted)])
            .collect()
    };
    let mut out = ui.section("Shell", "built-ins");
    out.push_str(&ui.table(
        &[Column::left("BUILT-IN"), Column::left("DOES")],
        &rows(
            &[
                (
                    "use <uuid>",
                    "set the working case (added as --case automatically)",
                ),
                ("use", "show the working case"),
                ("unuse", "clear the working case"),
                ("clear", "clear the screen"),
                ("banner", "show the YUKTI banner"),
                ("help", "this list; `<command> --help` for any command"),
                ("exit", "leave the shell (also quit, Ctrl-D)"),
            ],
            Tone::Strong,
        ),
    ));
    out.push_str(&ui.section("Commands", "type them without `yukti`"));
    out.push_str(&ui.table(
        &[Column::left("COMMAND"), Column::left("FOR")],
        &rows(
            &[
                (
                    "status · doctor · config",
                    "API health, diagnostics, settings",
                ),
                (
                    "case list|create|show|overview",
                    "cases and the investigative brief",
                ),
                (
                    "evidence ingest|list|show|verify",
                    "exhibits and chain of custody",
                ),
                (
                    "evidence extract|reprocess|runs|extraction",
                    "versioned extraction",
                ),
                ("reconstruction run|explain", "reconstruct the case"),
                ("timeline · spatial", "reconstructed events and movement"),
                ("contradictions · gaps", "leads and missing evidence"),
                ("provenance · report", "traceability and the dossier"),
                ("graph summary|neighbors|path", "knowledge graph"),
                ("whatif", "hypothetical scenarios"),
            ],
            Tone::Text,
        ),
    ));
    out
}

/// Commands people paste from Terminal or PowerShell guides. None is a yukti command.
const TERMINAL_COMMANDS: [&str; 44] = [
    "cd",
    "ls",
    "dir",
    "pwd",
    "mkdir",
    "rmdir",
    "rm",
    "cp",
    "mv",
    "cat",
    "touch",
    "echo",
    "export",
    "set",
    "unset",
    "source",
    "curl",
    "wget",
    "irm",
    "iwr",
    "iex",
    "open",
    "notepad",
    "start",
    "sudo",
    "brew",
    "apt",
    "git",
    "cargo",
    "npm",
    "python",
    "python3",
    "pip",
    "sh",
    "bash",
    "zsh",
    "pwsh",
    "powershell",
    "chmod",
    "which",
    "where",
    "tar",
    "unzip",
    "shasum",
];

/// A friendly explanation when a line is a Terminal/PowerShell command rather than a yukti one.
pub fn terminal_command_hint(first: &str) -> Option<String> {
    let lower = first.to_ascii_lowercase();
    let is_assignment = first.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    let looks_terminal = TERMINAL_COMMANDS.contains(&lower.as_str())
        || lower.contains('-')
            && lower.split('-').count() == 2
            && lower
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && first.chars().next().is_some_and(char::is_uppercase)
        || first.starts_with('$')
        || first.starts_with("./")
        || first.starts_with('/')
        || first.starts_with('~')
        || is_assignment;
    if !looks_terminal {
        return None;
    }
    let shown = if first.chars().count() > 30 {
        format!("{}...", first.chars().take(30).collect::<String>())
    } else {
        first.to_string()
    };
    Some(format!(
        "error: `{shown}` is a Terminal command, not a yukti command.\n\
         note: type `exit` to go back to Terminal and run it there, then run `yukti` again.\n\
         note: inside yukti, type commands without the word yukti, for example `case list`. Type `help` for the list.\n"
    ))
}

/// Decides what one line does. Pure: no I/O, so it is unit-tested.
pub fn plan(session: &mut Session, line: &str, globals: &[String], ui: &Ui) -> Action {
    let mut words = match split_words(line) {
        Ok(w) => w,
        Err(e) => return Action::Error(format!("error: {e}\n")),
    };
    if words.first().map(String::as_str) == Some("yukti") {
        words.remove(0);
    }
    let Some(first) = words.first().cloned() else {
        return Action::Nothing;
    };
    match first.as_str() {
        "exit" | "quit" | "q" => return Action::Exit,
        "clear" | "cls" => return Action::Clear,
        "help" | "?" => return Action::Print(help_text(ui)),
        "banner" => return Action::Print(ui.banner()),
        "unuse" => {
            session.case = None;
            return Action::Print(format!(
                "{MARGIN}{}\n",
                ui.paint(Tone::Muted, "working case cleared")
            ));
        }
        "use" => {
            return match words.get(1) {
                None => Action::Print(format!(
                    "{MARGIN}{}\n",
                    ui.paint(
                        Tone::Muted,
                        &session
                            .case
                            .as_ref()
                            .map_or("no working case; `use <uuid>`".to_string(), |c| format!(
                                "working case {c}"
                            ))
                    )
                )),
                Some(raw) => match CaseId::parse(raw, "use") {
                    Ok(id) => {
                        session.case = Some(id.as_str().to_string());
                        Action::Print(format!(
                            "{MARGIN}{} {}\n",
                            ui.mark(Glyph::Check, Tone::Ok),
                            ui.paint(Tone::Text, &format!("working case {}", id.as_str()))
                        ))
                    }
                    Err(e) => Action::Error(format!("error: {e}\n")),
                },
            };
        }
        "shell" => return Action::Error("error: already in a yukti shell\n".to_string()),
        _ => {}
    }
    if let Some(hint) = terminal_command_hint(&first) {
        return Action::Error(hint);
    }

    let mut argv = vec!["yukti".to_string()];
    argv.extend(globals.iter().cloned());
    argv.extend(words.iter().cloned());
    if let Some(case) = &session.case {
        let has_case = words
            .iter()
            .any(|w| w == "--case" || w.starts_with("--case="));
        let asks_help = words.iter().any(|w| w == "--help" || w == "-h");
        if !has_case && !asks_help {
            let mut with_case = argv.clone();
            with_case.extend(["--case".to_string(), case.clone()]);
            if Cli::try_parse_from(&with_case).is_ok() {
                return Action::Run(with_case);
            }
        }
    }
    Action::Run(argv)
}

fn prompt(session: &Session, ui: &Ui) -> String {
    let mut p = ui.paint(Tone::Brand, "yukti");
    if let Some(case) = &session.case {
        p.push(' ');
        p.push_str(&ui.paint(Tone::Subtle, &format!("[{}]", short_id(case, ui))));
    }
    if session.last_exit != 0 {
        p.push(' ');
        p.push_str(&ui.paint(
            Tone::Flag,
            &format!("{}{}", ui.g(Glyph::Cross), session.last_exit),
        ));
    }
    format!("{p} {} ", ui.paint(Tone::Brand, ui.g(Glyph::Prompt)))
}

/// Runs the read-eval-print loop on stdin until `exit` or end of input.
pub fn run_interactive(cli: &Cli, env: &EnvVars, runtime: &tokio::runtime::Runtime) -> ExitCode {
    let ui = Ui::resolve(cli.color, cli.ascii, &env.terminal);
    let globals = global_args(cli);
    let mut session = Session {
        case: env
            .case_id
            .as_deref()
            .and_then(|c| CaseId::parse(c, "YUKTI_CASE").ok())
            .map(|c| c.as_str().to_string()),
        last_exit: 0,
    };
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let _ = write!(stdout, "{}", ui.banner());
    let _ = writeln!(
        stdout,
        "{MARGIN}{}\n",
        ui.paint(Tone::Subtle, "Interactive session. `help` lists commands, `use <uuid>` sets the working case, `exit` leaves.")
    );
    let stdin = std::io::stdin();
    let mut lines = stdin.lock();
    loop {
        let _ = write!(stdout, "{}", prompt(&session, &ui));
        let _ = stdout.flush();
        let mut line = String::new();
        match lines.read_line(&mut line) {
            Ok(0) => {
                let _ = writeln!(stdout);
                break;
            }
            Ok(_) => {}
            Err(e) => {
                let _ = writeln!(stderr, "error: cannot read input: {e}");
                return ExitCode::from(crate::error::exit::INTERNAL);
            }
        }
        match plan(&mut session, line.trim(), &globals, &ui) {
            Action::Nothing => {}
            Action::Exit => break,
            Action::Clear => {
                let _ = write!(stdout, "\x1b[2J\x1b[H");
            }
            Action::Print(text) => {
                let _ = write!(stdout, "{text}");
            }
            Action::Error(text) => {
                let _ = write!(stderr, "{}", ui.for_stderr().style_diagnostics(&text));
                session.last_exit = crate::error::exit::USAGE;
            }
            Action::Run(argv) => match Cli::try_parse_from(&argv) {
                Err(err) => {
                    let _ = err.print();
                    session.last_exit = u8::try_from(err.exit_code()).unwrap_or(2);
                }
                Ok(parsed) if parsed.command == Command::Shell => {}
                Ok(parsed) => {
                    let rendered = runtime.block_on(crate::execute(&parsed, env));
                    let _ = stdout.write_all(rendered.stdout.as_bytes());
                    let _ = stderr.write_all(rendered.stderr.as_bytes());
                    session.last_exit = rendered.exit_code;
                }
            },
        }
        let _ = writeln!(stdout);
    }
    ExitCode::from(session.last_exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASE: &str = "3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c";

    #[test]
    fn splits_quoted_words() {
        assert_eq!(
            split_words(r#"case create --title "ATM skimming" --description 'a b'"#).unwrap(),
            vec![
                "case",
                "create",
                "--title",
                "ATM skimming",
                "--description",
                "a b"
            ]
        );
        assert_eq!(split_words(r"a\ b c").unwrap(), vec!["a b", "c"]);
        assert_eq!(split_words(r#"x """#).unwrap(), vec!["x", ""]);
        assert!(split_words("\"open").is_err());
    }

    #[test]
    fn working_case_is_added_only_where_accepted() {
        let ui = Ui::plain();
        let mut s = Session::default();
        assert!(matches!(
            plan(&mut s, &format!("use {CASE}"), &[], &ui),
            Action::Print(_)
        ));
        assert_eq!(s.case.as_deref(), Some(CASE));

        let Action::Run(argv) = plan(&mut s, "timeline --limit 3", &[], &ui) else {
            panic!()
        };
        assert_eq!(
            argv,
            vec!["yukti", "timeline", "--limit", "3", "--case", CASE]
        );

        // `case list` has no --case: the line runs unchanged.
        let Action::Run(argv) = plan(&mut s, "yukti case list", &[], &ui) else {
            panic!()
        };
        assert_eq!(argv, vec!["yukti", "case", "list"]);

        // An explicit --case wins.
        let Action::Run(argv) = plan(&mut s, "gaps --case other", &[], &ui) else {
            panic!()
        };
        assert_eq!(argv, vec!["yukti", "gaps", "--case", "other"]);

        assert!(matches!(plan(&mut s, "unuse", &[], &ui), Action::Print(_)));
        assert_eq!(s.case, None);
    }

    #[test]
    fn terminal_commands_get_a_hint_instead_of_a_usage_error() {
        let ui = Ui::plain();
        let mut s = Session::default();
        for line in [
            "mkdir -p ~/yukti-demo && cd ~/yukti-demo",
            "curl -fsSL -o sample_cdr.csv \"$url\"",
            "url=\"https://example.com/sample_cdr.csv\"",
            "export YUKTI_CASE=abc",
            "$env:YUKTI_CASE = \"abc\"",
            "New-Item -ItemType Directory demo",
            "irm https://example.com -OutFile x",
            "cd ~/yukti-demo",
        ] {
            match plan(&mut s, line, &[], &ui) {
                Action::Error(text) => {
                    assert!(text.contains("is a Terminal command"), "{line}: {text}");
                    assert!(text.contains("type `exit`"), "{line}");
                }
                other => panic!("{line}: {other:?}"),
            }
        }
        // Real yukti commands are untouched.
        assert!(matches!(
            plan(&mut s, "case list", &[], &ui),
            Action::Run(_)
        ));
        assert!(matches!(
            plan(&mut s, "yukti timeline --limit 2", &[], &ui),
            Action::Run(_)
        ));
        assert!(matches!(
            plan(&mut s, "whatif --case c --clock-offset 30", &[], &ui),
            Action::Run(_)
        ));
        assert_eq!(terminal_command_hint("evidence"), None);
        assert_eq!(terminal_command_hint("reconstruction"), None);
    }

    #[test]
    fn built_ins_and_globals() {
        let ui = Ui::plain();
        let mut s = Session::default();
        assert_eq!(plan(&mut s, "", &[], &ui), Action::Nothing);
        assert_eq!(plan(&mut s, "quit", &[], &ui), Action::Exit);
        assert!(matches!(
            plan(&mut s, "use CASE-026", &[], &ui),
            Action::Error(_)
        ));
        assert!(matches!(plan(&mut s, "shell", &[], &ui), Action::Error(_)));
        let globals = vec!["--api-url".to_string(), "http://h:1".to_string()];
        let Action::Run(argv) = plan(&mut s, "status", &globals, &ui) else {
            panic!()
        };
        assert_eq!(argv, vec!["yukti", "--api-url", "http://h:1", "status"]);
    }
}
