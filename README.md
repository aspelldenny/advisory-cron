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

## Quick start

```bash
# Generate a config skeleton in the current directory
advisory-cron init

# Register the launchd plist (macOS) to fire daily at 09:00 ICT
advisory-cron register --daily 09:00 --tz Asia/Ho_Chi_Minh

# Run the configured task immediately (one-shot, for testing)
advisory-cron run

# Show next fire time + last run status
advisory-cron status
```

## Status

🚧 **Bootstrap** — repo seeded 2026-05-27. Phase 1 MVP not yet shipped. Track progress in [`docs/BACKLOG.md`](docs/BACKLOG.md).

## License

MIT — see [LICENSE](LICENSE).
