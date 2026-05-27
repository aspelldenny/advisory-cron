# CHANGELOG — advisory-cron

> Newest entries at top. Follows sos-kit convention: 1 entry per phiếu (Tầng 1) or per ship batch (Tầng 2 grouping).
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/CHANGELOG_ARCHIVE.md`.

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
