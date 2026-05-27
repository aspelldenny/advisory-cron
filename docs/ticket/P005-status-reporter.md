# PHIẾU P005: Status reporter (Phase 1.5)

> **Loại:** Feature
> **Tầng:** 1 (CLI flags added — `status --label`, `--json`, `--last`, `--config` = CLI surface change per RULES.md:14; LaunchctlClient trait extension `+ fn print` = internal public-surface addition; status rendering touches launchd read-boundary + heartbeat read-boundary. BACKLOG.md:26 originally said "Tầng 2 — just rendering" — Architect overrides: 4 new CLI flags + trait method = Tầng 1 by CLI surface trigger. Architect humility: when uncertain, default to Tầng 1 per CLAUDE.md AI bias section.)
> **Ưu tiên:** P0 (Phase 1 acceptance §`status` shows next fire + last heartbeat — gates Phase 1 ship gate `docs/PROJECT.md:56` `[verified]`)
> **Ảnh hưởng:** `src/cli/status.rs` (rewrite body + extend `Args` struct), `src/launchd.rs` (extend `LaunchctlClient` trait with `fn print`; implement on `RealLaunchctl` + `NoopLaunchctl`), `tests/cli_status.rs` (mới — integration tests)
> **Dependency:** P001 ✅ (CLI scaffold), P002 ✅ (config schema), P003 ✅ (launchd module + LaunchctlClient trait + current_uid), P004 ✅ (heartbeat module + `read_last_n` + `task.label` field)
> **Phiếu version:** V2 (Turn 1 RESPOND applied: bootstrap 1-arg sig fix + parse_next_fire pivots to descriptor Hour/Minute on macOS 15 — Worker empirical capture confirmed timestamp key absent)

---

## Context

### Vấn đề hiện tại

P004 ship `run` + heartbeat 2026-05-27. `status` vẫn `bail!()` stub per `src/cli/status.rs:15` (`[verified per Worker Turn 1]` — exact text: `bail!("\`status\` not yet implemented (Phase 1.5)")`). Phase 1 acceptance gate (`docs/PROJECT.md:56` `[verified]`):

- "advisory-cron status shows next fire time from `launchctl print` + reads last heartbeat."

ARCHITECTURE.md:187 `[verified]` cron mechanism section: "`status` `launchctl print gui/$UID/com.advisorycron.<label>` parses output for next fire time". ARCHITECTURE.md:65 `[verified]` CLI surface table `status` row: `Args = --json (machine output)`. P004 heartbeat schema `[verified]` already locked — P005 only CONSUMES via `heartbeat::read_last_n`.

**V2 update — what we now know empirically:** On Darwin 25.5.0 (macOS 15), `launchctl print` for a `StartCalendarInterval` plist does NOT output a "next fire" timestamp. Schedule data appears only as `event triggers → descriptor → { "Hour" => N "Minute" => M }`. The Phase 1 acceptance gate is satisfied semantically by rendering "daily at HH:MM" — derived from the configured recurrence, which launchd echoes back in `descriptor`. See V2 spec changes in §Giải pháp module 3 below + Anchor #28 + Anchor #31.

Reference: `docs/BACKLOG.md:26` `[verified]` — "Phase 1.5 — Status reporter. `status` subcommand: parse `launchctl print gui/$UID/<label>` for next fire time, read last N lines of `heartbeat.jsonl`, render to stdout (table or simple text). Handle 'plist not loaded' + 'no heartbeats yet' cases cleanly. Tầng 2 (no schema change, just rendering). ~100 LOC."

**Tầng escalation rationale (Architect note):** BACKLOG says Tầng 2 anticipating "rendering only". Actual scope adds:
1. 4 new CLI flags (`--label`, `--json`, `--last`, `--config`) on `status` subcommand → RULES.md:14 Tầng 1 trigger ("CLI flag added/removed/renamed → ARCHITECTURE.md §CLI surface").
2. New method `fn print(&self, label: &str) -> Result<String>` on `LaunchctlClient` trait — additive, but a public surface extension to a P003 module.

Both are additive (no breaking change), but the Tầng 1 envelope ensures CHALLENGE round catches anchor drift before EXECUTE — which matters here because `launchctl print` output format is system-version dependent (not specced in any file we control). **V2 evidence:** the CHALLENGE round did exactly that — Worker probed real launchctl output and caught the timestamp-key absence. Tầng 1 classification justified ex post.

### Giải pháp

**2 module edits + 1 trait extension + 1 new integration test file + ARCHITECTURE.md update.**

**1. `src/launchd.rs` — extend `LaunchctlClient` trait with `print` method.**

```rust
pub trait LaunchctlClient {
    // existing methods: bootstrap, bootout (P003) — DO NOT change signatures
    fn print(&self, label: &str) -> Result<LaunchctlPrintOutput>;
}

pub struct LaunchctlPrintOutput {
    /// Full stdout captured from `launchctl print gui/<uid>/com.advisorycron.<label>`.
    pub raw_stdout: String,
    /// True when stderr indicated "Could not find service" (label not loaded).
    /// Caller renders "not loaded" status instead of attempting to parse.
    pub not_loaded: bool,
}
```

- **Trait addition is additive** — existing P003 trait method signatures (`bootstrap`, `bootout`) UNCHANGED. P003 `NoopLaunchctl` test impl gains a `print` method returning a canned string matching the macOS 15 descriptor format (so unit tests exercise the real parser path).
- **`RealLaunchctl::print`:** shells `launchctl print gui/<uid>/com.advisorycron.<label>` via `std::process::Command::new("launchctl").arg("print").arg(format!("gui/{uid}/com.advisorycron.{label}"))`. Capture `output.stdout` (decoded via `String::from_utf8_lossy`). When `output.status.code() != Some(0)` AND `output.stderr` contains "Could not find service" OR "No such process" substring → set `not_loaded = true`, `raw_stdout = String::new()`. Other non-zero exits → return `Err` (real launchctl error).
- **`uid` source:** reuse existing `current_uid()` helper from P003 (Anchor #4 `[verified]` — `src/launchd.rs::current_uid()` returns `u32`). NO new shell-out.
- **INV-10 / INV-11 compliance:** new `print` shell-out passes label as a discrete `.arg()` (no shell interpolation); `uid: u32` enforces numeric type; label MUST be pre-sanitized by caller — same allowlist as `bootstrap`/`bootout` callsites. P005 status caller validates label via the same `is_ascii_alphanumeric() || c == '-' || c == '_'` check used in P003 `generate_plist` (extract or re-inline — Worker self-decide, Tầng 2 stylistic).

**2. `src/cli/status.rs` — rewrite body + extend `Args` struct.**

```rust
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Label to query. If omitted, falls back to config-derived label.
    #[arg(long)]
    pub label: Option<String>,

    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Output as JSON (machine-readable). Default: human-readable text.
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Number of recent heartbeats to show. Default: 5.
    #[arg(long, default_value_t = 5)]
    pub last: usize,
}
```

Body steps:

1. Resolve config path: same `default_config_path()` pattern as P004 `src/cli/run.rs` — `bail!` on `$HOME` unset (Constraint #16 from P004 generalized). Inline the helper, NO `src/cli/mod.rs` edit.
2. Load config: `Config::load(&config_path)` → exit 2 on failure per ARCHITECTURE.md:74.
3. Resolve label: `args.label.clone().or_else(|| config.task.label.clone()).unwrap_or_else(|| "advisory-cron".to_string())`. Priority: CLI flag > config field > default literal.
4. Validate label per INV-12 (ASCII alphanumeric + `-` + `_`). On invalid → exit 1 with helpful error. **Architect decision: validate here AND inside `LaunchctlClient::print` impl (defense-in-depth — same 2-point pattern as register).**
5. Query launchctl: `let print_result = client.print(&label)?;` where `client` is `RealLaunchctl` (unit struct, no `::new()` — V2 fix per Worker Anchor #5 self-correct).
6. Parse next-fire schedule: `parse_next_fire(&print_result.raw_stdout) -> Option<String>` (helper in `src/cli/status.rs` private fn). **V2: parses macOS 15 descriptor block (`"Hour" => N` + `"Minute" => M`); returns `Some("daily at HH:MM")`.** If `print_result.not_loaded` → skip parse, render "not loaded".
7. Read heartbeats: `heartbeat::read_last_n(&config.heartbeat.log_path, args.last)`. Empty Vec → "No heartbeats yet" branch.
8. Render: if `args.json` → `serde_json::to_string_pretty(&StatusReport { ... })?`; else → human-readable text.
9. **Exit code: always 0.** Status is read-only — even "not loaded" or "no heartbeats" is a valid state to report, not an error. Caller pipes to `grep` etc. for status-conditional logic.

**3. `parse_next_fire` helper — V2 spec (descriptor-based, evidence-driven).**

V1 specced 4 timestamp-key candidates (`next fire = ...`, `next launch = ...`, etc.) based on guesses about macOS launchctl output. **V2 — Worker empirical capture (Debate Log Turn 1 [O1.2]) proved zero such key exists on macOS 15.** What launchctl DOES expose for `StartCalendarInterval` jobs is the configured recurrence in `event triggers → descriptor`:

```
event triggers = {
    com.advisorycron.<label>.<some-id> => {
        ...
        stream = com.apple.launchd.calendarinterval
        descriptor = {
            "Minute" => 0
            "Hour" => 9
        }
    }
}
```

This is the configured recurrence (not a next-fire timestamp), but it is sufficient to satisfy the Phase 1 acceptance gate as "Next fire: daily at 09:00" — honest about the source (configured recurrence echoed by launchd) and useful for the user.

**V2 parser contract:**
- Scan lines for `"Hour" =>` and `"Minute" =>` patterns inside the `descriptor` block.
- Extract numeric values; format as `"daily at HH:MM"` (zero-padded).
- If only Hour found → `"daily at HH:00"`. If only Minute → `"hourly at :MM"`. If neither → return `None` (caller renders "unknown").
- Multiple descriptor blocks (multiple triggers) → use first match. P005's `register` only creates one trigger; future multi-trigger work re-evaluates.

**Failure mode (V2):** Parser returns `None` only when neither `"Hour" =>` nor `"Minute" =>` line is present in the loaded plist's launchctl output. On macOS 15 + this project's `StartCalendarInterval` plists, both are always present → parser returns `Some` consistently. On future macOS versions where output format drifts, parser returns `None` → renders "unknown (launchctl format not recognized)" instead of crashing. Honest > confident-wrong.

**`[needs Worker verify]` cleared** — Anchor #28 capture in Debate Log Turn 1 is the authoritative fixture; Worker re-uses that exact snippet as the unit test fixture in Task 2 below.

**4. Human-readable render format (Architect spec, Worker flexible on whitespace):**

```
advisory-cron status — label: <label>
  Plist: <loaded | not loaded>
  Next fire: <daily at HH:MM | unknown | n/a (not loaded)>

Recent heartbeats (last <N>):
  [<ts>] exit=<code> duration=<ms>ms
      stdout: <first 80 chars of stdout_tail, or "(empty)">
      stderr: <first 80 chars of stderr_tail, or "(empty)">
  ...
```

If heartbeats empty:
```
Recent heartbeats: No heartbeats yet (no fires recorded at <heartbeat.log_path>)
```

If `stdout_tail` or `stderr_tail` is empty string → render `(empty)`. Heads-up #3 from spawn prompt addressed.

**5. JSON render format:**

```rust
#[derive(Serialize)]
struct StatusReport {
    label: String,
    plist_loaded: bool,
    next_fire: Option<String>,  // None if not loaded or unrecognized format
    heartbeat_log_path: String,
    last_runs: Vec<HeartbeatRecord>,  // reuse type from heartbeat.rs (already Serialize)
}
```

Output via `serde_json::to_string_pretty(&report)?` (pretty-print for human-readable JSON — common convention; machine parsers handle whitespace).

**6. `tests/cli_status.rs` — integration tests (4-5 cases).**

Test pattern mirrors `tests/cli_run.rs` (P004 Anchor #6 `[verified]` — `tempfile` + binary spawn). Cases:

- (a) Heartbeats exist, plist NOT loaded → exit 0, output contains "not loaded" + heartbeat ts strings.
- (b) Heartbeats absent (file does not exist), plist NOT loaded → exit 0, output contains "No heartbeats yet" + "not loaded".
- (c) `--json` mode with heartbeats absent → exit 0, stdout parseable as JSON with `last_runs: []`.
- (d) `--last 3` flag clamps output to 3 records (write 5 heartbeats via direct file injection; verify only 3 rendered).
- (e) Missing config → exit 2 (same as `run --config /bogus.toml` from P004 Task 7 case (d)).

**No integration test exercises `RealLaunchctl::print` against a real loaded plist** — that requires CI macOS runner + `launchctl bootstrap` side-effect. Acceptable: `parse_next_fire` unit-tested against fixture string in Task 4 (Worker uses Debate Log Turn 1 captured fixture as the authoritative sample).

### Heads-up resolutions

**Heads-up #1 — Tier escalation (Tầng 2 → Tầng 1):**

Spawn prompt asked Architect to confirm Tầng 2 or escalate. **Decision: Tầng 1.** Two triggers:

1. **CLI surface change** — `status` gains 4 new flags (`--label`, `--json`, `--last`, `--config`). RULES.md:14 `[verified]` row: "CLI flag added/removed/renamed → ARCHITECTURE.md §CLI surface". This is mechanically Tầng 1.
2. **Trait public surface extension** — `LaunchctlClient::print` is additive (no break) but is a new public API on a P003 module. CLAUDE.md `[verified]` AI bias section: "Default uncertainty → Tầng 1."

Cost of "wrong-side" Tier classification:
- **Wrong: marked Tầng 2 when actually Tầng 1** → skips CHALLENGE → anchor drift on `launchctl print` output format (which Architect CANNOT specify from docs alone) → Worker codes against guessed format → silent bug.
- **Wrong: marked Tầng 1 when actually Tầng 2** → adds one CHALLENGE round-trip (~10 min). Worker probes `launchctl print` actual output, codes against real format. Better failure mode.

Tầng 1 it is. **V2 evidence:** CHALLENGE round caught Anchor #28 timestamp-key absence empirically — exactly the failure mode the Tầng 1 envelope is designed to prevent. Validated ex post.

**Heads-up #2 — `fire_task` no timeout (PR#4 advisory note):**

Per spawn prompt: IRRELEVANT to status (read-only). Skip. P005 does NOT touch `runner::fire_task`. The `tokio::process::Command` use in P005 is `launchctl print` which exits quickly (no hang risk — system command). INV-2 timeout requirement noted but `launchctl print` is local IPC, not external HTTP, so trade-off is acceptable for Phase 1.

**Heads-up #3 — Empty `stdout_tail` / `stderr_tail` render:**

Per spawn prompt: status render shows `(empty)` instead of literal empty string. Spec'd in §Giải pháp module 4 above. Worker writes test case verifying this.

**Heads-up #4 — `default_config_path` `bail!` on `$HOME` unset:**

Per P004 Constraint #16 `[verified]` — reuse same inlined `default_config_path` pattern as `src/cli/run.rs`. Spec'd in §Giải pháp module 2 step 1. New Constraint #1 below.

**Heads-up #5 — `src/cli/mod.rs` MUST NOT be edited:**

Newtype dispatch `Commands::Status(status::Args)` already forwards `Args` opaquely. New flags declared INSIDE `status::Args` propagate via clap derive — same proven pattern from P003 (register --label/--schedule/--config), P004 (run --config). NO `mod.rs` edit. New Constraint #2 below.

### Scope

- CHỈ tạo/sửa:
  - `src/cli/status.rs` (rewrite body + extend `Args` struct với `label/config/json/last`)
  - `src/launchd.rs` (extend `LaunchctlClient` trait + add `print` to `RealLaunchctl` + `NoopLaunchctl`; add `LaunchctlPrintOutput` struct)
  - `tests/cli_status.rs` (mới — 4-5 integration tests)
  - `docs/ARCHITECTURE.md` (§CLI surface `status` row Args column update; §Modules table mark status.rs shipped 1.5; §Phase status update; **V2: note descriptor-based parser** in launchd.rs row)
  - `docs/CHANGELOG.md` (P005 entry citing CLI flags + trait extension + tests; NO new dep; **V2: cite descriptor-Hour/Minute parser pivot**)
  - `docs/discoveries/P005.md` (mới — Discovery Report)
  - `docs/DISCOVERIES.md` (1-line index entry)
  - `docs/security/INVARIANTS.md` (append INV-17 for `launchctl print` shell-out boundary — additive, follows INV-10/INV-11 pattern)
- KHÔNG sửa:
  - `src/cli/mod.rs` — NEWTYPE DISPATCH. `Commands::Status(status::Args)` already wraps `Args` opaquely. **`git diff src/cli/mod.rs` post-EXECUTE MUST be empty.** (P003 V2 Turn 1 [O1.1] hard rule, generalized.)
  - `src/cli/init.rs`, `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/run.rs` (P002/P003/P004 territory)
  - `src/config.rs` — no schema change. P005 only READS `config.task.label` (added P004) and `config.heartbeat.log_path` (added P002).
  - `src/runner.rs`, `src/heartbeat.rs` — P004 modules. P005 only CALLS `heartbeat::read_last_n` (existing signature).
  - `Cargo.toml` — NO new dep. `serde_json` already explicit (P004), `chrono` already direct (P002), `clap` already present. Status rendering uses only existing deps.
  - `Cargo.lock` — auto-regenerated; Worker verifies no surprise major bump (Sub-mech E).
  - `tests/cli_help.rs`, `tests/cli_init.rs`, `tests/cli_register.rs`, `tests/cli_run.rs` (P001-P004 regression — must continue passing unmodified, except potentially `cli_help` if substring-matches on `status --help` body — Worker check)
  - `README.md` (defer Phase 1.6)
  - `.phieu-counter` (Quản đốc bumped 004 → 005 already, confirmed Read at `005\n`)
- KHÔNG tạo: `src/alert.rs`, `src/retry.rs` (Phase 2), `src/core/`, `src/mcp/` (Phase 1.7)

### Skills consulted (optional)

*(Orchestrator chưa chạy skill nào cho phiếu này. Verification dựa Read docs + P004 phiếu V2 lessons captured in Anchor table.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Mỗi anchor carry humility marker. `[verified]` = em đã Read file confirm. `[unverified]` = docs imply, em chưa Read source. `[needs Worker verify]` = punt cho Thợ grep.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/status.rs` hiện stub `bail!("...not yet implemented (Phase 1.5)")` hoặc tương đương | Read `src/cli/status.rs` | `[needs Worker verify]` | ✅ Worker Turn 1: `src/cli/status.rs:15` — `bail!("\`status\` not yet implemented (Phase 1.5)")`. Exact text confirmed. |
| 2 | `src/cli/mod.rs` uses NEWTYPE dispatch `Commands::Status(status::Args)` per P003 V2 lesson Turn 1 [O1.1] + P004 Anchor #11 `[verified]` confirming `Run(run::Args)` follows same pattern | Read `src/cli/mod.rs` | `[needs Worker verify]` | ✅ Worker Turn 1: `src/cli/mod.rs:25` — `Status(status::Args)`. Dispatch at line 34: `Commands::Status(args) => status::run(args).await`. Zero mod.rs edit needed. |
| 3 | `src/launchd.rs` `LaunchctlClient` trait exists with `bootstrap` + `bootout` methods (P003 ship) — signature shape preserved verbatim | P003 phiếu + ARCHITECTURE.md:47 `[verified]` | `[unverified]` | ✅ **V2 RESOLVED.** Worker Turn 1 confirmed trait at `src/launchd.rs:152-162`. Actual signatures: `fn bootstrap(&self, plist_path: &Path) -> Result<()>` (**1 arg**; domain computed internally per P003 V2 doctrine), `fn bootout(&self, label: &str) -> Result<()>`. V1 spec's `bootstrap` showed wrong 2-arg form — corrected in V2 Task 1 code block below. |
| 4 | `src/launchd.rs::current_uid() -> Result<u32>` exists (P003 ship) | ARCHITECTURE.md:189 `[verified]` + INV-11 `[verified per Read INVARIANTS.md:127]` | `[verified]` | ✅ INV-11 implementation note states `src/launchd.rs::current_uid()` returns `u32`. P005 `RealLaunchctl::print` reuses. Worker Turn 1 confirmed at `src/launchd.rs:238`. |
| 5 | `src/launchd.rs::RealLaunchctl` struct exists with `LaunchctlClient` impl (P003 ship — bootstrap + bootout shell-outs) | ARCHITECTURE.md:47 `[verified]` + INV-10 implementation note `[verified per Read INVARIANTS.md:111]` | `[verified]` | ✅ INV-10 impl note: `RealLaunchctl` uses `Command::new("launchctl").arg("bootstrap")...`. Worker Turn 1: `src/launchd.rs:165` — `pub struct RealLaunchctl;` (**unit struct, instantiated as `RealLaunchctl` not `::new()`** — V2 Task 1+2 code corrected). |
| 6 | `src/launchd.rs::NoopLaunchctl` test impl exists (P003 ship — records calls for test assertions) | DISCOVERIES.md P003 entry `[verified per Read DISCOVERIES.md:15]` "LaunchctlClient trait + NoopLaunchctl" | `[verified]` | ✅ Worker Turn 1: `src/launchd.rs:216-233` — `NoopLaunchctl` struct with `bootstrap_calls` + `bootout_calls` Mutex fields. **Constructor is `NoopLaunchctl::default()` (NOT `::new()`)** — V2 Task 1 unit test corrected. |
| 7 | `src/heartbeat.rs::read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>` exists (P004 ship) | P004 phiếu V2 §Giải pháp module 2 + P004 Anchor table | `[verified]` | ✅ Confirmed in P004 phiếu Task 2 code block. Returns Vec oldest-first; empty Vec on missing file; skips malformed lines. P005 calls directly. |
| 8 | `src/heartbeat.rs::HeartbeatRecord` struct derives `Serialize + Deserialize` (P004 ship — durable schema) | P004 phiếu V2 §Giải pháp module 2 | `[verified]` | ✅ P004 spec: `#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]`. P005 reuses `HeartbeatRecord` in `StatusReport` JSON render directly — no new derive. |
| 9 | `src/config.rs::TaskConfig.label: Option<String>` exists (P004 ship) | P004 phiếu V2 Task 4 | `[verified]` | ✅ P004 ship per CHANGELOG `[verified per Read CHANGELOG:26-27]`. P005 uses `config.task.label.clone().unwrap_or_else(\|\| "advisory-cron".to_string())`. |
| 10 | `src/config.rs::HeartbeatConfig.log_path: PathBuf` exists (P002 ship) | ARCHITECTURE.md:115 `[verified]` + P002 phiếu | `[verified]` | ✅ Field confirmed. P005 reads `config.heartbeat.log_path` to pass to `heartbeat::read_last_n`. |
| 11 | `src/cli/run.rs::default_config_path() -> Result<PathBuf>` with `bail!` on `$HOME` unset exists (P004 ship — Constraint #16) | P004 phiếu V2 Task 3 code block | `[verified]` | ✅ P004 Task 3 code spec includes this. **Architect note:** P005 INLINES the same helper in `src/cli/status.rs` (private fn). DOES NOT extract a shared `cli::home_dir()` helper — that would require `src/cli/mod.rs` edit (Constraint #2 violation). Worker may copy-paste the function body from `src/cli/run.rs`. Tầng 2 stylistic — code duplication acceptable here since extraction violates the harder Constraint #2. |
| 12 | `Cargo.toml [dependencies]` already has `serde_json = "1"` (P004 ship) | DISCOVERIES.md P004 entry `[verified per Read DISCOVERIES.md:11]` "serde_json explicit dep" | `[verified]` | ✅ Confirmed. P005 uses `serde_json::to_string_pretty` for `--json` mode — no new dep needed. |
| 13 | `Cargo.toml [dependencies]` `chrono` already direct dep with `serde` feature (P002 ship) | P004 Anchor #3 `[verified]` `Cargo.toml:17` | `[verified]` | ✅ `chrono = { version = "0.4", features = ["serde"] }`. P005 only reads `HeartbeatRecord` (already chrono-serde'd). |
| 14 | `Cargo.toml [dev-dependencies]` has `tempfile = "3"` (for integration test) | P004 Anchor #6 `[verified]` `Cargo.toml:26-27` | `[verified]` | ✅ `tempfile = "3"` confirmed at `Cargo.toml:26`. P005 reuses for `tests/cli_status.rs`. |
| 15 | ARCHITECTURE.md §CLI surface table line 65 `status` row Args = `--json (machine output)` | Read `docs/ARCHITECTURE.md:65` | `[verified]` | ✅ Line 65 confirmed. P005 Task 5 (Docs Gate) extends to `--label / --config / --json / --last` per Args struct. |
| 16 | ARCHITECTURE.md §Modules table line 41 `src/cli/status.rs` Phase column = "1.1 skeleton ✅ → impl 1.5" | Read `docs/ARCHITECTURE.md:41` | `[verified]` | ✅ Line 41 confirmed. P005 Task 5 updates to "1.5 ✅". |
| 17 | ARCHITECTURE.md §Phase status line 277 mentions Phase 1.4 shipped + "Phases 1.5–1.7 pending" | Read `docs/ARCHITECTURE.md:277` | `[verified]` | ✅ Line 277 confirmed. P005 Task 5 updates to mark Phase 1.5 shipped. |
| 18 | ARCHITECTURE.md §Cron mechanism line 187 documents `launchctl print gui/$UID/com.advisorycron.<label>` as the status command | Read `docs/ARCHITECTURE.md:187` | `[verified]` | ✅ Line 187 confirmed. P005 `RealLaunchctl::print` matches this spec exactly. |
| 19 | ARCHITECTURE.md exit code 0 = "Success" (status always exits 0, even for "not loaded" / "no heartbeats" — read-only operation) | Read `docs/ARCHITECTURE.md:72` | `[verified]` | ✅ Confirmed. Architect decision: status exit code = 0 always (constraint #6). Exceptions: exit 2 for config-load failure (ARCHITECTURE.md:74); exit 1 for invalid label per INV-12 (rare path). |
| 20 | RULES.md Tầng 1 row "CLI flag added/removed/renamed → ARCHITECTURE.md §CLI surface" | Read `docs/RULES.md:14` | `[verified]` | ✅ Line 14 confirmed. P005 adds `--label / --config / --json / --last` to `status` → Tầng 1 trigger. |
| 21 | RULES.md Tầng 1 row "Security boundary touched (env var read, file write outside `.sos-state/`...) → AUTO Tầng 1 + INVARIANTS.md" | Read `docs/RULES.md:22` | `[verified]` | ✅ Confirmed. P005 reads `$HOME` env var (in `default_config_path`) + spawns child process (`launchctl print`). INV-17 append required. |
| 22 | INV-10 spec (launchctl shell-out: absolute path args only, no user-string interpolation) applies to new `launchctl print` shell-out | Read `docs/security/INVARIANTS.md:107-117` | `[verified]` | ✅ Confirmed. INV-10 trigger is "any future `launchctl` invocation". P005 `RealLaunchctl::print` MUST comply — pass label as discrete `.arg()`, never `format!` into a shell command. |
| 23 | INV-12 spec (label sanitization: ASCII alphanumeric + `-` + `_` only; enforced at 2 points) applies to status `--label` flag | Read `docs/security/INVARIANTS.md:137-153` | `[verified]` | ✅ Confirmed. P005 status accepts `--label` from CLI → MUST validate same allowlist at 2 points: (a) pre-flight in `src/cli/status.rs::run` before passing to `client.print(...)`, (b) inside `RealLaunchctl::print` (defense-in-depth — same pattern as `generate_plist`). |
| 24 | INV-13 spec (plist write boundary — `~/Library/LaunchAgents/com.advisorycron.*.plist` only) does NOT apply to P005 — status does NOT write plists | Read `docs/security/INVARIANTS.md:157-172` | `[verified]` | ✅ Confirmed. P005 is read-only against launchd; never invokes `fs::write` on a plist. |
| 25 | INV-14/15/16 (from P004) do NOT apply to P005 — status does NOT spawn user-config commands, does NOT write heartbeats, does NOT serialize uncontrolled stdout/stderr into JSON. P005 ONLY reads existing heartbeats + spawns `launchctl print` (system command, not user-config) | Read `docs/security/INVARIANTS.md:176-228` | `[verified]` | ✅ Confirmed. P005 needs ONE new INV (INV-17 for `launchctl print` shell-out boundary — additive); existing INVs unaffected. |
| 26 | Current max INV in INVARIANTS.md = INV-16 (P004 ship) — new entry will be INV-17 | DISCOVERIES.md P004 entry "INV-14/15/16 appended" `[verified per Read DISCOVERIES.md:11]` | `[verified]` | ✅ Worker Turn 1: `grep -c "^### INV-" docs/security/INVARIANTS.md` → **16**. Max INV = INV-16. INV-17 slot confirmed available. |
| 27 | Test baseline (post-P004 ship) = **51 tests total** (34 lib unit + 17 integration: 3 cli_help + 4 cli_init + 6 cli_register + 4 cli_run) per P004 CHANGELOG `[verified per Read CHANGELOG:37]` | Read `docs/CHANGELOG.md:37` | `[verified]` | ✅ Worker Turn 1: `cargo test --all` → 34 lib unit + 3 cli_help + 4 cli_init + 6 cli_register + 4 cli_run = **51 tests, all pass**. Baseline confirmed 2026-05-27. |
| 28 | `launchctl print gui/$UID/com.advisorycron.<label>` output format is system-version dependent — NOT specced in any doc Architect can read. The exact key holding next-fire timestamp varies by macOS version (14 vs 15 vs older) | Apple launchd documentation drift — not version-controlled in this repo | `[needs Worker verify]` | ✅ **V2 RESOLVED — strategy pivoted.** Worker Turn 1 empirical probe on Darwin 25.5.0 (macOS 15): `launchctl print` outputs NO "next fire", "next launch", "run at", or timestamp key for `StartCalendarInterval` jobs. Schedule data appears only as `event triggers → descriptor → { "Hour" => N "Minute" => M }`. **V2 strategy:** `parse_next_fire` parses `descriptor` block's Hour/Minute and renders "daily at HH:MM" (configured recurrence — honestly framed, satisfies acceptance gate semantically). Fixture captured in Debate Log Turn 1 used as authoritative unit test input. See new Anchor #31 for full rationale + linked Discovery Report. |
| 29 | `Args` struct uses `#[derive(clap::Args)]` (NOT `Parser`) — matches newtype dispatch pattern (P003/P004 verified) | P004 Task 3 spec `[verified]` + P003 register/unregister precedent | `[verified]` | ✅ Pattern: every per-subcommand `Args` struct uses `clap::Args` derive; `cli/mod.rs::Commands` enum wraps each variant as `<Cmd>(<cmd>::Args)`. P005 follows. Worker Turn 1 confirmed existing `status::Args` (alias `ClapArgs`). |
| 30 | `stdout_tail` / `stderr_tail` are `String` (may be empty); status render should show `(empty)` for empty strings per Heads-up #3 | P004 phiếu V2 §Giải pháp module 2 HeartbeatRecord struct | `[verified]` | ✅ Both fields `String`. Empty string is valid value (e.g., echo task with no stderr → `stderr_tail = ""`). P005 status render converts empty → `(empty)` (UX clarity). Worker writes integration test verifying. |
| 31 | **V2 NEW — Worker empirical capture (Turn 1):** macOS 15 `launchctl print` for `StartCalendarInterval` jobs exposes the configured recurrence as `descriptor = { "Minute" => M "Hour" => H }` nested inside `event triggers → <id> → descriptor`. The `descriptor` block always contains the configured Hour/Minute values that the plist's `StartCalendarInterval` was registered with. NO next-fire timestamp key is exposed. | Worker Turn 1 fixture: `cargo run -- register --label probe-p005 --schedule "0 9 * * *"` + `launchctl print gui/$(id -u)/com.advisorycron.probe-p005` (full 30-line snippet in Debate Log) | `[verified per Worker Turn 1]` | ✅ Empirical capture confirmed Hour/Minute keys present in expected format. `parse_next_fire` (V2) parses these two keys and formats `"daily at HH:MM"`. Robust across macOS versions as long as launchctl echoes `StartCalendarInterval` config keys (Apple has done this since launchd shipped). Future drift mitigated by `None` fallback → "unknown" render. |

**Anchors flagged for Worker Task 0 priority:** ~~#1, #2, #3, #6, #28, #30~~ — **V2 all resolved during Turn 1 CHALLENGE.** Worker re-reads only #1/#2/#3/#5/#6 in Task 0 to log file:line evidence for Debug Log (defensive — fast check, ~30 sec).

**Anchors flagged as `[needs Worker verify]`:** ~~#1, #2, #28~~ — **V2 all resolved.** All anchors now `[verified]` or `[verified per Worker Turn 1]`.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi Quản đốc triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (Turn 1 RESPOND applied)

### Turn 1 — Worker Challenge

**Date:** 2026-05-27

**Anchor verification:**

- Anchor #1: ✅ `src/cli/status.rs:15` — `bail!("\`status\` not yet implemented (Phase 1.5)")`. Exact line confirmed.
- Anchor #2: ✅ `src/cli/mod.rs:25` — `Status(status::Args)` newtype dispatch confirmed. `dispatch()` at line 34. Zero mod.rs edit required.
- Anchor #3: ⚠️ **MISMATCH — see [O1.1] below.**
- Anchor #4: ✅ `src/launchd.rs:238` — `pub fn current_uid() -> Result<u32>` confirmed.
- Anchor #5: ✅ `src/launchd.rs:165` — `pub struct RealLaunchctl;` (unit struct, no `::new()`). `impl LaunchctlClient for RealLaunchctl` at line 167.
- Anchor #6: ✅ `src/launchd.rs:216` — `pub struct NoopLaunchctl { bootstrap_calls, bootout_calls }`. Constructor is `NoopLaunchctl::default()` (NOT `::new()`). Phiếu Task 1 test code uses `NoopLaunchctl::new()` — Worker must correct to `NoopLaunchctl::default()` in EXECUTE. Tầng 2 stylistic, noted.
- Anchor #7: ✅ (P004 ship confirmed, not re-read for brevity — trusted from CHANGELOG).
- Anchor #8–#15: ✅ Trusted from CHANGELOG/DISCOVERIES P002-P004 entries.
- Anchor #16: ✅ `src/cli/status.rs` stub Phase column update needed in ARCHITECTURE.md.
- Anchor #17, #18, #19, #20, #21, #22, #23, #24, #25: ✅ Docs-referenced anchors trusted.
- Anchor #26: ✅ `grep -c "^### INV-" docs/security/INVARIANTS.md` → **16**. Max is INV-16. INV-17 slot confirmed available.
- Anchor #27: ✅ Baseline confirmed: `cargo test --all` → 34 unit + 3 cli_help + 4 cli_init + 6 cli_register + 4 cli_run = **51 tests, all pass**.
- Anchor #28: ❌ **CRITICAL STRUCTURAL MISMATCH — see [O1.2] below.**
- Anchor #29: ✅ `#[derive(clap::Args)]` pattern confirmed in existing `status::Args` (alias `ClapArgs`).
- Anchor #30: ✅ `stdout_tail` / `stderr_tail` are `String` in `HeartbeatRecord`. Render `(empty)` spec confirmed.

**Objections (Tầng 1 only):**

**[O1.1] — `LaunchctlClient::bootstrap` signature mismatch**

Phiếu Task 1 spec (`src/launchd.rs:390-391`) shows:
```
fn bootstrap(&self, domain: &str, plist_path: &Path) -> Result<()>;
```

Actual code at `src/launchd.rs:154`:
```
fn bootstrap(&self, plist_path: &Path) -> Result<()>;
```

The actual trait takes ONE arg (`plist_path`), not two. `domain` is computed internally via `current_uid()` at `src/launchd.rs:169-170`. The phiếu's annotated comment block in Task 1 shows the wrong signature for `bootstrap`. If Worker uses the phiếu's code block verbatim, it will break the existing P003 trait implementors.

The `bootout` signature IS correct: `fn bootout(&self, label: &str) -> Result<()>` matches `src/launchd.rs:161`.

**Impact:** Task 1 code block in phiếu is wrong on the `bootstrap` line. Worker must preserve the real signature. This is Tầng 1 because it is a public trait surface — the comment block says "signature from Anchor #3 Task 0 capture — Worker preserves exact form" but the phiếu itself has the wrong form embedded.

**Proposed resolution:**

- A. (Recommended) Architect acknowledges mismatch; confirms Worker should use actual `src/launchd.rs:154` signature verbatim when extending the trait in Task 1. Phiếu V2 corrects the comment block in Task 1 to show `fn bootstrap(&self, plist_path: &Path) -> Result<()>`. No other scope change needed.
- B. No phiếu edit needed — Worker self-corrects based on this Debate Log entry. Tầng 2 clarification. (Worker leans A — phiếu V2 correction is cleaner audit trail.)

---

**[O1.2] — `launchctl print` does NOT output any next-fire timestamp on macOS 15 (Darwin 25.5.0)**

Worker ran `cargo run -- register --label probe-p005 --schedule "0 9 * * *"` to load a real `StartCalendarInterval`-based plist, then captured `launchctl print gui/$(id -u)/com.advisorycron.probe-p005` full stdout (see fixture below). The output contains NO "next fire", "next launch", "run at", or any timestamp key.

The schedule data appears ONLY as nested descriptor values:
```
event triggers = {
    com.advisorycron.probe-p005.268435522 => {
        stream = com.apple.launchd.calendarinterval
        descriptor = {
            "Minute" => 0
            "Hour" => 9
        }
    }
}
```

This is the CONFIGURED schedule (Hour=9, Minute=0), not a next-fire timestamp. macOS 15 launchd does not expose a "next scheduled fire time" in `launchctl print` output for `StartCalendarInterval` jobs — it only exposes the recurrence parameters.

**Impact on phiếu spec:**

The `parse_next_fire` function cannot return a useful next-fire time even when the plist is loaded — it will always return `None` on this macOS version (25.5.0). The status output will always show "next fire: unknown (launchctl format not recognized)" for loaded plists.

This makes the Phase 1 acceptance gate ("status shows next fire time from `launchctl print`") semantically non-satisfiable on macOS 15.

**Proposed resolutions:**

- A. (Recommended) Architect updates `parse_next_fire` spec to additionally parse the `descriptor { "Hour" => N "Minute" => M }` pattern from `event triggers`. Status renders "Next fire: daily at HH:MM" (derived from configured hour/minute) instead of a timestamp. This is honest — "the service fires daily at 09:00" — and satisfies the acceptance gate semantically (we show the schedule). `NoopLaunchctl` canned output in Task 1 updated to include `"Hour" => 9\n\t\t\t\t"Minute" => 0` in the descriptor block to match real fixture format. Worker codes the alternative parser against this real fixture.
- B. Keep `parse_next_fire` as spec'd (try the 4 timestamp keys), accept it will always return `None` on macOS 15, render "unknown". Status is still useful because the heartbeat list is the primary value. Accept gate as "partial" — Phase 1 ship gates on heartbeat-list rendering being correct; next-fire parsing is best-effort with documented limitation in Discovery Report.

Worker leans A — "daily at 09:00" is genuinely more useful than "unknown" and is honest about what launchd exposes. However, if Architect prefers B (simpler, stay in original scope), Worker accepts — the `None` fallback in the phiếu's code is already robust.

**Fixture captured (full stdout for `com.advisorycron.probe-p005` on Darwin 25.5.0):**

```
gui/501/com.advisorycron.probe-p005 = {
	active count = 0
	path = /Users/nguyenhuuanh/Library/LaunchAgents/com.advisorycron.probe-p005.plist
	type = LaunchAgent
	state = not running
	program = /Users/nguyenhuuanh/advisory-cron/target/release/advisory-cron
	arguments = {
		/Users/nguyenhuuanh/advisory-cron/target/release/advisory-cron
		run
	}
	working directory = /Users/nguyenhuuanh
	stdout path = /tmp/advisory-cron-probe-p005.stdout.log
	stderr path = /tmp/advisory-cron-probe-p005.stderr.log
	domain = gui/501 [100015]
	runs = 0
	last exit code = (never exited)
	event triggers = {
		com.advisorycron.probe-p005.268435522 => {
			keepalive = 0
			service = com.advisorycron.probe-p005
			stream = com.apple.launchd.calendarinterval
			monitor = com.apple.UserEventAgent-Aqua
			descriptor = {
				"Minute" => 0
				"Hour" => 9
			}
		}
	}
	event channels = {
		"com.apple.launchd.calendarinterval" = {
			port = 0x0
			active = 0
			managed = 1
		}
	}
	spawn type = daemon (3)
	properties = inferred program | system service | tle system
}
```

Probe loaded, output captured, probe cleaned up (`unregister --label probe-p005` confirmed, `launchctl list | grep com.advisorycron` empty).

---

**Heads-up resolutions (Worker answers):**

- **Heads-up #2 (inline `default_config_path`):** AGREE. Inline in `src/cli/status.rs` as private fn. Zero `mod.rs` edit. Tầng 2 acceptable.
- **Heads-up #3 (empty stdout/stderr tail → `(empty)`):** AGREE. Spec confirmed. Worker implements `tail_first_n_or_empty` as per phiếu Task 2.
- **Heads-up #5 (`src/cli/mod.rs` untouched):** AGREE AND CONFIRMED. `mod.rs:25` already has `Status(status::Args)`. New `Args` fields declared INSIDE `status::Args` propagate via clap derive automatically. Zero `mod.rs` edit required.
- **Existing `--json` flag in stub `status::Args`:** Current stub at `src/cli/status.rs:10-12` already declares `pub json: bool`. Phiếu's new `Args` struct is a full rewrite — adds `label`, `config`, `last` alongside `json`. No conflict; full replacement is correct.
- **`RealLaunchctl` constructor:** Unit struct (`pub struct RealLaunchctl;` at `src/launchd.rs:165`). Instantiated as `RealLaunchctl` (no constructor call). Phiếu Task 2 shows `RealLaunchctl::new()` — Worker must correct to `RealLaunchctl` in EXECUTE. Tầng 2 self-decide.
- **`NoopLaunchctl` constructor:** `NoopLaunchctl::default()` (derived `Default`). Phiếu Task 1 unit test shows `NoopLaunchctl::new()` — Worker must correct to `NoopLaunchctl::default()` in EXECUTE. Tầng 2 self-decide.

**Tầng 1 classification check:**

Tầng 1 confirmed correct. Two mechanical triggers:
1. 4 new CLI flags on `status` — RULES.md:14 trigger.
2. `LaunchctlClient::print` — new public trait method.

Worker agrees Tầng 1 is the right call. CHALLENGE round was essential: it caught [O1.2] (the critical `launchctl print` format finding) that Architect could not verify from docs alone.

**Status:** ✅ ARCHITECT RESPONDED — phiếu bumped to V2

### Turn 1 — Architect Response (phiếu V2)

**Date:** 2026-05-27 (RESPOND mode)

- **[O1.1] → ACCEPT (Option A).** Worker's `src/launchd.rs:154` citation is authoritative; V1 phiếu Task 1 comment block embedded a 2-arg `bootstrap(domain, plist_path)` form that does not exist. V2 fix mechanical:
  - Task 1 trait code block (above, §Nhiệm vụ) now shows `fn bootstrap(&self, plist_path: &Path) -> Result<()>` verbatim — preserves P003 V2 doctrine (domain computed internally via `current_uid()`).
  - Anchor #3 Kết quả column updated to mark V2 RESOLVED with the actual signature.
  - Constraint #7 (trait extension ADDITIVE) reinforced — no further change needed beyond the comment block correction.
  - Risk if not fixed: Worker copy-pastes V1 code → trait redeclaration with wrong arity → cargo error at compile time (loud failure). Still, audit-trail clean V2 is preferred over Worker self-correct at EXECUTE.

- **[O1.2] → ACCEPT (Option A).** Worker's empirical probe is the highest-value evidence in this phiếu's verification chain. Docs (ARCHITECTURE.md:187) implied a parseable "next fire" key existed; reality (Darwin 25.5.0) shows zero such key for `StartCalendarInterval` jobs — only the configured `descriptor { "Hour" => N "Minute" => M }`. Architect's V1 spec was guess-based; V2 pivots to evidence-based:
  - **New parser strategy (Task 2 §parse_next_fire rewrite):** scan for `"Hour" =>` and `"Minute" =>` patterns inside the `descriptor` block. Format as `"daily at HH:MM"` (zero-padded). Return `Some` when both found; degrade gracefully (`Some` partial / `None` fallback) when not.
  - **NoopLaunchctl canned output (Task 1) updated:** fixture now includes `descriptor = { "Minute" => 0\n\t\t\t\t"Hour" => 9 }` matching real macOS 15 format. Unit tests exercise the real parser path.
  - **Unit test fixture (Task 2 `#[cfg(test)]` mod) updated:** uses Worker Turn 1 captured snippet as the authoritative input. Test asserts `parse_next_fire(...) == Some("daily at 09:00")`.
  - **§Vấn đề + §Giải pháp module 3 + Anchor #28 + new Anchor #31 + Constraint #5 + Constraint #16 all updated** to reflect descriptor-based strategy.
  - **Phase 1 acceptance gate satisfied semantically:** "shows next fire" → "shows configured recurrence as 'daily at HH:MM'" — honest framing about source (launchctl echoes config, not a live timer), useful for user (anh sees when the job is scheduled to run).
  - **Risk if Option B chosen instead:** Phase 1 acceptance gate would remain semantically non-satisfiable on macOS 15 (always "unknown"). Worker's heartbeat-list primary-value framing is fair but degrades user value unnecessarily when a clean alternative (descriptor parsing) exists.

- **Worker's Tầng 2 self-correct items (RealLaunchctl/NoopLaunchctl constructors)** — confirmed REFRAME to Tầng 2 Worker self-decide. V2 phiếu Task 1+2 code blocks UPDATED to show correct constructor forms (`RealLaunchctl` unit-struct instantiation, `NoopLaunchctl::default()`) as a courtesy — eliminates one self-correct round at EXECUTE.

**Phiếu bumped V1 → V2.** All Tầng 1 objections resolved. Worker may proceed to second CHALLENGE round (verify V2 consensus) OR Quản đốc may proceed to approval gate if V2 changes are trivially correct. Architect recommendation: skip second CHALLENGE (changes are mechanical + evidence-driven, low drift risk), proceed to approval gate.

**Status:** ✅ RESPONDED — phiếu bumped to V2

*(Repeat Turn 2, Turn 3 if needed. Cap = 3. V2 expected to be final.)*

### Turn 2 — Worker Challenge (V2 verification)

**Date:** 2026-05-27

**Worker accepted V2 — no new challenges.** Anchor verification:

- Anchor #1: ✅ (confirmed from Turn 1) — `src/cli/status.rs:15` stub `bail!(...)` still present (not yet implemented). EXECUTE will rewrite.
- Anchor #3: ✅ **V2 fix verified present.** Task 1 code block now shows `fn bootstrap(&self, plist_path: &Path) -> Result<()>` (1-arg), matching actual `src/launchd.rs:154`. V1 2-arg form is gone.
- Anchor #5: ✅ **V2 fix verified.** Task 1 and Task 2 show `RealLaunchctl` unit-struct instantiation (no `::new()`). Task 1 unit test `real_launchctl_print_rejects_invalid_label` uses `let client = RealLaunchctl;`.
- Anchor #6: ✅ **V2 fix verified.** Task 1 unit test `noop_launchctl_print_returns_canned_descriptor_output` uses `NoopLaunchctl::default()`.
- Anchor #28 + #31: ✅ `parse_next_fire` rewritten (descriptor-based, lines 870–902 in Task 2 spec). Scans `"Hour" =>` + `"Minute" =>` via `line.trim()` + `strip_prefix` — whitespace-agnostic. Returns `Some("daily at HH:MM")` / `Some("daily at HH:00")` / `Some("hourly at :MM")` / `None` per spec.
- `NoopLaunchctl::print` canned output: ✅ macOS 15 descriptor format present — `"Minute" => 0` + `"Hour" => 9` nested in `event triggers → descriptor` block.
- `MACOS15_FIXTURE` vs Turn 1 actual capture: ✅ Fixture is a valid trimmed subset. Retains `gui/501/com.advisorycron.probe-p005`, `.268435522` trigger ID, `stream = com.apple.launchd.calendarinterval`, `descriptor = { "Minute" => 0 "Hour" => 9 }`. Omits `active count / path / type / arguments / working directory / stdout path / stderr path / runs / last exit code / event channels / spawn type / properties` — all irrelevant to `parse_next_fire`. No Architect typos detected. UID `501` and trigger ID `.268435522` match Turn 1 capture verbatim.
- 7 `parse_next_fire` unit tests: ✅ All 7 present — `macos15_descriptor` / `none_unrecognized` / `empty_input` / `hour_only` / `minute_only` / `zero_pads_single_digit` / `rejects_out_of_range_hour`. Coverage matches spec.
- Constraint #5 (descriptor-based behavior): ✅ Updated to reflect `None` only when neither key found; "honest > confident-wrong" framing intact.
- Constraint #7 (bootstrap 1-arg): ✅ Updated — `(&self, plist_path: &Path)` explicitly stated with V2 annotation.
- Constraint #16 (default_config_path bail): ✅ Referenced in `status.rs` Task 2 `Lưu ý` + Constraint #1 (P004 Constraint #16 generalized).

**One non-blocking observation (Tầng 2 — no escalation):** The `MACOS15_FIXTURE` Rust string literal uses continuation-line indentation with spaces (Rust formatting), but the actual tab characters are encoded as `\t` escape sequences. `parse_next_fire` calls `line.trim()` on every line before matching, so leading whitespace is stripped regardless. Parser correctly handles both the fixture and real launchctl output.

**Ready for Chủ nhà approval gate.**

### Final consensus
- Phiếu version: V2 (Turn 2 Worker accepted)
- Total turns: 2 (V2 is final — all Tầng 1 objections resolved)
- Approved (autonomous narrate or Sếp gate): [date] — code execution may begin

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
[2026-05-27T16:00:00Z] event=task0_anchor_1_grep evidence=src/cli/status.rs:<line> "<exact bail message>"
[2026-05-27T16:05:00Z] event=task0_anchor_28_launchctl_print_capture evidence=<see Debate Log Turn 1 fixture — descriptor Hour/Minute>
[2026-05-27T16:30:00Z] event=cargo_check_baseline evidence=exit_code=0 duration_ms=<n>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks (RULES.md matrix) BEFORE marking phiếu DONE.
> Fill the table; mark N/A if not applicable to this phiếu.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | (manual) `cargo run --release -- status --label <existing-loaded-label>` | exit 0 + human-readable output containing "Plist: loaded" + "Next fire: daily at HH:MM" + heartbeat list (or "No heartbeats yet") | | |
| A (trigger) | (manual) `cargo run --release -- status --label <nonexistent> --json` | exit 0 + JSON with `plist_loaded: false` + `last_runs: []` | | |
| B (capability) | `cargo check` | exit 0, zero warnings | | |
| B (capability) | `cargo test --all 2>&1 \| grep 'test result'` | **51+ pass** (post-P004 baseline = 51; post-P005 floor = 51 + N new) | | |
| B (capability) | `cargo test --test cli_help` | 3/3 pass (P001 regression — verify `status --help` substring change does not break) | | |
| B (capability) | `cargo test --test cli_init` | 4/4 pass (P002 regression) | | |
| B (capability) | `cargo test --test cli_register` | 6/6 pass (P003 regression) | | |
| B (capability) | `cargo test --test cli_run` | 4/4 pass (P004 regression) | | |
| B (capability) | `cargo test --test cli_status` | new integration tests pass | | |
| B (capability) | `cargo test --lib launchd` | existing P003 tests + new `print` method test pass | | |
| B (capability) | `cargo test --lib status` (if Worker chooses to put unit tests inline in `cli/status.rs`) | `parse_next_fire` + render unit tests pass (V2 fixture: descriptor Hour/Minute) | | |
| C (migration) | N/A | (no schema change) | N/A | N/A |
| D (persistence) | `grep -l "status" docs/ARCHITECTURE.md` | ≥1 hit (§CLI surface row updated to reflect new flags) | | |
| D (persistence) | `grep -c "INV-" docs/security/INVARIANTS.md` | strictly greater than pre-P005 baseline (INV-17 added for `launchctl print` shell-out boundary) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | | |
| E (env drift) | `cargo build --release` from clean `target/` | exit 0, binary < 7MB | | |
| E (env drift) | `git diff src/cli/mod.rs` | **EMPTY** (V1 Constraint #2 hard rule) | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE verification (Worker mandatory)

1. **Anchor recap reads** — Read `src/cli/status.rs`, `src/cli/mod.rs`, `src/launchd.rs`. Log to Debug Log:
   - Anchor #1: exact line + bail! message of `src/cli/status.rs` stub (re-confirm Worker Turn 1 finding `src/cli/status.rs:15`).
   - Anchor #2: confirm `Status(status::Args)` newtype dispatch at `src/cli/mod.rs:25`. DO NOT attempt to edit mod.rs enum variants (P003 V2 hard rule).
   - Anchor #3: confirm `trait LaunchctlClient { ... }` at `src/launchd.rs:152-162` — capture current method signatures verbatim. **V2 fix:** expect `fn bootstrap(&self, plist_path: &Path) -> Result<()>` (1 arg) and `fn bootout(&self, label: &str) -> Result<()>`. If actual differs, escalate.
   - Anchor #5: confirm `pub struct RealLaunchctl;` (unit struct) at `src/launchd.rs:165` — instantiate as `RealLaunchctl` (NO `::new()`).
   - Anchor #6: confirm `pub struct NoopLaunchctl { ... }` at `src/launchd.rs:216` — instantiate as `NoopLaunchctl::default()` (NO `::new()`).
   - Anchor #26: `grep -c "^### INV-" docs/security/INVARIANTS.md` → confirm current max INV (expect INV-16). Pre-allocate INV-17 for `launchctl print` shell-out boundary.

2. **Anchor #28 — `launchctl print` actual output capture (V2: NO new probe needed — Worker Turn 1 fixture is authoritative).**
   - Worker Turn 1 already captured the macOS 15 output (Debate Log §Turn 1 — Worker Challenge §Fixture). Reuse that 30-line snippet as the `parse_next_fire` unit test fixture.
   - **Optional re-probe** only if Worker suspects another P00X has shipped between V2 and EXECUTE that changes launchd state. In solo dev mode this is impossible; skip the probe.
   - Save the fixture string in `src/cli/status.rs` `#[cfg(test)]` module for `parse_next_fire` unit tests (V2 spec — Task 2 below).

3. **Cargo.lock + Cargo.toml dep audit** — confirm:
   - `Cargo.toml [dependencies]` has explicit `serde_json = "1"` (P004 added — Anchor #12). If absent (impossible) → escalate AskUserQuestion.
   - NO `Cargo.toml` edits needed for P005 — confirm before Task 5 (Cargo.toml is in Files KHÔNG sửa list).

4. **Baseline `cargo check` + test count** — `cargo check` clean (zero warnings); `cargo test --all 2>&1 | grep "test result"` shows **51+ passing** per Anchor #27. If pre-P005 count differs from 51, escalate (something shifted since P004 ship).

5. **NO mod.rs edits invariant** — Worker reads `src/cli/mod.rs` and commits to memory: "I will NOT edit this file. Post-EXECUTE `git diff src/cli/mod.rs` must be empty." (P003 V2 [O1.1] hard rule generalized.)

### Task 1: Sửa `src/launchd.rs` — extend `LaunchctlClient` trait + add `print` to `RealLaunchctl` + `NoopLaunchctl`

**File:** `src/launchd.rs`

**Tìm:** the existing `trait LaunchctlClient { ... }` block at `src/launchd.rs:152-162` (Anchor #3 `[verified per Worker Turn 1]`).

**Thay bằng / Thêm:** Append `print` method to trait (do NOT change existing method signatures — **V2: bootstrap is 1-arg**). Add `LaunchctlPrintOutput` struct ABOVE the trait declaration.

```rust
/// Output of `launchctl print gui/<uid>/com.advisorycron.<label>`.
/// Returned by `LaunchctlClient::print`.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchctlPrintOutput {
    /// Full stdout captured from launchctl. Caller parses for "Hour"/"Minute"
    /// keys inside the `descriptor` block (V2 spec — macOS 15 launchctl does
    /// not expose a "next fire" timestamp, only the configured recurrence).
    pub raw_stdout: String,
    /// True when stderr indicated "Could not find service" — label is not currently loaded.
    /// Caller renders "not loaded" status instead of attempting to parse `raw_stdout`.
    pub not_loaded: bool,
}

pub trait LaunchctlClient {
    // existing methods — DO NOT change signatures (V2 fix: bootstrap is 1-arg)
    fn bootstrap(&self, plist_path: &Path) -> Result<()>;  // 1 arg — domain computed internally via current_uid() per P003 V2
    fn bootout(&self, label: &str) -> Result<()>;          // unchanged from P003

    /// Query launchd for the loaded job's status. Returns raw stdout for the caller
    /// to parse (parse format is system-version dependent — see P005 V2 parse_next_fire).
    /// `label` is the bare label (no `com.advisorycron.` prefix and no `gui/<uid>/`).
    /// Per INV-12, `label` MUST be ASCII alphanumeric + `-` + `_` only (caller validates;
    /// implementation re-validates as defense-in-depth).
    fn print(&self, label: &str) -> Result<LaunchctlPrintOutput>;
}
```

**Then update `RealLaunchctl` impl block to add `print` method:**

```rust
impl LaunchctlClient for RealLaunchctl {
    // existing bootstrap + bootout impls — DO NOT modify

    fn print(&self, label: &str) -> Result<LaunchctlPrintOutput> {
        // Defense-in-depth label sanitization (INV-12). Caller in src/cli/status.rs
        // also validates — this is the second of 2 enforcement points.
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            anyhow::bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }
        if label.is_empty() {
            anyhow::bail!("invalid label — empty string");
        }

        let uid = current_uid()?;
        let target = format!("gui/{uid}/com.advisorycron.{label}");

        let output = std::process::Command::new("launchctl")
            .arg("print")
            .arg(&target)
            .output()
            .with_context(|| format!("failed to spawn launchctl print {target}"))?;

        // launchctl exits non-zero when service not loaded.
        // Sample stderr (macOS 14+): "Could not find service \"com.advisorycron.<label>\" in domain for ..."
        // Sample stderr (older): "No such process"
        // Treat either substring as "not loaded" — render status accordingly, do NOT bubble error.
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Could not find service") || stderr.contains("No such process") {
                return Ok(LaunchctlPrintOutput {
                    raw_stdout: String::new(),
                    not_loaded: true,
                });
            }
            // Real launchctl error (permission denied, etc.) — bubble up.
            anyhow::bail!(
                "launchctl print {target} failed: exit={:?} stderr={}",
                output.status.code(),
                stderr
            );
        }

        Ok(LaunchctlPrintOutput {
            raw_stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            not_loaded: false,
        })
    }
}
```

**Then update `NoopLaunchctl` impl block to add `print` method (V2: canned output matches macOS 15 descriptor format):**

```rust
impl LaunchctlClient for NoopLaunchctl {
    // existing bootstrap + bootout impls — DO NOT modify

    fn print(&self, _label: &str) -> Result<LaunchctlPrintOutput> {
        // Canned output matches macOS 15 launchctl format (Worker Turn 1 captured fixture).
        // Worker may store last call args via interior mutability (same pattern P003 used
        // for bootstrap/bootout) if integration tests need to assert NoopLaunchctl was called.
        Ok(LaunchctlPrintOutput {
            raw_stdout: "gui/501/com.advisorycron.test = {\n\
                \tstate = not running\n\
                \tevent triggers = {\n\
                \t\tcom.advisorycron.test.268435522 => {\n\
                \t\t\tstream = com.apple.launchd.calendarinterval\n\
                \t\t\tdescriptor = {\n\
                \t\t\t\t\"Minute\" => 0\n\
                \t\t\t\t\"Hour\" => 9\n\
                \t\t\t}\n\
                \t\t}\n\
                \t}\n\
                }".to_string(),
            not_loaded: false,
        })
    }
}
```

**Unit tests (add to `src/launchd.rs` `#[cfg(test)]` module — V2: correct constructors):**

```rust
#[test]
fn noop_launchctl_print_returns_canned_descriptor_output() {
    let client = NoopLaunchctl::default();  // V2 fix: unit struct uses Default
    let result = client.print("test-label").expect("noop never fails");
    assert!(!result.not_loaded);
    // V2: assert descriptor Hour/Minute keys present (macOS 15 format)
    assert!(result.raw_stdout.contains("\"Hour\" => 9"));
    assert!(result.raw_stdout.contains("\"Minute\" => 0"));
}

#[test]
fn real_launchctl_print_rejects_invalid_label() {
    let client = RealLaunchctl;  // V2 fix: unit struct, no ::new()
    let result = client.print("../etc/passwd");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err:#}").contains("invalid label"));
}

#[test]
fn real_launchctl_print_rejects_empty_label() {
    let client = RealLaunchctl;  // V2 fix: unit struct, no ::new()
    let result = client.print("");
    assert!(result.is_err());
}
```

**Lưu ý:**

- **DO NOT change existing trait method signatures** — V2 confirmed `bootstrap` is 1-arg, `bootout` is 1-arg. Worker preserves verbatim. Adding `print` is the ONLY surface change.
- `NoopLaunchctl::default()` (NOT `::new()`) — derived `Default` per Worker Turn 1 Anchor #6.
- `RealLaunchctl` (NOT `::new()`) — unit struct per Worker Turn 1 Anchor #5.
- INV-10 / INV-11 / INV-12 compliance verified inline in code comments above.
- **Pre-validate label twice (defense-in-depth):** caller in `src/cli/status.rs::run` validates BEFORE calling `client.print(...)`. Implementation re-validates inside `print`. Per INV-12 2-point enforcement.

### Task 2: Sửa `src/cli/status.rs` — rewrite body + extend Args

**File:** `src/cli/status.rs`

**Tìm:** existing stub body at `src/cli/status.rs:15` — `bail!("\`status\` not yet implemented (Phase 1.5)")` (Worker Turn 1 Anchor #1 confirmed).

**Thay bằng (V2: RealLaunchctl unit-struct instantiation + descriptor-based parse_next_fire):**

```rust
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::PathBuf;

use crate::config::Config;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::launchd::{LaunchctlClient, LaunchctlPrintOutput, RealLaunchctl};

/// `advisory-cron status` — show next fire time + recent heartbeats.
///
/// Read-only. Exit code is always 0 unless config load fails (exit 2) or label is
/// invalid (exit 1). "Plist not loaded" and "no heartbeats yet" are valid statuses
/// to report, NOT errors.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Label to query (e.g., "advisory-scan-daily"). Falls back to config.task.label
    /// if omitted, then to literal "advisory-cron" if both unset.
    #[arg(long)]
    pub label: Option<String>,

    /// Path to config file (overrides default ~/.config/advisory-cron/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Output as JSON (machine-readable). Default: human-readable text.
    #[arg(long, default_value_t = false)]
    pub json: bool,

    /// Number of recent heartbeats to show. Default: 5.
    #[arg(long, default_value_t = 5)]
    pub last: usize,
}

#[derive(Serialize)]
struct StatusReport {
    label: String,
    plist_loaded: bool,
    next_fire: Option<String>,
    heartbeat_log_path: String,
    last_runs: Vec<HeartbeatRecord>,
}

pub async fn run(args: Args) -> Result<u8> {
    // 1. Resolve config path. Bail! on $HOME unset (P004 Constraint #16 generalized).
    let config_path = match args.config.clone() {
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

    // 3. Resolve label: CLI flag > config.task.label > literal "advisory-cron".
    let label = args
        .label
        .clone()
        .or_else(|| config.task.label.clone())
        .unwrap_or_else(|| "advisory-cron".to_string());

    // 4. Validate label (INV-12 first enforcement point).
    if !is_valid_label(&label) {
        eprintln!("error: invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        return Ok(1);
    }

    // 5. Query launchctl. V2 fix: RealLaunchctl is unit struct, no ::new().
    let client = RealLaunchctl;
    let print_result = match client.print(&label) {
        Ok(o) => o,
        Err(err) => {
            // Real launchctl error (not "not loaded" — that's caught as not_loaded=true).
            eprintln!("warning: launchctl print failed: {err:#}");
            // Still render heartbeats — partial status is better than nothing.
            LaunchctlPrintOutput {
                raw_stdout: String::new(),
                not_loaded: true,
            }
        }
    };

    // 6. Parse next-fire schedule (V2: descriptor Hour/Minute → "daily at HH:MM";
    //    None if not loaded or format unrecognized).
    let next_fire = if print_result.not_loaded {
        None
    } else {
        parse_next_fire(&print_result.raw_stdout)
    };

    // 7. Read recent heartbeats. Empty Vec on missing file (P004 read_last_n contract).
    let heartbeats = match heartbeat::read_last_n(&config.heartbeat.log_path, args.last) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("warning: failed to read heartbeats: {err:#}");
            Vec::new()
        }
    };

    // 8. Render.
    if args.json {
        let report = StatusReport {
            label: label.clone(),
            plist_loaded: !print_result.not_loaded,
            next_fire: next_fire.clone(),
            heartbeat_log_path: config.heartbeat.log_path.display().to_string(),
            last_runs: heartbeats,
        };
        let json = serde_json::to_string_pretty(&report)
            .context("failed to serialize StatusReport to JSON")?;
        println!("{json}");
    } else {
        render_human(&label, &print_result, &next_fire, &heartbeats, &config.heartbeat.log_path);
    }

    Ok(0)
}

fn is_valid_label(label: &str) -> bool {
    !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Resolve default config path. Bail! when `$HOME` is unset — never silently fall back
/// to `/`. Mirrors `src/cli/run.rs::default_config_path` (P004 Constraint #16).
fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(anyhow::Error::from)
        .context("$HOME environment variable is not set")?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home).join(".config/advisory-cron/config.toml"))
}

/// Parse the configured recurrence from `launchctl print` stdout.
///
/// **V2 strategy (evidence-driven, per Debate Log Turn 1 [O1.2]):** macOS 15
/// `launchctl print` for `StartCalendarInterval` jobs does NOT expose a next-fire
/// timestamp. It DOES expose the configured recurrence as nested:
///
/// ```text
/// event triggers = {
///     com.advisorycron.<label>.<id> => {
///         ...
///         descriptor = {
///             "Minute" => 0
///             "Hour" => 9
///         }
///     }
/// }
/// ```
///
/// This function scans line-by-line for `"Hour" =>` and `"Minute" =>` patterns
/// inside the descriptor block. Returns:
/// - `Some("daily at HH:MM")` when both Hour and Minute found.
/// - `Some("daily at HH:00")` when only Hour found (degenerate plist).
/// - `Some("hourly at :MM")` when only Minute found (degenerate plist).
/// - `None` when neither found (format unrecognized or future macOS drift).
///
/// Caller renders "unknown" for `None` rather than failing — honest > confident-wrong.
fn parse_next_fire(raw_stdout: &str) -> Option<String> {
    let mut hour: Option<u32> = None;
    let mut minute: Option<u32> = None;

    for line in raw_stdout.lines() {
        let trimmed = line.trim();
        // Match patterns like: "Hour" => 9   or   "Minute" => 0
        if let Some(rest) = trimmed.strip_prefix("\"Hour\"") {
            if let Some(num) = rest.trim_start_matches(|c: char| c == '=' || c == '>' || c.is_whitespace()).split_whitespace().next() {
                if let Ok(h) = num.parse::<u32>() {
                    if h < 24 {
                        hour = Some(h);
                    }
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"Minute\"") {
            if let Some(num) = rest.trim_start_matches(|c: char| c == '=' || c == '>' || c.is_whitespace()).split_whitespace().next() {
                if let Ok(m) = num.parse::<u32>() {
                    if m < 60 {
                        minute = Some(m);
                    }
                }
            }
        }
    }

    match (hour, minute) {
        (Some(h), Some(m)) => Some(format!("daily at {h:02}:{m:02}")),
        (Some(h), None) => Some(format!("daily at {h:02}:00")),
        (None, Some(m)) => Some(format!("hourly at :{m:02}")),
        (None, None) => None,
    }
}

fn render_human(
    label: &str,
    print_result: &LaunchctlPrintOutput,
    next_fire: &Option<String>,
    heartbeats: &[HeartbeatRecord],
    heartbeat_log_path: &std::path::Path,
) {
    println!("advisory-cron status — label: {label}");
    let plist_status = if print_result.not_loaded { "not loaded" } else { "loaded" };
    println!("  Plist: {plist_status}");
    let next_fire_display = match (print_result.not_loaded, next_fire) {
        (true, _) => "n/a (not loaded)".to_string(),
        (false, Some(s)) => s.clone(),
        (false, None) => "unknown (launchctl format not recognized)".to_string(),
    };
    println!("  Next fire: {next_fire_display}");
    println!();

    if heartbeats.is_empty() {
        println!(
            "Recent heartbeats: No heartbeats yet (no fires recorded at {})",
            heartbeat_log_path.display()
        );
    } else {
        println!("Recent heartbeats (last {}):", heartbeats.len());
        // Render newest-first (read_last_n returns oldest-first; reverse for display).
        for rec in heartbeats.iter().rev() {
            println!(
                "  [{ts}] exit={exit} duration={dur}ms",
                ts = rec.ts.to_rfc3339(),
                exit = rec.exit_code,
                dur = rec.duration_ms,
            );
            println!(
                "      stdout: {}",
                tail_first_n_or_empty(&rec.stdout_tail, 80)
            );
            println!(
                "      stderr: {}",
                tail_first_n_or_empty(&rec.stderr_tail, 80)
            );
        }
    }
}

fn tail_first_n_or_empty(s: &str, n: usize) -> String {
    if s.is_empty() {
        "(empty)".to_string()
    } else if s.len() <= n {
        s.to_string()
    } else {
        // Take first n bytes (snap forward to char boundary).
        let mut end = n;
        while end < s.len() && !s.is_char_boundary(end) {
            end += 1;
        }
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_label_allows_alnum_dash_underscore() {
        assert!(is_valid_label("advisory-scan-daily"));
        assert!(is_valid_label("test_label_1"));
        assert!(is_valid_label("a1b2"));
    }

    #[test]
    fn is_valid_label_rejects_path_traversal_and_empty() {
        assert!(!is_valid_label(""));
        assert!(!is_valid_label("../etc/passwd"));
        assert!(!is_valid_label("foo bar"));
        assert!(!is_valid_label("foo;rm"));
        assert!(!is_valid_label("foo.bar"));  // dot not in allowlist
    }

    /// V2: Authoritative fixture is Worker Turn 1 captured macOS 15 launchctl output.
    /// Trimmed to relevant descriptor block.
    const MACOS15_FIXTURE: &str = "gui/501/com.advisorycron.probe-p005 = {\n\
        \tstate = not running\n\
        \tevent triggers = {\n\
        \t\tcom.advisorycron.probe-p005.268435522 => {\n\
        \t\t\tstream = com.apple.launchd.calendarinterval\n\
        \t\t\tdescriptor = {\n\
        \t\t\t\t\"Minute\" => 0\n\
        \t\t\t\t\"Hour\" => 9\n\
        \t\t\t}\n\
        \t\t}\n\
        \t}\n\
        }";

    #[test]
    fn parse_next_fire_extracts_daily_from_macos15_descriptor() {
        // V2 fixture from Debate Log Turn 1 empirical capture.
        let result = parse_next_fire(MACOS15_FIXTURE);
        assert_eq!(result, Some("daily at 09:00".to_string()));
    }

    #[test]
    fn parse_next_fire_returns_none_on_unrecognized_format() {
        let sample = "some-totally-unknown launchctl output\nfoo = bar\n";
        assert_eq!(parse_next_fire(sample), None);
    }

    #[test]
    fn parse_next_fire_handles_empty_input() {
        assert_eq!(parse_next_fire(""), None);
    }

    #[test]
    fn parse_next_fire_handles_hour_only() {
        let sample = "descriptor = {\n\t\"Hour\" => 14\n}";
        assert_eq!(parse_next_fire(sample), Some("daily at 14:00".to_string()));
    }

    #[test]
    fn parse_next_fire_handles_minute_only() {
        let sample = "descriptor = {\n\t\"Minute\" => 30\n}";
        assert_eq!(parse_next_fire(sample), Some("hourly at :30".to_string()));
    }

    #[test]
    fn parse_next_fire_zero_pads_single_digit() {
        let sample = "descriptor = {\n\t\"Hour\" => 5\n\t\"Minute\" => 7\n}";
        assert_eq!(parse_next_fire(sample), Some("daily at 05:07".to_string()));
    }

    #[test]
    fn parse_next_fire_rejects_out_of_range_hour() {
        let sample = "descriptor = {\n\t\"Hour\" => 99\n\t\"Minute\" => 0\n}";
        // Hour 99 invalid → Hour=None, Minute=Some(0) → "hourly at :00"
        assert_eq!(parse_next_fire(sample), Some("hourly at :00".to_string()));
    }

    #[test]
    fn tail_first_n_or_empty_returns_marker_for_empty() {
        assert_eq!(tail_first_n_or_empty("", 80), "(empty)");
    }

    #[test]
    fn tail_first_n_or_empty_truncates_long_strings() {
        let s = "a".repeat(200);
        let result = tail_first_n_or_empty(&s, 80);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 84);  // 80 chars + "..."
    }
}
```

**Lưu ý:**

- `Args` struct uses `#[derive(clap::Args)]` (NOT `Parser`) — matches newtype dispatch (Anchor #29 `[verified]`). NO `src/cli/mod.rs` edits.
- `default_config_path` is INLINED here (not extracted to shared helper) — extraction would require `src/cli/mod.rs` edit (declare new `cli::util` module), violating Constraint #2. Code duplication with `src/cli/run.rs::default_config_path` is acceptable trade-off (Tầng 2 stylistic). Worker may copy the function body from `src/cli/run.rs` line-for-line.
- **V2: `RealLaunchctl` is a unit struct** — instantiate as `RealLaunchctl`, NOT `RealLaunchctl::new()` (Worker Turn 1 Anchor #5).
- **V2: `parse_next_fire` fixture** is the Worker Turn 1 captured macOS 15 output. The unit test `parse_next_fire_extracts_daily_from_macos15_descriptor` is the highest-value test — it locks the descriptor-based strategy against the real format.
- Exit code 1 for invalid label is the only non-0/non-2 exit code in status. Per ARCHITECTURE.md:72 (exit 1 = "Generic error"). Acceptable.
- `println!` for output (NOT `eprintln!`) — status output IS the value; goes to stdout for shell capture (`advisory-cron status --json | jq ...`).
- `read_last_n` returns oldest-first per P004 contract; render reverses for newest-first display (UX — most recent at top).

### Task 3: Tạo `tests/cli_status.rs` — integration tests

**File:** `tests/cli_status.rs` (mới)

**Thêm:**

```rust
//! Integration tests for `advisory-cron status` (Phase 1.5).
//!
//! Pattern mirrors `tests/cli_run.rs` (P004): spawn the compiled binary with a temp
//! config + temp heartbeat path, assert on exit code + stdout/stderr.
//!
//! These tests do NOT exercise `RealLaunchctl::print` against a real loaded plist
//! (would require side-effect on `~/Library/LaunchAgents/`). Real launchctl path
//! tested manually per Verification Trace Sub-mech A rows.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn binary_path() -> String {
    env!("CARGO_BIN_EXE_advisory-cron").to_string()
}

fn write_config(dir: &Path, heartbeat_path: &Path) -> std::path::PathBuf {
    let config_path = dir.join("config.toml");
    let contents = format!(
        r#"[task]
command = "/bin/echo"
args = ["hello"]
working_dir = "/tmp"
label = "p005-status-test"

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

fn write_heartbeat_line(path: &Path, exit_code: i32, label: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create heartbeat dir");
    }
    // Manually compose JSON (avoids depending on internal HeartbeatRecord struct from test crate).
    // Schema must match exactly — if this drifts, P005 test breaks loudly (good signal).
    let line = format!(
        r#"{{"ts":"2026-05-27T02:00:00Z","label":"{label}","exit_code":{exit_code},"duration_ms":100,"stdout_tail":"hello","stderr_tail":""}}{}"#,
        "\n"
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open heartbeat");
    file.write_all(line.as_bytes()).expect("write heartbeat");
}

#[test]
fn status_with_heartbeats_and_unloaded_plist_exits_zero_human() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb/heartbeat.jsonl");
    write_heartbeat_line(&heartbeat_path, 0, "p005-status-test");
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "definitely-not-loaded-label-p005",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not loaded"), "expected 'not loaded' in output:\n{stdout}");
    assert!(stdout.contains("exit=0"), "expected heartbeat exit=0 line:\n{stdout}");
}

#[test]
fn status_with_no_heartbeats_exits_zero_with_friendly_message() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("does-not-exist.jsonl");
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "any-label-p005",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No heartbeats yet"),
        "expected 'No heartbeats yet' in output:\n{stdout}"
    );
}

#[test]
fn status_json_mode_produces_valid_json() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb.jsonl");
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "any-label-p005",
            "--json",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("status --json output is not valid JSON: {e}\nstdout: {stdout}"));
    assert!(parsed.get("label").is_some());
    assert!(parsed.get("plist_loaded").is_some());
    assert!(parsed.get("last_runs").is_some());
    assert_eq!(parsed.get("last_runs").and_then(|v| v.as_array()).map(|a| a.len()), Some(0));
}

#[test]
fn status_last_flag_clamps_heartbeat_count() {
    let tmp = TempDir::new().expect("tempdir");
    let heartbeat_path = tmp.path().join("hb.jsonl");
    // Write 5 heartbeats.
    for i in 0..5 {
        write_heartbeat_line(&heartbeat_path, i, "p005-test");
    }
    let config_path = write_config(tmp.path(), &heartbeat_path);

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            config_path.to_str().unwrap(),
            "--label",
            "any-label-p005",
            "--last",
            "3",
        ])
        .output()
        .expect("spawn advisory-cron");

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Count occurrences of "exit=" — should appear exactly 3 times in human render
    // (one per heartbeat line).
    let count = stdout.matches("exit=").count();
    assert_eq!(count, 3, "expected exactly 3 heartbeat lines (--last 3), got {count}\nstdout: {stdout}");
}

#[test]
fn status_with_missing_config_exits_two() {
    let tmp = TempDir::new().expect("tempdir");
    let bogus_config = tmp.path().join("does-not-exist.toml");

    let output = Command::new(binary_path())
        .args([
            "status",
            "--config",
            bogus_config.to_str().unwrap(),
        ])
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

- `write_heartbeat_line` manually composes JSON to avoid depending on the internal `HeartbeatRecord` struct (integration tests live in `tests/`, a separate compilation unit; can't import private types easily). If schema drifts, this test fails loudly — good signal.
- Tests use `--label any-label-p005` for cases where the actual launchctl status doesn't matter (only heartbeat rendering matters). The `definitely-not-loaded-label-p005` in test (a) explicitly targets the "not loaded" path.
- No test exercises a REAL loaded plist — that requires `~/Library/LaunchAgents/` side effects + cleanup. Verification Trace Sub-mech A row covers manual end-to-end check.
- `env!("CARGO_BIN_EXE_advisory-cron")` is the cargo-provided pattern (P004 Task 7 + cli_run uses same). Confirmed via P004 Anchor reuse.

### Task 4: Update `docs/ARCHITECTURE.md` (Docs Gate — Tầng 1 CLI surface change)

**File:** `docs/ARCHITECTURE.md`

**Updates required (precise, per Anchors #15/16/17):**

**A. §CLI surface table — `status` row Args column update (line 65 `[verified]`):**

Current: `--json (machine output)`
New: `--label <name>` `--config <path>` (optional) `--json` (machine output) `--last <N>` (default 5)

**B. §Modules table — `src/cli/status.rs` row Phase column update (line 41 `[verified]`):**

Current: `1.1 skeleton ✅ → impl 1.5`
New: `1.5 ✅`

**C. §Modules table — `src/launchd.rs` row update (line 47 `[verified]`):**

Append note: "Extended P005: `LaunchctlClient::print` method + `LaunchctlPrintOutput` struct (status reporter). `parse_next_fire` parses macOS 15 `descriptor` block Hour/Minute (no timestamp key in launchctl output — per P005 Discovery)."

**D. §Phase status (line 277 `[verified]`):**

Append: "Phase 1.5 shipped: status reporter (`launchctl print` parsing of `descriptor` Hour/Minute → 'daily at HH:MM'; heartbeat read-render; new CLI flags `--label / --config / --json / --last`; `LaunchctlClient` trait extended with `print`; INV-17 appended for `launchctl print` shell-out boundary). **Discovery (P005):** macOS 15 launchctl does NOT expose a 'next fire' timestamp for `StartCalendarInterval` jobs — only configured recurrence. Acceptance gate satisfied via configured-recurrence rendering. Phases 1.6–1.7 pending."

### Task 5: Update `docs/CHANGELOG.md`

**File:** `docs/CHANGELOG.md`

**Thêm** new entry at top (newest first per CHANGELOG convention):

```markdown
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
- Total: 51 baseline + ~19 new = ~70 tests (Worker confirms final count post-EXECUTE)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §CLI surface `status` row Args column updated (new flags); §Modules table `src/cli/status.rs` row marked shipped 1.5 ✅; `src/launchd.rs` row notes trait extension + descriptor parser; §Phase status updated with Phase 1.5 + macOS 15 discovery.
- `docs/security/INVARIANTS.md` — INV-17 appended.

**No new dep.** `serde_json` (P004 explicit) + `chrono` (P002 direct) + `clap` (P001 direct) cover all P005 needs.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 51 + N pass (51 baseline + new unit + new integration)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #2 hard rule satisfied)
- Manual: `cargo run --release -- status --label <some-loaded-label>` shows "Next fire: daily at HH:MM" + heartbeats
- Manual: `--json` produces parseable JSON
- Manual: `env -u HOME cargo run -- status` → exit 2 with `$HOME` error (P004 Constraint #16 generalized)

---
```

### Task 6: Update `docs/security/INVARIANTS.md` (append INV-17)

**File:** `docs/security/INVARIANTS.md`

**Tìm:** end of INV-16 block (`[verified per Read INVARIANTS.md:228]` last "Implemented in Giám sát: No" line of INV-16, line 228).

**Thêm** new INV block after INV-16:

```markdown
---

### INV-17 — `launchctl print` shell-out: label sanitization + discrete arg passing

**Statement:** PR introducing `launchctl print` shell-out (P005 `RealLaunchctl::print`) MUST:
1. Validate `label` against the INV-12 allowlist (ASCII alphanumeric + `-` + `_`) at BOTH (a) caller in `src/cli/status.rs::run` (pre-flight) and (b) inside `RealLaunchctl::print` impl (defense-in-depth).
2. Pass `label` as a component of `format!("gui/{uid}/com.advisorycron.{label}")` where `uid: u32` (numeric, parsed via `current_uid()`); the resulting target string is passed as a discrete `.arg()` to `Command::new("launchctl")`. NO `Command::new("sh").arg("-c").arg(format!("launchctl print ... {label}"))` — shell interpolation PROHIBITED.

**Why:** Same threat model as INV-10 / INV-12 — `launchctl print` accepts a target string that contains the label as a path component. Without sanitization, a label like `../foo` or `foo;evil` could either probe unintended services or (if shell-interpolated) inject. `launchctl print` is read-only (no side effect like `bootstrap`), so the impact is limited to information disclosure — still worth defending.

**Implementation (Phase 1.5):** `src/launchd.rs::RealLaunchctl::print` — `format!("gui/{uid}/com.advisorycron.{label}")` where `uid: u32` and `label` is pre-validated. `Command::new("launchctl").arg("print").arg(&target)` — discrete args, no shell. Caller in `src/cli/status.rs::run` validates label via `is_valid_label` helper before invocation.

**Trigger keywords:** `RealLaunchctl::print` call sites, `Command::new("launchctl").arg("print")`, new `launchctl <verb>` shell-outs.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.
```

**Lưu ý:**

- Pre-allocated INV number = INV-17 per Anchor #26 `[verified]` (current max = INV-16 from P004).
- Append at end of file (after INV-16) — newest INV at bottom per file convention.

### Task 7: Update `.phieu-counter` is NOT required (Quản đốc bumped already, confirmed Read at `005`).

**File:** `.phieu-counter`

**Action:** NONE. Worker does NOT touch. Verify Read returns `005\n` at Task 0 — if drift, escalate.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/cli/status.rs` | Task 2: Rewrite body. Extend `Args` with `label/config/json/last`. Add `parse_next_fire` (**V2: descriptor Hour/Minute parser**), `is_valid_label`, `default_config_path`, `render_human`, `tail_first_n_or_empty` helpers + unit tests. |
| `src/launchd.rs` | Task 1: Add `LaunchctlPrintOutput` struct. Extend `LaunchctlClient` trait with `fn print` (**V2: bootstrap stays 1-arg, do NOT widen**). Implement on `RealLaunchctl` + `NoopLaunchctl` (**V2: NoopLaunchctl canned fixture uses macOS 15 descriptor format**). Add 3 unit tests. |
| `tests/cli_status.rs` | Task 3: NEW. 5 integration tests (heartbeats+unloaded, no-heartbeats, --json, --last clamp, missing config). |
| `docs/ARCHITECTURE.md` | Task 4: §CLI surface `status` row Args; §Modules table marks status.rs shipped 1.5 + launchd.rs row notes extension **+ descriptor parser**; §Phase status update **with macOS 15 discovery note**. |
| `docs/CHANGELOG.md` | Task 5: P005 entry citing CLI flags + trait extension + INV-17 + tests + **V2 descriptor-parser pivot**. NO new dep cited (zero dep delta). |
| `docs/security/INVARIANTS.md` | Task 6: Append INV-17 (launchctl print shell-out boundary). REQUIRED per RULES.md:22. |
| `docs/discoveries/P005.md` | Discovery Report (assumptions verified vs corrected + Anchor #28 macOS 15 launchctl output capture + descriptor-parser pivot rationale + edge cases + docs updated). REQUIRED. |
| `docs/DISCOVERIES.md` | 1-line index append (newest at top). REQUIRED. |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/cli/mod.rs` | **DO NOT EDIT.** Newtype dispatch `Commands::Status(status::Args)` already forwards `Args`; new flags declared INSIDE `status::Args` propagate via clap derive. Post-EXECUTE `git diff src/cli/mod.rs` MUST be empty. |
| `src/cli/init.rs`, `src/cli/register.rs`, `src/cli/unregister.rs` | P001-P003. No P005 touch. |
| `src/cli/run.rs` | P004. No P005 touch. P005's `default_config_path` is a copy of the P004 one — Worker does NOT modify run.rs's version. |
| `src/config.rs` | P002/P004 ship. P005 only READS `config.task.label` + `config.heartbeat.log_path` — no schema change. |
| `src/runner.rs` | P004. No P005 touch. Status does NOT spawn user-config commands. |
| `src/heartbeat.rs` | P004. P005 only CALLS `heartbeat::read_last_n` (existing signature). NO change to module. |
| `src/main.rs` | No new `mod` declaration (no new module added — `status.rs` already declared via `cli::status` in `src/cli/mod.rs` from P001). |
| `Cargo.toml` | NO new dep. `serde_json` (P004) + `chrono` (P002) + `clap` (P001) cover all P005 needs. Worker confirms zero diff. |
| `Cargo.lock` | Auto-regenerated by cargo. Worker confirms no surprise major-version jump via `cargo update --dry-run` (Sub-mech E). |
| `tests/cli_help.rs` | P001. 3 tests must continue passing. If existing tests substring-match on `status --help` body, may need additive line update — Worker check. |
| `tests/cli_init.rs` | P002. 4 tests must pass unmodified. |
| `tests/cli_register.rs` | P003. 6 tests must pass unmodified. |
| `tests/cli_run.rs` | P004. 4 tests must pass unmodified. |
| `README.md` | Defer Phase 1.6. |
| `.phieu-counter` | Quản đốc bumped 004 → 005 (confirmed Read at `005\n`). Worker does NOT touch. |

---

## Luật chơi (Constraints)

1. **`default_config_path` MUST `bail!` on `$HOME` unset.** Mirror `src/cli/run.rs` Constraint #16 (P004). NO silent `/` fallback. Inline the helper in `src/cli/status.rs` — do NOT extract to shared `cli::util` (would require `mod.rs` edit, Constraint #2 violation).
2. **NO `src/cli/mod.rs` edits.** Hard rule generalized from P003 V2 Turn 1 [O1.1] + P004 Constraint #1. Post-EXECUTE `git diff src/cli/mod.rs` MUST be empty.
3. **NO `unsafe { }` blocks.** Zero current need — all P005 code uses safe APIs (clap derive, std::process, serde_json). Escalate via AskUserQuestion if tempted.
4. **NO new deps.** `Cargo.toml` `[dependencies]` MUST NOT change. P005 reuses `serde_json` (P004) + `chrono` (P002) + `clap` (P001). Worker confirms zero diff. Escalate if discovery suggests `tabular`, `comfy-table`, or any rendering crate — Phase 1 rendering is plain `println!` for simplicity.
5. **`parse_next_fire` (V2 descriptor-based)** returns `None` ONLY when neither `"Hour" =>` nor `"Minute" =>` line found. On macOS 15 + this project's `StartCalendarInterval` plists, both are always present → parser returns `Some("daily at HH:MM")` consistently. Future macOS drift → `None` fallback → human render "unknown (launchctl format not recognized)" + JSON `next_fire: null`. DO NOT panic, DO NOT return Err, DO NOT guess. Honest > confident-wrong.
6. **Status exit code = 0 always**, EXCEPT:
   - Exit 2 if config load fails OR `$HOME` unset (config path unresolvable). Per ARCHITECTURE.md:74.
   - Exit 1 if `--label` (or fallback) fails INV-12 validation. Per ARCHITECTURE.md:72 (generic error).
   - launchctl print failure (real error, not "not loaded") → log warning to stderr, continue rendering heartbeats with `plist_loaded: false`. Exit 0. (Partial status > total failure.)
7. **Trait extension is ADDITIVE only.** `LaunchctlClient::bootstrap` (**V2: 1 arg — `(&self, plist_path: &Path)`**) and `bootout` (`(&self, label: &str)`) method signatures MUST NOT change. Worker preserves verbatim per Worker Turn 1 Anchor #3 capture.
8. **INV-12 label allowlist enforced at 2 points** (per INV-12 spec): caller in `src/cli/status.rs::run` (pre-flight) AND inside `RealLaunchctl::print` (defense-in-depth). Same allowlist as `generate_plist` (P003): ASCII alphanumeric + `-` + `_`, non-empty.
9. **INV-10 compliance for `launchctl print` shell-out:** target string built via `format!("gui/{uid}/com.advisorycron.{label}")` where `uid: u32` (numeric, parsed); passed as discrete `.arg()` to `Command::new("launchctl")`. NO `Command::new("sh").arg("-c").arg(format!(...))`.
10. **Empty `stdout_tail` / `stderr_tail` render as `(empty)`** in human mode (Heads-up #3 resolution). JSON mode passes through as empty string (machine parsers handle). Worker writes integration test verifying.
11. **`read_last_n` returns oldest-first**; human render REVERSES for newest-first display (UX — most recent at top). JSON mode preserves oldest-first (machine consumers can sort).
12. **NO heartbeat WRITE in status.** Status is strictly read-only. Worker MUST NOT call `heartbeat::append` anywhere in `src/cli/status.rs`. Hard rule.
13. **NO `tokio::main` async runtime for status** — `status::run` is `async fn` per the existing CLI dispatch pattern (matches P004 `run::run`), but the body itself has no `.await` (launchctl is sync shell-out via `std::process::Command`). `heartbeat::read_last_n` is sync (P004 ship). Async signature kept for dispatch compat only.
14. **NO `tracing` use in P005.** Stick to `eprintln!` per P002/P003/P004 pattern.
15. **NO Hard Stop trigger** — phiếu authorizes only: 1 trait method extension (`LaunchctlClient::print`), 1 new struct (`LaunchctlPrintOutput`), 4 new CLI flags on existing subcommand, 1 new INV (INV-17), 1 new integration test file. Anything else = HARD STOP, escalate via AskUserQuestion per CLAUDE.md §HARD STOPS section.
16. **Discovery Report MUST record:**
    - (a) Anchor #1 + #2 + #3 + #5 + #6 actual file:line evidence (Worker grep at Task 0 — defensive re-confirm of Turn 1 finding).
    - (b) Anchor #28 + #31 — macOS 15 `launchctl print` output capture (paste 30-line snippet, or reference Debate Log Turn 1 fixture) + descriptor-parser pivot rationale. **This is the highest-value Discovery — locks the strategy for future status-related work.**
    - (c) Confirmed `parse_next_fire` behavior on real fixture (e.g., "Picked descriptor `Hour=9` + `Minute=0` → rendered 'daily at 09:00'").
    - (d) `NoopLaunchctl::default()` + `RealLaunchctl` unit-struct constructor styles confirmed (Worker Turn 1 self-correct items).
    - (e) Post-P005 test count (51 baseline + N new — confirm N matches spec).
    - (f) Whether `cli_help.rs` needed update for `status --help` substring change.
    - (g) INV-17 number assignment (confirm not collided with concurrent phiếu — should not be possible in solo dev mode).

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings; binary < 7MB per PROJECT.md:60
- [ ] `cargo test --all` — all pass (51 baseline per Anchor #27 + ~19 new from this phiếu; Worker confirms exact N)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `git diff src/cli/mod.rs` — **empty** (Constraint #2 hard rule)
- [ ] `git diff Cargo.toml` — **empty** (Constraint #4 hard rule — NO new dep)
- [ ] `cargo update --dry-run` — no surprise major bump (Sub-mech E)

### Manual Testing
- [ ] **Status with no loaded plist + no heartbeats:** create temp config with `heartbeat.log_path = /tmp/p005-empty.jsonl` (file absent); `cargo run --release -- status --config <temp>.toml --label nonexistent-p005`; expect exit 0; stdout contains "not loaded" + "No heartbeats yet".
- [ ] **Status with heartbeats:** run `cargo run --release -- run --config <temp>.toml` first (P004 path) to generate heartbeats; then `cargo run --release -- status --config <temp>.toml --label nonexistent-p005`; expect exit 0; output shows heartbeat ts + exit_code + duration.
- [ ] **Status with loaded plist (real launchctl):** `cargo run -- register --label probe-p005-manual --schedule "0 9 * * *"`; then `cargo run -- status --label probe-p005-manual`; expect exit 0; output contains "loaded" + "Next fire: daily at 09:00" (**V2: derived from configured Hour/Minute, not a timestamp**). Cleanup: `cargo run -- unregister --label probe-p005-manual`. **Critical — this is the only test that exercises real `launchctl print` end-to-end on macOS 15.**
- [ ] **JSON mode:** `cargo run -- status --label nonexistent --json | jq .`; expect valid JSON with `label`, `plist_loaded`, `next_fire`, `heartbeat_log_path`, `last_runs` fields.
- [ ] **`--last 3` clamp:** generate 5+ heartbeats via 5 `run` invocations; `cargo run -- status --last 3 --label test`; verify exactly 3 heartbeat lines in human render.
- [ ] **Missing config:** `cargo run -- status --config /tmp/bogus.toml`; expect exit 2.
- [ ] **`$HOME` unset:** `env -u HOME cargo run -- status`; expect exit 2 with stderr containing "$HOME environment variable is not set".
- [ ] **Invalid label:** `cargo run -- status --label "../etc/passwd"`; expect exit 1 with stderr containing "invalid label".
- [ ] **Empty heartbeat tail render:** if recent run had empty stderr, status output shows `stderr: (empty)` (not blank line).

### Regression
- [ ] `cargo test --test cli_help` — 3/3 pass (P001 baseline — verify any `status --help` substring change does not break)
- [ ] `cargo test --test cli_init` — 4/4 pass (P002 baseline)
- [ ] `cargo test --test cli_register` — 6/6 pass (P003 baseline)
- [ ] `cargo test --test cli_run` — 4/4 pass (P004 baseline)
- [ ] `cargo test --lib launchd` — existing P003 unit tests pass + new `print` tests
- [ ] `cargo test --all 2>&1 | grep 'test result'` — aggregate count = 51 baseline + N new (Worker reports actual N)
- [ ] `cargo run -- run --config <temp>.toml` — P004 still works post-P005 (no regression to run path)
- [ ] `cargo run -- init --force` — P002 still writes default config

### Docs Gate
- [ ] `docs/CHANGELOG.md` — entry for P005 citing CLI flags + trait extension + INV-17 + tests + **V2 descriptor parser pivot** (no new dep)
- [ ] `docs/ARCHITECTURE.md` — §CLI surface `status` row Args updated; §Modules table `src/cli/status.rs` row marked shipped 1.5 ✅; `src/launchd.rs` row notes trait extension + **descriptor parser**; §Phase status updated to note Phase 1.5 shipped **with macOS 15 launchctl discovery**
- [ ] `docs/security/INVARIANTS.md` — INV-17 appended (launchctl print shell-out boundary)
- [ ] `README.md` — defer Phase 1.6 (no edit required this phiếu)
- [ ] `docs-gate --all --verbose` — pass (changelog + architecture + tickets + discovery checks)

### Discovery Report
- [ ] `docs/discoveries/P005.md` — full report per Constraint #16:
  - **Assumptions ĐÚNG:** list each verified anchor (#1 confirmed by Worker Turn 1, #2 confirmed by Worker Turn 1, #4, #5, #6 with NoopLaunchctl::default, #7, #8, #9, #10, #11, #12, #13, #14, #15, #16, #17, #18, #19, #20, #21, #22, #23, #24, #25, #26, #27, #29, #30)
  - **Assumptions SAI / DRIFT:** Anchor #3 V1 spec had wrong 2-arg bootstrap form → V2 corrected to 1-arg per Worker Turn 1; Anchor #28 V1 specced timestamp-key parsing → V2 pivoted to descriptor Hour/Minute per Worker Turn 1 empirical capture
  - **Anchor #28 + #31 capture:** paste the macOS 15 launchctl output (30 lines, from Debate Log Turn 1 fixture) — confirms `descriptor = { "Minute" => 0 "Hour" => 9 }` is the authoritative source; NO timestamp key exists on Darwin 25.5.0
  - **Edge cases / limitations discovered:** record any quirk (e.g., whether `parse_next_fire` handled the exact whitespace of real output, any cli_help.rs substring updates needed, NoopLaunchctl interior mutability decision if any)
  - **Docs updated:** ARCHITECTURE.md sections updated + INV-17 added + CHANGELOG entry hash
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended at top: `- 2026-05-27 P005: Status reporter shipped (status --label/--config/--json/--last; LaunchctlClient trait + print method; macOS 15 launchctl exposes NO next-fire timestamp — parse_next_fire pivoted to descriptor Hour/Minute → "daily at HH:MM"; INV-17 launchctl print shell-out boundary; ~19 new tests) → see docs/discoveries/P005.md`
- [ ] Sub-mechanism A-E Verification Trace filled (table above) — all rows ✅ or N/A

---

*Phiếu version V2 — Turn 1 RESPOND applied 2026-05-27. Two Tầng 1 objections resolved: [O1.1] ACCEPT Option A (bootstrap 1-arg signature corrected), [O1.2] ACCEPT Option A (parse_next_fire pivoted to descriptor Hour/Minute based on Worker empirical capture). All previously `[needs Worker verify]` anchors now resolved. Architect recommendation: skip second CHALLENGE round (V2 changes are mechanical + evidence-driven), proceed to approval gate.*
