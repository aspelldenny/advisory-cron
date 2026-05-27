# CHANGELOG — advisory-cron

> Newest entries at top. Follows sos-kit convention: 1 entry per phiếu (Tầng 1) or per ship batch (Tầng 2 grouping).
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/CHANGELOG_ARCHIVE.md`.

---

## 2026-05-27 — P002: Phase 1.2 — Config schema (TOML + serde) + wire `init` handler

**Phiếu:** P002 (Tầng 1 — introduces config schema, touched by every subsequent subcommand)

**Module added:**
- `src/config.rs` — `Config`, `TaskConfig`, `ScheduleConfig` (untagged enum), `HeartbeatConfig` structs; `load`, `default_for_home`, `write_default` functions. Zero new dependencies (uses `serde` + `toml` already in `Cargo.toml`).

**Config schema:**
- 3 TOML blocks: `[task]` (command, args, working_dir), `[schedule]` (cron expr OR calendar hour/minute), `[heartbeat]` (log_path)
- `[schedule]` uses `#[serde(untagged)]` enum — serde discriminates by field presence. Both variants round-trip cleanly.
- Default path: `~/.config/advisory-cron/config.toml` (hardcoded per PROJECT.md hard line #3)
- Validation: empty command rejected; Calendar hour 0–23, minute 0–59

**CLI: `advisory-cron init` wired:**
- `src/cli/init.rs` rewritten: calls `Config::write_default`, maps exit codes per ARCHITECTURE.md spec
- Exit 0 on success, exit 2 on config-exists-without-force + IO errors, exit 1 (via `Err`) on `$HOME` unset
- Stdout: `wrote default config to <path>`

**Tests added:**
- 9 unit tests in `src/config.rs` (schedule parsing, validation failures, write_default round-trip/overwrite)
- 4 integration tests in `tests/cli_init.rs` (write success, refuse overwrite, force overwrite, TOML parseable)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — new §Config schema section; §Modules table `src/config.rs` + `src/cli/init.rs` marked 1.2 ✅; §Phase status updated

**Discovery note:**
- `#[serde(untagged)]` enum confirmed working with TOML 0.8 — cron-shape and calendar-shape discriminate correctly. No fallback to tagged variant needed.
- Added `validate_rejects_invalid_minute` test (not in phiếu's 8-test count) — natural companion to `validate_rejects_invalid_hour`. Total: 9 unit tests.
- `pub fn load` carries `#[allow(dead_code)]` to suppress Rust 2024 binary-crate dead_code warning on forward-declared API (will be called by Phase 1.3 `register`).

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 16/16 pass (9 unit + 3 cli_help regression + 4 cli_init)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff

---

## 2026-05-27 — P001: Phase 1.1 — Scaffold + CLI surface (clap derive)

**Phiếu:** P001 (Tầng 1 — defines CLI contract for entire tool)

**Modules added:**
- `src/cli/mod.rs` — `Commands` enum (5 subcommands) + `dispatch()` fn
- `src/cli/init.rs` — `init` stub with `--force` arg
- `src/cli/register.rs` — `register` stub with `--schedule` + `--label` args
- `src/cli/unregister.rs` — `unregister` stub with `--label` arg
- `src/cli/run.rs` — `run` stub (no args)
- `src/cli/status.rs` — `status` stub with `--json` arg

**src/main.rs rewritten:**
- clap derive `Cli` struct with `#[command(subcommand)]`
- `#[tokio::main(flavor = "current_thread")]` (current_thread flavor, matching `rt` feature in Cargo.toml — see Discovery P001 for detail)
- `ExitCode::from(u8)` return for clean stdio flush
- Dispatches via `cli::dispatch()`, maps `Err` → exit 1 with `{err:#}` to stderr

**CLI surface (5 stubs):**
- Each stub returns `bail!("not yet implemented (Phase 1.x)")` → exit 1
- `--help` for all 5 subcommands exits 0 and shows correct arg docs
- `--version` prints `advisory-cron 0.1.0`

**Tests added:**
- `tests/cli_help.rs` — 3 integration tests (top-level help, per-sub help, unknown sub exits nonzero)
- All 3 pass: `cargo test --all` clean

**Layering decision:**
- `src/core/` NOT created (defer until Phase 1.2 has real logic to host — anti-completeness-bias decision from Architect)
- `src/cli/mod.rs` is the only parent; no intermediate abstraction needed for 5-stub phase

**Discovery note:**
- Cargo.toml had tokio `rt` feature but not `rt-multi-thread`. Phiếu spec used bare `#[tokio::main]` (defaults to multi-thread). Fixed to `#[tokio::main(flavor = "current_thread")]` — zero dep change, behavior identical for stub CLI.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 3/3 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff

---

## 2026-05-27 — Phase 1 scope expansion: CLI + MCP dual surface

**Trigger:** Sếp re-defined Phase 1 ship-gate. "Tool rust phải gói thành MCP và CLI mới là hoàn thành." → CLI-only Phase 1 insufficient; MCP server (stdio) must ship in same wave.

**Decisions captured:**
- MCP tool set = full parity với CLI (5 tools: `init` / `register` / `unregister` / `run` / `status`)
- Transport = stdio subcommand `advisory-cron mcp` (single binary, no daemon, matches hard line #1)
- Sprint shape = thêm Phase 1.7 (MCP wrapper) sau 1.5, đẩy 1.6 (docs) xuống cuối

**Doc updates (Tầng 1 — touches acceptance criteria + sprint + module layout):**
- `docs/PROJECT.md` — Vision para extended; MVP scope item 6 added (MCP server, 5 tools); acceptance criteria +3 bullets (MCP handshake, Claude Desktop integration test, MCP schema doc); binary size budget raised 5MB → 7MB
- `docs/BACKLOG.md` — Active sprint title + goal expanded; Phase 1.7 added (~300 LOC, Tầng 1); Phase 1.6 moved to end, scope raised 60 → 90min
- `docs/ARCHITECTURE.md` — Modules table +4 entries (`cli/mcp.rs`, `core/mod.rs`, `mcp/server.rs`, `mcp/tools.rs`); layering invariant added (`core::*` is CLI/MCP-agnostic); CLI surface +`mcp` subcommand; new "MCP surface" section with tool registry + Claude Desktop config sketch + behavioral invariant; exit code 5 (MCP transport error)

**Known TBD for Architect (P00x — Phase 1.7):**
- Rust MCP SDK choice (likely `rmcp` official Anthropic crate — verify via `context7` before spec)
- Whether to introduce `schemars` for auto-derived JSON tool schemas (size budget consideration)
- Exact integration test shape ("MCP register ≡ CLI register" diff against shared temp LaunchAgents dir)

**Not yet started:** no phiếu opened. Next: P001 = Phase 1.1 (CLI scaffold).

---

## 2026-05-27 — Pre-flight: secrets + env prep (no code)

Sếp batched all "nguyên liệu" before opening P001 so the sprint can run end-to-end without mid-flight blocks.

**Audit results (toolchain):**
- ✅ Claude Code CLI `/Users/nguyenhuuanh/.local/bin/claude` v2.1.152
- ✅ Claude Desktop installed + config at `~/Library/Application Support/Claude/claude_desktop_config.json`
- ✅ docs-gate + ship CLI at `~/.cargo/bin/`
- ✅ Rust 1.94.1 (MSRV 1.85 satisfied)
- ✅ gh CLI logged in as `aspelldenny` via keyring
- ✅ launchctl available (Darwin Bootstrapper 7.0.0)

**Secrets staged (outside repo, gitignored defense-in-depth):**
- `~/.advisory-cron-secrets.env` chmod 600 — `TG_BOT_TOKEN` + `TG_BOT_USERNAME=chiha_alert_bot` + `TG_CHAT_ID=1184530337`
- End-to-end verified: `curl ... sendMessage` returned `ok:True message_id:21`
- Bot reused from Soulsign project (not advisory-cron exclusive) — Sếp accepted shared-bot risk

**Shell env cleanup (`~/.zshrc`):**
- Line 21: `export GITHUB_TOKEN="gho_s1lB..."` → commented out (OAuth, was shadowed)
- Line 341: `export GITHUB_TOKEN=ghp_59Zq...` → commented out (invalid per `gh auth status`)
- `gh` CLI continues to work via keyring; clean shell test (`env -i ... zsh -i -c`) shows `GITHUB_TOKEN: (unset)`
- Current Claude Code session env still carries old value (inherited at spawn) — Sếp `exec zsh` or open new terminal to flush

**Sếp acknowledged risk:** plaintext tokens (TG + 2 GitHub) appeared in chat output; Sếp's threat model = Claude Code session is private → accepted. Recommend rotation at end of cycle.

**Pre-req status for sprint:**
- Phase 1.1–1.7: ✅ all green, no external input pending
- Phase 2.1: ✅ secrets ready (BACKLOG entry updated)
- Phase 2.2–2.3, Phase 3+: no external input needed

---

## 2026-05-27 — Bootstrap (seed)

**Repo initialized.** `cargo new` Phase 0 scaffold + sos-kit doctrine seed by orchestrator (running from tarot main session 2026-05-27).

Seeded structure:
- `CLAUDE.md` — Rust shape + ported generic doctrine (DOD, Discovery Report, AI BIAS WARNINGS rule 6, Sub-mechanism A-E catalog, Knowledge durability, DOCS GATE 2 Tầng)
- `docs/` — PROJECT.md (PRD), BACKLOG.md (3 phase), ARCHITECTURE.md, WORKFLOW.md, ORCHESTRATION.md, RULES.md, CHANGELOG.md, DISCOVERIES.md, ticket/TICKET_TEMPLATE.md, security/INVARIANTS.md
- `.claude/agents/` — 5 vai (architect, worker, orchestrator, advisory-watch, boundary-check) — copied from `~/sos-kit/agents/`, adapted for Rust + autonomous mode default
- `.claude/skills/` — symlink to `~/sos-kit/skills/` (13 generic skills shared)
- `.claude/commands/` — `/advisory-scan`, `/security-review`
- `.claude/settings.local.json` — permission allowlist + SessionStart hook
- `scripts/session-start-banner.sh` — Rust-flavored banner (BACKLOG active + advisory staleness + open PRs)
- `.git/hooks/pre-commit` — sos-kit canonical hook (auto-detects Rust → `cargo check`)
- `.mcp.json` — filesystem + github + sequential-thinking + context7 + docs-gate + ship (omit guard/vps/sentry — not relevant)
- `.docs-gate.toml`, `.sos-stack.toml`, `.phieu-counter`, `LICENSE`, `README.md`, `.gitignore`, `Cargo.toml` (deps: clap + tokio + serde + toml + chrono + anyhow + thiserror + tracing + reqwest)

No code shipped yet. Phase 1 MVP starts when Sếp opens fresh session in `~/advisory-cron`.

**Source / lineage:**
- Doctrine from `~/tarot/CLAUDE.md` (2026-05-27 snapshot) — DOD, AI BIAS WARNINGS Sub-mechanism A-E catalog, Knowledge durability, Discovery Report convention
- Agents (5 vai) from `~/sos-kit/agents/` — generic baseline, customized worker (Rust Layer 2 matrix) + orchestrator (autonomous mode default, runlog, rule 10/11 ported)
- Skills from `~/sos-kit/skills/` — symlink (13 generic)
- Hooks from `~/sos-kit/hooks/pre-commit` — auto-detects Cargo.toml → `cargo check`
- Templates from `~/sos-kit/templates/` — INVARIANTS, BACKLOG, .docs-gate.toml, .sos-stack.toml
- CLAUDE.md shape skeleton from `~/docs-gate/CLAUDE.md` — Rust project structure

**Stress test:** Sếp's intent is to drive Phase 1 → Phase 3 1-mạch không can thiệp, observe whether 4-vai workflow (Quản đốc + Architect + Worker + Giám sát) can auto-chain without manual gating. autonomous mode default flag set in `.claude/agents/orchestrator.md`.
