# PHIẾU P011: Sprint debt cleanup — INV-12 2-point enforcement + DISCOVERIES hook format alignment

> **Loại:** chore
> **Tầng:** 2
> **Ưu tiên:** P2
> **Ảnh hưởng:** `src/core/register.rs` (pre-flight label allowlist check), `src/core/unregister.rs` (pre-flight label allowlist check IF current state only checks empty), `.git/hooks/pre-commit` (DISCOVERIES grep pattern alignment), `docs/CHANGELOG.md` (single P011 entry covering both items), `docs/BACKLOG.md` (move 2 debt items from "Open backlog" → "Recently shipped"). NO `src/launchd.rs`, NO `src/mcp/tools.rs`, NO `docs/security/INVARIANTS.md`, NO `docs/ARCHITECTURE.md`.
> **Dependency:** P001..P010 shipped (sprint closed). No code/spec dependency on the deferred 3rd debt item (`fire_task` no process timeout — Tầng 1, separate phiếu).

---

## Context

### Vấn đề hiện tại

`docs/BACKLOG.md` "Open backlog" section currently lists **3 debt items** carried over from PR#1/#3 worker escalations + PR#3 security review advisory note. Sếp's P011 brief picks the **2 small (Tầng 2) items** to clear in a single chore phiếu; item 3 (`fire_task` no process timeout — Tầng 1, adds config field + tokio::time::timeout wiring) defers to its own future phiếu.

**Item 1 — INV-12 label sanitization 2-point enforcement** (PR#3 security review advisory note, 2026-05-27):

- `docs/security/INVARIANTS.md` INV-12 (line 137-153) **spec requires** label sanitization at TWO points:
  1. Pre-flight check in `register::run` (before generating plist)
  2. Inside `generate_plist` (defense-in-depth)
- INV-12 line 147 implementation paragraph says: `src/launchd.rs::generate_plist — label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')` — point 2 is in place.
- **Current state of point 1 per Sếp brief:** `src/core/register.rs` `run` pre-flight only checks `args.label.is_empty()` (NOT the full ASCII allowlist). This weakens defense-in-depth to a single point (only `generate_plist`).
- Note: INV-12 implementation prose references `src/cli/register.rs::run_with_deps` — that's the **pre-P006** location. P006 extracted register logic to `src/core/register.rs`. INV-12's prose is stale on the location but the spec ("pre-flight in register::run") is unchanged. P011 enforces the spec at the post-P006 location (`src/core/register.rs`). No INVARIANTS.md edit needed (Worker may update the line 147 prose location reference if Tầng 2 cleanup feels in-scope; Architect leaves to Worker — Worker self-decides per Tầng 2 nature).
- Also relevant: `src/core/unregister.rs` `run` likely has a parallel pre-flight (Sếp brief flags it as worth checking). If current state mirrors register (empty-check only), apply the same allowlist tighten for symmetry. If current state already has full allowlist (P006 worker may have added it; unlikely but possible), skip this part — log in Discovery.

**Item 2 — DISCOVERIES.md hook vs CLAUDE.md format mismatch** (PR#1 worker escalation, 2026-05-27):

- `.git/hooks/pre-commit` line 137 checks: `grep -q "## .*$PHIEU_ID" docs/DISCOVERIES.md 2>/dev/null` — i.e. expects an **H2 header** `## ...P<NNN>...`.
- `CLAUDE.md` doctrine §"⛔ DISCOVERY REPORT — BẮT BUỘC MỖI PHIẾU" step 2 (line ~110 of CLAUDE.md) says: append 1-line **list-item** index entry of form `- 2026-MM-DD P<NNN>: <summary> → see docs/discoveries/P<NNN>.md`.
- Worker has been writing **BOTH formats** in every Discovery entry since P001 to satisfy both contracts (see `docs/DISCOVERIES.md` lines 9, 11, 13, 15, 17, 19... — H2 headers AND list items coexist for each phiếu). This is redundant + error-prone (next worker may forget one).
- **Sếp decision (from brief):** Hook script → align with CLAUDE.md doctrine. CLAUDE.md is authoritative. Hook must accept the list-item format. After this phiếu, Worker only writes the list-item; H2 headers in existing P001-P010 entries stay valid (the new grep pattern accepts either; we don't churn historical entries).
- The hook script lives at `.git/hooks/pre-commit` (project copy, per Architect Anchor #5). A near-identical copy also exists at `/Users/nguyenhuuanh/sos-kit/hooks/pre-commit` (system-wide kit source). Architect leaves the system-wide kit copy alone (out of scope — sos-kit doctrine, not this repo). Worker updates ONLY the project's `.git/hooks/pre-commit`.

Both items are **Tầng 2** per CLAUDE.md AI Bias Warnings + RULES.md Tầng 2 matrix:

- Item 1 = behavior change in 1-2 files inside `src/core/*` (existing modules, ≤30 LOC delta total). Stricter input validation only — valid labels unchanged, invalid labels rejected earlier (was rejected at `generate_plist`, will now also be rejected at `core::register::run`). No CLI flag added, no exit code change, no config schema change, no dependency change. ARCHITECTURE.md already documents the allowlist constraint via INV-12 reference (line 144-148 of INVARIANTS.md cross-referenced from ARCHITECTURE.md §Modules row `src/launchd.rs`). No ARCHITECTURE.md edit needed.
- Item 2 = pre-commit hook script edit (repo config, not Rust source). Hook is a developer-tooling boundary, not the CLI surface. No source change required to docs/ARCHITECTURE.md (hook script is not enumerated there).

### Giải pháp

**Item 1 — Add full ASCII allowlist check in `core::register::run` pre-flight + (conditionally) `core::unregister::run` pre-flight:**

- Mirror the exact check shape used inside `generate_plist` in `src/launchd.rs`:
  - Predicate: `label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')` (per INV-12 line 147 + INV-17 line 240 — same allowlist).
  - Empty check stays (current behavior).
  - Combined: empty OR contains-disallowed-char → return error with same wording style as the existing empty-check error (Worker matches existing prose convention from current `register::run` empty-check error message).
- Apply identical check in `core::unregister::run` IF current pre-flight there only checks empty. If `unregister::run` already does the full check (worker may have added it during P006 extraction), leave it alone + log in Discovery as already-correct.
- Helper function approach is permitted (Worker may add a private `is_valid_label(&str) -> bool` or `validate_label(&str) -> Result<()>` in `src/core/register.rs` or shared in `src/core/mod.rs` — Architect leaves to Worker as Tầng 2 stylistic call). If Worker prefers, the existing `is_valid_label` helper in `src/launchd.rs` (per Sếp brief — also noted in MCP `validate_label`) may be re-exported / re-used from `core::register` — but Architect does NOT mandate this; the simplest in-place check is acceptable.
- Worker adds 1-2 unit tests asserting that `core::register::run` rejects an invalid label (e.g. `"foo bar"`, `"foo/bar"`, `"foo;rm"`) BEFORE invoking the launchctl client (i.e. before any plist generation). Test must NOT depend on filesystem state — use the same `NoopLaunchctl` pattern used in existing register tests. Worker reuses the existing register test scaffolding.

**Item 2 — Update `.git/hooks/pre-commit` DISCOVERIES check pattern to accept CLAUDE.md doctrine list-item format:**

- Current pattern (line 137 of `.git/hooks/pre-commit`): `grep -q "## .*$PHIEU_ID" docs/DISCOVERIES.md 2>/dev/null`
- New pattern MUST accept the CLAUDE.md doctrine list-item form:
  - `- 2026-MM-DD P<NNN>: <summary> → see docs/discoveries/P<NNN>.md`
- **Recommended replacement** (one regex, accepts both legacy H2 form AND CLAUDE.md list-item form so historical P001-P010 entries don't break):
  ```bash
  # Match either:
  #   - "## ... P<NNN>" (legacy H2 header — kept for backwards-compat with P001-P010 entries)
  #   - "- YYYY-MM-DD P<NNN>:" (CLAUDE.md doctrine list-item — going forward)
  if ! grep -Eq "(^## .*${PHIEU_ID}([^0-9]|$)|^- [0-9]{4}-[0-9]{2}-[0-9]{2} ${PHIEU_ID}:)" docs/DISCOVERIES.md 2>/dev/null; then
  ```
  - **Why the negative lookahead `([^0-9]|$)` on the H2 form:** prevents `P011` matching against a hypothetical future `## P0110`. The list-item form's trailing `:` already provides the boundary.
  - **Why `grep -E`:** the regex uses `[0-9]{4}` and alternation — extended regex syntax required. The current pattern uses `grep -q` (no `-E`) — adding `-E` is a 1-flag change.
- Worker tests the new pattern by:
  1. Confirming existing `docs/DISCOVERIES.md` (H2 + list-item dual entries for P001-P010) still matches via the legacy alt of the regex.
  2. Creating a temp test scenario: a fresh `docs/discoveries-test.md` (or in-memory string passed to grep) containing ONLY a list-item line (no H2) — confirm new regex matches.
  3. Confirming a totally-missing phiếu ID still fails (i.e. the regex doesn't accidentally match everything).
  4. Running an actual `git commit` dry-run with a fake P011 phiếu file + only-list-item discovery entry → confirm hook exits 0.
- Worker MAY use shellcheck-style validation (`shellcheck .git/hooks/pre-commit`) if it's installed locally — Tầng 2 nice-to-have, not required.

**No INV change.** INV-12 spec is ALREADY 2-point (per the prose at lines 138-148). P011 just enforces what INV-12 already says. No INVARIANTS.md edit. The line 147 prose mentions `src/cli/register.rs::run_with_deps` (pre-P006 location) — Worker MAY update this prose location reference to `src/core/register.rs::run` as a Tầng 2 freebie if it feels in-scope. Architect leaves to Worker; if Worker updates, log in Discovery; if not, log as a known prose-staleness item in Discovery (very minor).

**No ARCHITECTURE.md change.** No new module, no CLI surface change, no config field, no exit code change. Behavior is "stricter input validation" — backward-compatible for ANY valid label (alphanumeric + `-` + `_`). Invalid labels were already rejected (at `generate_plist`) — now rejected one layer earlier. ARCHITECTURE.md §Modules row for `src/launchd.rs` mentions `LaunchctlClient` + plist generation but doesn't enumerate per-fn validation steps; no edit needed.

**No CHANGELOG omission.** Single P011 entry in `docs/CHANGELOG.md` covers both items. Tầng 2 ship — concise entry (≤25 lines).

**BACKLOG move:** After P011 ships, move 2 items from "Open backlog" → "Recently shipped" in `docs/BACKLOG.md`. Item 3 (`fire_task` no timeout) stays in "Open backlog".

### Scope

- **CHỈ sửa:**
  - `src/core/register.rs` (add full allowlist check in `run` pre-flight; potentially add private `is_valid_label` helper or call into one — Worker's call)
  - `src/core/unregister.rs` (IF current pre-flight only checks empty — add same allowlist check; IF already correct — skip and log in Discovery)
  - `.git/hooks/pre-commit` (line 137 pattern update — accept either H2 header OR CLAUDE.md doctrine list-item format)
  - `docs/CHANGELOG.md` (single P011 Tầng 2 entry — both items covered)
  - `docs/BACKLOG.md` (move 2 items "Open backlog" → "Recently shipped")
  - Unit tests in `src/core/register.rs` (and `src/core/unregister.rs` IF that file changes) — invalid-label rejection at pre-flight
  - `docs/discoveries/P011.md` + `docs/DISCOVERIES.md` index (mandatory per CLAUDE.md)

- **KHÔNG sửa:**
  - `src/launchd.rs` — `generate_plist` allowlist already correct; point 2 of INV-12 not touched
  - `src/cli/mod.rs` — Constraint #1 re-instated post-P006 (dispatch unchanged; register/unregister CLI handlers route to `core::*` unchanged)
  - `src/cli/register.rs`, `src/cli/unregister.rs` — handlers route to `core::*` unchanged
  - `src/mcp/tools.rs` — INV-18 `validate_label` at MCP boundary already enforces the allowlist; point 1 of INV-12 is the **CLI/core path**, point 3 (MCP boundary) is INV-18's domain; NOT in P011 scope
  - `src/alert.rs` — Constraint #11 re-instated post-P008/P009/P010
  - `src/heartbeat.rs` — Constraint #12 from P009 (append signature) irrelevant here; heartbeat untouched
  - `src/runner.rs` — Constraint #5 from P009 untouched
  - `src/config.rs` — no config field added (Tầng 2 declaration)
  - `src/core/run.rs` — out of scope (heartbeat/retry/alert path)
  - `src/core/status.rs` — out of scope (status path, separate concern)
  - `src/core/init.rs`, `src/core/mod.rs`, `src/core/config_path.rs`, `src/main.rs` — untouched
  - `docs/security/INVARIANTS.md` — INV-12 spec unchanged (P011 just enforces what's already specified). Worker MAY update the stale `cli/register.rs::run_with_deps` location reference at line 147 as a free Tầng 2 cleanup, OR log as known stale prose in Discovery — either is acceptable.
  - `docs/ARCHITECTURE.md` — no module/CLI/config/exit-code change; no edit needed (Tầng 2)
  - `README.md` — Tầng 2, no CLI surface change, no edit needed
  - `Cargo.toml` — no new dep, Constraint enforced
  - `/Users/nguyenhuuanh/sos-kit/hooks/pre-commit` — system-wide sos-kit hook out of scope (sos-kit doctrine repo, not advisory-cron); leave alone

### Skills consulted

*(none — Architect sourced từ BACKLOG.md "Open backlog" entries, INVARIANTS.md INV-12 / INV-17 / INV-18, ARCHITECTURE.md §Modules + §MCP surface, CHANGELOG.md P006/P010 entries, P010 phiếu (for Constraint inheritance patterns), `.git/hooks/pre-commit` direct Read, CLAUDE.md DISCOVERY REPORT doctrine, Sếp brief.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Architect KHÔNG có Bash/Grep — anchors sourced từ INVARIANTS.md, ARCHITECTURE.md §Modules, CHANGELOG P006 entry (`core::*` extraction), Sếp brief, direct Read of `.git/hooks/pre-commit`. Worker BẮT BUỘC verify thực tế tại Task 0.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/core/register.rs` exists with a `pub fn run` (or `pub async fn run`) that takes `RegisterArgs` + `&L: LaunchctlClient` and performs pre-flight checks BEFORE calling `generate_plist`. Current pre-flight: ONLY `args.label.is_empty()` check (per Sếp brief). | `grep -n "pub fn run\|pub async fn run" src/core/register.rs` + `grep -n "is_empty\|empty()" src/core/register.rs` to locate current pre-flight | `[unverified]` — Architect inferred from Sếp brief + ARCHITECTURE.md §Modules row `src/core/register.rs` (line 46) | ⏳ TO VERIFY |
| 2 | `src/core/unregister.rs` exists with parallel `pub fn run`/`pub async fn run`. **Pre-flight shape unknown** — may have empty-check only (parallel to register), may have full allowlist already, may have NO pre-flight (P006 extracted unregister to core but Architect doesn't know the validation shape). Worker reports actual state. | `grep -n "pub fn run\|pub async fn run" src/core/unregister.rs` + `grep -n "is_empty\|label\|allow\|alphanumeric" src/core/unregister.rs` | `[needs Worker verify]` — Architect explicitly does NOT know | ⏳ TO VERIFY |
| 3 | `src/launchd.rs::generate_plist` contains the full allowlist check: `label.chars().all(|c| c.is_ascii_alphanumeric() \|\| c == '-' \|\| c == '_')` per INVARIANTS.md INV-12 line 147 | INVARIANTS.md line 147 reading + `grep -n "is_ascii_alphanumeric\|chars().all" src/launchd.rs` | `[verified]` — Architect Read INVARIANTS.md INV-12 prose verbatim | ✅ INV-12 line 147 confirms |
| 4 | `src/launchd.rs` exports a helper named `is_valid_label` (or similar) per Sếp brief mention. Worker confirms exact name + location (`launchd.rs` or `mcp/tools.rs` or both) — Sếp brief mentions `validate_label` in `mcp/tools.rs` (INV-18) + `is_valid_label` in `core/register.rs` (P006). Architect does NOT mandate reuse — Worker chooses simplest path. | `grep -rn "fn is_valid_label\|fn validate_label" src/` | `[needs Worker verify]` | ⏳ TO VERIFY |
| 5 | `.git/hooks/pre-commit` line 137 contains `grep -q "## .*$PHIEU_ID" docs/DISCOVERIES.md 2>/dev/null` — H2-only pattern. Hook script lives at `.git/hooks/pre-commit` (in-repo). | Direct Read by Architect of `.git/hooks/pre-commit` | `[verified]` — Architect Read the file end-to-end | ✅ Confirmed at line 137 |
| 6 | `docs/DISCOVERIES.md` contains BOTH H2 header (`## P<NNN> — ... shipped`) AND list-item (`- 2026-MM-DD P<NNN>: ...`) entries for each phiếu P001-P010 (Worker dual-format mitigation). New hook regex MUST keep matching the existing H2 form so historical entries don't break the hook. | Direct Read by Architect of `docs/DISCOVERIES.md` (lines 9-47) | `[verified]` — Architect Read the file | ✅ Both formats present per phiếu — confirmed P001-P010 |
| 7 | `INVARIANTS.md` INV-12 spec (line 137-153) says enforcement at 2 points: (1) pre-flight in `register::run`, (2) inside `generate_plist`. Line 147 implementation prose references `cli/register.rs::run_with_deps` (pre-P006 location, stale). | Direct Read by Architect of INVARIANTS.md INV-12 block | `[verified]` — full block read | ✅ Confirmed, prose stale on location |
| 8 | `INVARIANTS.md` INV-18 spec (line 250-266) covers MCP boundary `validate_label` — a THIRD enforcement point (CLI pre-flight + `generate_plist` + MCP boundary). P011 only touches point 1 (core::register pre-flight); does NOT touch MCP boundary. | Direct Read by Architect of INVARIANTS.md INV-18 block | `[verified]` | ✅ Confirmed |
| 9 | Existing `src/core/register.rs::tests` (or `src/cli/register.rs::tests`, if P006 left tests at the CLI layer) has a test scaffold using `NoopLaunchctl` — Worker re-uses for invalid-label rejection test. ARCHITECTURE.md §Modules row mentions `RealLaunchctl`/`NoopLaunchctl` impls (line 54). | `grep -rn "NoopLaunchctl" src/` to locate; `grep -n "mod tests" src/core/register.rs` to confirm test module exists | `[needs Worker verify]` | ⏳ TO VERIFY |
| 10 | Baseline test count post-P010 = 141 tests. P011 adds 2-4 new tests (invalid-label rejection cases × register + maybe unregister). Final expected: ≥143 tests. | Sếp brief "141 tests" + CHANGELOG P010 line 42 ("133 → 141 net") | `[verified]` per CHANGELOG | ✅ 141 baseline confirmed |
| 11 | `Cargo.toml` has no new dep needed — INV-12 allowlist check uses only `char::is_ascii_alphanumeric` from `std`. Hook script change is bash/grep only. | Architect reasoning (no external crate need); Worker confirms by `git diff Cargo.toml` empty | `[verified]` — std-only | ✅ No new dep |
| 12 | `BACKLOG.md` "Open backlog" section currently has exactly 3 debt items (line 53-57 of BACKLOG.md). P011 ships → 2 move to "Recently shipped", 1 stays. | Direct Read by Architect of BACKLOG.md "Open backlog" section | `[verified]` — file Read | ✅ Confirmed: items at lines 55, 56, 57 |
| 13 | `BACKLOG.md` "Recently shipped" section exists at line 67-69 (currently empty per `(empty until Phase 1 ships)`). With P001-P010 sprint summary now in CHANGELOG, the BACKLOG "Recently shipped" should add a P001-P010 sprint summary line + P011 debt-cleanup line (or, more conservatively, just add the 2 debt items as moved). Worker chooses succinct form. | Direct Read by Architect of BACKLOG.md | `[verified]` — Recently shipped currently empty | ✅ Confirmed |
| 14 | `.phieu-counter` file currently contains `011` (Architect bumped to 011 before this phiếu drafted; Sếp brief notes "Counter bumped 010 → 011"). | Direct Read by Architect | `[verified]` — read confirmed `011` | ✅ Confirmed |
| 15 | The hook script uses bash, has `set -uo pipefail` (line 12), uses `grep -q` without `-E` at line 137 — changing to `grep -Eq` for the new alternation pattern is a 1-flag change (no other line breaks). | Direct Read by Architect of hook lines 12 + 137 | `[verified]` | ✅ Confirmed |
| 16 | The hook script `.git/hooks/pre-commit` is what Git executes on commit in THIS repo (path `.git/hooks/pre-commit` is the default Git hook location; `core.hooksPath` is NOT overridden — confirmed because `git config core.hooksPath` returns nothing per common Git setup, AND the hook is present at the default location). Worker confirms `git config --get core.hooksPath` returns empty (or value matches `.git/hooks`). | Worker runs `git config --get core.hooksPath` — expected empty/unset | `[needs Worker verify]` | ⏳ TO VERIFY |

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND). Cap = 3 turns.
> **Tầng 2 phiếu — CHALLENGE skipped per ORCHESTRATION rule (Tầng 2 routes Architect → Approval → EXECUTE directly).**

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge
*(Tầng 2 — CHALLENGE skipped per ORCHESTRATION rule. If Worker discovers code-reality mismatch during Task 0 anchor verification, escalate by appending objection here and pause for Architect RESPOND.)*

**Status:** ⏭️ SKIPPED (Tầng 2 routing)

### Final consensus
- Phiếu version: V1
- Total turns: 0 (Tầng 2 — CHALLENGE bypassed)
- Approved (autonomous mode — Sếp brief authorized autonomous): 2026-05-27 — code execution may begin after Worker Task 0 anchor verification passes

---

## Debug Log (advisory-cron specific)

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | N/A — no new launchd plist; pre-flight check runs inside existing `core::register::run` call path | — | | N/A |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test --lib register` (new invalid-label rejection unit tests) | new tests pass + all existing register tests pass | | |
| B (capability) | `cargo test --lib unregister` (if unregister.rs changed) | new tests pass + all existing unregister tests pass | | |
| B (capability) | `cargo test --all` | ≥143 tests pass (141 baseline + new invalid-label tests) | | |
| B (capability) | `bash -n .git/hooks/pre-commit` (syntax check) | exit 0 | | |
| B (capability) | Manual hook dry-run with synthetic list-item-only DISCOVERIES.md entry — see Task 4 Lưu ý | hook accepts list-item form | | |
| C (migration) | N/A — no schema change | — | | N/A |
| D (persistence) | `grep -l "INV-12" docs/security/INVARIANTS.md` | ≥1 hit (spec unchanged) | | |
| D (persistence) | `grep -l "P011" docs/CHANGELOG.md docs/DISCOVERIES.md` | ≥1 hit each | | |
| D (persistence) | `grep -c "label.chars().all\|is_ascii_alphanumeric" src/core/register.rs` | ≥1 hit (new allowlist check present) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (P011 adds zero deps) | | |
| E (env drift) | `cargo build --release` clean target | exit 0, binary ≤7MB (no size delta — std-only addition) | | |

---

## Nhiệm vụ

### Task 0 — Anchor verification (BẮT BUỘC TRƯỚC mọi Task khác)

**Mục đích:** Architect không có Bash/Grep — anchor table dựa docs + direct Read of `.git/hooks/pre-commit`. Worker grep-verifies the source-side anchors (#1, #2, #4, #9, #16) before touching code.

**Lệnh chạy:**

```bash
# Anchor #1 — core::register::run pre-flight shape
grep -n "pub fn run\|pub async fn run" src/core/register.rs
grep -n "is_empty\|empty()" src/core/register.rs
grep -n "is_ascii_alphanumeric\|chars().all" src/core/register.rs   # expect ZERO hits BEFORE this phiếu

# Anchor #2 — core::unregister::run pre-flight shape (UNKNOWN — Worker reports actual)
grep -n "pub fn run\|pub async fn run" src/core/unregister.rs
grep -n "is_empty\|empty()\|is_ascii_alphanumeric\|chars().all" src/core/unregister.rs

# Anchor #4 — existing label validator helpers (Worker decides reuse vs duplicate)
grep -rn "fn is_valid_label\|fn validate_label" src/

# Anchor #9 — NoopLaunchctl test scaffolding (Worker reuses for invalid-label tests)
grep -rn "NoopLaunchctl" src/

# Anchor #16 — confirm Git uses .git/hooks/ in this repo
git config --get core.hooksPath || echo "(unset — default .git/hooks/)"

# Sanity recap of anchors #3, #5, #6, #7, #11 (Architect-verified, Worker double-check)
grep -n "is_ascii_alphanumeric\|chars().all" src/launchd.rs                  # #3
sed -n "135,140p" .git/hooks/pre-commit                                       # #5 — confirm pattern at line 137 unchanged
sed -n "53,58p" docs/BACKLOG.md                                               # #12 — confirm 3 debt items in Open backlog
```

**Output:** fill Verification Anchors table → if anchors ✅ → proceed Task 1. If ⚠️/❌ → write Debate Log Turn 1 objection (Architect RESPOND mode required).

**Special focus for Worker:**

- **Anchor #1:** if `core::register::run` ALREADY has a full allowlist check (Sếp brief may be stale — P006 worker could have added it without doc trail), Task 1 is a NO-OP — STOP and report; entire phiếu's Item 1 may be obsolete. Architect expects empty-check-only state per Sếp brief.
- **Anchor #2:** Worker REPORTS the actual current state of `unregister::run` pre-flight in Discovery Report. Three possible outcomes:
  1. Empty-check only → apply Task 2 (parallel to Task 1).
  2. Full allowlist already → skip Task 2, log as already-correct in Discovery.
  3. No pre-flight at all (e.g. unregister doesn't take a label arg through validation) → confirm via Worker's grep; if confirmed, Task 2 is skipped + Discovery notes.
- **Anchor #4:** if `is_valid_label` or `validate_label` already exists in `launchd.rs` or `mcp/tools.rs`, Worker MAY re-use (export + import). If not, Worker MAY define a new local private predicate in `src/core/register.rs`. Both approaches acceptable — Worker chooses the lower-LOC option per Tầng 2 minimalism.
- **Anchor #16:** if `git config --get core.hooksPath` returns a non-default value (unlikely but possible), the hook at `.git/hooks/pre-commit` may NOT be the active one. STOP and escalate — Architect did not anticipate hooksPath override.

---

### Task 1: Add full ASCII allowlist label check in `core::register::run` pre-flight

**File:** `src/core/register.rs`

**Tìm:** Worker uses Task 0 Anchor #1 results to locate the pre-flight block in `pub fn run` (or `pub async fn run`). Current shape per Sếp brief: a single `if args.label.is_empty() { return Err(...) }` (or equivalent `.bail!`/`.context()`-style early return) BEFORE any call into `generate_plist` or the `LaunchctlClient`.

**Thay bằng / Thêm:** Replace the empty-only check with a combined empty-or-disallowed-char check. Two acceptable implementation shapes — Worker picks one matching existing `register.rs` error idiom:

**Option A — single combined check, inline:**
```rust
// Before any plist generation or launchctl call:
if args.label.is_empty()
    || !args.label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
{
    anyhow::bail!(
        "invalid label `{}`: must be non-empty ASCII alphanumeric + `-` + `_` only (INV-12)",
        args.label
    );
}
```

**Option B — private helper + call:**
```rust
fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// In `run`:
if !is_valid_label(&args.label) {
    anyhow::bail!(
        "invalid label `{}`: must be non-empty ASCII alphanumeric + `-` + `_` only (INV-12)",
        args.label
    );
}
```

**Worker decision:** if Anchor #4 finds an existing `is_valid_label` / `validate_label` already in scope (e.g. from `launchd.rs` or `mcp/tools.rs`), prefer to **re-use** that helper rather than duplicate the predicate — define a pub(crate) re-export if needed. If no existing helper, Option A or Option B both fine. Pick the one with lower delta.

**Lưu ý:**

- **Predicate MUST exactly mirror `generate_plist`'s check** in `src/launchd.rs` (per Anchor #3) — same allowlist character set (`is_ascii_alphanumeric` + `-` + `_`), no Unicode allowance, no additional permitted chars. If `generate_plist` and `register::run` diverge, defense-in-depth is broken differently in each layer.
- **Error must be raised BEFORE invoking `&L: LaunchctlClient`** (so `NoopLaunchctl` test won't see a `.bootstrap` call when the label is invalid). This is the testable property — see Task 5.
- **Error message wording** is Worker's call as long as it: (a) names the invalid label, (b) cites INV-12. Architect's snippet above is suggestive, not mandatory. Match existing `register.rs` error-msg style if it differs.
- **`anyhow::bail!` vs `return Err(anyhow::anyhow!(...))`** — pick whichever pattern is used by the existing empty-check error. Both work.
- **No new file**, no new public surface — predicate is private or pub(crate). Hard Stop #1 honored.
- **No `unsafe { }`** — entire predicate is safe Rust. Hard Stop #7 honored.

---

### Task 2: (Conditional) Apply parallel allowlist check in `core::unregister::run` pre-flight

**File:** `src/core/unregister.rs`

**Run condition:** Apply this task ONLY IF Task 0 Anchor #2 reveals that `unregister::run` pre-flight currently performs an empty-check-only (or weaker) validation on `args.label`. If the current state already has a full allowlist check, OR if there is no label validation at all because unregister doesn't validate label at the pre-flight layer (e.g. it relies on `generate_plist`-equivalent or `launchctl` failure for invalid labels), **skip this task** and log Anchor #2's findings in Discovery Report.

**Tìm:** the pre-flight block in `pub fn run` / `pub async fn run` of `src/core/unregister.rs` — parallel to Task 1's target.

**Thay bằng / Thêm:** Same pattern as Task 1 (Option A or Option B). Same predicate, same error wording style. If a shared helper was used in Task 1, reuse here too.

**Lưu ý:**

- Worker reports decision in Discovery Report: "Task 2 applied" / "Task 2 skipped — unregister already correct" / "Task 2 skipped — unregister has no label pre-flight by design (Tầng 2 stylistic, escalate to BACKLOG if symmetry desired)."
- If Task 2 IS applied, Worker MUST also add at least 1 unit test asserting invalid-label rejection in `core::unregister::run` (parallel to Task 5). If Task 2 is skipped, no unregister test addition.
- This task is **NOT** a Hard Stop — Worker self-decides skip/apply based on Anchor #2 evidence. Document in Discovery either way.

---

### Task 3: Update `.git/hooks/pre-commit` DISCOVERIES grep pattern

**File:** `.git/hooks/pre-commit`

**Tìm:** Line 137 (per Architect Anchor #5). Current code (verbatim):
```bash
            if ! grep -q "## .*$PHIEU_ID" docs/DISCOVERIES.md 2>/dev/null; then
```

**Thay bằng:**
```bash
            # P011 — accept either legacy H2 form (## ...P<NNN>) OR CLAUDE.md doctrine
            # list-item form (- YYYY-MM-DD P<NNN>:). Going forward only the list-item
            # form is required; H2 kept for backwards-compat with P001-P010 entries.
            if ! grep -Eq "(^## .*${PHIEU_ID}([^0-9]|$)|^- [0-9]{4}-[0-9]{2}-[0-9]{2} ${PHIEU_ID}:)" docs/DISCOVERIES.md 2>/dev/null; then
```

**Lưu ý:**

- **`grep -Eq` (extended regex) is required** because the new pattern uses `[0-9]{4}` quantifier and `|` alternation. Without `-E`, those would be treated literally and the regex would never match.
- **The `([^0-9]|$)` negative-class boundary on the H2 alt** prevents `P011` from accidentally matching against a hypothetical `## P0110` h2 header. Without it, the regex would say "any character" and `P0110` would match `P011`. Important even though no such P0110 exists today — defensive.
- **The list-item alt `^- [0-9]{4}-[0-9]{2}-[0-9]{2} ${PHIEU_ID}:`** anchors on (a) line-start `-`, (b) ISO-date prefix, (c) phiếu ID, (d) trailing `:`. This is exactly the CLAUDE.md doctrine line shape (CLAUDE.md §DISCOVERY REPORT step 2).
- **`grep -Eq` returns 0 on match, 1 on no-match.** The surrounding `if ! ...` logic is unchanged. Worker confirms the exit-on-fail path (lines 138-139 of current hook) still increments `FAIL_COUNT` correctly.
- **Bash `set -uo pipefail`** at line 12 — `${PHIEU_ID}` (already braced in current code) is safe; the new regex uses `${PHIEU_ID}` so no shellcheck warning.
- **Test the new regex against the actual current `docs/DISCOVERIES.md`:**
  - Each of P001-P010 should match via the H2 alternative (lines 9, 13, 17, 21, 25, 29, 33, 37, 41, 45 of DISCOVERIES.md have `## P<NNN>` headers).
  - Each of P001-P010 should ALSO match via the list-item alternative (lines 11, 15, 19, 23, 27, 31, 35, 39, 43, 47 have `- 2026-05-27 P<NNN>:` items).
  - The P011 entry Worker will append will match via the list-item alternative only (Worker MAY also add an H2 header for cosmetic consistency with P001-P010, but doctrine going forward is list-item only — Worker's call, Tầng 2).
- **Manual dry-run validation** (Worker runs locally — Sub-mechanism B):
  ```bash
  # 1. Confirm new pattern matches all existing P001-P010 entries:
  for n in 001 002 003 004 005 006 007 008 009 010; do
      PHIEU_ID="P$n"
      grep -Eq "(^## .*${PHIEU_ID}([^0-9]|$)|^- [0-9]{4}-[0-9]{2}-[0-9]{2} ${PHIEU_ID}:)" docs/DISCOVERIES.md && echo "$PHIEU_ID ✅" || echo "$PHIEU_ID ❌"
  done
  # Expect: 10 ✅ lines.

  # 2. Confirm new pattern matches P011 ONCE Worker has added the P011 list-item line to docs/DISCOVERIES.md:
  #    (defer this check until Task 6 lands the P011 entry)

  # 3. Confirm new pattern does NOT match a phiếu ID with no entry:
  PHIEU_ID="P999" grep -Eq "(^## .*${PHIEU_ID}([^0-9]|$)|^- [0-9]{4}-[0-9]{2}-[0-9]{2} ${PHIEU_ID}:)" docs/DISCOVERIES.md && echo "FALSE POSITIVE ❌" || echo "P999 correctly no-match ✅"
  ```
- **No other line of `.git/hooks/pre-commit` is touched.** The change is line 137 (regex pattern + 3 new comment lines just above it). Worker does NOT touch the type-check section (lines 25-64), docs-gate section (lines 68-90), or any other v2 check.
- **The system-wide kit copy at `/Users/nguyenhuuanh/sos-kit/hooks/pre-commit` is OUT OF SCOPE** — sos-kit doctrine repo, not advisory-cron. Worker MUST NOT edit it (would be cross-repo creep).

---

### Task 4: Add unit tests for invalid-label rejection at pre-flight

**File:** `src/core/register.rs` (and `src/core/unregister.rs` IF Task 2 applied)

**Tìm:** Worker locates the existing `#[cfg(test)] mod tests { ... }` block at the bottom of `src/core/register.rs` per Anchor #9. The existing test scaffold uses `NoopLaunchctl` (or equivalent test-double) for `LaunchctlClient` injection.

**Thay bằng / Thêm:** Add at least 2 new tests (3 recommended) covering the rejected-label cases. Suggested test shapes:

```rust
#[tokio::test]  // (or #[test] if `run` is not async — Worker matches existing)
async fn register_rejects_label_with_whitespace() {
    let launchctl = NoopLaunchctl::default();
    let args = RegisterArgs {
        label: "foo bar".to_string(),  // space disallowed by INV-12
        ../* fill other required fields */
    };
    let result = run(args, &launchctl).await;
    assert!(result.is_err(), "label with whitespace must be rejected at pre-flight");
    // Verify NoopLaunchctl recorded ZERO bootstrap calls (pre-flight rejected before invocation)
    assert_eq!(launchctl.bootstrap_calls(), 0,
        "pre-flight rejection must occur before LaunchctlClient invocation");
}

#[tokio::test]
async fn register_rejects_label_with_path_separator() {
    let launchctl = NoopLaunchctl::default();
    let args = RegisterArgs { label: "foo/bar".to_string(), ..Default::default() };
    let result = run(args, &launchctl).await;
    assert!(result.is_err(), "label with `/` must be rejected (path traversal vector per INV-12)");
    assert_eq!(launchctl.bootstrap_calls(), 0);
}

#[tokio::test]
async fn register_rejects_label_with_shell_metachar() {
    let launchctl = NoopLaunchctl::default();
    let args = RegisterArgs { label: "foo;rm".to_string(), ..Default::default() };
    let result = run(args, &launchctl).await;
    assert!(result.is_err(), "label with `;` must be rejected (shell metachar per INV-12)");
    assert_eq!(launchctl.bootstrap_calls(), 0);
}

#[tokio::test]
async fn register_accepts_valid_label_unchanged() {
    let launchctl = NoopLaunchctl::default();
    let args = RegisterArgs { label: "advisory-scan-daily".to_string(), ..Default::default() };
    let result = run(args, &launchctl).await;
    // Pre-flight must NOT reject — downstream may still fail in test env (e.g. missing
    // config file), so we only assert the pre-flight DIDN'T cause the failure.
    // Look at NoopLaunchctl recorded calls or assert the error type is NOT a pre-flight error.
    // Worker adapts to existing register-test idiom.
}
```

**Lưu ý:**

- **`NoopLaunchctl::default()` + `bootstrap_calls()` accessor** is illustrative — Worker uses whatever inspection API the existing `NoopLaunchctl` in `src/launchd.rs` provides (Anchor #9). If the existing scaffold has different inspection hooks, adapt.
- **`RegisterArgs { ..Default::default() }`** assumes `RegisterArgs: Default` — Worker confirms via Task 0; if not derived, Worker constructs full struct literal matching existing test pattern.
- **`#[tokio::test]` vs `#[test]`** — Worker matches existing tests in `register.rs::tests` (depends on whether `register::run` is async). Per ARCHITECTURE.md §Modules row line 46, `register` is not annotated async — but P010 added async to `core::run::run`; Worker grep-verifies and matches.
- **Test names follow existing snake_case convention** (`register_<verb>_<condition>`). Worker matches existing register-test naming style.
- **Don't add a redundant "accepts valid" test** if existing register tests already exercise the happy path with a valid label — Tầng 2 minimalism. Add only enough tests to cover the 3 invalid-input classes (whitespace, path separator, shell metachar). One representative per class.
- **If Task 2 applied (unregister also changed),** add at least 1 parallel `unregister_rejects_invalid_label` test in `src/core/unregister.rs::tests`. Don't duplicate the full 3-class matrix — 1 test (e.g. whitespace case) is sufficient since the predicate is the same.
- **No integration test additions in `tests/` dir** — unit tests in `src/core/*::tests` are enough for this Tầng 2 phiếu. Tests scope-creep is a Hard Stop.

---

### Task 5: Write Discovery Report

**File:** `docs/discoveries/P011.md` (NEW) + 1-line index in `docs/DISCOVERIES.md`

**Tìm:** N/A — new file + append to existing.

**Thay bằng / Thêm:** New file `docs/discoveries/P011.md` per CLAUDE.md DISCOVERY REPORT format:

```markdown
## Discovery Report — P011

### Assumptions trong phiếu — ĐÚNG:
- [Each anchor that verified ✅ — list with file:line citation]

### Assumptions trong phiếu — SAI so với code thật:
- [Anchor #2: phiếu assumed unregister pre-flight is empty-check-only;
  actual state was X → Task 2 (applied / skipped / partially applied)]
- [Anchor #N: ...]
- [If no mismatches: "Không có"]

### Edge cases / limitations phát hiện thêm:
- [INVARIANTS.md INV-12 line 147 prose references stale pre-P006 location
  `cli/register.rs::run_with_deps`; Worker (updated to `core/register.rs::run`
  / left as-is and logged as known stale prose)]
- [hook regex edge case: ...]
- [Anything else]

### Docs đã cập nhật theo discoveries:
- [If INVARIANTS.md line 147 prose updated: list it]
- [BACKLOG.md: moved 2 items "Open backlog" → "Recently shipped"]
- [CHANGELOG.md: single P011 entry]
```

And append to `docs/DISCOVERIES.md` (newest at top, **CLAUDE.md doctrine list-item form going forward — Worker MAY also include an H2 header for visual parity with P001-P010, fully optional**):

```markdown
## P011 — Sprint debt cleanup (INV-12 2-point + DISCOVERIES hook align) shipped 2026-05-27

- 2026-05-27 P011: Sprint debt cleanup shipped (INV-12 label sanitization pre-flight added in core::register::run [+ core::unregister::run IF applied]; .git/hooks/pre-commit DISCOVERIES regex aligned with CLAUDE.md doctrine list-item form, legacy H2 form still accepted; +N tests; 2 BACKLOG debt items moved to Recently shipped; item 3 fire_task no-timeout deferred to separate Tầng 1 phiếu; no INV/schema/dep change) → see docs/discoveries/P011.md
```

**Lưu ý:**

- **Mandatory items to log in Discovery (per Sếp brief + Architect anchors):**
  1. Whether Task 2 was applied (unregister) — actual current state of `unregister::run` pre-flight per Anchor #2.
  2. Whether INVARIANTS.md line 147 stale-prose location reference was updated (Tầng 2 freebie) or left.
  3. Whether Worker reused an existing `is_valid_label` / `validate_label` helper (Anchor #4) or added a new one.
  4. Hook script test outcomes — did the new regex match P001-P010 cleanly? Any unexpected matches?
  5. Final test count (baseline 141 → final N).
- **The CLAUDE.md doctrine line is the AUTHORITATIVE format going forward.** Worker writes the list-item; the H2 header is optional cosmetic carryover.

---

### Task 6: Update CHANGELOG.md + BACKLOG.md

**File:** `docs/CHANGELOG.md` + `docs/BACKLOG.md`

**Tìm:** Top of CHANGELOG (newest at top) + BACKLOG "Open backlog" + "Recently shipped" sections.

**Thay bằng / Thêm:**

**`docs/CHANGELOG.md` — prepend P011 entry above the existing P010 entry:**

```markdown
## 2026-05-27 — P011: Sprint debt cleanup (Tầng 2 — INV-12 + DISCOVERIES hook align)

**Phiếu:** P011 (Tầng 2 — 2 items from "Open backlog" cleared; item 3 `fire_task` no-timeout deferred to separate Tầng 1 phiếu)

**Item 1 — INV-12 label sanitization 2-point enforcement restored:**
- `src/core/register.rs::run` pre-flight extended from empty-only check to full ASCII alphanumeric + `-` + `_` allowlist (mirrors `generate_plist` in `src/launchd.rs` per INV-12 line 147).
- `src/core/unregister.rs::run` pre-flight: [applied parallel check / already correct — no change / N/A — no label pre-flight present] (Worker fills per Discovery).
- Defense-in-depth restored to 2 points per INV-12 spec; MCP boundary (INV-18) remains a third independent enforcement point.
- New unit tests in `src/core/register.rs::tests` (and unregister.rs::tests if applied) — invalid-label rejection asserted BEFORE `LaunchctlClient` invocation.

**Item 2 — DISCOVERIES.md hook format aligned with CLAUDE.md doctrine:**
- `.git/hooks/pre-commit` line 137 regex updated: now accepts either legacy H2 header (`## ...P<NNN>`) OR CLAUDE.md doctrine list-item (`- YYYY-MM-DD P<NNN>:`). `grep -q` → `grep -Eq` (extended regex required for alternation).
- Going forward Worker writes only the list-item form; existing P001-P010 dual-format entries continue to match via the legacy alternative.
- Worker no longer needs to write both formats (1 source of truth per CLAUDE.md doctrine).

**Tests:**
- Baseline 141 → final N (Worker fills exact count). +2-4 new invalid-label rejection tests.

**No INV change, no schema change, no dep change.**
- INV-12 spec at INVARIANTS.md line 137-153 unchanged — P011 just enforces what's already there.
- (Optionally — Worker logs in Discovery) INVARIANTS.md line 147 stale prose location reference (`cli/register.rs::run_with_deps` pre-P006) updated to `core/register.rs::run` post-P006 OR left as-is and logged.

**BACKLOG.md:** 2 debt items moved "Open backlog" → "Recently shipped"; item 3 (`fire_task` no process timeout) stays in "Open backlog" (deferred to its own Tầng 1 phiếu when picked).

**Acceptance (verified):**
- `cargo build --release` — zero warnings
- `cargo test --all` — all pass (≥143 tests)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `bash -n .git/hooks/pre-commit` — exit 0 (syntax valid)
- `grep -c "label.chars().all\|is_ascii_alphanumeric" src/core/register.rs` — ≥1 hit
- `git diff src/launchd.rs` — empty (KHÔNG sửa)
- `git diff src/cli/mod.rs` — empty (Constraint #1)
- `git diff src/mcp/tools.rs` — empty (KHÔNG sửa — INV-18 MCP boundary untouched)
- `git diff docs/security/INVARIANTS.md` — empty OR ≤1 line (the optional INV-12 line 147 prose location reference update)
- `git diff docs/ARCHITECTURE.md` — empty (no module/CLI/exit-code change)
- `git diff Cargo.toml` — empty (no dep change)

---

```

**`docs/BACKLOG.md`** — make these edits in place:

1. In `## 💡 Open backlog (chưa thuộc sprint)` section (lines 53-57): DELETE the first 2 items (INV-12 label sanitization, DISCOVERIES.md hook vs CLAUDE.md format mismatch). LEAVE item 3 (`fire_task` no process timeout) in place.

2. In `## ✅ Recently shipped` section (lines 67-69): REPLACE `(empty until Phase 1 ships)` with:

```markdown
- **2026-05-27 — Sprint Phase 1 + Phase 2 (P001-P010) shipped.** 10 phiếu over the sprint. CLI scaffold → config schema → launchd plist → task runner + heartbeat → status reporter → MCP server wrapper → README/ARCHITECTURE polish → Telegram alert → retry policy → crash-safe heartbeat. 141 tests passing, 22 modules, 21 INVs, single-binary ≤7MB. See CHANGELOG.md sprint summary line 65+.
- **2026-05-27 — P011: Sprint debt cleanup (Tầng 2).** INV-12 label sanitization 2-point enforcement restored in `core::register::run` [and `core::unregister::run` IF applied]; `.git/hooks/pre-commit` DISCOVERIES grep aligned with CLAUDE.md doctrine list-item format. Item 3 (`fire_task` no process timeout) stays deferred.
```

**Lưu ý:**

- **Worker MAY consolidate the sprint-summary entry into a single line** if it feels verbose — Architect's draft is suggestive. Match the existing BACKLOG.md tone (concise bullet form).
- **Item 3 of "Open backlog" MUST stay in place** (the `fire_task` no process timeout entry). It's the Tầng 1 separate-phiếu candidate. Worker confirms it remains by grepping `grep -c "fire_task no process timeout" docs/BACKLOG.md` after edit → expected 1.
- **No re-numbering of remaining Open backlog items** — bullet list, no numbers to update.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/core/register.rs` | Task 1: pre-flight allowlist check (replace empty-only). Task 4: invalid-label rejection unit tests. |
| `src/core/unregister.rs` | Task 2 (conditional, per Anchor #2): parallel allowlist check + 1 unit test. |
| `.git/hooks/pre-commit` | Task 3: line 137 regex updated to accept CLAUDE.md doctrine list-item format (and keep legacy H2). |
| `docs/CHANGELOG.md` | Task 6: prepend P011 entry. |
| `docs/BACKLOG.md` | Task 6: delete 2 debt items from "Open backlog"; populate "Recently shipped" with sprint + P011 entries. |
| `docs/DISCOVERIES.md` | Task 5: append P011 list-item (CLAUDE.md doctrine) — H2 header optional. |
| `docs/discoveries/P011.md` | Task 5: new file, full Discovery Report. |
| `docs/security/INVARIANTS.md` | OPTIONAL — Worker may update INV-12 line 147 stale prose location reference (`cli/register.rs::run_with_deps` → `core/register.rs::run`). If skipped, log in Discovery. |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/launchd.rs` | `generate_plist` allowlist check unchanged (point 2 of INV-12). `git diff src/launchd.rs` empty. |
| `src/mcp/tools.rs` | INV-18 MCP `validate_label` unchanged (point 3 of label sanitization). `git diff src/mcp/tools.rs` empty. |
| `src/cli/mod.rs` | Constraint #1 — dispatch unchanged post-P006. `git diff src/cli/mod.rs` empty. |
| `src/cli/register.rs`, `src/cli/unregister.rs` | thin shells route to `core::*` unchanged. `git diff` empty for both. |
| `src/alert.rs` | Constraint #11 — env-free alert module preserved. `git diff src/alert.rs` empty. |
| `src/heartbeat.rs` | P010 atomic protocol untouched. `git diff src/heartbeat.rs` empty. |
| `src/runner.rs` | P009 Constraint #5 — single-fire primitive preserved. `git diff src/runner.rs` empty. |
| `src/core/run.rs` | Heartbeat/retry/alert path untouched. `git diff src/core/run.rs` empty. |
| `src/core/status.rs` | Status path untouched. `git diff src/core/status.rs` empty. |
| `src/core/init.rs`, `src/core/mod.rs`, `src/core/config_path.rs` | All untouched. `git diff` empty. |
| `src/main.rs`, `src/cli/init.rs`, `src/cli/run.rs`, `src/cli/status.rs`, `src/cli/mcp.rs` | All untouched. `git diff` empty. |
| `src/config.rs` | Schema unchanged (no new fields). `git diff src/config.rs` empty. |
| `Cargo.toml` | No dep add/remove. `git diff Cargo.toml` empty. |
| `docs/ARCHITECTURE.md` | No module/CLI/exit-code/config-schema change. `git diff docs/ARCHITECTURE.md` empty. |
| `README.md` | No CLI surface change. `git diff README.md` empty. |
| `/Users/nguyenhuuanh/sos-kit/hooks/pre-commit` | System-wide kit hook out of scope. Worker MUST NOT edit (cross-repo creep). |

---

## Luật chơi (Constraints)

1. **Constraint #1 re-instated (post-P006):** `src/cli/mod.rs` untouched. `git diff src/cli/mod.rs` empty. Register/unregister handlers route to `core::*` unchanged — pre-flight added in `core::*` only.

2. **Constraint #4 re-instated:** `core::*` env-internal honored — no new `std::env::var` reads added. Pre-flight allowlist uses only `&str` operations (`char::is_ascii_alphanumeric`).

3. **Constraint #11 re-instated:** `src/alert.rs` env-free property preserved. `git diff src/alert.rs` empty.

4. **Constraint #12 re-instated (from P009):** `heartbeat::append` signature + caller surface unchanged. `git diff src/heartbeat.rs` empty.

5. **No new dependency.** `git diff Cargo.toml` empty. Allowlist predicate uses only std (`char::is_ascii_alphanumeric`). Hard Stop #2 honored.

6. **No CLI interface change.** No new subcommand, no new flag, no exit code semantic change. Hard Stop #3 honored.

7. **No config schema change.** No new field in `Config` / any sub-struct. `git diff src/config.rs` empty. Hard Stop #4 honored.

8. **No `unsafe { }`.** Hard Stop #7 honored.

9. **Predicate exact-match with `generate_plist`.** The allowlist character set in `core::register::run` (and `core::unregister::run` IF applied) MUST be identical to the one in `src/launchd.rs::generate_plist` (per Anchor #3) — `is_ascii_alphanumeric` + `-` + `_`. If they diverge, defense-in-depth is broken differently in each layer. Worker confirms by grep-comparing the two predicates side-by-side.

10. **Pre-flight before launchctl invocation.** The new check MUST execute BEFORE any call to `&L: LaunchctlClient` method. The test in Task 4 asserting `bootstrap_calls() == 0` on invalid label proves this.

11. **Hook regex must be backward-compat with P001-P010 entries.** The legacy H2 alternative in the new regex MUST keep matching the existing `## P<NNN>` headers in `docs/DISCOVERIES.md` (Architect verified per Anchor #6). If a Worker test reveals any of P001-P010 entries no longer match, STOP — regex is wrong.

12. **Item 3 of "Open backlog" (fire_task no timeout) stays unchanged.** That's the deferred Tầng 1 item. Worker confirms after BACKLOG edit by `grep -c "fire_task no process timeout" docs/BACKLOG.md` = 1.

13. **No INV change.** `git diff docs/security/INVARIANTS.md` either empty OR ≤1 line (the optional line 147 prose location reference update). No new INV added. INV-12 / INV-17 / INV-18 specs all unchanged.

14. **No ARCHITECTURE change.** `git diff docs/ARCHITECTURE.md` empty. Tầng 2 declaration — no module/CLI/config/exit-code/schema change.

15. **No system-wide hook edit.** `/Users/nguyenhuuanh/sos-kit/hooks/pre-commit` is sos-kit doctrine repo — Worker MUST NOT touch. Edit only the project copy at `.git/hooks/pre-commit`.

16. **DISCOVERIES going-forward format:** CLAUDE.md doctrine list-item (`- YYYY-MM-DD P<NNN>: ... → see docs/discoveries/P<NNN>.md`). H2 header optional for P011 (Worker's call); MANDATORY from P012 onwards becomes list-item-only.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (baseline 141 + 2-4 new = ≥143)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `bash -n .git/hooks/pre-commit` — exit 0 (bash syntax check)

### Manual Testing
- [ ] **Invalid label rejection (register):** Worker runs `cargo test --lib register::tests::register_rejects_label_with_whitespace` → pass. Confirms label `"foo bar"` rejected at pre-flight (before LaunchctlClient call).
- [ ] **Invalid label rejection (path traversal):** test `register_rejects_label_with_path_separator` for `"foo/bar"` → pass.
- [ ] **Invalid label rejection (shell metachar):** test `register_rejects_label_with_shell_metachar` for `"foo;rm"` → pass.
- [ ] **Valid label still works:** existing register happy-path test (e.g. `register_writes_plist_and_bootstraps` or whatever the current name is) still passes with valid label `"test"` or `"advisory-scan-daily"`.
- [ ] **Hook regex matches all P001-P010:** Worker runs the shell loop in Task 3 Lưu ý → 10 ✅ lines.
- [ ] **Hook regex does NOT match a missing phiếu ID:** `PHIEU_ID="P999"` → no-match.
- [ ] **Dry-run actual commit:** Worker stages all P011 changes (code + tests + docs + DISCOVERIES entry + CHANGELOG entry + BACKLOG move + Discovery Report file), runs `git commit --dry-run` (or actual commit then `git reset --soft HEAD~1` if needed) → hook output shows all checks ✅ including the new DISCOVERIES regex match for P011.

### Regression
- [ ] `cargo test --lib --no-run` builds clean (no test-only changes broke compilation).
- [ ] `cargo test --test cli_run` (P004 integration) — still passes.
- [ ] `cargo test --test cli_run_retry` (P009 integration) — still passes.
- [ ] `cargo test --test cli_run_alert` (P008 integration) — still passes.
- [ ] `cargo test --test cli_run_crash_safe` (P010 integration, if present) — still passes.
- [ ] Existing valid-label register/unregister tests still pass — Worker confirms by `cargo test --lib register` (or wider scope).
- [ ] `cargo test --lib heartbeat` — still passes (heartbeat untouched).
- [ ] `cargo run --release -- status` (smoke — no flags) — still produces sensible output for default label (no panic).

### Docs Gate
- [ ] `docs/CHANGELOG.md` — single P011 entry prepended above P010.
- [ ] `docs/ARCHITECTURE.md` — `git diff` empty (Tầng 2 declaration, no edit needed).
- [ ] `README.md` — `git diff` empty (no CLI surface change).
- [ ] `docs/BACKLOG.md` — 2 items moved to "Recently shipped"; item 3 remains in "Open backlog". `grep -c "fire_task no process timeout" docs/BACKLOG.md` = 1.
- [ ] `docs/security/INVARIANTS.md` — `git diff` either empty OR ≤1 line (optional INV-12 line 147 prose update). If updated, Discovery logs it.
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P011.md` — full report written per Task 5.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry (CLAUDE.md doctrine list-item form) appended (newest at top). H2 header optional.
- [ ] Discovery explicitly answers all 5 mandatory items from Task 5 Lưu ý:
  1. Task 2 (unregister) applied / skipped / partially — actual state of `unregister::run` pre-flight.
  2. INVARIANTS.md line 147 prose update — done / skipped + log.
  3. `is_valid_label` helper reuse decision — reuse / new / inline.
  4. Hook regex test outcomes — P001-P010 match results.
  5. Final test count.
- [ ] Sub-mechanism A-E Verification Trace table filled (above).
