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
| `src/main.rs` | clap-derive CLI entry point. Parses subcommand + dispatches to `cli::*` handlers. | 1.1 |
| `src/cli/init.rs` | `advisory-cron init` — write default config to `~/.config/advisory-cron/config.toml`. | 1.2 |
| `src/cli/register.rs` | `advisory-cron register` — generate plist + `launchctl bootstrap`. | 1.3 |
| `src/cli/unregister.rs` | `advisory-cron unregister` — `launchctl bootout` + remove plist. | 1.3 |
| `src/cli/run.rs` | `advisory-cron run` — fire task once, write heartbeat. | 1.4 |
| `src/cli/status.rs` | `advisory-cron status` — read launchd next-fire + last heartbeat. | 1.5 |
| `src/config.rs` | TOML config schema (serde-derive). Validation on load. | 1.2 |
| `src/launchd.rs` | Plist XML generation + `launchctl` shell invocation wrappers. macOS-only. | 1.3 |
| `src/runner.rs` | `tokio::process::Command` task spawn + capture stdout/stderr/exit. | 1.4 |
| `src/heartbeat.rs` | JSONL append + read-last-N. | 1.4 |

*(Phase 2 adds `src/alert.rs` for Telegram + `src/retry.rs` for retry policy.)*

---

## CLI surface

| Subcommand | Args | Behavior | Phase |
|------------|------|----------|-------|
| `init` | `--force` (overwrite) | Write default config | 1.2 |
| `register` | `--schedule <cron>` `--label <name>` | Generate + load plist | 1.3 |
| `unregister` | `--label <name>` | Remove + unload plist | 1.3 |
| `run` | (no args) | Fire configured task once | 1.4 |
| `status` | `--json` (machine output) | Show next fire + last heartbeat | 1.5 |

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Generic error (parse, IO) |
| 2 | Config not found / invalid |
| 3 | launchd operation failed |
| 4 | Task fire failed (subcommand `run` only) |
| 130 | SIGINT (Ctrl+C) |

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

**Lifecycle:**
- `register` writes plist + bootstraps
- `unregister` `launchctl bootout gui/$UID <label>` + removes plist file
- `status` `launchctl print gui/$UID/<label>` parses output for next fire time

**Why launchd and not cron?** Sếp uses macOS. launchd is the native scheduler — handles sleep/wake, integrates with GUI session, doesn't need crontab editing. Linux support via systemd timer or cron deferred to Phase 3.

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

Phase 2 will add `src/alert.rs`:
- Telegram bot POST on exit_code != 0
- Configurable via `[alert.telegram]` block
- Best-effort (alert failure ≠ task failure)

Error categories (anyhow context chain):

| Category | Recovery |
|----------|----------|
| Config parse fail | Exit 2, print line:col of TOML error |
| launchd operation fail | Exit 3, surface stderr from `launchctl` |
| Task spawn fail | Exit 4, log to heartbeat with exit_code=-1 + stderr_tail=<spawn error> |
| Heartbeat write fail | Log warning to stderr, do NOT fail the run (task already succeeded) |

---

## Phase status

- 🚧 **Phase 1** — Bootstrap. Module structure planned (above), no code shipped yet.
- ⏸️ **Phase 2** — Deferred. Trigger: Phase 1 dogfood xanh 3 ngày.
- ⏸️ **Phase 3** — Deferred. Trigger: Phase 2 ship + need Linux support.

*(Worker updates this section at end of each phase EXECUTE — Tầng 2 status text.)*
