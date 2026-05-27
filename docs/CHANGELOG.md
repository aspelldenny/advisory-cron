# CHANGELOG — advisory-cron

> Newest entries at top. Follows sos-kit convention: 1 entry per phiếu (Tầng 1) or per ship batch (Tầng 2 grouping).
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/CHANGELOG_ARCHIVE.md`.

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
