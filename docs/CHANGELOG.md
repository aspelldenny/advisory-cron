# CHANGELOG — advisory-cron

> Newest entries at top. Follows sos-kit convention: 1 entry per phiếu (Tầng 1) or per ship batch (Tầng 2 grouping).
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/CHANGELOG_ARCHIVE.md`.

---

## 2026-05-27 — P004: Phase 1.4 — Task runner + heartbeat JSONL + wire `run` handler

**Phiếu:** P004 (Tầng 1 — 2 new modules, new dep `serde_json`, new optional config field `task.label`, new CLI flag `run --config`, INVARIANTS.md updated)

**New dependency:**
- `serde_json = "1"` promoted from transitive (via reqwest) to explicit in `[dependencies]`. Required for `serde_json::to_string` / `serde_json::from_str` in `heartbeat.rs`. Marginal binary delta = 0 (already compiled transitively). Per RULES.md Tầng 1 row: CHANGELOG entry citing crate + reason.

**Modules added:**
- `src/runner.rs` — `RunResult` struct; `fire_task(config) -> Result<RunResult>` spawns task via `tokio::process::Command`, captures stdout/stderr/exit-code/duration. Signal-killed → exit_code=-1. Spawn failure → `anyhow::Error` (caller builds spawn-fail heartbeat). 3 unit tests.
- `src/heartbeat.rs` — `HeartbeatRecord` struct (durable schema: ts, label, exit_code, duration_ms, stdout_tail, stderr_tail); `append(path, record)` (JSONL, auto-creates parent dir); `read_last_n(path, n)` (skips malformed lines); `tail_utf8(s, max_bytes)` (char-boundary snap, no grapheme dep). 7 unit tests.

**CLI: `advisory-cron run` wired (Phase 1.4 acceptance):**
- `src/cli/run.rs` rewritten: loads config via `--config <path>` or default `~/.config/advisory-cron/config.toml`; fires task via `runner::fire_task`; builds `HeartbeatRecord`; appends to `heartbeat.log_path`.
- **`default_config_path` bails on `$HOME` unset** — mirrors `register.rs` pattern, never silently falls back to `/` (P004 V2 Turn 1 Architect decision).
- Exit codes: 0 = task success; 2 = config load fail OR `$HOME` unset; 4 = task non-zero exit OR spawn-fail. Heartbeat write failure = stderr warning only, never changes exit code.
- `--config <path>` flag added — declared in `run::Args` (NOT in `cli/mod.rs`, per newtype dispatch constraint). `git diff src/cli/mod.rs` empty.

**Config schema change (`src/config.rs`):**
- `TaskConfig` gains `pub label: Option<String>` with `#[serde(default)]` — backward compat (old configs without `label` field deserialize to `None`). `default_for_home` seeds `label: Some("advisory-cron")` so `advisory-cron init` writes the new field visibly.

**INVARIANTS.md updated:**
- Appended INV-14 (child process spawn boundary), INV-15 (heartbeat file write boundary), INV-16 (JSON serialization boundary). Per RULES.md:22 — security boundary touched.

**Tests added:**
- 3 unit tests in `src/runner.rs` (echo success, nonexistent binary error, non-zero exit)
- 7 unit tests in `src/heartbeat.rs` (serde roundtrip, append creates dir+file, append+read roundtrip, missing file returns empty, malformed line skip, tail_utf8 variants)
- 3 unit tests in `src/config.rs` (label absent → None, label present → Some, default_for_home includes label)
- 4 integration tests in `tests/cli_run.rs` (echo success exit 0 + heartbeat written, failing task exit 4, spawn-fail exit 4 + exit_code=-1 in heartbeat, missing config exit 2)
- Total: 51 tests (34 unit + 3 cli_help regression + 4 cli_init regression + 6 cli_register regression + 4 cli_run)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules table marks `src/runner.rs` + `src/heartbeat.rs` 1.4 ✅; §Config schema field reference adds `task.label` row + TOML block updated; §CLI surface table `run` row updated to show `--config <path>` flag; §Phase status updated to 1.4 shipped
- `docs/security/INVARIANTS.md` — INV-14..INV-16 appended

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, binary 1.1MB
- `cargo test --all` — 51/51 pass (33 baseline + 18 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #1 hard rule satisfied)
- `env -u HOME advisory-cron run` → exit 2 with `$HOME environment variable is not set` (Constraint #16 confirmed)

---

## 2026-05-27 — P003: Phase 1.3 — launchd plist generator + `register`/`unregister` handlers

**Phiếu:** P003 (Tầng 1 — module added, CLI flags added, schedule type relaxed, plist schema spec, exit code 3 first use, INVARIANTS.md updated)

**Module added:**
- `src/launchd.rs` — `generate_plist` (pure XML builder, 7-key plist matching ARCHITECTURE.md spec), `plist_path_for`, `default_launch_agents_dir`, `LaunchctlClient` trait, `RealLaunchctl` (shells `launchctl bootstrap`/`bootout` via `std::process::Command`), `NoopLaunchctl` (test impl recording calls), `current_uid` (`id -u` shell-out, zero-unsafe, zero-dep). 11 unit tests.

**CLI: `register` + `unregister` wired:**
- `src/cli/register.rs` rewritten: loads config, generates plist, writes to `~/Library/LaunchAgents/`, bootstraps via `launchctl`. Testable surface via `run_with_deps<L: LaunchctlClient>`.
- `src/cli/unregister.rs` rewritten: idempotent bootout + plist file removal. Warns on "label not loaded" or "plist already absent" — continues to exit 0. Exit 3 only on hard IO failure.
- **`--config <path>` flag added to both** — declared inside `Args` struct (NOT on Commands enum) per newtype dispatch pattern confirmed Turn 1 [O1.1]. Zero edits to `src/cli/mod.rs`.
- **`register --schedule` relaxed from required `String` to `Option<String>`** — config-driven schedule works without redundant CLI flag.
- Exit codes: register (0=success, 1=$HOME unset, 2=config/cron-parse fail, 3=plist write / launchctl bootstrap fail); unregister (0=success including idempotent, 1=$HOME unset, 3=plist remove IO fail).

**Plist XML schema (7 keys — matches ARCHITECTURE.md §Cron mechanism):**
- `Label`, `ProgramArguments` (`[<self_exe>, "run"]`), `StartCalendarInterval` (Hour+Minute), `StandardOutPath` (`/tmp/advisory-cron-<label>.stdout.log`), `StandardErrorPath`, `WorkingDirectory`, `RunAtLoad` (`<false/>`)

**Cron expression support:**
- Simple `M H * * *` daily form only (Phase 1 launchd constraint). Complex expressions → exit 2 with helpful error. Config-driven `hour`/`minute` calendar form has no such restriction.

**`#[allow(dead_code)]` removal:**
- `src/config.rs:72` `#[allow(dead_code)]` on `pub fn load` removed — first binary callsite wired in `register::run`. (Heads-up #1 resolution.)

**INVARIANTS.md updated:**
- Appended INV-10 through INV-13 covering: `launchctl` shell-out boundary, `id -u` shell-out boundary, `~/Library/LaunchAgents/` write boundary, label sanitization defense.

**Tests added:**
- 11 unit tests in `src/launchd.rs` (plist gen, cron parsing, label sanitization, NoopLaunchctl recording, xml escape, current_uid sanity)
- 6 integration tests in `tests/cli_register.rs` (plist write, cron simple form, complex cron exit 2, missing config exit 2, idempotent unregister, round-trip register→unregister)
- Total: 33 tests (20 unit + 3 cli_help regression + 4 cli_init regression + 6 cli_register)

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules table marks `src/launchd.rs`/`register.rs`/`unregister.rs` 1.3 ✅; §CLI surface table adds `--config`/`--schedule` optional notes; §Cron mechanism adds M H * * * constraint + idempotency note + UID resolution note; §Phase status updated to 1.3 shipped
- `docs/security/INVARIANTS.md` — INV-10..INV-13 appended

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings
- `cargo test --all` — 33/33 pass
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (V2 [O1.1] constraint satisfied)

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
