//! `yukti demo`: a guided demonstration on the synthetic demo case.
//!
//! The evidence files ship inside the binary (`data/synthetic/demo/`), are written to the YUKTI
//! home (or the temp folder) and ingested like any other exhibit. Every step runs a real command
//! through [`crate::execute`], so the demo shows exactly what an investigator would see. It pauses
//! between steps on a terminal (Enter to continue) unless `--fast` is given.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use serde_json::Value;

use crate::cli::{Cli, DemoArgs};
use crate::config::EnvVars;
use crate::ui::{Glyph, Tone, Ui, MARGIN};

/// The demo exhibits (synthetic), embedded at build time.
pub const DEMO_FILES: [(&str, &str); 3] = [
    (
        "carrier_a_cdr.csv",
        include_str!("../../../data/synthetic/demo/carrier_a_cdr.csv"),
    ),
    (
        "carrier_b_tower_dump.csv",
        include_str!("../../../data/synthetic/demo/carrier_b_tower_dump.csv"),
    ),
    (
        "whatsapp_chat.txt",
        include_str!("../../../data/synthetic/demo/whatsapp_chat.txt"),
    ),
];

/// Where the demo writes its exhibits.
pub fn demo_dir() -> PathBuf {
    let base = crate::server::Paths::resolve()
        .map(|p| p.home)
        .unwrap_or_else(std::env::temp_dir);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    base.join("demo").join(stamp.to_string())
}

struct Runner<'a> {
    globals: Vec<String>,
    env: &'a EnvVars,
    runtime: &'a tokio::runtime::Runtime,
    ui: Ui,
    pause: bool,
    step: usize,
    total: usize,
}

impl Runner<'_> {
    fn out(&self, text: &str) {
        let mut o = std::io::stdout();
        let _ = o.write_all(text.as_bytes());
        let _ = o.flush();
    }

    fn run(&self, args: &[&str]) -> (crate::output::Rendered, bool) {
        let mut argv = vec!["yukti".to_string()];
        argv.extend(self.globals.iter().cloned());
        argv.extend(args.iter().map(|s| s.to_string()));
        match Cli::try_parse_from(&argv) {
            Ok(cli) => {
                let r = self.runtime.block_on(crate::execute(&cli, self.env));
                let ok = r.exit_code == 0;
                (r, ok)
            }
            Err(e) => (
                crate::output::Rendered {
                    stdout: String::new(),
                    stderr: format!("error: {e}\n"),
                    exit_code: 2,
                },
                false,
            ),
        }
    }

    /// Runs a command and shows its full output.
    fn show(&self, args: &[&str]) -> bool {
        let (r, ok) = self.run(args);
        self.out(&r.stdout);
        if !r.stderr.is_empty() {
            let _ = std::io::stderr().write_all(r.stderr.as_bytes());
        }
        ok
    }

    /// Runs a command with --json and returns the document.
    fn json(&self, args: &[&str]) -> Option<Value> {
        let mut a: Vec<&str> = args.to_vec();
        a.push("--json");
        let (r, _) = self.run(&a);
        serde_json::from_str(&r.stdout).ok()
    }

    fn step(&mut self, title: &str, why: &str) {
        self.step += 1;
        if self.pause && self.step > 1 {
            self.out(&format!(
                "\n{MARGIN}{} {}",
                self.ui.paint(Tone::Brand, self.ui.g(Glyph::Prompt)),
                self.ui
                    .paint(Tone::Subtle, "press Enter for the next step ")
            ));
            let mut line = String::new();
            let _ = std::io::stdin().lock().read_line(&mut line);
        }
        let ui = &self.ui;
        self.out(&format!(
            "\n\n{MARGIN}{} {}  {}\n",
            ui.paint(Tone::Brand, ui.g(Glyph::Bar)),
            ui.bold(Tone::Brand, &format!("STEP {}/{}", self.step, self.total)),
            ui.bold(Tone::Strong, &title.to_uppercase())
        ));
        self.out(&ui.para(MARGIN.len(), Tone::Muted, why));
        self.out("\n");
    }
}

/// Runs the demonstration. Returns the process exit code.
pub fn run(
    cli: &Cli,
    args: &DemoArgs,
    env: &EnvVars,
    runtime: &tokio::runtime::Runtime,
) -> ExitCode {
    let ui = Ui::resolve(cli.color, cli.ascii, &env.terminal);
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut r = Runner {
        globals: crate::shell::global_args(cli),
        env,
        runtime,
        ui: ui.clone(),
        pause: interactive && !args.fast,
        step: 0,
        total: 10,
    };
    r.out(&ui.banner());
    r.out(&ui.para(
        MARGIN.len(),
        Tone::Text,
        "A guided tour on a SYNTHETIC case: a phone number seen in Noida, Saket and Mumbai within half an hour, two operators' call records and a WhatsApp chat. No real person or event.",
    ));

    // 1. Engine.
    r.step(
        "Engine",
        "YUKTI's engine runs locally. The CLI talks to it over HTTP, like the web workspace.",
    );
    if !r.show(&["status"]) {
        r.out(&ui.hint("the engine is not ready: run `yukti server start` or `yukti doctor`"));
        return ExitCode::from(crate::error::exit::UNREACHABLE);
    }

    // 2. Case.
    r.step(
        "Open a case",
        "Every investigation starts as a case with an FIR reference.",
    );
    let Some(doc) = r.json(&[
        "case",
        "create",
        "--title",
        "DEMO - Courier route (synthetic)",
        "--fir-number",
        "DEMO-FIR-7/2026",
    ]) else {
        r.out(&ui.hint("could not create the demo case"));
        return ExitCode::from(crate::error::exit::SERVER_ERROR);
    };
    let case = doc["case"]["case_id"].as_str().unwrap_or("").to_string();
    r.show(&["case", "show", "--case", &case]);

    // 3. Evidence.
    r.step(
        "Ingest evidence",
        "Each exhibit is read, never modified, and hashed twice: by the engine and by the CLI. A mismatch would stop the demo with exit code 7.",
    );
    let dir = demo_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        r.out(&ui.hint(&format!("cannot write the demo files to {}", dir.display())));
        return ExitCode::from(crate::error::exit::INTERNAL);
    }
    let mut ids: Vec<(String, String)> = Vec::new();
    for (name, body) in DEMO_FILES {
        let path = dir.join(name);
        if std::fs::write(&path, body).is_err() {
            return ExitCode::from(crate::error::exit::INTERNAL);
        }
        let p = path.display().to_string();
        let doc = r.json(&[
            "evidence",
            "ingest",
            &p,
            "--case",
            &case,
            "--source-timezone",
            "Asia/Kolkata",
            "--auto-extract",
        ]);
        let Some(doc) = doc else {
            r.out(&ui.hint(&format!("ingest failed for {name}")));
            return ExitCode::from(crate::error::exit::SERVER_ERROR);
        };
        let ok = doc["hash_match"].as_bool() == Some(true);
        let source = doc["response"]["receipt"]["source_type"]
            .as_str()
            .or_else(|| doc["response"]["source_type"].as_str())
            .unwrap_or("?");
        let records = doc["response"]["extraction"]["valid_records_count"]
            .as_u64()
            .unwrap_or(0);
        r.out(&format!(
            "{MARGIN}{} {}  {}  {}\n{MARGIN}  {}\n",
            if ok {
                ui.mark(Glyph::Check, Tone::Ok)
            } else {
                ui.mark(Glyph::Cross, Tone::Flag)
            },
            ui.bold(Tone::Strong, &crate::ui::pad(name, 26)),
            ui.paint(Tone::Muted, &crate::ui::pad(source, 12)),
            ui.paint(Tone::Text, &format!("{records} record(s) extracted")),
            ui.paint(
                Tone::Subtle,
                &format!(
                    "SHA-256 {} {}",
                    doc["sha256_hash"].as_str().unwrap_or(""),
                    if ok { "(verified twice)" } else { "MISMATCH" }
                )
            )
        ));
        ids.push((
            name.to_string(),
            doc["evidence_id"].as_str().unwrap_or("").to_string(),
        ));
    }

    // 4. Reconstruction.
    r.step(
        "Reconstruct",
        "Events from all three exhibits are placed on one timeline with uncertainty windows. Assumptions are labelled, and movements that would need impossible speeds become contradiction leads.",
    );
    let _ = r.json(&["reconstruction", "run", "--case", &case]);
    r.show(&["case", "overview", "--case", &case]);

    // 5. Breaking point.
    r.step(
        "How strong is each lead?",
        "YUKTI computes how wrong an assumption would have to be before a lead disappears, then proves the boundary by recomputing just inside and just outside it.",
    );
    r.show(&["robustness", "--case", &case]);

    // 6. What-if.
    let operator_b = ids
        .iter()
        .find(|(n, _)| n == "carrier_b_tower_dump.csv")
        .map(|(_, i)| i.clone())
        .unwrap_or_default();
    r.step(
        "Test the weak lead",
        "Suppose operator B's clock was 3 minutes fast. The simulation is hypothetical and is not stored.",
    );
    r.show(&[
        "whatif",
        "--case",
        &case,
        "--name",
        "Operator B clock 3 min fast",
        "--exhibit-offset",
        &format!("{operator_b}=-180"),
    ]);

    // 7. Defence view.
    r.step("Cross-examine the case", "The weakest points first, as defence counsel would raise them, each with what would close it.");
    r.show(&["challenge", "--case", &case]);

    // 8. Ask.
    r.step("Ask in plain English", "Answers come only from the evidence, with a source for every fact. No language model is used.");
    r.show(&["ask", "--case", &case, "where was 9811002233 at 9:15 pm"]);

    // 9. Court package.
    r.step(
        "Court-ready package",
        "Every stored exhibit is re-hashed, the case is sealed, and a draft Section 63 BSA certificate is prepared for an officer and an expert to complete and sign.",
    );
    r.show(&["custody", "verify", "--case", &case]);
    let cert = dir.join("certificate.md");
    let cert_s = cert.display().to_string();
    let _ = r.json(&["certificate", "--case", &case, "--out", &cert_s]);
    r.out(&ui.kv("Certificate", Tone::Strong, &cert_s));

    // 10. The same case in the web workspace.
    r.step(
        "Watch it in the web workspace",
        "The browser workspace reads the same engine, so this case is already there. The reconstruction theatre replays every step above as an animated, narrated story.",
    );
    r.show(&["open", "--case", &case, "--view", "theatre", "--print"]);

    r.out(&format!(
        "\n\n{MARGIN}{} {}\n",
        ui.mark(Glyph::Check, Tone::Ok),
        ui.bold(Tone::Strong, "Demo complete.")
    ));
    r.out(&ui.next("explore", &format!("export YUKTI_CASE={case}")));
    r.out(&ui.next("animate it", "yukti replay"));
    r.out(&ui.next(
        "in the browser",
        &format!("yukti open --case {case} --view theatre"),
    ));
    r.out(&ui.next("interactive", "yukti"));
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_files_are_embedded_and_synthetic() {
        assert_eq!(DEMO_FILES.len(), 3);
        assert!(DEMO_FILES[0].1.starts_with("record_id,caller_phone"));
        assert!(crate::commands::evidence::sha256_hex(DEMO_FILES[2].1.as_bytes()).len() == 64);
        assert!(DEMO_FILES[2].1.contains(" - Priya (Courier): "));
    }
}
