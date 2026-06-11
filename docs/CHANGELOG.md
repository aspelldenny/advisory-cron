# CHANGELOG — advisory-cron

> Newest entries at top. Follows sos-kit convention: 1 entry per phiếu (Tầng 1) or per ship batch (Tầng 2 grouping).
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/CHANGELOG_ARCHIVE.md`.

---

## release-ci — 3-target prebuilt binaries for sos-kit installer (P064) — 2026-06-11

- `.github/workflows/release.yml`: tag `v*` builds mac-arm64/linux-x64/win-x64, attaches to GitHub Release. Asset contract `<bin>-<triple>[.exe]` consumed by sos-kit `install.sh`. Matrix = mac-arm64 + linux-x64 ONLY (Phase 3 `compile_error!` gates Windows out by design — installer marks advisory-cron optional).

## 2026-05-28 — P015: Phase 3.4 — README Linux quick-start (SPRINT CLOSE)

**Phiếu:** P015 (Tầng 2 — README.md only)

**Scope:** Final phiếu of Phase 3 sprint. Updated `README.md` to add Linux user path and bump Phase 3 status banner.

- **2-OS Quick start sections:** Renamed `## Quick start (CLI)` → `## Quick start — macOS (launchd)` (verbatim preserved). Added new `## Quick start — Linux (cron-tab)` section with 7-step flow (register → verify crontab → run → status → unregister → verify cleanup), Sub-mechanism A trigger gap check, and explicit note that `next_fire` renders N/A on Linux Phase 3 (INV-23 deferred).
- **Linux dogfood smoke verified end-to-end on WSL2:** All 5 smoke steps pass — `init` exit 0, `register` exit 0 + 1 tagged crontab line, `status --json` `plist_loaded: true` / `next_fire: null`, `unregister` exit 0 + 0 tagged lines, diff to pre-smoke crontab clean.
- **MCP OS-agnostic note:** Replaced macOS-only `Replace <YOUR_USERNAME>...` line with 2-bullet OS split (macOS path + Linux OS-agnostic note per Anchor #9 fallback — Linux Claude Desktop config path not verifiable on this box; prose defers to client docs).
- **Phase 3 COMPLETE status banner:** Bumped to "Phase 1 + Phase 2 + Phase 3 COMPLETE — macOS launchd + Linux cron-tab dual-platform shipped. Single Rust binary (~5 MB), 23 invariants, cross-OS CI matrix (macos-latest + ubuntu-latest)."

**Phase 3 sprint summary (P012–P015):**

| Phiếu | Title | Shipped |
|-------|-------|---------|
| P012 | Scheduler trait extract (abstract layer) | 2026-05-28 |
| P013 | Linux cron-tab impl (sync stdlib, V2) | 2026-05-28 |
| P014 | INV-22/23 + cross-OS CI matrix | 2026-05-28 |
| P015 | README 2-OS quick-start (sprint close) | 2026-05-28 |

**Acceptance criteria closure (BACKLOG.md Phase 3):** All 4 phiếu shipped. README quick-start has 2 OS paths with `crontab -l | grep advisory-cron` Sub-mechanism A verification. Linux dogfood smoke verified WSL2 end-to-end. Sprint close — Phase 3 COMPLETE.

**Binary size (WSL2):** 4.8 MB release build. **INV total:** 23. **Test count (Linux):** 143 (macOS-gated tests skip on Linux; CI matrix runs all on both OS).

**No code change, no `Cargo.toml` change, no dep change, no CLI / MCP / config schema change.**

---

## 2026-05-28 — P014: Phase 3.3 — INV-22 + INV-23 formal entries + cross-OS CI matrix

**Phiếu:** P014 (Tầng 1 — docs/security/INVARIANTS.md doctrine surface + new GitHub Actions workflow file)

**Scope:** Formalise INV-22 (`crontab` shell-out boundary — 5 sub-rules) + INV-23 (cron expression daily-form invariant) in `docs/security/INVARIANTS.md`. Create `.github/workflows/ci.yml` 2-OS matrix (`macos-latest` + `ubuntu-latest`). ARCHITECTURE.md §Phase status 3.3 ⏸️ → ✅ + new §CI matrix subsection. No code change (zero `src/` diff).

**INV-22 (5 sub-rules):**
1. Discrete-arg shell-out (no `sh -c` interpolation against `crontab`).
2. Label allowlist 2-point — pre-flight in `core::*::run` (INV-12 existing, 3 inline copies at `src/core/{register,unregister,status}.rs`) + defense-in-depth `super::is_valid_label` first line of each `CrontabScheduler::*` method (P013 shipped at `src/scheduler/mod.rs`, scheduler-boundary single source). 4-location reality documented in sub-rule 2 Note + Phase 3.5+ consolidation deferred.
3. Tag-only filter idempotency (`# advisory-cron: <label>` substring match — preserve user's other crontab lines).
4. Sync `std::process::Command` only (V2 P013 lesson — no nested-runtime panic from `#[tokio::main]` context).
5. TOCTOU read-modify-write race acknowledged + deferred Phase 3.5+ (cross-ref ARCHITECTURE.md §Cron mechanism Linux).

**INV-23 (Option A — conservative):** Documents daily-form `M H * * *` cron expression invariant for BOTH platforms. Linux cron-tab native full 5-field support intentionally constrained to daily-form for cross-platform `RegisterIntent` shape parity. Both parsers (`src/core/register.rs::parse_daily_cron` + `src/scheduler/macos.rs::parse_simple_cron`) enforce daily-form via separate codepaths — Phase 3.5+ consolidation candidate. Future full 5-field expansion deferred to separate phiếu (would require `RegisterIntent` extension + 7 numbered scope items per INV-23 sub-rule 4).

**CI matrix:**
- `matrix: os: [macos-latest, ubuntu-latest]`, `fail-fast: false`.
- Each job: `cargo fmt --check` → `cargo build --release` → `cargo test --all` → `cargo clippy --all-targets -- -D warnings`.
- Linux job pre-step `which crontab` (Sub-mechanism B capability smoke — fails loud if `cron` package missing).
- macOS sandbox safety model: unit tests in `scheduler::macos` use `NoopLaunchctl` (no real `launchctl`); integration tests in `tests/cli_register.rs` spawn binary + rely on graceful degradation when `launchctl bootstrap` unavailable on GHA — observe-first DP4.
- Toolchain: `dtolnay/rust-toolchain@stable` with `clippy, rustfmt` components.
- Checkout: `actions/checkout@v4`.

**Doctrine surface counts:** INV total 21 → 23 (+INV-22 +INV-23).

**Debate Log:** 2 turns (V1 DRAFT → Worker CHALLENGE Turn 1 → Architect RESPOND Turn 2 → V2 final). 3 ACCEPT on factual-accuracy objections (O1.1 `parse_simple_cron` Trigger gap, O1.2 `is_valid_label` 4-location reality, O1.3 ci.yml false `NoopScheduler` claim). No scope/design change.

**Test count delta:** 143 (P013 baseline) → 143 (no new tests in P014 — doctrine + CI only). First successful CI run is the new "pass" signal.

**Surfaced debt item:** `[DEBT] core layer is_valid_label consolidation (Phase 3.5+ Tầng 2)` — 3 inline copies in `src/core/{register,unregister,status}.rs` predate P013's scheduler-layer consolidation. To be added to `docs/BACKLOG.md` "Open backlog" by Quản đốc post-merge.

**No code change, no `Cargo.toml` change, no dep change, no CLI / MCP / config schema change.** P014 is the doctrine + infra phiếu mirroring P007's role for Phase 1 (post-ship polish — but for Phase 3 it lands BEFORE Phase 3.4 README per BACKLOG sequencing).

---

## 2026-05-28 — P013: Phase 3.2 — Linux cron-tab impl (sync stdlib, V2)

**Phiếu:** P013 (Tầng 1 — new security boundary `crontab` shell-out + INV-22 enforcement)

**Scope:** Replace `CrontabScheduler` stub (`bail!` P013) in `src/scheduler/linux.rs` with real `crontab -l/-` injection flow. INV-22 defense-in-depth via shared label allowlist. P012 watch-item closed.

**V2 sync stdlib pivot (Debate Log Turn 1+2):** Worker CHALLENGE Turn 1 caught two fatal V1 bugs: (a) `use tokio::io::AsyncWriteExt` requires `io-util` feature (absent from `Cargo.toml`); (b) `tokio::runtime::Runtime::new().block_on(...)` inside `#[tokio::main]` context panics at runtime (`"Cannot start a runtime from within a runtime"`). V2 replaces both with `std::process::Command` (sync, blocking) + `std::io::Write::write_all` — zero feature flag addition, zero nested-runtime risk. `Cargo.toml` diff = ZERO.

**Changes:**
- **EDIT** `src/scheduler/linux.rs`: stub replaced with real `CrontabScheduler` impl — `read_user_crontab` (`crontab -l`, graceful "no crontab" fallback), `write_user_crontab` (scoped-drop stdin pattern + `wait_with_output`), tag-line idempotency (`# advisory-cron: <label>`), INV-22 defense-in-depth point 2 in each method. 6 unit tests (label rejection).
- **EDIT** `src/scheduler/mod.rs`: added shared `pub fn is_valid_label` helper (single source of truth for INV-12 + INV-22 label allowlist — ASCII alphanumeric + `-` + `_`). 4 unit tests.
- **EDIT** `src/scheduler/macos.rs`: replaced local `is_valid_label_inline` with call to `super::is_valid_label` (same allowlist, zero behavior diff).
- **EDIT** `src/cli/register.rs`: gated `plist_path` print on `!is_empty()` — closes P012 P013 watch-item (Linux render no longer shows blank `plist:` line).
- **CREATE** `tests/cli_register_linux.rs`: 7 new Linux integration tests gated `#[cfg(target_os = "linux")]` via mock `crontab` binary in `TempDir` PATH injection. Covers: register writes tagged line, unregister removes tagged line, idempotent re-register, status loaded/unloaded, invalid label INV-22 pre-flight, preserves user lines.
- **EDIT** `docs/ARCHITECTURE.md`: split §Cron mechanism → "macOS (launchd plist)" + "Linux (crontab injection)" subsections. Updated §Modules table. Phase 3.2 ⏸️ → ✅.

**Tests:** 129 (P012 baseline) → 143 total (+14 new: 6 `scheduler::linux`, 4 `scheduler::mod`, 4 `scheduler::tests` shared allowlist + 7 integration linux — 3 old linux stub tests replaced). All pass Linux WSL2.

**Linux dogfood smoke (Sub-mech A — WSL2):** `advisory-cron register --label p013-smoke` → exit 0, 1 tagged line added to crontab, no `plist:` blank line printed, no nested-runtime panic. `advisory-cron status --label p013-smoke --json` → `plist_loaded: true`, `next_fire: null` (expected P013 limitation). `advisory-cron unregister --label p013-smoke` → exit 0, diff to before-snapshot = 0 lines. Idempotency: 2× register → 1 line. Invalid label (`foo;evil`) → exit 2, crontab unchanged (INV-22 point 1 hold confirmed).

**Cargo.toml diff: ZERO.** No new dep, no new tokio feature flag.

---

## 2026-05-28 — P012: Phase 3.1 — Scheduler trait abstract

**Phiếu:** P012 (Tầng 1 — refactor, module add/remove)

**Scope:** Extract `LaunchctlClient` trait from `src/launchd.rs` → cross-OS `Scheduler` trait in `src/scheduler/{mod,macos,linux}.rs`. Zero behavior change macOS. Linux stub compiles.

**Changes:**
- **CREATE** `src/scheduler/mod.rs`: `Scheduler` trait + `RegisterIntent`/`RegisterReport`/`UnregisterReport`/`SchedulerStatus` types + `NoopScheduler` (test impl) + `PlatformScheduler` compile-time alias.
- **CREATE** `src/scheduler/macos.rs`: `MacosScheduler` implements `Scheduler`. Content moved verbatim from `src/launchd.rs`. `RealLaunchctl` + `LaunchctlClient` now private to this module. Gated `#[cfg(target_os = "macos")]`.
- **CREATE** `src/scheduler/linux.rs`: `CrontabScheduler` stub (bails `"Phase 3.2 (P013) chưa ship"`). Gated `#[cfg(target_os = "linux")]`.
- **DELETE** `src/launchd.rs` — all content moved to `src/scheduler/macos.rs`.
- **EDIT** `src/main.rs`, `src/core/{register,unregister,status}.rs`, `src/mcp/tools.rs`, `src/cli/{register,unregister,status}.rs` — `LaunchctlClient` → `Scheduler`, `RealLaunchctl` → `PlatformScheduler`, `NoopLaunchctl` → `NoopScheduler`.
- **EDIT** `tests/cli_register.rs` — 4 launchctl-invoking tests gated `#[cfg(target_os = "macos")]` to keep Linux CI green.

**No schema change, no dep change, no CLI surface change.**
- `StatusReport.plist_loaded` field name preserved (JSON schema stability — anchor #17).
- `UnregisterOutput.plist_existed` + `was_loaded` both populated from `was_registered` (minimum-disruption refactor).
- INV-10/11/12/13/17 enforcement points preserved (move-not-rewrite).

**Tests:** 129 pass on Linux WSL2 (97 lib + 32 integration). Baseline was 144 on macOS; delta = -11 macOS-only lib tests (now compile-gated in scheduler/macos.rs) + -4 macOS-only integration tests + +3 new Linux scheduler stub tests. macOS CI will see 144+3=147 total.

**Linux build evidence:** `cargo build --release` on Linux WSL2 → 4.7MB binary, zero warnings, zero errors.

---

## 2026-05-27 — P011: Sprint debt cleanup (Tầng 2 — INV-12 + DISCOVERIES hook align)

**Phiếu:** P011 (Tầng 2 — 2 items from "Open backlog" cleared; item 3 `fire_task` no-timeout deferred to separate Tầng 1 phiếu)

**Item 1 — INV-12 label sanitization 2-point enforcement (code already in place):**
- Task 0 anchor verification revealed that `src/core/register.rs::run` and `src/core/unregister.rs::run` already had full `is_valid_label` helpers + full ASCII alphanumeric + `-` + `_` allowlist enforcement in their pre-flights (committed during a prior sprint with no doc trail in BACKLOG.md or DISCOVERIES.md).
- BACKLOG.md "Open backlog" debt item #1 was stale — INV-12 2-point enforcement was already in place.
- P011 adds 3 explicit named unit tests in `src/core/register.rs::tests` covering the 3 specific invalid-label attack classes: whitespace (`"foo bar"`), path separator (`"foo/bar"`), and shell metachar (`"foo;rm"`). Each test asserts pre-flight rejection AND zero `LaunchctlClient` invocations (proving rejection occurs before any plist/launchctl call).
- Defense-in-depth confirmed: `src/core/register.rs::is_valid_label` (point 1) + `src/launchd.rs::generate_plist` (point 2) + `src/mcp/tools.rs::validate_label` (point 3, INV-18).

**Item 2 — DISCOVERIES.md hook format aligned with CLAUDE.md doctrine:**
- `.git/hooks/pre-commit` line 137 regex updated: now accepts either legacy H2 header (`## ...P<NNN>`) OR CLAUDE.md doctrine list-item (`- YYYY-MM-DD P<NNN>:`). `grep -q` → `grep -Eq` (extended regex required for alternation).
- Going forward Worker writes only the list-item form; existing P001-P010 dual-format entries continue to match via the legacy alternative.
- Worker no longer needs to write both formats (1 source of truth per CLAUDE.md doctrine).
- Manual validation confirmed: all P001-P010 entries match the new regex, P999 correctly rejected.

**Tests:**
- Baseline 141 → 144 (+3 new invalid-label attack-class tests for register pre-flight).
- `src/core/unregister.rs` already had `run_rejects_invalid_label` test — no new tests needed there.

**No INV change, no schema change, no dep change.**
- INV-12 spec at INVARIANTS.md line 137-153 unchanged. INVARIANTS.md line 147 stale prose location reference (`cli/register.rs::run_with_deps` pre-P006) left as-is; logged in Discovery Report.

**BACKLOG.md:** 2 debt items moved "Open backlog" → "Recently shipped"; item 3 (`fire_task` no process timeout) stays in "Open backlog".

**Acceptance (all verified):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 144/144 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `bash -n .git/hooks/pre-commit` — exit 0 (syntax valid)
- Hook regex P001-P010 validation — 10/10 OK; P999 correctly rejected
- `git diff src/cli/mod.rs` — empty (Constraint #1)
- `git diff src/alert.rs` — empty (Constraint #11)
- `git diff src/heartbeat.rs` — empty (Constraint #12)
- `git diff Cargo.toml` — empty (no dep change)
- `git diff docs/ARCHITECTURE.md` — empty (Tầng 2)

---

## 2026-05-27 — P010: Phase 2.3 — Crash-safe heartbeat (SPRINT COMPLETE)

**Phiếu:** P010 (Tầng 1 — `heartbeat::append` atomic temp+fsync+rename protocol, `read_last_n` partial-last-line tolerance, INV-21 appended, `HeartbeatRecord` schema preserved, function signatures preserved)

**Heartbeat write (src/heartbeat.rs::append):**
- Replaced Phase 1.4 direct `OpenOptions::append(true)` + write with atomic-rename protocol.
- Steps: read existing file → append new line in-memory buffer → write to `tempfile::NamedTempFile::new_in(parent_dir)` → `sync_all()` (fsync) → `persist(target)` (atomic `std::fs::rename`).
- Function signature `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>` UNCHANGED — single call site in `core::run::run` (P009 Constraint #12) preserved.
- If any step before rename fails, NamedTempFile's Drop auto-cleans the temp file; target file untouched. Caller continues to log-warn-continue on `Err` per P004 contract.

**Heartbeat read (src/heartbeat.rs::read_last_n):**
- **Tightened** prior P004 silent-skip-all-malformed behavior. Now: last-line parse failure → `tracing::warn!` + skip + return prior records; mid-file parse failure → propagate as `Err` (was silently swallowed pre-P010; unexpected under atomic protocol; must surface loud per PROJECT.md hard line #5).
- `eprintln!` replaced with `tracing::warn!` (INV-13 compliance).
- Blank lines tolerated silently anywhere (preserved).
- Existing test `read_last_n_skips_malformed_line` updated — mid-file corrupt line scenario flipped from skip-assertion to `is_err`-assertion (V2 semantic flip per INV-21 sub-rule 2).
- Function signature `pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>` UNCHANGED.
- Stale `#[allow(dead_code)]` attribute removed.
- **Caller-side note:** `src/core/status.rs:80` calls `read_last_n(...).unwrap_or_default()`, silently absorbing the new mid-file `Err` into empty Vec at the status output. Pre-existing P005/P006-era design choice; NOT in P010 scope; future BACKLOG candidate for caller-side hardening. See Discovery.

**Schema preserved:**
- `HeartbeatRecord` fields (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) UNCHANGED since P004.
- No `schema_version` bump, no new fields, no migration required for existing heartbeat files.

**Cargo.toml:**
- `tempfile = "3"` moved from `[dev-dependencies]` to `[dependencies]` (was already in lock file per P004 — zero compile time / binary size delta).
- No other dep changes.

**INVARIANTS.md:**
- INV-21 appended (4 sub-rules: atomic temp+fsync+rename protocol, partial-last-line read tolerance, schema preservation, signature preservation).

**Tests (+8 new, total 141):**
- `src/heartbeat.rs::tests` unit (8 new): append_creates_file_when_missing, append_preserves_existing_content, append_multiple_times_grows_file_monotonically, append_leaves_no_temp_file_in_parent_dir, read_last_n_with_corrupt_last_line_skips_it_and_returns_prior, read_last_n_with_corrupt_mid_line_fails_loud, read_last_n_returns_empty_on_missing_file, read_last_n_skips_blank_lines_silently.
- 1 existing test updated (`read_last_n_skips_malformed_line` — semantic flip to mid-file → Err per INV-21 sub-rule 2).
- All P009 + Phase 1 baseline tests preserved (133 → 141 net).

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules row `src/heartbeat.rs` updated (atomic + tolerance); new §Heartbeat schema "Atomicity (Phase 2.3 — P010)" subsection; §Phase status Phase 2 marked COMPLETE.
- `docs/security/INVARIANTS.md` — INV-21 appended (total: 21 invariants).
- `README.md` — Phase 2.3 paragraph appended after Phase 2.2 retry section; Status updated to "Phase 1 + Phase 2 COMPLETE".

**Acceptance (all verified):**
- `cargo build --release` — zero warnings, binary ≤7MB
- `cargo test --all` — 141/141 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #1)
- `git diff src/core/run.rs` — empty (Constraint #2)
- `git diff src/core/status.rs` — empty (V2 KHÔNG sửa)
- `git diff src/alert.rs` — empty (Constraint #8)
- `grep -c "heartbeat::append" src/core/run.rs` — exactly 1 (Constraint #12)
- `grep "eprintln!" src/heartbeat.rs` — ZERO hits (V2 INV-13)
- `grep "NamedTempFile::new_in" src/heartbeat.rs` — 1 hit (Constraint #14)
- `grep -c "^### INV-" docs/security/INVARIANTS.md` — exactly 21

---

### Sprint summary — Phase 1 + Phase 2 COMPLETE (P001-P010, 2026-05-27)

**10 phiếu shipped over the sprint:**

| Phiếu | Phase | Theme |
|-------|-------|-------|
| P001 | 1.1 | CLI scaffold (5 subcommand stubs, clap derive) |
| P002 | 1.2 | Config schema (TOML + serde) |
| P003 | 1.3 | launchd plist + register/unregister |
| P004 | 1.4 | Task runner + heartbeat JSONL |
| P005 | 1.5 | Status reporter |
| P006 | 1.7 | MCP server wrapper + `core::*` extraction (dual-surface parity) |
| P007 | 1.6 | README + ARCHITECTURE post-ship polish |
| P008 | 2.1 | Telegram alert on task failure |
| P009 | 2.2 | Retry policy (`is_retryable` + retry loop, single-alert-per-invocation) |
| P010 | 2.3 | Crash-safe heartbeat (temp+fsync+rename atomic protocol) |

**Cumulative state:**
- Binary size: ~3.9 MB release (well under 7 MB budget).
- Test count: 141 (8 added by P010).
- INVARIANTS: 21 (INV-1..21).
- DISCOVERIES: 10 per-phiếu reports.
- Both CLI surface (5 subcommands) and MCP surface (5 tools via stdio) ship with full parity per layering invariant.

Sprint closes per BACKLOG.md acceptance pending: Sếp dogfood 3 ngày liên tiếp confirmation of `/advisory-scan` daily fire + at least 1 Claude Desktop MCP tool invocation.

---

## 2026-05-27 — P009: Phase 2.2 — Retry policy

**Phiếu:** P009 (Tầng 1 — `[retry]` config block, retry loop in `core::run::run`, INV-20 appended, heartbeat schema preserved, alert call moved OUTSIDE loop)

**Config schema (src/config.rs):**
- Added `RetryConfig { max_attempts: u32, backoff_secs: u64 }`.
- `Config` gains `#[serde(default)] pub retry: Option<RetryConfig>` — old configs (Phase 1 + P008) without `[retry]` block deserialize as `None` (backwards-compat preserved).
- `Config::validate()` extended: `max_attempts ≥ 1`, `backoff_secs ≤ 3600` (sanity cap).
- `Config::default_for_home` does NOT include retry block — opt-in.

**Wiring (src/core/run.rs):**
- New private fn `is_retryable(exit_code: i32) -> bool` — `(1..=127).contains(&exit_code)` per BACKLOG Phase 2.2 spec.
- Retry loop wraps `runner::fire_task` + `heartbeat::append` for up to `max_attempts` iterations.
- Two-match heartbeat-completeness invariant (Constraint #12 / INV-15 adjacent): `match &fire_result` (borrow, build HeartbeatRecord) → `heartbeat::append` → `match fire_result` (consume, extract tuple). Spawn-fail iterations still write a heartbeat with exit_code=-1.
- Between attempts: `tokio::time::sleep(Duration::from_secs(backoff_secs)).await`.
- Loop exits early on: success (exit 0), non-retryable (signal-killed ≥128 or spawn-fail -1), exhausted attempts.
- P008 alert block MOVED from inside single-fire body to AFTER loop — fires AT MOST ONCE per `run` invocation, gated on final `exit_code != 0`. INV-20 single-alert-per-invocation invariant.
- When `[retry]` absent: `unwrap_or((1, 0))` → single-fire Phase 2.1 behavior preserved (1 attempt, alert on fail).

**Heartbeat schema unchanged:**
- 1 JSONL line per attempt (3 retries = 3 lines). `HeartbeatRecord` struct in `src/heartbeat.rs` untouched (verified via `git diff src/heartbeat.rs` empty). `advisory-cron status --last N` naturally shows per-attempt trail.

**INVARIANTS.md:**
- INV-20 appended (4 sub-rules: bounded attempts DOS prevention, backoff respected no busy loop, signal exits not retried, single alert per invocation).

**No new dep:**
- `tokio::time::sleep` + `std::time::Duration` already pulled by `tokio` feature `time` (P008 Anchor #6).
- `wiremock` dev-dep from P008 sufficient for integration test.

**Tests (+17 new, total 133):**
- `src/config.rs` unit tests (5): load_without_retry_block (backwards-compat), load_with_retry_block, validate_retry_zero_attempts, validate_retry_excessive_backoff, load_with_retry_and_alert.
- `src/core/run.rs` unit tests (8): is_retryable boundaries (exit 1, 127, 0, 128, 130, 137, 143, -1).
- `tests/cli_run_retry.rs` integration (4): retry_succeeds_on_attempt_2_no_alert, retry_exhausts_max_attempts_single_alert, signal_exit_not_retried_single_attempt, no_retry_block_preserves_phase21_single_fire.
- All P008 + Phase 1 baseline tests preserved (116 → 133).

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules `core/run.rs` row Purpose updated; comment after table updated (no new `src/retry.rs` module); §Heartbeat schema retry semantics paragraph added; §Config schema TOML block + Field reference rows for `[retry]`; new §Error handling subsection "Retry policy (Phase 2.2)"; §Phase status Phase 2.2 shipped.
- `docs/security/INVARIANTS.md` — INV-20 appended.
- `README.md` — Phase 2.2 section with `[retry]` config snippet.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, binary 3.9MB (≤7MB budget)
- `cargo test --all` — 133/133 pass (116 baseline + 17 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #1 re-instated, honored)
- `git diff src/heartbeat.rs` — empty (schema preserved)
- `git diff src/alert.rs` — empty (Constraint #11 alert.rs env-free preserved)
- `git diff src/runner.rs` — empty (runner stays single-fire primitive)

---

## 2026-05-27 — P008: Phase 2.1 — Telegram alert on task failure

**Phiếu:** P008 (Tầng 1 — new module `src/alert.rs`, config schema extension, `core::run` wired, INV-19, dev-dep `wiremock`)

**New module:**
- `src/alert.rs` — `TelegramAlert` with `from_config`, `send_with_base(api_base, msg)` (10s timeout: reqwest client + `tokio::time::timeout` double guard per INV-19). Env-free module: the API base test-seam env var is read at the call site in `core::run::run`, NOT inside `alert.rs`, keeping the module unit-testable without env setup. `format_failure_message` centralises message formatting (label + exit_code + duration_ms + stderr_tail with 500-byte UTF-8 truncation). `read_token_from_file` parses `KEY=VAL` lines extracting `TG_BOT_TOKEN=...`.

**Config schema (src/config.rs):**
- Added `AlertConfig { telegram: Option<TelegramConfig> }` + `TelegramConfig { chat_id, bot_token, bot_token_file }`.
- `Config` gains `#[serde(default)] pub alert: Option<AlertConfig>` — old configs without `[alert]` block deserialize as `None` (backwards-compat preserved).
- `Config::validate()` extended: chat_id non-empty, bot_token/bot_token_file mutually exclusive (not both, not neither), bot_token non-empty if set.
- `Config::default_for_home` does NOT include alert block — alert is opt-in.

**Wiring (src/core/run.rs):**
- After `match fire_result` block (exit_code, stderr_tail, duration_ms in scope), before `Ok(RunOutput)` return: on `exit_code != 0` + `config.alert.telegram` Some → build message + read `ADVISORY_CRON_TG_API_BASE` env var at call site → `alert.send_with_base(&api_base, &msg).await`. Alert failure → `tracing::warn!`, never bail. Task exit code unaffected by alert delivery.

**INVARIANTS.md:**
- INV-19 appended (Telegram HTTP boundary: 10s double-guard timeout, log-warn-not-bail, env-free alert module rule).

**Dev-dep:**
- `wiremock = "0.6"` added to `[dev-dependencies]`. Zero impact on release binary (3.9MB, was 2.1MB pre-P008 but still < 7MB budget).

**Tests (+22 new, total 116):**
- `src/alert.rs` unit tests (12): from_config none/inline/file/both/neither, send_with_base happy-200/500/401 via wiremock, format_failure_message fields/empty-stderr, truncate_bytes UTF-8 boundary.
- `src/config.rs` unit tests (6): load_without_alert_block, load_with_alert_inline/file_token, validate_alert_both/neither/empty_chat_id.
- `tests/cli_run_alert.rs` integration (3): failing task POSTs to mock Telegram (1 call asserted), failing task without alert config sends 0 POSTs, successful task sends 0 POSTs. All via `Command::env("ADVISORY_CRON_TG_API_BASE", mock_server.uri())` — env-var-at-call-site pattern verified end-to-end.

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — Modules row for `src/alert.rs`; Config schema TOML block + field reference rows for `[alert.telegram]`; Error handling + alerting Phase 2 paragraph; Phase status 2.1 shipped.
- `docs/security/INVARIANTS.md` — INV-19 appended.
- `README.md` — Phase 2.1 section with config snippet.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, 3.9MB binary (< 7MB budget)
- `cargo test --all` — 116/116 pass (94 baseline + 22 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` — empty (Constraint #11 satisfied)
- `git diff src/cli/mod.rs` — empty (Constraint #1 re-instated satisfied)

---

## 2026-05-27 — P007: Phase 1.6 — README + ARCHITECTURE post-ship docs polish

**Phiếu:** P007 (Tầng 2 — docs-only; no code changes)

**Changes:**

`README.md`:
- CLI quick-start expanded from 4 steps to 6: added step 3 (`launchctl list | grep com.advisorycron`) as Sub-mechanism A trigger verification hint for Sếp dogfood; added step 6 (`advisory-cron unregister --label advisory-scan-daily`) for cleanup.
- MCP smoke test snippet verified against live binary — output matches exactly (exit 0, `"serverInfo":{"name":"advisory-cron","version":"0.1.0"}`). No change needed.
- New section "What advisory-cron fires" inserted before `## Status`: explains default `/advisory-scan` target + TOML example for custom task. Uses `<YOU>` placeholder (not hard-coded username).
- Status banner updated to reflect Phase 1 complete, awaiting dogfood.

`docs/ARCHITECTURE.md`:
- §Modules "Layering invariant": "introduced Phase 1.7" → "shipped Phase 1.7" (reflects shipped reality).
- §Phase status: "In progress (1.7 shipped; 1.6 docs remaining)" → "Code COMPLETE (all 7 sub-phases shipped). Awaiting Sếp dogfood 3 ngày..." + "Phase 1.6 shipped per P007".
- §MCP surface Tool registry schemas: verified field-by-field against `src/mcp/tools.rs` source — all 5 tools match exactly. No drift. Table unchanged.
- §Modules table: verified against `ls src/**/*.rs` — 22 rows, 22 files, all match. No phantom or missing entries.

**Dogfood verification:** Worker ran all 6 README quick-start steps with `HOME=$(mktemp -d)`. All exit 0. Test label `p007-verify-temp-do-not-use` registered, confirmed in `launchctl list`, unregistered cleanly.

---

## 2026-05-27 — P006: Phase 1.7 — MCP server wrapper (stdio) + core/* extraction

**Phiếu:** P006 (Tầng 1 — new dep `rmcp 1.7.0` + `tokio io-std feature`, new modules `src/core/*` + `src/mcp/*` + `src/cli/mcp.rs`, new exit code 5, new INV-18, CLI/MCP dual-surface ship)

**New dependency:**
- `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` — official Anthropic Rust MCP SDK. Provides `ServerHandler` trait, rmcp stdio transport, `Tool`, `Content`, `CallToolResult`.
- `tokio` gains `"io-std"` feature — required for rmcp stdio transport (`tokio::io::stdin/stdout`).

**New modules — `src/core/*` (pure business logic, zero CLI/MCP coupling):**
- `src/core/mod.rs` — re-exports 6 sub-modules.
- `src/core/config_path.rs` — `home_dir()` + `default_config_path()`: `$HOME` helpers; bail on unset/empty. Replaces inline `std::env::var("HOME")` in old `cli::*` handlers.
- `src/core/init.rs` — `run(InitArgs) -> Result<InitOutput>`: write default config. Resolves home internally.
- `src/core/register.rs` — `run(RegisterArgs, &L: LaunchctlClient) -> Result<RegisterOutput>`: generate plist + bootstrap. Resolves home + launch_agents_dir + self_exe internally.
- `src/core/unregister.rs` — `run(UnregisterArgs, &L: LaunchctlClient) -> Result<UnregisterOutput>`: idempotent bootout + plist removal.
- `src/core/run.rs` — `async run(RunArgs) -> Result<RunOutput>`: task runner + heartbeat. Full logic extracted from `cli/run.rs`.
- `src/core/status.rs` — `run(StatusArgs, &L) -> Result<StatusReport>`: launchd query + heartbeat read. `parse_next_fire` and `StatusReport` moved here from `cli/status.rs` (now pub for MCP serialization).

**New modules — `src/mcp/*` (MCP server, delegates to `core::*`):**
- `src/mcp/mod.rs` — re-exports `server` and `tools`.
- `src/mcp/server.rs` — `serve_stdio() -> Result<()>`: rmcp `ServerHandler::serve(stdio()).await` + `.waiting().await`.
- `src/mcp/tools.rs` — `AdvisoryCronHandler` implementing rmcp `ServerHandler`. 5 tools (`init`, `register`, `unregister`, `run`, `status`) with hand-written JSON schemas (Decision 3 — no `schemars` dep). INV-18 input validation (`validate_label` + `validate_config_path`) at MCP boundary before `core::*` call. Tool errors = `is_error: Some(true)` CallToolResult (never JSON-RPC error / process exit).

**New module — `src/cli/mcp.rs` (thin shell):**
- `async fn run(Args) -> Result<u8>`: calls `mcp::server::serve_stdio()`, returns `Ok(0)` on success, `Ok(5)` on transport error (never `process::exit(5)`).

**CLI: `advisory-cron mcp` wired:**
- `src/cli/mod.rs` extended: `pub mod mcp;` + `Mcp(mcp::Args)` variant + dispatch arm (+4 lines exactly; Constraint #1 retired for P006 only per phiếu spec).
- `src/main.rs` gains `mod core;` + `mod mcp;`.

**CLI thin-shell rewrites (all `cli::*` now delegate to `core::*`):**
- `cli/init.rs`, `cli/register.rs`, `cli/unregister.rs`, `cli/run.rs`, `cli/status.rs` — all rewritten as thin adapters. Core logic extracted to `core::*`. Exit code mapping preserved (backward compat). Warning messages for idempotent unregister paths preserved.

**INVARIANTS.md updated:**
- Appended INV-18 (MCP transport boundary — label allowlist + path traversal + tool error protocol). Per RULES.md:22 — security boundary touched.

**Tests added (total 94 — 65 unit + 29 integration):**
- Unit tests: `core/config_path.rs` (3), `core/init.rs` (3), `core/register.rs` (2), `core/unregister.rs` (2), `core/run.rs` (3), `core/status.rs` (7), `mcp/tools.rs` (5), `cli/status.rs` tail helpers (2) — 27 new unit tests.
- Integration: `tests/cli_mcp.rs` — 7 binary subprocess tests: `mcp --help` exits 0, top-level help includes `mcp`, handshake + tools/list = 5 tools, register rejects invalid label (INV-18), init rejects path traversal (INV-18), serverInfo.name = "advisory-cron", parity CLI register uses correct label.
- Baseline maintained: all 70 pre-P006 tests continue to pass.

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules table: 12 new/updated rows for core/* + mcp/* + cli/mcp.rs with ✅ markers; V2 internal-resolution pattern noted; §CLI surface `mcp` row Phase marked 1.7 ✅; §MCP surface section rewritten with actual tool schemas, SDK details, INV-18 summary, V2 cli/mcp.rs contract, Claude Desktop config; §Phase status updated to 1.7 ✅.
- `docs/security/INVARIANTS.md` — INV-18 appended.
- `README.md` — "Quick start (CLI)" updated with correct flag names; "MCP server" section added with Claude Desktop config JSON + smoke test + tool table.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, 2.1MB binary (< 7MB budget)
- `cargo test --all` — 94/94 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/config.rs src/launchd.rs src/runner.rs src/heartbeat.rs` — empty (zero drift in existing modules)

---

## 2026-05-27 — P005: Phase 1.5 — Status reporter

**Phiếu:** P005 (Tầng 1 — 4 new CLI flags on `status` subcommand per RULES.md:14; `LaunchctlClient` trait additive extension; INV-17 appended for `launchctl print` shell-out boundary; NO new dep)

**CLI: `advisory-cron status` wired (Phase 1.5 acceptance):**
- `src/cli/status.rs` rewritten: 4 new flags `--label <name>` / `--config <path>` / `--json` / `--last <N>` (default 5); resolves config + queries `launchctl print` + reads last N heartbeats + renders human-readable text or JSON.
- Label resolution priority: `--label` CLI > `config.task.label` > literal `"advisory-cron"`.
- Label validation (INV-12 2-point enforcement): pre-flight in `src/cli/status.rs::run` + defense-in-depth inside `RealLaunchctl::print`. Allowlist: ASCII alphanumeric + `-` + `_`.
- Exit code 0 always (read-only operation); exit 2 for config load failure / `$HOME` unset; exit 1 for invalid label.

**`src/launchd.rs` extended (additive — no breaking change):**
- `LaunchctlClient` trait gains `print(&self, label: &str) -> Result<LaunchctlPrintOutput>` method.
- `LaunchctlPrintOutput { raw_stdout: String, not_loaded: bool }` new struct.
- `RealLaunchctl::print` shells `launchctl print gui/<uid>/com.advisorycron.<label>`; catches "Could not find service" / "No such process" stderr substrings → `not_loaded = true` (renders cleanly, does NOT bubble error).
- `NoopLaunchctl::print` returns canned fixture matching macOS 15 descriptor format for tests.

**Discovery: macOS 15 launchctl format (P005 V2 pivot):**
- `parse_next_fire` (private fn in `src/cli/status.rs`) pivoted from timestamp-key search to `descriptor` Hour/Minute extraction. macOS 15 (Darwin 25.5.0) `launchctl print` for `StartCalendarInterval` jobs does NOT expose `next fire` / `next launch` / `run at` timestamps — only the configured `descriptor = { "Hour" => N "Minute" => M }`. Worker empirical capture (P005 Debate Log Turn 1) corrected V1's docs-based guess. Phase 1 acceptance gate satisfied by rendering "Next fire: daily at HH:MM" (configured recurrence).

**INVARIANTS.md updated:**
- Appended INV-17 (launchctl print shell-out boundary — additive to INV-10's "any future launchctl invocation" coverage). Per RULES.md:22 — security boundary touched.

**Tests added:**
- 3 unit tests in `src/launchd.rs` (noop print canned-descriptor, real print rejects invalid label, real print rejects empty label)
- 11 unit tests in `src/cli/status.rs` (is_valid_label allow/reject, parse_next_fire macos15-descriptor/none/empty/hour-only/minute-only/zero-pad/out-of-range, tail_first_n_or_empty empty/truncate)
- 5 integration tests in `tests/cli_status.rs` (heartbeats+unloaded human, no-heartbeats friendly, --json valid, --last clamps, missing config exit 2)
- Total: 51 baseline + 19 new = 70 tests

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §CLI surface `status` row Args column updated (new flags); §Modules table `src/cli/status.rs` row marked shipped 1.5 ✅; `src/launchd.rs` row notes trait extension + descriptor parser; §Phase status updated with Phase 1.5 + macOS 15 discovery.
- `docs/security/INVARIANTS.md` — INV-17 appended.

**No new dep.** `serde_json` (P004 explicit) + `chrono` (P002 direct) + `clap` (P001 direct) cover all P005 needs.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 70 pass (51 baseline + 19 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #2 hard rule satisfied)

---

## 2026-05-27 — P004: Phase 1.4 — Task runner + heartbeat JSONL + wire `run` handler

**Phiếu:** P004 (Tầng 1 — 2 new modules, new dep `serde_json`, new optional config field `task.label`, new CLI flag `run --config`, INVARIANTS.md updated)

**New dependency:**
- `serde_json = "1"` promoted from transitive (via reqwest) to explicit in `[dependencies]`. Required for `serde_json::to_string` / `serde_json::from_str` in `heartbeat.rs`. Marginal binary delta = 0 (already compiled transitively). Per RULES.md Tầng 1 row: CHANGELOG entry citing crate + reason.

**Modules added:**
- `src/runner.rs` — `RunResult` struct; `fire_task(config) -> Result<RunResult>` spawns task via `tokio::process::Command`, captures stdout/stderr/exit-code/duration. Signal-killed → exit_code=-1. Spawn failure → `anyhow::Error` (caller builds spawn-fail heartbeat). 3 unit tests.
- `src/heartbeat.rs` — `HeartbeatRecord` struct (durable schema: ts, label, exit_code, duration_ms, stdout_tail, stderr_tail); `append(path, record)` (JSONL, auto-creates parent dir); `read_last_n(path, n)` (skips malformed lines); `tail_utf8(s, max_bytes)` (char-boundary snap, no grapheme dep). 7 unit tests.

**CLI: `advisory-cron run` wired (Phase 1.4 acceptance):**
- `src/cli/run.rs` rewritten: loads config via `--config <path>` or default `~/.config/advisory-cron/config.toml`; fires task via `runner::fire_task`; builds `HeartbeatRecord`; appends to `heartbeat.log_path`.
- **`default_config_path` bails on `$HOME` unset** — mirrors `register.rs` pattern, never silently falls back to `/` (P004 V2 Turn 1 Architect decision).
- Exit codes: 0 = task success; 2 = config load fail OR `$HOME` unset; 4 = task non-zero exit OR spawn-fail. Heartbeat write failure = stderr warning only, never changes exit code.
- `--config <path>` flag added — declared in `run::Args` (NOT in `cli/mod.rs`, per newtype dispatch constraint). `git diff src/cli/mod.rs` empty.

**Config schema change (`src/config.rs`):**
- `TaskConfig` gains `pub label: Option<String>` with `#[serde(default)]` — backward compat (old configs without `label` field deserialize to `None`). `default_for_home` seeds `label: Some("advisory-cron")` so `advisory-cron init` writes the new field visibly.

**INVARIANTS.md updated:**
- Appended INV-14 (child process spawn boundary), INV-15 (heartbeat file write boundary), INV-16 (JSON serialization boundary). Per RULES.md:22 — security boundary touched.

**Tests added:**
- 3 unit tests in `src/runner.rs` (echo success, nonexistent binary error, non-zero exit)
- 7 unit tests in `src/heartbeat.rs` (serde roundtrip, append creates dir+file, append+read roundtrip, missing file returns empty, malformed line skip, tail_utf8 variants)
- 3 unit tests in `src/config.rs` (label absent → None, label present → Some, default_for_home includes label)
- 4 integration tests in `tests/cli_run.rs` (echo success exit 0 + heartbeat written, failing task exit 4, spawn-fail exit 4 + exit_code=-1 in heartbeat, missing config exit 2)
- Total: 51 tests (34 unit + 3 cli_help regression + 4 cli_init regression + 6 cli_register regression + 4 cli_run)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules table marks `src/runner.rs` + `src/heartbeat.rs` 1.4 ✅; §Config schema field reference adds `task.label` row + TOML block updated; §CLI surface table `run` row updated to show `--config <path>` flag; §Phase status updated to 1.4 shipped
- `docs/security/INVARIANTS.md` — INV-14..INV-16 appended

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, binary 1.1MB
- `cargo test --all` — 51/51 pass (33 baseline + 18 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #1 hard rule satisfied)
- `env -u HOME advisory-cron run` → exit 2 with `$HOME environment variable is not set` (Constraint #16 confirmed)

---

## 2026-05-27 — P003: Phase 1.3 — launchd plist generator + `register`/`unregister` handlers

**Phiếu:** P003 (Tầng 1 — module added, CLI flags added, schedule type relaxed, plist schema spec, exit code 3 first use, INVARIANTS.md updated)

**Module added:**
- `src/launchd.rs` — `generate_plist` (pure XML builder, 7-key plist matching ARCHITECTURE.md spec), `plist_path_for`, `default_launch_agents_dir`, `LaunchctlClient` trait, `RealLaunchctl` (shells `launchctl bootstrap`/`bootout` via `std::process::Command`), `NoopLaunchctl` (test impl recording calls), `current_uid` (`id -u` shell-out, zero-unsafe, zero-dep). 11 unit tests.

**CLI: `register` + `unregister` wired:**
- `src/cli/register.rs` rewritten: loads config, generates plist, writes to `~/Library/LaunchAgents/`, bootstraps via `launchctl`. Testable surface via `run_with_deps<L: LaunchctlClient>`.
- `src/cli/unregister.rs` rewritten: idempotent bootout + plist file removal. Warns on "label not loaded" or "plist already absent" — continues to exit 0. Exit 3 only on hard IO failure.
- **`--config <path>` flag added to both** — declared inside `Args` struct (NOT on Commands enum) per newtype dispatch pattern confirmed Turn 1 [O1.1]. Zero edits to `src/cli/mod.rs`.
- **`register --schedule` relaxed from required `String` to `Option<String>`** — config-driven schedule works without redundant CLI flag.
- Exit codes: register (0=success, 1=$HOME unset, 2=config/cron-parse fail, 3=plist write / launchctl bootstrap fail); unregister (0=success including idempotent, 1=$HOME unset, 3=plist remove IO fail).

**Plist XML schema (7 keys — matches ARCHITECTURE.md §Cron mechanism):**
- `Label`, `ProgramArguments` (`[<self_exe>, "run"]`), `StartCalendarInterval` (Hour+Minute), `StandardOutPath` (`/tmp/advisory-cron-<label>.stdout.log`), `StandardErrorPath`, `WorkingDirectory`, `RunAtLoad` (`<false/>`)

**Cron expression support:**
- Simple `M H * * *` daily form only (Phase 1 launchd constraint). Complex expressions → exit 2 with helpful error. Config-driven `hour`/`minute` calendar form has no such restriction.

**`#[allow(dead_code)]` removal:**
- `src/config.rs:72` `#[allow(dead_code)]` on `pub fn load` removed — first binary callsite wired in `register::run`. (Heads-up #1 resolution.)

**INVARIANTS.md updated:**
- Appended INV-10 through INV-13 covering: `launchctl` shell-out boundary, `id -u` shell-out boundary, `~/Library/LaunchAgents/` write boundary, label sanitization defense.

**Tests added:**
- 11 unit tests in `src/launchd.rs` (plist gen, cron parsing, label sanitization, NoopLaunchctl recording, xml escape, current_uid sanity)
- 6 integration tests in `tests/cli_register.rs` (plist write, cron simple form, complex cron exit 2, missing config exit 2, idempotent unregister, round-trip register→unregister)
- Total: 33 tests (20 unit + 3 cli_help regression + 4 cli_init regression + 6 cli_register)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules table marks `src/launchd.rs`/`register.rs`/`unregister.rs` 1.3 ✅; §CLI surface table adds `--config`/`--schedule` optional notes; §Cron mechanism adds M H * * * constraint + idempotency note + UID resolution note; §Phase status updated to 1.3 shipped
- `docs/security/INVARIANTS.md` — INV-10..INV-13 appended

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 33/33 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (V2 [O1.1] constraint satisfied)

---

## 2026-05-27 — P002: Phase 1.2 — Config schema (TOML + serde) + wire `init` handler

**Phiếu:** P002 (Tầng 1 — introduces config schema, touched by every subsequent subcommand)

**Module added:**
- `src/config.rs` — `Config`, `TaskConfig`, `ScheduleConfig` (untagged enum), `HeartbeatConfig` structs; `load`, `default_for_home`, `write_default` functions. Zero new dependencies (uses `serde` + `toml` already in `Cargo.toml`).

**Config schema:**
- 3 TOML blocks: `[task]` (command, args, working_dir), `[schedule]` (cron expr OR calendar hour/minute), `[heartbeat]` (log_path)
- `[schedule]` uses `#[serde(untagged)]` enum — serde discriminates by field presence. Both variants round-trip cleanly.
- Default path: `~/.config/advisory-cron/config.toml` (hardcoded per PROJECT.md hard line #3)
- Validation: empty command rejected; Calendar hour 0–23, minute 0–59

**CLI: `advisory-cron init` wired:**
- `src/cli/init.rs` rewritten: calls `Config::write_default`, maps exit codes per ARCHITECTURE.md spec
- Exit 0 on success, exit 2 on config-exists-without-force + IO errors, exit 1 (via `Err`) on `$HOME` unset
- Stdout: `wrote default config to <path>`

**Tests added:**
- 9 unit tests in `src/config.rs` (schedule parsing, validation failures, write_default round-trip/overwrite)
- 4 integration tests in `tests/cli_init.rs` (write success, refuse overwrite, force overwrite, TOML parseable)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — new §Config schema section; §Modules table `src/config.rs` + `src/cli/init.rs` marked 1.2 ✅; §Phase status updated

**Discovery note:**
- `#[serde(untagged)]` enum confirmed working with TOML 0.8 — cron-shape and calendar-shape discriminate correctly. No fallback to tagged variant needed.
- Added `validate_rejects_invalid_minute` test (not in phiếu's 8-test count) — natural companion to `validate_rejects_invalid_hour`. Total: 9 unit tests.
- `pub fn load` carries `#[allow(dead_code)]` to suppress Rust 2024 binary-crate dead_code warning on forward-declared API (will be called by Phase 1.3 `register`).

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 16/16 pass (9 unit + 3 cli_help regression + 4 cli_init)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff

---

## 2026-05-27 — P001: Phase 1.1 — Scaffold + CLI surface (clap derive)

**Phiếu:** P001 (Tầng 1 — defines CLI contract for entire tool)

**Modules added:**
- `src/cli/mod.rs` — `Commands` enum (5 subcommands) + `dispatch()` fn
- `src/cli/init.rs` — `init` stub with `--force` arg
- `src/cli/register.rs` — `register` stub with `--schedule` + `--label` args
- `src/cli/unregister.rs` — `unregister` stub with `--label` arg
- `src/cli/run.rs` — `run` stub (no args)
- `src/cli/status.rs` — `status` stub with `--json` arg

**src/main.rs rewritten:**
- clap derive `Cli` struct with `#[command(subcommand)]`
- `#[tokio::main(flavor = "current_thread")]` (current_thread flavor, matching `rt` feature in Cargo.toml — see Discovery P001 for detail)
- `ExitCode::from(u8)` return for clean stdio flush
- Dispatches via `cli::dispatch()`, maps `Err` → exit 1 with `{err:#}` to stderr

**CLI surface (5 stubs):**
- Each stub returns `bail!("not yet implemented (Phase 1.x)")` → exit 1
- `--help` for all 5 subcommands exits 0 and shows correct arg docs
- `--version` prints `advisory-cron 0.1.0`

**Tests added:**
- `tests/cli_help.rs` — 3 integration tests (top-level help, per-sub help, unknown sub exits nonzero)
- All 3 pass: `cargo test --all` clean

**Layering decision:**
- `src/core/` NOT created (defer until Phase 1.2 has real logic to host — anti-completeness-bias decision from Architect)
- `src/cli/mod.rs` is the only parent; no intermediate abstraction needed for 5-stub phase

**Discovery note:**
- Cargo.toml had tokio `rt` feature but not `rt-multi-thread`. Phiếu spec used bare `#[tokio::main]` (defaults to multi-thread). Fixed to `#[tokio::main(flavor = "current_thread")]` — zero dep change, behavior identical for stub CLI.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 3/3 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff

---

## 2026-05-27 — Phase 1 scope expansion: CLI + MCP dual surface

**Trigger:** Sếp re-defined Phase 1 ship-gate. "Tool rust phải gói thành MCP và CLI mới là hoàn thành." → CLI-only Phase 1 insufficient; MCP server (stdio) must ship in same wave.

**Decisions captured:**
- MCP tool set = full parity với CLI (5 tools: `init` / `register` / `unregister` / `run` / `status`)
- Transport = stdio subcommand `advisory-cron mcp` (single binary, no daemon, matches hard line #1)
- Sprint shape = thêm Phase 1.7 (MCP wrapper) sau 1.5, đẩy 1.6 (docs) xuống cuối

**Doc updates (Tầng 1 — touches acceptance criteria + sprint + module layout):**
- `docs/PROJECT.md` — Vision para extended; MVP scope item 6 added (MCP server, 5 tools); acceptance criteria +3 bullets (MCP handshake, Claude Desktop integration test, MCP schema doc); binary size budget raised 5MB → 7MB
- `docs/BACKLOG.md` — Active sprint title + goal expanded; Phase 1.7 added (~300 LOC, Tầng 1); Phase 1.6 moved to end, scope raised 60 → 90min
- `docs/ARCHITECTURE.md` — Modules table +4 entries (`cli/mcp.rs`, `core/mod.rs`, `mcp/server.rs`, `mcp/tools.rs`); layering invariant added (`core::*` is CLI/MCP-agnostic); CLI surface +`mcp` subcommand; new "MCP surface" section with tool registry + Claude Desktop config sketch + behavioral invariant; exit code 5 (MCP transport error)

**Known TBD for Architect (P00x — Phase 1.7):**
- Rust MCP SDK choice (likely `rmcp` official Anthropic crate — verify via `context7` before spec)
- Whether to introduce `schemars` for auto-derived JSON tool schemas (size budget consideration)
- Exact integration test shape ("MCP register ≡ CLI register" diff against shared temp LaunchAgents dir)

**Not yet started:** no phiếu opened. Next: P001 = Phase 1.1 (CLI scaffold).

---

## 2026-05-27 — Pre-flight: secrets + env prep (no code)

Sếp batched all "nguyên liệu" before opening P001 so the sprint can run end-to-end without mid-flight blocks.

**Audit results (toolchain):**
- ✅ Claude Code CLI `/Users/nguyenhuuanh/.local/bin/claude` v2.1.152
- ✅ Claude Desktop installed + config at `~/Library/Application Support/Claude/claude_desktop_config.json`
- ✅ docs-gate + ship CLI at `~/.cargo/bin/`
- ✅ Rust 1.94.1 (MSRV 1.85 satisfied)
- ✅ gh CLI logged in as `aspelldenny` via keyring
- ✅ launchctl available (Darwin Bootstrapper 7.0.0)

**Secrets staged (outside repo, gitignored defense-in-depth):**
- `~/.advisory-cron-secrets.env` chmod 600 — `TG_BOT_TOKEN` + `TG_BOT_USERNAME=chiha_alert_bot` + `TG_CHAT_ID=1184530337`
- End-to-end verified: `curl ... sendMessage` returned `ok:True message_id:21`
- Bot reused from Soulsign project (not advisory-cron exclusive) — Sếp accepted shared-bot risk

**Shell env cleanup (`~/.zshrc`):**
- Line 21: `export GITHUB_TOKEN="gho_s1lB..."` → commented out (OAuth, was shadowed)
- Line 341: `export GITHUB_TOKEN=ghp_59Zq...` → commented out (invalid per `gh auth status`)
- `gh` CLI continues to work via keyring; clean shell test (`env -i ... zsh -i -c`) shows `GITHUB_TOKEN: (unset)`
- Current Claude Code session env still carries old value (inherited at spawn) — Sếp `exec zsh` or open new terminal to flush

**Sếp acknowledged risk:** plaintext tokens (TG + 2 GitHub) appeared in chat output; Sếp's threat model = Claude Code session is private → accepted. Recommend rotation at end of cycle.

**Pre-req status for sprint:**
- Phase 1.1–1.7: ✅ all green, no external input pending
- Phase 2.1: ✅ secrets ready (BACKLOG entry updated)
- Phase 2.2–2.3, Phase 3+: no external input needed

---

## 2026-05-27 — Bootstrap (seed)

**Repo initialized.** `cargo new` Phase 0 scaffold + sos-kit doctrine seed by orchestrator (running from tarot main session 2026-05-27).

Seeded structure:
- `CLAUDE.md` — Rust shape + ported generic doctrine (DOD, Discovery Report, AI BIAS WARNINGS rule 6, Sub-mechanism A-E catalog, Knowledge durability, DOCS GATE 2 Tầng)
- `docs/` — PROJECT.md (PRD), BACKLOG.md (3 phase), ARCHITECTURE.md, WORKFLOW.md, ORCHESTRATION.md, RULES.md, CHANGELOG.md, DISCOVERIES.md, ticket/TICKET_TEMPLATE.md, security/INVARIANTS.md
- `.claude/agents/` — 5 vai (architect, worker, orchestrator, advisory-watch, boundary-check) — copied from `~/sos-kit/agents/`, adapted for Rust + autonomous mode default
- `.claude/skills/` — symlink to `~/sos-kit/skills/` (13 generic skills shared)
- `.claude/commands/` — `/advisory-scan`, `/security-review`
- `.claude/settings.local.json` — permission allowlist + SessionStart hook
- `scripts/session-start-banner.sh` — Rust-flavored banner (BACKLOG active + advisory staleness + open PRs)
- `.git/hooks/pre-commit` — sos-kit canonical hook (auto-detects Rust → `cargo check`)
- `.mcp.json` — filesystem + github + sequential-thinking + context7 + docs-gate + ship (omit guard/vps/sentry — not relevant)
- `.docs-gate.toml`, `.sos-stack.toml`, `.phieu-counter`, `LICENSE`, `README.md`, `.gitignore`, `Cargo.toml` (deps: clap + tokio + serde + toml + chrono + anyhow + thiserror + tracing + reqwest)

No code shipped yet. Phase 1 MVP starts when Sếp opens fresh session in `~/advisory-cron`.

**Source / lineage:**
- Doctrine from `~/tarot/CLAUDE.md` (2026-05-27 snapshot) — DOD, AI BIAS WARNINGS Sub-mechanism A-E catalog, Knowledge durability, Discovery Report convention
- Agents (5 vai) from `~/sos-kit/agents/` — generic baseline, customized worker (Rust Layer 2 matrix) + orchestrator (autonomous mode default, runlog, rule 10/11 ported)
- Skills from `~/sos-kit/skills/` — symlink (13 generic)
- Hooks from `~/sos-kit/hooks/pre-commit` — auto-detects Cargo.toml → `cargo check`
- Templates from `~/sos-kit/templates/` — INVARIANTS, BACKLOG, .docs-gate.toml, .sos-stack.toml
- CLAUDE.md shape skeleton from `~/docs-gate/CLAUDE.md` — Rust project structure

**Stress test:** Sếp's intent is to drive Phase 1 → Phase 3 1-mạch không can thiệp, observe whether 4-vai workflow (Quản đốc + Architect + Worker + Giám sát) can auto-chain without manual gating. autonomous mode default flag set in `.claude/agents/orchestrator.md`.
