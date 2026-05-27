# PHIẾU P004: Task runner + heartbeat log (Phase 1.4)

> **Loại:** Feature
> **Tầng:** 1 (heartbeat JSONL schema = durable contract — P005 status + P008 Phase 2 alert consume; new dep `serde_json` promoted from transitive → explicit; new module `src/runner.rs` + `src/heartbeat.rs`; new optional config field `task.label`)
> **Ưu tiên:** P0 (Phase 1 acceptance §`run` fires task + writes heartbeat — gates Phase 1.5 status + every downstream Phase 2 alert)
> **Ảnh hưởng:** `src/runner.rs` (mới), `src/heartbeat.rs` (mới), `src/cli/run.rs` (rewrite body + extend `Args` struct), `src/main.rs` (thêm 2 `mod` declarations), `src/config.rs` (add optional `label` field tới `TaskConfig`), `Cargo.toml` (`[dependencies]` += `serde_json = "1"`), `tests/cli_run.rs` (mới — integration test with /bin/echo task)
> **Dependency:** P001 (CLI scaffold) ✅, P002 (config schema) ✅, P003 (register/unregister + LaunchAgents) ✅
> **Phiếu version:** V2 (Turn 1 [O1.1] ACCEPT — Anchor #24 baseline corrected 13 → 33; Task 3 `default_config_path` aligned to register.rs `home_dir()` bail! pattern)

---

## Context

### Vấn đề hiện tại

P003 ship `register` + `unregister` 2026-05-27. `run` + `status` vẫn `bail!()` stub. Phase 1 acceptance gate (`docs/PROJECT.md:55` `[verified]`):

- "advisory-cron run fires the configured task once, captures stdout/stderr, writes heartbeat."

Hiện tại launchd plist Phase 1.3 sẽ fire `<self_exe> run` mỗi lần lịch tới — nhưng `run` chỉ bail. Tức là plist registered nhưng task không bao giờ chạy thật → toàn bộ giá trị "tool exists vì shipping a check ≠ the check running" (Sub-mechanism A — PROJECT.md vision paragraph) vẫn vô hiệu cho đến khi P004 ship.

Đồng thời P005 status cần `heartbeat.jsonl` schema cố định để đọc `read_last_n` → P004 phải lock-in schema NGAY (P005 không được phép đổi schema retroactively — đó là logic Tầng 1 dòng "Heartbeat schema change" tại `docs/RULES.md:19` `[verified]`).

Reference: `docs/BACKLOG.md:24` `[verified]` — "Phase 1.4 — Task runner + heartbeat log. Function `fire_task(config) -> RunResult { exit_code, stdout, stderr, duration }`. Uses `tokio::process::Command`. On completion, append 1 JSON line to `heartbeat.jsonl`: `{ts, label, exit_code, duration_ms, stdout_tail, stderr_tail}`. `run` subcommand invokes this once. Tầng 1 (defines heartbeat schema — durable contract for `status` + future Phase 2 alert). ~200 LOC."

### Giải pháp

**3 module + 1 config field + 1 explicit-dep promotion + 1 CLI flag.**

**1. `src/runner.rs` — task spawn + capture.**

```rust
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

pub async fn fire_task(config: &Config) -> Result<RunResult> { ... }
```

- Uses `tokio::process::Command` (NOT `std::process`). `Cargo.toml:18` `[verified]` already has tokio `process` feature.
- Resolves `config.task.command` + `config.task.args` + sets `current_dir(config.task.working_dir)`.
- Captures both stdout + stderr (`.output().await`).
- Times wall-clock duration: `std::time::Instant::now()` before spawn, `.elapsed().as_millis() as u64` after.
- Exit code: `output.status.code().unwrap_or(-1)` — signal-killed (no code) → `-1`. Documented in code comment.
- Stdout/stderr converted to `String` via `String::from_utf8_lossy(&output.stdout).into_owned()` — non-UTF-8 bytes become U+FFFD replacement chars (acceptable — heartbeat is human-readable diagnostic, not exact byte preservation).
- Errors propagated: spawn failure (binary not found, perm denied) → `anyhow::Error` upward. `run` subcommand handler logs heartbeat with `exit_code=-1` + `stderr_tail=<error message>` per ARCHITECTURE.md:268 §Error handling row "Task spawn fail".

**2. `src/heartbeat.rs` — JSONL append + read-last-N.**

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HeartbeatRecord {
    pub ts: DateTime<Utc>,
    pub label: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()> { ... }
pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>> { ... }

// Helper, pub(crate) — Worker may choose private if not needed cross-module.
pub(crate) fn tail_utf8(s: &str, max_bytes: usize) -> String { ... }
```

- **Schema (locked, durable):** matches `docs/ARCHITECTURE.md:235` exact `[verified]`. Fields in serde-derive order = `ts`, `label`, `exit_code`, `duration_ms`, `stdout_tail`, `stderr_tail`. `ts` = `chrono::DateTime<Utc>` (chrono `serde` feature `[verified]` at `Cargo.toml:17`). RFC3339 format auto by chrono serde default.
- **No `schema_version` field in Phase 1** — `docs/ARCHITECTURE.md:249` `[verified]` defers schema versioning to Phase 2 when first breaking change happens.
- **`append`:** `OpenOptions::new().create(true).append(true).open(log_path)` → `serde_json::to_string(record)?` → `write_all(line.as_bytes())` → `write_all(b"\n")`. 1 record = 1 line. JSONL convention.
- **Parent dir auto-create:** `if let Some(parent) = log_path.parent() { fs::create_dir_all(parent).context(...)?; }`. Resolves Heads-up #4 from spawn prompt. Default heartbeat dir `~/.local/state/advisory-cron/` may not exist on fresh install.
- **`read_last_n`:** `BufRead::lines()` collect all → take last `n` → parse each via `serde_json::from_str` → return `Vec<HeartbeatRecord>`. If file does not exist → return `Ok(vec![])` (caller distinguishes "no fires yet" vs "read error"). Malformed line → log warning to stderr, skip that line, continue (defensive — append failures mid-write should never corrupt downstream read). Order preserved oldest→newest within the returned Vec (P005 status can reverse if needed).
- **`tail_utf8(s: &str, max_bytes: usize)`:** truncate `s` to last `max_bytes` bytes, snapping to a UTF-8 character boundary (use `str::is_char_boundary` walk-backward) so output is still valid UTF-8. NOT grapheme cluster boundary (would need `unicode-segmentation` dep — out of scope; char boundary sufficient to avoid serde_json producing invalid JSON). Comment in code explains "char-boundary only, not grapheme — diagnostic readability acceptable mid-cluster split". Worker writes unit test verifying multi-byte char near boundary not split mid-byte.

**3. `src/cli/run.rs` — rewrite body.**

`Args` struct gets `--config <path>` per same pattern as P003 register/unregister:

```rust
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml)
    #[arg(long)]
    pub config: Option<PathBuf>,
}
```

Body steps:

1. Load config: `args.config.unwrap_or_else(|| default_config_path())` → `Config::load(&path)?`. Default path resolution mirrors P003 register/unregister exactly — Worker confirmed at `src/cli/register.rs:49-50` `[verified per Turn 1 Anchor #25]` that register.rs inlines `args.config.unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"))` using a `home_dir()` helper that `bail!`s when `$HOME` is unset. **V2 update (Turn 1 self-note):** P004's `default_config_path` MUST use the same `home_dir()` bail! pattern — NOT `std::env::var("HOME").unwrap_or_else(|_| "/".to_string())` (silent fallback to `/` would write the heartbeat config to `/` root which is a quiet bug: file write may succeed for root user, or fail with cryptic permission error for non-root, both worse than an explicit `$HOME` unset error). See Task 3 §Lưu ý for exact pattern.
2. Resolve label: `config.task.label.clone().unwrap_or_else(|| "advisory-cron".to_string())`. Architect decision: see §Heads-up #1 resolution below (Option A — new optional `task.label` field).
3. Fire task: `let result = runner::fire_task(&config).await;`
4. Build heartbeat: handle 2 cases:
   - `Ok(run_result)` → `HeartbeatRecord { ts: Utc::now(), label, exit_code: run_result.exit_code, duration_ms: run_result.duration_ms, stdout_tail: tail_utf8(&run_result.stdout, 1024), stderr_tail: tail_utf8(&run_result.stderr, 1024) }`
   - `Err(spawn_err)` → `HeartbeatRecord { ts: Utc::now(), label, exit_code: -1, duration_ms: <measure even spawn-fail elapsed>, stdout_tail: String::new(), stderr_tail: format!("spawn failed: {spawn_err:#}") }`. Per ARCHITECTURE.md:268 §Error handling table row "Task spawn fail" `[verified]`.
5. Append heartbeat: `heartbeat::append(&config.heartbeat.log_path, &record)`. Append failure → log warning to stderr per ARCHITECTURE.md:269 row "Heartbeat write fail" `[verified]` ("Log warning to stderr, do NOT fail the run") — `eprintln!("warning: heartbeat write failed: {err:#}")` then continue. **Hard rule:** heartbeat write failure NEVER changes the exit code derived from task exit code (task already ran — operator needs to know task result regardless of log durability).
6. **Exit code resolution (Architect decision — see §Heads-up #2 resolution below):**
   - Task fired + exited 0 → exit 0
   - Task fired + exited non-zero → exit **4** (ARCHITECTURE.md:76 `[verified]` exit code 4 = "Task fire failed (subcommand `run` only)")
   - Task spawn-failed (binary not found etc.) → exit 4 (same category — task failed to complete successfully)
   - Config load failure → exit 2 (ARCHITECTURE.md:74 `[verified]`)
   - Heartbeat write failure on otherwise-successful task → exit follows task exit code (warn-stderr only, no code change)

   Rationale: exit code 4 collapses "task ran but failed" + "task could not be spawned" — both are "fire failed from operator perspective". Operator inspects heartbeat to distinguish via `exit_code` field (`-1` vs `>0`). Phase 2 alert classifies the same way.

**4. `src/main.rs` — add 2 `mod` declarations.**

```rust
mod runner;
mod heartbeat;
```

Same pattern as P003's `mod launchd;` — Anchor #7 `[verified per Turn 1]` confirms `src/main.rs:6-8` contains `mod cli; mod config; mod launchd;`. P004 extends this block.

**5. `src/config.rs` — add optional `label` field to `TaskConfig`.**

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TaskConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    /// Optional label for heartbeat record. Defaults to "advisory-cron" when omitted.
    /// Distinct from `register --label` (launchd plist label) — heartbeat label
    /// identifies "what this run was about" in Phase 2 alerting (single config →
    /// single conceptual task identity), while plist label may vary per registration.
    #[serde(default)]
    pub label: Option<String>,
}
```

- `#[serde(default)]` on `Option<String>` deserializes to `None` when field absent → backward compat with all P002/P003 configs.
- `Config::default_for_home` adds `label: Some("advisory-cron".to_string())` so `advisory-cron init` writes a sensible default value (so users see the knob exists rather than discovering it only via docs).
- Schema documented in ARCHITECTURE.md §Config schema field reference table (P004 Docs Gate adds row).

**6. `Cargo.toml` — promote `serde_json` from transitive to explicit dep.**

Add line in `[dependencies]`:
```toml
serde_json = "1"
```

- **Why required:** Rust binary code CANNOT `use serde_json::...` unless the crate is in `[dependencies]`, even if it's present in `Cargo.lock` as a transitive (`reqwest 0.12.28` pulls `serde_json 1.0.150` per `Cargo.lock:934` `[verified]`). Strict dep visibility per cargo doctrine.
- **Marginal cost = 0:** `serde_json 1.0.150` already compiled into the dep graph via reqwest → adding to `[dependencies]` adds 0 new transitive crates, 0 compile time delta. Just makes the namespace available to our code.
- **Acceptable per RULES.md:16** `[verified]` Tầng 1 trigger "Cargo.toml [dependencies] add/remove — CHANGELOG entry citing crate + reason". P004 CHANGELOG cites this explicitly.
- Considered alternatives: (a) hand-roll JSON via `format!` with manual escape — fragile (rejected — would re-implement serde_json's escape logic, hard to maintain, breaks Heartbeat schema durability principle); (b) skip serde and use raw string — same problem. Option (a)/(b) saves zero compile time vs explicit dep promotion since serde_json already in lock.

### Heads-up resolutions

**Heads-up #1 — label source for HeartbeatRecord (Architect decision required, see spawn prompt):**

**Decision: Option A (new `Option<String>` `label` field in `TaskConfig`, default fallback `"advisory-cron"`).**

Rationale:

- **Option B (derive from launchd env var)** rejected: when `launchctl bootstrap` fires `<self_exe> run`, no `LAUNCH_*` env vars are auto-injected by launchd in user agent context. Sếp would need to add `<key>EnvironmentVariables</key>` block in plist — couples P004 back to P003 plist generator (re-spec). Brittle.
- **Option C (hardcode "advisory-cron-run")** rejected for Phase 2 readiness: ARCHITECTURE.md:235 example heartbeat already shows `"label": "advisory-scan-daily"` — implies user-meaningful identifier. Phase 2 Telegram alert needs to distinguish "which task fired" by label when user runs N different configs (one per repo's advisory-cron, all alerting to same chat). Hardcoded label makes that impossible without future schema migration.
- **Option A** chosen with the key constraint: **label is independent of P003's `register --label` CLI flag.** P003 contract preserved — `--label` still required, still the plist's `Label` key. P004's `task.label` is heartbeat-only identity. They CAN match (Sếp's discipline) but the tool doesn't enforce coupling. Defer label-unification to Phase 2+ if Sếp confirms friction.
- Backward compat: `Option<String>` with `#[serde(default)]` means existing P002/P003 configs deserialize unchanged. Falls back to literal `"advisory-cron"` when unset.
- Cost: 1 optional field in `TaskConfig` + 1 row in ARCHITECTURE.md Config schema table + 1 line in `default_for_home`. Minimal schema impact.

**Heads-up #2 — exit code semantics for `run` (Architect decision):**

**Decision: exit 4 for ANY non-zero task outcome (including spawn-fail), exit 0 only when task exited 0.**

Per ARCHITECTURE.md:76 `[verified]` exit code 4 = "Task fire failed (subcommand `run` only)". Single category, no sub-classification at process boundary — heartbeat's `exit_code` field carries the granularity (`-1` for spawn-fail / signal-kill, `>0` for task non-zero exit). launchd treats both as "fire failed"; Phase 2 alert classifies via heartbeat field.

Pass-through alternative (task exit code → run exit code) rejected: would conflict with advisory-cron's own exit code namespace (0/1/2/3/4/5/130 in ARCHITECTURE.md:69-78 table `[verified]`). If task exits 2 (some Python script convention for "config error"), Sếp/launchd shouldn't confuse that with advisory-cron's own "exit 2 = config invalid".

**Heads-up #3 — tokio `process` feature confirmation:**

`Cargo.toml:18` `[verified]`: `tokio = { version = "1", features = ["rt", "macros", "process", "time", "fs"] }`. P002 Worker verified this earlier (per P002 discovery). P004 Worker re-confirms via Anchor #2 grep before using `tokio::process::Command`.

**Heads-up #4 — heartbeat dir auto-create:**

Resolved in §Giải pháp module 2 `append` design: `if let Some(parent) = log_path.parent() { fs::create_dir_all(parent).context(...)?; }`. Default `~/.local/state/advisory-cron/` may not exist on fresh install. Idempotent (create_dir_all returns Ok if already exists). Worker integration test (Task 6) verifies: TempDir parent does NOT pre-create the heartbeat dir; `run` invocation succeeds and dir is auto-created.

**Heads-up #5 — `serde_json` explicit promotion:**

Resolved in §Giải pháp module 6 (explicit `[dependencies]` add). NOT escalated as separate AskUserQuestion because:

- Cargo.lock already contains `serde_json 1.0.150` via reqwest transitive (`Cargo.lock:1066` `[verified]`).
- Marginal binary size + compile time delta = ~0.
- Spawn prompt explicitly said "if missing → escalate as Tầng 1 dep addition" — em treats this AS the Tầng 1 dep addition, declared in phiếu header (Tầng: 1 includes "new dep `serde_json` promoted from transitive → explicit").
- CHANGELOG entry mandatory per RULES.md:16 — Worker writes this in Docs Gate step.

If Worker discovers Cargo.lock has actually drifted (impossible given Architect just read it, but defensive) and `serde_json` is absent, escalate AskUserQuestion at Task 0 (rare path).

### Scope

- CHỈ tạo/sửa:
  - `src/runner.rs` (mới — `RunResult`, `fire_task`, unit tests)
  - `src/heartbeat.rs` (mới — `HeartbeatRecord`, `append`, `read_last_n`, `tail_utf8`, unit tests)
  - `src/cli/run.rs` (rewrite body + extend `Args` struct với `config: Option<PathBuf>`)
  - `src/main.rs` (thêm `mod runner;` + `mod heartbeat;`)
  - `src/config.rs` (add `pub label: Option<String>` field to `TaskConfig` + update `default_for_home`)
  - `Cargo.toml` (`[dependencies]` += `serde_json = "1"`)
  - `tests/cli_run.rs` (mới — integration test with `/bin/echo` task)
  - `docs/ARCHITECTURE.md` (update §Config schema table, §Modules table mark runner.rs + heartbeat.rs shipped 1.4, §Heartbeat schema confirm spec matches impl, §Phase status)
  - `docs/CHANGELOG.md` (entry citing `serde_json` dep + Phase 1.4 ship)
  - `docs/discoveries/P004.md` (mới)
  - `docs/DISCOVERIES.md` (1-line index append)
  - `docs/security/INVARIANTS.md` (append INV entry for `tokio::process::Command` spawn boundary + heartbeat file write boundary — see Docs Gate)
- KHÔNG sửa:
  - `src/cli/mod.rs` — NEWTYPE DISPATCH (P003 V2 lesson Turn 1 [O1.1] `[verified]` via P003 phiếu). `Commands::Run(run::Args)` already wraps `Args` opaquely; new `--config` flag declared INSIDE `run::Args` propagates via clap derive. **`git diff src/cli/mod.rs` post-EXECUTE MUST be empty.**
  - `src/cli/init.rs`, `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/status.rs` (P002/P003 + Phase 1.5 territory)
  - `src/launchd.rs` (P003 shipped — P004 does NOT touch plist generation)
  - `Cargo.toml` `[dev-dependencies]` (`tempfile`, `tokio-test` already present `[verified]`)
  - `tests/cli_help.rs`, `tests/cli_init.rs`, `tests/cli_register.rs` (P001/P002/P003 regression — must continue passing unmodified)
  - `README.md` (defer Phase 1.6)
  - `.phieu-counter` (Quản đốc bumped 003 → 004 already)
- KHÔNG tạo: `src/alert.rs`, `src/retry.rs` (Phase 2), `src/core/`, `src/mcp/` (Phase 1.7)

### Skills consulted (optional)

*(Orchestrator chưa chạy skill nào cho phiếu này. Verification dựa Read docs + Cargo.lock + P003 V2 lessons captured in Anchor table.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Mỗi anchor carry humility marker. `[verified]` = em đã Read file confirm. `[unverified]` = docs imply, em chưa Read source. `[needs Worker verify]` = punt cho Thợ grep.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/run.rs` hiện stub `bail!("not yet implemented (Phase 1.4)")` hoặc tương đương | Read `src/cli/run.rs` | `[needs Worker verify]` → `[verified per Turn 1]` | ✅ `src/cli/run.rs:11` — `bail!("` run` not yet implemented (Phase 1.4)")` (note backtick around `run`). Stub confirmed. |
| 2 | `Cargo.toml:18` tokio features include `process` | Read `Cargo.toml` | `[verified]` | ✅ `tokio = { version = "1", features = ["rt", "macros", "process", "time", "fs"] }` confirmed. |
| 3 | `Cargo.toml:17` chrono includes `serde` feature | Read `Cargo.toml` | `[verified]` | ✅ `chrono = { version = "0.4", features = ["serde"] }` confirmed. P004 uses `DateTime<Utc>` serde derive. |
| 4 | `Cargo.lock` already contains `serde_json` (transitive via reqwest) — marginal cost = 0 for explicit promotion | Read `Cargo.lock` | `[verified]` | ✅ `Cargo.lock:1066` — `serde_json 1.0.150`. Pulled by reqwest at `Cargo.lock:934`. Adding to `[dependencies]` adds 0 new transitive crates. |
| 5 | `Cargo.lock` already contains `chrono` (direct dep at top-level since P002) | Read `Cargo.lock` | `[verified]` | ✅ Confirmed top-level dep at `Cargo.lock:8-20`. |
| 6 | `Cargo.toml:26` `[dev-dependencies]` has `tempfile = "3"` + `tokio-test = "0.4"` (for integration test) | Read `Cargo.toml` | `[verified]` | ✅ `Cargo.toml:26-27` both present. |
| 7 | `src/main.rs` post-P003 has `mod cli;` + `mod config;` + `mod launchd;` (P003 added launchd) — pattern to extend for `runner` + `heartbeat` | Read `src/main.rs` | `[needs Worker verify]` → `[verified per Turn 1]` | ✅ `src/main.rs:6-8` — block is `mod cli; mod config; mod launchd;`. P004 Task 6 adds `mod runner;` + `mod heartbeat;` (lines 9-10 or alphabetical per Worker style). |
| 8 | `src/config.rs` `TaskConfig` struct shape (`command: String, args: Vec<String>, working_dir: PathBuf`) per ARCHITECTURE.md:108-110 `[verified]` | Read `docs/ARCHITECTURE.md` | `[verified]` | ✅ Confirmed shape. P004 adds `label: Option<String>` as new field. |
| 9 | `src/config.rs::Config::default_for_home(home: &Path) -> Config` exists (P002 ship) | P002 phiếu + ARCHITECTURE.md:134 | `[verified]` | ✅ Confirmed per ARCHITECTURE.md §Source module "Config, TaskConfig, ScheduleConfig, HeartbeatConfig structs + load, default_for_home, write_default functions". |
| 10 | `src/config.rs::HeartbeatConfig.log_path: PathBuf` exists with default `~/.local/state/advisory-cron/heartbeat.jsonl` | ARCHITECTURE.md:115 + P002 phiếu | `[verified]` | ✅ Field name + type + default confirmed at ARCHITECTURE.md:115 Field reference. |
| 11 | `src/cli/mod.rs` uses NEWTYPE dispatch `Commands::Run(run::Args)` per P003 V2 lesson Turn 1 [O1.1] | P003 phiếu V2 Anchor #3 result + dispatch confirmed at `src/cli/mod.rs:30` for Register | `[verified per Turn 1]` | ✅ `src/cli/mod.rs:23` — `Run(run::Args)` enum variant. `src/cli/mod.rs:33` — `Commands::Run(args) => run::run(args).await` dispatch. Newtype pattern confirmed. NO mod.rs edit needed. |
| 12 | ARCHITECTURE.md heartbeat schema `{ts, label, exit_code, duration_ms, stdout_tail, stderr_tail}` (no schema_version in Phase 1) | Read `docs/ARCHITECTURE.md:230-249` | `[verified]` | ✅ Schema exact match at lines 232-249. Field type table at 240-247. Note at 249 confirms "No version field in Phase 1". |
| 13 | ARCHITECTURE.md exit code 4 = "Task fire failed (subcommand `run` only)" | Read `docs/ARCHITECTURE.md:76` | `[verified]` | ✅ Confirmed line 76. |
| 14 | ARCHITECTURE.md exit code 2 = "Config not found / invalid" (run config-load failure path) | Read `docs/ARCHITECTURE.md:74` | `[verified]` | ✅ Confirmed line 74. |
| 15 | ARCHITECTURE.md §Error handling table row "Heartbeat write fail → Log warning to stderr, do NOT fail the run" | Read `docs/ARCHITECTURE.md:269` | `[verified]` | ✅ Confirmed line 269. Heartbeat write failure NEVER changes exit code. |
| 16 | ARCHITECTURE.md §Error handling table row "Task spawn fail → Exit 4, log to heartbeat with exit_code=-1 + stderr_tail=<spawn error>" | Read `docs/ARCHITECTURE.md:268` | `[verified]` | ✅ Confirmed line 268. P004 spawn-fail path matches. |
| 17 | RULES.md Tầng 1 row "Heartbeat schema change → ARCHITECTURE.md §Heartbeat schema + version bump" | Read `docs/RULES.md:19` | `[verified]` | ✅ Confirmed line 19. P004 ship LOCKS schema — no version field needed yet (Phase 1 baseline). |
| 18 | RULES.md Tầng 1 row "Cargo.toml [dependencies] add → CHANGELOG entry citing crate + reason" | Read `docs/RULES.md:16` | `[verified]` | ✅ Confirmed line 16. P004 CHANGELOG cites `serde_json` promotion + reason (heartbeat JSONL serialize). |
| 19 | RULES.md Tầng 1 row "Module added → ARCHITECTURE.md §Modules table" | Read `docs/RULES.md:21` | `[verified]` | ✅ Confirmed line 21. P004 updates Modules table — mark `src/runner.rs` + `src/heartbeat.rs` as shipped 1.4. |
| 20 | RULES.md Tầng 1 row "Security boundary touched (env var read, file write outside `.sos-state/` or `docs/runlog/`) → AUTO Tầng 1 + docs/security/INVARIANTS.md review" | Read `docs/RULES.md:22` | `[verified]` | ✅ Confirmed. P004 writes to `~/.local/state/advisory-cron/heartbeat.jsonl` (outside sos-state) AND spawns child process via `tokio::process::Command` (external program exec) — BOTH trigger INVARIANTS.md append. |
| 21 | `docs/security/INVARIANTS.md` exists (P003 V2 Anchor #15 confirmed) — append-only, no stub creation needed | Glob `docs/security/INVARIANTS.md` | `[verified]` → `[verified per Turn 1: current max = INV-13]` | ✅ Confirmed file present. Worker grep: current max INV = **INV-13**. New entries will be INV-14, INV-15, INV-16. |
| 22 | chrono `DateTime<Utc>` with `serde` feature serializes to RFC3339 string by default | chrono docs (well-known stable behavior since 0.4) | `[unverified]` | ⏳ Stable behavior since chrono 0.4 baseline. Worker confirms via unit test round-trip in Task 2 (HeartbeatRecord serde roundtrip). If chrono changed default to non-RFC3339, escalate (would be silent ARCHITECTURE.md spec violation). |
| 23 | `tokio::process::Command::output().await` returns `std::io::Result<std::process::Output>` with `.status.code() -> Option<i32>` | tokio docs (stable since 1.x process feature) | `[unverified]` | ⏳ Stable API since tokio 1.0 process feature. Worker confirms via cargo check at Task 0 baseline + unit-test compile in Task 1. |
| 24 | **Test baseline (CORRECTED V2 per Turn 1 [O1.1]):** P003 ship test baseline = **33 tests total** (20 lib unit tests in `src/config.rs` / `src/cli/*` / `src/launchd.rs` + 3 `cli_help` + 4 `cli_init` + 6 `cli_register` integration). `cargo test --all 2>&1 | grep 'test result'` must show all 33 pass pre-P004. Post-P004 floor = **33 + N new tests** (N from runner unit + heartbeat unit + cli_run integration + config new label-field test). V1 stated "13 tests" — that figure counted integration suites only, missed the 20 lib unit tests; corrected to 33 here. | `cargo test --all` per Worker Turn 1 empirical run | `[verified per Turn 1]` | ✅ 33 confirmed. V1 spawn prompt had the bug; V2 anchor + Verification Trace + Task 0 Step 4 + Automated nghiệm thu all updated. |
| 25 | `default_config_path` resolution pattern in `src/cli/register.rs` / `unregister.rs` — Worker should reuse if helper extracted, or inline `home.join(".config/advisory-cron/config.toml")` if not | P003 phiếu Task 3 body step 1 (Heads-up #2 resolution) | `[verified per Turn 1]` | ✅ `src/cli/register.rs:49-50` — `args.config.unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"))`. Pattern is inlined, NOT a separate helper. **Important V2 correction:** register.rs uses a local `home_dir()` helper that `bail!`s when `$HOME` is unset — P004's `default_config_path` MUST adopt the same `bail!` semantics, NOT V1's silent `unwrap_or_else(\|_\| "/".to_string())` fallback. See Task 3 §Lưu ý + V2 §Giải pháp module 3 body step 1 update. |

**Anchors flagged for Worker Task 0 priority:** #1, #7, #11 → all confirmed in Turn 1 (no further Task 0 grep needed). #22+#23 still pending — compile-time confirms via cargo check before commit.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi Quản đốc triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (Turn 1 [O1.1] ACCEPTED — anchor #24 baseline corrected; Task 3 `default_config_path` aligned to register.rs `home_dir()` bail! pattern)

### Turn 1 — Worker Challenge

**Date:** 2026-05-27

**Anchor verification (all 25 anchors checked):**

- Anchor #1: ✅ `src/cli/run.rs:11` — `bail!("`run` not yet implemented (Phase 1.4)")` confirmed stub (note: backtick around `run` inside the string — exact message differs slightly from phiếu's parenthetical `"not yet implemented (Phase 1.4)"` wording but functionally a stub as expected).
- Anchor #2: ✅ `Cargo.toml:18` — `tokio = { version = "1", features = ["rt", "macros", "process", "time", "fs"] }` confirmed.
- Anchor #3: ✅ `Cargo.toml:17` — `chrono = { version = "0.4", features = ["serde"] }` confirmed.
- Anchor #4: ✅ `Cargo.lock:1066` — `serde_json 1.0.150` confirmed as transitive dep. NOT yet in `Cargo.toml [dependencies]` — confirmed P004 will add it.
- Anchor #5: ✅ `Cargo.lock` shows chrono as top-level dep.
- Anchor #6: ✅ `Cargo.toml:26-27` — `tempfile = "3"` and `tokio-test = "0.4"` both present in `[dev-dependencies]`.
- Anchor #7: ✅ `src/main.rs:6-8` — mod block is `mod cli; mod config; mod launchd;` (lines 6-8). P004 Task 6 adds `mod runner;` + `mod heartbeat;` at lines 9-10 (or alphabetical per Worker style).
- Anchor #8: ✅ `src/config.rs:39-43` — `TaskConfig { command: String, args: Vec<String>, working_dir: PathBuf }` confirmed. P004 adds `label: Option<String>` as 4th field.
- Anchor #9: ✅ `src/config.rs:99` — `pub fn default_for_home(home: &Path) -> Self` confirmed.
- Anchor #10: ✅ `src/config.rs:61-63` — `HeartbeatConfig { log_path: PathBuf }` confirmed. Default path `home.join(".local/state/advisory-cron/heartbeat.jsonl")` at line 108.
- Anchor #11: ✅ `src/cli/mod.rs:23` — `Run(run::Args)` enum variant. `src/cli/mod.rs:33` — `Commands::Run(args) => run::run(args).await` dispatch. Newtype pattern confirmed. NO mod.rs edit needed for P004.
- Anchor #12: ✅ Deferred to Architect's `[verified]` read of `docs/ARCHITECTURE.md:230-249`. Worker trusts — no source discrepancy possible (schema is spec, not code, at this stage).
- Anchor #13: ✅ Deferred to Architect's `[verified]` read of `docs/ARCHITECTURE.md:76`.
- Anchor #14: ✅ Deferred to Architect's `[verified]` read of `docs/ARCHITECTURE.md:74`.
- Anchor #15: ✅ Deferred to Architect's `[verified]` read of `docs/ARCHITECTURE.md:269`.
- Anchor #16: ✅ Deferred to Architect's `[verified]` read of `docs/ARCHITECTURE.md:268`.
- Anchor #17: ✅ Deferred to Architect's `[verified]` read of `docs/RULES.md:19`.
- Anchor #18: ✅ Deferred to Architect's `[verified]` read of `docs/RULES.md:16`.
- Anchor #19: ✅ Deferred to Architect's `[verified]` read of `docs/RULES.md:21`.
- Anchor #20: ✅ Deferred to Architect's `[verified]` read of `docs/RULES.md:22`.
- Anchor #21: ✅ `docs/security/INVARIANTS.md` exists and current max INV is INV-13 (confirmed by grep). New INV entries will be INV-14, INV-15, INV-16.
- Anchor #22: ⚠️ `[unverified]` by Architect — Worker notes: chrono serde RFC3339 auto-serialization is stable behavior in chrono 0.4 (serde feature). The roundtrip unit test in Task 2 `heartbeat_record_serde_roundtrip` will confirm at compile+test time. If test passes, anchor confirmed. Escalate only if test fails.
- Anchor #23: ⚠️ `[unverified]` by Architect — Worker notes: `tokio::process::Command::output().await` returning `io::Result<std::process::Output>` with `.status.code() -> Option<i32>` is stable tokio 1.x API. Baseline `cargo check` passes (exit 0, zero warnings). Will be confirmed by Task 1 compile.
- Anchor #24: ❌ **MISMATCH — see Objection [O1.1] below.** Phiếu states "13 tests baseline." Actual `cargo test --all` output: **33 tests total** (20 lib unit tests from `src/config.rs` + `src/cli/` + `src/launchd.rs` + 3 cli_help + 4 cli_init + 6 cli_register = 33). The "13" figure appears to count only integration tests, not lib unit tests.
- Anchor #25: ✅ `src/cli/register.rs:49-50` — `args.config.unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"))`. Pattern is inlined, NOT a separate helper function. Worker notes: register.rs uses `home_dir()` (which bail!s if $HOME unset), while the phiếu's run.rs code in Task 3 uses `std::env::var("HOME").unwrap_or_else(|_| "/".to_string())` (silently falls back to `/`). This behavioral difference is Tầng 2 — phiếu explicitly delegates as "Worker self-decide." Worker will align run.rs with register.rs pattern (use `bail!` on HOME unset via a local `home_dir()` helper) — noted here for Discovery Report, not an objection.

---

**Objections (Tầng 1 only):**

**[O1.1] — Anchor #24 test count factually wrong: phiếu states "13 tests baseline", actual is 33 tests.**

Evidence:
- `cargo test --all 2>&1 | grep -E "running [0-9]+ test"` output:
  - `running 20 tests` (lib unit tests — config.rs has 9, cli/register.rs has additional, launchd.rs has others)
  - `running 3 tests` (tests/cli_help.rs)
  - `running 4 tests` (tests/cli_init.rs)
  - `running 6 tests` (tests/cli_register.rs)
  - Total = **33 tests**

- The phiếu's Anchor #24 states: "13 tests pass (regression target)" and cites "spawn prompt confirms 13 tests baseline." The 13 figure counts only integration test suites (3+4+6=13), ignoring the 20 lib unit tests already present.

- Impact: The Verification Trace table row "B (capability) — `cargo test --lib config` — 9+ pass" is internally correct (config has 9 unit tests) but Anchor #24's "≥ 13" framing creates a false floor. Post-P004, the phiếu's own acceptance check ("13 + new (cli_run integration + runner unit + heartbeat unit) ≥ 13 + N") will mislead the Worker: 33 + N is the correct regression floor, not 13 + N.

- This is Tầng 1 because: another Worker reading Anchor #24 in EXECUTE mode at Step 4 ("must show 13+ passing") would pass the check with only integration tests and miss a lib unit test regression. The acceptance gate is architecturally unsound as written.

**Proposed alternatives (Worker recommends A):**

A. **(Recommended)** Architect updates Anchor #24 to: "Baseline: 33 tests total (20 lib unit + 13 integration). `cargo test --all 2>&1 | grep 'test result'` must show all pass. Post-P004 floor: 33 + N new tests." Update the Verification Trace "B (capability)" row for `cargo test --all` expected value to show "33+ pass" and the Task 0 Step 4 instruction to match.

B. Architect keeps 13 as the integration-only count but explicitly qualifies it: "13 integration tests pass (cli_help 3 + cli_init 4 + cli_register 6); 20 lib unit tests additionally present — Worker run `cargo test --all` to confirm all pass before and after." Lower precision but fewer edits.

**Status:** ✅ RESOLVED in Turn 1 Architect Response below.

### Turn 1 — Architect Response (phiếu V2)

**Date:** 2026-05-27

- **[O1.1] → ACCEPT (Option A).** Worker's empirical evidence is conclusive: V1's "13" was inherited from a spawn prompt summary that counted integration suites only and missed the 20 lib unit tests. This is a mechanical baseline correction — no architectural conflict, no spec ambiguity. Actions taken in V2:
  - **Anchor #24** rewritten: baseline = **33 tests total (20 lib unit + 13 integration)**. Acceptance gate: `cargo test --all 2>&1 | grep 'test result'` must show all 33 pass pre-P004, and 33 + N pass post-P004.
  - **Task 0 Step 4** updated: "Baseline test count — `cargo test --all 2>&1 | grep -E 'test result'` → must show 33+ passing per Anchor #24 V2."
  - **Verification Trace** updated: added a new explicit row `B (capability) — cargo test --all — 33+ pass (V2 baseline correction per Turn 1 [O1.1])`. Existing per-suite rows preserved for granularity.
  - **Automated nghiệm thu** updated: bullet now reads "`cargo test --all` — all 33+ pass (33 baseline + new runner unit + heartbeat unit + cli_run integration + config new test for label field)."
  - No other phiếu logic affected. Per-suite rows (`cli_help` 3, `cli_init` 4, `cli_register` 6, `--lib config` 9+) remain correct; the bug was only in the aggregate framing.

- **Worker's Tầng 2 self-note on `$HOME` `unwrap_or_else(\|_\| "/".to_string())` vs register.rs `bail!` pattern → AGREE, AND elevating to spec-level note.** Strictly this is Tầng 2 territory (Worker self-decide on local helper style), but the silent `/` fallback is genuinely a quiet-bug risk worth blessing explicitly rather than leaving as "Worker style." Reasoning: if `$HOME` is unset (rare but real — e.g., cron without env injection, container init), V1's silent `/` fallback would either (a) succeed writing to `/` for root and pollute filesystem root, or (b) fail with a cryptic "permission denied opening `/`" error that doesn't point to the `$HOME` root cause. register.rs's `bail!("$HOME not set")` pattern surfaces the real error immediately. Actions taken in V2:
  - **§Giải pháp module 3 body step 1** updated: explicit note that P004's `default_config_path` MUST use the same `home_dir()` bail! pattern as register.rs, not silent fallback. Reasoning inline.
  - **Anchor #25 result column** updated: now notes the bail! semantics requirement explicitly so this isn't lost as informal Worker Discovery.
  - **Task 3 §Lưu ý** updated: bullet for `default_config_path` now reads "Mirror register.rs's `home_dir()` helper which `bail!`s on `$HOME` unset. Do NOT use `std::env::var("HOME").unwrap_or_else(\|_\| "/".to_string())` — silent `/` fallback is a quiet bug. Worker may either extract a shared `cli::home_dir()` helper (Tầng 2 stylistic — acceptable if no `mod.rs` edit needed; e.g., put the helper in a new file `src/cli/util.rs` and declare via `mod util;` in mod.rs ← that WOULD touch mod.rs, so DON'T do this) OR inline the same bail! pattern locally in `src/cli/run.rs`. Inlining is the recommended path to preserve Constraint #1 (no mod.rs edits)."
  - **Task 3 code block** updated: `default_config_path` function rewritten to return `Result<PathBuf>` and use `anyhow::bail!` on `$HOME` unset, with the call site `args.config.map(Ok).unwrap_or_else(default_config_path)?` (or equivalent — Worker self-decide on exact error-propagation style as long as bail! semantics preserved).
  - **Constraint added (#16):** "`default_config_path` MUST bail! on `$HOME` unset (mirror register.rs pattern). NO silent `/` fallback. Per V2 Turn 1 self-note resolution."

**Status:** ✅ RESPONDED — phiếu bumped to V2. Worker may proceed to Turn 2 CHALLENGE (re-verify V2 anchors + V2 Task 3 spec) or, if no remaining objections, signal consensus → Quản đốc approval gate.

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Turn 2 — Worker Challenge

**Date:** 2026-05-27

**V2 change verification (9 items from Architect Response):**

1. **Anchor #24 corrected to "33 tests (20 lib unit + 13 integration)"** — PRESENT at Anchor #24 result column (line 252) and Debate Log Turn 1 Architect Response.
2. **Task 0 Step 4 updated to "33+ passing"** — PRESENT at Task 0 Step 4 text: "must show **33+ passing** per Anchor #24 V2".
3. **Verification Trace explicit row `cargo test --all` → 33+ pass** — PRESENT in Verification Trace table, row `B (capability) — cargo test --all — 33+ pass (V2 baseline correction per Turn 1 [O1.1])`.
4. **Automated nghiệm thu updated to "all 33+ pass"** — PRESENT in Nghiệm thu §Automated: "`cargo test --all` — **all 33+ pass** (V2 baseline correction...)".
5. **Regression aggregate guard row added** — PRESENT in §Regression: "`cargo test --all 2>&1 | grep 'test result'` — **aggregate 33+ pass** (V2 — guard against lib unit regression)".
6. **Task 3 `default_config_path` rewritten: returns `Result<PathBuf>` with `bail!` on $HOME unset** — PRESENT in Task 3 code block. Function signature `fn default_config_path() -> Result<PathBuf>` with `bail!("$HOME environment variable is empty")` guard confirmed.
7. **Constraint #16 added** — PRESENT: "16. `default_config_path` MUST `bail!` on `$HOME` unset. Mirror `src/cli/register.rs:49-50` `home_dir()` pattern. NO silent `/` fallback."
8. **Manual test `env -u HOME cargo run -- run` → expect exit 2** — PRESENT in §Manual Testing: "$HOME unset (V2 — Constraint #16 confirmation): `env -u HOME cargo run -- run`; expect exit 2 with stderr containing '$HOME environment variable is not set'."
9. **Constraint #14 extended to item (f)** — PRESENT: item (f) reads "`default_config_path` `bail!` semantics — confirm Worker did NOT regress to silent `/` fallback."

**Anchor re-verification for Turn 2 (anchors flagged in spawn prompt):**

- Anchor #1: ✅ `[verified per Turn 1]` confirmed in Marker column.
- Anchor #7: ✅ `[verified per Turn 1]` confirmed in Marker column.
- Anchor #11: ✅ `[verified per Turn 1]` confirmed in Marker column.
- Anchor #21: ✅ `[verified per Turn 1: current max = INV-13]` confirmed. Worker re-grep: `grep -c "INV-" /Users/nguyenhuuanh/advisory-cron/docs/security/INVARIANTS.md` confirms INV-13 as current max — new entries INV-14/15/16 pre-allocated in Task 8 H.
- Anchor #25: ✅ `[verified per Turn 1]` confirmed. Worker re-read `src/cli/register.rs:111-119` in Turn 2: `home_dir()` helper uses `env::var("HOME").ok().filter(|s| !s.is_empty())` with `bail!` on None path — semantically equivalent to V2 Task 3's `default_config_path` (both bail on unset/empty $HOME). No discrepancy.

**Anchors #22 + #23 status:** still `[unverified]` by Architect per phiếu note. Confirmed: these will be resolved by compile + roundtrip test during EXECUTE (not Tầng 1 objections — known stable APIs).

**Tầng 1 objections:** None.

**Worker accepted V2 — no new challenges.** All 9 V2 changes verified present and correctly placed. Anchor verification: #1 ✅, #7 ✅, #11 ✅, #21 ✅ (INV-13 max confirmed), #25 ✅ (register.rs bail! semantics re-confirmed at lines 111-119). V2 phiếu is internally consistent, anchors confirmed against actual code. Ready for Chủ nhà approval gate.

**Status:** ✅ CONSENSUS — V2 accepted. Worker ready for EXECUTE on Chủ nhà approval.

### Final consensus
- Phiếu version: V2
- Total turns: 1 full debate turn (Turn 1 Worker challenge + Architect response) + Turn 2 Worker acceptance
- Approved (autonomous narrate or Sếp gate): 2026-05-27 — code execution may begin on Chủ nhà approval

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.
> Purpose: post-mortem trace, especially for autonomous mode where Sếp didn't watch live.
> Append-only — Worker writes, không edit/delete.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

Example:
```
[2026-05-27T15:00:00Z] event=task0_anchor_1_grep evidence=src/cli/run.rs:<line> "<exact bail message>"
[2026-05-27T15:01:00Z] event=task0_anchor_7_grep evidence=src/main.rs:<line> "mod runner; mod heartbeat; absent — to be added"
[2026-05-27T15:02:00Z] event=task0_anchor_11_grep evidence=src/cli/mod.rs:<line> "Commands::Run(run::Args)" newtype confirmed
[2026-05-27T15:30:00Z] event=cargo_check_baseline evidence=exit_code=0 duration_ms=<n>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks (RULES.md matrix) BEFORE marking phiếu DONE.
> Fill the table; mark N/A if not applicable to this phiếu.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | (manual post-EXECUTE) `cargo run --release -- run --config tests/fixtures/echo-task.toml` (or inline tempfile config) | exit 0 + heartbeat appended | | |
| A (trigger) | `cat <heartbeat-path>` | 1 JSON line matching schema (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) | | |
| B (capability) | `cargo check` | exit 0, zero warnings | | |
| B (capability) | `cargo test --all 2>&1 \| grep 'test result'` | **33+ pass** (V2 baseline correction per Turn 1 [O1.1]: 20 lib unit + 13 integration baseline; post-P004 floor = 33 + N new) | | |
| B (capability) | `cargo test --test cli_help` | 3/3 pass (P001 regression) | | |
| B (capability) | `cargo test --test cli_init` | 4/4 pass (P002 regression) | | |
| B (capability) | `cargo test --test cli_register` | 6/6 pass (P003 regression) | | |
| B (capability) | `cargo test --test cli_run` | new integration tests pass | | |
| B (capability) | `cargo test --lib runner` | unit tests for `fire_task` + `RunResult` pass | | |
| B (capability) | `cargo test --lib heartbeat` | unit tests for `append`, `read_last_n`, `tail_utf8` pass | | |
| B (capability) | `cargo test --lib config` | 9+ pass (existing 9 + new `task.label` field default test) | | |
| C (migration) | `echo '[task]\ncommand="x"\nargs=[]\nworking_dir="/"\n\n[schedule]\nhour=9\nminute=0\n\n[heartbeat]\nlog_path="/tmp/x"' > /tmp/old.toml; cargo run -- run --config /tmp/old.toml` (config without `task.label`) | exit non-zero only if echo path bogus; serde accepts missing label (default None) | | |
| D (persistence) | `grep -l "Heartbeat schema" docs/ARCHITECTURE.md` | ≥1 hit (existing §Heartbeat schema preserved + spec re-confirmed) | | |
| D (persistence) | `grep -l "src/runner.rs\|src/heartbeat.rs" docs/ARCHITECTURE.md` | ≥1 hit each (Modules table updated to shipped 1.4 ✅) | | |
| D (persistence) | `grep -c "INV-" docs/security/INVARIANTS.md` | strictly greater than pre-P004 baseline (new INV entry for tokio::process::Command + heartbeat write boundary) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | | |
| E (env drift) | `cargo build --release` from clean `target/` | exit 0, binary < 7MB | | |
| E (env drift) | `git diff src/cli/mod.rs` | **EMPTY** (V1 [O1.1] preemptive hard rule from P003 V2 lesson) | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE verification (Worker mandatory)

1. **Anchor recap reads** — Read `src/cli/run.rs`, `src/cli/mod.rs`, `src/main.rs`, `src/config.rs`. Log to Debug Log:
   - Anchor #1: exact line + message of `bail!` in run.rs (confirmed Turn 1: `src/cli/run.rs:11`)
   - Anchor #7: exact line range of `mod` declaration block in main.rs + current contents (confirmed Turn 1: `src/main.rs:6-8` = `mod cli; mod config; mod launchd;`)
   - Anchor #11: confirm `Commands::Run(run::Args)` newtype dispatch pattern at src/cli/mod.rs — DO NOT attempt to edit mod.rs enum variants (P003 V2 Turn 1 hard lesson)
   - Anchor #25: pattern used by `register.rs` / `unregister.rs` for `args.config.unwrap_or_else(|| ...)` default path resolution — capture exact expression for reuse (confirmed Turn 1: `src/cli/register.rs:49-50` uses inline `home_dir()` helper with `bail!` on `$HOME` unset — replicate this pattern in run.rs's `default_config_path`, NOT silent fallback)

2. **Cargo.lock + Cargo.toml dep audit** — confirm:
   - `Cargo.lock` has `serde_json 1.0.150` (or newer 1.x) — Anchor #4. If absent (impossible per Architect read, but defensive) → escalate AskUserQuestion.
   - `Cargo.toml [dependencies]` does NOT yet have explicit `serde_json` line — confirms P004 will add it as new.
   - `Cargo.toml [dependencies]` `tokio` features include `process` — Anchor #2.
   - `Cargo.toml [dependencies]` `chrono` features include `serde` — Anchor #3.

3. **Baseline `cargo check`** — confirm clean (zero warnings post-P003) BEFORE any edit. Record duration in Debug Log.

4. **Baseline test count (V2 corrected)** — `cargo test --all 2>&1 | grep -E "test result"` → must show **33+ passing** per Anchor #24 V2 (20 lib unit + 13 integration). V1's "13" figure was integration-only and is no longer the gate. If pre-P004 count differs from 33, escalate (something shifted in lib unit tests since the Turn 1 measurement).

5. **NO mod.rs edits invariant** — Worker reads `src/cli/mod.rs` and commits to memory: "I will NOT edit this file. Post-EXECUTE `git diff src/cli/mod.rs` must be empty." (P003 V2 [O1.1] hard rule generalized to all subsequent CLI phiếu.)

### Task 1: Tạo `src/runner.rs` — task spawn + stdout/stderr capture

**File:** `src/runner.rs` (mới)

**Thêm:**

```rust
//! Phase 1.4 — task runner. Spawns a configured child process via tokio,
//! captures stdout + stderr + exit code + wall-clock duration.
//!
//! Public surface:
//! - `RunResult` — value type returned to caller (cli::run handler)
//! - `fire_task(config)` — async one-shot spawn + capture
//!
//! Design constraints (from P004 phiếu):
//! - Use `tokio::process::Command` (NOT std::process). Cargo.toml tokio "process" feature confirmed.
//! - Captured stdout/stderr lossy-converted to String (non-UTF8 → U+FFFD). Diagnostic-readable
//!   acceptable; advisory-cron is not a byte-precise log collector.
//! - Signal-killed children (no exit code) reported as exit_code = -1.
//! - Spawn failure (binary not found etc.) propagates as anyhow::Error — caller (cli::run)
//!   builds spawn-fail heartbeat per ARCHITECTURE.md §Error handling.

use anyhow::{Context, Result};
use std::time::Instant;
use tokio::process::Command;

use crate::config::Config;

/// Result of one task fire. Returned to `cli::run` handler which builds heartbeat record.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// Spawn `config.task.command` with `config.task.args` in `config.task.working_dir`,
/// wait for exit, capture stdout + stderr.
///
/// Errors: spawn failure (binary not found / perm denied / fork failed).
/// Non-zero exit code is NOT an error — it's a `RunResult` with `exit_code != 0`.
pub async fn fire_task(config: &Config) -> Result<RunResult> {
    let started = Instant::now();
    let output = Command::new(&config.task.command)
        .args(&config.task.args)
        .current_dir(&config.task.working_dir)
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to spawn task `{cmd}` with args {args:?} in dir {dir:?}",
                cmd = config.task.command,
                args = config.task.args,
                dir = config.task.working_dir,
            )
        })?;
    let duration_ms = started.elapsed().as_millis() as u64;

    Ok(RunResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, HeartbeatConfig, ScheduleConfig, TaskConfig};
    use std::path::PathBuf;

    fn echo_config(args: Vec<&str>) -> Config {
        Config {
            task: TaskConfig {
                command: "/bin/echo".to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                working_dir: PathBuf::from("/tmp"),
                label: Some("test".to_string()),
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/tmp/unused.jsonl"),
            },
        }
    }

    #[tokio::test]
    async fn fire_task_echo_captures_stdout_exit_zero() {
        let config = echo_config(vec!["hello"]);
        let result = fire_task(&config).await.expect("echo should succeed");
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.stderr, "");
        // duration is non-zero (echo takes >0ms even fast) but small (<1s on any machine)
        assert!(result.duration_ms < 1_000, "duration_ms = {}", result.duration_ms);
    }

    #[tokio::test]
    async fn fire_task_nonexistent_binary_returns_err() {
        let config = echo_config(vec![]);
        let mut bogus = config.clone();
        bogus.task.command = "/nonexistent/binary-that-does-not-exist".to_string();
        let result = fire_task(&bogus).await;
        assert!(result.is_err(), "expected spawn-fail error");
    }

    #[tokio::test]
    async fn fire_task_nonzero_exit_returns_ok_with_code() {
        // /bin/sh -c "exit 7" — captured, returned as RunResult.exit_code = 7
        let mut config = echo_config(vec![]);
        config.task.command = "/bin/sh".to_string();
        config.task.args = vec!["-c".to_string(), "exit 7".to_string()];
        let result = fire_task(&config).await.expect("sh -c spawns");
        assert_eq!(result.exit_code, 7);
    }
}
```

**Lưu ý:**

- Field order in struct literal MUST match `TaskConfig` definition post-P004 (i.e., include `label` field). If Worker re-orders, ensure `#[serde]` derive still serializes in spec order.
- Tests assume `/bin/echo` + `/bin/sh` exist (POSIX baseline — true on macOS + Linux CI runners). If hardening needed Phase 2+.
- `Instant::now()` measures monotonic wall-clock — safe across system clock adjustments. Don't replace with `SystemTime`.
- `String::from_utf8_lossy(&output.stdout).into_owned()` — `into_owned()` required because `from_utf8_lossy` returns `Cow<str>` (borrowed if input is valid UTF-8). Owned `String` simplifies caller (avoid lifetime through `RunResult`).

### Task 2: Tạo `src/heartbeat.rs` — HeartbeatRecord + append + read + tail_utf8

**File:** `src/heartbeat.rs` (mới)

**Thêm:**

```rust
//! Phase 1.4 — heartbeat JSONL log writer + reader.
//!
//! Schema is DURABLE — Phase 2 alert + P005 status both consume. Adding fields requires
//! `schema_version` bump per ARCHITECTURE.md §Heartbeat schema. Field order in this struct
//! definition matches doc spec line-by-line.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// One heartbeat = one task fire result, serialized as 1 JSON line.
///
/// Schema spec: `docs/ARCHITECTURE.md` §Heartbeat schema. Field order MUST match.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HeartbeatRecord {
    pub ts: DateTime<Utc>,
    pub label: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

/// Append one heartbeat record as a single JSON line to `log_path`.
///
/// Creates parent directory if missing (per Heads-up #4 — `~/.local/state/advisory-cron/`
/// may not exist on fresh install). Creates the file if missing. Append-only — never
/// truncates or rotates (ARCHITECTURE.md PROJECT.md hard line #4 "Heartbeat log is append-only").
pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()> {
    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create heartbeat dir {parent:?}"))?;
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("failed to open heartbeat file {log_path:?} for append"))?;

    let line = serde_json::to_string(record)
        .context("failed to serialize HeartbeatRecord")?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("failed to write heartbeat line to {log_path:?}"))?;
    file.write_all(b"\n")
        .with_context(|| format!("failed to write newline to {log_path:?}"))?;

    Ok(())
}

/// Read the last `n` heartbeat records (oldest-first within the returned Vec).
///
/// Returns `Ok(vec![])` if the file does not exist (no fires yet — distinguish from read error).
/// Malformed lines are skipped with a stderr warning; continuing parsing — defensive against
/// partial-write corruption (P004 does NOT use crash-safe write+rename; Phase 2.3 will).
pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>> {
    if !log_path.exists() {
        return Ok(vec![]);
    }

    let file = fs::File::open(log_path)
        .with_context(|| format!("failed to open heartbeat file {log_path:?} for read"))?;
    let reader = BufReader::new(file);

    let mut records: Vec<HeartbeatRecord> = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("failed to read line {i} of {log_path:?}"))?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<HeartbeatRecord>(&line) {
            Ok(rec) => records.push(rec),
            Err(err) => {
                eprintln!("warning: skipping malformed heartbeat line {i}: {err}");
            }
        }
    }

    let start = records.len().saturating_sub(n);
    Ok(records.into_iter().skip(start).collect())
}

/// Truncate `s` to the last `max_bytes` bytes, snapping to a UTF-8 character boundary
/// (NOT grapheme cluster — that would need `unicode-segmentation` dep).
///
/// Returns owned String. If `s.len() <= max_bytes`, returns full copy.
pub(crate) fn tail_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Start from the byte index (len - max_bytes), walk forward to next char boundary.
    let mut start = s.len() - max_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn sample_record() -> HeartbeatRecord {
        HeartbeatRecord {
            ts: Utc.with_ymd_and_hms(2026, 5, 27, 2, 0, 0).unwrap(),
            label: "advisory-scan-daily".to_string(),
            exit_code: 0,
            duration_ms: 45230,
            stdout_tail: "last 1KB of stdout".to_string(),
            stderr_tail: "".to_string(),
        }
    }

    #[test]
    fn heartbeat_record_serde_roundtrip() {
        let rec = sample_record();
        let json = serde_json::to_string(&rec).unwrap();
        let parsed: HeartbeatRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, parsed);
        // Confirm schema field names appear verbatim
        assert!(json.contains("\"ts\":"));
        assert!(json.contains("\"label\":"));
        assert!(json.contains("\"exit_code\":"));
        assert!(json.contains("\"duration_ms\":"));
        assert!(json.contains("\"stdout_tail\":"));
        assert!(json.contains("\"stderr_tail\":"));
    }

    #[test]
    fn append_creates_parent_dir_and_file() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a/b/c/heartbeat.jsonl");
        let rec = sample_record();
        append(&nested, &rec).expect("append should create parents + file");
        assert!(nested.exists());
        let contents = fs::read_to_string(&nested).unwrap();
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.ends_with('\n'), "trailing newline required for JSONL");
    }

    #[test]
    fn append_then_read_last_n_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("heartbeat.jsonl");
        for i in 0..5 {
            let mut rec = sample_record();
            rec.exit_code = i as i32;
            append(&path, &rec).unwrap();
        }
        let last3 = read_last_n(&path, 3).unwrap();
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].exit_code, 2);
        assert_eq!(last3[2].exit_code, 4);
    }

    #[test]
    fn read_last_n_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does-not-exist.jsonl");
        let result = read_last_n(&path, 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn read_last_n_skips_malformed_line() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("heartbeat.jsonl");
        let rec = sample_record();
        append(&path, &rec).unwrap();
        // Inject a bad line
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"this-is-not-json\n").unwrap();
        append(&path, &rec).unwrap();
        let recs = read_last_n(&path, 10).unwrap();
        assert_eq!(recs.len(), 2, "should skip malformed line, keep 2 valid");
    }

    #[test]
    fn tail_utf8_under_limit_returns_full() {
        let s = "hello";
        assert_eq!(tail_utf8(s, 1024), "hello");
    }

    #[test]
    fn tail_utf8_over_limit_truncates_to_char_boundary() {
        // "héllo" — é is 2 bytes (0xC3 0xA9). Build a string where a naive byte-cut would split it.
        let s = "aaaaaaaaaaé"; // 10 ASCII + 2 bytes for é = 12 bytes total
        // Request last 3 bytes — naive cut at index 9 lands inside é. Must snap forward.
        let tail = tail_utf8(s, 3);
        assert!(std::str::from_utf8(tail.as_bytes()).is_ok(), "must be valid UTF-8");
        // Should NOT contain a half-é. Either contains é fully (snap-forward landed at é boundary)
        // or skips é (snap-forward to byte 12 = empty).
        assert!(tail.is_empty() || tail == "é" || tail == "a" || tail == "aé",
            "unexpected tail: {tail:?}");
    }

    #[test]
    fn tail_utf8_pure_ascii_exact_cut() {
        let s = "0123456789";
        assert_eq!(tail_utf8(s, 4), "6789");
    }
}
```

**Lưu ý:**

- `serde_json` import requires `Cargo.toml [dependencies]` line added in Task 5 — Worker confirm dep present before this file compiles.
- `chrono::DateTime<Utc>` serde derive emits RFC3339 by default per Anchor #22 `[unverified]`. The roundtrip unit test catches any chrono behavior change.
- `tail_utf8` is `pub(crate)` so `cli::run` handler can call it without re-exporting. If Worker chooses private + duplicate call site implementation in cli::run, that's a Tầng 2 stylistic call.
- The `tail_utf8_over_limit_truncates_to_char_boundary` test uses loose assertions — multiple valid outcomes depending on exact byte boundary snap. The hard invariant is "must be valid UTF-8" (verified explicitly).
- File creation is NOT crash-safe (no fsync, no write+rename). Phase 2.3 fixes — documented as known limitation in heartbeat.rs module doc + ARCHITECTURE.md §Phase status note.

### Task 3: Sửa `src/cli/run.rs` — rewrite body, extend Args with --config

**File:** `src/cli/run.rs`

**Tìm:** the existing stub body confirmed Turn 1 at `src/cli/run.rs:11` (`bail!("`run` not yet implemented (Phase 1.4)")`).

**Thay bằng (V2 — `default_config_path` now uses `bail!` on `$HOME` unset, mirroring register.rs):**

```rust
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::PathBuf;

use crate::config::Config;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::runner::{self};

/// `advisory-cron run` — fire the configured task once + append heartbeat.
///
/// Exit codes (see docs/ARCHITECTURE.md §CLI surface exit codes):
/// - 0: task fired and exited 0
/// - 2: config not found / invalid OR $HOME unset (default path unresolvable)
/// - 4: task fired non-zero OR spawn failed (heartbeat distinguishes via exit_code field: -1 for spawn-fail)
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub async fn run(args: Args) -> Result<u8> {
    // 1. Resolve config path. If --config not given, default to ~/.config/advisory-cron/config.toml.
    //    bail! on $HOME unset (mirror src/cli/register.rs:49-50 home_dir() pattern; never silently
    //    fall back to "/" — that would either write to filesystem root or fail with a cryptic
    //    permission error). See P004 V2 Turn 1 Architect Response.
    let config_path = match args.config {
        Some(p) => p,
        None => match default_config_path() {
            Ok(p) => p,
            Err(err) => {
                eprintln!("error: failed to resolve default config path: {err:#}");
                return Ok(2);
            }
        },
    };

    // 2. Load config — exit 2 on failure per ARCHITECTURE.md:74.
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: failed to load config {config_path:?}: {err:#}");
            return Ok(2);
        }
    };

    // 3. Resolve heartbeat label (defaults to "advisory-cron" if config.task.label unset).
    let label = config
        .task
        .label
        .clone()
        .unwrap_or_else(|| "advisory-cron".to_string());

    // 4. Fire task. Both Ok and Err build a heartbeat record.
    let started_for_spawn_fail = std::time::Instant::now();
    let fire_result = runner::fire_task(&config).await;

    let record = match &fire_result {
        Ok(rr) => HeartbeatRecord {
            ts: Utc::now(),
            label: label.clone(),
            exit_code: rr.exit_code,
            duration_ms: rr.duration_ms,
            stdout_tail: heartbeat::tail_utf8(&rr.stdout, 1024),
            stderr_tail: heartbeat::tail_utf8(&rr.stderr, 1024),
        },
        Err(spawn_err) => HeartbeatRecord {
            ts: Utc::now(),
            label: label.clone(),
            exit_code: -1,
            duration_ms: started_for_spawn_fail.elapsed().as_millis() as u64,
            stdout_tail: String::new(),
            stderr_tail: format!("spawn failed: {spawn_err:#}"),
        },
    };

    // 5. Append heartbeat. Per ARCHITECTURE.md:269, heartbeat write fail is a warning,
    //    NOT a run failure — task already ran, operator needs the exit code regardless.
    if let Err(hb_err) = heartbeat::append(&config.heartbeat.log_path, &record) {
        eprintln!("warning: heartbeat write failed: {hb_err:#}");
    }

    // 6. Resolve exit code per phiếu §Giải pháp module 3 step 6.
    let exit = match &fire_result {
        Ok(rr) if rr.exit_code == 0 => 0u8,
        Ok(_) => 4u8,                   // task ran but exited non-zero
        Err(_) => 4u8,                  // spawn failed
    };
    Ok(exit)
}

/// Resolve default config path. Bail! when `$HOME` is unset — never silently fall back
/// to `/`. Mirrors `src/cli/register.rs:49-50` `home_dir()` helper pattern (per P004 V2
/// Turn 1 Architect Response).
fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(anyhow::Error::from)
        .context("$HOME environment variable is not set")?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home).join(".config/advisory-cron/config.toml"))
}
```

**Lưu ý:**

- `Args` struct uses `#[derive(clap::Args)]` (NOT `Parser`) — matches the newtype dispatch pattern `Commands::Run(run::Args)` confirmed via P003 V2 Anchor #3 + Turn 1 Anchor #11. New `--config` flag propagates via clap derive — NO `src/cli/mod.rs` edits needed.
- **`default_config_path` MUST bail! on `$HOME` unset** (V2 update per Turn 1 self-note resolution). DO NOT use `std::env::var("HOME").unwrap_or_else(\|_\| "/".to_string())` — silent `/` fallback either writes to filesystem root (root user) or fails with a cryptic permission error (non-root). register.rs:49-50 already uses this `bail!` pattern; replicate the semantics. Worker may inline the helper as shown above OR factor it into a private helper in `src/cli/run.rs` — either is fine as long as `src/cli/mod.rs` is NOT touched (Constraint #1).
- If Worker discovers register.rs uses a NAMED helper (e.g., `crate::cli::home_dir()`) accessible without mod.rs edits (e.g., already declared as `pub(crate)` in a shipped file), reuse it. Worker self-decide based on actual register.rs layout — but the bail! semantics are NOT optional.
- Step 4 measures `started_for_spawn_fail` separately because `runner::fire_task` returns Err BEFORE measuring duration internally — we need a parallel timer for the spawn-fail heartbeat duration field.
- Step 5 `eprintln!` uses stderr (not `tracing::warn!`) — keeps Phase 1 simple. P002/P003 pattern.
- Step 6: order of patterns matters — `Ok(rr) if rr.exit_code == 0` MUST come before `Ok(_)` else the catch-all swallows the success case.
- Exit code 2 now also covers "default config path unresolvable due to $HOME unset" (documented in the doc-comment exit code table above). This subsumes under "config not found / invalid" per ARCHITECTURE.md:74 — Worker reflects this in the §Phase status note if the Docs Gate review requires (probably not needed — exit 2 description is already general enough).

### Task 4: Sửa `src/config.rs` — add `label: Option<String>` field to `TaskConfig`

**File:** `src/config.rs`

**Tìm:** the `TaskConfig` struct definition (per P002 ship — Worker grep `struct TaskConfig` and read surrounding context). Turn 1 confirmed location: `src/config.rs:39-43`.

**Thay bằng / Thêm:** Append `label` field to `TaskConfig`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TaskConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    /// Optional label identifying this task in heartbeat records.
    /// Distinct from `register --label` (which becomes the launchd plist Label key).
    /// Phase 2 alert may use this to distinguish multiple advisory-cron configs reporting
    /// to the same Telegram chat. Defaults to "advisory-cron" when omitted.
    #[serde(default)]
    pub label: Option<String>,
}
```

**Also update** `default_for_home` constructor (Turn 1 confirmed at `src/config.rs:99`) — find the existing `TaskConfig { ... }` literal and add `label: Some("advisory-cron".to_string()),` to that literal so `advisory-cron init` writes a visible default value.

**Lưu ý:**

- `#[serde(default)]` on `Option<String>` gives `None` when field is absent in TOML — backward compat with all P002/P003 configs. Worker writes a unit test confirming an old config (without `label = "..."`) deserializes successfully with `label: None`.
- Adding a new field to a struct breaks any caller that builds `TaskConfig { ... }` literally — Worker grep for `TaskConfig {` in test files and update each literal to include `label: ...` field. Likely sites: `tests/cli_init.rs` (if it builds Config literal), `src/config.rs` itself (`default_for_home`), and any new tests in `src/runner.rs` / `src/heartbeat.rs` (Task 1 / Task 2 already include the field).
- Don't add validation for `label` (e.g., no charset check). Phase 1 keeps it loose — heartbeat is a diagnostic, not a security boundary. Phase 2 may add validation if Telegram message escaping requires.

### Task 5: Sửa `Cargo.toml` — promote `serde_json` to explicit dep

**File:** `Cargo.toml`

**Tìm:** the `[dependencies]` section (lines 13-23 per Architect Read).

**Thay bằng / Thêm:** Add ONE line at the appropriate alphabetical position (or end — Worker style choice, Tầng 2):

```toml
serde_json = "1"
```

**Lưu ý:**

- No feature flags needed — default features include `std` which is what we use.
- Cargo.lock auto-updates on next `cargo build`/`cargo check` — no manual edit needed. Worker verify post-edit: `cargo check` succeeds with same `serde_json 1.0.150` (or newer 1.x) selected; no fresh transitive crates pulled.
- CHANGELOG entry mandatory per RULES.md:16 — Task 8 below.

### Task 6: Sửa `src/main.rs` — declare `mod runner;` + `mod heartbeat;`

**File:** `src/main.rs`

**Tìm:** the `mod` declaration block confirmed Turn 1 at `src/main.rs:6-8` (`mod cli; mod config; mod launchd;`).

**Thay bằng / Thêm:** Add 2 lines to the existing mod block:

```rust
mod runner;
mod heartbeat;
```

Placement: alphabetical preferred (between existing `mod` lines), but Worker may match the existing block's convention (e.g., source-order if that's the established pattern). Tầng 2 stylistic.

**Lưu ý:**

- Both modules used only inside the binary; no `pub` needed.
- If Worker discovers existing block uses `pub(crate) mod` or other modifier, match the pattern.

### Task 7: Tạo `tests/cli_run.rs` — integration test (binary spawn, /bin/echo task)

**File:** `tests/cli_run.rs` (mới)

**Thêm:**

```rust
//! Integration tests for `advisory-cron run` (Phase 1.4).
//!
//! Pattern follows P002 `tests/cli_init.rs` + P003 `tests/cli_register.rs`:
//! spawn the compiled binary with a temp config + temp heartbeat path,
//! assert on exit code + filesystem side effects.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn binary_path() -> String {
    // Match P002/P003 pattern. Worker confirm exact env var name used in cli_init.rs.
    env!("CARGO_BIN_EXE_advisory-cron").to_string()
}

fn write_config(dir: &Path, command: &str, args: &[&str], heartbeat_path: &Path) -> std::path::PathBuf {
    let config_path = dir.join("config.toml");
    let args_toml: String = args
        .iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    let contents = format!(
        r#"[task]
command = "{command}"
args = [{args_toml}]
working_dir = "/tmp"
label = "p004-integration"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{}"
"#,
        heartbeat_path.display()
    );
    fs::write(&config_path, contents).expect("write config");
    config_path
}

#[test]
fn run_with_echo_task_exits_zero_and_writes_one_heartbeat() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb/heartbeat.jsonl");
    let config_path = write_config(tmp.path(), "/bin/echo", &["hello-p004"], &heartbeat_path);

    let output = Command::new(binary_path())
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Heartbeat file must exist (parent auto-created)
    assert!(heartbeat_path.exists(), "heartbeat file should be created");
    let contents = fs::read_to_string(&heartbeat_path).expect("read heartbeat");
    assert_eq!(contents.lines().count(), 1, "expect exactly 1 heartbeat line");

    // Parse the line as JSON, confirm all 6 schema fields present
    let line = contents.lines().next().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
    assert!(parsed.get("ts").is_some());
    assert!(parsed.get("label").is_some());
    assert_eq!(parsed.get("label").and_then(|v| v.as_str()), Some("p004-integration"));
    assert_eq!(parsed.get("exit_code").and_then(|v| v.as_i64()), Some(0));
    assert!(parsed.get("duration_ms").is_some());
    let stdout_tail = parsed.get("stdout_tail").and_then(|v| v.as_str()).unwrap_or("");
    assert!(stdout_tail.contains("hello-p004"));
    assert_eq!(parsed.get("stderr_tail").and_then(|v| v.as_str()), Some(""));
}

#[test]
fn run_with_failing_task_exits_four_and_writes_heartbeat() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("heartbeat.jsonl");
    let config_path = write_config(
        tmp.path(),
        "/bin/sh",
        &["-c", "exit 7"],
        &heartbeat_path,
    );

    let output = Command::new(binary_path())
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(output.status.code(), Some(4), "expect exit 4 for task non-zero");
    assert!(heartbeat_path.exists());
    let line = fs::read_to_string(&heartbeat_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("valid JSON");
    assert_eq!(parsed.get("exit_code").and_then(|v| v.as_i64()), Some(7));
}

#[test]
fn run_with_nonexistent_binary_exits_four_and_writes_spawn_fail_heartbeat() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("heartbeat.jsonl");
    let config_path = write_config(
        tmp.path(),
        "/this/binary/definitely/does/not/exist",
        &[],
        &heartbeat_path,
    );

    let output = Command::new(binary_path())
        .args(["run", "--config", config_path.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(output.status.code(), Some(4), "expect exit 4 for spawn-fail");
    assert!(heartbeat_path.exists());
    let line = fs::read_to_string(&heartbeat_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).expect("valid JSON");
    assert_eq!(
        parsed.get("exit_code").and_then(|v| v.as_i64()),
        Some(-1),
        "spawn-fail heartbeat uses exit_code = -1 (no real child exit code available)"
    );
    let stderr_tail = parsed.get("stderr_tail").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        stderr_tail.contains("spawn failed"),
        "stderr_tail should describe spawn failure, got: {stderr_tail}"
    );
}

#[test]
fn run_with_missing_config_exits_two() {
    let tmp = TempDir::new().expect("tempdir");
    let bogus_config = tmp.path().join("does-not-exist.toml");

    let output = Command::new(binary_path())
        .args(["run", "--config", bogus_config.to_str().unwrap()])
        .output()
        .expect("spawn advisory-cron");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expect exit 2 for missing config per ARCHITECTURE.md:74"
    );
}
```

**Lưu ý:**

- Requires `serde_json` in `[dependencies]` (Task 5) — `[dev-dependencies]` would also work but explicit dep is needed in prod anyway, so reuse.
- `env!("CARGO_BIN_EXE_advisory-cron")` is a cargo-provided env var resolving at compile time to the path of the binary built for tests — Worker confirm pattern by reading `tests/cli_init.rs` or `tests/cli_register.rs` (`[needs Worker verify]`). If different mechanism used (e.g., `escargot` crate), match it.
- Tests run sequentially when fs operations target the same TempDir — but each test creates its own TempDir, so `cargo test -- --test-threads=N` works fine.
- The `run_with_nonexistent_binary_exits_four_and_writes_spawn_fail_heartbeat` test confirms heartbeat IS written even on spawn-fail (operator needs to see "spawn failed" in the heartbeat, not silently no record). This is the Phase 1 vision PROJECT.md hard line #5 "Failure mode = noisy" enforced at test level.
- No real `launchctl` invocation in these tests — they exercise the `run` subcommand standalone. End-to-end launchd-fires-run integration is a manual test (Phase 1 acceptance gate).

### Task 8: Update `docs/ARCHITECTURE.md` + `docs/CHANGELOG.md` (Docs Gate — Tầng 1)

**Files:**
- `docs/ARCHITECTURE.md`
- `docs/CHANGELOG.md`
- `docs/security/INVARIANTS.md`

**Updates required (precise, per Anchor #17/18/19/20):**

**A. `docs/ARCHITECTURE.md` §Modules table:** mark rows for `src/runner.rs` and `src/heartbeat.rs` as shipped 1.4 ✅ (currently "1.4" without ✅ per ARCHITECTURE.md:48-49 `[verified]`).

**B. `docs/ARCHITECTURE.md` §Config schema → Field reference table:** add row for new field:

```
| `[task]` | `label` | `string (optional)` | no | Heartbeat label for this config — distinct from `register --label` plist label | `"advisory-cron"` |
```

**C. `docs/ARCHITECTURE.md` §Config schema → Full schema TOML block:** add `label = "advisory-scan-daily"` line under `[task]` block as the (uncommented) example to make the new field visible to first-time readers.

**D. `docs/ARCHITECTURE.md` §CLI surface table:** update `run` row's `Args` column from `(no args)` (line 64) to `--config <path>` (optional — overrides default config path).

**E. `docs/ARCHITECTURE.md` §Heartbeat schema:** confirm spec matches implementation (no change needed — schema was specced correctly; if Worker's roundtrip test reveals any drift, ESCALATE not edit).

**F. `docs/ARCHITECTURE.md` §Phase status:** update line 275 to add "Phase 1.4 shipped" entry.

**G. `docs/CHANGELOG.md`:** add entry citing:
- Phase 1.4 ship (runner + heartbeat module)
- New explicit dep `serde_json = "1"` (cite reason: heartbeat JSONL serialize; promoted from transitive via reqwest; zero binary size delta)
- New optional config field `task.label`
- New CLI flag `run --config <path>`
- New tests: integration `tests/cli_run.rs` + unit tests in runner.rs + heartbeat.rs

**H. `docs/security/INVARIANTS.md`:** append project-specific INV entry covering P004 boundaries:
- **Child process spawn boundary:** `tokio::process::Command::new(&config.task.command).args(&config.task.args)` — command + args come from user config TOML. Phase 1 trusts config file (user-controlled, no third-party input). Phase 2 should add validation if config sourcing changes (e.g., remote config fetch).
- **Heartbeat file write boundary:** writes to `config.heartbeat.log_path` (user-configured PathBuf). `fs::create_dir_all(parent)` may create directories anywhere user has write permission. Trust boundary = user owns the config; advisory-cron does not sanitize the path.
- **JSON serialization boundary:** `stdout_tail` / `stderr_tail` derive from child process output (uncontrolled bytes). `String::from_utf8_lossy` neutralizes non-UTF-8. `serde_json::to_string` handles JSON escape correctly (does NOT need manual escape). Worker confirms no shell-interpretation of these strings anywhere in P004 code.

INV-number assignment: per Turn 1 Anchor #21 confirmation, current max INV in `docs/security/INVARIANTS.md` = **INV-13**. New entries are **INV-14** (process spawn boundary), **INV-15** (heartbeat file write boundary), **INV-16** (JSON serialization boundary).

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/runner.rs` | Task 1: NEW. `RunResult`, `fire_task`, unit tests. |
| `src/heartbeat.rs` | Task 2: NEW. `HeartbeatRecord`, `append`, `read_last_n`, `tail_utf8`, unit tests. |
| `src/cli/run.rs` | Task 3: Rewrite body. Extend `Args` with `config: Option<PathBuf>`. `default_config_path` uses `bail!` on `$HOME` unset (V2). |
| `src/config.rs` | Task 4: Add `label: Option<String>` field to `TaskConfig`. Update `default_for_home` literal. |
| `Cargo.toml` | Task 5: `[dependencies]` += `serde_json = "1"`. |
| `src/main.rs` | Task 6: Add `mod runner;` + `mod heartbeat;` to existing mod block. |
| `tests/cli_run.rs` | Task 7: NEW. 4 integration tests (echo success, sh non-zero, missing binary spawn-fail, missing config). |
| `docs/ARCHITECTURE.md` | Task 8 A-F: §Modules row marks, §Config schema field + TOML block + CLI surface table row update + §Phase status note. |
| `docs/CHANGELOG.md` | Task 8 G: P004 entry citing dep + module + config field + CLI flag + tests. |
| `docs/security/INVARIANTS.md` | Task 8 H: append INV-14/15/16 (process spawn, heartbeat write, JSON serialize boundaries). REQUIRED per RULES.md:22 (security boundary touched). |
| `docs/discoveries/P004.md` | Discovery Report (assumptions verified vs corrected + edge cases + docs updated). REQUIRED. |
| `docs/DISCOVERIES.md` | 1-line index append (newest at top). REQUIRED. |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/cli/mod.rs` | **DO NOT EDIT.** Newtype dispatch `Commands::Run(run::Args)` already forwards `Args`; new `--config` flag declared INSIDE `run::Args` propagates via clap derive. Post-EXECUTE `git diff src/cli/mod.rs` MUST be empty. |
| `src/cli/init.rs` | P002 ship. No P004 touch. Continues to work with new `task.label` field via `default_for_home` update. |
| `src/cli/register.rs` | P003 ship. No P004 touch. Unaffected by `task.label` (register reads its own `--label` flag, not config). |
| `src/cli/unregister.rs` | P003 ship. No P004 touch. |
| `src/cli/status.rs` | Phase 1.5 territory. Still bail!() stub after P004. |
| `src/launchd.rs` | P003 ship. No P004 touch. |
| `tests/cli_help.rs` | P001. 3 tests must continue passing (regression). New `--config` flag on `run` may add a line to its `--help` output — if existing tests substring-match on exact run-help text, they may need an additive line update. Worker check; if no substring match on run-help body, tests pass unmodified. |
| `tests/cli_init.rs` | P002. 4 tests must pass. If any test deserializes a default config and checks fields, `task.label` field must be allowed (test should accept either `None` or `Some("advisory-cron")` per Architect's `default_for_home` update). |
| `tests/cli_register.rs` | P003. 6 tests must pass unmodified. |
| `README.md` | Defer Phase 1.6. |
| `.phieu-counter` | Quản đốc bumped 003 → 004. Worker does NOT touch. |
| `Cargo.lock` | Auto-regenerated by cargo on next build. Worker confirms no surprise major-version jump in `cargo update --dry-run` (Sub-mech E). |

---

## Luật chơi (Constraints)

1. **NO `src/cli/mod.rs` edits.** Hard rule generalized from P003 V2 Turn 1 [O1.1]. Post-EXECUTE `git diff src/cli/mod.rs` MUST be empty.
2. **NO `unsafe { }` blocks.** Escalate via AskUserQuestion if tempted (zero current need — all P004 code uses safe APIs).
3. **NO new deps beyond `serde_json`.** That dep is the entire Tầng 1 dep delta authorized for P004. Worker MUST NOT add `libc`, `users`, `whoami`, `nix`, `time`, or any other crate. Escalate if discovery suggests one.
4. **NO heartbeat schema fields beyond the 6 specced.** No `schema_version`, no `retry_attempt`, no `host`. Phase 2 may add — gated by ARCHITECTURE.md migration note.
5. **NO heartbeat rotation / compaction.** PROJECT.md hard line #4 — append-only, user-managed via logrotate. Worker NEVER writes a "rotate if file > N MB" code path.
6. **Heartbeat write failure NEVER changes exit code.** Per ARCHITECTURE.md:269. Warning to stderr only.
7. **Exit code 4 for any non-zero task outcome OR spawn-fail.** Per Architect §Heads-up #2 decision. Worker MUST NOT pass through task exit code to advisory-cron exit code (would clash with advisory-cron's own 0/1/2/3/4/5/130 namespace).
8. **`stdout_tail` / `stderr_tail` MUST be UTF-8 valid** (use `tail_utf8` helper, char-boundary snap). Non-UTF-8 input handled via `String::from_utf8_lossy` upstream. This protects `serde_json::to_string` from producing invalid JSON.
9. **`#[serde(default)]` on `TaskConfig.label`** so old configs (without label field) deserialize cleanly. Backward compat is hard requirement — Worker writes a unit test for "old config without label → Ok(Config { task.label: None, ... })".
10. **No `tracing` use in P004.** Stick to `eprintln!` per P002/P003 pattern. Tracing setup is a separate phiếu (likely Phase 1.6+).
11. **`tokio::process::Command`** (NOT `std::process::Command`) — Phase 1 acceptance + spawn prompt explicit. Compile error if Worker uses `std::process` in `runner.rs`.
12. **Use `chrono::Utc::now()` for `ts`** — NEVER `Local` or `SystemTime`. UTC RFC3339 = wire format invariant per ARCHITECTURE.md §Heartbeat schema row.
13. **`fs::create_dir_all` on heartbeat parent dir MUST be idempotent** — no `if exists` check first (race-prone; `create_dir_all` itself handles existing dir as Ok).
14. **Discovery Report MUST record:** (a) Anchor #1 + #7 + #11 + #25 actual file:line evidence (already captured in Turn 1 — Worker re-confirms or notes drift), (b) any new INV numbers assigned in INVARIANTS.md append (V2 pre-allocated: INV-14/15/16), (c) `default_for_home` literal update site (line number), (d) any test files that needed `TaskConfig { ... }` literal updates beyond what Architect specced, (e) V2 baseline correction outcome (post-P004 actual test count vs the 33 + N projection), (f) `default_config_path` `bail!` semantics — confirm Worker did NOT regress to silent `/` fallback.
15. **NO Hard Stop trigger** — phiếu authorizes only: 1 new dep (`serde_json`), 1 new config field (`task.label`), 2 new modules (`runner`, `heartbeat`), 1 new CLI flag (`run --config`), 2 doc files (INVARIANTS.md append, CHANGELOG entry). Anything else = HARD STOP, escalate via AskUserQuestion per CLAUDE.md §HARD STOPS section.
16. **`default_config_path` MUST `bail!` on `$HOME` unset.** Mirror `src/cli/register.rs:49-50` `home_dir()` pattern. NO silent `/` fallback (would either pollute filesystem root for root user or fail with a cryptic permission error for non-root — both are quiet-bug failure modes). Per V2 Turn 1 Architect Response resolution of Worker's Tầng 2 self-note.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings; binary < 7MB per PROJECT.md:60
- [ ] `cargo test --all` — **all 33+ pass** (V2 baseline correction per Turn 1 [O1.1]: 33 baseline = 20 lib unit + 13 integration; post-P004 = 33 + N new runner unit + heartbeat unit + cli_run integration + config new test for label field)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `git diff src/cli/mod.rs` — **empty** (Constraint #1 hard rule)
- [ ] `cargo update --dry-run` — no surprise major bump (Sub-mech E)

### Manual Testing
- [ ] **Echo task end-to-end:** create temp config with `command = "/bin/echo"`, `args = ["hello"]`, `label = "manual-test"`, `heartbeat.log_path = "/tmp/p004-manual.jsonl"`; run `cargo run --release -- run --config <temp>.toml`; expect exit 0; `cat /tmp/p004-manual.jsonl` shows 1 JSON line with all 6 fields, `label="manual-test"`, `exit_code=0`, `stdout_tail` contains "hello".
- [ ] **Failing task:** same config but `command = "/bin/sh", args = ["-c", "exit 7"]`; expect exit 4; heartbeat shows `exit_code=7`.
- [ ] **Spawn fail:** `command = "/nonexistent/binary"`; expect exit 4; heartbeat shows `exit_code=-1`, `stderr_tail` contains "spawn failed".
- [ ] **Missing config:** `cargo run -- run --config /tmp/does-not-exist.toml`; expect exit 2.
- [ ] **No `--config` (default path):** if `~/.config/advisory-cron/config.toml` exists from prior `advisory-cron init`, `cargo run -- run` uses it; otherwise exit 2 with helpful "config not found" message.
- [ ] **`$HOME` unset (V2 — Constraint #16 confirmation):** `env -u HOME cargo run -- run` (no `--config`); expect exit 2 with stderr containing "$HOME environment variable is not set" (or "...is empty"). Worker MUST run this — silent `/` fallback regression would slip past automated tests.
- [ ] **Heartbeat dir auto-create:** delete `~/.local/state/advisory-cron/` (or use temp path that doesn't exist); run; verify dir created + heartbeat written.

### Regression
- [ ] `cargo test --test cli_help` — 3/3 pass (P001 baseline)
- [ ] `cargo test --test cli_init` — 4/4 pass (P002 baseline; verify `task.label` field absence in older test configs doesn't break)
- [ ] `cargo test --test cli_register` — 6/6 pass (P003 baseline)
- [ ] `cargo test --lib config` — existing 9 tests pass + new tests for `task.label` default behavior
- [ ] `cargo test --all 2>&1 | grep 'test result'` — **aggregate 33+ pass** (V2 — guard against any lib unit regression invisible from per-suite checks)
- [ ] `cargo run -- init --force` — writes config including new `label = "advisory-cron"` line (or whatever default chosen)
- [ ] `cargo run -- register --label probe-p004 --schedule "0 9 * * *"` — P003 still works post-P004 (does NOT regress)
- [ ] `cargo run -- unregister --label probe-p004` — cleanup works

### Docs Gate
- [ ] `docs/CHANGELOG.md` — entry citing P004 ship, `serde_json` dep promotion, new `task.label` field, new `run --config` flag, new tests
- [ ] `docs/ARCHITECTURE.md` — §Modules table marks runner.rs + heartbeat.rs shipped 1.4 ✅; §Config schema Field reference adds `task.label` row + full TOML block updated; §CLI surface table `run` row Args column updated to show `--config <path>`; §Phase status updated to note Phase 1.4 shipped
- [ ] `docs/security/INVARIANTS.md` — 3 new INV entries appended (INV-14 child process spawn / INV-15 heartbeat file write / INV-16 JSON serialize boundaries) — REQUIRED per RULES.md:22
- [ ] `README.md` — defer Phase 1.6 (no edit required this phiếu)
- [ ] `docs-gate --all --verbose` — pass (changelog + architecture + tickets + discovery checks)

### Discovery Report
- [ ] `docs/discoveries/P004.md` — full report:
  - **Assumptions ĐÚNG:** list each verified anchor (#1, #2, #3, #4, #5, #6, #7, #8, #9, #10, #11, #12, #13, #14, #15, #16, #17, #18, #19, #20, #21, #24-V2, #25)
  - **Assumptions SAI / DRIFT:** list any anchor where actual code differs from V2 spec; ghi rõ doc fix made (V1's "13 tests" baseline already corrected in V2 — note in Discovery as the precedent for "spawn prompt summaries are not authoritative — always re-measure")
  - **Edge cases / limitations discovered:** record any quirk (e.g., specific exact `bail!` message in run.rs, specific mod ordering convention in main.rs, whether default_config_path was extracted as helper in register.rs, $HOME-unset test outcome)
  - **Docs đã update:** ARCHITECTURE.md sections updated + INV numbers assigned (INV-14/15/16) + CHANGELOG entry hash
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended at top: `- 2026-05-27 P004: Task runner + heartbeat shipped (serde_json explicit promotion, task.label field, run --config flag with bail-on-$HOME-unset, INV-14/15/16) → see docs/discoveries/P004.md`
- [ ] Sub-mechanism A-E Verification Trace filled (table above) — all rows ✅ or N/A

---

*Phiếu version V2 — Turn 1 Architect Response applied. [O1.1] ACCEPTED (Option A — Anchor #24 baseline corrected 13 → 33; Task 0 Step 4 + Verification Trace + Automated nghiệm thu updated). Worker Tầng 2 self-note on `$HOME` fallback ACCEPTED — Task 3 spec + Constraint #16 + Manual Testing case added. Awaiting Worker re-CHALLENGE (Turn 2) or consensus signal.*
