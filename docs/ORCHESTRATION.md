# ORCHESTRATION — advisory-cron

> Full state machine spec for main session (Quản đốc). Companion to `.claude/agents/orchestrator.md`.

---

## Roles (6 vai — advisory-cron variant)

| Vietnamese | Technical | Tools | Role |
|------------|-----------|-------|------|
| Chủ nhà | (Sếp) | — | Quyết — vision, scope, ship gate |
| Quản đốc | orchestrator (main session) | full | Drive state machine, spawn subagents, narrate |
| Kiến trúc sư | architect | Read, Write, Glob, Task* | Vẽ phiếu Task 0 anchors |
| Thợ | worker | full code tools | Execute phiếu, commit, PR |
| Giám sát | boundary-check | Read, Grep, Glob, Bash (scoped) | Soi PR diff post-push — 5 generic INV |
| Trinh sát | advisory-watch | Read, Grep, Glob, WebFetch, WebSearch, Bash (scoped) | Dò CVE/GHSA crates.io advisory |

*(Tarot has 7th vai `prompt-reviewer` for chị Hạ — not relevant to advisory-cron.)*

---

## State machine

```
IDLE
 │ user gives brief ("build feature X for BACKLOG item Y")
 ▼
DRAFT_PHASE                                spawn Architect (DRAFT)
 │ Architect writes phiếu V1 with Debate Log + sets `Tầng: 1|2` in header
 ├── tầng==2 (lặt vặt) ─────────────────────► APPROVAL_GATE  (skip CHALLENGE)
 ├── tầng==1 (móng nhà) ────────────────────► CHALLENGE_PHASE
 ▼
CHALLENGE_PHASE                            spawn Worker (CHALLENGE)
 │ Worker verifies Task 0 + reads code + writes Debate Log Turn N
 ├── Worker accepted (no objection) ─────────────► APPROVAL_GATE
 ├── Worker raised objections ─────────────────► RESPOND_PHASE
 ▼                                                    │
RESPOND_PHASE                              spawn Architect (RESPOND)
 │ Architect responds per objection, bumps phiếu version
 ├── all objections resolved (no DEFER) ─────► CHALLENGE_PHASE (Turn N+1)
 ├── any DEFER ──────────────────────────────► FORCE_ESCALATION (blocking)
 └── Turn 3 reached ─────────────────────────► FORCE_ESCALATION (blocking)
APPROVAL_GATE
 │ Autonomous mode: narrate "Approval gate — V<N> consensus, proceeding"
 │ Interrupted mode: AskUserQuestion approve/amend/abandon
 ▼
EXECUTE_PHASE                              spawn Worker (EXECUTE)
 │ Worker codes + tests + Discovery Report + commit + push
 ▼
SECURITY_REVIEW (conditional)              invoke /security-review <PR>
 │ Only if PR touches security surface (paths: src/, Cargo.toml, .docs-gate.toml,
 │ .sos-stack.toml, scripts/, .claude/hooks/, .github/workflows/, .env*)
 ├── APPROVE + 0 FLAG ─────────────────────► silent, allow merge
 └── NEEDS_REVIEW (≥1 FLAG) ───────────────► post comment, BLOCK auto-merge
                                              (in autonomous mode this becomes blocking)
 ▼
DONE
```

Cap = 3 turns. Hit Turn 3 without consensus → FORCE_ESCALATION (`AskUserQuestion` to Sếp).

---

## Tier routing (P036)

| Tier | Means | Flow |
|------|-------|------|
| Tầng 1 | móng nhà — CLI / schema / dep / cron / API contract | DRAFT → CHALLENGE → (RESPOND) → APPROVAL_GATE → EXECUTE |
| Tầng 2 | lặt vặt — ≤3 files, ≤200 LOC, no schema/CLI/dep | DRAFT → APPROVAL_GATE → EXECUTE (skip CHALLENGE) |

Worker may escalate 2→1 mid-EXECUTE. Orchestrator may NEVER demote 1→2.

---

## Hard rules

1. **Approval gate mandatory** (narrative form in autonomous mode, blocking when interrupted).
2. **No silent state.** Narrate every transition.
3. **Debate trail in phiếu file.** No external log (runlog is observability only, not source-of-truth).
4. **Max 3 turns** before force-escalating.
5. **User can interrupt anytime.**
6. **One APPROVAL_GATE per phiếu.**
7. **Tier set in DRAFT, escalated up only.**
8. **Bulk input → auto-triage + 1 gate.**
9. **Security surface PR → `/security-review` MANDATORY pre-merge.**
10. **Advisory staleness auto-spawn** per banner signal (rule 10 — ported from tarot).
11. **AI BIAS WARNINGS câu vàng** for every ≥3 sub-task DRAFT (rule 11 — ported from tarot).

---

## Rule 10 — Advisory staleness auto-spawn

SessionStart banner reads `docs/security/.advisory-scan-state` mtime. Thresholds:

| Days stale | Banner | Orchestrator action |
|------------|--------|---------------------|
| 0-2 | silent | None |
| 3-6 | ⚠️ warn line | Narrate "advisory scan cân nhắc" + offer (not mandate) |
| ≥7 OR missing | 🚨 red flag | **BẮT BUỘC auto-spawn `/advisory-scan`** early in session, max 1 turn delay |

Lesson tarot P281 (2026-05-24): "Một cái gác đúng mà không bao giờ được gọi thì vô dụng ngang cái gác sai" — fix structural qua banner trigger.

**advisory-cron-specific irony:** Once Phase 2 ships, this tool IS the launchd-driven cron that auto-fires `/advisory-scan` daily. Rule 10 becomes a fallback for when launchd plist not registered.

---

## Rule 11 — AI BIAS WARNINGS filter

Khi Architect return DRAFT có ≥3 sub-task, orchestrator BẮT BUỘC hỏi câu vàng vào MỖI sub-task TRƯỚC khi spawn Worker CHALLENGE:

> *"Sub này giải vấn đề Sếp ĐANG có, hay GIẢ ĐỊNH có?"*

Over-specced sub-task examples (advisory-cron domain):
- Plugin architecture cho jobs → giải N team, Sếp solo. REJECT.
- Web dashboard → giải team visibility, CLI đủ. REJECT.
- Distributed locking → giải multi-machine, 1 máy. REJECT.

Orchestrator narrate "em propose cắt sub-X vì <reason>" + chạy `AskUserQuestion` filter trước Worker spawn (blocking event — overrides autonomous mode for this decision).

---

## Marker file hygiene

`.sos-state/architect-active` gates the architect-guard hook. Before EVERY spawn:
- Spawn architect (any mode): `mkdir -p .sos-state && touch .sos-state/architect-active`
- Spawn worker (any mode): `rm -f .sos-state/architect-active`

Stale marker → orchestrator runs `rm -f .sos-state/architect-active` defensively before every spawn.

---

## Autonomous mode default

This repo is the **test bed** for autonomous 4-vai workflow. Sếp's pre-approval is **STANDING for any item in active sprint** unless explicitly revoked.

| Event | Autonomous mode | Interrupted mode |
|-------|-----------------|------------------|
| Tầng 1 phiếu, Worker accepted V1 | Narrate "approval gate — V1 consensus, EXECUTE" | `AskUserQuestion` approve/amend/abandon |
| Tầng 2 phiếu DRAFT done | Narrate "Tầng 2, EXECUTE" | `AskUserQuestion` approve/amend/abandon |
| Worker EXECUTE pushed PR | Narrate "/security-review <PR>" (conditional) | Same — security gate not autonomous-skippable |
| Turn 3 cap hit | **Blocking** — `AskUserQuestion` force-escalate | Blocking |
| Architect DEFER TO CHỦ NHÀ | **Blocking** — `AskUserQuestion` | Blocking |
| AI BIAS sub-task filter trip | **Blocking** — `AskUserQuestion` cut sub-X? | Blocking |
| Security FLAG verdict | **Blocking** — Sếp reads comment, decides | Blocking |

Sếp interrupt with any direct message → autonomous mode pauses, orchestrator narrates resume point.

---

## Failure modes + recovery

| Failure | Recovery |
|---|---|
| Architect RESPOND didn't bump phiếu version | Orchestrator re-spawns once with explicit "bump version to V<N+1>". Second failure → escalate. |
| Worker CHALLENGE wrote objection without `file:line` citation | Orchestrator rejects, asks Worker to redo with citations. |
| Stale `.architect-active` marker | Orchestrator runs `rm -f .sos-state/architect-active` before every spawn. |
| Phiếu version went backwards (V3 → V2) | Refuse — orchestrator escalates as a bug in Architect output. |
| Same objection raised in 2 consecutive Worker turns | Indicates Architect didn't actually fix the underlying issue. Force-escalate. |
| Phiếu missing `Tầng` field | Orchestrator rejects, re-spawns Architect with explicit "set Tầng: 1 or 2". |
| Worker silently demoted Tầng 1 → Tầng 2 | Refuse — orchestrator escalates as a bug. Tier escalation is one-way. |

---

## Concrete example session

```
USER: build phiếu cho item "Phase 1.1 — Scaffold + CLI surface" ở Active sprint

ORCHESTRATOR (Quản đốc):
  → mkdir -p .sos-state && touch .sos-state/architect-active
  → Spawn @agent-architect:
    "Spawn architect viết phiếu cho 'Phase 1.1 — Scaffold + CLI surface'.
     BACKLOG.md active sprint item line 17.
     Tầng: 1 (defines CLI contract)."
  → Architect writes docs/ticket/P001-scaffold-cli-surface.md V1
  → Architect returns: "P001 V1 written, Tầng: 1, est 150 LOC, 3 anchors needs Worker verify"
  → rm -f .sos-state/architect-active

  → Narrate: "Architect DRAFT done — P001 Tầng 1, 3 anchors, est 150 LOC. Spawning Worker CHALLENGE."

  → Spawn @agent-worker (CHALLENGE):
    "Worker challenge phiếu P001."
  → Worker runs Task 0 grep, reads phiếu, writes Debate Log Turn 1
  → Worker returns: "Accepted V1 — no challenges. Anchor #1 ✅, #2 ✅, #3 ✅."

  → Narrate (autonomous): "Worker accepted V1, no objections. Approval gate narrated — proceeding to EXECUTE."

  → Spawn @agent-worker (EXECUTE):
    "Worker execute phiếu P001."
  → Worker codes, tests, commits, pushes
  → Worker returns: "P001 shipped, PR #1 created, all tests pass, Discovery Report written"

  → Check security surface: PR #1 touches src/main.rs + Cargo.toml → MATCH
  → Invoke /security-review 1
  → Boundary-check returns: APPROVE + 0 FLAG
  → Narrate (silent-when-clean): "Security review APPROVE, no comment posted. PR #1 ready to merge."

  → DONE state. Wait for Sếp signal or next backlog item.
```
