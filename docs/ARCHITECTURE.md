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
| `src/core/register.rs` | `run(RegisterArgs, &L: LaunchctlClient) -> Result<RegisterOutput>` — generate plist + bootstrap. Resolves home + `launch_agents_dir` + `self_exe` internally. | 1.7 ✅ |
| `src/core/unregister.rs` | `run(UnregisterArgs, &L: LaunchctlClient) -> Result<UnregisterOutput>` — bootout + remove plist. Idempotent. Resolves home internally. | 1.7 ✅ |
| `src/core/run.rs` | `async run(RunArgs) -> Result<RunOutput>` — fire task once + write heartbeat. Full runner logic extracted from `cli/run.rs`. | 1.7 ✅ |
| `src/core/status.rs` | `run(StatusArgs, &L: LaunchctlClient) -> Result<StatusReport>` — launchd query + heartbeat read. `parse_next_fire` moved here from `cli/status.rs`. `StatusReport` pub (shared by CLI render + MCP serialize). | 1.7 ✅ |
| `src/mcp/mod.rs` | Re-exports `server` and `tools` sub-modules. | 1.7 ✅ |
| `src/mcp/server.rs` | `serve_stdio() -> Result<()>` — rmcp `ServerHandler::serve(stdio()).await` + `.waiting().await`. Converts SDK errors to `anyhow::Error`. | 1.7 ✅ |
| `src/mcp/tools.rs` | `AdvisoryCronHandler` implementing rmcp `ServerHandler`. 5 tools with hand-written JSON schemas (Decision 3). INV-18 validation (`validate_label`, `validate_config_path`) at MCP boundary before `core::*` call. Tool errors as `is_error=true` results. | 1.7 ✅ |
| `src/config.rs` | TOML config schema (serde-derive). Validation on load. | 1.2 ✅ |
| `src/launchd.rs` | Plist XML generation + `launchctl` shell invocation wrappers. macOS-only. `LaunchctlClient` trait + `RealLaunchctl`/`NoopLaunchctl` impls + `current_uid()` helper. Extended P005: `LaunchctlClient::print` method + `LaunchctlPrintOutput` struct (status reporter). `parse_next_fire` moved to `src/core/status.rs` in P006. | 1.3 ✅ → 1.5 ✅ |
| `src/runner.rs` | `tokio::process::Command` task spawn + capture stdout/stderr/exit. `RunResult` struct. | 1.4 ✅ |
| `src/heartbeat.rs` | JSONL append + read-last-N. `HeartbeatRecord` struct (durable schema). `tail_utf8` helper. | 1.4 ✅ |
| `src/alert.rs` | `TelegramAlert::send_with_base` outbound POST to Telegram Bot API. Best-effort (alert fail ≠ task fail). Env-free module — the API base test-seam env var is read at the call site in `core::run::run`, NOT here. INV-19 boundary (10s timeout double-guard: reqwest client + `tokio::time::timeout`). `format_failure_message` centralises message format. | 2.1 ✅ |

*(Phase 2.2 will add `src/retry.rs` for retry policy. Phase 2.3 adds crash-safe heartbeat write.)*

**Layering invariant (shipped Phase 1.7):** `core::*` knows nothing about CLI or MCP. `cli::*` and `mcp::*` are both thin adapters. A single code path = single behavior — `register` from CLI and `register` from MCP MUST produce identical plist + identical side effects.

**V2 internal-resolution pattern (P006):** Every `core::*::run` fn resolves its own env dependencies (`$HOME`, `LaunchAgents` dir, `current_exe`) internally via stdlib. ONLY `&L: LaunchctlClient` is injected for testability (prod = `RealLaunchctl`, test = `NoopLaunchctl`). No config-path or home-dir threading through call stacks.

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

## Cron mechanism (Phase 1 — launchd)

`advisory-cron register` generates a plist file at `~/Library/LaunchAgents/com.advisorycron.<label>.plist`:

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

**UID resolution:** `launchctl` requires numeric UID (not `$UID` shell expansion). `src/launchd.rs::current_uid()` shells out `id -u` (zero-unsafe, zero-dep — Heads-up #5 Option B resolution).

**Bootout idempotency note:** empirically verified (Anchor #17) — when label not loaded, `launchctl bootout` exits 3 with stdout `"Boot-out failed: 3: No such process"`. advisory-cron treats ANY non-zero launchctl exit as warn-continue (no substring branching).

**Why launchd and not cron?** Sếp uses macOS. launchd is the native scheduler — handles sleep/wake, integrates with GUI session, doesn't need crontab editing. Linux support via systemd timer or cron deferred to Phase 3.

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

---

## Error handling + alerting

Phase 1: errors go to stderr + exit code. No external alerting.

Phase 2 ships `src/alert.rs` (P008):
- Telegram bot POST on `exit_code != 0` (best-effort).
- Configurable via `[alert.telegram]` block (chat_id + bot_token OR bot_token_file).
- INV-19 boundary: 10s timeout (reqwest client + `tokio::time::timeout` outer guard), error returned to caller as `Result<()>`.
- `alert.rs` is env-free; the test-only API base override env var is read at the call site in `core::run::run` and passed to `send_with_base(base, msg)`. This keeps `alert.rs` unit-testable in isolation.
- Caller in `core::run::run` log-warn-continues on alert send error — alert failure does NOT fail the task. Heartbeat JSONL is the durable failure record; Telegram is the push channel.

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
- 🚧 **Phase 2** — In progress. Phase 2.1 (Telegram alert) shipped per P008. Phase 2.2 (retry) + 2.3 (state recovery) pending.
- ⏸️ **Phase 3** — Deferred. Trigger: Phase 2 ship + need Linux support.

*(Worker updates this section at end of each phase EXECUTE — Tầng 2 status text.)*
