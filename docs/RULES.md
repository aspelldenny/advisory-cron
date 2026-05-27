# RULES — advisory-cron

> Detailed enforcement spec. CLAUDE.md gives the tóm tắt; this file gives the full bảng.

---

## DOCS GATE 2 Tầng — detail

### Tầng 1 (CỨNG) — thiếu update = KHÔNG commit

| Type of change | Required docs update |
|----------------|----------------------|
| CLI subcommand added/removed/renamed | `docs/ARCHITECTURE.md` §CLI surface table + `README.md` quick-start |
| CLI flag added/removed/renamed | `docs/ARCHITECTURE.md` §CLI surface |
| Exit code added or semantic change | `docs/ARCHITECTURE.md` §CLI surface exit codes |
| `Cargo.toml` `[dependencies]` add/remove | `docs/CHANGELOG.md` entry citing crate + reason |
| Config field added/removed (any config file) | `docs/ARCHITECTURE.md` §Config schema + migration note in CHANGELOG if breaking |
| launchd plist layout change | `docs/ARCHITECTURE.md` §Cron mechanism + example plist block |
| Heartbeat schema change | `docs/ARCHITECTURE.md` §Heartbeat schema + version bump |
| External API contract (Telegram body, Claude Code invocation) | `docs/ARCHITECTURE.md` §Error handling + alerting + CHANGELOG |
| Module added/removed | `docs/ARCHITECTURE.md` §Modules table |
| Security boundary touched (env var read, file write outside `.sos-state/` or `docs/runlog/`) | **AUTO Tầng 1.** `docs/security/INVARIANTS.md` review + CHANGELOG entry |
| New `unsafe { }` block introduced | **AUTO Tầng 1.** `docs/security/INVARIANTS.md` rationale + CHANGELOG |

### Tầng 2 (mềm) — không block commit

- Local variable / parameter name rename
- Internal error message wording (non-CLI-facing)
- Tracing log span name / log level (internal)
- Comment edits
- Doc typo fix
- Code style (rustfmt)
- Adding tests without changing prod code

---

## Hard Stops — DỪNG NGAY, HỎI SẾP

Khi Worker đang EXECUTE và gặp 1 trong các tình huống sau → STOP, escalate `AskUserQuestion`:

1. **Thêm module / file mới** ngoài scope phiếu
2. **Thêm dependency** không có trong phiếu (`Cargo.toml`)
3. **Đổi CLI interface** (subcommand, flag, exit code) ngoài scope
4. **Đổi config schema** ngoài scope
5. **Đổi cron mechanism** (plist layout, schedule format) ngoài scope
6. **Refactor code không liên quan** đến phiếu
7. **Write `unsafe { }`** even if "obviously safe" — escalate
8. **Force-push** to recover from rebase conflict
9. **`launchctl bootout`** on label not in phiếu (would clobber other jobs)
10. **`cargo install --force`** outside phiếu's worktree
11. **Edit `.claude/settings.local.json`** UNLESS phiếu explicitly lists it
12. **Delete files under `.sos-state/`**
13. **`rm -rf` on absolute paths** or `~/`

For each: AskUserQuestion với options A. abandon op / B. Sếp executes manually / C. update phiếu scope (return to Architect).

---

## Discovery Report format (mandatory)

**Per-phiếu file:** `docs/discoveries/P<NNN>.md`

```markdown
## Discovery Report — P<NNN>

### Assumptions trong phiếu — ĐÚNG:
- [Liệt kê từng assumption khớp code thật]

### Assumptions trong phiếu — SAI so với code thật:
- [Assumption X: phiếu ghi A, code thật là B → đã sửa docs]
- [Nếu không có sai lệch → "Không có"]

### Edge cases / limitations phát hiện thêm:
- [Phiếu không đề cập nhưng phát hiện khi đọc/sửa code]
- [Nếu không có → "Không có"]

### Docs đã cập nhật theo discoveries:
- [File nào đã sửa, sửa gì]
- [Nếu không có → "Không có"]

### Layer 2 capability checks fired (Sub-mechanism A-E):
- [List which sub-mechanism check ran + result]
```

**Index entry** in `docs/DISCOVERIES.md` (newest at top):

```markdown
- 2026-MM-DD P<NNN>: <one-line summary>, <key finding> → see docs/discoveries/P<NNN>.md
```

---

## Knowledge durability convention

| Type of knowledge | Location | Rotate behavior |
|-------------------|----------|-----------------|
| **Durable doctrine** (luật, structural fix, pattern catalog, Sub-mechanism breakdown) | `CLAUDE.md` / `.claude/agents/*.md` / `docs/RULES.md` / `docs/security/INVARIANTS.md` | **NEVER rotate.** All agents read on context load. |
| **Operational evidence** (specific instance: file:line bug found, anchor mismatch fixed, scan result) | `docs/DISCOVERIES.md` index + `docs/discoveries/P<NNN>.md` | **Rotate** when DISCOVERIES.md > 1000 lines → `docs/Archive/DISCOVERIES_ARCHIVE.md`. Historical context only. |
| **Session-level observability** (state transitions, subagent spawn timing) | `docs/runlog/<date>-<sid>.jsonl` | **Git-ignored.** Rotate by date (filename). Sếp opens for post-mortem only. |

**Cross-reference DOCTRINE → DISCOVERIES** = soft link. Broken OK after rotate. Doctrine self-contained without DISCOVERIES.

---

## Layer 2 capability check matrix (Sub-mechanism A-E)

Every phiếu MUST run applicable checks in EXECUTE phase Task 0:

| Sub-mech | Symptom | Check command | Expected |
|----------|---------|---------------|----------|
| A — Trigger gap | thing exists, nothing pulls trigger | `launchctl list \| grep <label>` | row present |
| | | `launchctl print user/$UID/<label>` | next fire time set |
| B — Capability gap | spec written ≠ runtime tool capable | `cargo check` | exit 0 |
| | | `cargo test <module>` | targeted tests pass |
| C — Migration completeness | schema migrated, old data lost | `jq '.field \| length' state.json` before/after | counts match (or grow) |
| D — Persistence lifecycle | doctrine in rotate-prone file | `grep -l "<rule name>" CLAUDE.md docs/RULES.md` | ≥1 hit in persistent location |
| E — Environment drift | local pass ≠ fresh-install pass | `cargo update --dry-run` | no surprise major bump |
| | | `cargo build --release` from clean `target/` | exit 0 |

If any check fails → Discovery Report records it. Decide: fix in this phiếu OR escalate to follow-up phiếu.

---

## Commit sequence

```
1. Code changes (tested pass per Step Gate)
2. Update docs/CHANGELOG.md (Tầng 1 entry minimum)
3. Update docs/ARCHITECTURE.md (Tầng 1 sections per matrix above)
4. Update CLAUDE.md if conventions changed (rare)
5. Write Discovery Report (per-phiếu file + 1-line DISCOVERIES.md index)
6. git add <specific files>  # KHÔNG git add -A blindly
7. cargo build --release && cargo test --all && cargo clippy --all-targets -- -D warnings
8. git commit -m "<type>(P<NNN>): <summary>"
```

---

## Git workflow safety rails

| Operation | Allowed | Forbidden |
|-----------|---------|-----------|
| `git push <branch>` | ✅ | `git push --force` / `-f` |
| `git reset --hard` | only inside phiếu worktree | outside phiếu worktree |
| `git checkout -b` | ✅ for new phiếu branch | overwriting `main` |
| `git rebase main` | ✅ to update phiếu branch | rebase main onto branch |
| `git merge --squash` | post-PR approval | direct to main without PR |
| `gh pr merge` | only after `/security-review` if security surface touched (rule 9) | bypass security review on auto-merge |

---

## Phiếu lifecycle

1. **DRAFT** — Architect writes phiếu V1 in `docs/ticket/P<NNN>-<slug>.md`. `Tầng: 1|2` mandatory in header.
2. **CHALLENGE** (Tầng 1 only) — Worker reads phiếu + grep-verifies anchors + writes Debate Log Turn N.
3. **RESPOND** — Architect responds per objection, bumps phiếu version.
4. **APPROVAL_GATE** — orchestrator narrate (autonomous) or AskUserQuestion (interrupted).
5. **EXECUTE** — Worker codes, tests, writes Discovery Report, commits, pushes.
6. **SECURITY_REVIEW** (conditional) — orchestrator invokes `/security-review <PR>` if security surface touched.
7. **MERGE** — Sếp or orchestrator merges PR after green CI + APPROVE verdict.
8. **CLEANUP** — branch deleted, phiếu moved to `Recently shipped` in BACKLOG, CHANGELOG entry confirmed.

Banner shows `🧹 Phiếu P<NNN> approved + merged. Run: phieu-done P<NNN>` post-merge — Sếp acks, orchestrator MUST NOT auto-run cleanup.
