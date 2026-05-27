# PHIẾU P006: MCP server wrapper (Phase 1.7 — stdio, full parity)

> **Loại:** Feature
> **Tầng:** 1 (mandatory — new dep `rmcp` adds to `Cargo.toml`, new CLI subcommand `mcp`, new public surface `core::*` + `mcp::*` modules, new exit code path 5, new acceptance gate "MCP register ≡ CLI register", binary size budget gate <7MB. RULES.md:13/16/21/22 all trigger. Constraint #1 from prior phiếu — "don't touch `src/cli/mod.rs`" — explicitly RETIRED for P006 because adding a new subcommand variant requires extending `Commands` enum; see §Constraint #1 retirement below.)
> **Ưu tiên:** P0 (Phase 1 acceptance §6 MVP scope item 6 + acceptance bullets 6/7/8/9/10/12 — gates Phase 1 ship gate `docs/PROJECT.md:58-64` `[verified]`)
> **Ảnh hưởng:**
> - NEW: `src/core/mod.rs`, `src/core/init.rs`, `src/core/register.rs`, `src/core/unregister.rs`, `src/core/run.rs`, `src/core/status.rs` (extracted pure functions)
> - NEW: `src/mcp/mod.rs`, `src/mcp/server.rs`, `src/mcp/tools.rs`, `src/cli/mcp.rs`
> - NEW: `tests/cli_mcp.rs` (MCP handshake + tool-call integration test)
> - REWRITE thin shells: `src/cli/init.rs`, `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/run.rs`, `src/cli/status.rs` (delegate to `core::*` — preserve CLI behavior byte-for-byte)
> - EDIT: `src/cli/mod.rs` (add `Mcp(mcp::Args)` variant + dispatch arm — first legitimate edit since P003)
> - EDIT: `Cargo.toml` (add `rmcp` with `server` + `transport-io` features; ADD `io-std` to tokio features per Worker Turn 1 V2 [O-non-blocking])
> - EDIT: `docs/ARCHITECTURE.md` (§Modules table for new modules; §MCP surface populated; §CLI surface row for `mcp`; §Phase status)
> - EDIT: `docs/CHANGELOG.md`, `docs/security/INVARIANTS.md` (INV-18 for MCP transport boundary), `docs/DISCOVERIES.md` + `docs/discoveries/P006.md`
> - EDIT: `README.md` (Claude Desktop config snippet — promoted from Phase 1.6 deferred since acceptance bullet 10 ties it to P006)
> **Dependency:** P001 ✅ (CLI scaffold), P002 ✅ (config), P003 ✅ (launchd), P004 ✅ (runner+heartbeat), P005 ✅ (status reporter — provides StatusReport struct that becomes the model for MCP `status` tool output)

---

## Context

### Vấn đề hiện tại

Phase 1 acceptance gate `docs/PROJECT.md:58-64` `[verified]` requires:

- `advisory-cron mcp` starts MCP server over stdio (JSON-RPC 2.0); `initialize` handshake returns server info + 5 tools.
- MCP tools `init` / `register` / `unregister` / `run` / `status` callable from Claude Desktop with config snippet documented in README.
- `cargo build --release` produces single binary < 7MB.
- `cargo test --all` all pass (includes MCP handshake integration test).
- MCP tool schema (input/output JSON for each of 5 tools) documented in ARCHITECTURE.md.

Current state post-P005 (CHANGELOG `[verified]`): All 5 CLI subcommands fully working — `init` (P002), `register`/`unregister` (P003), `run` (P004), `status` (P005). Each handler in `src/cli/<sub>.rs` currently contains the full logic inline (config load + side-effect + return). Binary 1.2MB (P005 ship). Tests 70.

What's MISSING:
1. No `src/core/` extraction — the "Layering invariant" introduced in ARCHITECTURE.md:53 `[verified]` ("`core::*` knows nothing about CLI or MCP. `cli::*` and `mcp::*` are both thin adapters") is documented but UNFULFILLED. Phases 1.2–1.5 deferred this — P006 is the FORCED MOMENT to make it real, because MCP handlers MUST share code paths with CLI handlers to satisfy the "MCP register ≡ CLI register" behavioral invariant (ARCHITECTURE.md:228 `[verified]`).
2. No `src/mcp/` modules — the MCP server bootstrap + tool registry don't exist yet. `cli/mcp.rs` doesn't exist either.
3. No `Cargo.toml` MCP dep — backlog item BACKLOG.md:28 `[verified]` explicitly says "Architect MUST research Rust MCP SDK choice (likely `rmcp` official Anthropic crate — verify via context7 before specing)".
4. No CLI dispatcher arm for `mcp` — `src/cli/mod.rs::Commands` enum has 5 variants per P001 ship, must add a 6th.
5. No README Claude Desktop config snippet — acceptance bullet 10 ties this to P006 (it's natural to ship docs WITH the MCP surface so they're testable in same dogfood pass; Phase 1.6 docs ticket only needs to confirm + polish).

Reference: `docs/BACKLOG.md:28` `[verified]` Phase 1.7 — "Subcommand `advisory-cron mcp` starts JSON-RPC 2.0 server over stdin/stdout. Exposes 5 tools 1-1 with CLI subcommands... Each tool's handler calls the SAME core function as its CLI counterpart (zero logic duplication — CLI layer + MCP layer both thin shells over `core::*` functions)."

### Giải pháp

**6 sub-decisions resolved by Architect (rationale + chosen option for each):**

---

**Decision 1 — MCP SDK choice → `rmcp` (CONFIRMED V2 — Worker Turn 1 verified).**

Worker Turn 1 confirmed: `rmcp = "1.7.0"` exists on crates.io (Official Anthropic Rust SDK), stdio transport available via `transport-io` feature, compatible with `current_thread` tokio runtime. Decision LOCKED.

Pinned dep: `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`.
- `server` feature → `transport-async-rw` + `schemars` (transitive, mandatory but no `#[derive(JsonSchema)]` on our types needed) + `pastey`.
- `transport-io` feature → stdio transport + adds `tokio/io-std` requirement (see Task 1 V2).
- `macros` feature DROPPED — Decision 3 hand-written schemas + manual `ServerHandler` impl don't need `#[tool]` / `#[tool_router]` / `#[tool_handler]` macros. Saves `pastey` proc-macro cost.

Fallback path retired — rmcp confirmed viable, no need for hand-rolled JSON-RPC or alternative crate.

---

**Decision 2 — `core::*` extraction scope → Option A (FULL extraction, all 5 handlers).**

Worker Turn 1 measured: total CLI handler LOC = 734 (init=43, register=119, unregister=82, run=110, status=380). Extractable logic ~120 LOC for status (rest is tests + render helpers). **Total extraction diff well under 600 LOC budget — P006 stays as single phiếu, NO P006a/P006b split needed.**

Option A rationale unchanged:
1. ARCHITECTURE.md:53 Layering invariant IS the spec.
2. "MCP register ≡ CLI register" behavioral invariant (ARCHITECTURE.md:228) testable only with single function under both layers.
3. Single-shot refactor cost bounded — Worker confirmed.
4. P006a/P006b split fallback retired per Worker measurement.

**Concrete `core::*` API shape — V2 (Architect updated per Worker Turn 1 [O1.1], [O1.2], [O1.3]):**

**V2 design principle (NEW — applies to all 5 `core::*::run` fns):**
- Every `core::*::run` resolves its OWN environment dependencies internally (home dir, launch_agents_dir, self_exe, current_exe).
- The ONLY injected dependency is `&L: LaunchctlClient` trait (for testability via NoopLaunchctl).
- `Args` structs carry user-facing inputs only (label, schedule, force, config_path override, etc.) — NOT env-derived paths.
- Rationale: matches existing `src/cli/*::run_with_deps` real-code pattern (Worker Turn 1 [O1.3] citation `src/cli/register.rs:41-46`), keeps `Args` structs minimal, mirrors `default_config_path()` pattern from P004/P005. Testability preserved via `LaunchctlClient` trait alone.

```rust
// src/core/init.rs — V2 [O1.2 ACCEPT]
pub struct InitArgs {
    pub force: bool,
    pub config_path: Option<PathBuf>,  // None → default ~/.config/advisory-cron/config.toml
}
pub struct InitOutput {
    pub config_path: PathBuf,
    pub written: bool,        // derived from pre-call path.exists() check (see Task 3 V2)
}
pub fn run(args: InitArgs) -> Result<InitOutput>;
// IMPL: resolve home internally via std::env::var("HOME"), compute path,
//       derive `written` from path.exists() BEFORE calling Config::write_default,
//       call Config::write_default(&path, &home, args.force).

// src/core/register.rs — V2 [O1.3 ACCEPT]
pub struct RegisterArgs {
    pub label: String,
    pub schedule: Option<String>,   // M H * * * form; None → use config schedule
    pub config_path: Option<PathBuf>,
}
pub struct RegisterOutput {
    pub plist_path: PathBuf,
    pub label: String,
    pub bootstrapped: bool,
}
pub fn run<L: LaunchctlClient>(args: RegisterArgs, client: &L) -> Result<RegisterOutput>;
// IMPL: resolve home internally via std::env::var("HOME"),
//       compute launch_agents_dir via default_launch_agents_dir(&home),
//       resolve self_exe via std::env::current_exe(),
//       NO new fields on RegisterArgs beyond user inputs.

// src/core/unregister.rs — V2 [O1.3 ACCEPT, analogous]
pub struct UnregisterArgs {
    pub label: String,
    pub config_path: Option<PathBuf>,   // reserved (parity with CLI)
}
pub struct UnregisterOutput {
    pub label: String,
    pub plist_existed: bool,
    pub was_loaded: bool,
}
pub fn run<L: LaunchctlClient>(args: UnregisterArgs, client: &L) -> Result<UnregisterOutput>;
// IMPL: resolve home internally,
//       compute launch_agents_dir internally,
//       no env-derived paths in Args.

// src/core/run.rs — V2 [analogous to O1.3 pattern]
pub struct RunArgs {
    pub config_path: Option<PathBuf>,
}
pub struct RunOutput {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub heartbeat_appended: bool,
}
pub async fn run(args: RunArgs) -> Result<RunOutput>;
// IMPL: resolve home internally (only if config_path is None), load config,
//       call runner::fire_task, append heartbeat. No deps injected (no launchctl needed).

// src/core/status.rs — V2 [analogous to O1.3 pattern]
pub struct StatusArgs {
    pub label: Option<String>,
    pub config_path: Option<PathBuf>,
    pub last: usize,        // default 5
}
pub use crate::heartbeat::HeartbeatRecord;
pub struct StatusReport {
    pub label: String,
    pub plist_loaded: bool,
    pub next_fire: Option<String>,
    pub heartbeat_log_path: String,
    pub last_runs: Vec<HeartbeatRecord>,
}
pub fn run<L: LaunchctlClient>(args: StatusArgs, client: &L) -> Result<StatusReport>;
// IMPL: resolve home internally (if config_path is None), load config,
//       call client.print() for launchctl state, read heartbeat log.
```

**Note on `StatusReport` move:** unchanged from V1. MOVE to `src/core/status.rs`, make pub. CLI handler re-uses via `use crate::core::status::StatusReport`.

**Dependency injection pattern — V2 clarification:**
- `LaunchctlClient` trait = ONLY injected dep across all core::* fns that need launchctl (register, unregister, status).
- `init` and `run` need no client (init touches filesystem only; run spawns child + writes heartbeat — both via existing modules without trait injection).
- All other deps (home, launch_agents_dir, self_exe, current_exe) resolved INSIDE the `core::*::run` body using stdlib calls.
- CLI shells pass `&RealLaunchctl`; tests pass `&NoopLaunchctl`; MCP tool handlers pass `&RealLaunchctl`.

---

**Decision 3 — MCP tool schema source → Option X (HAND-WRITE schemas) — CONFIRMED V2.**

Worker Turn 1 verified: rmcp's `Tool.input_schema = Arc<JsonObject>` where `JsonObject = serde_json::Map<String, Value>`. Hand-written schemas via `serde_json::json!({...}).as_object().cloned()` fully viable. No `#[derive(JsonSchema)]` needed on our types. Decision X CONFIRMED.

(Tool schemas table unchanged from V1 — see V1 Decision 3 for 5 schemas; Worker copies verbatim.)

```jsonc
// init
{
  "name": "init",
  "description": "Write a default advisory-cron config to ~/.config/advisory-cron/config.toml (or path specified). Refuses overwrite unless force=true.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "force": { "type": "boolean", "description": "Overwrite existing config", "default": false },
      "config_path": { "type": "string", "description": "Optional override path" }
    }
  }
}

// register
{
  "name": "register",
  "description": "Generate a launchd plist and bootstrap it for the given label. Schedule in M H * * * form (daily) overrides config schedule if provided.",
  "inputSchema": {
    "type": "object",
    "required": ["label"],
    "properties": {
      "label": { "type": "string", "description": "Label (ASCII alphanumeric + -_)" },
      "schedule": { "type": "string", "description": "Cron M H * * * form (daily only)" },
      "config_path": { "type": "string" }
    }
  }
}

// unregister
{
  "name": "unregister",
  "description": "Boot out the launchd job for the given label and remove the plist file. Idempotent.",
  "inputSchema": {
    "type": "object",
    "required": ["label"],
    "properties": {
      "label": { "type": "string" },
      "config_path": { "type": "string" }
    }
  }
}

// run
{
  "name": "run",
  "description": "Fire the configured task once; capture stdout/stderr; append heartbeat. Returns exit_code, duration_ms, tails.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "config_path": { "type": "string" }
    }
  }
}

// status
{
  "name": "status",
  "description": "Read launchd state + last N heartbeats. Returns plist_loaded, next_fire (configured recurrence), heartbeat_log_path, last_runs.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "label": { "type": "string" },
      "config_path": { "type": "string" },
      "last": { "type": "integer", "minimum": 0, "default": 5 }
    }
  }
}
```

Output schemas omitted — MCP spec allows tools to return arbitrary JSON; Claude Desktop infers shape from response.

---

**Decision 4 — Subcommand wiring → unchanged.** `src/mcp/{mod,server,tools}.rs` + thin CLI delegator `src/cli/mcp.rs`. (See V1 for full rationale.)

```rust
// src/cli/mcp.rs
#[derive(Debug, clap::Args)]
pub struct Args {
    // intentionally empty — stdio MCP has no flags in Phase 1
}
```

---

**Decision 5 — Integration test shape → unchanged.** HYBRID: 1 binary smoke test + N in-process tests + parity test. (See V1 for full rationale.)

---

**Decision 6 — Claude Desktop config snippet → unchanged.** README.md section + ARCHITECTURE.md MCP surface section. (See V1 for full content.)

---

**Constraint #1 retirement — unchanged.** P006 may add exactly +3 lines to `src/cli/mod.rs` (mod decl + enum variant + dispatch arm). Constraint reinstated for P007+.

---

**Layering migration risk + mitigation — unchanged.** Pure mechanical extraction + CLI shell preserves exit code mapping + regression tests are safety net.

**Acceptance — single function check — unchanged.**

### Scope — unchanged from V1.

### Skills consulted (optional)

V2 update: Worker Turn 1 did the rmcp probe work (no Skills needed retroactively). All 35 anchors verified ✅.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Mỗi anchor carry humility marker. `[verified]` = em đã Read file confirm. `[unverified]` = docs imply, em chưa Read source. `[needs Worker verify]` = punt cho Thợ grep/probe.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `rmcp` crate exists on crates.io with stdio transport for MCP server | `cargo search rmcp` + `cargo add rmcp --dry-run` + read docs.rs/rmcp or context7 query | `[needs Worker verify]` | ✅ `rmcp = "1.7.0"` on crates.io. Official Anthropic Rust SDK. stdio() fn confirmed in transport/io.rs. API: `service.serve(stdio()).await?` + `service.waiting().await?`. |
| 2 | `rmcp` runtime model: does it REQUIRE multi-thread tokio, or work with `current_thread`? Current advisory-cron uses `#[tokio::main(flavor = "current_thread")]` per `src/main.rs` (P001 ship — DISCOVERIES.md:27 `[verified]`) | Read rmcp docs / examples; probe with `tokio::main(flavor = "current_thread")` first; if rmcp panics → switch to multi-thread + add `rt-multi-thread` to Cargo.toml tokio features | `[needs Worker verify]` | ✅ Compatible with `current_thread`. Probe compiled successfully. rmcp uses `tokio::spawn` (not `spawn_blocking`), fully compatible with `current_thread` runtime. No `rt-multi-thread` change needed. **V2: tokio `io-std` feature MUST be added (required by `transport-io` feature) — see Task 1 V2.** |
| 3 | `src/cli/mod.rs::Commands` enum has 5 variants currently (Init, Register, Unregister, Run, Status), each in newtype dispatch form (P001 ship) — P006 adds 6th variant `Mcp(mcp::Args)` | Read `src/cli/mod.rs` | `[needs Worker verify]` | ✅ Confirmed `src/cli/mod.rs:14-26`. 5 variants, uniform newtype pattern. `dispatch()` at line 28. **CRITICAL V2: `dispatch()` returns `anyhow::Result<u8>` NOT `Result<()>` — all CLI handlers return `Result<u8>` — see [O1.1] ACCEPT + Task 6 V2 fix.** |
| 4 | `src/main.rs` declares `pub mod cli;` and uses `#[tokio::main(flavor = "current_thread")]` (P001 ship, DISCOVERIES.md:27 `[verified]`) | Read `src/main.rs` | `[needs Worker verify]` | ✅ Confirmed `src/main.rs:6` = `mod cli;`, line 27 = `#[tokio::main(flavor = "current_thread")]`. Note: current mods are `mod` not `pub mod`. P006 adds `mod core; mod mcp;` (not necessarily `pub mod` — check visibility needs during EXECUTE). |
| 5 | `src/cli/init.rs::run` exists with body that calls `Config::write_default` + maps exit codes (P002 ship, CHANGELOG `[verified per Read CHANGELOG:153-156]`) | Read `src/cli/init.rs` | `[verified per CHANGELOG]` | ✅ CHANGELOG cites Phase 1.2 ship. P006 extracts body to `core::init::run`, CLI shell becomes 10 lines. **V2: `Config::write_default` real signature `(path, home, force) -> Result<()>` per Worker [O1.2] — `core::init` derives `written` from pre-call `path.exists()` check.** |
| 6 | `src/cli/register.rs::run` / `run_with_deps<L: LaunchctlClient>` exists (P003 ship, CHANGELOG `[verified per Read CHANGELOG:104-108]`) | Read `src/cli/register.rs` | `[verified per CHANGELOG]` | ✅ CHANGELOG cites "Testable surface via `run_with_deps<L: LaunchctlClient>`". **V2: real signature is 4-arg `run_with_deps(args, launchctl, launch_agents_dir, home)` per Worker [O1.3] — `core::register::run` resolves `launch_agents_dir` + `home` + `self_exe` internally, exposes only `args + &client`.** |
| 7 | `src/cli/unregister.rs::run` exists, idempotent (P003 ship, CHANGELOG:105 `[verified]`) | Read `src/cli/unregister.rs` | `[verified per CHANGELOG]` | ✅ Same pattern as register. **V2: also has 4-arg `run_with_deps` per Worker [O1.3] — `core::unregister::run` resolves env internally.** |
| 8 | `src/cli/run.rs::run` exists, loads config, fires task via `runner::fire_task`, appends heartbeat, `default_config_path` bails on `$HOME` unset (P004 ship, CHANGELOG:63-67 `[verified per Read CHANGELOG]`) | Read `src/cli/run.rs` | `[verified per CHANGELOG]` | ✅ Extract to `core::run::run` async fn. V2 unchanged. |
| 9 | `src/cli/status.rs::run` exists with `StatusReport` struct (currently private — P005 Discovery `[verified per Read docs/discoveries/P005.md:73-87]`); `parse_next_fire` private fn; `is_valid_label` private fn | Read `src/cli/status.rs` | `[verified per P005 Discovery]` | ✅ P005 Discovery explicitly flags StatusReport visibility decision for P006. Architect Decision 2: MOVE StatusReport to `src/core/status.rs` as `pub struct`. `parse_next_fire` + `is_valid_label` MOVE alongside. |
| 10 | `src/launchd.rs::LaunchctlClient` trait has `bootstrap(&self, plist_path: &Path) -> Result<()>`, `bootout(&self, label: &str) -> Result<()>`, `print(&self, label: &str) -> Result<LaunchctlPrintOutput>` methods (P003 + P005 ship, P005 Anchor #3 V2 `[verified per Worker Turn 1]`) | P005 Discovery + CHANGELOG | `[verified per CHANGELOG]` | ✅ Trait stable post-P005. P006 does NOT extend trait. |
| 11 | `Cargo.toml` `[dependencies]` currently has clap, serde, toml, chrono, tokio (features rt/macros/process/time/fs), anyhow, thiserror, tracing, tracing-subscriber, reqwest, serde_json (P004 ship) | Read `Cargo.toml` | `[verified]` | ✅ Read Cargo.toml — 11 explicit deps as listed. **V2: P006 adds `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` AND adds `io-std` to tokio features.** |
| 12 | `Cargo.toml` `[dev-dependencies]` has `tempfile = "3"` and `tokio-test = "0.4"` | Read `Cargo.toml` | `[verified]` | ✅ Read Cargo.toml lines 27-28. P006 reuses both for `tests/cli_mcp.rs`. |
| 13 | Binary size budget: PROJECT.md acceptance bullet 9 `[verified per Read PROJECT.md:60]` says < 7MB | Read PROJECT.md | `[verified]` | ✅ "raised from 5MB to budget MCP SDK". Current P005 binary 1.2MB. Worker probe: projected total 1.6-2.2MB — well under 7MB. |
| 14 | ARCHITECTURE.md:53 "Layering invariant" defines `core::*` must be CLI/MCP-agnostic — single code path = single behavior | Read ARCHITECTURE.md:53 | `[verified]` | ✅ Spec already documented. P006 makes it real. Decision 2 Option A IS the literal implementation. |
| 15 | ARCHITECTURE.md:42-45 predicts module layout `src/cli/mcp.rs`, `src/core/mod.rs`, `src/mcp/server.rs`, `src/mcp/tools.rs` | Read ARCHITECTURE.md:42-45 | `[verified]` | ✅ Architect honors predicted layout (Decision 4). |
| 16 | ARCHITECTURE.md:66 CLI surface row `mcp` already documented: Args = "(no args — stdio only)", Behavior = "Start MCP server on stdin/stdout; serves 5 tools mirroring above", Phase = "1.7" | Read ARCHITECTURE.md:66 | `[verified]` | ✅ Row present, status pending. P006 EXECUTE marks Phase 1.7 ✅. |
| 17 | ARCHITECTURE.md:77 exit code 5 = "MCP transport error (subcommand `mcp` only — stdio closed, malformed JSON-RPC)" | Read ARCHITECTURE.md:77 | `[verified]` | ✅ Exit code reserved. **V2: `src/cli/mcp.rs` maps rmcp transport errors → `return Ok(5)` (not `process::exit(5)`) per Worker [O1.1] ACCEPT. Other tool-handler errors → exit 1 (generic) via core's Err path.** |
| 18 | ARCHITECTURE.md:197-228 §MCP surface section exists with tool registry table + Claude Desktop sketch + behavioral invariant | Read ARCHITECTURE.md:197-228 | `[verified]` | ✅ Section pre-populated. P006 Docs Gate updates "TBD by Architect" placeholders → concrete schemas from Decision 3. |
| 19 | PROJECT.md:37 MVP scope item 6 + acceptance bullets 58-64 specify the 5-tool MCP server requirement | Read PROJECT.md:37 + 58-64 | `[verified]` | ✅ Acceptance criteria locked. |
| 20 | RULES.md:13-22 Tầng 1 triggers: CLI subcommand added, Cargo.toml dep add, exit code semantic, module added, security boundary touched | Read RULES.md:13-22 | `[verified]` | ✅ ALL 5 triggers fire for P006. Tầng 1 mandatory. |
| 21 | Pre-P006 max INV in INVARIANTS.md = INV-17 (P005 ship) | DISCOVERIES.md P005 entry + Read INVARIANTS.md:232 | `[verified]` | ✅ INV-17 last. INV-18 slot available. |
| 22 | Existing test count post-P005 = 70 (per CHANGELOG:45 `[verified]`); P006 adds MCP handshake test + 5-7 tool tests + parity test → estimate +7-10 tests → ~77-80 total | Read CHANGELOG:45 | `[verified]` | ✅ Baseline 70. Post-P006 target 77+. Existing 70 MUST stay green. |
| 23 | `src/cli/mod.rs` newtype dispatch pattern is uniform across 5 subcommands (each `Commands::<Cap>(<sub>::Args)`, dispatch matches variant → `<sub>::run(args).await`) — Architect adds 6th `Mcp(mcp::Args)` following exact same template | P003 V2 [O1.1] doctrine + P005 Anchor #2 evidence `[verified per Worker]` | `[verified per Worker Turn 1]` | ✅ Verified. `src/cli/mod.rs` lines 6-12 = 5 `pub mod` declarations. Lines 14-26 = `Commands` enum. Lines 28-36 = `dispatch` fn returning `anyhow::Result<u8>`. Exactly +3 lines diff. |
| 24 | `default_config_path() -> Result<PathBuf>` pattern from P004 (Constraint #16, bail on `$HOME` unset) is currently DUPLICATED across `src/cli/run.rs` + `src/cli/status.rs` — P006 EXTRACTION moves this to a shared `src/core/config_path.rs` (private to `core`) | P004 V2 + P005 phiếu | `[verified per P005 phiếu]` | ✅ Duplication acknowledged in P005 Anchor #11. P006 fixes it. New file `src/core/config_path.rs`. |
| 25 | INV-12 label sanitization 2-point enforcement applies to MCP tool boundary — MCP tools must validate label at MCP tool entry BEFORE invoking `core::*` (defense-in-depth) | INV-12 spec at `docs/security/INVARIANTS.md:137-153` | `[verified]` | ✅ P006 INV-18 (new — see Task 8) extends INV-12 to MCP tool boundary explicitly. |
| 26 | INV-10/14/15/16 REMAIN VALID after extraction because `core::*` REUSES `src/launchd.rs` + `src/runner.rs` + `src/heartbeat.rs` unchanged | INVARIANTS.md INV-10/14/15/16 spec | `[verified]` | ✅ No INV-1 through INV-17 changes. Only INV-18 appended. |
| 27 | Claude Desktop config path = `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS (Bootstrap CHANGELOG line 249 `[verified per Read CHANGELOG:249-250]`) | Read CHANGELOG | `[verified]` | ✅ Bootstrap entry confirms path. |
| 28 | Tests in `tests/cli_help.rs` use `each_subcommand_help_exits_zero` which exercises ALL subcommands' `--help` — pick up new `mcp` automatically (per P005 Discovery `[verified per Read docs/discoveries/P005.md:27]`) | P005 Discovery | `[verified per P005 Discovery]` | ✅ Confirmed self-extending. |
| 29 | `LaunchctlClient` trait can be passed to `core::register::run<L: LaunchctlClient>(...args, &client)` — generic dispatch works in core. MCP tool handler in `src/mcp/tools.rs` creates `RealLaunchctl` (unit struct) and passes by reference | P003 trait shape + P005 Anchor #5 evidence | `[verified per CHANGELOG]` | ✅ Trait stable. Both CLI and MCP construct `RealLaunchctl`. |
| 30 | Phase 1.6 (README + ARCHITECTURE polish) comes AFTER P006 per BACKLOG.md:30 — but Phase 1.6 only POLISHES; P006 ships the FIRST cut | BACKLOG.md:30 `[verified]` | `[verified]` | ✅ P006 ships first README + ARCH MCP content. Phase 1.6 polishes. |
| 31 | `serde_json::to_value` / `from_value` works for the `*Output` / `StatusReport` structs (all derive `Serialize` + `Deserialize` for round-trip in MCP tool boundary) | serde_json common API + P004 HeartbeatRecord pattern | `[verified per CHANGELOG]` | ✅ HeartbeatRecord already Serialize+Deserialize. P006 derives same on all new types. |
| 32 | Behavioral invariant test "MCP register ≡ CLI register" feasible via `NoopLaunchctl` shared between both paths | P005 NoopLaunchctl evidence + P003 ship | `[verified per CHANGELOG]` | ✅ NoopLaunchctl records calls in Mutex Vec. Both paths share same instance → assertions trivial. |
| 33 | Hand-written JSON Schema strings for 5 tools fit in <100 LOC of `src/mcp/tools.rs` | Decision 3 + schema sizes ~16 lines each × 5 = ~80 | `[verified per Architect-authored schemas]` | ✅ Schemas in Decision 3 section sum ~80 lines. |
| 34 | Worker may need to add `Cargo.toml` `[features]` block if rmcp ships as feature-gated | `[needs Worker verify rmcp feature set]` | `[verified per Worker Turn 1]` | ✅ Confirmed: `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`. `macros` feature DROPPED (Decision 1 V2 — hand-written approach doesn't need `#[tool]` proc macros, saves pastey). |
| 35 | Sub-mech E env drift: rmcp + transitive deps cleanly resolve | `cargo update --dry-run` + clean rebuild | `[verified per Worker Turn 1]` | ✅ `cargo update --dry-run` baseline clean. rmcp 1.7.0 new unique runtime deps: schemars 1.x, tokio-util 0.7.x, async-trait 0.1.x (compile-time). thiserror version mismatch (advisory-cron=1, rmcp=2) → both compiled in parallel, no blocker. Projected binary ~1.6-2.2MB. |

**V2 status:** All 35 anchors ✅ VERIFIED by Worker Turn 1. No `[needs Worker verify]` blockers remain.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi Quản đốc triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (Architect responded to Turn 1)

### Turn 1 — Worker Challenge

**Date:** 2026-05-27

**Anchor verification (all 35 anchors, focusing on the 7 `[needs Worker verify]` first):**

- Anchor #1 (`rmcp` exists): ✅ `rmcp = "1.7.0"` on crates.io. Official Anthropic Rust MCP SDK. `cargo search rmcp` confirms. crates.io API: `max_version = 1.7.0`, description = "Rust SDK for Model Context Protocol".
- Anchor #2 (rmcp tokio runtime): ✅ Compatible with `current_thread`. Verified: compiled rmcp 1.7.0 probe binary with `#[tokio::main(flavor = "current_thread")]` — `cargo build` succeeded with no errors. rmcp uses `tokio::spawn` internally (service.rs:729) which works on current_thread runtime. No `spawn_blocking` found. **No tokio features change needed.**
- Anchor #3 (`src/cli/mod.rs` Commands enum shape): ✅ Confirmed — `src/cli/mod.rs:14-26` has 5-variant newtype dispatch enum: `Init(init::Args)`, `Register(register::Args)`, `Unregister(unregister::Args)`, `Run(run::Args)`, `Status(status::Args)`. Pattern uniform across all 5. P006 adds 6th `Mcp(mcp::Args)`.
- Anchor #4 (`src/main.rs` mod declarations + tokio flavor): ✅ Confirmed `src/main.rs:6-10` declares `mod cli; mod config; mod heartbeat; mod launchd; mod runner;`. `src/main.rs:27` = `#[tokio::main(flavor = "current_thread")]`. P006 adds `pub mod core; pub mod mcp;`. tokio flavor stays `current_thread` (Anchor #2 clean).
- Anchor #23 (`src/cli/mod.rs` exact structure): ✅ Verified. `src/cli/mod.rs` lines 6-12 = 5 `pub mod` declarations. Lines 14-26 = `Commands` enum. Lines 28-36 = `dispatch` fn. Exactly +1 mod decl + +1 enum variant + +1 match arm = 3 lines diff as phiếu predicts. **Critical finding for O1.1 below.**
- Anchor #34 (rmcp feature flags): ✅ Confirmed. rmcp `server` feature requires: `["transport-async-rw", "dep:schemars", "dep:pastey"]`. `transport-io` feature adds stdio support: `["transport-async-rw", "tokio/io-std"]`. Correct Cargo.toml entry: `rmcp = { version = "1.7.0", features = ["server", "transport-io", "macros"] }`. schemars is a mandatory transitive dep via `server` feature — but as expected, no need to add schemars as direct dep.
- Anchor #35 (env drift / dep resolve): ✅ `cargo update --dry-run` baseline clean. rmcp 1.7.0 dep resolution clean. New unique runtime deps added: `schemars 1.x`, `tokio-util 0.7.x`, `async-trait 0.1.x` (proc macro — stripped), `pin-project-lite` (already present). `futures-util` already in advisory-cron tree via reqwest. No version conflicts except `thiserror`: advisory-cron has `thiserror = "1"`, rmcp requires `thiserror ^2` — Cargo will compile both versions in parallel (acceptable, both are proc-macro crates stripped from release binary).
- Anchors #5–#12 (`[verified per CHANGELOG]`): ✅ All CLI handler files read and confirmed. init.rs = 43 LOC, register.rs = 119 LOC, unregister.rs = 82 LOC, run.rs = 110 LOC, status.rs = 380 LOC. Total CLI handler LOC = 734. Extraction delta will be well under 600 LOC (most lines are just relocated). **P006 stays as single phiếu (no P006a/P006b split needed).**
- Anchors #13, #19 (binary size budget + PROJECT.md acceptance): ✅ Current binary = 1.2MB. rmcp probe (rmcp+tokio+anyhow, stripped release) = 1.4MB. advisory-cron already has reqwest, tracing-subscriber, clap — much larger footprint than the probe. New unique contributions from rmcp: schemars (~0.3MB), tokio-util (~0.1MB), rmcp code (~0.2MB). Projected total: ~1.6–2.2MB. **Well under 7MB budget** with ~4.8MB headroom.
- Anchors #14–#18, #20–#22, #24–#33 (`[verified]` by Architect): ✅ Worker spot-checked: ARCHITECTURE.md layering invariant confirmed, exit codes table confirmed, INV-17 last confirmed in INVARIANTS.md, StatusReport struct confirmed as private in `src/cli/status.rs:40-47` (non-pub, `struct StatusReport`), `is_valid_label` and `parse_next_fire` confirmed as private fns in `src/cli/status.rs:141` and `src/cli/status.rs:186`. All CHANGELOG citations consistent with code reality.

**rmcp API surface (verified via GitHub source + probe build):**
- stdio() fn: `pub fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout)` — in `transport/io.rs`
- Server pattern: `MyHandler.serve(stdio()).await?` → `server.waiting().await?`
- `ServerHandler` trait has default impls for `list_tools` (returns empty), `call_tool` (returns method-not-found), `get_info` (returns default). Worker can implement these manually without macros.
- `Tool` struct: `{ name: Cow<'static, str>, description: Option<Cow<'static, str>>, input_schema: Arc<JsonObject>, ... }` where `JsonObject = serde_json::Map<String, Value>`. Hand-written schemas via `serde_json::json!({}).as_object().unwrap().clone()` — fully viable, no `#[derive(JsonSchema)]` needed on our types. **Decision 3 (hand-write schemas) CONFIRMED viable**.
- Tool result: `CallToolResult::success(IntoContents)` — `String` impl `IntoContents` so `serde_json::to_string(&output)?` works.

---

**Objections — 3 Tầng 1 issues found (function signatures don't match real code):**

**[O1.1] — `src/cli/mcp.rs::run` return type WRONG — causes compile error**

Phiếu Task 6 code block (`src/cli/mcp.rs:2-9`) prescribes:
```rust
pub async fn run(_args: Args) -> Result<()> { ... }
```
Real code at `src/cli/mod.rs:28`:
```rust
pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>
```
All 5 existing CLI handlers (`src/cli/init.rs:18`, `register.rs:34`, `unregister.rs:24`, `run.rs:24`, `status.rs:49`) return `Result<u8>`. The match arm `Commands::Mcp(args) => mcp::run(args).await` must also return `Result<u8>` or the match expression won't type-check.

Also: `mcp::run`'s `process::exit(5)` in the `.map_err` closure returns `!` and panics the type inference — the correct pattern for `Result<u8>` is `return Ok(5)` in an error path (matching how the other 4 handlers return `Ok(3)` or `Ok(2)` etc.).

Fix required: `pub async fn run(_args: Args) -> Result<u8>` with transport errors mapped to `return Ok(5)` (not `process::exit(5)`).

**[O1.2] — `Config::write_default` signature mismatch — `InitOutput.written` not derivable**

Phiếu Task 3 prescribes:
```rust
let written = Config::write_default(&path, args.force)?;
// implies signature: write_default(path, force) -> Result<bool>
```
Real signature at `src/config.rs:123`:
```rust
pub fn write_default(path: &Path, home: &Path, force: bool) -> Result<()>
```
- Takes a `home: &Path` second arg (not present in `InitArgs`)
- Returns `Result<()>` not `Result<bool>`
- On "config exists + no force" → returns `Err(anyhow!("config already exists..."))` NOT `Ok(false)`

The `InitOutput { written: bool }` cannot be derived as written. Fix options:
- A (Recommended): Adjust `core::init::run` to: (1) take `home: Option<PathBuf>` in `InitArgs` OR resolve home internally like existing `init.rs` does, (2) call `Config::write_default(&path, &home, args.force)`, (3) derive `written` by checking `path.exists()` BEFORE the call — if it already existed and `force=false`, `written=false`; otherwise `written=true`.
- B: Change `InitOutput.written` semantics to `config_existed_before: bool` derived from `path.exists()` check BEFORE calling `write_default`.

**[O1.3] — `core::register::run` proposed API missing `home` and `self_exe` parameters**

Phiếu Decision 2 / Task 4 proposes:
```rust
pub fn run<L: LaunchctlClient>(args: RegisterArgs, client: &L) -> Result<RegisterOutput>
```
Real `src/cli/register.rs:41-46` `run_with_deps` takes 4 args:
```rust
pub async fn run_with_deps<L: LaunchctlClient>(
    args: Args,
    launchctl: &L,
    launch_agents_dir: &std::path::Path,  // MISSING from core API
    home: &std::path::Path,               // MISSING from core API
) -> Result<u8>
```
`launch_agents_dir` is needed for `plist_path_for` + `fs::create_dir_all`. `home` is needed for config path resolution AND `default_launch_agents_dir(&home)`. Additionally, `src/cli/register.rs:76` uses `env::current_exe()` to get `self_exe` for `generate_plist` — this is NOT in `RegisterArgs` and NOT in `src/launchd::generate_plist`'s known API either.

Same applies to `core::unregister` — `run_with_deps` at `src/cli/unregister.rs:31-35` takes `launch_agents_dir` not present in proposed API.

Fix options:
- A (Recommended): `core::register::run` and `core::unregister::run` resolve `home` internally (via `std::env::var("HOME")`), mirroring the `default_config_path()` pattern. The `launch_agents_dir` is then computed from `home` inside the function. `self_exe` resolved via `env::current_exe()` inside the function. The `client: &L` remains the only injected dependency (testable via NoopLaunchctl). `launch_agents_dir` stays as an internal detail, NOT exposed in `RegisterArgs`.
- B: Add `launch_agents_dir: Option<PathBuf>` to `RegisterArgs` (explicit injection for tests). More verbose but maintains dependency injection discipline.

---

**Additional research findings (non-blocking, inform EXECUTE):**

1. **rmcp `macros` feature optional for hand-written approach**: `#[tool]`, `#[tool_router]`, `#[tool_handler]` macros require `macros` feature. Since Decision 3 uses hand-written schemas + manual `ServerHandler` impl, the `macros` feature can be DROPPED. Recommended Cargo.toml: `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`. This reduces compile time and avoids proc-macro dependency `pastey`. Worker self-decide (Tầng 2) in EXECUTE.

2. **schemars is mandatory transitive dep** (not avoidable with `server` feature) but does NOT require `#[derive(JsonSchema)]` on our own types. Decision 3 (hand-written schemas) fully compatible — construct `Tool.input_schema` via `serde_json::json!({...}).as_object().cloned()`.

3. **Total CLI handler LOC = 734** (init=43, register=119, unregister=82, run=110, status=380). status.rs is large (380 LOC) because it has 11 unit tests + `parse_next_fire` + `render_human` + `tail_first_n_or_empty`. The **extractable logic (non-test, non-render) is ~120 LOC for status** — fits in single phiếu easily. **No P006a/P006b split needed.**

4. **Constraint #1 retirement is legitimate**: Worker agrees. Confirmed `src/cli/mod.rs` has never been touched since P003. Exactly 3 lines needed (+mod decl, +enum variant, +match arm). Pattern unchanged.

5. **Tokio `io-std` feature**: `transport-io` feature in rmcp adds `tokio/io-std` requirement. Current `Cargo.toml` tokio features = `["rt", "macros", "process", "time", "fs"]` — does NOT include `io-std`. Worker must add `io-std` to tokio features. This is a Tầng 1 trigger (Cargo.toml change) — already in P006 scope (Task 1 note "possibly add tokio feature additions"). Worker notes this as a concrete action during EXECUTE Task 1.

6. **INV-18 new invariant**: Worker accepts the invariant as specified. Will append to INVARIANTS.md per Task 8.

---

**Proposed resolutions for Architect RESPOND:**

[O1.1]: Architect confirm — `mcp::run` returns `Result<u8>`, transport error path = `return Ok(5)` (not `process::exit`).

[O1.2]: Architect pick Option A or B above. Worker recommends Option A: resolve home internally in `core::init::run` (mirrors the existing `init.rs` `home_dir()` pattern), derive `written` from a pre-call `path.exists()` check.

[O1.3]: Architect pick Option A or B above. Worker recommends Option A: home + launch_agents_dir + self_exe resolved internally in `core::register::run` and `core::unregister::run`. Client (`&L`) remains the only injected dep (test-friendliness preserved). This makes `RegisterArgs` cleaner and the API matches the phiếu's original intent.

**Status:** ✅ RESPONDED (Architect Turn 1 — phiếu V2)

### Turn 1 — Architect Response (phiếu V2)

**Date:** 2026-05-27

**Verdicts per objection:**

- **[O1.1] → ACCEPT.** Worker's real-code citation (`src/cli/mod.rs:28` returns `anyhow::Result<u8>`; all 5 handlers return `Result<u8>`) is correct. V1 phiếu was wrong — `mcp::run -> Result<()>` would not type-check inside the dispatch match arm. Action: Task 6 V2 updated — `pub async fn run(_args: Args) -> Result<u8>` with transport error path `return Ok(5)`. The match arm in `src/cli/mod.rs` dispatch fn keeps the existing `Result<u8>` contract; no dispatch-level change. Also updated phiếu §Decision 2 + Task 6 + Anchor #3/#17 to reflect this.

- **[O1.2] → ACCEPT (Option A — Worker's recommendation).** Worker's citation of `src/config.rs:123` `write_default(path: &Path, home: &Path, force: bool) -> Result<()>` is correct. V1 phiếu assumed a phantom 2-arg `Result<bool>` signature. Adopting Option A: `core::init::run` resolves `home` internally via `std::env::var("HOME")` (mirrors `default_config_path()` pattern), derives `written` from `path.exists()` check BEFORE calling `Config::write_default(&path, &home, args.force)`. On `Err` from `write_default` due to "config exists + no force" → `core::init::run` propagates the Err; CLI shell catches it and maps to exit 2 (preserving P002 ship behavior). Action: Task 3 V2 rewritten — see below.

- **[O1.3] → ACCEPT (Option A — Worker's recommendation).** Worker's citation of `src/cli/register.rs:41-46` 4-arg `run_with_deps(args, launchctl, launch_agents_dir, home)` + `src/cli/register.rs:76` inline `env::current_exe()` is correct. V1 phiếu's `RegisterArgs { label, schedule, config_path }` was incomplete. Adopting Option A: `core::register::run` + `core::unregister::run` resolve `home` (via `std::env::var("HOME")`), `launch_agents_dir` (via `default_launch_agents_dir(&home)`), `self_exe` (via `std::env::current_exe()`) ALL internally. Only `&L: LaunchctlClient` injected. `RegisterArgs` stays minimal (user-facing inputs only). Testability via `LaunchctlClient` trait alone — confirmed sufficient for parity test (NoopLaunchctl records `bootstrap()` calls; home + launch_agents_dir + self_exe are environmental constants in test runs, not asserted). Action: Task 4 V2 rewritten — see below.

- **[Non-blocking — tokio `io-std` feature] → ACCEPT.** Worker's finding (rmcp `transport-io` requires `tokio/io-std`) is mechanical truth. Adding `io-std` to existing tokio features array in `Cargo.toml`. Action: Task 1 V2 explicitly mandates the tokio feature add (not "possibly" — definitively).

- **[Non-blocking — drop `macros` feature] → ACCEPT.** Worker's finding (hand-written approach doesn't need `#[tool]` proc-macros; saves `pastey` cost) consistent with Decision 3 X. Action: Pinned `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` (no `macros`). Documented in Decision 1 V2 + Task 1 V2 + Anchor #34.

**Analogous internal-resolution pattern applied to `core::unregister::run`, `core::run::run`, `core::status::run`** (consistency principle — Architect-extended V2):
- All 5 `core::*::run` functions resolve their OWN env dependencies internally (home, launch_agents_dir, self_exe as applicable).
- ONLY `&L: LaunchctlClient` is injected (for fns that need launchctl — register, unregister, status).
- `init` and `run` need no client injection.
- `Args` structs carry user-facing inputs only.
- Decision 2 V2 captures this design principle explicitly. Tasks 3/4/5 all updated.

**Changes summary (V1 → V2):**

| Section | V1 | V2 |
|---------|----|----|
| Header §Ảnh hưởng `Cargo.toml` line | "add `rmcp` + likely `schemars`" | "add `rmcp` (server + transport-io); ADD `io-std` to tokio features" |
| Decision 1 | rmcp + fallback option open | rmcp 1.7.0 LOCKED, fallback retired, features pinned, `macros` DROPPED |
| Decision 2 — `core::*` API shapes | All fns spec'd Args + minimal deps | All fns resolve env internally; only `&L: LaunchctlClient` injected; `InitOutput.written` derived from pre-call `path.exists()` |
| Anchor #3 | `[needs Worker verify]` | `[verified per Worker Turn 1]` + critical note about `Result<u8>` |
| Anchor #5 | basic verified-per-CHANGELOG | V2 note about real `Config::write_default` 3-arg signature |
| Anchor #6, #7 | basic verified-per-CHANGELOG | V2 note about real 4-arg `run_with_deps` signature |
| Anchor #11, #17, #34 | placeholders | V2 confirmations of exact deps/features and exit code path |
| Task 1 | "possibly add tokio feature" | DEFINITIVELY add `io-std`; pin rmcp features |
| Task 3 | `Config::write_default(&path, args.force)` (wrong) | Resolve home + path.exists() pre-check + `Config::write_default(&path, &home, args.force)` |
| Task 4 | RegisterArgs had no env-derived fields, no internal resolution | RegisterArgs unchanged; core::register::run resolves home/launch_agents_dir/self_exe internally |
| Task 5 | run + status same pattern as V1 | Reaffirmed internal env resolution (consistency with O1.3 ACCEPT) |
| Task 6 | `mcp::run -> Result<()>`, `process::exit(5)` | `mcp::run -> Result<u8>`, `return Ok(5)` on transport error |

**Status:** ✅ RESPONDED — phiếu bumped to V2. All 3 mechanical objections ACCEPTED. No DEFER TO SẾP. Ready for Worker Turn 2 (CHALLENGE re-verify) OR approval gate if Worker confirms V2 matches real code.

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Turn 2 — Worker Challenge

**Date:** 2026-05-27

**Scope:** Verify V2 phiếu against real code for all 3 accepted objections (O1.1, O1.2, O1.3) + new Constraint #4 + Cargo.toml spec + Discovery Report task coverage.

**Anchor re-verification (spot-check V2-specific changes):**

- Anchor #3 V2 note (`dispatch` returns `anyhow::Result<u8>`): ✅ Re-confirmed `src/cli/mod.rs:28` — `pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>`. Task 6 V2 code block correctly prescribes `pub async fn run(_args: Args) -> Result<u8>` + `Ok(5)` error path.
- Anchor #5 V2 note (`Config::write_default` 3-arg): ✅ Re-read `src/config.rs:123` — `pub fn write_default(path: &Path, home: &Path, force: bool) -> Result<()>`. Task 3 V2 code block honors this: `Config::write_default(&path, &home, args.force)` with `home_dir()?` resolved internally. Pre-call `path.exists()` check present at code block line 706.
- Anchor #6 V2 note (register 4-arg `run_with_deps`): ✅ Re-read `src/cli/register.rs:41-46` — confirms 4-arg signature `(args, launchctl, launch_agents_dir, home)`. Task 4 V2 correctly moves env resolution inside `core::register::run`.
- Anchor #7 V2 note (unregister analogous): ✅ Re-read `src/cli/unregister.rs:31-35` — actual `run_with_deps` is **3-arg** `(args, launchctl, launch_agents_dir)` (no separate `home` arg — home was used only to compute launch_agents_dir before calling). V2 phiếu Task 4 unregister body comment correctly says "resolve home INTERNALLY → compute launch_agents_dir INTERNALLY" — net result identical, no discrepancy.
- Anchor #11 V2 Cargo.toml: ✅ Re-read `Cargo.toml` — tokio features currently `["rt", "macros", "process", "time", "fs"]`, no `io-std`. Task 1 mandate to add `io-std` is mechanically correct. `rmcp` not yet in file — Task 1 adds it.
- Anchor #34 V2 (rmcp features pinned, `macros` dropped): ✅ Task 1 V2 line 581: `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` — no `macros`. Consistent with Decision 1 V2 + Constraint #12.
- Anchor #17 V2 (exit code 5 via `Ok(5)`): ✅ Task 6 V2 code block + Constraint #10 both state `return Ok(5)` not `process::exit(5)`.

**Constraint #4 coverage check (all 5 core fns):**

Constraint #4: "Every `core::*::run` fn resolves its own `home` / `launch_agents_dir` / `self_exe` / config path internally... ONLY `&L: LaunchctlClient` injected for fns needing launchctl."

- `core::init::run` — resolves home via `home_dir()` + config path via `default_config_path()`; no L injected. ✅ Covered (Task 3 V2 + Decision 2 V2 `init.rs` block).
- `core::register::run<L>` — resolves home+launch_agents_dir+self_exe internally; only `&L` injected. ✅ Covered (Task 4 V2 + Decision 2 V2 `register.rs` block).
- `core::unregister::run<L>` — resolves home+launch_agents_dir internally; only `&L` injected. ✅ Covered (Task 4 V2 + Decision 2 V2 `unregister.rs` block).
- `core::run::run` — resolves config path via `default_config_path()`; no L injected. ✅ Covered (Task 5 V2 + Decision 2 V2 `run.rs` block).
- `core::status::run<L>` — resolves config path via `default_config_path()`; only `&L` injected. ✅ Covered (Task 5 V2 + Decision 2 V2 `status.rs` block).

All 5 covered. Constraint #4 is complete and internally consistent.

**Discovery Report V2 capture check:**

Task 12 explicitly mandates: "V2 capture: record the 3 mechanical objections from CHALLENGE Turn 1 (O1.1 `Result<u8>`, O1.2 `Config::write_default` 3-arg, O1.3 `run_with_deps` 4-arg) and the V2 design principle 'core::*::run resolves env deps internally; only `&LaunchctlClient` injected'." ✅ Present.

**No new Tầng 1 objections found.** All 3 O1.x accepted fixes are present in V2, Cargo.toml spec is mechanically correct against real `Cargo.toml`, Constraint #4 covers all 5 core fns, Discovery Report capture is explicit.

**Worker accepted V2 — no challenges.** Anchor verification: all spot-checked anchors ✅. Ready for Chủ nhà approval gate.

---

### Final consensus
- Phiếu version: V<N>
- Total turns: <count>
- Approved (autonomous narrate or Sếp gate): [date] — code execution may begin

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.
> Purpose: post-mortem trace, especially for autonomous mode where Sếp didn't watch live.
> Append-only — Worker writes, không edit/delete.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

Example expected entries for P006:
```
[2026-05-27T18:00:00Z] event=task0_anchor_1_rmcp_search evidence=cargo search rmcp → "rmcp = \"1.7.0\" — Anthropic official Rust MCP SDK"
[2026-05-27T18:05:00Z] event=task0_anchor_2_runtime_probe evidence=rmcp examples use #[tokio::main(flavor="current_thread")] — compatible, no rt-multi-thread change
[2026-05-27T18:10:00Z] event=task0_anchor_3_mod_rs_layout evidence=src/cli/mod.rs:14-26 Commands enum 5 variants confirmed; dispatch returns Result<u8> at line 28
[2026-05-27T18:30:00Z] event=core_extraction_init_done evidence=src/core/init.rs created; Config::write_default 3-arg signature handled
[2026-05-27T19:00:00Z] event=cargo_build_size_check evidence=target/release/advisory-cron = <X> MB (<7MB budget ✅)
[2026-05-27T19:30:00Z] event=mcp_handshake_test_pass evidence=tests/cli_mcp.rs::initialize_handshake_returns_5_tools PASS
[2026-05-27T19:45:00Z] event=parity_test_pass evidence=core::register::run via CLI path == via MCP path (NoopLaunchctl call vec equal)
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks (RULES.md matrix) BEFORE marking phiếu DONE.
> Fill the table; mark N/A if not applicable to this phiếu.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | Manual: `advisory-cron mcp` then pipe `{"jsonrpc":"2.0","id":1,"method":"initialize",...}` | response includes `serverInfo` + 5 tools | | |
| A (trigger) | Manual: register `advisory-cron` in Claude Desktop config, restart, ask Claude "list tools from advisory-cron" | 5 tools visible | | |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test --all` | ≥70 tests pass (baseline) + MCP tests | | |
| B (capability) | `cargo test --test cli_mcp` | new MCP integration tests pass | | |
| C (migration) | N/A — no schema change (only new files + thin-shell rewrites preserving behavior) | | | N/A |
| D (persistence) | `grep -l "core::register" src/cli/register.rs src/mcp/tools.rs` | ≥2 hits (parity invariant verified mechanically) | | |
| D (persistence) | `grep -l "INV-18" docs/security/INVARIANTS.md` | ≥1 hit | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (rmcp + transitive) | | |
| E (env drift) | `cargo build --release` clean target | exit 0 + binary <7MB | | |
| E (env drift) | `ls -lh target/release/advisory-cron` | size in bytes recorded in Debug Log | | |
| layering | `grep -rn "core::" src/cli/` | every CLI handler calls into `core::<sub>::run` exactly once | | |
| layering | `grep -rn "core::" src/mcp/` | every MCP tool handler calls into `core::<sub>::run` exactly once | | |
| layering | `git diff src/cli/mod.rs` | exactly +3-5 lines (variant, dispatch arm, mod decl) — NO other edits | | |
| layering | `git diff src/launchd.rs src/runner.rs src/heartbeat.rs src/config.rs` | empty (KHÔNG sửa list) | | |
| INV-12/18 | `grep -n "is_valid_label" src/mcp/tools.rs src/core/` | label validation present at MCP tool boundary + core (2-point) | | |

---

## Nhiệm vụ

### Task 0 — Probe rmcp + verify layout assumptions (V2 — mostly already done by Worker Turn 1)

**File:** N/A — research + Debug Log entries only.

**V2 status:** Worker Turn 1 already completed Anchors #1, #2, #3, #4, #23, #34, #35 verification. Task 0 in EXECUTE is now confirmatory:

1. Re-run `cargo search rmcp` → confirm 1.7.0 still latest stable (or note bump).
2. Re-confirm `src/cli/mod.rs:14-26` Commands enum still has 5 variants (no other PR landed between CHALLENGE and EXECUTE).
3. Run `cargo update --dry-run` baseline → compare with post-rmcp-add.
4. Log all anchors to Debug Log.

**Lưu ý:** All V1 fallback paths (rmcp doesn't exist → hand-rolled JSON-RPC) are retired by Worker Turn 1 confirmation. If between CHALLENGE and EXECUTE something changed (e.g., rmcp yanked from crates.io) → STOP, CHALLENGE Turn 2.

### Task 1 — Add `rmcp` dep + add tokio `io-std` feature (Cargo.toml) — V2 [O-non-blocking ACCEPT]

**File:** `Cargo.toml`

**Tìm:** `[dependencies]` block currently ending with `serde_json = "1"` (line 24 `[verified]`).

**Thay bằng / Thêm:**

```toml
# After serde_json line, add:
rmcp = { version = "1.7.0", features = ["server", "transport-io"] }
```

**Tìm:** existing tokio dep line — currently `tokio = { version = "1", features = ["rt", "macros", "process", "time", "fs"] }` (per Worker Turn 1 verification).

**Thay bằng:**

```toml
tokio = { version = "1", features = ["rt", "macros", "process", "time", "fs", "io-std"] }
```

**Lưu ý:**
- `rmcp` features pinned: `server` (mandatory) + `transport-io` (stdio). `macros` DROPPED — hand-written tool registration per Decision 3, saves `pastey` proc-macro.
- `io-std` tokio feature MANDATORY per Worker Turn 1 — `rmcp` `transport-io` feature requires `tokio/io-std` for `Stdin` / `Stdout` async wrappers.
- `current_thread` tokio runtime PRESERVED — no `rt-multi-thread` needed (Worker confirmed rmcp compatible with current_thread).
- INV-5 (dep major bump audit) does NOT apply — this is a NEW dep, not a major bump. BUT INV-5 spirit applies to the new transitive set: schemars 1.x, tokio-util 0.7.x, async-trait, pin-project-lite. Document in CHANGELOG.

### Task 2 — Create `src/core/config_path.rs` (shared $HOME helper) — V2 unchanged

**File:** `src/core/config_path.rs` (NEW)

**Thêm:**

```rust
//! Shared default config path resolver — bails loud on $HOME unset.
//! Used by all core::* run functions to honor PROJECT.md hard line #3
//! ("No magic config discovery beyond 2 paths").

use anyhow::{bail, Result};
use std::path::PathBuf;

pub(crate) fn default_config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        anyhow::anyhow!("$HOME environment variable is not set")
    })?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home)
        .join(".config")
        .join("advisory-cron")
        .join("config.toml"))
}

/// V2 (per Architect Turn 1 RESPOND): shared helper for `core::*` fns that need
/// to resolve $HOME for non-config-path purposes (e.g., launch_agents_dir).
pub(crate) fn home_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        anyhow::anyhow!("$HOME environment variable is not set")
    })?;
    if home.is_empty() {
        bail!("$HOME environment variable is empty");
    }
    Ok(PathBuf::from(home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_path_bails_on_missing_home() {
        let saved = std::env::var("HOME").ok();
        // SAFETY: test-only env mutation — reverts after test, no production unsafe.
        // Rust 2024 mandates `unsafe` wrap on `set_var`/`remove_var` per recent stdlib.
        unsafe { std::env::remove_var("HOME"); }
        let result = default_config_path();
        if let Some(h) = saved { unsafe { std::env::set_var("HOME", h); } }
        assert!(result.is_err());
    }

    #[test]
    fn default_config_path_returns_expected_subpath() {
        unsafe { std::env::set_var("HOME", "/tmp/probe-home"); }
        let p = default_config_path().unwrap();
        assert!(p.ends_with(".config/advisory-cron/config.toml"));
    }
}
```

**Lưu ý:**
- `pub(crate)` visibility — only `core::*` callers, not exposed beyond crate.
- **V2 ADDS `home_dir()` helper** — needed by `core::register::run` + `core::unregister::run` + `core::init::run` for env-internal resolution per O1.3 ACCEPT. Avoids duplicating `std::env::var("HOME")` checks across 4+ files.
- Tests use `unsafe` env mutation — Rust 2024 requires unsafe for `set_var`/`remove_var`. INV-6 (`unsafe` block rationale) requires comment block: "test-only env mutation, reverts after test, no production unsafe."
- This file REPLACES the duplicated `default_config_path` in `src/cli/run.rs` + `src/cli/status.rs`.

### Task 3 — Extract `core::init` — V2 [O1.2 ACCEPT]

**File:** `src/core/init.rs` (NEW)

**Tìm in `src/cli/init.rs`:** the `pub async fn run(args: Args) -> Result<u8>` body — call to `Config::write_default` + `--force` handling + exit-code-driving println.

**Thay bằng / Thêm** in `src/core/init.rs`:

```rust
use crate::config::Config;
use crate::core::config_path::{default_config_path, home_dir};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InitArgs {
    pub force: bool,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitOutput {
    pub config_path: PathBuf,
    pub written: bool,
}

/// V2 (per Architect Turn 1 RESPOND [O1.2 ACCEPT]):
/// - Resolves `home` internally via `home_dir()`.
/// - Derives `written` from pre-call `path.exists()` check (BEFORE `Config::write_default`).
/// - Calls real 3-arg `Config::write_default(path, home, force)` per `src/config.rs:123`.
/// - On Err from write_default ("config exists + no force") → propagates Err.
///   CLI shell catches and maps to exit 2 (preserving P002 ship behavior).
pub fn run(args: InitArgs) -> Result<InitOutput> {
    let path = match args.config_path {
        Some(p) => p,
        None => default_config_path()?,
    };
    let home = home_dir()?;

    let path_existed_before = path.exists();
    // If config exists + no force → write_default returns Err.
    // If write_default succeeds → file was written (either no prior file, or --force).
    match Config::write_default(&path, &home, args.force) {
        Ok(()) => Ok(InitOutput {
            config_path: path,
            // written = true iff write_default succeeded, regardless of prior existence
            // (because with --force=true the existing file IS overwritten = newly written).
            written: true,
        }),
        Err(e) => {
            // "config exists + no force" is the EXPECTED Err path that maps to exit 2.
            // Propagate Err with context — CLI shell pattern-matches the message.
            // Worker MAY refine error matching during EXECUTE if Config::write_default
            // exposes a more specific error type/variant.
            Err(e).map_err(|e| {
                if path_existed_before && !args.force {
                    // Annotate so CLI shell + MCP tool handler can map to "exit 2" / "InitOutput { written: false }" appropriately.
                    e.context("config already exists; use force=true to overwrite")
                } else {
                    e
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    // Move init-pure-logic tests from src/cli/init.rs here if any exist.
    // CLI-specific tests (exit code mapping, output formatting) stay in tests/cli_init.rs.
}
```

Then **rewrite** `src/cli/init.rs` to thin shell:

```rust
use crate::core::init::{InitArgs, run as core_run};
use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[arg(long, default_value_t = false)]
    pub force: bool,
    #[arg(long)]
    pub config: Option<PathBuf>,
}

/// V2: returns `Result<u8>` matching dispatch contract.
pub async fn run(args: Args) -> Result<u8> {
    match core_run(InitArgs {
        force: args.force,
        config_path: args.config,
    }) {
        Ok(output) => {
            println!("wrote default config to {}", output.config_path.display());
            Ok(0)
        }
        Err(e) => {
            // Worker: pattern-match e's chain for "config already exists" → exit 2;
            // otherwise → propagate as generic exit 1 (or whatever P002 ship behavior was).
            // Refer to existing src/cli/init.rs error mapping for exact pattern.
            eprintln!("{e:#}");
            // V2: preserve P002 ship exit code 2 for "config exists + no force"
            if format!("{e:#}").contains("config already exists") {
                Ok(2)
            } else {
                Ok(1)
            }
        }
    }
}
```

**Lưu ý:**
- **V2 PER [O1.2] ACCEPT:** `Config::write_default(path, home, force) -> Result<()>` real signature honored. `InitOutput.written` derived in `core::init::run` body via pre-call `path.exists()` + Ok/Err of `write_default`. CLI shell catches Err and maps to exit 2 for the "exists + no force" case.
- Preserve exit code semantics: exit 2 if "config already exists" Err propagates, exit 0 on success, exit 1 for other errors (matches P002 ship behavior per CHANGELOG:156 `[verified]`).
- Worker MAY refine the Err pattern matching if `Config::write_default` exposes a typed error variant — Tầng 2 self-decide based on actual `src/config.rs` API.
- `tests/cli_init.rs` must continue to pass unmodified — exit codes preserved.

### Task 4 — Extract `core::register` and `core::unregister` — V2 [O1.3 ACCEPT]

**File:** `src/core/register.rs` (NEW)

Extract body of `src/cli/register.rs::run_with_deps<L: LaunchctlClient>` (P003 ship, real 4-arg `(args, launchctl, launch_agents_dir, home)`) into `core::register::run<L: LaunchctlClient>(args: RegisterArgs, client: &L) -> Result<RegisterOutput>`.

**V2 KEY CHANGE (per [O1.3] ACCEPT):** `core::register::run` resolves `home`, `launch_agents_dir`, `self_exe` ALL INTERNALLY. Only `&client: &L` injected. `RegisterArgs` stays minimal (user-facing fields only).

```rust
use crate::config::Config;
use crate::core::config_path::{default_config_path, home_dir};
use crate::launchd::{generate_plist, plist_path_for, default_launch_agents_dir, LaunchctlClient};
use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RegisterArgs {
    pub label: String,
    pub schedule: Option<String>,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegisterOutput {
    pub plist_path: PathBuf,
    pub label: String,
    pub bootstrapped: bool,
}

/// V2 (per Architect Turn 1 RESPOND [O1.3 ACCEPT]):
/// - Resolves `home` internally via `home_dir()`.
/// - Resolves `launch_agents_dir` internally via `default_launch_agents_dir(&home)`.
/// - Resolves `self_exe` internally via `std::env::current_exe()`.
/// - ONLY `&client: &L: LaunchctlClient` is injected (preserves testability via NoopLaunchctl).
/// - Args struct carries user-facing inputs only (label, schedule, config_path).
pub fn run<L: LaunchctlClient>(args: RegisterArgs, client: &L) -> Result<RegisterOutput> {
    // Worker pastes existing P003 run_with_deps body, adapting:
    //   - validate label (INV-12 allowlist) — keep existing check
    //   - resolve home INTERNALLY: let home = home_dir()?;
    //   - resolve launch_agents_dir INTERNALLY: let launch_agents_dir = default_launch_agents_dir(&home);
    //   - resolve self_exe INTERNALLY: let self_exe = std::env::current_exe().context("resolve current exe")?;
    //   - resolve config path: let cfg_path = args.config_path.map(Ok).unwrap_or_else(default_config_path)?;
    //   - load config, resolve schedule (args.schedule overrides config.schedule)
    //   - generate plist XML via generate_plist(&label, &schedule, &self_exe, &cfg_path, ...) — Worker matches real generate_plist signature
    //   - compose plist_path via plist_path_for(&args.label, &launch_agents_dir)
    //   - fs::create_dir_all(&launch_agents_dir)?;
    //   - fs::write(&plist_path, xml)?;
    //   - client.bootstrap(&plist_path)?;
    //   - Ok(RegisterOutput { plist_path, label: args.label, bootstrapped: true })
    todo!("Worker: paste P003 run_with_deps body, adapt to internal env resolution per V2 [O1.3 ACCEPT]")
}

#[cfg(test)]
mod tests {
    // Move per-logic tests from src/cli/register.rs if any.
    // CLI exit code tests stay in tests/cli_register.rs.
}
```

**File:** `src/core/unregister.rs` (NEW)

Mirror pattern for `unregister`:

```rust
use crate::core::config_path::home_dir;
use crate::launchd::{plist_path_for, default_launch_agents_dir, LaunchctlClient};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct UnregisterArgs {
    pub label: String,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnregisterOutput {
    pub label: String,
    pub plist_existed: bool,
    pub was_loaded: bool,
}

/// V2 (per Architect Turn 1 RESPOND [O1.3 ACCEPT]):
/// - Resolves `home` + `launch_agents_dir` internally.
/// - ONLY `&client: &L: LaunchctlClient` injected.
pub fn run<L: LaunchctlClient>(args: UnregisterArgs, client: &L) -> Result<UnregisterOutput> {
    // Worker pastes existing P003 unregister::run_with_deps body, adapting:
    //   - validate label (INV-12)
    //   - resolve home INTERNALLY: let home = home_dir()?;
    //   - resolve launch_agents_dir INTERNALLY: let launch_agents_dir = default_launch_agents_dir(&home);
    //   - compose plist_path via plist_path_for(&args.label, &launch_agents_dir)
    //   - was_loaded = check via client.print() existence
    //   - plist_existed = plist_path.exists()
    //   - client.bootout(&args.label) — idempotent (NotLoaded = Ok)
    //   - if plist_existed → fs::remove_file(&plist_path)?;
    //   - Ok(UnregisterOutput { label, plist_existed, was_loaded })
    todo!("Worker: paste P003 unregister::run_with_deps body, adapt to internal env resolution per V2 [O1.3 ACCEPT]")
}

#[cfg(test)]
mod tests { }
```

**Thin CLI shells:** `src/cli/register.rs` and `src/cli/unregister.rs` reduced to ~15 lines each:

```rust
// src/cli/register.rs (V2 sketch)
use crate::core::register::{RegisterArgs, run as core_run};
use crate::launchd::RealLaunchctl;
use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[arg(long)] pub label: String,
    #[arg(long)] pub schedule: Option<String>,
    #[arg(long)] pub config: Option<PathBuf>,
}

/// V2: returns Result<u8> matching dispatch contract.
pub async fn run(args: Args) -> Result<u8> {
    let client = RealLaunchctl;
    match core_run(RegisterArgs {
        label: args.label,
        schedule: args.schedule,
        config_path: args.config,
    }, &client) {
        Ok(output) => {
            println!("registered {} → {}", output.label, output.plist_path.display());
            Ok(0)
        }
        Err(e) => {
            eprintln!("{e:#}");
            // Worker: map exit codes per ARCHITECTURE.md:71-77 ($HOME=1, config/cron=2, plist/bootstrap=3)
            // by pattern-matching error chain (existing P003 mapping logic).
            Ok(/* mapped exit code */ 1)
        }
    }
}
```

(Analogous for unregister shell.)

**Lưu ý:**
- **V2 PER [O1.3] ACCEPT:** Args structs unchanged from user-facing. Env resolution moves INTO `core::*::run`. `&L: LaunchctlClient` only injected dep.
- Label sanitization 2-point enforcement (INV-12) preserved: pre-flight in `src/cli/<sub>.rs::run` shell + inside `generate_plist` (existing). MCP tool boundary becomes a THIRD enforcement point per INV-18 (Task 8).
- `register` exit code map per ARCHITECTURE.md:71-77 `[verified]`: 0=success, 1=$HOME unset, 2=config/cron-parse fail, 3=plist write / bootstrap fail. Map happens in CLI shell, NOT in `core::register::run` (core returns typed errors via `anyhow::Error` context; shell pattern-matches).
- `--config <path>` flag remains on CLI Args. `RegisterArgs.config_path: Option<PathBuf>` carries through.
- `--schedule` remains optional `String` (P003 V2 relaxation `[verified]`).
- Worker fills the exact `generate_plist(...)` call args per real `src/launchd.rs` signature — phiếu cannot prescribe exact arg list without reading `src/launchd.rs`. `[needs Worker verify generate_plist signature]`.

### Task 5 — Extract `core::run` (the task runner) and `core::status` — V2 [analogous internal-resolution pattern]

**File:** `src/core/run.rs` (NEW)

```rust
use crate::config::Config;
use crate::core::config_path::default_config_path;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::runner::{self, RunResult};
use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RunArgs {
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunOutput {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub heartbeat_appended: bool,
}

/// V2 (per Architect Turn 1 RESPOND, analogous internal-resolution pattern):
/// - Resolves config path via `default_config_path()` if Args.config_path is None
///   (which internally bails on $HOME unset — no separate home arg needed here).
/// - No LaunchctlClient injection needed (run touches filesystem + child process only).
pub async fn run(args: RunArgs) -> Result<RunOutput> {
    // Worker pastes existing P004 src/cli/run.rs::run body, adapting:
    //   - resolve config path: let cfg_path = args.config_path.map(Ok).unwrap_or_else(default_config_path)?;
    //   - load config
    //   - call runner::fire_task(&config).await? → RunResult
    //   - build HeartbeatRecord from RunResult + Utc::now()
    //   - heartbeat::append(&config.heartbeat.log_path, &record) — track success/fail for heartbeat_appended
    //   - return RunOutput { exit_code, duration_ms, stdout_tail, stderr_tail, heartbeat_appended }
    todo!("Worker: paste P004 cli/run.rs body, adapt return + use default_config_path()")
}
```

**File:** `src/core/status.rs` (NEW)

Move existing `StatusReport`, `parse_next_fire`, `is_valid_label`, and the `pub async fn run` body from `src/cli/status.rs` into `src/core/status.rs`. Make `StatusReport` `pub`. Take `&L: LaunchctlClient` as second arg.

```rust
use crate::config::Config;
use crate::core::config_path::default_config_path;
use crate::heartbeat::{self, HeartbeatRecord};
use crate::launchd::{LaunchctlClient, LaunchctlPrintOutput};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct StatusArgs {
    pub label: Option<String>,
    pub config_path: Option<PathBuf>,
    pub last: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusReport {
    pub label: String,
    pub plist_loaded: bool,
    pub next_fire: Option<String>,
    pub heartbeat_log_path: String,
    pub last_runs: Vec<HeartbeatRecord>,
}

/// V2 (per Architect Turn 1 RESPOND, analogous internal-resolution pattern):
/// - Resolves config path internally via `default_config_path()` if Args.config_path is None.
/// - `&client: &L: LaunchctlClient` injected (needed for launchctl print).
pub fn run<L: LaunchctlClient>(args: StatusArgs, client: &L) -> Result<StatusReport> {
    // Worker pastes existing P005 src/cli/status.rs::run body, returns StatusReport directly.
    todo!("Worker: paste P005 cli/status.rs body, return StatusReport directly")
}

pub(crate) fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub(crate) fn parse_next_fire(stdout: &str) -> Option<String> {
    // [Worker pastes P005 parse_next_fire body verbatim — descriptor Hour/Minute parser]
    todo!("Worker: paste P005 parse_next_fire body verbatim")
}

#[cfg(test)]
mod tests {
    // MOVE the 11 P005 unit tests (is_valid_label, parse_next_fire variants, tail_first_n_or_empty) here.
    // tests/cli_status.rs integration tests stay where they are.
}
```

**Thin CLI shells** `src/cli/run.rs` + `src/cli/status.rs` reduced to:
- Parse Args
- Build core args
- (status) instantiate `RealLaunchctl`
- Call `core::*::run`
- (status only) Format StatusReport as human-text or JSON per `--json` flag
- (run only) format human-readable run summary
- Map exit codes per ARCHITECTURE.md:71-77
- **V2: each shell returns `Result<u8>`** (matching dispatch contract).

**Lưu ý:**
- `tail_first_n_or_empty` is a UI/rendering helper used only by CLI human-text mode — STAYS in `src/cli/status.rs` (don't move to core). Worker self-decide: if also used by MCP, move; if only CLI human render, keep CLI-side.
- Exit codes map identically: status always exits 0 unless config load fails (exit 2) or invalid label (exit 1). Per P005 Constraint #6 `[verified]`.

### Task 6 — Add `src/cli/mcp.rs` thin shell + `src/cli/mod.rs` enum extension — V2 [O1.1 ACCEPT]

**File:** `src/cli/mcp.rs` (NEW)

**V2 KEY CHANGE (per [O1.1] ACCEPT):** `pub async fn run(...) -> Result<u8>` (not `Result<()>`); transport errors → `return Ok(5)` (not `process::exit(5)`).

```rust
use anyhow::Result;
use clap::Args as ClapArgs;

#[derive(Debug, ClapArgs)]
pub struct Args {
    // intentionally empty — stdio MCP has no flags in Phase 1
}

/// V2 (per Architect Turn 1 RESPOND [O1.1 ACCEPT]):
/// - Returns `Result<u8>` matching `dispatch()` contract at `src/cli/mod.rs:28`
///   (all 5 existing CLI handlers return Result<u8>).
/// - Transport errors → `return Ok(5)` (NOT `process::exit(5)`).
///   Exit code 5 reserved per ARCHITECTURE.md:77 for "MCP transport error".
pub async fn run(_args: Args) -> Result<u8> {
    match crate::mcp::server::serve_stdio().await {
        Ok(()) => Ok(0),
        Err(e) => {
            // Map transport errors to exit code 5 per ARCHITECTURE.md:77.
            // Use Ok(5) not Err(e) — Err would bubble through main and produce a
            // generic exit code; we want explicit MCP-transport exit 5.
            eprintln!("MCP transport error: {e:#}");
            Ok(5)
        }
    }
}
```

**File:** `src/cli/mod.rs` (EDIT — Constraint #1 RETIRED for P006 only)

**Tìm:** existing `Commands` enum body + `dispatch` fn match arms + module declarations at top.

**Thay bằng / Thêm:** (Worker reads exact line numbers in Task 0; applies minimal diff:)

```rust
// 1) Top of file — add module declaration alongside existing per-subcommand mods:
pub mod mcp;

// 2) In Commands enum — add 6th variant:
#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    Init(init::Args),
    Register(register::Args),
    Unregister(unregister::Args),
    Run(run::Args),
    Status(status::Args),
    Mcp(mcp::Args),  // NEW — P006
}

// 3) In dispatch fn — add 6th match arm.
//    V2: dispatch fn returns `anyhow::Result<u8>` (per Worker Turn 1 verification of real
//    src/cli/mod.rs:28 signature — NOT Result<()> as V1 phiếu wrongly assumed).
pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8> {
    match cmd {
        Commands::Init(args) => init::run(args).await,
        Commands::Register(args) => register::run(args).await,
        Commands::Unregister(args) => unregister::run(args).await,
        Commands::Run(args) => run::run(args).await,
        Commands::Status(args) => status::run(args).await,
        Commands::Mcp(args) => mcp::run(args).await,  // NEW — P006, also returns Result<u8>
    }
}
```

**Lưu ý:**
- **V2 PER [O1.1] ACCEPT:** dispatch fn ALREADY returns `Result<u8>` in real code at `src/cli/mod.rs:28` — Worker only adds the 6th match arm, does NOT change the dispatch signature. The new `mcp::run` MUST return `Result<u8>` to satisfy the match expression's type.
- **EXACT diff budget: +3 lines to existing `mod.rs` (mod decl, enum variant, match arm). Anything more = scope creep. Worker `git diff src/cli/mod.rs | wc -l` MUST report ≤ ~10 (with diff headers).**
- This is the FIRST `src/cli/mod.rs` edit since P003. Constraint #1 explicitly retired for this phiếu.
- Constraint #1 REINSTATED for P007+ (see Luật chơi Constraint #2 below): "don't touch `src/cli/mod.rs` UNLESS adding a new subcommand variant."

### Task 7 — Create `src/mcp/{mod,server,tools}.rs` + `src/main.rs` mod decl — V2 unchanged

**File:** `src/main.rs` (EDIT)

**Tìm:** existing `mod cli;` line at `src/main.rs:6` (per Worker Turn 1 Anchor #4 verified).

**Thay bằng / Thêm:** Add `mod core;` and `mod mcp;` adjacent. Tokio flavor stays `current_thread` (Worker Turn 1 Anchor #2 confirmed rmcp compatible).

**File:** `src/mcp/mod.rs` (NEW)

```rust
//! MCP server (stdio JSON-RPC 2.0) — exposes 5 tools mirroring CLI subcommands.
//! Thin adapter over crate::core::* — see ARCHITECTURE.md §MCP surface.

pub mod server;
pub mod tools;
```

**File:** `src/mcp/server.rs` (NEW)

Bootstrap rmcp server with stdio transport, register 5 tools via `tools::register_tools`, await stdin EOF.

Per Worker Turn 1 rmcp API verification: `MyHandler.serve(stdio()).await?` + `server.waiting().await?`. Worker writes the handler struct implementing rmcp's `ServerHandler` trait (manual impl per Decision 1 V2 — no macros).

Skeleton:

```rust
use anyhow::Result;
use rmcp::transport::io::stdio;
use rmcp::ServiceExt;

pub async fn serve_stdio() -> Result<()> {
    let handler = crate::mcp::tools::AdvisoryCronHandler::default();
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

(Worker adjusts exact import paths + handler API per actual rmcp 1.7.0 — Worker Turn 1 verified `ServerHandler` trait + `stdio()` fn + `serve` ext method exist.)

**File:** `src/mcp/tools.rs` (NEW)

Define `AdvisoryCronHandler` struct implementing rmcp's `ServerHandler` trait. The trait has default impls for `list_tools`, `call_tool`, `get_info` (per Worker Turn 1 confirmation) — override these to provide our 5 tools.

Each tool's handler:
1. Deserialize input JSON to `core::*Args`
2. Validate INV-12 + INV-18 (label allowlist) BEFORE invoking core
3. Instantiate `RealLaunchctl` if needed (register/unregister/status)
4. Await `core::*::run(args, &client)` (or sync for non-async cores)
5. Serialize `*Output` / `StatusReport` via `serde_json::to_string`
6. Return `CallToolResult::success(serialized_string)` — `String` impl `IntoContents` per Worker Turn 1 verification.

Skeleton:

```rust
use anyhow::{bail, Result};
use crate::core;
use crate::launchd::RealLaunchctl;
// rmcp imports per Worker verified API surface

#[derive(Default)]
pub struct AdvisoryCronHandler;

fn validate_label(label: &str) -> Result<()> {
    if label.is_empty() || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("invalid label: must be ASCII alphanumeric + - + _");
    }
    Ok(())
}

// impl rmcp::ServerHandler for AdvisoryCronHandler {
//     async fn list_tools(...) -> ListToolsResult {
//         // Return 5 Tool structs with hand-written input_schema via serde_json::json!
//     }
//     async fn call_tool(&self, req: CallToolRequest, ...) -> CallToolResult {
//         match req.name.as_ref() {
//             "init" => { /* deserialize InitArgs, call core::init::run, serialize InitOutput */ }
//             "register" => { /* validate_label, deserialize, RealLaunchctl, core::register::run */ }
//             "unregister" => { ... }
//             "run" => { /* call core::run::run().await */ }
//             "status" => { /* if label present → validate_label, core::status::run */ }
//             _ => CallToolResult::error("unknown tool")
//         }
//     }
// }
```

**Lưu ý:**
- INV-18 label sanitization at MCP boundary is MANDATORY — do not delegate solely to core::*::run's internal validation. Defense-in-depth pattern matches INV-12 2-point enforcement.
- Tool result format: `CallToolResult::success(...)` with serialized JSON string (Worker Turn 1 verified `String: IntoContents`).
- Worker exercises full discretion mapping rmcp's exact `ServerHandler` trait shape (method signatures may differ slightly — Worker reads rmcp docs during EXECUTE).
- DO NOT use rmcp macros (`#[tool]`, `#[tool_router]`) — Decision 1 V2 + Task 1 V2 drop `macros` feature.

### Task 8 — Append INV-18 to `docs/security/INVARIANTS.md` — V2 unchanged

**File:** `docs/security/INVARIANTS.md`

**Tìm:** end of file (after INV-17 block).

**Thay bằng / Thêm:**

```markdown
---

### INV-18 — MCP transport boundary: stdio JSON-RPC, label sanitization at tool entry

**Statement:** PR introducing MCP tool handlers (`src/mcp/tools.rs`) MUST:
1. Validate every `label` field at the MCP tool boundary BEFORE invoking `core::*::run` — ASCII alphanumeric + `-` + `_` allowlist (mirrors INV-12 pre-flight). This is a THIRD enforcement point in addition to INV-12's two points (CLI pre-flight + `generate_plist` defense-in-depth). MCP clients are external — never trust input.
2. Validate every `config_path` field is either absent (None → defaults) or an absolute path. Path components MUST NOT contain `..` traversal sequences. (`PathBuf::components().any(|c| matches!(c, std::path::Component::ParentDir))` → reject.)
3. Tool result serialization via `serde_json::to_string` (or `to_value`) on `#[derive(Serialize)]` structs — NEVER hand-roll JSON (INV-16 generalization).
4. Tool handler errors propagate as MCP error objects (NOT process exit) — only transport-level errors (stdin EOF, malformed JSON-RPC frame) escape to `serve_stdio` and map to exit code 5 per ARCHITECTURE.md:77 (via `return Ok(5)` in `src/cli/mcp.rs::run` per V2 P006 [O1.1]).

**Why:** MCP boundary IS the new attack surface in Phase 1.7. CLI was trusted (user owns their own keyboard). MCP tools are callable by any process Claude Desktop / Code talks to — the MCP server has no way to verify the upstream client's intentions. Defense at the boundary is non-negotiable.

**Implementation (Phase 1.7):** `src/mcp/tools.rs` — `validate_label` helper called at start of every tool handler that takes a `label`. `validate_config_path` for path inputs. `core::*::run` still validates internally (defense-in-depth). `serde_json::to_string(output)?` for all tool results.

**Trigger keywords:** new MCP tool handler additions, `rmcp::ServerHandler` impls, MCP server boot in any new transport beyond stdio.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE; Giám sát soi PR diff for MCP-related additions.
```

**Lưu ý:** INV-18 EXTENDS rather than replaces INV-12. Both stay active. CHANGELOG entry MUST cite INV-18.

### Task 9 — Update `docs/ARCHITECTURE.md` (Tầng 1 docs gate) — V2 unchanged

(All V1 edits remain — §Modules table, §CLI surface mcp row Phase ✅, §MCP surface schemas populated from Decision 3, §Phase status Phase 1.7 ✅.)

**Additional V2 doc updates:**
- §Modules table: note that `core::*::run` fns "resolve env deps internally; only `&LaunchctlClient` injected" (V2 design principle).
- §MCP surface section: confirm `src/cli/mcp.rs::run -> Result<u8>` returning 5 on transport error (V2 [O1.1]).

### Task 10 — Create `tests/cli_mcp.rs` (handshake + parity + per-tool) — V2 unchanged

(Test cases 1-7 unchanged from V1. Parity test uses NoopLaunchctl shared between CLI path + MCP in-process path. Each `core::*::run` call passes the same NoopLaunchctl reference; `client.bootstrap_calls` Vec compared for equality.)

**V2 clarification for parity test:**
Since `core::register::run` now resolves `home` / `launch_agents_dir` / `self_exe` INTERNALLY (per V2 [O1.3]), the parity test relies on:
- Both CLI path and MCP path running in the same test process → same `$HOME` → same launch_agents_dir → same plist_path computed → same bootstrap call args.
- Test MAY set `$HOME` to a tempdir at start to make assertions deterministic. Worker self-decide via standard test isolation patterns.

### Task 11 — Add Claude Desktop config snippet to `README.md` — V2 unchanged

(Content from V1 unchanged.)

### Task 12 — CHANGELOG entry + DISCOVERIES.md index + discoveries/P006.md — V2 unchanged

**Files:**
- `docs/CHANGELOG.md` — prepend new entry under "## 2026-MM-DD — P006: Phase 1.7 — MCP server wrapper"
- `docs/DISCOVERIES.md` — prepend 1-line entry: `- 2026-MM-DD P006: MCP server wrapper shipped (rmcp 1.7.0 dep, core extraction completes layering invariant, 5 tools parity-tested, INV-18 transport boundary, binary <7MB, ~77 tests) → see docs/discoveries/P006.md`
- `docs/discoveries/P006.md` (NEW) — full Discovery Report per RULES.md:64 format including:
  - Assumptions ĐÚNG / SAI / Edge cases / Docs updated / Layer 2 sub-mech results
  - **Special: record exact rmcp version + features chosen + binary size delta + parity-test evidence**
  - Note resolution of Decisions 1-6 (which option was actually viable post-probe vs Architect recommendation)
  - **V2 documentation: record the 3 mechanical objections raised in CHALLENGE Turn 1 (O1.1 `Result<u8>`, O1.2 `Config::write_default` 3-arg, O1.3 `run_with_deps` 4-arg) + how V2 phiếu accepted all 3 + the V2 design principle "core::*::run resolves env internally"**

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `Cargo.toml` | Task 1 V2: add `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`; ADD `io-std` to tokio features |
| `src/main.rs` | Task 7: add `mod core; mod mcp;` adjacent to `mod cli;`. Tokio flavor stays `current_thread`. |
| `src/cli/mod.rs` | Task 6 V2: +3 lines (Mcp variant + dispatch arm + mod decl). Dispatch fn signature `Result<u8>` UNCHANGED. |
| `src/cli/init.rs` | Task 3 V2: rewrite as thin shell returning `Result<u8>` over `core::init::run` |
| `src/cli/register.rs` | Task 4 V2: rewrite as thin shell returning `Result<u8>` over `core::register::run` |
| `src/cli/unregister.rs` | Task 4 V2: rewrite as thin shell returning `Result<u8>` over `core::unregister::run` |
| `src/cli/run.rs` | Task 5 V2: rewrite as thin shell returning `Result<u8>` over `core::run::run` |
| `src/cli/status.rs` | Task 5 V2: rewrite as thin shell returning `Result<u8>` over `core::status::run` |
| `src/cli/mcp.rs` | Task 6 V2: NEW — thin shell returning `Result<u8>` (transport err → `Ok(5)`) calling `mcp::server::serve_stdio()` |
| `src/core/mod.rs` | Task 3-5: NEW — module root re-exports |
| `src/core/config_path.rs` | Task 2 V2: NEW — `default_config_path()` + `home_dir()` helpers |
| `src/core/init.rs` | Task 3 V2: NEW — extracted init logic; resolves home internally; derives `written` from pre-call `path.exists()` |
| `src/core/register.rs` | Task 4 V2: NEW — extracted register logic; resolves home + launch_agents_dir + self_exe INTERNALLY; only `&L: LaunchctlClient` injected |
| `src/core/unregister.rs` | Task 4 V2: NEW — extracted unregister logic; same internal resolution pattern |
| `src/core/run.rs` | Task 5 V2: NEW — extracted task runner logic; resolves config path internally |
| `src/core/status.rs` | Task 5 V2: NEW — extracted status logic + public StatusReport + parse_next_fire + is_valid_label; resolves config path internally |
| `src/mcp/mod.rs` | Task 7: NEW — module root |
| `src/mcp/server.rs` | Task 7: NEW — `serve_stdio` bootstrap rmcp + stdio transport |
| `src/mcp/tools.rs` | Task 7: NEW — `AdvisoryCronHandler` impl rmcp `ServerHandler` + 5 tool dispatch + JSON schemas + INV-18 validation |
| `tests/cli_mcp.rs` | Task 10: NEW — 7 tests (handshake + parity + per-tool + INV-18) |
| `docs/security/INVARIANTS.md` | Task 8: append INV-18 |
| `docs/ARCHITECTURE.md` | Task 9 V2: §Modules + §MCP surface + §Phase status + V2 internal-resolution note |
| `docs/CHANGELOG.md` | Task 12: P006 entry citing rmcp 1.7.0 + tokio io-std + INV-18 + V2 debate resolution |
| `docs/DISCOVERIES.md` | Task 12: 1-line index |
| `docs/discoveries/P006.md` | Task 12: NEW — full Discovery Report incl V2 objection trail |
| `README.md` | Task 11: NEW "MCP server" section |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/launchd.rs` | LaunchctlClient trait + impls unchanged; `core::*` reuses as-is. `git diff src/launchd.rs` empty post-EXECUTE. |
| `src/runner.rs` | `fire_task` unchanged; `core::run` calls it directly. `git diff src/runner.rs` empty. |
| `src/heartbeat.rs` | `append` + `read_last_n` + `HeartbeatRecord` unchanged. `git diff src/heartbeat.rs` empty. |
| `src/config.rs` | Schema unchanged. `Config::write_default(path, home, force) -> Result<()>` signature reused as-is. `git diff src/config.rs` empty. |
| `tests/cli_help.rs` | Picks up `mcp` subcommand automatically via clap derive. Worker MAY add +1 line if substring asserts on subcommand count (Tầng 2 self-decide). |
| `tests/cli_init.rs`, `tests/cli_register.rs`, `tests/cli_run.rs`, `tests/cli_status.rs` | All regression — MUST pass UNMODIFIED post-refactor. CLI behavior preserved. |
| `.phieu-counter` | Already at `006`. No edit. |

---

## Luật chơi (Constraints)

1. **Tầng 1 docs gate mandatory.** Per RULES.md:13/16/21/22 — CLI subcommand add + Cargo.toml dep add + module add + security boundary touch ALL fire. ARCHITECTURE.md + CHANGELOG + INVARIANTS.md MUST be updated in same commit as code. `docs-gate --all --verbose` MUST pass before commit.

2. **Newtype dispatch preservation (REINSTATED for P007+).** For P006 only, `src/cli/mod.rs` MAY gain exactly +3 lines (Mcp variant + dispatch arm + mod decl). The dispatch fn signature stays `anyhow::Result<u8>` (per V2 — real code already this; do NOT change). For P007 and beyond: `git diff src/cli/mod.rs` MUST be empty UNLESS adding a NEW subcommand variant.

3. **Layering invariant enforcement.** Every CLI handler in `src/cli/<sub>.rs::run` MUST contain exactly one `core::<sub>::run` call as its primary logic. Every MCP tool handler in `src/mcp/tools.rs` MUST contain exactly one `core::<sub>::run` call. `grep -c "core::<sub>::run" src/` MUST return ≥ 2 for each of init/register/unregister/run/status. Verified via Sub-mech D.

4. **V2 design principle — `core::*::run` resolves env deps internally.** Every `core::*::run` fn resolves its own `home` / `launch_agents_dir` / `self_exe` / config path internally (via `core::config_path::{default_config_path, home_dir}` + `std::env::current_exe()`). The ONLY injected dependency is `&L: LaunchctlClient` trait for fns needing launchctl. Args structs carry user-facing inputs only. (Per Architect Turn 1 RESPOND [O1.3] ACCEPT — applies to all 5 core fns for consistency.)

5. **Behavioral parity test mandatory.** `tests/cli_mcp.rs::parity_register_cli_eq_mcp` MUST exist and pass — proves the layering invariant is real-runtime, not just file-organization decorative.

6. **Binary size budget hard cap < 7MB.** PROJECT.md acceptance bullet 9 `[verified]`. Worker Turn 1 projected 1.6-2.2MB (well under). If `ls -lh target/release/advisory-cron` reports > 7MB → STOP and escalate CHALLENGE Turn 2.

7. **No `unsafe { }` blocks beyond the test-only env-var mutation** in `src/core/config_path.rs` tests (which Rust 2024 mandates `unsafe` wrapping for `set_var`/`remove_var`). Per INV-6 — comment block rationale required. Production code stays `unsafe`-free.

8. **INV-18 label validation 3-point enforcement.** Label-bearing MCP tools (`register`, `unregister`, `status`) MUST validate the allowlist (a) at MCP tool handler entry BEFORE deserialize-to-core-args (INV-18 new), (b) in `core::*::run` for shared core enforcement (INV-12 preserved), (c) inside `generate_plist` for register (INV-12 preserved). 3 points total, defense-in-depth.

9. **Exit codes preserved byte-for-byte for CLI path.** Refactoring CLI handlers to thin shells MUST NOT alter exit code mapping. Each CLI shell returns `Result<u8>` with exit code derived per ARCHITECTURE.md:71-77. Regression suite (`tests/cli_*.rs` P002-P005) is the safety net — if any prior test fails post-refactor, STOP and audit.

10. **MCP exit code 5 via `Ok(5)`, NOT `process::exit(5)`.** Per V2 [O1.1] ACCEPT — `src/cli/mcp.rs::run` returns `Ok(5)` on transport error. `process::exit` would bypass the dispatch chain's exit-code return path.

11. **No new subcommand beyond `mcp`.** P006 adds exactly one subcommand. No "while we're at it" additions (Hard Stop #1).

12. **No `Cargo.toml` dep beyond `rmcp` (and tokio `io-std` feature add).** schemars comes transitively via rmcp `server` feature — that's transitive, OK. If Architect needs to add `schemars` as DIRECT dep → CHALLENGE Turn 2 to escalate Decision 3 fallback.

13. **`current_thread` tokio runtime preserved** — Worker Turn 1 confirmed rmcp compatible. No `rt-multi-thread` feature add. Switching would require a Cargo.toml change requiring CHANGELOG entry per INV-5.

14. **Worker autonomy on rmcp API surface details.** Architect cannot prescribe `rmcp::ServerHandler` exact method signatures without re-Reading rmcp docs. Worker has full discretion within constraints of Tasks 6-7-10 to map rmcp's actual API to the responsibilities documented. If rmcp API surface dramatically differs from typical MCP server patterns → CHALLENGE Turn 2.

15. **Phase 1.6 (next phiếu) territory KHÔNG TOUCHED.** P006 ships MINIMUM README MCP section + ARCHITECTURE MCP schema population. Full README polish, ARCHITECTURE quick-start verification, dual-path dogfood scripts — that's Phase 1.6.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings, binary < 7MB
- [ ] `cargo test --all` — ≥77 tests pass (70 baseline + 7 new from `tests/cli_mcp.rs` + any moved unit tests)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `git diff src/cli/mod.rs | wc -l` — ≤ 12 lines incl headers (i.e. ≤ 3-5 logical lines added; dispatch fn signature UNCHANGED)
- [ ] `git diff src/launchd.rs src/runner.rs src/heartbeat.rs src/config.rs` — empty (KHÔNG sửa list)
- [ ] `grep "Result<u8>" src/cli/mcp.rs` — present (V2 [O1.1] enforcement)
- [ ] `grep -E "fn run\b" src/core/register.rs | grep -v "client: &L"` — empty (V2 [O1.3] enforcement: signature has `&L: LaunchctlClient` injected)
- [ ] `grep "path.exists()" src/core/init.rs` — present (V2 [O1.2] enforcement: pre-call check)

### Manual Testing
- [ ] `advisory-cron mcp` runs and accepts JSON-RPC `initialize` via stdin → response includes `serverInfo` + 5 tools listed
- [ ] `advisory-cron mcp` with `tools/list` JSON-RPC → returns 5 named tools (init, register, unregister, run, status)
- [ ] `advisory-cron mcp` with `tools/call` JSON-RPC `init` (force=false, no config_path) → InitOutput JSON with `written: true` or Err response (depending on whether `~/.config/advisory-cron/config.toml` exists)
- [ ] Register `advisory-cron` MCP server in Claude Desktop config + restart Claude Desktop → Claude can list 5 tools from chat ("list tools from advisory-cron")
- [ ] Sếp dogfood: ask Claude to `register --label sếp-test-p006 --schedule "0 12 * * *"` via MCP → verify with `launchctl list | grep com.advisorycron.sếp-test-p006` shows row → `advisory-cron unregister --label sếp-test-p006` cleans up

### Regression
- [ ] `advisory-cron init` (CLI path) writes config identical to pre-P006 behavior (exit codes preserved — exit 0 on write, exit 2 on "already exists")
- [ ] `advisory-cron register --label test-p006-cli --schedule "0 10 * * *"` works identically (compare plist content to pre-P006 register output if archived)
- [ ] `advisory-cron unregister --label test-p006-cli` idempotent (exit 0 even if not loaded)
- [ ] `advisory-cron run` fires task + writes heartbeat (exit codes preserved per ARCHITECTURE.md:71-77)
- [ ] `advisory-cron status --json --last 5` returns same JSON shape as P005 ship (StatusReport field set unchanged)
- [ ] All 5 existing integration test files (`cli_init.rs`, `cli_register.rs`, `cli_run.rs`, `cli_status.rs`, `cli_help.rs`) pass UNMODIFIED (or `cli_help.rs` +1 line for subcommand count if applicable — Tầng 2 acceptable)

### Docs Gate
- [ ] `docs/ARCHITECTURE.md` — §Modules updated (6+ new rows + 5 thin-shell notes + V2 internal-resolution note), §CLI surface `mcp` row Phase ✅, §MCP surface schemas populated from Decision 3, §Phase status Phase 1.7 ✅
- [ ] `docs/CHANGELOG.md` — P006 entry citing: rmcp 1.7.0 dep + features (`server` + `transport-io`, no `macros`), tokio `io-std` feature add, current_thread preserved, 6 new core modules, 3 new mcp modules, 1 new cli module, INV-18 added, total test count, binary size delta, V2 debate resolution (3 mechanical objections accepted)
- [ ] `docs/security/INVARIANTS.md` — INV-18 appended (incl V2 mention of `Ok(5)` exit pattern)
- [ ] `README.md` — MCP server section added with verified Claude Desktop config snippet
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P006.md` — full report written per RULES.md:64 format
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top)
- [ ] Sub-mechanism A-E Verification Trace filled (table above)
- [ ] **Critical capture for P006:** exact rmcp version + feature flags chosen + actual binary size + parity test evidence (NoopLaunchctl call vec diff or equality assert output) + any Decision-1-to-6 deviations from Architect recommendation (with rationale)
- [ ] **V2 capture:** record the 3 mechanical objections from CHALLENGE Turn 1 (O1.1 `Result<u8>`, O1.2 `Config::write_default` 3-arg, O1.3 `run_with_deps` 4-arg) and the V2 design principle "core::*::run resolves env deps internally; only `&LaunchctlClient` injected". Note this debate prevented a compile-failure-on-EXECUTE and locked in a cleaner core API.
