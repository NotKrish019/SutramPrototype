# yukti CLI: getting started

A step-by-step guide to installing `yukti`, checking it's set up with `yukti doctor`, and running a full case from start to finish. For every option and output format, see [README.md](README.md).

Commands are written for **Windows PowerShell**. They work the same in bash, except the `$c = ...` variable lines.

## 1. Install (once)

You need Rust 1.82 or newer (`rustc --version`).

```powershell
cd <repo>\cli
cargo install --path .
```

This puts `yukti.exe` in `%USERPROFILE%\.cargo\bin`, which rustup already added to your PATH. Open a **new** terminal and check:

```powershell
yukti --version      # yukti 0.1.0
yukti --help         # lists every command
```

If you get *"yukti is not recognized"*, add `%USERPROFILE%\.cargo\bin` to your PATH and open a new terminal.

**After pulling CLI changes**, reinstall with `cargo install --path . --force`.

## 2. Start the API (every session)

`yukti` is a client: it needs the YUKTI Rust API running. In a **separate terminal**, leave this running:

```powershell
cd <repo>\backend-rust
cargo run --bin sutram-backend-rust
```

Plain `cargo run` fails here because that crate has two binaries. The API listens on `http://127.0.0.1:8080`. PostgreSQL and Neo4j are optional: without them the API stores cases and evidence on disk.

## 3. Check the setup: `yukti doctor`

```powershell
yukti doctor
```

| You see | Meaning | What to do |
|---|---|---|
| `Result  OK` | Everything is ready, including PostgreSQL and Neo4j | Nothing |
| `Result  OK with warnings` | `yukti` works. The warnings are optional pieces (no PostgreSQL, no Neo4j) | Nothing, for local use |
| `Result  FAILED` | Something must be fixed | Follow the `hint:` under each `[FAIL]` line |

The most common failure is `[FAIL] API ... no response`: the API from step 2 isn't running.

`yukti doctor` only reads. It never creates or changes anything, so run it as often as you like. `yukti status` is the shorter version: just the API and readiness.

## 4. Walk through a case

```powershell
# Create a case. Copy the UUID it prints.
yukti case create --title "My demo case"
$c = "PASTE-THE-UUID-HERE"

# Look at it
yukti case show --case $c
yukti case list

# Upload the synthetic sample evidence (run from the repo root)
cd <repo>
yukti evidence ingest --case $c data\synthetic\sample_cdr.csv --source-timezone Asia/Kolkata --auto-extract
yukti evidence ingest --case $c data\synthetic\sample_chat_export.json --source-timezone Asia/Kolkata --auto-extract
yukti evidence list --case $c

# Reconstruct the timeline
yukti reconstruction run --case $c
yukti reconstruction explain --case $c
```

What you should see with the samples:
- **Each upload:** a SHA-256 computed by the API and `extraction succeeded` (5 records from the CDR, 3 from the chat export).
- **`reconstruction run`:** 8 timeline events, 5 trajectories and 0 contradiction leads.
- **`Export safeguard BLOCKED`:** expected. The samples don't declare a clock offset, and locations are estimated from cell towers. The output lists each of these assumptions.

Add `--json` to any command for machine-readable output.

## 5. Command cheat sheet

| Command | Does |
|---|---|
| `yukti doctor` | Setup checklist with fix hints |
| `yukti status` | API up? Database and graph ready? |
| `yukti case list` | All cases |
| `yukti case create --title "..."` | New case (prints its UUID) |
| `yukti case show --case <UUID>` | One case |
| `yukti evidence ingest --case <UUID> <FILE>` | Upload one evidence file (max about 2 MiB) |
| `yukti evidence list --case <UUID>` | A case's evidence |
| `yukti reconstruction run --case <UUID>` | Rebuild the timeline, spatial picture and contradiction leads |
| `yukti reconstruction explain --case <UUID>` | Full detail plus the judicial report |
| `yukti <command> --help` | Help for any command |

**Global options:** `--json`, `--api-url <URL>` (or `$env:YUKTI_API_URL`), `--timeout <SECONDS>` (or `$env:YUKTI_TIMEOUT_SECS`).

## 6. Exit codes

Check with `$LASTEXITCODE` in PowerShell, or `$?` in bash.

| Code | Meaning |
|---|---|
| 0 | Success |
| 2 | Bad input or configuration (nothing was sent) |
| 3 | API not reachable: start it (step 2) |
| 4 | API said 4xx, for example the case wasn't found |
| 5 | API said 5xx, or a dependency isn't ready |
| 6 | Unexpected response: is `--api-url` pointing at the YUKTI API? |
| 69 | Not supported by the API yet (`reconstruction compare`) |

## 7. Run the CLI's tests

These don't need the API running:

```powershell
cd <repo>\cli
cargo test
cargo clippy --all-targets -- -D warnings
```
