# 0004 — Installable `yukti` with a bundled local server

- **Status:** Proposed (2026-11-24). Pending team review.
- **Scope:** `cli/` (new `server` module, `main.rs` entry behaviour, `install/`),
  `.github/workflows/release.yml`, and `.github/workflows/cli.yml` installer checks.
  `backend-rust/` source is unchanged; only its existing `sutram-backend-rust` binary is packaged.
- **Builds on:** [0002](0002-standalone-cli-crate.md) and [0003](0003-cli-design-system-and-workbench.md).

## Context

Using YUKTI required a Rust toolchain, a clone of the repository, and two terminals: one for
`cargo run --bin sutram-backend-rust` and one for the CLI. The goal is a Claude Code-like experience:
install with one command on macOS or Windows, then type `yukti`.

## Decision

1. **Release archives bundle two programs.** `yukti` (the CLI) and `yukti-server` (the unchanged API binary,
   renamed) ship for macOS arm64 and x64, Windows x64 and Linux x64. Each archive is published to a GitHub
   Release with a SHA-256 file when a `yukti-v*` tag is pushed.
2. **The installers are one-liners with no other dependencies:** `install.sh` (`curl … | sh`) and `install.ps1`
   (`irm … | iex`). They verify the SHA-256 before installing anything, install per user (no admin
   rights), and add the folder to the user's PATH idempotently. A mismatch installs nothing.
3. **Plain `yukti` on a terminal opens the interactive session.** Without a terminal it still prints usage
   and exits 2, so scripts are unaffected.
4. **`yukti` starts the bundled server on demand.** It does so only when all of these hold:
   - the API URL is a loopback address;
   - nothing answers `/health` there;
   - `yukti-server` sits next to `yukti` (or `YUKTI_SERVER_BIN` is set);
   - `YUKTI_NO_AUTOSTART` is not set.

   The server runs detached, in filesystem mode, with its data under `YUKTI_HOME` (`~/.yukti` or
   `%LOCALAPPDATA%\yukti`). `status` and `doctor` never start it, so they report its real state.
   `yukti server start|stop|status|logs` manages it explicitly.
5. **The CLI–API boundary is unchanged.** The CLI manages the server's process but still talks to it
   over HTTP only, and still contains no forensic logic.

## Consequences

- Teammates and evaluators can run YUKTI without Rust, Docker or a database. Installed use is filesystem
  mode. PostgreSQL and Neo4j stay opt-in, through the API's own environment variables, which the bundled
  server inherits.
- The binaries are not code-signed. The installers clear macOS quarantine and unblock the Windows `.exe`
  files, but downloading archives through a browser can still trigger Gatekeeper or SmartScreen.
  Signing and notarisation are possible follow-ups, as are an `.msi` and a Homebrew tap.
- The release workflow can only be exercised on GitHub. Locally, `install.sh` was tested end to end
  against a packaged macOS arm64 archive. `install.ps1` is checked by PowerShell's parser in CI but has
  not been run on Windows yet.
- Publishing a release is an outward-facing step and is done by a maintainer pushing a tag.
