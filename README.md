# advisory-cron

**Local cron wrapper for periodic Claude Code checks.**

Fires periodic tasks (e.g. `/advisory-scan`, daily reports, backup verifies) via macOS launchd or Linux cron, with heartbeat logging and Telegram alert on failure. Single Rust binary, no runtime dependencies beyond standard system tools.

> **Why this exists:** `advisory-watch` subagent + `/advisory-scan` slash command exist in sos-kit, but they fire only when something pulls the trigger. GitHub Actions cron can pause (quota), and Claude Code is not always open. Local launchd plist fires regardless of editor state.

## Install

```bash
cargo install --git https://github.com/aspelldenny/advisory-cron
```

Or from source:

```bash
git clone https://github.com/aspelldenny/advisory-cron.git
cd advisory-cron
cargo install --path .
```

## Quick start (CLI)

```bash
# 1. Write default config to ~/.config/advisory-cron/config.toml
advisory-cron init

# 2. Register a launchd plist that fires daily at 09:00
advisory-cron register --label advisory-scan-daily --schedule "0 9 * * *"

# 3. Verify the plist is loaded
launchctl list | grep com.advisorycron

# 4. Fire the configured task immediately (one-shot test)
advisory-cron run

# 5. Show launchd state + last 5 heartbeats
advisory-cron status --label advisory-scan-daily

# 6. Unregister when done testing
advisory-cron unregister --label advisory-scan-daily
```

## MCP server (Claude Desktop / Claude Code)

`advisory-cron` exposes all 5 CLI operations as MCP tools over stdio — one JSON-RPC 2.0 server, no network daemon.

**Step 1 — Install the binary** (see above).

**Step 2 — Register with Claude Desktop.**

Add the following to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "advisory-cron": {
      "command": "/Users/<YOUR_USERNAME>/.cargo/bin/advisory-cron",
      "args": ["mcp"]
    }
  }
}
```

Replace `<YOUR_USERNAME>` with your macOS username. Confirm binary path with `which advisory-cron`.

**Step 3 — Restart Claude Desktop.** The following tools become available:

| Tool | What it does |
|------|-------------|
| `init` | Write default config (`force`, `config_path` params) |
| `register` | Generate + bootstrap launchd plist (`label` required, `schedule`, `config_path` optional) |
| `unregister` | Bootout + remove plist (`label` required) |
| `run` | Fire configured task once, append heartbeat |
| `status` | Return launchd state + last N heartbeat records as JSON |

**Quick smoke test** (no Claude Desktop needed):

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}' \
  | advisory-cron mcp
```

Expected: JSON response with `"serverInfo":{"name":"advisory-cron",...}`.

## What advisory-cron fires

Out of the box, `advisory-cron init` writes a config that runs `claude -p /advisory-scan` (sos-kit's vulnerability scanner) daily at 09:00. To fire something else, edit `~/.config/advisory-cron/config.toml`:

```toml
[task]
command = "claude"
args = ["-p", "/my-slash-command"]
working_dir = "/Users/<YOU>/some-repo"
label = "my-task"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "/Users/<YOU>/.local/state/advisory-cron/heartbeat.jsonl"
```

Re-register after editing: `advisory-cron unregister --label my-task && advisory-cron register --label my-task`.

## Status

Phase 1 complete — CLI + MCP wrapper (stdio) shipped; awaiting Sếp dogfood. Track progress in [`docs/BACKLOG.md`](docs/BACKLOG.md).

## License

MIT — see [LICENSE](LICENSE).
