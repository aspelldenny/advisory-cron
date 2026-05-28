# ARCHITECTURE — advisory-cron

> Living document. Architect updates when phiếu touches Tầng 1 (module / CLI / config schema / cron mechanism / error handling). Worker updates inline status when Phase ships.

---

## Overview

`advisory-cron` is a stateless CLI binary. It generates and manages **launchd plists** (Phase 1 — macOS) that fire user-configured tasks on a schedule. It does NOT run as a daemon — launchd owns scheduling, advisory-cron owns plist lifecycle + on-demand execution + heartbeat logging.

```
┌─────────────────┐  register   ┌─────────────────┐  fires    ┌──────────────────┐
│ advisory-cron   │────────────▶│ launchd plist   │──────────▶│ advisory-cron run│
│ (CLI binary)    │             │ (~/Library/...) │           │ (one-shot)       │
└─────────────────┘             └─────────────────┘           └────────┬─────────┘
        ▲                                                              │
        │ status reads                                                 │ writes
        │                                                              ▼
        │                                                       ┌──────────────────┐
        └───────────────────────────────────────────────────────│ heartbeat.jsonl  │
                                                                 │ (append-only)    │
                                                                 └──────────────────┘
```

**Key insight:** advisory-cron itself never sleeps or loops. It either (a) writes/removes a plist, (b) fires the configured task once and exits, or (c) reads status from launchd + heartbeat file. All scheduling is delegated to the OS.

---

## Modules

Planned module layout for Phase 1. Worker may adjust per phiếu's spec (Architect designs canonical names).

| Module | Purpose | Phase ships |
|--------|---------|-------------|
| `src/main.rs` | clap-derive CLI entry point. Parses subcommand + dispatches to `cli::*` handlers. | 1.1 ✅ |
| `src/cli/mod.rs` | Commands enum + dispatch fn routing to per-subcommand handlers. | 1.1 ✅ |
| `src/cli/init.rs` | `advisory-cron init` — write default config to `~/.config/advisory-cron/config.toml`. Skeleton shipped 1.1; impl shipped 1.2. | 1.2 ✅ |
| `src/cli/register.rs` | `advisory-cron register` — generate plist + `launchctl bootstrap`. Skeleton shipped 1.1; impl shipped 1.3. | 1.3 ✅ |
| `src/cli/unregister.rs` | `advisory-cron unregister` — `launchctl bootout` + remove plist. Skeleton shipped 1.1; impl shipped 1.3. | 1.3 ✅ |
| `src/cli/run.rs` | `advisory-cron run` — fire task once, write heartbeat. Skeleton shipped 1.1; impl deferred to 1.4. | 1.1 skeleton ✅ → impl 1.4 |
| `src/cli/status.rs` | `advisory-cron status` — read launchd next-fire + last heartbeat. Skeleton shipped 1.1; impl shipped 1.5. | 1.5 ✅ |
| `src/cli/mcp.rs` | `advisory-cron mcp` — thin shell: calls `mcp::server::serve_stdio()`, returns `Ok(5)` on transport error (not `process::exit`). | 1.7 ✅ |
| `src/core/mod.rs` | Re-exports `config_path`, `init`, `register`, `unregister`, `run`, `status` sub-modules. Zero CLI/MCP coupling. | 1.7 ✅ |
| `src/core/config_path.rs` | `home_dir() -> Result<PathBuf>` and `default_config_path() -> Result<PathBuf>` — shared `$HOME` helpers used by all `core::*::run` fns. Bails if `$HOME` is unset or empty. | 1.7 ✅ |
| `src/core/init.rs` | `run(InitArgs) -> Result<InitOutput>` — write default config. Resolves home via `home_dir()` internally. | 1.7 ✅ |
| `src/core/register.rs` | `run(RegisterArgs, &S: Scheduler) -> Result<RegisterOutput>` — build `RegisterIntent` + delegate to `Scheduler::register`. Resolves home + `self_exe` internally; plist generation moved into `MacosScheduler`. Inline `parse_daily_cron` (domain logic, not scheduler logic). | 1.7 ✅ → 3.1 ✅ |
| `src/core/unregister.rs` | `run(UnregisterArgs, &S: Scheduler) -> Result<UnregisterOutput>` — delegate to `Scheduler::unregister`. Idempotent. `UnregisterOutput` keeps `plist_existed`+`was_loaded` populated from `was_registered` (JSON schema stable). | 1.7 ✅ → 3.1 ✅ |
| `src/core/run.rs` | `async run(RunArgs) -> Result<RunOutput>` — retry loop wraps `runner::fire_task` (Phase 2.2); 1 heartbeat per attempt; alert outside loop (1 max per invocation). Full runner logic extracted from `cli/run.rs`. | 1.7 ✅ + 2.2 retry ✅ |
| `src/core/status.rs` | `run(StatusArgs, &S: Scheduler) -> Result<StatusReport>` — scheduler query + heartbeat read. `parse_next_fire` parses macOS descriptor format via `SchedulerStatus.raw_descriptor`. `StatusReport.plist_loaded` field name preserved (JSON schema stability). | 1.7 ✅ → 3.1 ✅ |
| `src/mcp/mod.rs` | Re-exports `server` and `tools` sub-modules. | 1.7 ✅ |
| `src/mcp/server.rs` | `serve_stdio() -> Result<()>` — rmcp `ServerHandler::serve(stdio()).await` + `.waiting().await`. Converts SDK errors to `anyhow::Error`. | 1.7 ✅ |
| `src/mcp/tools.rs` | `AdvisoryCronHandler` implementing rmcp `ServerHandler`. 5 tools with hand-written JSON schemas (Decision 3). INV-18 validation (`validate_label`, `validate_config_path`) at MCP boundary before `core::*` call. Tool errors as `is_error=true` results. | 1.7 ✅ |
| `src/config.rs` | TOML config schema (serde-derive). Validation on load. | 1.2 ✅ |
| ~~`src/launchd.rs`~~ | **Deleted Phase 3.1 (P012).** Content moved to `src/scheduler/macos.rs`. | ~~1.3 ✅ → 1.5 ✅~~ DELETED |
| `src/scheduler/mod.rs` | `Scheduler` trait + `RegisterIntent`/`RegisterReport`/`UnregisterReport`/`SchedulerStatus` types. `PlatformScheduler` compile-time alias. `NoopScheduler` (test impl). Shared `is_valid_label` helper (P013) used by both `macos.rs` defense-in-depth AND `linux.rs` defense-in-depth (single source of truth for INV-12 + INV-22 allowlist). Phase 3.1 (P012). | 3.1 ✅ |
| `src/scheduler/macos.rs` | `MacosScheduler` implements `Scheduler` for macOS (launchd). Plist XML generation, `launchctl` shell-out, INV-10/11/12/13/17 enforcement all INSIDE this module. `RealLaunchctl` + `LaunchctlClient` private (file-internal). Gated `#[cfg(target_os = "macos")]`. Phase 3.1 (P012). | 3.1 ✅ |
| `src/scheduler/linux.rs` | Real impl: `CrontabScheduler` uses `crontab -l` (read, tolerate `no crontab for user` stderr) + `crontab -` (stdin pipe write) — **sync `std::process::Command`** (zero tokio runtime nesting, zero new feature flag). Tag-line idempotency `# advisory-cron: <label>`. INV-22 defense-in-depth via `super::is_valid_label`. Gated `#[cfg(target_os = "linux")]`. Phase 3.2 (P013). | 3.2 ✅ |
| `src/runner.rs` | `tokio::process::Command` task spawn + capture stdout/stderr/exit. `RunResult` struct. | 1.4 ✅ |
| `src/heartbeat.rs` | JSONL atomic append (temp+fsync+rename per INV-21) + read-last-N with partial-last-line tolerance. `HeartbeatRecord` struct (durable schema, unchanged since P004). `tail_utf8` helper. | 1.4 ✅ + 2.3 crash-safe ✅ |
| `src/alert.rs` | `TelegramAlert::send_with_base` outbound POST to Telegram Bot API. Best-effort (alert fail ≠ task fail). Env-free module — the API base test-seam env var is read at the call site in `core::run::run`, NOT here. INV-19 boundary (10s timeout double-guard: reqwest client + `tokio::time::timeout`). `format_failure_message` centralises message format. | 2.1 ✅ |

*(Phase 2.2 ships retry policy inline in `src/core/run.rs` — no new module per P009 Architect decision. Phase 2.3 adds crash-safe heartbeat write.)*

**Layering invariant (shipped Phase 1.7):** `core::*` knows nothing about CLI or MCP. `cli::*` and `mcp::*` are both thin adapters. A single code path = single behavior — `register` from CLI and `register` from MCP MUST produce identical plist + identical side effects.

**V2 internal-resolution pattern (P006):** Every `core::*::run` fn resolves its own env dependencies (`$HOME`, `LaunchAgents` dir, `current_exe`) internally via stdlib. ONLY `&S: Scheduler` is injected for testability (prod = `PlatformScheduler`, test = `NoopScheduler`). No config-path or home-dir threading through call stacks.

### Scheduler trait (Phase 3.1 — P012)

`src/scheduler/mod.rs` exposes the `Scheduler` trait with 3 methods: `register`, `unregister`, `status`. All carry **high-level intent** (no OS-specific concepts leak into the trait surface):

- `register(&self, intent: &RegisterIntent) -> Result<RegisterReport>` — plist-vs-crontab hidden inside impl.
- `unregister(&self, label: &str) -> Result<UnregisterReport>` — idempotent.
- `status(&self, label: &str) -> Result<SchedulerStatus>` — raw descriptor for `parse_next_fire` parsing.

**Compile-time dispatch** via `PlatformScheduler` type alias:
- macOS: `pub use macos::MacosScheduler as PlatformScheduler;`
- Linux: `pub use linux::CrontabScheduler as PlatformScheduler;`
- Other OS: `compile_error!("Phase 3 supports macOS + Linux only")`

Phase 3.2 (P013) fills `CrontabScheduler` with real crontab logic. P012 ships as a compilation stub.

---

## CLI surface

| Subcommand | Args | Behavior | Phase |
|------------|------|----------|-------|
| `init` | `--force` (overwrite) | Write default config | 1.2 |
| `register` | `--schedule <cron>` (optional — overrides config; `M H * * *` daily form only) `--label <name>` `--config <path>` (optional — overrides default config path) | Generate + load plist | 1.3 ✅ |
| `unregister` | `--label <name>` `--config <path>` (reserved, unused P003) | Remove + unload plist (idempotent) | 1.3 ✅ |
| `run` | `--config <path>` (optional — overrides default config path) | Fire configured task once, write heartbeat | 1.4 ✅ |
| `status` | `--label <name>` `--config <path>` (optional) `--json` (machine output) `--last <N>` (default 5) | Show next fire + last heartbeat | 1.5 ✅ |
| `mcp` | (no args — stdio only) | Start MCP server on stdin/stdout; serves 5 tools mirroring above | 1.7 ✅ |

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Generic error (parse, IO) |
| 2 | Config not found / invalid |
| 3 | launchd operation failed |
| 4 | Task fire failed (subcommand `run` only) |
| 5 | MCP transport error (subcommand `mcp` only — stdio closed, malformed JSON-RPC) |
| 130 | SIGINT (Ctrl+C) |

---

## Config schema (Phase 1.2)

Advisory-cron reads a single TOML config file. Default path: `~/.config/advisory-cron/config.toml`. The path is currently hardcoded (no repo-local discovery in Phase 1 — deferred per PROJECT.md hard line #3).

### Full schema

```toml
[task]
command = "claude"
args = ["-p", "/advisory-scan"]
working_dir = "/Users/<user>"
label = "advisory-scan-daily"  # optional — heartbeat label; defaults to "advisory-cron"

[schedule]
# Option A — cron expression (5-field: min hour dom mon dow):
cron = "0 9 * * *"
# Option B — launchd-friendly calendar (mutually exclusive with cron):
# hour = 9
# minute = 0

[heartbeat]
log_path = "/Users/<user>/.local/state/advisory-cron/heartbeat.jsonl"

# (Phase 2.1 — optional)
[alert.telegram]
chat_id = "1184530337"
# Choose ONE of:
bot_token_file = "~/.advisory-cron-secrets.env"  # path to KEY=VAL file with TG_BOT_TOKEN=...
# bot_token = "8678210414:AAGN..."  # inline (less secure — config file must be chmod 600)

# (Phase 2.2 — optional)
[retry]
max_attempts = 3        # 1 = no retry; ≥2 = retry up to (max_attempts - 1) times after initial failure
backoff_secs = 30       # seconds to sleep between attempts (capped at 3600 by validate)
```

### Field reference

| Block | Field | Type | Required | Description | Default (init) |
|-------|-------|------|----------|-------------|----------------|
| `[task]` | `command` | `string` | yes | Executable to run (PATH-resolved) | `"claude"` |
| `[task]` | `args` | `string[]` | yes | Args passed to `command` | `["-p", "/advisory-scan"]` |
| `[task]` | `working_dir` | `path` | yes | Working directory for command spawn | `$HOME` |
| `[task]` | `label` | `string (optional)` | no | Heartbeat label for this config — distinct from `register --label` plist label | `"advisory-cron"` |
| `[schedule]` | `cron` | `string` | one-of | Standard cron expression | — |
| `[schedule]` | `hour` | `u8 (0–23)` | one-of | Calendar hour for launchd `StartCalendarInterval` | `9` |
| `[schedule]` | `minute` | `u8 (0–59)` | one-of | Calendar minute | `0` |
| `[heartbeat]` | `log_path` | `path` | yes | Append-only JSONL heartbeat file | `~/.local/state/advisory-cron/heartbeat.jsonl` |
| `[alert.telegram]` | `chat_id` | `string` | yes (if block present) | Telegram chat ID (numeric string or `@channelname`) | — |
| `[alert.telegram]` | `bot_token` | `string (one-of)` | one-of bot_token/bot_token_file | Inline bot token (config file should be chmod 600) | — |
| `[alert.telegram]` | `bot_token_file` | `path (one-of)` | one-of bot_token/bot_token_file | Path to `KEY=VAL` file containing `TG_BOT_TOKEN=...` | — |
| `[retry]` | `max_attempts` | `u32` | yes (if block present) | Max fire attempts per `run` invocation (≥1) | — |
| `[retry]` | `backoff_secs` | `u64` | yes (if block present) | Seconds between attempts (0..=3600) | — |

### Schedule variants

`[schedule]` is a serde `#[serde(untagged)]` enum. Serde discriminates by field presence:
- If `cron` field present → `ScheduleConfig::Cron` (standard cron expression)
- If `hour` + `minute` fields present → `ScheduleConfig::Calendar` (launchd `StartCalendarInterval`)
- Both forms round-trip cleanly through `toml::to_string_pretty` / `toml::from_str`. Confirmed via unit tests in `src/config.rs`.

### Validation

Beyond serde structural check, `Config::validate()` enforces:
- `task.command` non-empty after trim
- `schedule.hour` ∈ 0..=23 (Calendar variant only)
- `schedule.minute` ∈ 0..=59 (Calendar variant only)

Validation errors → exit code 2 per §CLI surface exit codes.

### Source module

`src/config.rs` — `Config`, `TaskConfig`, `ScheduleConfig`, `HeartbeatConfig` structs + `load`, `default_for_home`, `write_default` functions. Zero new dependencies (uses `serde` + `toml` already in `Cargo.toml`).

---

## Cron mechanism — macOS (launchd plist)

`advisory-cron register` on macOS generates a plist file at `~/Library/LaunchAgents/com.advisorycron.<label>.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC ...>
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.advisorycron.<label></string>

    <key>ProgramArguments</key>
    <array>
        <string>/Users/<user>/.cargo/bin/advisory-cron</string>
        <string>run</string>
    </array>

    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key><integer>9</integer>
        <key>Minute</key><integer>0</integer>
    </dict>

    <key>StandardOutPath</key>
    <string>/tmp/advisory-cron-<label>.stdout.log</string>

    <key>StandardErrorPath</key>
    <string>/tmp/advisory-cron-<label>.stderr.log</string>

    <key>WorkingDirectory</key>
    <string><user's repo path></string>

    <key>RunAtLoad</key>
    <false/>
</dict>
</plist>
```

Then `launchctl bootstrap gui/$UID ~/Library/LaunchAgents/com.advisorycron.<label>.plist` registers it with the user session.

**Cron → Calendar mapping (Phase 1 constraint):** `register --schedule` accepts only `M H * * *` daily form (all of day-of-month, month, day-of-week must be `*`). launchd uses `StartCalendarInterval` (Hour/Minute), not a crontab engine — complex expressions (ranges, lists, steps, day-of-week) are unsupported in Phase 1. Config-driven `[schedule]` with `hour`/`minute` calendar form has no such restriction.

**Lifecycle:**
- `register` writes plist + bootstraps. Plist written BEFORE bootstrap attempt — if bootstrap fails, plist is left for user inspection.
- `unregister` `launchctl bootout gui/$UID/com.advisorycron.<label>` + removes plist file. **Idempotent:** succeeds (exit 0) if label not loaded or plist already absent.
- `status` `launchctl print gui/$UID/com.advisorycron.<label>` parses output for next fire time. **P005 Discovery:** macOS 15 (Darwin 25.5.0) exposes NO "next fire" timestamp key — only the configured `descriptor = { "Hour" => N "Minute" => M }`. `parse_next_fire` extracts these to render "daily at HH:MM" (configured recurrence). Future macOS versions without this key → renders "unknown (launchctl format not recognized)".

**UID resolution:** `launchctl` requires numeric UID (not `$UID` shell expansion). `src/scheduler/macos.rs::current_uid()` (private) shells out `id -u` (zero-unsafe, zero-dep — Heads-up #5 Option B resolution).

**Bootout idempotency note:** empirically verified (Anchor #17) — when label not loaded, `launchctl bootout` exits 3 with stdout `"Boot-out failed: 3: No such process"`. advisory-cron treats ANY non-zero launchctl exit as warn-continue (no substring branching).

**Why launchd and not cron?** Sếp uses macOS. launchd is the native scheduler — handles sleep/wake, integrates with GUI session, doesn't need crontab editing. Linux support via systemd timer or cron deferred to Phase 3.

---

## Cron mechanism — Linux (crontab injection)

`advisory-cron register` on Linux invokes `crontab -l` to read the user's existing crontab, filters out any prior advisory-cron-managed line tagged `# advisory-cron: <label>` (idempotent re-register), appends a new managed line, and pipes the result back via `crontab -` (stdin):

```
<minute> <hour> * * * <self_exe> run # advisory-cron: <label>
```

Example: `register --label scan --schedule "0 9 * * *"` writes:

```
0 9 * * * /home/sep/.cargo/bin/advisory-cron run # advisory-cron: scan
```

**Sync shell-out (P013 V2):** the Linux impl uses sync `std::process::Command` for both `crontab -l` and `crontab -`. The `Scheduler` trait is sync (P012), and the CLI entry runs under `#[tokio::main]` — using `tokio::process::Command` would either require `io-util` feature (absent) or a nested `tokio::runtime::Runtime` (panics inside an existing runtime). Stdlib `std::process` + `std::io::Write` is the natural fit: zero new feature flag, zero nested-runtime risk, ~10ms blocking cost per call is negligible at ~1 register/day.

**Cron form constraint (Phase 3.2):** daily form `M H * * *` only (parity with macOS launchd `StartCalendarInterval` per Phase 1 INV-11). Full 5-field cron (ranges, lists, steps, DOW) deferred to P014 INV-23.

**"No crontab" graceful path:** when the user has no crontab yet, `crontab -l` exits 1 with stderr `"no crontab for <user>"`. `CrontabScheduler::register` treats this as empty input and proceeds normally. `CrontabScheduler::status` returns `is_registered: false` silently (mirrors macOS `bootout` idempotent fallback).

**Last-writer-wins race:** between `crontab -l` (read) and `crontab -` (write), another process modifying the user crontab races. P013 accepts last-writer-wins. Hardening (advisory `flock(2)` on sentinel) deferred — Phase 3.5+ if dogfood reveals need.

**No `--config` interpolation:** the managed line invokes `advisory-cron run` with no `--config` flag, mirroring the macOS plist `ProgramArguments = ["<self_exe>", "run"]` pattern. `run` resolves config via `core::config_path::default_config_path()`. If Sếp uses non-default config path, Phase 3.5+ `RegisterIntent` extension would be required.

**No stdout/stderr redirect:** the cron line omits redirect operators (`> /path`). cron discards stdout/stderr by default (or mails on some distros). `advisory-cron run` writes its own heartbeat JSONL via `core::run::run` — the durable observability record. Manual `> /path` redirect can be added post-register by editing the crontab line directly (NOT preserved on re-register — Phase 3.5+ concern).

**`next_fire` parsing:** Phase 3.2 (P013) does NOT parse `M H * * *` from the Linux descriptor — `core::status::run` calls `parse_next_fire` which is macOS-format-specific. On Linux `next_fire` renders `None`. P014 INV-23 adds a parallel `parse_cron_descriptor` parser when full 5-field cron support lands.

---

## MCP surface (Phase 1.7 — stdio)

`advisory-cron mcp` launches an MCP (Model Context Protocol) JSON-RPC 2.0 server over stdin/stdout. Designed for Claude Desktop / Claude Code MCP client integration — NOT a long-running network daemon (matches hard line #1: client owns process lifetime).

**Transport:** stdio only in Phase 1 (rmcp `transport::io::stdio()`). HTTP/SSE deferred (not needed for solo macOS).

**SDK:** `rmcp = "1.7.0"` (official Anthropic Rust MCP SDK). `ServerHandler` trait with `get_info`, `list_tools`, `call_tool` methods. Hand-written JSON schemas via `serde_json::json!` + `Arc<serde_json::Map>` (Decision 3 — no `#[derive(JsonSchema)]` on our types needed; avoids `schemars` dep).

**Tool registry:** 5 tools, 1-1 with CLI subcommands. Each tool's handler in `src/mcp/tools.rs` delegates to the corresponding `core::*::run` (same code path as CLI).

| MCP tool | Mirrors CLI | Input schema | Output |
|----------|-------------|--------------|--------|
| `init` | `advisory-cron init` | `{ force?: bool, config_path?: string }` | `{ config_path: string, written: bool }` JSON |
| `register` | `advisory-cron register` | `{ label: string (required), schedule?: string, config_path?: string }` | `{ plist_path, label, bootstrapped }` JSON |
| `unregister` | `advisory-cron unregister` | `{ label: string (required), config_path?: string }` | `{ label, plist_existed, was_loaded }` JSON |
| `run` | `advisory-cron run` | `{ config_path?: string }` | `{ exit_code, duration_ms, stdout_tail, stderr_tail, heartbeat_appended }` JSON |
| `status` | `advisory-cron status` | `{ label?: string, config_path?: string, last?: int (default 5) }` | `StatusReport` JSON |

**INV-18 — MCP transport boundary validation** (3-point defense-in-depth):
1. `label` field: ASCII alphanumeric + `-` + `_`, non-empty. Validated via `validate_label()` before `core::*` call.
2. `config_path` field: must not contain `..` components. Validated via `validate_config_path()` before `core::*` call.
3. Tool errors returned as `CallToolResult { is_error: Some(true), content: [text message] }` — never as JSON-RPC error responses or process exits.

**`cli/mcp.rs::run` return contract (V2 [O1.1]):**
- Returns `Result<u8>` (not `Result<()>` or `process::exit()`).
- Transport success → `Ok(0)`.
- Transport/initialization error → `Ok(5)` (after `eprintln!` to stderr).
- Exit code 5 propagated by `main.rs` via the standard dispatch return.

**Claude Desktop registration:**

```json
// ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "advisory-cron": {
      "command": "/Users/<user>/.cargo/bin/advisory-cron",
      "args": ["mcp"]
    }
  }
}
```

Replace `<user>` with your macOS username. Verify binary path with `which advisory-cron`.

**Behavioral invariant:** `register` via MCP MUST produce identical plist + identical `launchctl` state as `register` via CLI. Enforced by both routing to `core::register::run`. Tested by `tests/cli_mcp.rs::parity_cli_register_uses_correct_label_suffix`.

---

## CI matrix (Phase 3.3 — P014)

advisory-cron uses a 2-OS GitHub Actions matrix to guarantee cross-OS health on every push + PR against `main`:

| Job | Runner | Tests gated | Tests common |
|-----|--------|-------------|--------------|
| `test (macos-latest)` | macos-latest (Apple Silicon) | `scheduler::macos` unit tests (use in-module `NoopLaunchctl` — no real `launchctl`) + `tests/cli_register.rs` (`#[cfg(target_os = "macos")]`, spawn compiled binary) | `core::*`, `config`, `runner`, `heartbeat`, `alert`, `scheduler::mod` (cross-OS), `mcp::*` |
| `test (ubuntu-latest)` | ubuntu-latest | `scheduler::linux` unit tests + `tests/cli_register_linux.rs` (`#[cfg(target_os = "linux")]`) | same |

Each job runs (in order, fail-fast within the job):
1. `cargo fmt --all -- --check`
2. `cargo build --release`
3. `cargo test --all`
4. `cargo clippy --all-targets -- -D warnings`

`fail-fast: false` at the matrix level — if macOS fails we still see the Linux signal (and vice versa) per push.

**Sub-mechanism B capability check (Linux only):** before `cargo test`, the Linux job runs `which crontab` to verify `cron` package is present on the `ubuntu-latest` runner. If absent, the step fails loud — a follow-up phiếu would add `sudo apt-get install -y cron`. The macOS job has no equivalent pre-step because `launchctl` is part of macOS itself (no install step possible).

**macOS GHA sandbox safety model (two layers):** P012 design intent splits the macOS test surface into two:
1. **Unit tests inside `src/scheduler/macos.rs`** — use the in-module `NoopLaunchctl` test impl. These NEVER shell-out to real `launchctl` and run cleanly on any host (Sếp's Mac, GHA `macos-latest` sandbox, future Linux dev box building with `--target x86_64-apple-darwin`).
2. **Integration tests in `tests/cli_register.rs`** — spawn the compiled binary (per the test file's own header docstring, these CANNOT inject `NoopScheduler`). On the GHA `macos-latest` runner, `launchctl bootstrap` may be unavailable or restricted; integration tests rely on graceful degradation (exit non-zero with a sandbox-related error message rather than panic). DP4 (observe-first): the first CI run reveals which integration tests fail on the sandbox; a Tầng 2 follow-up adds `#[ignore]` or `if: matrix.os != 'macos-latest'` step skip selectively. The only real `launchctl` paths exercised end-to-end live in Sếp's dogfood + manual `advisory-cron register` on a real Mac, not in CI.

**Why no caching, no release artifact upload, no tag-triggered release:** advisory-cron is solo + small binary (~3.9 MB release). Per-job CI ~3 minutes is acceptable. Phase 4+ would add `actions/cache@v4` if CI cost matters; Phase 6+ would add release-on-tag workflow if `cargo publish` is in scope.

---

## Heartbeat schema

Append-only JSONL at `$XDG_STATE_HOME/advisory-cron/heartbeat.jsonl` (default `~/.local/state/advisory-cron/heartbeat.jsonl`):

```json
{"ts": "2026-05-27T02:00:00Z", "label": "advisory-scan-daily", "exit_code": 0, "duration_ms": 45230, "stdout_tail": "...last 1KB of stdout...", "stderr_tail": ""}
```

Fields:

| Field | Type | Description |
|-------|------|-------------|
| `ts` | RFC3339 UTC | When the fire completed |
| `label` | string | launchd label (matches config) |
| `exit_code` | int | Process exit code (0 = success) |
| `duration_ms` | int | Wall-clock ms from spawn → exit |
| `stdout_tail` | string | Last 1KB of stdout (truncated UTF-8) |
| `stderr_tail` | string | Last 1KB of stderr (truncated UTF-8) |

**Schema versioning:** No version field in Phase 1. If we change schema in Phase 2 (adding `retry_attempt`, etc.), bump to add `schema_version: 1` + migration path.

**Retry semantics (Phase 2.2):** when `[retry]` config is present and a task fires multiple times in one `advisory-cron run` invocation, EACH attempt produces ONE heartbeat JSONL line (with its own `ts`, `exit_code`, `duration_ms`). The schema does NOT carry a `retry_attempt` field — Phase 2.2 explicitly preserves the Phase 1.4 schema. `advisory-cron status --last N` naturally shows the per-attempt trail.

### Atomicity (Phase 2.3 — P010)

`heartbeat::append` uses a **temp+fsync+rename** protocol to guarantee that each call is crash-safe: the heartbeat file at any moment observable to another process is either the file as it was BEFORE the call, or the file as it would be after the call's full success. There is no observable partial state.

Protocol (per call):
1. Read existing heartbeat file contents into memory (empty if file missing).
2. Append the new JSONL line (with trailing `\n`) to the in-memory buffer.
3. Create a `NamedTempFile` in the **same directory** as the target heartbeat file (required — atomic rename only works on the same filesystem).
4. Write the full buffer to the temp file.
5. `fsync` the temp file via `sync_all()` (data + metadata — file size durable across power loss).
6. Atomically rename the temp file over the target via `NamedTempFile::persist(target)` (POSIX `rename(2)` — atomic on same-fs).

If any step before the rename fails, the temp file is auto-cleaned via `Drop`; the target file is untouched. The caller (`core::run::run`) log-warn-continues on `Err` per P004 contract — task is NOT failed on heartbeat write failure.

`heartbeat::read_last_n` tolerates ONE corrupt or truncated trailing line (likely from a pre-P010 interrupted write): `tracing::warn!` + skip. Corruption at any line OTHER than the last propagates as `Err` — mid-file corruption is impossible under the atomic-write protocol and must surface loud per PROJECT.md hard line #5.

Trade-off: atomic-rename rewrites the entire heartbeat file on every append. At Sếp's expected usage (1 fire/day, ~1 KB/day, ~365 KB/year), the per-append cost is microseconds. INV-21 documents the boundary in full.

**Why not `fsync-append` (O_APPEND + fsync)?** POSIX guarantees only writes ≤ `PIPE_BUF` (typically 4 KiB) are atomic with O_APPEND. Heartbeat records are usually well under this, but large `stderr_tail` content with high JSON-escape expansion could exceed it silently. Atomic rename is a hard POSIX guarantee independent of size — conservative choice for a fault-tolerance phiếu.

---

## Error handling + alerting

Phase 1: errors go to stderr + exit code. No external alerting.

Phase 2 ships `src/alert.rs` (P008):
- Telegram bot POST on `exit_code != 0` (best-effort).
- Configurable via `[alert.telegram]` block (chat_id + bot_token OR bot_token_file).
- INV-19 boundary: 10s timeout (reqwest client + `tokio::time::timeout` outer guard), error returned to caller as `Result<()>`.
- `alert.rs` is env-free; the test-only API base override env var is read at the call site in `core::run::run` and passed to `send_with_base(base, msg)`. This keeps `alert.rs` unit-testable in isolation.
- Caller in `core::run::run` log-warn-continues on alert send error — alert failure does NOT fail the task. Heartbeat JSONL is the durable failure record; Telegram is the push channel.

### Retry policy (Phase 2.2 — P009)

When `[retry]` block is configured, `core::run::run` wraps `runner::fire_task` in a bounded loop:

- Up to `max_attempts` total fires per `advisory-cron run` invocation
- `tokio::time::sleep(backoff_secs)` between attempts (skip before first, skip after last)
- `is_retryable(exit_code)`: retryable iff `exit_code ∈ 1..=127`. NOT retryable: `0` (success), `≥128` (signal-killed), `-1` (spawn-failure sentinel per INV-14)
- 1 heartbeat JSONL line per attempt (schema unchanged from Phase 1.4)
- Telegram alert fires AT MOST ONCE per invocation — after the loop, gated on final `exit_code != 0`. Successful retry → zero alerts. Exhausted retries → one alert. Signal-killed → one alert (immediate, no retry).

INV-20 enforces all four rules: bounded attempts, backoff respected, signal-exits not retried, single-alert-per-invocation.

When `[retry]` block is absent, behavior is Phase 2.1 single-fire (1 attempt, alert on fail) — backwards-compat preserved via `unwrap_or((1, 0))` default.

Error categories (anyhow context chain):

| Category | Recovery |
|----------|----------|
| Config parse fail | Exit 2, print line:col of TOML error |
| launchd operation fail | Exit 3, surface stderr from `launchctl` |
| Task spawn fail | Exit 4, log to heartbeat with exit_code=-1 + stderr_tail=<spawn error> |
| Heartbeat write fail | Log warning to stderr, do NOT fail the run (task already succeeded) |

---

## Phase status

- ✅ **Phase 1** — Code COMPLETE (all 7 sub-phases shipped). Awaiting Sếp dogfood 3 ngày để close sprint per BACKLOG acceptance. Phase 1.1 shipped: CLI scaffold (5 subcommand stubs, clap derive). Phase 1.2 shipped: config schema (TOML + serde, `advisory-cron init` wired). Phase 1.3 shipped: launchd plist generator + `register`/`unregister` handlers (newtype dispatch, LaunchctlClient trait, idempotent unregister, zero new dep). Phase 1.4 shipped: task runner + heartbeat JSONL (`src/runner.rs` + `src/heartbeat.rs` + `run --config` flag wired; `serde_json` explicit dep; `task.label` optional config field). Phase 1.5 shipped: status reporter (`launchctl print` parsing of `descriptor` Hour/Minute → "daily at HH:MM"; heartbeat read-render; new CLI flags `--label / --config / --json / --last`; `LaunchctlClient` trait extended with `print`; INV-17 appended for `launchctl print` shell-out boundary). **Discovery (P005):** macOS 15 launchctl does NOT expose a "next fire" timestamp for `StartCalendarInterval` jobs — only configured recurrence via `descriptor = { "Hour" => N "Minute" => M }`. Acceptance gate satisfied via configured-recurrence rendering. Phase 1.7 shipped: MCP server wrapper (rmcp 1.7.0 stdio; `core::*` extraction for dual-surface parity; 5 tools; INV-18; 94 tests pass). Phase 1.6 (README + ARCHITECTURE docs polish) shipped per P007.
- ✅ **Phase 2** — COMPLETE. Phase 2.1 (Telegram alert) shipped per P008. Phase 2.2 (retry policy) shipped per P009 (`is_retryable` private fn + retry loop in `core/run.rs`; 1 heartbeat per attempt schema preserved; alert moved OUTSIDE loop per INV-20 single-alert-per-invocation; `[retry]` opt-in config block). Phase 2.3 (state recovery) shipped per P010 (heartbeat `append` refactored to temp+fsync+rename atomic protocol; `read_last_n` tolerates corrupt last line; INV-21 added; no schema change). **All 10 phiếu of the sprint shipped — sprint closes 2026-05-27.**
- 🚧 **Phase 3** — In progress.
  - ✅ **Phase 3.1** (P012): `Scheduler` trait extracted. `src/launchd.rs` → `src/scheduler/{mod,macos,linux}.rs`. `PlatformScheduler` compile-time alias. macOS behavior unchanged; Linux stub compiles (`bail!` P013). Linux WSL2 build verified: 4.7MB binary, zero warnings.
  - ✅ **Phase 3.2** (P013): `CrontabScheduler` real impl shipped — sync `std::process::Command` for `crontab -l`/`-` (no tokio feature add, no nested-runtime panic), INV-22 defense-in-depth via shared `scheduler::is_valid_label`, Linux WSL2 dogfood smoke verified (register/unregister round-trip clean, idempotency confirmed), 14 new tests (143 total), binary 4.8MB. P012 watch-item closed (empty `plist_path` render gated in `cli/register.rs`).
  - ✅ **Phase 3.3** (P014): INV-22 (`crontab` shell-out boundary — 5 sub-rules parallel to INV-10/12/17) + INV-23 (cron expression daily-form invariant cross-platform) appended to `docs/security/INVARIANTS.md`. GitHub Actions CI workflow `.github/workflows/ci.yml` created — `matrix: os: [macos-latest, ubuntu-latest]` running `cargo fmt --check`, `cargo build --release`, `cargo test --all`, `cargo clippy --all-targets -- -D warnings` on each. Linux job pre-step `which crontab` (Sub-mechanism B capability smoke). No code change; doctrine + CI infra only.

*(Worker updates this section at end of each phase EXECUTE — Tầng 2 status text.)*
