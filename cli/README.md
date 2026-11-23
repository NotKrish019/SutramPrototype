# yukti: YUKTI command-line client

`yukti` is a standalone command-line client for the YUKTI Rust API (`backend-rust/`).

- It talks to the API **over HTTP only**, through the same endpoints the React workspace uses.
- It never connects to PostgreSQL or Neo4j, shares no code with the backend crate, and contains no forensic logic.

## Build and run

The crate is standalone. It has its own `Cargo.toml` and `Cargo.lock`, and there is no root Cargo workspace.

```bash
cd cli
cargo build                      # binary: cli/target/debug/yukti(.exe)
cargo run -- status              # run without installing
cargo install --path .           # optional: put `yukti` on your PATH (~/.cargo/bin)
```

The minimum Rust version is 1.82.

Start the API first (from `backend-rust/`: `cargo run --bin sutram-backend-rust`; plain `cargo run` fails because that crate has two binaries). It listens on `http://127.0.0.1:8080` by default.

## Commands

| Command | API calls | Description |
|---|---|---|
| `yukti status` | `GET /health`, `GET /health/ready` | API reachability, plus database and graph readiness |
| `yukti doctor` | `GET /health`, `GET /health/ready`, `GET /api/v1/cases` | Setup checklist: config, API, readiness, database, graph and case API, with a fix hint for each problem |
| `yukti case list` | `GET /api/v1/cases`, then `GET /health/ready` (storage mode) | Cases in the API's order |
| `yukti case create --title …` | `POST /api/v1/cases` | Register a case and print the UUID the API assigned |
| `yukti case show --case <UUID>` | `GET /api/v1/cases/:case_id` | One case's fields |
| `yukti evidence ingest --case <UUID> <FILE>` | `GET /api/v1/cases/:case_id`, then `POST /api/v1/evidence/upload` (multipart) | Upload one evidence file |
| `yukti evidence list --case <UUID>` | `GET /api/v1/cases/:case_id`, then `GET /api/v1/cases/:case_id/evidence` | A case's evidence receipts |
| `yukti reconstruction run --case <UUID>` | `GET /api/v1/cases/:case_id`, then `POST /api/v1/cases/:case_id/reconstruction` | Reconstruct the case with the API's defaults and summarise the result |
| `yukti reconstruction explain --case <UUID>` | `GET /api/v1/cases/:case_id`, `GET …/reconstruction`, then `GET …/report` | Full reconstruction detail plus the judicial report and dossier |
| `yukti reconstruction compare --case <UUID>` | none | Not available yet: exits `69` |

### Planned commands

These are not implemented yet:
- `evidence trace`
- `reconstruction compare` (interface only; it exits `69` until the backend offers scenario comparison)
- `gaps list`
- `graph open`

Each will be added as a new `Command` variant in `src/cli.rs`, with a handler module under `src/commands/`. Commands that need API functionality that doesn't exist yet will exit with code `69`.

### `yukti doctor`

```bash
yukti doctor
yukti doctor --json
```

A checklist that tells you whether `yukti` is set up and usable. Each line is `[PASS]`, `[WARN]`, `[FAIL]` or `[SKIP]`, and every warning or failure comes with a `hint:` saying what to do.

| Check | Passes when | Warns when | Fails when |
|---|---|---|---|
| CLI | always (shows the version) | | |
| Config | the API URL and timeout are valid (shows where each came from: flag, environment or default) | | either is invalid (exit 2) |
| API | `GET /health` answers like the YUKTI API | | unreachable (exit 3) or a different server answered |
| Readiness | `ready` | `degraded` | `not_ready` (exit 5) or unreadable |
| Database | `connected` | filesystem mode (no PostgreSQL) | configured but not connected |
| Graph | `connected` | disabled (no Neo4j) | configured but not connected |
| Cases API | `GET /api/v1/cases` returns a list | | any error |

- **Read-only.** Doctor only sends `GET` requests. It never creates a case, uploads evidence or runs a reconstruction.
- **Skipped checks.** A check that can't run because an earlier one failed shows `[SKIP]`. For example, everything after `API` is skipped when the API is unreachable.
- **Exit code.** `0` when nothing failed, even with warnings. Otherwise the exit code of the first failed check, following the exit-code table below.
- **Invalid configuration** is reported as a failed `Config` check, not as a bare error, so `doctor` still prints the checklist.

Example (API running in filesystem mode, which is fine for local use):

```text
yukti doctor

  [PASS] CLI         yukti 0.1.0
  [PASS] Config      API URL http://127.0.0.1:8080 (from default), timeout 10 s (from default)
  [PASS] API         reachable at http://127.0.0.1:8080 (GET /health -> HTTP 200, service sutram-backend-rust)
  [WARN] Readiness   degraded (GET /health/ready -> HTTP 200)
                     hint: The API works, but an optional dependency is not configured (see below).
  [WARN] Database    not configured (filesystem mode, no PostgreSQL)
                     hint: Fine for local use: cases and evidence are kept on disk. ...
  [WARN] Graph       disabled (Neo4j not configured)
                     hint: Optional: knowledge-graph projection is off. ...
  [PASS] Cases API   GET /api/v1/cases -> 2 case(s)

4 passed, 3 warning(s), 0 failed, 0 skipped
Result  OK with warnings: yukti is set up and you can use every command. Warnings are optional setup. (exit 0)
```

### `yukti status`

```bash
yukti status
yukti status --json
yukti --api-url http://127.0.0.1:8080 status
```

The command runs two checks:

1. **`GET /health`** checks that the API is reachable. Readiness isn't checked when this fails.
2. **`GET /health/ready`** checks dependency readiness. The API reports one of three states:
   - `ready` (HTTP 200): every configured dependency answered.
   - `degraded` (HTTP 200): nothing failed, but PostgreSQL isn't configured (filesystem mode) and/or Neo4j is disabled.
   - `not_ready` (HTTP 503): a configured dependency failed. The API's `error` text is shown.

A database or graph is shown as **connected only when the API itself reports `"connected"`**. Every other value is shown as what it is: not configured, disabled, disconnected, failed to initialize, or unrecognized. Any 5xx readiness response is never treated as ready, whatever its body says.

Example (API in filesystem mode):

```text
YUKTI API     http://127.0.0.1:8080
API           reachable (GET /health -> HTTP 200, service sutram-backend-rust)
Readiness     DEGRADED (GET /health/ready -> HTTP 200)
  Database    not configured (filesystem mode, no PostgreSQL)
  Graph       disabled (Neo4j not configured)
Result        API is up in degraded mode: at least one dependency is not configured (see above). (exit 0)
```

### `yukti case`

```bash
yukti case list
yukti case list --json
yukti case create --title "Warehouse burglary"
yukti case create --title "Warehouse burglary" --description "Night-time break-in" \
  --fir-number "FIR-112/2026" --investigator-id INV-7 --metadata '{"station":"Saket"}'
yukti case show --case 3f2b8c1e-7a4d-4e1b-9c3a-5d6e7f8a9b0c
```

**`case list`**
- Prints UUID, status, creation time and title for each record, **in the order the API returned them**.
- An empty list prints `No cases returned by the API.` and exits 0.
- Records are never altered, merged or invented.
- **Storage warnings (on stderr).** Without PostgreSQL, the API lists evidence-storage folders instead of database rows. A folder without `case.json` comes back as a placeholder whose title and timestamps are generated by the API at request time; the API marks these with `metadata.storage = "filesystem"`. To make this visible, `case list` also reads `GET /health/ready`:
  - **Database reported `"connected"`:** no storage warning. This is the case even when readiness is `degraded` only because Neo4j is disabled.
  - **Database `unconfigured_filesystem_mode` or any other non-connected value:** a warning that the list comes from filesystem storage.
  - **Readiness can't be read:** a warning that the storage mode is unknown. The list itself still succeeds.
  - **Per record:** placeholder-marked records are tagged `[filesystem placeholder]`, and records whose `case_id` isn't a UUID are listed in a warning.
- **Not detectable:** with PostgreSQL configured, the API silently falls back to the filesystem listing if the database query fails. The CLI can't detect that.

**`case create`**
- `--title` is required and must not be blank.
- Optional: `--description`, `--fir-number`, `--investigator-id`, `--metadata <JSON object>` and `--case-id <UUID>`. These are the fields of the API's `CreateCaseRequest`.
- Only the options you pass are sent. Blank optional values and non-object metadata are rejected locally (exit 2) before any request is made.
- The printed UUID is exactly what the API returned. Normally omit `--case-id` and let the API assign one. If you pass it and the API returns a different ID, a warning is shown.
- With PostgreSQL configured, a `--case-id` that already exists is rejected by the database insert, and the API currently reports that as HTTP 500, not 409.

**`case show`**
- `--case` must be a hyphenated UUID. It's sent in canonical lowercase form.
- Names such as `CASE-026` and other non-UUID values are rejected locally (exit 2) and are never mapped to an ID.
- HTTP 404 exits 4.

### `yukti evidence`

```bash
yukti evidence ingest --case <UUID> cdr.csv --source-timezone Asia/Kolkata --clock-offset-seconds -30
yukti evidence ingest --case <UUID> chat.json
yukti evidence ingest --case <UUID> cdr.csv --auto-extract --metadata '{"seized_by":"INV-7"}'
yukti evidence list --case <UUID>
yukti evidence list --case <UUID> --json
```

**`evidence ingest --case <UUID> <FILE>`** runs these steps in order, and stops at the first failure:

1. **Local validation, with no network access.** Any failure exits 2 and nothing is sent.
   - `--case` must be a UUID.
   - `<FILE>` must exist, be a regular file, be readable and be non-empty.
   - `--metadata` must be a JSON object.
   - `--clock-offset-seconds` must be a finite number. The API reads this field as a floating-point number and silently drops values it can't parse, so the CLI rejects them instead.
   - Blank option values are rejected.
2. **Case check.** `GET /api/v1/cases/:case_id`. If this fails (for example a 404, exit 4), **nothing is uploaded**. The upload endpoint itself doesn't check that the case exists.
3. **Upload.** `POST /api/v1/evidence/upload` as `multipart/form-data`, with the file's bytes unmodified. The file is only opened for reading.

**Forensic rules:**
- **Only the options you pass are sent.** The timezone and clock offset are never defaulted to UTC or 0, so the API records them as undeclared, and the output shows `not declared`.
- **`--metadata` can't contain `source_timezone` or `clock_offset_seconds`.** The API merges metadata after those fields, so such keys would silently override them. Use the dedicated options instead.
- **`--source-type` replaces the type the API detects from the content** and is stored exactly as given. The API's extractor only processes call-detail-record types (`TABULAR_CSV`, `CALL_DETAIL_RECORD`, `CDR`) and chat-export types (`CHAT_EXPORT`, `CHAT`). The CLI warns when you use `--source-type`.
- **The SHA-256 shown is computed by the API** from the bytes it stored. The CLI doesn't compute or verify a hash itself; compare with a local `sha256sum` if you need to.

**Auto-extraction:**
- **`--auto-extract`** sends `auto_extract=true`. The API then returns an extraction run alongside the receipt, and its status and record counts are shown.
- **Exit code:** the command still exits 0 when the evidence was ingested but extraction reports `FAILED`. Check `extraction.succeeded` in JSON mode.
- **Errors after storage:** the API stores the evidence *before* extracting. So if the upload fails at a non-validation stage, a note explains that the evidence may already be ingested.

**Observed API limits (backend behaviour, verified against the running API):**
- **About 2 MiB maximum.** The API currently rejects uploads larger than about 2 MiB with HTTP 400 ("Streaming read error … multipart/form-data") or 413, even though its configured maximum evidence size is 50 MB. The CLI adds a note when a file over 2 MiB is rejected this way.
- **Content validation.** Files whose content the API doesn't recognise are rejected with HTTP 422 before anything is stored.

**`evidence list --case <UUID>`**
- Checks the case first. The evidence endpoint returns `[]` for an unknown case, so without that check "no evidence" and "no such case" would look the same.
- Then prints each receipt in the API's order: evidence ID, source type, size, creation time, ingestion status, file name, MIME type and the API-computed SHA-256.
- Records naming a different `case_id` are flagged. With PostgreSQL configured, the API reports every listed item's status as `INGESTED`.

### `yukti reconstruction`

```bash
yukti reconstruction run --case <UUID>
yukti reconstruction run --case <UUID> --json
yukti reconstruction explain --case <UUID>
yukti reconstruction explain --case <UUID> --json
yukti reconstruction compare --case <UUID>      # exits 69
```

Every subcommand validates `--case` locally first (exit 2, nothing sent). `run` and `explain` then check the case with `GET /api/v1/cases/:case_id` and stop if that fails. The reconstruction and report endpoints don't check that the case exists: for an unknown case they return an empty reconstruction.

**`reconstruction run --case <UUID>`**
- Sends `POST /api/v1/cases/:case_id/reconstruction` with the body `{}`: no `options`, `event_candidates` or `raw_events`. The API therefore uses its default `ReconstructionOptions` and the events it loads from its own stores (PostgreSQL, or the derived filesystem store). The CLI never fabricates events or options.
- Prints the server's `case_id` exactly as returned, then counts read from the response:
  - Timeline events: strictly ordered, concurrent/uncertain, excluded, overlapping pairs.
  - Spatial: trajectories, movement segments, segments over the speed threshold, overlaps.
  - Contradiction leads.
  - Labelled assumptions.
- Also prints the export safeguard (`CLEAR`/`BLOCKED`) and its reasons, the assumptions grouped with their basis, excluded events, a one-line entry per contradiction lead, and the API's disclaimer.
- **No run ID.** The API doesn't return one. The output says so instead of making one up.
- **Persistence isn't reported.** The API saves the result on a best-effort basis and returns HTTP 200 even if saving fails.
- A timeout (exit 3) doesn't mean the run failed: the API may still complete and store it.
- `BLOCKED` is not an error: `run` exits 0 whenever the API returned a reconstruction.

**`reconstruction explain --case <UUID>`**
- **The reconstruction.** `GET /api/v1/cases/:case_id/reconstruction` is shown first:
  - The timeline in the API's order, with time windows, timezone, clock offset, raw timestamp and assumption kinds.
  - The assumptions with their basis and the events they apply to.
  - Excluded events.
  - Spatial findings: trajectories, segments with speed-threshold evaluation, overlaps.
  - Contradiction leads with their neutral explanation and limitations.
  - The export safeguard.
- **Provenance is unknown.** The API returns its latest stored reconstruction if it has one, and otherwise computes one on the fly with default options without storing it. The response doesn't say which happened. The output shows `Provenance UNKNOWN`, and JSON shows `"provenance": "unknown_stored_or_computed_on_the_fly"`.
- **The report.** `GET /api/v1/cases/:case_id/report` is shown next: report ID, title, generation time, judicial export status and reason, forensic disclaimer, and the Markdown dossier exactly as returned. The API loads or recomputes the reconstruction again for this request, so the CLI warns if the report's counts differ from the reconstruction shown.
- **If the report request fails,** the reconstruction is still shown, the report is marked `UNAVAILABLE` (nothing is substituted), and the command exits with the report error's code (for example 5). JSON mode sets `"complete": false` and fills `report.error`.
- **If the reconstruction request fails,** the report isn't requested.

**`reconstruction compare --case <UUID>`**
- The API has no scenario-comparison endpoint yet, and its contract hasn't been agreed. The command sends no request, compares nothing locally, and doesn't change the active reconstruction.
- It exits `69` with the `unsupported` error.

## Configuration

Each setting is resolved in this order: command-line flag, then environment variable, then default. An environment variable that is set but blank counts as unset.

| Setting | Flag | Environment variable | Default |
|---|---|---|---|
| API base URL | `--api-url <URL>` | `YUKTI_API_URL` | `http://127.0.0.1:8080` |
| Request timeout (whole seconds, 1-3600) | `--timeout <SECONDS>` | `YUKTI_TIMEOUT_SECS` | `10` |
| JSON output | `--json` | — | off |

Global options can appear before or after the command (`yukti --json status` = `yukti status --json`).

**API URL rules:**
- It must be an absolute `http`/`https` URL with a host.
- A path prefix is allowed, for example `http://proxy.local/yukti`.
- A trailing slash is ignored.
- Embedded credentials, query strings and fragments are rejected.

Invalid configuration exits with code `2`, and the message names the source (`--api-url`, `YUKTI_API_URL`, …). The CLI doesn't follow HTTP redirects.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. For `status`, this means the API is reachable and readiness is `ready` or `degraded`. |
| `1` | Internal CLI failure, not caused by the API (for example, the async runtime couldn't start). |
| `2` | Invalid command, arguments or configuration. |
| `3` | API unreachable: connection refused, DNS failure or timeout. |
| `4` | The API answered with an HTTP 4xx (for example, a missing endpoint). |
| `5` | The API answered with an HTTP 5xx, or readiness is `not_ready`. |
| `6` | Unexpected response: invalid JSON, wrong shape, unknown status value, or a non-2xx/4xx/5xx status. |
| `69` | The functionality isn't supported by the API yet. |

`yukti status` never exits `0` when the API is unreachable or not ready.

## Output

- **Human mode (default):** results go to stdout and errors that stop a command go to stderr. The text is plain ASCII with no colour, so it's safe in logs. `yukti status` always prints its report to stdout, including on failure.
- **JSON mode (`--json`):** exactly one JSON document is written to stdout, for success and failure alike, and nothing is written to stderr. Each document has a `schema` field naming its versioned shape.

### `yukti.cli.doctor/v1`

```json
{
  "schema": "yukti.cli.doctor/v1",
  "cli_version": "0.1.0",
  "api_url": "http://127.0.0.1:8080",
  "result": "ok_with_warnings",
  "exit_code": 0,
  "passed": 4, "warnings": 3, "failed": 0, "skipped": 0,
  "checks": [
    { "id": "database", "label": "Database", "status": "warn",
      "detail": "not configured (filesystem mode, no PostgreSQL)",
      "hint": "Fine for local use: ...", "error": null }
  ]
}
```

- `result` is `ok`, `ok_with_warnings` or `failed`. `api_url` is `null` when the configuration is invalid.
- `checks` always lists `cli`, `config`, `api`, `readiness`, `database`, `graph`, `cases_api`, in that order.
- Each check's `status` is `pass`, `warn`, `fail` or `skipped`. `error` uses the same shape as `yukti.cli.error/v1`'s `error`.

### `yukti.cli.status/v1`

```jsonc
{
  "schema": "yukti.cli.status/v1",
  "api_url": "http://127.0.0.1:8080",          // resolved base URL, no trailing slash
  "result": "degraded",                         // ready | degraded | not_ready | unreachable |
                                                // health_failed | readiness_unavailable | unexpected_response
  "exit_code": 0,
  "api_reachable": true,                        // the API answered /health with any HTTP response
  "service": "sutram-backend-rust",             // from /health, null if it failed
  "health":    { "url": "...", "http_status": 200, "body": { ... }, "error": null },
  "readiness": { "url": "...", "http_status": 200, "body": { ... }, "error": null },  // null if not checked
  "dependencies": {                             // null unless /health/ready returned a ReadinessResponse
    "database": "unconfigured_filesystem_mode", // raw API values, not reinterpreted
    "graph": "disabled",
    "error": null                               // the API's own error text, if any
  }
}
```

- `body` is the server's JSON response, kept exactly as sent. It's `null` when there was no response or the body wasn't JSON.
- `error` is `{ "kind", "message", "http_status"?, "api_code"?, "url"? }`.

### `yukti.cli.case_list/v1`

```jsonc
{
  "schema": "yukti.cli.case_list/v1",
  "api_url": "http://127.0.0.1:8080",
  "count": 2,
  "cases": [ { /* CaseDto exactly as returned */ } ],   // API order
  "storage": {                                          // from GET /health/ready
    "readiness_status": "degraded",                     // null if unknown
    "database": "unconfigured_filesystem_mode",         // raw API value, null if unknown
    "error": null                                       // why it is unknown, if it is
  },
  "placeholder_case_ids": ["…"],                        // records with metadata.storage = "filesystem"
  "warnings": ["…"]                                     // the same warnings human mode prints on stderr
}
```

### `yukti.cli.case/v1`

This document is shared by `case create` and `case show`.

```jsonc
{
  "schema": "yukti.cli.case/v1",
  "api_url": "http://127.0.0.1:8080",
  "action": "create",            // or "show"
  "case": { /* CaseDto exactly as returned */ },
  "warnings": []
}
```

`CaseDto` fields as the API serializes them:
- `case_id`, `title` and `status` are strings.
- `description`, `fir_number` and `investigator_id` are strings or `null`.
- `metadata` is a JSON object.
- `created_at` and `updated_at` are RFC 3339 UTC strings.

### `yukti.cli.evidence_ingest/v1`

```jsonc
{
  "schema": "yukti.cli.evidence_ingest/v1",
  "api_url": "http://127.0.0.1:8080",
  "case_id": "…",
  "file_path": "data/synthetic/sample_cdr.csv",     // as given on the command line
  "request": {                                     // multipart fields exactly as sent; null = not sent
    "case_id": "…", "file_name": "sample_cdr.csv", "file_size_bytes": 624,
    "source_type": null, "source_timezone": "Asia/Kolkata", "clock_offset_seconds": "-30",
    "metadata": null, "auto_extract": "true"
  },
  "response": { /* complete response body: EvidenceUploadReceipt, or {receipt, extraction} */ },
  "evidence_id": "…",
  "sha256_hash": "…",                              // computed by the API, not verified by the CLI
  "auto_extract_requested": true,
  "extraction": { "status": "SUCCESS", "succeeded": true },   // null if no extraction run returned
  "warnings": []
}
```

### `yukti.cli.evidence_list/v1`

```jsonc
{
  "schema": "yukti.cli.evidence_list/v1",
  "api_url": "http://127.0.0.1:8080",
  "case_id": "…",
  "count": 2,
  "evidence": [ { /* EvidenceUploadReceipt exactly as returned */ } ],   // API order
  "warnings": []
}
```

`EvidenceUploadReceipt` fields:
- `evidence_id`, `case_id`, `original_filename`, `source_type`, `mime_type`, `sha256_hash` and `storage_uri` are strings.
- `file_size_bytes` is an integer.
- `status` is `INGESTED`, `EXTRACTION_PENDING`, `EXTRACTED`, `EXTRACTION_FAILED` or `CORRUPTED`.
- `metadata` is an object. It holds the declared `source_timezone` and `clock_offset_seconds`, plus your `--metadata`.
- `created_at` is an RFC 3339 UTC string.

The extraction run's `status` is `PENDING`, `RUNNING`, `SUCCESS`, `PARTIAL_SUCCESS`, `FAILED` or `INTERRUPTED`.

### `yukti.cli.reconstruction_run/v1`

```jsonc
{
  "schema": "yukti.cli.reconstruction_run/v1",
  "api_url": "http://127.0.0.1:8080",
  "case_id": "…",                         // as requested (canonical UUID)
  "server_case_id": "…",                  // response case_id exactly as returned; null if absent
  "request": { "method": "POST", "path": "/api/v1/cases/…/reconstruction", "body": {} },
  "summary": {                            // counted by the CLI from "response"
    "timeline_total_events": 2, "timeline_strictly_ordered": 2, "timeline_concurrent_uncertain": 0,
    "timeline_excluded_events": 1, "timeline_overlap_pairs": 0,
    "spatial_trajectories": 1, "spatial_movement_segments": 1,
    "spatial_segments_exceeding_speed_threshold": 1, "spatial_overlaps": 0,
    "contradiction_leads": 1, "contradiction_evaluated_segments": 1, "contradiction_flagged_segments": 1,
    "assumptions": 2, "events_with_assumptions": 2
  },
  "export_safeguard": { "status": "BLOCKED", "reasons": ["…"] },   // as returned; null if absent
  "response": { /* FullCaseReconstructionResult exactly as returned */ },
  "api_limitations": ["…"],               // e.g. no run ID is returned
  "warnings": []
}
```

There is deliberately no `run_id` field: the API doesn't return one.

### `yukti.cli.reconstruction_explanation/v1`

```jsonc
{
  "schema": "yukti.cli.reconstruction_explanation/v1",
  "api_url": "http://127.0.0.1:8080",
  "case_id": "…",
  "complete": true,                        // false when the report could not be retrieved
  "reconstruction": {
    "request": { "method": "GET", "path": "/api/v1/cases/…/reconstruction" },
    "provenance": "unknown_stored_or_computed_on_the_fly",
    "server_case_id": "…",
    "summary": { /* as in reconstruction_run/v1 */ },
    "response": { /* FullCaseReconstructionResult exactly as returned */ }
  },
  "report": {
    "request": { "method": "GET", "path": "/api/v1/cases/…/report" },
    "response": { /* CaseForensicReport exactly as returned */ },   // null on failure
    "error": null                                                   // error object on failure
  },
  "api_limitations": ["…"],
  "warnings": []
}
```

When `complete` is `false`, the process exit code is the report error's code.

### `yukti.cli.error/v1`

This document is used when a command can't run or complete, for example because the configuration or arguments are invalid, or because the API returned an HTTP error.

```json
{
  "schema": "yukti.cli.error/v1",
  "exit_code": 2,
  "error": { "kind": "config", "message": "invalid configuration: ..." }
}
```

Error `kind` values:
- `config`
- `invalid_argument` (exit 2: blank title, malformed UUID, invalid metadata JSON)
- `unreachable`
- `timeout`
- `http_client_error`
- `http_server_error`
- `http_unexpected_status`
- `unexpected_response`
- `unsupported`
- `internal`

HTTP errors include the API's `{"error":{"code","message"}}` envelope fields as `api_code` and `message`. If the body isn't that envelope, a short single-line excerpt is shown instead.

## Layout

```text
src/main.rs           process entry: argument parsing, runtime, printing, exit code
src/lib.rs            execute(): config -> client -> command dispatch (used by tests)
src/cli.rs            clap definitions (global options + Command enum)
src/config.rs         flag/env/default resolution and URL/timeout validation
src/client.rs         all HTTP: endpoint paths, typed API calls, transport-error mapping
src/dto.rs            client-side mirrors of API types (cases, evidence receipts, extraction
                      runs, upload fields, reconstruction results, case reports), CaseId validation
src/error.rs          CliError, exit-code contract, API error-envelope parsing
src/output.rs         Rendered output, JSON error document
src/commands/         one module per command group (status.rs, doctor.rs, case.rs, evidence.rs,
                      reconstruction.rs); each renders its own human and JSON output
tests/support/        std-only mock HTTP server (method routing, per-route delays, request
                      recording) and fixtures
tests/status_command.rs   status behaviour against mock APIs (library level)
tests/doctor_command.rs   doctor checklist against mock APIs, incl. read-only and skip behaviour
tests/case_command.rs     case list/create/show against mock APIs (library level)
tests/evidence_command.rs evidence ingest/list against mock APIs, incl. multipart byte checks
tests/reconstruction_command.rs  reconstruction run/explain/compare against mock APIs
tests/binary.rs       compiled-binary tests: env precedence, exit codes, streams
```

`src/lib.rs` exists for two reasons:
- Integration tests can run commands in-process.
- The exit-code constants and error variants are public API, so they aren't flagged as dead code before every command uses them.

## Testing

```bash
cd cli
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
cargo clippy --all-targets -- -D warnings
```

- The tests don't need Docker, PostgreSQL, Neo4j or a running API. They use a small mock HTTP server built on the standard library, bound to `127.0.0.1` on a random port.
- The response fixtures copy the real handler output in `backend-rust/src/api/routes/health.rs` and `cases.rs`. If those handlers change, update `tests/support/mod.rs` and `src/dto.rs`.
- The reconstruction and report fixtures in `tests/reconstruction_command.rs` follow `FullCaseReconstructionResult` and `CaseForensicReport` in `backend-rust/src/reconstruction/`. Update them together with `src/dto.rs` when those types change.
- CI: `.github/workflows/cli.yml` runs on changes under `cli/`. It runs fmt, a locked check, tests and clippy (all targets) on Linux, and a locked check and tests on Windows MSVC. See `docs/decisions/0002-standalone-cli-crate.md` for why the CLI is a separate HTTP-only crate.
- Every dependency is also used by `backend-rust`, so the crate builds offline (`cargo build --offline`) on any machine that has already built the backend.
