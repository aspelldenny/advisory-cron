# PROJECT — advisory-cron

> **PRD-lite** for the kit-builder seat (Sếp). Vision, persona, MVP scope, non-goals, acceptance criteria.

---

## Vision (one paragraph)

`advisory-cron` is a local cron wrapper that fires periodic Claude Code tasks — chiefly `/advisory-scan`, but generalizable to any slash command, shell command, or Claude Code agent invocation — via macOS launchd (Phase 1) or Linux cron/systemd (Phase 2+). It produces a heartbeat JSONL log and sends a Telegram alert when a fire fails or skips. The tool exists because **shipping a check ≠ the check running** (Sub-mechanism A — trigger gap). GitHub Actions cron can pause due to quota, and Claude Code is not always open in a session. A local launchd plist fires regardless of editor state. The same binary is exposed **both as a CLI tool and as an MCP server** (stdio subcommand `advisory-cron mcp`) so Claude Desktop / Claude Code can drive it directly from a chat session — register, fire, inspect status — without dropping to a terminal.

## Why this exists (the trigger gap)

In tarot project 2026-05-27 dogfood, Sếp asked: "thằng trinh sát thì sao? hôm nay đến ngày chạy chưa em? sao anh ko thấy nó chạy tự động?". Three layers of automation existed but none fired:

1. **GHA cron** `if: false` (paused 2026-05-15 → 2026-06-01 — quota cạn).
2. **SessionStart banner threshold** ≥7 days stale (orchestrator auto-spawn rule 10) — but only 1 day stale → silent.
3. **Slash command `/advisory-scan`** — manual, awaiting Sếp/Claude type.

Result: tool built but underfires. `advisory-cron` removes the "Claude Code must be open" + "GHA must have quota" assumptions.

## Persona

| Role | Description |
|------|-------------|
| **Sếp (Chủ nhà)** | Solo dev maintaining ~5 repos. Wants automation that fires regardless of which editor/terminal is open. Wakes laptop daily ~9-10 ICT; macOS user. |
| **sos-kit consumers (future)** | Other devs adopting sos-kit pattern. Their repos may have advisory-watch, daily-report, backup-verify, etc. — all benefit from a generic cron wrapper. |

## MVP scope (Phase 1)

A single `advisory-cron` binary that:

1. **Generates a launchd plist** for a configured task (e.g. fire `/advisory-scan` daily at 09:00 ICT) and loads it via `launchctl bootstrap`.
2. **Runs the configured task on-demand** (one-shot) for testing — `advisory-cron run`.
3. **Logs heartbeats** to `~/.local/state/advisory-cron/heartbeat.jsonl` (or platform-appropriate XDG path).
4. **Reads its own config** from `~/.config/advisory-cron/config.toml` (or repo-local override).
5. **Reports status** — `advisory-cron status` shows next fire time + last run result.
6. **Exposes all 5 subcommands as MCP tools** — `advisory-cron mcp` runs an MCP server over stdio (JSON-RPC 2.0). Full parity: each CLI subcommand (`init`, `register`, `unregister`, `run`, `status`) has a 1-1 MCP tool, so Claude Desktop / Claude Code can invoke any operation from chat.

Out of scope for Phase 1: Telegram alert, retry policy, Linux support, sos-kit packaging, HTTP/SSE MCP transport (stdio only).

## Non-goals (Sếp pre-empts AI bias)

- ❌ Web dashboard / GUI
- ❌ Multi-machine distributed scheduling
- ❌ Plugin architecture for arbitrary "jobs"
- ❌ Service mesh / observability stack
- ❌ Database-backed state (file-based is fine for solo)
- ❌ Auto-fix / auto-merge any output (Sếp gates always)
- ❌ Cross-platform installer / brew formula (Phase 3+ optional)

## Acceptance criteria (Phase 1 ship gate)

- [ ] `advisory-cron init` writes `~/.config/advisory-cron/config.toml` with sane defaults.
- [ ] `advisory-cron register --schedule "0 9 * * *"` generates a launchd plist + loads it.
- [ ] `advisory-cron run` fires the configured task once, captures stdout/stderr, writes heartbeat.
- [ ] `advisory-cron status` shows next fire time from `launchctl print` + reads last heartbeat.
- [ ] `advisory-cron unregister` removes the plist cleanly.
- [ ] `advisory-cron mcp` starts MCP server over stdio (JSON-RPC 2.0); `initialize` handshake returns server info + 5 tools.
- [ ] MCP tools `init` / `register` / `unregister` / `run` / `status` callable from Claude Desktop with config snippet documented in README.
- [ ] `cargo build --release` produces single binary < 7MB (raised from 5MB to budget MCP SDK).
- [ ] `cargo test --all` all pass (includes MCP handshake integration test).
- [ ] README.md install + quick start verified by Sếp dogfood for BOTH CLI and MCP path (Claude Desktop calls `register` via MCP, then sees it in `launchctl list`).
- [ ] Heartbeat JSONL schema documented in ARCHITECTURE.md.
- [ ] MCP tool schema (input/output JSON for each of 5 tools) documented in ARCHITECTURE.md.

## Phase 2+ (deferred — not committed yet)

- **Phase 2:** Telegram alert on fail + retry policy + state recovery from crashed run.
- **Phase 3:** Linux support (systemd timer / cron-tab generation).
- **Phase 4:** sos-kit promotion — copy binary into `~/sos-kit/bin/`, add bootstrap hook in `~/sos-kit/bootstrap/` for new repos.
- **Phase 5:** Optional `cargo publish` if API surface stable.

## Hard lines (founder taste — non-negotiable)

1. **No daemon process.** advisory-cron is fire-and-forget; launchd owns scheduling. No long-running rust process.
2. **No telemetry to external services** beyond user-configured Telegram. No anonymous usage stats.
3. **No magic config discovery beyond 2 paths.** Repo-local `.advisory-cron.toml` OR `~/.config/advisory-cron/config.toml`. Period.
4. **Heartbeat log is append-only.** Never compacted, never auto-rotated by advisory-cron itself. User rotates via standard logrotate if needed.
5. **Failure mode = noisy.** Silent failure is the bug we're fixing. If fire fails, log + alert. Never swallow.
