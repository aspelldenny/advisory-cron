# WORKFLOW — advisory-cron

> Generic workflow for Rust CLI projects in sos-kit v2.1+. Adapted from tarot's WORKFLOW.md, trimmed of tarot-specific bits (Next.js, Prisma, AI provider routing).

---

## §0 — Phiếu naming + branch convention

**Format:** `<type>/P<NNN>-<slug>` (ví dụ `feat/P001-launchd-plist-register`).

- **Type** ∈ {feat, fix, chore, docs, infra}
- **NNN** = 3 digits từ `.phieu-counter` (atomic increment)
- **Slug** = kebab-case ngắn (3-5 words)

**Tạo phiếu:**

```bash
# 1. Atomic counter increment
N=$(cat .phieu-counter | tr -d '[:space:]')
NEXT=$(printf "%03d" $((10#$N + 1)))
echo $NEXT > .phieu-counter

# 2. Branch + file
git checkout -b feat/P${NEXT}-<slug>
cp docs/ticket/TICKET_TEMPLATE.md docs/ticket/P${NEXT}-<slug>.md

# 3. Sed header to fill ID
sed -i '' "s/P<NNN>/P${NEXT}/g" docs/ticket/P${NEXT}-<slug>.md
```

> Future: shell function `phieu <slug>` in sos-kit/bin/ wraps this. Until then, do manually.

**Gotchas:**
- Counter atomicity: increment TRƯỚC `git checkout`. If checkout fails → rollback counter (`echo $N > .phieu-counter`).
- Counter is local (gitignored). Cross-machine sync via DISCOVERIES note.
- Filename khớp branch (bỏ type prefix).

---

## §1 — 5 loại phiếu (Rust-flavored)

### 1.1 CLI surface change (subcommand / flag / exit code)

**Always Tầng 1.** Touches user-facing contract.

Steps:
1. Architect specifies clap struct change in phiếu (e.g. "add subcommand `dryrun`").
2. Worker CHALLENGE verifies no name collision with existing subcommands (`grep "^\s*#\[command" src/cli/`).
3. Worker EXECUTE adds variant + handler + `--help` text + integration test.
4. Update `README.md` quick-start if new top-level command.
5. Update `docs/ARCHITECTURE.md` CLI surface table.

### 1.2 Config schema change (`.docs-gate.toml`, `.sos-stack.toml`, own config)

**Always Tầng 1.** Touches durable file format.

Steps:
1. Architect specifies new field + default value + validation rule.
2. Worker CHALLENGE checks if change breaks existing user configs (semver-think — additive vs breaking).
3. Worker EXECUTE adds field to struct + serde derive + validation in `Config::load()`.
4. Update `.advisory-cron.toml.example` (or equivalent).
5. Update `docs/ARCHITECTURE.md` config schema section.
6. Migration note in CHANGELOG if breaking.

### 1.3 New dependency (`Cargo.toml [dependencies]`)

**Always Tầng 1.** Touches supply chain + license.

Steps:
1. Architect cites why this crate (alternatives considered, license, MSRV compat).
2. Worker CHALLENGE verifies:
   - `cargo add <crate> --dry-run` works
   - License compatible with MIT (check `cargo deny` config — when added)
   - MSRV >= our target (1.85 for edition 2024)
3. Worker EXECUTE `cargo add <crate>@<version>`.
4. Run `cargo build --release` + `cargo test --all` + `cargo clippy --all-targets -- -D warnings`.
5. Discovery Report notes new crate + reason.

### 1.4 Internal refactor (no public API change)

**Default Tầng 2** unless touches >3 modules or >200 LOC.

Steps:
1. Architect describes refactor goal + scope.
2. Worker EXECUTE (skip CHALLENGE if Tầng 2):
   - Re-run all tests after each commit
   - Discovery Report notes any anchor mismatch
3. CHANGELOG entry only if behavior observably changed.

### 1.5 Docs / phiếu-only

**Always Tầng 2.** No code touched.

Steps:
1. Architect writes docs spec in phiếu.
2. Worker EXECUTE edits docs file.
3. docs-gate green pre-commit.

---

## §2 — Step Gate (after each subtask)

After each Nhiệm vụ:

```bash
cargo check                    # ~5s — fast feedback
cargo test --all <module>      # target subset if known
cargo clippy --all-targets -- -D warnings  # before commit
```

If any fails → fix before next subtask. Don't accumulate broken state.

---

## §3 — Docs Gate (before commit)

⛔ **BẮT BUỘC** trước mỗi commit.

1. **Tầng 1 trigger check** — any of these touched:
   - CLI subcommand / flag / exit code
   - Config schema field
   - New dependency
   - Cron mechanism (plist layout, schedule format)
   - External API contract (Telegram webhook URL/body, Claude Code invocation pattern)
   - Module signature visible across modules
   - Security boundary (env var read, file write outside `.sos-state/` or `docs/runlog/`)

   → If yes: docs (CHANGELOG + ARCHITECTURE) MUST be updated this commit.

2. **Run docs-gate:**

```bash
docs-gate --all --verbose
# OR via MCP: mcp__docs-gate__check_all
```

Exit 0 = pass. Exit non-zero = fix + retry. Never `git commit --no-verify`.

---

## §4 — Commit sequence

```
1. Code changes (tested pass)
2. Update docs/CHANGELOG.md (Tầng 1 entry minimum)
3. Update docs/ARCHITECTURE.md (Tầng 1 sections)
4. Update CLAUDE.md if conventions changed (rare)
5. Write Discovery Report:
   - docs/discoveries/P<NNN>.md (full report)
   - 1-line index in docs/DISCOVERIES.md
6. git add <specific files>  # KHÔNG git add -A blindly
7. cargo build --release && cargo test --all && cargo clippy --all-targets -- -D warnings
8. git commit -m "<type>(P<NNN>): <summary>"
```

Commit message conventional commits format (matches branch type).

---

## §5 — PR + ship

```bash
git push origin <branch>
gh pr create --title "<type>(P<NNN>): <summary>" --body "..."
```

PR body includes:
- Phiếu link
- Discovery Report summary (1-2 lines)
- Test plan (manual + automated)
- 🤖 Generated with Claude Code footer

If Worker (orchestrator-spawned EXECUTE) — orchestrator narrates push + invokes `/security-review <PR>` if security surface touched.

---

## §6 — Cleanup post-merge

```bash
git checkout main
git pull
git branch -d <merged-branch>
# Worktree cleanup if used:
git worktree remove <worktree-path>
```

Move BACKLOG item from "Active sprint" to "Recently shipped" with 1-line summary.

---

## §0.5 — Session opening (Sếp opens new chat)

Resume priority order:
1. **Tier 1 auto-load:** Active sprint from BACKLOG.md (banner-injected at SessionStart).
2. **Tier 2:** PR open + phiếu active (banner shows open PR count).
3. **Tier 3:** New idea / item from BACKLOG.md open backlog.

KHÔNG pick item mới khi PR open hoặc phiếu active còn dở.
