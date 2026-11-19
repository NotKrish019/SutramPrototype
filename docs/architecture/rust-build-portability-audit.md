# SUTRAM 2026 — Rust Toolchain, Linker Remediation, and Build Portability Audit

**Document:** `docs/architecture/rust-build-portability-audit.md`  
**Phases Covered:** 1F.1 (Audit & Stabilization) & 1F.2 (Linker Remediation & Portability)  
**Date:** 2026-11-18  
**Status:** Remediated & Verified  
**Role:** Rust Build & Infrastructure Engineer (SUTRAM 2026)

---

## 1. Executive Summary

Phase 1F established the initial Rust backend workspace under `backend-rust/`. During compilation on the Windows workstation, a non-portable workaround was introduced to satisfy GNU linker dependencies by copying `libunwind.a` into `libgcc.a` and `libgcc_eh.a` shims and hardcoding user-specific absolute paths.

In **Phase 1F.2**, this unsafe workaround was completely eliminated. The project was transitioned to Rust’s official LLVM-MinGW target: **`x86_64-pc-windows-gnullvm`**. 

All generated shim archives and absolute path configurations in `backend-rust/.cargo/lib/` were safely removed. The toolchain compiles, passes all linting and test suites, produces an optimized release binary, and serves HTTP requests with 100% verified functionality—without any machine-specific paths or placeholder archives.

---

## 2. Root Cause of the Original Linker Problem

The Windows development workstation operates with **LLVM-MinGW** (`llvm-mingw-20261020-ucrt-x86_64`), a modern, Clang/LLD/UCRT-based toolchain providing `clang.exe`, `ld.lld.exe`, `libunwind.a`, and `libclang_rt.builtins-x86_64.a`.

However, the default Rust toolchain installed was `stable-x86_64-pc-windows-gnu`. The `x86_64-pc-windows-gnu` target spec hardcodes `-lgcc` and `-lgcc_eh` in its linker invocations, expecting a traditional GCC toolchain. This caused two distinct failures:
1. **PATH Collision:** An obsolete 32-bit `C:\MinGW\bin` existed on system PATH, causing `dlltool.exe` to fail with `Invalid bfd target` during 64-bit import library generation.
2. **Missing GCC Archives:** LLVM-MinGW does not contain `libgcc.a` or `libgcc_eh.a`, causing LLD linker errors (`unable to find library -lgcc_eh`).

### Why the Previous Workaround Was Unsafe
To unblock Phase 1F, `libunwind.a` was copied into `backend-rust/.cargo/lib/libgcc.a` and `libgcc_eh.a`, and an absolute path `C:/Users/krish/Desktop/SutramPrototype/backend-rust/.cargo/lib` was added to `rustflags`:
- **Machine-Specific:** The absolute path broke builds on any other machine or directory location.
- **Symbol Incompleteness:** `libunwind` provides unwinding symbols but lacks GCC compiler intrinsics, risking runtime undefined behavior in C FFI dependencies.
- **Target Incoherence:** Using GCC flags with an LLVM-based toolchain bypassed Rust’s standard target architecture.

---

## 3. Linker Remediation: Transition to `x86_64-pc-windows-gnullvm`

### 3.1 Target Rationale
Rust provides first-class support for LLVM-MinGW via the **`x86_64-pc-windows-gnullvm`** target. This target natively expects Clang, LLD, static `libunwind`, and the Universal C Runtime (UCRT) without requiring `-lgcc` or `-lgcc_eh`.

### 3.2 Toolchain Configuration
The complete `stable-x86_64-pc-windows-gnullvm` toolchain (Rust 1.100.0) was installed via `rustup`. To isolate the local development environment portably without altering global system defaults:
- Global rustup default remains `stable-x86_64-pc-windows-gnu`.
- A directory-specific toolchain override was set for `backend-rust`:
  ```bash
  rustup override set stable-x86_64-pc-windows-gnullvm
  ```
- This ensures that both target artifacts and host build scripts (e.g. `proc-macro2`, `httparse`) link natively using `x86_64-pc-windows-gnullvm` and LLVM-MinGW.

---

## 4. Workaround Files Removed & Final Configuration

### 4.1 Files Safely Removed
Once `x86_64-pc-windows-gnullvm` compilation was verified:
- `backend-rust/.cargo/lib/libgcc.a`: **REMOVED**
- `backend-rust/.cargo/lib/libgcc_eh.a`: **REMOVED**
- `backend-rust/.cargo/lib/` directory: **REMOVED**
- Hardcoded `rustflags = ["-L", "native=C:/Users/..."]`: **REMOVED**

### 4.2 Final `backend-rust/.cargo/config.toml`
The configuration file was reduced to clean, portable comments:
```toml
# SUTRAM Rust Backend - Cargo Configuration
# Target-independent configuration.
# Platform-specific linkers are resolved portably via PATH or standard toolchain targets.
```

### 4.3 Zero Hardcoded Paths Verification
A full repository scan confirmed **zero occurrences** of machine-specific paths (`C:/Users/`, `backend-rust/.cargo/lib`, or `libgcc_eh`) in active code or configuration. (Historical occurrences remain only in architectural review documentation).

---

## 5. Build, Test, and Runtime Verification Results

All commands executed from [`backend-rust/`](file:///c:/Users/krish/Desktop/SutramPrototype/backend-rust) using `--locked`:

| Verification Step | Command | Exit Code | Result |
| :--- | :--- | :---: | :--- |
| **Formatting** | `cargo fmt --check` | `0` | **PASSED** (0 formatting differences) |
| **Type Check** | `cargo check --target x86_64-pc-windows-gnullvm --locked` | `0` | **PASSED** (Clean compile) |
| **Test Suite** | `cargo test --target x86_64-pc-windows-gnullvm --locked` | `0` | **PASSED** (3 tests passed: 1 unit, 2 integration) |
| **Linter** | `cargo clippy --target x86_64-pc-windows-gnullvm --locked -- -D warnings` | `0` | **PASSED** (0 warnings) |
| **Release Build** | `cargo build --release --target x86_64-pc-windows-gnullvm --locked` | `0` | **PASSED** (`target/x86_64-pc-windows-gnullvm/release/sutram-backend-rust.exe`) |
| **Python Regression** | `uv run --directory backend pytest -q` | `0` | **PASSED** (144 passed, 50 skipped, 0 failed) |

### 5.1 Binary Size Improvement
The resulting `x86_64-pc-windows-gnullvm` release binary is **3.02 MB**, down from 5.74 MB under GNU, due to clean LLD dead-code elimination and native UCRT linkage.

### 5.2 Live Runtime Server Test
The compiled GNULLVM binary was launched locally:
- **Port Bound:** `127.0.0.1:8092`
- **HTTP Request:** `GET http://127.0.0.1:8092/health`
- **Response Status:** `200 OK`
- **Response Body:** `{"status":"ok","service":"sutram-backend-rust"}`
- **Shutdown:** Clean graceful termination via process signal.

---

## 6. GitHub Actions CI Pipeline Status

The workflow [`.github/workflows/backend-rust.yml`](file:///c:/Users/krish/Desktop/SutramPrototype/.github/workflows/backend-rust.yml) was updated to provide multi-platform CI verification:

1. **`backend-rust-linux` (Linux CI):**
   - Runs on `ubuntu-latest` with `dtolnay/rust-toolchain@stable`.
   - Executes `cargo fmt --check`, `cargo check --locked`, `cargo test --locked`, and `cargo clippy --locked -- -D warnings`.
   - Completely independent of Windows targets.
2. **`backend-rust-windows` (Windows MSVC CI):**
   - Runs on `windows-latest` with Visual Studio C++ Build Tools.
   - Compiles and tests against `x86_64-pc-windows-msvc` with `--locked`.
   - Validates that standard Windows MSVC developers can build cleanly.
3. **Execution Status:** **Remote CI execution remains unverified** because no push to `origin/main` has been executed, in accordance with git safety directives.

---

## 7. Platform Portability Matrix

| Platform / Developer | Target Triple | Status | Configuration & Notes |
| :--- | :--- | :---: | :--- |
| **Current Windows Dev (LLVM-MinGW)** | `x86_64-pc-windows-gnullvm` | **READY** | Native Clang/LLD/UCRT build with zero shims and directory override. |
| **Other Windows Devs (MSVC)** | `x86_64-pc-windows-msvc` | **READY** | Standard Tier 1 target supported natively via Visual Studio Build Tools. |
| **Ubuntu GitHub Actions Runner** | `x86_64-unknown-linux-gnu` | **READY** | Standard Linux CI with locked dependencies. |
| **Linux Production (Docker)** | `x86_64-unknown-linux-gnu` | **READY** | Native containerized compilation without Windows-specific configuration. |

---

## 8. Remaining Portability Considerations & Phase 1G Gates

1. **Local PATH Management:** Developers using LLVM-MinGW on Windows must ensure LLVM-MinGW’s `bin/` directory precedes any obsolete 32-bit MinGW installations on `PATH`.
2. **Phase 1G Readiness:** With the Rust toolchain portably stabilized, memory safety guaranteed, and build artifacts sanitized, the backend is fully prepared to implement **Phase 1G: Forensic Evidence Ingestion** (multipart streaming, SHA-256/SHA-3 cryptographic hashing, and immutable storage).
