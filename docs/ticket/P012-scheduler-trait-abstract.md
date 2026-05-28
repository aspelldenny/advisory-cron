# PHIẾU P012: Scheduler trait abstract — extract `LaunchctlClient` → cross-OS `Scheduler` trait

> **Loại:** chore (refactor)
> **Tầng:** 1
> **Phase:** 3.1
> **Ưu tiên:** P1
> **Branch:** `feat/P012-scheduler-trait-abstract`
> **Ảnh hưởng:** `src/launchd.rs` (sunset → re-export), `src/scheduler/` (new module tree), `src/core/register.rs`, `src/core/unregister.rs`, `src/core/status.rs`, `src/mcp/tools.rs`, `src/main.rs` (mod decl), all `LaunchctlClient` / `RealLaunchctl` / `NoopLaunchctl` call sites
> **Dependency:** Không (P011 đã merge — sprint Phase 1+2 closed)
> **Tier-1 reason:** refactor trait extraction touches `core::*` injection points + new module tree (`src/scheduler/{mod,macos,linux}.rs`). Module add = Tầng 1 per RULES.md "Module added/removed". Cross-OS compile dispatch via `#[cfg(target_os = ...)]` = mechanism-level change worth ARCHITECTURE.md §Modules update.

---

## Context

### Vấn đề hiện tại

Phase 1+2 ship trên macOS dùng `src/launchd.rs::LaunchctlClient` trait + `RealLaunchctl` impl. Phase 3 yêu cầu support Linux qua `crontab -l/-` injection (BACKLOG.md Active sprint, Decision log 2026-05-28). Hiện trait là **macOS-specific** (method `bootstrap(plist_path: &Path)` chỉ làm sense với launchd; `LaunchctlPrintOutput.raw_stdout` chứa launchctl output format). Linux impl Phase 3.2 (P013) sẽ cần trait surface cross-OS — KHÔNG thể impl `bootstrap(plist_path)` với crontab (cron không có plist file concept).

Phase 3 cần **3 phiếu liên tiếp**: P012 (this — trait abstract, zero behavior change macOS), P013 (Linux crontab impl), P014 (INV-22/23 + CI matrix). P012 là pre-requisite cho P013 — không có trait abstract thì P013 phải fork trait hierarchy (anti-pattern).

### Giải pháp

Extract `LaunchctlClient` (`src/launchd.rs`) → `Scheduler` trait (`src/scheduler/mod.rs`) với surface **high-level intent**, KHÔNG raw OS-specific args:

- `register(intent: &RegisterIntent) -> Result<()>` — thay vì `bootstrap(plist_path)`. Plist generation move xuống `MacosScheduler::register` (currently lives in `core::register::run` via `generate_plist()` + `fs::write` + `client.bootstrap(plist_path)` 3-step). Trait giấu plist-vs-crontab khác biệt.
- `unregister(label: &str) -> Result<UnregisterReport>` — same as current `bootout(label)` + plist removal. macOS impl bundle 2-step; Linux impl 1-step crontab filter. `UnregisterReport { was_registered: bool }` exposed (replaces split `was_loaded`/`plist_existed` per-OS concepts).
- `status(label: &str) -> Result<SchedulerStatus>` — generalizes `print(label) -> LaunchctlPrintOutput`. `SchedulerStatus { is_registered: bool, raw_descriptor: Option<String> }`. macOS impl populates `raw_descriptor` từ launchctl stdout; Linux impl populates từ matched crontab line. `core::status::parse_next_fire` continues to parse macOS `descriptor = { "Hour" => ... }` format; Phase 3.2 P013 adds parallel `parse_cron_next_fire` cho Linux line — out of scope P012.

Compile-time dispatch:

```rust
// src/scheduler/mod.rs
pub mod macos;
pub mod linux;

#[cfg(target_os = "macos")]
pub use macos::MacosScheduler as PlatformScheduler;
#[cfg(target_os = "linux")]
pub use linux::CrontabScheduler as PlatformScheduler;
```

Linux module ship as **stub** P012: `CrontabScheduler` struct + `impl Scheduler` body `bail!("Phase 3.2 — P013 chưa ship")` cho mọi method. Lý do: Linux CI job sẽ compile P012 binary; không thể bỏ trống module hoặc `#[cfg]` out hết — `PlatformScheduler` re-export phải resolve. Linux tests skip qua `#[cfg(target_os = "macos")]` gate trên `NoopScheduler`-using tests.

`LaunchctlClient` trait + `RealLaunchctl` + `NoopLaunchctl` + `LaunchctlPrintOutput` + `current_uid` + plist helpers (`generate_plist`, `plist_path_for`, `default_launch_agents_dir`, `parse_simple_cron`, `xml_escape`) — all move sang `src/scheduler/macos.rs`. `src/launchd.rs` sunset → DELETE (file removed). `MacosScheduler` = new struct wrapping `RealLaunchctl` + plist generation responsibility (move from core::register).

### Scope

**CHỈ sửa:**

1. **CREATE** `src/scheduler/mod.rs` — declare `Scheduler` trait + `RegisterIntent`/`UnregisterReport`/`SchedulerStatus` types + `pub use` re-exports + `NoopScheduler` (test impl moved/renamed from `NoopLaunchctl`) + compile-time `PlatformScheduler` alias.
2. **CREATE** `src/scheduler/macos.rs` — `MacosScheduler` struct + `impl Scheduler for MacosScheduler`. Contents moved verbatim từ `src/launchd.rs`: plist generation (`generate_plist`, `plist_path_for`, `default_launch_agents_dir`, `parse_simple_cron`, `xml_escape`), `RealLaunchctl` (now PRIVATE — only used by `MacosScheduler` internally), `LaunchctlPrintOutput` (now PRIVATE), `current_uid`. All existing `#[cfg(test)] mod tests` move alongside.
3. **CREATE** `src/scheduler/linux.rs` — `CrontabScheduler` stub struct + `impl Scheduler` returning `bail!("Phase 3.2 — P013 chưa ship")` cho mỗi method. Hidden behind `#[cfg(target_os = "linux")]` (compile-time gated cả module).
4. **DELETE** `src/launchd.rs` — content all moved to `src/scheduler/macos.rs`. Module declaration `mod launchd;` removed from `src/main.rs`.
5. **EDIT** `src/main.rs` — replace `mod launchd;` with `mod scheduler;`.
6. **EDIT** `src/core/register.rs` — generic `<L: LaunchctlClient>` → `<S: Scheduler>`. `RegisterArgs` body unchanged. Logic: build `RegisterIntent` (label + resolved schedule + self_exe + working_dir + stdout/stderr paths) → `scheduler.register(&intent)`. Plist generation + `fs::write` + plist_path composition move INTO `MacosScheduler::register`. `RegisterOutput.plist_path` field: keep for backwards-compat — populate từ scheduler.register return (extend trait's `register` to return `Result<RegisterReport>` containing `plist_path: Option<PathBuf>` — `Some` on macOS, `None` on Linux. Phase 3.2 may add `cron_line: Option<String>` symmetric field).
7. **EDIT** `src/core/unregister.rs` — generic `<L: LaunchctlClient>` → `<S: Scheduler>`. Replace 2-step `client.bootout()` + `fs::remove_file()` with single `scheduler.unregister(&args.label) -> UnregisterReport`. `UnregisterOutput { label, plist_existed, was_loaded }` → `UnregisterOutput { label, was_registered }` (DECISION: collapse to single field — `plist_existed` and `was_loaded` were never independently asserted by callers; if Worker challenge: defend as cleaner cross-OS surface). **OR** preserve both fields backwards-compat by populating `plist_existed = was_registered`, `was_loaded = was_registered` — Worker self-decides which keeps test surface stable (see Task 7 below).
8. **EDIT** `src/core/status.rs` — generic `<L: LaunchctlClient>` → `<S: Scheduler>`. Replace `client.print(&label) -> LaunchctlPrintOutput` with `scheduler.status(&label) -> SchedulerStatus`. `print_result.not_loaded` → `status.is_registered`. `raw_stdout` → `raw_descriptor: Option<String>`. `parse_next_fire(&raw_stdout)` continues to parse macOS descriptor format from `raw_descriptor.unwrap_or_default()` — UNCHANGED semantically.
9. **EDIT** `src/mcp/tools.rs` — `let client = RealLaunchctl;` → `let scheduler = scheduler::PlatformScheduler::default();` (3 call sites: `handle_register`, `handle_unregister`, `handle_status`). Trait import path updates.
10. **EDIT** `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/status.rs` — `RealLaunchctl` → `PlatformScheduler` (1 site each). Import path `crate::launchd::RealLaunchctl` → `crate::scheduler::PlatformScheduler`.
11. **EDIT** all `tests/cli_*.rs` integration tests — no changes (spawn binary subprocess, không inject trait). Verify by grep "LaunchctlClient" hits 0 in tests/ after refactor.

**KHÔNG sửa (OUT OF SCOPE — cứng, reject creep):**

- KHÔNG impl Linux crontab logic (defer P013)
- KHÔNG thêm INV-22 (label allowlist crontab) / INV-23 (cron expression validation) (defer P014)
- KHÔNG update README — `cargo` quick-start vẫn working macOS (defer P015)
- KHÔNG đổi `src/heartbeat.rs`, `src/runner.rs`, `src/config.rs`, `src/alert.rs`, `src/core/run.rs`, `src/core/init.rs`, `src/core/config_path.rs`
- KHÔNG đổi MCP tool schema (`make_tools()` output unchanged — input/output JSON shape preserved)
- KHÔNG đổi CLI subcommand / flag / exit code mapping
- KHÔNG add new dep (`Cargo.toml [dependencies]` unchanged)
- KHÔNG đổi `HeartbeatRecord` schema, plist XML layout, launchctl wrapper semantics
- KHÔNG thêm new INV — INV-10..INV-21 trait surface preserved via `MacosScheduler` wrapping `RealLaunchctl` (boundary semantics unchanged)
- KHÔNG đổi exit codes (1/2/3/4/5/130 mapping preserved)

### Skills consulted

*(none — refactor scope, no external library question)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `LaunchctlClient` trait có 3 methods: `bootstrap(&self, plist_path: &Path) -> Result<()>`, `bootout(&self, label: &str) -> Result<()>`, `print(&self, label: &str) -> Result<LaunchctlPrintOutput>` | `Read src/launchd.rs` lines 164–183 | `[verified]` | ✅ Khớp src/launchd.rs:168, :175, :182 |
| 2 | `RealLaunchctl::bootstrap` uses `Command::new("launchctl").arg("bootstrap").arg(&domain).arg(plist_path)` — 3 discrete args, no shell (INV-10) | `Read src/launchd.rs` lines 188–207 | `[verified]` | ✅ Khớp src/launchd.rs:192–195 |
| 3 | `RealLaunchctl::print` validates label allowlist defense-in-depth (INV-17) + uses discrete args | `Read src/launchd.rs` lines 231–277 | `[verified]` | ✅ Khớp src/launchd.rs:234–242, :247–251 |
| 4 | `current_uid()` shells `id -u` + `parse::<u32>()` (INV-11) | `Read src/launchd.rs` lines 327–341 | `[verified]` | ✅ Khớp src/launchd.rs:328–340 |
| 5 | `generate_plist` validates label allowlist defense-in-depth (INV-12 second point) | `Read src/launchd.rs` lines 41–99 | `[verified]` | ✅ Khớp src/launchd.rs:49–54 |
| 6 | `NoopLaunchctl` is `pub struct` with `Default`-derived constructor (no `::new()`) | `Read src/launchd.rs` lines 283–323 | `[verified]` | ✅ Khớp src/launchd.rs:285 + tests use `NoopLaunchctl::default()` per src/launchd.rs:474 |
| 7 | `core::register::run<L: LaunchctlClient>` signature — generic param injection | `Read src/core/register.rs` line 46 | `[verified]` | ✅ Khớp src/core/register.rs:46 |
| 8 | `core::register::run` body: generates plist via `generate_plist` → writes to `plist_path_for(label, launch_agents_dir)` → calls `client.bootstrap(&plist_path)` (3 separate stages) | `Read src/core/register.rs` lines 76–90 | `[verified]` | ✅ Khớp src/core/register.rs:77–90 |
| 9 | `core::unregister::run` calls `client.bootout(&args.label)` then `fs::remove_file(&plist_path)` (2 stages) | `Read src/core/unregister.rs` lines 60–78 | `[verified]` | ✅ Khớp src/core/unregister.rs:60–78 |
| 10 | `core::status::run<L: LaunchctlClient>` calls `client.print(&label)` returning `LaunchctlPrintOutput { raw_stdout, not_loaded }` consumed by `parse_next_fire(&raw_stdout)` | `Read src/core/status.rs` lines 37–88 | `[verified]` | ✅ Khớp src/core/status.rs:37, :63–69, :72–76 |
| 11 | `src/mcp/tools.rs` has 3 sites instantiating `RealLaunchctl`: `handle_register`, `handle_unregister`, `handle_status` | `Read src/mcp/tools.rs` lines 232–350 | `[verified]` | ✅ Khớp src/mcp/tools.rs:255, :287, :338 |
| 12 | CLI shells `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/status.rs` each instantiate `RealLaunchctl` exactly once | `Read src/cli/*.rs` | `[verified]` | ✅ Khớp src/cli/register.rs:40, src/cli/unregister.rs:35, src/cli/status.rs:38 |
| 13 | Integration tests in `tests/cli_*.rs` spawn binary subprocess; NONE import `LaunchctlClient` or `NoopLaunchctl` | `Read tests/cli_register.rs + tests/cli_status.rs + tests/cli_mcp.rs` | `[verified]` | ✅ All 3 use `Command::new(BIN)` only; zero trait imports observed |
| 14 | `RegisterOutput.plist_path: PathBuf` is `pub` field, returned to CLI/MCP for stdout render | `Read src/core/register.rs` lines 26–31 | `[verified]` | ✅ Khớp src/core/register.rs:26–31 + cli/register.rs:51 prints it |
| 15 | `UnregisterOutput` has `plist_existed: bool` + `was_loaded: bool` fields used by `cli/unregister.rs` for warning render | `Read src/core/unregister.rs` lines 24–29 + src/cli/unregister.rs:44–55 | `[verified]` | ✅ Khớp — both fields read in `cli/unregister.rs:44, :49` |
| 16 | `StatusReport.plist_loaded: bool` + `next_fire: Option<String>` is `pub` — serialized to JSON in `--json` mode | `Read src/core/status.rs` lines 25–32 + src/cli/status.rs:77–82 | `[verified]` | ✅ Khớp — both fields rendered in human + JSON modes |
| 17 | `tests/cli_status.rs::status_json_mode_produces_valid_json` asserts `parsed.get("plist_loaded").is_some()` — rename = break | `Read tests/cli_status.rs` lines 122–155 | `[verified]` | ✅ Khớp tests/cli_status.rs:146 — `plist_loaded` field name LOAD-BEARING |
| 18 | `Cargo.toml` currently has NO `#[cfg(target_os = ...)]` dependencies — all deps are cross-OS | `Read Cargo.toml` | `[needs Worker verify]` | ✅ `grep -nE "^\[target\." Cargo.toml` → 0 hits. All deps are cross-OS. |
| 19 | Linux WSL2 host has `/usr/bin/crontab` per BACKLOG Decision log 2026-05-28 | docs reference | `[unverified]` | ⏳ N/A for P012 (Linux module is stub `bail!`) — verify in P013 |
| 20 | `RealLaunchctl` and `NoopLaunchctl` are unit structs (no fields beyond test-side `Mutex<Vec<...>>` recording) — can become `MacosScheduler` / `NoopScheduler` with `#[derive(Default)]` painlessly | `Read src/launchd.rs` lines 186, 283–288 | `[verified]` | ✅ Khớp — `RealLaunchctl` is `pub struct RealLaunchctl;` unit; `NoopLaunchctl` only fields are recording mutexes |
| 21 | INV-12/17 defense-in-depth label validation is INSIDE `RealLaunchctl::print` + `generate_plist` — moving these into `MacosScheduler` preserves both enforcement points (caller in `core::*` still validates as 1st point) | `Read src/launchd.rs:49–54, :234–242` + `Read src/core/register.rs:48–53`, `src/core/status.rs:55–60` | `[verified]` | ✅ Khớp — 2-point enforcement preserved automatically if move-not-rewrite |

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.**

⚠️ Anchor #18 (Cargo.toml `[cfg(target_os)]` deps) — Architect không Read Cargo.toml trong DRAFT (envelope budget). Worker MUST grep before EXECUTE.
⚠️ Anchor #19 — N/A cho P012 (Linux stub `bail!` chỉ phải compile; runtime trigger không relevant).

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker CHALLENGE (2026-05-28)

**Anchors verified:**
- Anchor #18 (`Cargo.toml` no `[target.'cfg(...)'.dependencies]`): `grep -nE "^\[target\." Cargo.toml` → `OK: 0 hits` — ✅ confirmed clean. P012 `#[cfg(target_os)]` dispatch is purely compile-time type alias, no conditional dep blocks to reconcile.
- Anchor #4 (`src/main.rs` mod declarations): actual set = `alert`, `cli`, `config`, `core`, `heartbeat`, `launchd`, `mcp`, `runner` (lines 6–13). Exactly one `mod launchd;` at line 11. Phiếu Task 4 "replace `mod launchd;` → `mod scheduler;`" is a 1-for-1 swap — ✅ correct as specified, no surprise mods.
- Cross-check `LaunchctlClient` impact grep: 19 hits across 6 files (`src/launchd.rs` ×5, `src/core/register.rs` ×6, `src/core/unregister.rs` ×3, `src/core/status.rs` ×3, `src/core/mod.rs` ×1, `src/core/run.rs` ×1) — Architect estimated ~18 across 6 files; actual = 19, delta = +1 within ±2 tolerance. ✅ No block. The extra hit is a comment string in `core/register.rs` test assertions ("LaunchctlClient invocation" in 3 assert messages) — Tầng 2 cosmetic, Worker updates in EXECUTE.

**Decision point review:**

- DP1 (RegisterIntent `{ hour, minute }` vs raw cron string): APPROVE. `CrontabScheduler::register` on Linux (P013) receives `hour`/`minute` and trivially reconstructs `"<minute> <hour> * * *"` — no friction for Phase 3.2. The `cron_expr: Option<String>` defer path is a sound escape hatch if P013 needs full 5-field passthrough. Trait surface is not prematurely narrow.
- DP2 (`plist_loaded` field name kept): APPROVE. Anchor #17 confirms `tests/cli_status.rs:146` is load-bearing. JSON schema stability for MCP consumers is a real constraint. Rename = separate phiếu. Keeping is the correct minimal-risk choice.
- DP3 (`UnregisterOutput` dual field `plist_existed` + `was_loaded` both populated from `was_registered`): APPROVE. `NoopScheduler::unregister` returns `was_registered: false`, so `test::run_idempotent_when_plist_absent` assertion `!output.plist_existed` still holds. Minimum-disruption justified.
- DP4 (`RegisterOutput.plist_path: PathBuf` empty on Linux via `unwrap_or_default()`): APPROVE WITH NOTE. On Linux P012 the stub `bail!`s before `MacosScheduler::register` ever returns, so the empty `PathBuf` is never rendered to CLI in this phiếu. Note for Discovery: P013 must either (a) populate `plist_path` with a crontab-equivalent descriptor or (b) gate the CLI render path on `!plist_path.as_os_str().is_empty()` — otherwise Linux users of P013 see "Plist path: " with blank output. Not a P012 blocker but must surface in P013 phiếu acceptance criteria.
- DP5 (`generate_plist_from_intent` adapter — synthetic Config, no rewrite): APPROVE. Mechanical-move discipline is correct for a pure refactor phiếu. Adapter keeps all INV-10/12/13 enforcement points inside `generate_plist` body unchanged. A direct rewrite would require re-verifying INV coverage — unnecessary risk for P012.
- DP6 (`parse_simple_cron` → `parse_daily_cron`, inlined in `core::register`): APPROVE. Config-domain parsing (TOML `ScheduleConfig::Cron` → `(u8, u8)`) belongs in `core::register`, not in the scheduler abstraction layer. The rename clarifies the daily-only constraint. Phase 3.2 P013 can add `parse_cron_next_fire` independently without coupling to this helper.

**Out-of-decision-point objections:**

- [Tầng 2, no block] `src/core/register.rs` lines 184, 210, 236: test assertion strings say "pre-flight rejection must occur before LaunchctlClient invocation" — after refactor these strings refer to the old type name. No user-facing impact (test output only). Worker self-resolves during EXECUTE: update to "before Scheduler invocation".
- [Tầng 2, no block] `src/core/mod.rs` line 9 and `src/core/run.rs` line 51: doc comments reference `LaunchctlClient` — cosmetic, Worker updates during EXECUTE.
- [Observation, not objection] `src/scheduler/linux.rs` module is correctly `#[cfg(target_os = "linux")]` gated at declaration in `mod.rs` — meaning on macOS host the file is not compiled. The 3 stub tests in `linux.rs` therefore only run on Linux CI. This is correct behavior per Constraint #10.

**Verdict:** APPROVE_AS_IS — no Tầng 1 objections. All anchors verified ✅. Decision points DP1–DP6 defensible without Architect response needed.

**Estimated EXECUTE effort vs Architect's ~350 LOC:** agree. Mechanical move + adapter shim is the bulk; the 9 edit sites are small. No hidden scope discovered.

### Final consensus
- Phiếu version: V1
- Total turns: 1 (Worker accepted — no Architect response needed)
- Approved: 2026-05-28 — code execution may begin

---

## Debug Log

> Worker emit observability records during EXECUTE. Append-only.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command snippet>
```

---

## Verification Trace — Sub-mechanism A-E checks

> Worker MUST run applicable checks BEFORE marking phiếu DONE.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger gap) | N/A (refactor, không add cron/hook) | — | — | N/A |
| B (capability) | `cargo check` (host = Linux WSL2) | exit 0, 0 errors | | |
| B (capability) | `cargo check --target x86_64-apple-darwin` | exit 0 (or skip if cross-target toolchain absent — CI matrix covers) | | |
| B (capability) | `cargo test --all` (Linux host) | all pass; macOS-only tests gated by `#[cfg(target_os = "macos")]` | | |
| B (capability) | `cargo test -p advisory-cron --lib scheduler::macos` (if macOS available) | targeted scheduler tests pass | | |
| C (migration completeness) | `grep -rn "LaunchctlClient" src/ tests/` BEFORE refactor | ~18 hits across `src/launchd.rs` + `src/core/register.rs` + `src/core/unregister.rs` + `src/core/status.rs` + `src/mcp/tools.rs` + `src/cli/{register,unregister,status}.rs` | | |
| C (migration completeness) | `grep -rn "LaunchctlClient" src/ tests/` AFTER refactor | 0 hits (trait fully renamed) | | |
| C (migration completeness) | `grep -rn "RealLaunchctl\|NoopLaunchctl" src/ tests/` AFTER refactor | 0 hits in public surface (both moved to `MacosScheduler` / `NoopScheduler`; `RealLaunchctl` may remain PRIVATE inside `scheduler::macos`) | | |
| C (migration completeness) | `grep -rn "src/launchd" .` AFTER refactor | 0 hits (file deleted, mod decl removed) | | |
| D (persistence lifecycle) | N/A (no doctrine file rotation) | — | — | N/A |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | | |
| E (env drift) | `cargo build --release` từ clean `target/` (Linux host) | exit 0, zero warnings, binary ≤7MB | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verify (Sub-mechanism B/C/E)

**Mục đích:** Verify Architect's assumptions about source structure BEFORE touching code. Run all of:

1. `grep -rn "LaunchctlClient" src/ tests/` — count baseline hits (Architect estimates ~18, post-refactor target = 0).
2. `grep -rn "RealLaunchctl\|NoopLaunchctl" src/ tests/` — list call sites. Confirm only the files Architect listed in Scope (core::*, cli::*, mcp::tools, launchd.rs unit tests).
3. `grep -n "\[dependencies\]" Cargo.toml` + `grep -A20 "\[dependencies\]" Cargo.toml` — confirm zero `[target.'cfg(...)'.dependencies]` blocks (anchor #18 verify). If non-zero → DISCOVERY_REPORT before proceeding.
4. `cargo check` baseline — confirm 0 errors, 0 warnings on `main` BEFORE refactor.
5. `cargo test --all` baseline count — record total test count (BACKLOG.md says 144). Post-refactor must match (no test deletion expected).

**Nếu bất kỳ check nào fail expected:** STOP, escalate via AskUserQuestion + DISCOVERY_REPORT.

### Task 1 — CREATE `src/scheduler/mod.rs`

**File:** `src/scheduler/mod.rs` (NEW)

**Nội dung (Architect proposed surface — Worker may refine signatures during CHALLENGE):**

```rust
//! Phase 3.1 — Cross-OS scheduler abstraction.
//!
//! Replaces Phase 1.3 `src/launchd.rs::LaunchctlClient`. macOS impl in `macos.rs`
//! (launchd via launchctl); Linux impl in `linux.rs` (crontab — stub P012, real P013).
//!
//! Compile-time dispatch: `PlatformScheduler` alias resolves to `MacosScheduler` on
//! macOS targets, `CrontabScheduler` on Linux. Other OSes do not compile (Phase 3 = macOS + Linux only).

use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// High-level intent passed to `Scheduler::register`. Abstracts plist-vs-crontab.
#[derive(Debug, Clone)]
pub struct RegisterIntent {
    /// Bare label (full launchd label / crontab tag = `com.advisorycron.<label>` / `# advisory-cron: <label>`).
    pub label: String,
    /// Hour 0..=23 + minute 0..=59 — daily form only (Phase 1 / Phase 3.1 constraint).
    /// Phase 3.2 P013 may extend with full cron expression for Linux only; macOS stays daily.
    pub hour: u8,
    pub minute: u8,
    /// Absolute path to `advisory-cron` binary (resolved by core via `env::current_exe()`).
    pub self_exe: PathBuf,
    /// Working directory for the fired task.
    pub working_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RegisterReport {
    /// macOS: path to written plist file. Linux (P013): None. Surfaced for CLI render.
    pub plist_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UnregisterReport {
    /// True if the scheduler had a registration matching `label` before this call.
    /// macOS: true if launchctl had the job loaded OR the plist file existed.
    /// Linux (P013): true if a tagged crontab line existed.
    pub was_registered: bool,
}

#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    /// True if the scheduler currently has a registration for `label`.
    pub is_registered: bool,
    /// Raw scheduler-specific descriptor for downstream parsing.
    /// macOS: `launchctl print` stdout (parsed by `core::status::parse_next_fire`).
    /// Linux (P013): matched crontab line (parsed by future `parse_cron_next_fire`).
    pub raw_descriptor: Option<String>,
}

/// Cross-OS scheduling abstraction. macOS = launchd; Linux = crontab.
pub trait Scheduler {
    /// Register a recurring task. Idempotent on re-register (overwrites existing registration).
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport>;

    /// Unregister by label. Idempotent: returns `was_registered=false` if no prior registration.
    fn unregister(&self, label: &str) -> Result<UnregisterReport>;

    /// Query registration state + raw descriptor for next-fire parsing.
    fn status(&self, label: &str) -> Result<SchedulerStatus>;
}

#[cfg(target_os = "macos")]
pub use macos::MacosScheduler as PlatformScheduler;

#[cfg(target_os = "linux")]
pub use linux::CrontabScheduler as PlatformScheduler;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("advisory-cron Phase 3 supports macOS + Linux only");

// ---- NoopScheduler (test impl — replaces NoopLaunchctl) ----

/// Test impl that records calls. Used by `core::*::tests` + (future) lib tests.
/// `pub` to allow integration test crate to import directly.
#[derive(Debug, Default)]
pub struct NoopScheduler {
    pub register_calls: std::sync::Mutex<Vec<RegisterIntent>>,
    pub unregister_calls: std::sync::Mutex<Vec<String>>,
    pub status_calls: std::sync::Mutex<Vec<String>>,
}

impl Scheduler for NoopScheduler {
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport> {
        self.register_calls.lock().unwrap().push(intent.clone());
        Ok(RegisterReport { plist_path: None })
    }

    fn unregister(&self, label: &str) -> Result<UnregisterReport> {
        self.unregister_calls.lock().unwrap().push(label.to_string());
        Ok(UnregisterReport { was_registered: false })
    }

    fn status(&self, label: &str) -> Result<SchedulerStatus> {
        self.status_calls.lock().unwrap().push(label.to_string());
        // Canned descriptor matches macOS 15 launchctl format (preserve test compat).
        Ok(SchedulerStatus {
            is_registered: true,
            raw_descriptor: Some(
                "descriptor = {\n\t\"Minute\" => 0\n\t\"Hour\" => 9\n}".to_string(),
            ),
        })
    }
}
```

**Lưu ý:**

- `compile_error!` ensures Windows / FreeBSD targets fail loud at compile time (per BACKLOG Decision log: Phase 3 = macOS + Linux only).
- `NoopScheduler::status` returns `raw_descriptor` containing the macOS descriptor format — this preserves `core::status::tests::parse_next_fire_*` test compatibility (those tests parse this format).
- `RegisterIntent` carries `hour`/`minute` (already-resolved) — `core::register::run` resolves `ScheduleConfig::Cron`/`Calendar` enum → `(u8, u8)` BEFORE building intent. This pushes `parse_simple_cron` responsibility OUT of scheduler trait (it's domain logic, not scheduler logic).
- Worker may push back on `hour`/`minute` form — if Phase 3.2 (P013) Linux wants raw 5-field cron, this becomes friction. Architect's call: P013 can ADD optional `cron_expr: Option<String>` field at that time; P012 doesn't predict P013's exact needs.

### Task 2 — CREATE `src/scheduler/macos.rs`

**File:** `src/scheduler/macos.rs` (NEW — content moved verbatim from `src/launchd.rs`)

**Tìm:** All content of `src/launchd.rs` lines 1–519 (module doc, all pub fns, traits, impls, tests).

**Thay bằng / Thêm:** Same content, moved to `src/scheduler/macos.rs`, with:

1. **Re-scope visibility** — `RealLaunchctl`, `LaunchctlClient`, `LaunchctlPrintOutput`, `NoopLaunchctl`, `current_uid`, `generate_plist`, `plist_path_for`, `default_launch_agents_dir`, `xml_escape`, `parse_simple_cron` ALL become `pub(super)` or `pub(crate)` as needed by `MacosScheduler` impl only. **Exception:** `RealLaunchctl` becomes `pub(self)` (private to this file) — only `MacosScheduler` uses it.
2. **ADD new `MacosScheduler` struct + impl Scheduler:**

```rust
use super::{RegisterIntent, RegisterReport, Scheduler, SchedulerStatus, UnregisterReport};

#[derive(Debug, Default)]
pub struct MacosScheduler;

impl Scheduler for MacosScheduler {
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport> {
        use crate::core::config_path::home_dir;
        let home = home_dir().context("failed to resolve $HOME")?;
        let launch_agents_dir = default_launch_agents_dir(&home);
        fs::create_dir_all(&launch_agents_dir)
            .with_context(|| format!("failed to create {}", launch_agents_dir.display()))?;

        // Generate plist XML — moved from core::register::run.
        let plist_xml = generate_plist_from_intent(intent)?;

        let plist_path = plist_path_for(&intent.label, &launch_agents_dir);
        fs::write(&plist_path, &plist_xml)
            .with_context(|| format!("failed to write plist to {}", plist_path.display()))?;

        RealLaunchctl.bootstrap(&plist_path)
            .context("launchctl bootstrap failed")?;

        Ok(RegisterReport { plist_path: Some(plist_path) })
    }

    fn unregister(&self, label: &str) -> Result<UnregisterReport> {
        // Defense-in-depth label validation (INV-12).
        if !is_valid_label_inline(label) {
            anyhow::bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }
        use crate::core::config_path::home_dir;
        let home = home_dir().context("failed to resolve $HOME")?;
        let launch_agents_dir = default_launch_agents_dir(&home);
        let plist_path = plist_path_for(label, &launch_agents_dir);
        let plist_existed = plist_path.exists();

        let was_loaded = RealLaunchctl.bootout(label).is_ok();

        match fs::remove_file(&plist_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| {
                format!("failed to remove plist at {}", plist_path.display())
            }),
        }
        Ok(UnregisterReport { was_registered: plist_existed || was_loaded })
    }

    fn status(&self, label: &str) -> Result<SchedulerStatus> {
        let print_out = RealLaunchctl.print(label)?;
        Ok(SchedulerStatus {
            is_registered: !print_out.not_loaded,
            raw_descriptor: if print_out.not_loaded { None } else { Some(print_out.raw_stdout) },
        })
    }
}

/// Adapter — bridges Phase 1 `generate_plist(config, label, self_exe)` to Phase 3 intent shape.
/// Builds a synthetic `Config` for the plist generator (which currently reads `config.task.working_dir`
/// + `config.schedule`). Architect chose this adapter shape to AVOID rewriting `generate_plist` body —
/// keeps the move strictly mechanical. Worker MAY refactor `generate_plist` to take intent directly
/// (eliminates synthetic Config) — see Constraint #6.
fn generate_plist_from_intent(intent: &RegisterIntent) -> Result<String> {
    use crate::config::{Config, HeartbeatConfig, ScheduleConfig, TaskConfig};
    let synthetic = Config {
        task: TaskConfig {
            command: String::new(),  // unused by generate_plist
            args: Vec::new(),         // unused
            working_dir: intent.working_dir.clone(),
            label: None,
        },
        schedule: ScheduleConfig::Calendar { hour: intent.hour, minute: intent.minute },
        heartbeat: HeartbeatConfig { log_path: std::path::PathBuf::new() }, // unused
        alert: None,
        retry: None,
    };
    generate_plist(&synthetic, &intent.label, &intent.self_exe)
}

/// Inline INV-12 check — keeps validation local to scheduler::macos defense-in-depth.
fn is_valid_label_inline(label: &str) -> bool {
    !label.is_empty()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
```

**Lưu ý:**

- `RealLaunchctl` stays as the underlying launchctl shell-out wrapper — `MacosScheduler` calls into it. This preserves INV-10/11/12/13/17 enforcement points without rewrite.
- `generate_plist_from_intent` adapter shape — Architect chose mechanical-move over rewriting `generate_plist` to take intent directly. Worker may push back and propose direct rewrite if cleaner; Architect leans adapter for refactor-discipline. The synthetic Config is local to this file, dead-on-read for unused fields.
- All `#[cfg(test)] mod tests { ... }` in `src/launchd.rs` (lines 343–519) move alongside. Tests references like `NoopLaunchctl::default()` → unchanged INSIDE `scheduler::macos::tests` (still uses the renamed-to-private `NoopLaunchctl`). Add 2–3 new `#[cfg(test)]` tests for `MacosScheduler::{register,unregister,status}` round-trip using `TempDir HOME` + checking plist file existence + delegating to underlying `RealLaunchctl` (which already has tests).
- `pub(self) struct RealLaunchctl;` — file-private, only `MacosScheduler` uses it. `NoopLaunchctl` may stay `pub(super)` so unit tests in same file can use it (or remove entirely if `NoopScheduler` in mod.rs covers all test cases — Worker self-decides).

### Task 3 — CREATE `src/scheduler/linux.rs` (STUB)

**File:** `src/scheduler/linux.rs` (NEW)

**Nội dung:**

```rust
//! Phase 3.2 — Linux crontab scheduler.
//!
//! P012 ships this as STUB returning `bail!` for every method.
//! Real implementation lands in P013 (Phase 3.2):
//!   - `register`: `crontab -l` → parse → append `<cron_expr> <self_exe> run --config <path> # advisory-cron: <label>` → `crontab -` pipe back
//!   - `unregister`: same flow, filter out tag line
//!   - `status`: grep tag from `crontab -l`

use anyhow::{Result, bail};

use super::{RegisterIntent, RegisterReport, Scheduler, SchedulerStatus, UnregisterReport};

#[derive(Debug, Default)]
pub struct CrontabScheduler;

impl Scheduler for CrontabScheduler {
    fn register(&self, _intent: &RegisterIntent) -> Result<RegisterReport> {
        bail!("CrontabScheduler::register — Phase 3.2 (P013) chưa ship")
    }

    fn unregister(&self, _label: &str) -> Result<UnregisterReport> {
        bail!("CrontabScheduler::unregister — Phase 3.2 (P013) chưa ship")
    }

    fn status(&self, _label: &str) -> Result<SchedulerStatus> {
        bail!("CrontabScheduler::status — Phase 3.2 (P013) chưa ship")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_register_bails_with_p013_message() {
        let s = CrontabScheduler;
        let intent = RegisterIntent {
            label: "test".into(),
            hour: 9,
            minute: 0,
            self_exe: std::path::PathBuf::from("/bin/x"),
            working_dir: std::path::PathBuf::from("/tmp"),
        };
        let err = s.register(&intent).unwrap_err();
        assert!(format!("{err:#}").contains("P013"));
    }

    #[test]
    fn stub_unregister_bails_with_p013_message() {
        let s = CrontabScheduler;
        let err = s.unregister("test").unwrap_err();
        assert!(format!("{err:#}").contains("P013"));
    }

    #[test]
    fn stub_status_bails_with_p013_message() {
        let s = CrontabScheduler;
        let err = s.status("test").unwrap_err();
        assert!(format!("{err:#}").contains("P013"));
    }
}
```

**Lưu ý:**

- Module gated `#[cfg(target_os = "linux")]` at `mod.rs` level — file only compiles on Linux targets. On macOS, file exists but compile path skips it.
- Tests inside `linux.rs` also gated by virtue of module gate — only run on Linux CI job.
- The 3 stub tests guarantee Linux binary compiles + the trait impl is wired (not just file-present). Required for Sub-mech B Linux check.

### Task 4 — DELETE `src/launchd.rs` + update `src/main.rs` mod decl

**File:** `src/main.rs`

**Tìm:**
```rust
mod launchd;
```

**Thay bằng:**
```rust
mod scheduler;
```

**Lưu ý:** `mod launchd;` may not be the only line — Worker greps `mod ` in `src/main.rs` to confirm exact set of mod declarations. Architect did not Read `src/main.rs` (envelope budget) — `[needs Worker verify]`.

After this edit, run `git rm src/launchd.rs` (file deletion tracked).

### Task 5 — EDIT `src/core/register.rs`

**File:** `src/core/register.rs`

**Tìm:** Lines 10–14 imports + line 46 generic signature + lines 76–90 plist generation body.

**Thay bằng:**

1. **Imports** — remove `crate::launchd::*`, add:
```rust
use crate::scheduler::{RegisterIntent, Scheduler};
```

2. **Signature** — line 46:
```rust
pub fn run<S: Scheduler>(args: RegisterArgs, scheduler: &S) -> Result<RegisterOutput> {
```

3. **Body, lines 73–90** — REPLACE the plist generation + write + bootstrap 3-step with intent build + scheduler call:
```rust
    // Resolve hour/minute from config.schedule.
    let (hour, minute) = match &config.schedule {
        crate::config::ScheduleConfig::Calendar { hour, minute } => (*hour, *minute),
        crate::config::ScheduleConfig::Cron { cron } => {
            // Inline simple-cron parse (was in launchd::parse_simple_cron — Worker decides
            // whether to keep cron parse in core::register or pass cron string to scheduler).
            // ARCHITECT NOTE: Phase 3.2 may want cron string passthrough for Linux (full 5-field).
            // For P012, parse here to keep Scheduler trait surface (hour, minute) — see Constraint #6.
            parse_daily_cron(cron)?
        }
    };

    let self_exe = std::env::current_exe()
        .context("failed to resolve current executable path")?;

    let intent = RegisterIntent {
        label: args.label.clone(),
        hour,
        minute,
        self_exe,
        working_dir: config.task.working_dir.clone(),
    };

    let report = scheduler
        .register(&intent)
        .context("scheduler register failed")?;

    Ok(RegisterOutput {
        plist_path: report.plist_path.unwrap_or_default(),
        label: args.label,
        bootstrapped: true,
    })
}

// Inline copy of launchd::parse_simple_cron — Worker MAY relocate to a shared module
// if it ends up needed by other call sites. For P012 mechanical refactor, inline here.
fn parse_daily_cron(expr: &str) -> Result<(u8, u8)> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        anyhow::bail!("cron expression must be 5 fields (got {}): {expr:?}", parts.len());
    }
    if parts[2] != "*" || parts[3] != "*" || parts[4] != "*" {
        anyhow::bail!("Phase 3.1: launchd cron support requires day/month/dow all `*` (daily fire)");
    }
    let minute: u8 = parts[0].parse()
        .with_context(|| format!("cron minute must be numeric (got {:?})", parts[0]))?;
    let hour: u8 = parts[1].parse()
        .with_context(|| format!("cron hour must be numeric (got {:?})", parts[1]))?;
    if hour > 23 || minute > 59 {
        anyhow::bail!("cron hour must be 0..=23 and minute 0..=59 (got hour={hour} minute={minute})");
    }
    Ok((hour, minute))
}
```

4. **Tests** (lines 100–238) — update `NoopLaunchctl` → `crate::scheduler::NoopScheduler`. Existing 5 tests preserved semantically; assertions on `client.bootstrap_calls.lock().unwrap().len()` → `scheduler.register_calls.lock().unwrap().len()`. New tests for pre-flight INV-12 rejection assertions are unchanged (still reject before scheduler call).

5. **Field `RegisterOutput.plist_path`** — preserved (CLI consumer in `cli/register.rs:51` prints it). When Linux ships (P013), `plist_path` will be empty `PathBuf` from `unwrap_or_default()`. CLI render must tolerate empty path — verify in P013, not here. Note: Worker MAY propose changing `plist_path: PathBuf` to `Option<PathBuf>` for clarity but that's API-breaking → Tầng 1 add'l docs → defer to follow-up. For P012: keep `PathBuf` + `unwrap_or_default()`. Acceptable.

**Lưu ý:**

- `parse_simple_cron` was in `src/launchd.rs:105–141`. Architect chose to **inline a renamed copy `parse_daily_cron` in `core::register`** because (a) it's domain-level (TOML config → calendar form), not scheduler-level (b) Phase 3.2 P013 Linux will want raw cron string passthrough — different parsing needs. Worker may push back: alternative = keep `parse_simple_cron` inside `scheduler::macos` and call from core (needs `pub` export). Architect's lean: inline in core for cleaner separation. Worker self-decides during EXECUTE.
- Pre-flight INV-12 validation (lines 47–53) unchanged — first enforcement point preserved.

### Task 6 — EDIT `src/core/unregister.rs`

**File:** `src/core/unregister.rs`

**Tìm:** Line 11 import + line 42 generic signature + lines 56–84 body.

**Thay bằng:**

1. **Imports** — remove `crate::launchd::*`, add:
```rust
use crate::scheduler::Scheduler;
```

2. **Signature** — line 42:
```rust
pub fn run<S: Scheduler>(args: UnregisterArgs, scheduler: &S) -> Result<UnregisterOutput> {
```

3. **Body** — REPLACE the 2-step `client.bootout()` + `fs::remove_file()` with single `scheduler.unregister()`:
```rust
    // 1. Validate label (INV-12 pre-flight) — unchanged.
    // ... existing validation ...

    // 2. Delegate to scheduler.
    let report = scheduler.unregister(&args.label)
        .context("scheduler unregister failed")?;

    Ok(UnregisterOutput {
        label: args.label,
        // Phase 3.1: collapse `plist_existed` + `was_loaded` → `was_registered`.
        // Backwards-compat: populate both old fields from `was_registered` so CLI render
        // (cli/unregister.rs:44, :49 — warning messages) still triggers.
        plist_existed: report.was_registered,
        was_loaded: report.was_registered,
    })
```

4. **Tests** (lines 87–128) — update `NoopLaunchctl::default()` → `NoopScheduler::default()`. Existing assertions preserved. Note: `NoopScheduler::unregister` currently returns `was_registered: false` — `test::run_idempotent_when_plist_absent` asserts `!output.plist_existed` which now maps from `was_registered=false` → ✅ stays true. Backwards-compat preserved.

**Lưu ý:**

- DECISION: `UnregisterOutput` keeps BOTH `plist_existed` + `was_loaded` fields, populated identically from `was_registered`. Reason: not breaking the public `pub` struct shape (serde-derive serializes both fields — MCP JSON output stable). Worker may challenge if cleaner to break — Architect defends as **minimum-disruption refactor** (P013 can collapse later if Sếp wants).
- Pre-flight INV-12 validation (lines 44–49) unchanged.

### Task 7 — EDIT `src/core/status.rs`

**File:** `src/core/status.rs`

**Tìm:** Lines 11 imports + line 37 generic signature + lines 63–76 body around `client.print()`.

**Thay bằng:**

1. **Imports**:
```rust
use crate::scheduler::Scheduler;
```
(remove `crate::launchd::{LaunchctlClient, LaunchctlPrintOutput}`)

2. **Signature** — line 37:
```rust
pub fn run<S: Scheduler>(args: StatusArgs, scheduler: &S) -> Result<StatusReport> {
```

3. **Body, lines 62–76** — REPLACE:
```rust
    // 5. Query scheduler.
    let status = match scheduler.status(&label) {
        Ok(s) => s,
        Err(_err) => crate::scheduler::SchedulerStatus {
            is_registered: false,
            raw_descriptor: None,
        },
    };

    // 6. Parse next-fire schedule (macOS descriptor format).
    let next_fire = if status.is_registered {
        status.raw_descriptor.as_deref().and_then(parse_next_fire)
    } else {
        None
    };

    // 7. Read recent heartbeats. (unchanged)
    let last_runs = ...;

    Ok(StatusReport {
        label,
        plist_loaded: status.is_registered,  // field name preserved — see Note below
        next_fire,
        heartbeat_log_path: config.heartbeat.log_path.display().to_string(),
        last_runs,
    })
```

4. **`parse_next_fire(s: &str) -> Option<String>`** signature unchanged — Worker should confirm function still takes `&str`. Current returns `Option<String>` (line 107). Tests (lines 183–222) parse macOS descriptor format — UNCHANGED, still work because `NoopScheduler::status` returns descriptor format-compatible string.

5. **Tests** (lines 147–222) — `is_valid_label` tests unchanged. `parse_next_fire` tests unchanged. No `NoopLaunchctl` usage in this file's tests (tests/cli_status.rs is integration — spawn binary).

**Lưu ý:**

- **CRITICAL field name preservation:** `StatusReport.plist_loaded: bool` is a `pub` field, serialized to JSON in `--json` mode. `tests/cli_status.rs:146` asserts `parsed.get("plist_loaded").is_some()` (verified anchor #17). **Renaming would break the test + change MCP/CLI JSON output schema = Tầng 1 break.** Architect KEEPS the name `plist_loaded` even though it's macOS-flavored — it maps to `is_registered` semantically. Phase 3.2 P013 may add a deprecation note in CHANGELOG; rename later if Sếp wants.
- Worker may challenge as "anti-pattern leaving macOS-flavor name in cross-OS struct." Architect's DEFEND: backwards-compat JSON schema preservation outweighs naming purity. Test breakage = Tier-1 break. Renaming is its own follow-up phiếu.
- `parse_next_fire(s: &str)` accepts `&str` now (anchor #10 verified `parse_next_fire(&print_result.raw_stdout)` at status.rs:75). New body passes `&str` via `status.raw_descriptor.as_deref()` — Worker verifies signature unchanged.

### Task 8 — EDIT `src/mcp/tools.rs`

**File:** `src/mcp/tools.rs`

**Tìm:** Lines 19 import + lines 255, 287, 338 (3 call sites of `RealLaunchctl`).

**Thay bằng:**

1. **Import**:
```rust
use crate::scheduler::PlatformScheduler;
```
(remove `crate::launchd::RealLaunchctl`)

2. **3 sites** (handle_register, handle_unregister, handle_status):
```rust
let scheduler = PlatformScheduler::default();
match core::register::run(..., &scheduler) { ... }
```
(replace `let client = RealLaunchctl;` + `&client`)

**Lưu ý:**

- MCP tool schema (`make_tools()` output JSON, lines 75–169) UNCHANGED. Input/output JSON shape stable. INV-18 boundary unchanged.
- All `#[cfg(test)]` tests inside `src/mcp/tools.rs` (lines 352–413) currently don't call `core::*::run` with a scheduler — they test `validate_label`/`validate_config_path`/`make_tools`/`handle_init` (which doesn't take scheduler). Zero changes needed in tests.

### Task 9 — EDIT `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/status.rs`

**3 Files. Same pattern each:**

**Tìm:** Import `crate::launchd::RealLaunchctl` + line instantiating `let client = RealLaunchctl;`.

**Thay bằng:**
- Import: `use crate::scheduler::PlatformScheduler;`
- Body: `let scheduler = PlatformScheduler::default(); ... core_run(..., &scheduler) ...`

**Lưu ý:**

- Per-file exact line numbers verified in anchor #12:
  - `src/cli/register.rs:40` — `let client = RealLaunchctl;`
  - `src/cli/unregister.rs:35` — `let client = RealLaunchctl;`
  - `src/cli/status.rs:38` — `let client = RealLaunchctl;`
- Exit code mapping (lines 58–69 register, 59–67 unregister, 57–66 status) UNCHANGED. Error string matching (e.g. `msg.contains("invalid label")`) still works — `MacosScheduler` errors contain same substrings as `RealLaunchctl` errors did.

### Task 10 — VERIFY integration tests still pass (NO source edits)

**Files (verify-only):** `tests/cli_register.rs`, `tests/cli_unregister.rs` (if exists), `tests/cli_status.rs`, `tests/cli_mcp.rs`, `tests/cli_run.rs`, `tests/cli_run_alert.rs`, `tests/cli_run_retry.rs`, `tests/cli_init.rs`, `tests/cli_help.rs`.

**Verify (grep):**
1. `grep -rn "LaunchctlClient\|NoopLaunchctl\|RealLaunchctl" tests/` → expect 0 hits (per anchor #13 baseline = 0, post-refactor still 0).
2. Run `cargo test --test cli_register --test cli_status --test cli_mcp` — all pass on macOS host (Linux host: macOS-flagged tests skip; Linux pass count = subset).

**Lưu ý:**

- Integration tests spawn the binary subprocess — Scheduler trait change is invisible to them. If a test breaks, ROOT CAUSE is exit code / stdout format drift, not trait refactor — that would be a bug introduced by Worker, not a phiếu defect.

---

## Files cần sửa

| File | Thay đổi | Task |
|------|---------|------|
| `src/scheduler/mod.rs` | **CREATE** — Scheduler trait + types + NoopScheduler + PlatformScheduler alias | Task 1 |
| `src/scheduler/macos.rs` | **CREATE** — move all content from `src/launchd.rs` + add MacosScheduler impl | Task 2 |
| `src/scheduler/linux.rs` | **CREATE** — stub `CrontabScheduler` bailing "P013 chưa ship" | Task 3 |
| `src/launchd.rs` | **DELETE** (git rm) | Task 4 |
| `src/main.rs` | Replace `mod launchd;` → `mod scheduler;` | Task 4 |
| `src/core/register.rs` | Generic `<L: LaunchctlClient>` → `<S: Scheduler>`; body builds `RegisterIntent` + delegates; inline `parse_daily_cron` helper | Task 5 |
| `src/core/unregister.rs` | Generic switch; body delegates to `scheduler.unregister()`; `UnregisterOutput` field-compat (populate both `plist_existed`+`was_loaded` from `was_registered`) | Task 6 |
| `src/core/status.rs` | Generic switch; body delegates to `scheduler.status()`; preserve `StatusReport.plist_loaded` field name (JSON schema stable) | Task 7 |
| `src/mcp/tools.rs` | 3 sites: `RealLaunchctl` → `PlatformScheduler::default()` | Task 8 |
| `src/cli/register.rs` | Import + 1 site: `RealLaunchctl` → `PlatformScheduler::default()` | Task 9 |
| `src/cli/unregister.rs` | Import + 1 site: same | Task 9 |
| `src/cli/status.rs` | Import + 1 site: same | Task 9 |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/heartbeat.rs` | Untouched — no scheduler coupling |
| `src/runner.rs` | Untouched — no scheduler coupling |
| `src/config.rs` | Untouched — TOML schema stable |
| `src/alert.rs` | Untouched — no scheduler coupling |
| `src/core/run.rs` | Untouched — runs configured task, no register/unregister/status |
| `src/core/init.rs` | Untouched |
| `src/core/config_path.rs` | Untouched |
| `src/cli/run.rs` | Untouched |
| `src/cli/init.rs` | Untouched |
| `src/cli/mcp.rs` | Untouched — thin shell over `mcp::server::serve_stdio()` |
| `src/mcp/server.rs` | Untouched |
| `src/mcp/mod.rs` | Untouched |
| `tests/cli_*.rs` (all 8 integration files) | Verify zero `LaunchctlClient`/`NoopLaunchctl`/`RealLaunchctl` references after refactor (per anchor #13 baseline = 0) |
| `Cargo.toml` | Verify zero new `[dependencies]` added; zero `[target.'cfg(...)']` blocks (per anchor #18 needs verify) |

---

## Luật chơi (Constraints)

1. **No new dep.** `Cargo.toml [dependencies]` MUST NOT gain any new line. If Worker thinks a new dep helps (e.g. `cfg-if`, `cron-parser`), STOP + escalate via AskUserQuestion (Hard Stop #2).
2. **No CLI surface change.** Exit codes 0/1/2/3/4/5/130 mapping unchanged. Subcommand names, flag names unchanged. STDOUT/STDERR human render unchanged.
3. **No MCP schema change.** `make_tools()` input/output JSON shape preserved (verified by `tests/cli_mcp.rs::mcp_handshake_and_tools_list_returns_5_tools`).
4. **No `unsafe` block.** Worker MUST NOT introduce `unsafe { }` (INV-6). Existing `unsafe { std::env::set_var(...) }` blocks in `#[cfg(test)]` modules MAY remain (test-only, pre-existing pattern Phase 1+2).
5. **No security boundary semantics change.** INV-10/11/12/13/17 all enforcement points preserved by moving — not rewriting — the existing `RealLaunchctl` + `generate_plist` + `current_uid` bodies. If Worker rewrites any INV-relevant code path, MUST flag in Discovery Report under "INV preservation check".
6. **Cron parse location decision** (Task 1 + Task 5 surface): `parse_simple_cron` / `parse_daily_cron` moves OUT of scheduler trait. Architect's call: live in `core::register` (domain logic). Worker may push back during CHALLENGE with alternative (keep in `scheduler::macos`, pass cron string through trait). If Worker disagrees: propose, Architect responds; default = inline in core.
7. **`StatusReport.plist_loaded` field name preservation** (Task 7): field name kept verbatim even though semantically should be `is_registered`. Reason: JSON schema stability for MCP + `tests/cli_status.rs:146` assertion. If Worker pushes back: defend; renaming = separate phiếu.
8. **`UnregisterOutput` two-field preservation** (Task 6): keep both `plist_existed` + `was_loaded` populated from single `was_registered`. Reason: minimum-disruption refactor.
9. **Zero behavior change macOS.** All Phase 1+2 acceptance criteria still hold post-refactor:
   - `advisory-cron register --label foo` writes plist + bootstraps (verifiable: `~/Library/LaunchAgents/com.advisorycron.foo.plist` exists).
   - `advisory-cron unregister --label foo` removes plist + bootouts (idempotent).
   - `advisory-cron status --label foo` parses descriptor → "daily at HH:MM".
   - `advisory-cron mcp` 5 tools + handshake works.
10. **Linux Sub-mech B check is `cargo check` only** — Linux stub `bail!` means NO runtime test on Linux. Sếp doesn't dogfood Linux until P013 ships.
11. **Module add (Tầng 1 trigger) → ARCHITECTURE.md §Modules table update required** (docs-gate). Add 3 rows: `src/scheduler/mod.rs`, `src/scheduler/macos.rs`, `src/scheduler/linux.rs`. Remove 1 row: `src/launchd.rs`. New section "§Scheduler trait" (1 paragraph) covering compile-time dispatch + cross-OS intent abstraction. ARCHITECTURE.md §Phase status: add Phase 3.1 ✅ row.
12. **Discovery Report mandatory** (CLAUDE.md). Must cover: anchor verification recap (especially #18 Cargo.toml grep result), Sub-mech C migration grep counts (pre/post `LaunchctlClient` hits), INV preservation check (10/11/12/13/17 each still 2-point enforced post-refactor).

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings, binary ≤7MB
- [ ] `cargo test --all` — all pass (count ≥ 144 baseline, ideally +3–5 for new MacosScheduler + 3 for CrontabScheduler stub tests if running on Linux host)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing (macOS dogfood OR `cargo check --target` if cross-target available)
- [ ] On macOS host: `cargo build --release` produces working binary; `./target/release/advisory-cron register --label probe-p012 --schedule "0 11 * * *"` writes `~/Library/LaunchAgents/com.advisorycron.probe-p012.plist`; `launchctl list | grep com.advisorycron.probe-p012` row present; `./target/release/advisory-cron status --label probe-p012` → "daily at 11:00"; `./target/release/advisory-cron unregister --label probe-p012` exits 0 + plist gone.
- [ ] On Linux host: `cargo build --release` succeeds (stub compiles); `./target/release/advisory-cron register --label probe-p012 --schedule "0 11 * * *"` returns exit ≠ 0 with stderr containing "P013 chưa ship" (acceptable — Linux is stub).
- [ ] On Linux host: `./target/release/advisory-cron init` still works (no scheduler call); `advisory-cron --help` shows all 6 subcommands.

### Regression
- [ ] `cargo test --test cli_register` — 5 tests pass (or skip on Linux: those marked-or-implicitly-macOS may bail with stub error; Worker decides whether to gate tests `#[cfg(target_os = "macos")]` in this phiếu OR accept Linux test failures as expected per stub bail — Architect leans **gate the tests** to keep CI green on both jobs).
- [ ] `cargo test --test cli_status` — 5 tests pass on macOS; on Linux: subset that don't invoke `RealLaunchctl::print` still pass; ones that do, gate `#[cfg(target_os = "macos")]`.
- [ ] `cargo test --test cli_mcp` — 7 tests pass on macOS (5 tools listed + handshake + parity).
- [ ] `cargo test --test cli_run --test cli_run_alert --test cli_run_retry --test cli_init --test cli_help` — all pass (no scheduler coupling).
- [ ] `grep -rn "LaunchctlClient" src/ tests/` returns 0 hits.
- [ ] `grep -rn "src/launchd" .` returns 0 hits (file removed cleanly, no stale refs).

### Docs Gate (Tầng 1 — module add)
- [ ] `docs/CHANGELOG.md` — entry for P012 with: scope (trait extract), 0 schema change, 0 dep change, 0 INV change, test count delta.
- [ ] `docs/ARCHITECTURE.md` §Modules table — DELETE `src/launchd.rs` row; ADD 3 `src/scheduler/{mod,macos,linux}.rs` rows; ADD new §Scheduler trait section after §Cron mechanism explaining compile-time dispatch + `PlatformScheduler` alias.
- [ ] `docs/ARCHITECTURE.md` §Phase status — Phase 3.1 ✅ row added.
- [ ] `README.md` — UNCHANGED (no CLI surface drift; Linux quick-start defer P015).
- [ ] `docs/security/INVARIANTS.md` — UNCHANGED (INV-10..21 all preserved; INV-22/23 defer P014). If Worker disagrees (e.g. wants to add a note that INV-10/17 now live inside `scheduler::macos`), defer — that's a doc-polish concern.
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P012.md` — full report written covering:
  - Anchor verification results (especially #18 Cargo.toml grep — confirm zero `[target.'cfg(...)']` deps; #20 unit-struct confirm).
  - Sub-mech C migration grep counts (before/after `LaunchctlClient` references).
  - INV preservation audit: INV-10 (`launchctl bootstrap` discrete args), INV-11 (`current_uid` parse u32), INV-12 (2-point label validate — core + scheduler::macos), INV-13 (plist path boundary), INV-17 (`launchctl print` shell-out) — each: ✅ preserved / ❌ regressed.
  - Any assumption-correction (Architect was wrong about X — code was Y; docs fixed).
  - Layer 2 Sub-mech A-E checks fired + results.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top).
- [ ] Sub-mechanism A-E Verification Trace filled.
