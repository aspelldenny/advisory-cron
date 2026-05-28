# PHIẾU P014: INV-22 + INV-23 formal entries + cross-OS CI matrix

> **Loại:** docs + infra
> **Tầng:** 1
> **Phase:** 3.3
> **Ưu tiên:** P1
> **Branch:** `feat/P014-inv22-23-ci-matrix`
> **Ảnh hưởng:** `docs/security/INVARIANTS.md` (+INV-22 +INV-23 — 2 new entries, ~120 LOC), `.github/workflows/ci.yml` (NEW FILE, ~70 LOC), `docs/ARCHITECTURE.md` (§Phase status 3.3 ⏸️ → ✅ + §CI matrix new subsection), `docs/CHANGELOG.md`
> **Dependency:** P012 + P013 merged (INV-22 enforcement code shipped P013; INV-23 doctrine formalises Phase 3.2 cron-form constraint already shipped in `src/core/register.rs::parse_daily_cron`)
> **Tier-1 reason:** Touches `docs/security/INVARIANTS.md` doctrine surface (2 new INV entries) + creates NEW CI workflow file (build-gate boundary). Per CLAUDE.md §HARD STOPS rule 1 (new file beyond phiếu scope) and §DOCS GATE Tầng 1 (security boundary doctrine = AUTO Tầng 1). Touches Sub-mechanism D (knowledge durability — INV doctrine lives in non-rotate-prone `docs/security/INVARIANTS.md`).

---

## Context

### Vấn đề hiện tại

P012 (Phase 3.1) extracted `Scheduler` trait; P013 (Phase 3.2) shipped Linux `CrontabScheduler` real impl including INV-22 enforcement (2-point label allowlist defense-in-depth) AND the daily-form-only cron constraint mirroring Phase 1 macOS launchd `StartCalendarInterval`. The **enforcement code shipped**; the **doctrine entries do not yet exist** in `docs/security/INVARIANTS.md`. The catalog still ends at INV-21 (`docs/security/INVARIANTS.md:318`).

Per CLAUDE.md "Sub-mechanism D — Persistence lifecycle gap": doctrine living only in shipped code + phiếu Discovery Reports = "effective lost" when a future agent's Boundary-check / new Worker reads `INVARIANTS.md` and sees only INV-1..21. P014 closes the doctrine gap.

Concurrently: Phase 3 BACKLOG acceptance criterion #8 (`cargo test --all` on Linux pass — cross-OS test matrix CI runner) requires a CI workflow with both macOS + Linux jobs. **No `.github/workflows/` directory exists yet** — Architect Glob `.github/**/*` returned 0 files (Anchor #1 below). P014 creates the workflow from scratch.

### Giải pháp

Three parallel tracks, all docs/infra only (zero src/ code change):

1. **INV-22 formal entry** — append to `docs/security/INVARIANTS.md` after INV-21. 5 sub-rules covering: (a) `crontab` shell-out via discrete `.arg()` only (no `sh -c` interpolation), (b) label allowlist 2-point (pre-flight in `core::*::run` + defense-in-depth inside `CrontabScheduler::*` first line via shared `super::is_valid_label`), (c) tag-only filter idempotency (`# advisory-cron: <label>` substring match — don't touch other user crontab lines), (d) sync `std::process::Command` only on the scheduler boundary (V2 P013 lesson — no nested-runtime async bridge from inside `#[tokio::main]`), (e) TOCTOU read-modify-write race acknowledged + deferred Phase 3.5+ (cross-reference `docs/ARCHITECTURE.md` §Cron mechanism Linux for full discussion). Parallel format to INV-10/12/17.

2. **INV-23 formal entry — Option A (conservative)** — append after INV-22. Documents the **daily form `M H * * *` cron expression invariant** for BOTH platforms in Phase 3:
   - macOS: native — launchd `StartCalendarInterval` accepts only Hour + Minute (no day-of-month/month/day-of-week native).
   - Linux: cron-tab natively supports full 5-field, but P013 deliberately constrained to daily form for parity with macOS. The asymmetry would either require (i) `RegisterIntent` extension carrying full 5-field cron string OR (ii) two scheduler trait methods (one daily, one full cron) — both rejected for Phase 3. Future expansion lives behind a separate phiếu (cited as future scope in INV-23 note).
   - **Why Option A and not Option B (expand Linux to full 5-field):** Phase 3 acceptance criterion in BACKLOG.md line 26 says `parse "M H * * *" → "daily at HH:MM"` — daily-form only. Extending Linux to full 5-field would re-open Tầng 1 design debate (`RegisterIntent` shape, `parse_daily_cron` rename + extension, `core::register::run` validation logic, new test coverage for cron variants) — scope creep for a docs+CI phiếu. Architect explicitly chose A; documented decision in §Decision Log.

3. **GitHub Actions CI matrix** — CREATE `.github/workflows/ci.yml` with `strategy: matrix: os: [macos-latest, ubuntu-latest]`. Each job runs `cargo build --release`, `cargo test --all`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Pre-step `which crontab` (Linux job — capability smoke per Sub-mechanism B; fails loud if `crontab` absent so we know we need an explicit install step in a follow-up phiếu). macOS job: integration tests in `tests/cli_register.rs` SPAWN the compiled binary and CANNOT inject `NoopScheduler` (per the test file's own header docstring). Macros safety on the GHA `macos-latest` runner relies instead on (i) `#[cfg(target_os = "macos")]`-gated unit tests inside `src/scheduler/macos.rs` using the in-module `NoopLaunchctl` test impl (no real `launchctl` shell-out) and (ii) integration tests gracefully degrading when `launchctl bootstrap` is unavailable on the sandboxed runner — observe-first per DP4, Tầng 2 follow-up if a test breaks. See INV-10 macOS trust model for the trust boundary rationale.

4. **ARCHITECTURE.md polish** — small additive edits:
   - §Phase status: Phase 3.3 ⏸️ → ✅ (Worker fills at EXECUTE close).
   - NEW §CI matrix subsection (after §MCP surface): documents the 2-OS job matrix + which test set runs where + Sub-mechanism B Linux `crontab` capability check.
   - NO §Cron mechanism edits (P013 already split into "macOS launchd plist" + "Linux crontab injection" subsections — Anchor #5 verifies).
   - NO §Scheduler trait section edits (P012 already added it — Anchor #6 verifies via the §Modules table row).

### Scope

**CHỈ sửa:**

1. **EDIT** `docs/security/INVARIANTS.md` — append INV-22 entry (after INV-21 closing line) + INV-23 entry (after INV-22). Use EXACT text from §INV-22 + §INV-23 sections below — Worker copy verbatim. No edits to INV-1..21.

2. **CREATE** `.github/workflows/ci.yml` — new file. Use EXACT yaml from §CI workflow file (full body) section below — Worker copy verbatim. NO `.github/dependabot.yml`, `.github/CODEOWNERS`, etc. — out of scope.

3. **EDIT** `docs/ARCHITECTURE.md`:
   - §Phase status entry for Phase 3.3 — update `⏸️ Phase 3.3 (P014): CI matrix (macOS + Linux parallel jobs). Deferred.` to `✅ Phase 3.3 (P014): INV-22 + INV-23 documented in docs/security/INVARIANTS.md (5 sub-rules each, parallel format to INV-10/12/17). CI workflow .github/workflows/ci.yml created with macos-latest + ubuntu-latest matrix.`
   - APPEND new subsection §CI matrix after §MCP surface — use EXACT text from §ARCHITECTURE.md CI matrix subsection below.

4. **EDIT** `docs/CHANGELOG.md` — prepend P014 entry per CHANGELOG convention (anchor: P013 entry at `docs/CHANGELOG.md:9`). Use template in §CHANGELOG entry below.

**KHÔNG sửa (OUT OF SCOPE — cứng, reject creep):**

- KHÔNG đổi P013 enforcement code (`src/scheduler/linux.rs`, `src/scheduler/mod.rs`, `src/scheduler/macos.rs`) — INV-22 documents what's already shipped, doesn't re-implement.
- KHÔNG consolidate `src/core/{register,unregister,status}.rs` inline `is_valid_label` copies — Phase 3.5+ Tầng 2 cleanup item surfaced by Turn 1 (see Decision Log DP7). P014 only documents the reality; consolidation is a separate refactor phiếu.
- KHÔNG extend Linux to full 5-field cron — Option A. `src/core/register.rs::parse_daily_cron` unchanged. `RegisterIntent` shape unchanged.
- KHÔNG update README — P015 (Phase 3.4).
- KHÔNG add new INV beyond INV-22, INV-23.
- KHÔNG add new dep, KHÔNG add new tokio feature flag. `Cargo.toml` ZERO diff.
- KHÔNG đổi MCP tool schema, CLI subcommand, flag, exit code.
- KHÔNG đổi config schema (`Config`, `TaskConfig`, `ScheduleConfig`, `HeartbeatConfig`, `AlertConfig`, `RetryConfig`).
- KHÔNG add `.github/dependabot.yml`, `.github/CODEOWNERS`, `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/*`. Phase 4+ if dogfood reveals need.
- KHÔNG add `actionlint` / `gh workflow lint` to CI as a step — Worker MAY run locally for spec confidence, but the workflow file itself doesn't lint itself.
- KHÔNG add caching (`actions/cache@v4` for `target/` or `~/.cargo/registry`) — small-binary project; ~3min CI per job acceptable. Phase 4+ optimisation if CI cost matters.
- KHÔNG add release workflow (binary publish on tag push, `cargo publish`, etc.) — Phase 6 future wave per BACKLOG.
- KHÔNG đổi Phase status 3.2 / 3.1 / 2.x / 1.x text — only 3.3 row.

### Skills consulted

*(none — docs entries + standard GitHub Actions matrix workflow. No new external library question. yaml syntax + standard `actions/checkout@v4` + `dtolnay/rust-toolchain@stable` are well-established patterns. Worker MAY invoke context7 if friction with rust-toolchain action options — expected zero friction.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `.github/workflows/` directory does NOT exist (no prior CI workflow) | Architect `Glob(".github/**/*")` + `Glob("**/*.yml")` | `[verified]` | ✅ Both globs returned 0 files. P014 creates `.github/workflows/ci.yml` from scratch. |
| 2 | `docs/security/INVARIANTS.md` ends at INV-21 (line 318+); INV-22 + INV-23 do NOT yet exist | Architect Read full file (365 lines, Read result above) | `[verified]` | ✅ INV-21 closes at line 343 (`Implemented in Giám sát` row). Section "How INV are checked" starts line 346. INV-22 + INV-23 NEW entries go between line 343 and line 346 (or equivalently: between INV-21 closing and `## How INV are checked` H2). |
| 3 | INV-10, INV-12, INV-17 are the parallel format to model INV-22 after (label sanitization + launchctl shell-out patterns) | Architect Read `docs/security/INVARIANTS.md` lines 105–172 + 232–246 | `[verified]` | ✅ Confirmed: INV-10 (launchctl shell-out boundary), INV-12 (label sanitization 2-point), INV-17 (`launchctl print` shell-out + label sanitization). INV-22 reuses the same Statement / Why / Implementation / Trigger keywords / Status / Implemented in Giám sát structure. |
| 4 | `tests/cli_register.rs` integration tests CANNOT inject `NoopScheduler` (they spawn the compiled binary); macOS-gated unit tests inside `src/scheduler/macos.rs` use the in-module `NoopLaunchctl` test impl | Worker Turn 1 grep + test file header docstring (`tests/cli_register.rs:3,6`) | `[verified — Turn 1 V2 correction]` | ✅ Worker Turn 1 confirmed: `tests/cli_register.rs:3,6` header explicitly states "CRITICAL: integration tests spawn the compiled binary, so they CANNOT inject NoopScheduler. Unit tests inside `src/scheduler/macos.rs #[cfg(test)] mod` are the only place NoopScheduler is wired." V2 phiếu (Task 3 yaml comment + Giải pháp prose) reflects this. CI macOS safety relies on (a) unit tests using `NoopLaunchctl` (no real shell-out) + (b) integration tests gracefully degrading when `launchctl bootstrap` is unavailable on GHA sandbox. Observe-first per DP4. |
| 5 | `docs/ARCHITECTURE.md` §Cron mechanism is ALREADY split into "macOS — launchd plist" (line 186) + "Linux — crontab injection" (line 242) subsections — P014 does NOT need to re-split | Architect Read `docs/ARCHITECTURE.md` lines 186–268 | `[verified]` | ✅ Confirmed: line 186 `## Cron mechanism — macOS (launchd plist)` + line 242 `## Cron mechanism — Linux (crontab injection)`. Split already done by P013. P014 only ADDS §CI matrix subsection (after §MCP surface line 272). |
| 6 | `docs/ARCHITECTURE.md` §Scheduler trait section EXISTS at line 68 (added by P012) — P014 does NOT need to add | Architect Read `docs/ARCHITECTURE.md` lines 68–82 | `[verified]` | ✅ Confirmed: line 68 `### Scheduler trait (Phase 3.1 — P012)`. No edit needed. |
| 7 | `src/core/register.rs::parse_daily_cron` (or equivalent fn) is the M H * * * parser that INV-23 references | ARCHITECTURE.md §Modules row for `src/core/register.rs` line 46: "Inline `parse_daily_cron` (domain logic, not scheduler logic)" + Worker Turn 1 grep confirmed line 111 | `[verified via Worker Turn 1]` | ✅ Confirmed: `src/core/register.rs:111: fn parse_daily_cron(expr: &str) -> Result<(u8, u8)>`. Worker Turn 1 ALSO discovered parallel parser `src/scheduler/macos.rs:111: fn parse_simple_cron(expr: &str) -> Result<(u8, u8)>` — both enforce daily form; V2 INV-23 Trigger keywords list both (see Task 2 below). |
| 8 | `src/scheduler/mod.rs::is_valid_label` is the canonical INV-22 sub-rule 2 helper FOR THE SCHEDULER BOUNDARY; `src/core/{register,unregister,status}.rs` carry 3 inline copies predating P013 (separate, consistent — NOT consolidated) | Worker Turn 1 grep: 4 implementations found (`src/scheduler/mod.rs:68`, `src/core/register.rs:37`, `src/core/unregister.rs:35`, `src/core/status.rs:99`) | `[verified — Turn 1 V2 correction]` | ✅ Worker Turn 1 confirmed: P013 consolidated the SCHEDULER layer (`scheduler::is_valid_label` shared by `MacosScheduler::unregister` + `CrontabScheduler::{register, unregister, status}`); P013 did NOT touch the core layer. V2 INV-22 sub-rule 2 qualifies "single source of truth FOR THE SCHEDULER BOUNDARY" + notes core-layer copies + Phase 3.5+ consolidation deferred (DP7 — see Decision Log). |
| 9 | `ubuntu-latest` GHA runner ships `crontab` binary in default image (no `apt-get install` step needed) | GitHub-hosted runner image spec — ubuntu-22.04 / ubuntu-24.04 include `cron` package in base image per `ubuntu-latest` runner-images repo conventions | `[unverified]` | ⏳ Workflow includes explicit `which crontab` smoke step (Sub-mechanism B) — fails loud at CI runtime if absent. Worker observes first CI run; if step exits 1 → DISCOVERY_REPORT + Architect/Sếp decides Tầng 2 follow-up (`sudo apt-get install -y cron`). NOT a blocker for P014 ship — workflow file lands either way; the smoke step is the safety net. |
| 10 | `macos-latest` GHA runner CAN run `cargo test --all` for tests gated `#[cfg(target_os = "macos")]` — unit tests use `NoopLaunchctl` (no real `launchctl`); integration tests degrade gracefully when `launchctl bootstrap` fails on GHA sandbox | Combination of Anchor #4 (above) + GHA macos-latest runner has full Rust toolchain via `dtolnay/rust-toolchain@stable` action | `[needs Worker verify — first CI run]` | ⏳ Verify via first CI run after P014 ships. If macOS integration tests fail due to `launchctl bootstrap` denial on GHA runner (not gracefully degrading) → DISCOVERY_REPORT, Architect Tầng 2 follow-up to add `#[ignore]` or `#[cfg_attr(ci, ignore)]` to specific failing tests OR `if: matrix.os != 'macos-latest'` step skip. Low-medium risk (observe-first per DP4). |
| 11 | `docs/CHANGELOG.md` newest entry is at line 9 (P013) — P014 entry goes BEFORE it (newest at top per file header line 3) | Architect Read `docs/CHANGELOG.md` lines 1–30 | `[verified]` | ✅ Confirmed. P014 entry inserted between line 8 (`---` separator after header) and line 9 (`## 2026-05-28 — P013...`). |
| 12 | `dtolnay/rust-toolchain@stable` is the standard GitHub Actions Rust toolchain installer (not `actions-rs/toolchain` which is deprecated) | Industry convention 2024+; rust-toolchain action by dtolnay is the de-facto standard for solo + medium Rust projects | `[unverified]` | ⏳ Worker confirms via context7 query `dtolnay/rust-toolchain` if uncertain. Fallback: `actions-rust-lang/setup-rust-toolchain@v1` is the more featured alternative; either acceptable. Architect picks `dtolnay/rust-toolchain@stable` for minimalism (advisory-cron uses stable Rust 1.85+ per CLAUDE.md MSRV target). |
| 13 | `actions/checkout@v4` is the current stable checkout action (not v3 / v2 / v1) | Industry standard since 2023; v4 supports Node 20+ runner | `[unverified]` | ⏳ Worker MAY upgrade to v4 minor if a later v4.x.y is preferred; Architect specs `@v4` (semver-major). |
| 14 | Phase 3 BACKLOG acceptance criterion #8 (line 30) requires `cargo test --all` Linux pass via CI runner Linux job — P014 satisfies this | BACKLOG.md line 30 verbatim: `cargo test --all` trên Linux: pass (cross-OS test matrix — CI runner thêm Linux job) | `[verified]` | ✅ Confirmed. CI matrix Linux job runs `cargo test --all` — satisfies criterion. |
| 15 | P013 Discovery Report watch-item P014 (line 47–49) names exactly the 3 sub-deliverables of this phiếu: INV-22 formal doc + INV-23 cron expression validation + CI matrix | `docs/discoveries/P013.md` lines 47–49 | `[verified]` | ✅ Confirmed: "P014: INV-22 formal documentation trong docs/security/INVARIANTS.md + INV-23 cron expression validation + CI matrix (Linux + macOS parallel jobs)". P014 scope matches verbatim. |
| 16 | TOCTOU race between `crontab -l` and `crontab -` is documented in P013 phiếu (Risk + Edge cases) + acknowledged in ARCHITECTURE.md §Cron mechanism Linux | `docs/ARCHITECTURE.md` line 262: "Last-writer-wins race: between `crontab -l` (read) and `crontab -` (write), another process modifying the user crontab races. P013 accepts last-writer-wins." | `[verified]` | ✅ Confirmed. INV-22 sub-rule (e) cross-references this exact ARCHITECTURE.md location rather than duplicating prose. |

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.**

⚠️ Anchor #10 — `[needs Worker verify]` (first CI run). Mechanical observation. Low-medium risk — fix via `#[ignore]` selectively if a macOS integration test fails to gracefully degrade on GHA sandbox. V2 wording aligned with Worker Turn 1 reality.
⚠️ Anchor #9 — `[unverified]`. Sub-mechanism B smoke step in workflow is the safety net. Failure surfaces at first CI run, not at phiếu ship.
⚠️ Anchor #12 + #13 — `[unverified]` (industry-standard action versions). Worker MAY tweak if context7 reveals better current best practice. Either-OR acceptable.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns.

**Phiếu version:** V2 (Turn 1 Worker CHALLENGE → Turn 1 Architect RESPONSE applied)

### Turn 1 — Worker CHALLENGE (2026-05-28)

**Anchors verified (3 Architect-deferred mechanical):**

1. **`parse_daily_cron` location + signature:**
   - Command: `grep -n "fn parse_daily_cron\|fn parse_simple_cron" src/core/register.rs src/scheduler/macos.rs`
   - Result: `src/core/register.rs:111:fn parse_daily_cron(expr: &str) -> Result<(u8, u8)>` — private fn, correct location. Also found: `src/scheduler/macos.rs:111:fn parse_simple_cron(expr: &str) -> Result<(u8, u8)>` — a SEPARATE fn in macos.rs scheduler.
   - Verdict: ⚠️ — see O1.1 below (two parsers, not one).

2. **`is_valid_label` location + sharedness:**
   - Command: `grep -rn "pub fn is_valid_label\|fn is_valid_label" src/scheduler/ src/core/`
   - Result: **4 copies found:**
     - `src/scheduler/mod.rs:68:pub fn is_valid_label` — the shared helper
     - `src/core/unregister.rs:35:pub fn is_valid_label`
     - `src/core/register.rs:37:pub fn is_valid_label`
     - `src/core/status.rs:99:pub(crate) fn is_valid_label`
   - The CrontabScheduler imports from `src/scheduler/mod.rs` (confirmed via `linux.rs` grep: `use ... is_valid_label` from mod), but 3 additional inline copies still exist in `src/core/*.rs`.
   - Verdict: ⚠️ — see O1.2 below. INV-22 text claims "single source of truth lives in `src/scheduler/mod.rs`" — this is partially false; 3 inline copies co-exist. Doctrine wording needs scoping.

3. **`NoopScheduler` for test injection:**
   - Command: `grep -rn "NoopScheduler\|NoopLaunchctl" src/scheduler/ tests/`
   - Result:
     - `src/scheduler/mod.rs:130,136,142` — `NoopScheduler` defined here (test impl replacing `NoopLaunchctl`)
     - `tests/cli_register.rs:3,6` — **CRITICAL NOTE in the test file header**: "CRITICAL: integration tests spawn the compiled binary, so they CANNOT inject NoopScheduler. Unit tests inside `src/scheduler/macos.rs #[cfg(test)] mod` are the only place NoopScheduler is wired."
     - `src/scheduler/macos.rs:170,396,401,561,583` — `NoopLaunchctl` (local test struct inside macos.rs)
   - Verdict: ⚠️ — see O1.3 below. The CI yaml comment and Anchor #4 assumption ("tests/cli_register.rs integration tests via NoopScheduler injection") is INCORRECT per the test file's own docstring. Integration tests spawn the compiled binary — `NoopScheduler` is NOT injected. The safe-to-run-on-GHA claim holds for a different reason (the integration tests may not call `launchctl` at all, or call it in a way that fails gracefully), but the mechanism cited is wrong.

**Sanity checks:**

4. **INV count baseline:**
   - Command: `grep -c "^### INV-" docs/security/INVARIANTS.md`
   - Result: **21** — matches Architect's Sub-mechanism C expected baseline.
   - Verdict: ✅

5. **CI yaml syntactic sanity (read from phiếu Task 3 body):**
   - `name: ci` ✅
   - `on: push/pull_request branches: [main]` ✅
   - `jobs.test.strategy.matrix.os: [macos-latest, ubuntu-latest]` ✅ valid syntax
   - `uses: actions/checkout@v4` ✅ real action, current major version
   - `uses: dtolnay/rust-toolchain@stable` ✅ standard de-facto Rust toolchain installer for GHA (not deprecated actions-rs/toolchain)
   - `with: components: clippy, rustfmt` ✅ valid components syntax for dtolnay action
   - `fail-fast: false` ✅ correct placement under `strategy`
   - `cargo fmt --all -- --check` ✅ (note: phiếu uses `--all` which is fine, equivalent to `--check` applied to workspace)
   - `cargo test --all` ✅ runs both lib + integration tests
   - Linux `which crontab` step: `if: matrix.os == 'ubuntu-latest'` ✅ correct GHA expression syntax
   - No caching, no release, no upload-artifact ✅ per constraints
   - Verdict: ✅ APPROVE — yaml structurally correct, action versions current.

6. **INV-22 wording sanity (5 sub-rules coverage):**
   - (a) discrete-arg shell-out ✅ — sub-rule 1 covers `Command::new("crontab").arg(...)` vs `sh -c` clearly
   - (b) label allowlist 2-point ✅ — sub-rule 2 covers pre-flight + defense-in-depth; cites `src/scheduler/mod.rs::is_valid_label` + `src/core::*::run` pre-flight
   - (c) tag-only filter idempotency ✅ — sub-rule 3 covers `# advisory-cron: <label>` substring match clearly
   - (d) sync stdlib only ✅ — sub-rule 4 covers `tokio::runtime::Runtime::new().block_on` panic trap clearly
   - (e) TOCTOU acknowledged ✅ — sub-rule 5 cites ARCHITECTURE.md existing prose
   - Gap from P013 reality: sub-rule 2 says "single source of truth lives in `src/scheduler/mod.rs::is_valid_label`" — factually contested (3 inline copies in core/*.rs still exist). Wording MUST be scoped to "for the scheduler boundary" or qualified. Unqualified "single source of truth" claim in a security doctrine entry is wrong.
   - Verdict: ⚠️ — sub-rule 2 wording needs qualification. See O1.2.

7. **INV-23 Option A wording:**
   - Sub-rule 1: daily-form only, rejects ranges/lists/steps ✅
   - Sub-rule 2: cross-platform parity rationale ✅ (mirrors INV-11 / macOS `StartCalendarInterval` constraint)
   - Sub-rule 3: u8 bounds-check at parse time ✅ — parallel to INV-11's HH:MM bound
   - Sub-rule 4: no code change in P014, future expansion documented with 6-item scope ✅
   - Note: INV-23 Implementation cites `src/core/register.rs::parse_daily_cron` (line 111 confirmed). However, `src/scheduler/macos.rs` has a SEPARATE `parse_simple_cron` function (also line 111) that may parse the same form — this asymmetry is not reflected in INV-23 text (INV-23 only cites `register.rs::parse_daily_cron`, not `macos.rs::parse_simple_cron`). This is a documentation gap, not necessarily an INV wording error — but future Worker auditing INV-23 trigger keywords won't catch `parse_simple_cron` changes.
   - Verdict: ⚠️ — see O1.1 (trigger keyword gap, Tầng 1 question).

**Decision point review:**

- **DP1 (5 sub-rules incl. TOCTOU as sub-rule 5):** APPROVE in principle. INV philosophy question: sub-rule 5 says "acknowledged + deferred" — this is honest documentation, not enforcement. Parallel precedent exists (INV-21 has deferred-hardening notes). Keeping it in INV rather than ARCHITECTURE-only ensures `grep INV-22` surfaces the gap for auditors. APPROVE.
- **DP2 (Option A daily-form):** APPROVE. Phase 3 BACKLOG criterion is daily-form only; Linux full 5-field would require RegisterIntent + parser + test changes — clearly scope creep for a docs+CI phiếu. APPROVE.
- **DP3 (yaml from scratch):** APPROVE. Anchor #1 confirmed 0 `.github/` files.
- **DP4 (macOS GHA observe-first):** APPROVE with note — the safety rationale is valid (`NoopScheduler` exists in `src/scheduler/mod.rs`), but the mechanism cited in the CI yaml COMMENT and Anchor #4 text is wrong: integration tests spawn the binary and CANNOT inject `NoopScheduler` (per `tests/cli_register.rs` header). The actual macOS CI safety is: integration tests either (a) test only non-launchctl paths, or (b) the macOS gated `#[cfg]` unit tests inside `src/scheduler/macos.rs` run via `cargo test --lib` path and use `NoopLaunchctl` (local to `macos.rs`), not `NoopScheduler` from `mod.rs`. The observe-first approach is still correct; the doc comment in ci.yml will mislead future Workers. This is O1.3 — wording objection in the CI yaml comment block.
- **DP5 (ubuntu observe-first crontab):** APPROVE. GHA `ubuntu-latest` default image does include `cron` package. The `which crontab` smoke step is the right safety net. Observe-first is correct.
- **DP6 (TOCTOU as sub-rule 5):** APPROVE. INV-22 cross-refs existing ARCHITECTURE.md prose; no duplication.

**Out-of-decision-point objections:**

- None beyond O1.1, O1.2, O1.3 below.

---

**Objections (Tầng 1 — phiếu cần sửa):**

**[O1.1] — `parse_simple_cron` in `src/scheduler/macos.rs` not cited in INV-23 trigger keywords**

- Evidence: `src/scheduler/macos.rs:111:fn parse_simple_cron(expr: &str) -> Result<(u8, u8)>` — parallel daily-form parser in the macOS scheduler, not cited in INV-23 trigger keywords or Implementation paragraph.
- INV-23 cites only `src/core/register.rs::parse_daily_cron`. A future PR changing `parse_simple_cron` without updating INV-23 would be an undetected invariant gap (Giám sát won't flag it, trigger keyword not listed).
- **This is a Tầng 1 wording question** — the INV doctrine text, once shipped, is the audit contract. Silently omitting a known parallel fn from Trigger keywords = future audit gap.
- Proposed fixes (Worker recommends A):
  - **A (Recommended):** Add `parse_simple_cron` to INV-23 Trigger keywords sentence: append `parse_simple_cron` (macOS daily-form parser in `src/scheduler/macos.rs` — same constraint, separate codepath). Implementation paragraph note: "Note: `src/scheduler/macos.rs::parse_simple_cron` is the parallel macOS-scheduler-internal parser — also accepts only daily form; same bounds-check applies."
  - **B:** Leave as-is, note in DISCOVERY_REPORT for P015+ to add a separate INV or unify parsers. Lower audit value but ships P014 today.

**[O1.2] — INV-22 sub-rule 2 "single source of truth" claim is factually incorrect**

- Evidence: `is_valid_label` has 4 implementations: `src/scheduler/mod.rs:68` (shared, `pub`) + `src/core/register.rs:37` (`pub`) + `src/core/unregister.rs:35` (`pub`) + `src/core/status.rs:99` (`pub(crate)`). The CrontabScheduler uses `src/scheduler/mod.rs::is_valid_label` (confirmed from `linux.rs` import). But `src/core/*.rs` modules each carry their own inline copy — those pre-date P013 and were NOT consolidated.
- INV-22 sub-rule 2 text: "single source of truth for the allowlist lives in `src/scheduler/mod.rs::is_valid_label`" — this claim is wrong for the core layer.
- **Tầng 1 wording question** — security doctrine claiming "single source of truth" when 3 parallel copies exist is misleading; a future Worker changing allowlist chars in only one copy would appear to satisfy INV-22 while 3 other copies remain stale.
- Proposed fixes (Worker recommends A):
  - **A (Recommended):** Qualify the sentence in sub-rule 2: "For the scheduler boundary, the single source of truth is `src/scheduler/mod.rs::is_valid_label`. Note: `src/core/{register,unregister,status}.rs` each carry a local copy predating P013 consolidation — these remain consistent but are NOT the scheduler's single source. Consolidation to one crate-wide helper is deferred (Phase 3.5+ or a dedicated refactor phiếu)." This is honest documentation without scope-expanding the phiếu.
  - **B:** Remove "single source of truth" language entirely, replace with "scheduler boundary uses `src/scheduler/mod.rs::is_valid_label`; core layer uses separate local copies."

**[O1.3] — CI yaml COMMENT block incorrectly states `tests/cli_register.rs` uses `NoopScheduler` injection**

- Evidence: `tests/cli_register.rs:3,6` — file header comment explicitly: "CRITICAL: integration tests spawn the compiled binary, so they CANNOT inject NoopScheduler. Unit tests inside `src/scheduler/macos.rs #[cfg(test)] mod` are the only place NoopScheduler is wired."
- The CI yaml comment block (Task 3 body, 4th bullet) states: "exercises... tests/cli_register.rs integration tests via NoopScheduler injection" — this directly contradicts the test file's own documentation.
- The ci.yml comment propagates false information into a permanent file. A future Worker reading ci.yml who encounters a macOS CI failure will assume NoopScheduler injection protects them, which is incorrect — they'll debug the wrong layer.
- **Tầng 1 wording question** — the comment is in the CI workflow file that P014 ships; once in main, it's the permanent record.
- Proposed fix (Worker recommends A — only option):
  - **A (Recommended):** Replace the misleading bullet in the ci.yml comment block: change `tests/cli_register.rs integration tests via NoopScheduler injection` to `tests/cli_register.rs integration tests (spawn compiled binary; macOS sandbox safety relies on no real launchctl shell-out in binary code paths exercised by integration tests — see INV-10)`. Remove the "via NoopScheduler injection" clause entirely from the yaml comment.

---

**Verdict:** NEEDS_RESPOND — 3 Tầng 1 wording objections (O1.1, O1.2, O1.3). All fixable in V2 with targeted text patches. No code changes. No LOC estimate change (~270 remains valid). Architect Turn 2 needed.

**LOC estimate:** agree ~270 (docs ~220 + yaml ~50)

**Status:** ✅ RESPONDED — phiếu bumped to V2 (see Turn 2 below)

---

### Turn 2 — Architect RESPOND (2026-05-28)

**O1.1 (INV-23 Trigger keywords miss `parse_simple_cron`):** **ACCEPT.** Worker correctly identified the parallel parser in `src/scheduler/macos.rs:111` would be invisible to future Giám sát auditing INV-23 if not listed in Trigger keywords. V2 adds `parse_simple_cron` (`src/scheduler/macos.rs`) to INV-23 Trigger keywords + 1-sentence Implementation note acknowledging both parsers enforce the same daily-form constraint via separate codepaths + Phase 3.5+ consolidation candidate flagged. Fix applied in Task 2 (INV-23 Implementation paragraph + Trigger keywords sentence).

**O1.2 (INV-22 sub-rule 2 "single source of truth" claim wrong):** **ACCEPT.** Worker grep confirmed 4 implementations of `is_valid_label`: 1 shared in `src/scheduler/mod.rs:68` (used by both schedulers post-P013) + 3 inline copies in `src/core/{register,unregister,status}.rs` (predating P013, NOT consolidated). The V1 unqualified "single source of truth" claim is factually wrong and would mislead future audit. V2 qualifies sub-rule 2 per Worker fix A: "single source of truth FOR THE SCHEDULER BOUNDARY is `src/scheduler/mod.rs::is_valid_label`" + explicit note that `src/core/{register,unregister,status}.rs` carry local copies predating P013 + Phase 3.5+ consolidation deferred. Fix applied in Task 1 (INV-22 sub-rule 2 wording).

**O1.3 (CI yaml comment false `NoopScheduler` claim):** **ACCEPT.** Worker correctly cited `tests/cli_register.rs:3,6` header docstring — integration tests spawn the compiled binary and CANNOT inject `NoopScheduler`. The V1 yaml comment would have permanently lied to future readers. V2 removes the "via NoopScheduler injection" clause from the yaml comment block AND from the Giải pháp prose (point 3) AND from the §CI matrix subsection ARCHITECTURE.md text AND from Anchor #4 result column. Replacement language cites: (a) unit tests inside `src/scheduler/macos.rs` use in-module `NoopLaunchctl` (no real `launchctl`), (b) integration tests rely on graceful degradation when `launchctl bootstrap` is unavailable on GHA sandbox, (c) observe-first per DP4. Fix applied in Task 3 (ci.yml comment), Giải pháp prose, §CI matrix subsection, and Anchor #4.

**Net change V1 → V2:**

- **INV-23 (Task 2):** Implementation paragraph adds 1 sentence about `parse_simple_cron`; Trigger keywords list extended by 1 entry (`parse_simple_cron` in `src/scheduler/macos.rs`).
- **INV-22 sub-rule 2 (Task 1):** wording revised from unqualified "single source of truth" → qualified "single source of truth FOR THE SCHEDULER BOUNDARY" + 2-sentence note on core-layer copies + Phase 3.5+ deferred.
- **CI yaml comment block (Task 3):** the macOS-job bullet rewritten — removes "via NoopScheduler injection" lie; replaces with accurate description (compiled-binary spawn + observe-first sandbox degradation).
- **Giải pháp prose (point 3):** macOS-job description revised to match Task 3 yaml comment accuracy.
- **§CI matrix subsection (Task 4b):** "Why no `tokio::process::Command` against `launchctl`" paragraph revised to remove false `NoopScheduler` injection claim.
- **Anchor #4:** marker updated from `[needs Worker verify]` → `[verified — Turn 1 V2 correction]`; result column rewritten with Worker Turn 1 evidence.
- **Anchor #7:** marker updated from `[verified via ARCHITECTURE.md]` → `[verified via Worker Turn 1]`; result column extended with `parse_simple_cron` discovery.
- **Anchor #8:** marker updated from `[verified via P013]` → `[verified — Turn 1 V2 correction]`; result column rewritten to reflect 4-implementation reality + DP7 reference.
- **Anchor #10:** marker updated from `[needs Worker verify]` → `[needs Worker verify — first CI run]`; result column rewritten to remove false `NoopScheduler` injection claim.
- **Scope (Files KHÔNG sửa):** new bullet — KHÔNG consolidate core-layer `is_valid_label` copies (DP7, Phase 3.5+ Tầng 2).
- **Decision Log:** new row DP7 — "core-layer `is_valid_label` consolidation deferred Phase 3.5+".
- **LOC estimate:** unchanged ~270 (all V2 edits are surgical text revisions; no new sections, no new sub-rules).
- All other DPs (1, 2, 3, 4, 5, 6) preserved unchanged.
- Cargo.toml / Cargo.lock / src/ — still zero diff.

**New debt item surfaced (for Quản đốc to log post-merge):**

- **[DEBT] core-layer `is_valid_label` consolidation:** `src/core/{register,unregister,status}.rs` each carry an inline `is_valid_label` copy predating P013's scheduler-layer consolidation. Four implementations are kept consistent today, but a future Worker changing the allowlist characters in only one location would silently desynchronise the others. Consolidation into a single crate-wide helper (or import of `scheduler::is_valid_label` from the core layer) is a Phase 3.5+ Tầng 2 refactor candidate. Quản đốc add to `docs/BACKLOG.md` "Open backlog" section post-P014 merge as `[DEBT] core layer is_valid_label consolidation (Phase 3.5+ Tầng 2)`.

**Status:** ✅ RESPONDED — phiếu V2 ready for Worker Turn 2 review (or proceed to Sếp approval gate if Worker accepts V2 without further objection).

---

## Debug Log

> Worker emit observability records during EXECUTE. Append-only.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command snippet>
```

---

## Verification Trace — Sub-mechanism A-E checks

> Worker MUST run applicable checks BEFORE marking phiếu DONE.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | First `git push` to PR branch fires `.github/workflows/ci.yml` workflow — Worker observes via `gh run list --workflow=ci.yml --limit 1` or GitHub web UI | 1 workflow run started, 2 jobs (macos-latest + ubuntu-latest) | | |
| A (trigger) | `gh workflow list` after push | `ci` workflow enabled and listed | | |
| B (capability) | `.github/workflows/ci.yml` exists + valid yaml | `cat .github/workflows/ci.yml` exits 0, file non-empty | | |
| B (capability) | Workflow Linux job `which crontab` step exits 0 in CI logs | `/usr/bin/crontab` or `/bin/crontab` printed | | |
| B (capability) | Workflow Linux job `cargo test --all` step exits 0 | green ✅ in GHA logs | | |
| B (capability) | Workflow macOS job `cargo test --all` step exits 0 | green ✅ in GHA logs | | |
| B (capability) | Workflow Linux job `cargo clippy --all-targets -- -D warnings` exits 0 | green ✅ | | |
| B (capability) | Workflow macOS job `cargo clippy --all-targets -- -D warnings` exits 0 | green ✅ | | |
| B (capability) | Local `cargo build --release` after edits | exit 0, 0 warnings (sanity — no src/ changes so should always pass) | | |
| C (migration completeness) | `grep -c "^### INV-" docs/security/INVARIANTS.md` BEFORE | 21 | | |
| C (migration completeness) | `grep -c "^### INV-" docs/security/INVARIANTS.md` AFTER | 23 (+INV-22 +INV-23) | | |
| C (migration completeness) | `grep -n "INV-22\|INV-23" docs/security/INVARIANTS.md` AFTER | ≥2 hits each (header + cross-refs) | | |
| D (persistence) | `grep -l "INV-22\|INV-23" docs/security/INVARIANTS.md` | 1 hit (non-rotate-prone — INVARIANTS.md is durable doctrine per CLAUDE.md §Knowledge durability convention) | | |
| D (persistence) | `grep -l "INV-22\|INV-23" CLAUDE.md docs/RULES.md` 2>/dev/null | 0 hits (acceptable — INVARIANTS.md is the single doctrine home; CLAUDE.md soft-references via §HARD STOPS rule 4 boundary text) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (sanity — no Cargo.toml edits) | | |
| E (env drift) | `git diff Cargo.toml Cargo.lock` after edits | empty | | |
| E (env drift) | `git diff src/` after edits | empty (P014 is docs+infra only) | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability + assumption verify

**Mục đích:** Verify all `[needs Worker verify]` and `[unverified]` anchors BEFORE touching files.

Run all of (record output for each in Debug Log):

1. **Anchor #1 sanity — `.github/workflows/` truly absent:**
   ```bash
   ls -la .github/ 2>&1
   ls -la .github/workflows/ 2>&1
   ```
   Expected: `.github` does not exist (per Architect Glob). If `.github/workflows/ci.yml` already exists → STOP, escalate to Architect (someone else wrote a CI workflow between Architect DRAFT and Worker EXECUTE → re-plan).

2. **Anchor #2 sanity — INV-21 closing line + INV-22/23 not yet present:**
   ```bash
   grep -c "^### INV-" docs/security/INVARIANTS.md
   grep -n "INV-22\|INV-23" docs/security/INVARIANTS.md
   ```
   Expected: count = 21 (no INV-22 or INV-23 yet). 0 matches for INV-22/23 grep. If different → STOP, re-read INVARIANTS.md current state.

3. **Anchor #4 re-confirm — `NoopScheduler` injection NOT in `tests/cli_register.rs` (CI macOS safety mechanism):**
   ```bash
   grep -n "NoopScheduler\|RealLaunchctl\|launchctl" tests/cli_register.rs
   sed -n '1,10p' tests/cli_register.rs
   ```
   Expected: `NoopScheduler` NOT mentioned in tests/cli_register.rs (header docstring at lines 3,6 explicitly forbids it — integration tests spawn the binary). If `NoopScheduler` IS used in tests/cli_register.rs → STOP, escalate to Architect (Worker Turn 1 evidence overturned; phiếu needs re-revision).

4. **Anchor #7 — `parse_daily_cron` + `parse_simple_cron` locations for INV-23 Implementation citation:**
   ```bash
   grep -n "fn parse_daily_cron" src/core/register.rs
   grep -n "fn parse_simple_cron" src/scheduler/macos.rs
   ```
   Expected: 1 hit each at line ~111 (per Worker Turn 1). Record both line numbers for the INV-23 Implementation paragraph + Trigger keywords list.

5. **Anchor #8 — `is_valid_label` 4-location reality (INV-22 sub-rule 2 verification):**
   ```bash
   grep -rn "pub fn is_valid_label\|pub(crate) fn is_valid_label\|fn is_valid_label" src/scheduler/ src/core/
   ```
   Expected: 4 hits (Worker Turn 1 evidence) — `src/scheduler/mod.rs:68`, `src/core/register.rs:37`, `src/core/unregister.rs:35`, `src/core/status.rs:99`. Record line numbers for INV-22 sub-rule 2 wording. If count ≠ 4 (someone consolidated mid-debate, or added a 5th copy) → STOP, escalate (phiếu's qualifier may be stale).

6. **Anchor #16 — ARCHITECTURE.md §Cron mechanism Linux TOCTOU note presence:**
   ```bash
   grep -n "Last-writer-wins\|TOCTOU\|flock" docs/ARCHITECTURE.md
   ```
   Expected: at least 1 hit at line ~262 (per Anchor #16). Used as cross-reference target for INV-22 sub-rule (e).

7. **Anchor #11 — CHANGELOG.md insertion point:**
   ```bash
   head -15 docs/CHANGELOG.md
   ```
   Expected: line 9 starts `## 2026-05-28 — P013...`. P014 entry inserted between line 8 and line 9.

8. **Workflow yaml lint (optional but recommended — Worker self-confidence):**
   ```bash
   # If actionlint installed (via `brew install actionlint` or `go install github.com/rhysd/actionlint/cmd/actionlint@latest`):
   actionlint .github/workflows/ci.yml || echo "actionlint not installed — skipping (workflow validated at CI runtime)"
   ```
   Expected: clean OR actionlint absent (acceptable — GitHub Actions runtime validation is the canonical check).

**Nếu bất kỳ check nào fail expected:** STOP, escalate via AskUserQuestion + DISCOVERY_REPORT.

---

### Task 1 — APPEND INV-22 to `docs/security/INVARIANTS.md`

**File:** `docs/security/INVARIANTS.md`

**Tìm:** end of INV-21 block. Anchor: the line `**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE (unit tests for atomic-write protocol + corrupt-last-line tolerance). Giám sát soi PR diff for heartbeat-related changes; if PR reintroduces `OpenOptions::append(true)` against a heartbeat path OR removes the `tempfile::persist` call, flag as INV-21 violation.` (line ~342) followed by `---` separator (line ~343) followed by `## How INV are checked` H2 (line ~346).

**Thay bằng / Thêm (BETWEEN the `---` after INV-21 and `## How INV are checked` H2):**

```markdown
### INV-22 — `crontab` shell-out boundary: discrete-arg + label allowlist 2-point + tag-only filter + sync stdlib + TOCTOU acknowledged

**Statement:** PR introducing or modifying Linux `crontab` shell-out (`src/scheduler/linux.rs::CrontabScheduler::{register, unregister, status}` or any future `crontab` invocation) MUST satisfy ALL of:

1. **Discrete-arg shell-out (no shell interpolation):** Both `crontab -l` (read) and `crontab -` (stdin write) MUST be invoked via `std::process::Command::new("crontab").arg("-l")` or `.arg("-")` — discrete args only, NEVER `Command::new("sh").arg("-c").arg(format!("crontab ... {user_value}"))`. The `crontab` binary itself does NOT parse a shell line; stdin content is written via `child.stdin.write_all(...)` (sync `std::io::Write`).

2. **Label allowlist 2-point enforcement (defense-in-depth):**
   - **Point 1 (caller pre-flight):** `core::register::run` / `core::unregister::run` / `core::status::run` validate `label` via their respective local `is_valid_label` helper (INV-12 allowlist) BEFORE invoking the scheduler. This already exists pre-P013 (mirrors macOS path).
   - **Point 2 (defense-in-depth, inside each `CrontabScheduler` method):** the first executable line of `CrontabScheduler::register`, `::unregister`, `::status` is `if !super::is_valid_label(label) { bail!(...) }` — single source of truth **FOR THE SCHEDULER BOUNDARY** is `src/scheduler/mod.rs::is_valid_label` (ASCII alphanumeric + `-` + `_`, non-empty). The same scheduler-layer helper backs `MacosScheduler::unregister` (single scheduler-layer allowlist for both schedulers — INV-12 and INV-22 share one helper at the scheduler boundary).
   - **Note on the 4-location reality:** `src/core/{register,unregister,status}.rs` each carry a local `is_valid_label` copy that predates P013's scheduler-layer consolidation. These 3 core-layer copies are kept consistent with the scheduler-layer source by convention, but they are NOT the scheduler's single source of truth — they are independent pre-flight guards (Point 1 above). Consolidation of the 4 copies into a single crate-wide helper is OUT OF SCOPE for P013/P014 and deferred to Phase 3.5+ (or a dedicated refactor phiếu — see DP7 in P014 Decision Log). A future Worker changing the allowlist characters MUST update all 4 locations atomically; INV-22 sub-rule 2 + INV-12 jointly cover the audit surface until consolidation lands.

3. **Tag-only filter idempotency (don't touch user's other crontab lines):** Both `register` and `unregister` MUST filter the user's existing crontab by substring match on `# advisory-cron: <label>` only. Lines without this tag MUST pass through unmodified to the `crontab -` stdin pipe. This guarantees: (a) re-registering the same label replaces (not duplicates) the line; (b) user's non-advisory-cron crontab entries are preserved; (c) `unregister` removes only the tagged line.

4. **Sync `std::process::Command` only (no nested-runtime panic):** The `Scheduler` trait is sync. From `#[tokio::main]` context, calling `tokio::runtime::Runtime::new().block_on(...)` panics: `"Cannot start a runtime from within a runtime"`. Linux scheduler impl MUST use `std::process::Command` (blocking) — NOT `tokio::process::Command` + nested runtime bridge. Blocking shell-out (~10ms per call, ~3 calls/register) is acceptable for the workflow. This rule was learned the hard way in P013 V1 → V2 pivot (Worker CHALLENGE Turn 1 catch — see `docs/discoveries/P013.md`).

5. **TOCTOU race acknowledged + deferred (Phase 3.5+):** Between `crontab -l` (read) and `crontab -` (write), another process modifying the user's crontab races. P013 accepts last-writer-wins. Hardening (advisory `flock(2)` on a sentinel file, e.g. `~/.local/state/advisory-cron/crontab.lock`) is OUT OF SCOPE for Phase 3.2/3.3 and deferred to Phase 3.5+ if dogfood reveals real concurrent-modification incidents. See `docs/ARCHITECTURE.md` §Cron mechanism — Linux (crontab injection) "Last-writer-wins race" paragraph for the full discussion.

**Why:** `crontab` is the FIRST shell-out boundary on Linux (parallel to `launchctl` boundary on macOS — INV-10/12/17). The user crontab is a multi-process shared resource (cron daemon reads it; `crontab -e` edits it; external tools may write it). advisory-cron MUST: (a) never inject shell metacharacters via labels (allowlist), (b) never clobber other crontab lines (tag-only filter), (c) never deadlock or panic at runtime (sync stdlib avoids nested-runtime trap). Without any one of these, advisory-cron either creates an injection vector OR silently destroys the user's other cron jobs OR hangs the CLI.

**Implementation (Phase 3.2 — P013):** `src/scheduler/linux.rs::CrontabScheduler::{register, unregister, status}` — each starts with `if !super::is_valid_label(label) { bail!(...) }`. Scheduler-boundary helper `src/scheduler/mod.rs::is_valid_label` (single source for the scheduler layer; 3 core-layer copies at `src/core/{register,unregister,status}.rs` predate P013 — see sub-rule 2 Note). Tag prefix constant `const TAG_PREFIX: &str = "# advisory-cron: ";` used by both filter (read side) and emit (write side). `read_user_crontab` + `write_user_crontab` private helpers use `std::process::Command` + `std::io::Write::write_all` (sync stdlib, no tokio I/O).

**Trust boundary:** `crontab` binary is part of the host's standard `cron` package (Linux distro convention). advisory-cron does NOT validate or fingerprint the `crontab` binary — Phase 3 trusts the host PATH (same trust model as `tokio::process::Command::new("launchctl")` on macOS per INV-10). If the host has a malicious `crontab` shim in PATH, advisory-cron is already compromised at install time; this is out of scope for runtime defense.

**Trigger keywords:** `CrontabScheduler::*` method bodies, `Command::new("crontab")` (sync or tokio), `Command::new("sh").arg("-c")` combined with format strings containing user input near scheduler code, `tokio::runtime::Runtime::new` + `block_on` near scheduler code (forbidden), `OpenOptions` against user's crontab path (forbidden — must use `crontab -` stdin pipe), new `flock(2)` usage near `crontab` calls (would be the Phase 3.5+ hardening — explicit phiếu required), `is_valid_label` modifications in ANY of the 4 locations (`src/scheduler/mod.rs`, `src/core/{register,unregister,status}.rs` — Worker MUST update all 4 atomically until Phase 3.5+ consolidation lands).

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE (unit tests for invalid-label rejection in `scheduler::linux::tests` + integration tests in `tests/cli_register_linux.rs` for tag-filter idempotency + dogfood smoke for end-to-end). Giám sát soi PR diff for `crontab`-related changes; if PR reintroduces `tokio::process::Command` against `crontab` OR `Runtime::new().block_on` near scheduler code OR removes the `is_valid_label` first-line check OR mutates only a subset of the 4 `is_valid_label` copies, flag as INV-22 violation.

---
```

**Lưu ý:**
- Worker copy verbatim. All line numbers referenced narratively, not embedded as `LINE` placeholders.
- The trailing `---` separator above closes INV-22; INV-23 (Task 2) inserts its own H3 directly after this `---`.
- Format matches INV-10/12/17/21 — 5 numbered sub-rules, Why, Implementation, Trust boundary, Trigger keywords, Status, Implemented in Giám sát rows.
- V2 wording change vs V1: sub-rule 2 qualified ("FOR THE SCHEDULER BOUNDARY") + new "Note on the 4-location reality" 3-sentence block + Implementation paragraph 1-sentence addendum + Trigger keywords extended with 4-copy reminder + Giám sát PR-diff check extended.

---

### Task 2 — APPEND INV-23 to `docs/security/INVARIANTS.md` (after INV-22)

**File:** `docs/security/INVARIANTS.md`

**Tìm:** end of INV-22 block (the trailing `---` separator written in Task 1).

**Thay bằng / Thêm (directly after Task 1's trailing `---`):**

```markdown
### INV-23 — Cron expression validation: daily-form `M H * * *` only for both platforms in Phase 3

**Statement:** PR introducing or modifying `register --schedule <cron>` parsing in `src/core/register.rs::parse_daily_cron` OR the parallel macOS-scheduler-internal parser `src/scheduler/macos.rs::parse_simple_cron` (or any future cron-expression-accepting code path) MUST satisfy ALL of:

1. **Daily form only (Phase 3 invariant):** the accepted cron expression form is `<minute> <hour> * * *` where all of day-of-month, month, day-of-week are literal `*`. Ranges (`1-5`), lists (`1,3,5`), steps (`*/2`), and day-of-week / day-of-month constraints are REJECTED with a parse error (exit code 1, `anyhow` context citing the offending expression). The accepted form mirrors Phase 1 macOS `StartCalendarInterval` Hour + Minute calendar form per ARCHITECTURE.md §Cron mechanism — macOS.

2. **Cross-platform parity (no Linux asymmetry in Phase 3):** Linux cron-tab natively supports the full 5-field cron expression. P013 deliberately constrained the Linux scheduler to the same daily form as macOS to preserve a single `RegisterIntent` shape (`{ label, hour: u8, minute: u8, self_exe, working_dir }`) across both schedulers. Extending Linux to full 5-field WITHOUT extending `RegisterIntent` would create an asymmetry where `--schedule "0 9 * * 1-5"` would work on Linux but error on macOS — a usability foot-gun. Phase 3 explicitly chooses parity over Linux-native expressiveness.

3. **Hour ∈ 0..=23, Minute ∈ 0..=59 bounds-check at parse time:** Both parsers MUST `.parse::<u8>()` both fields and bounds-check before returning. Out-of-range values (e.g. `25 0 * * *`) exit 1 with parse error citing the bound. Empty fields, non-numeric fields, and negative numbers all reject at the `u8` parse step (no `i32` cast to silently round-trip negatives).

4. **No code change in P014:** INV-23 is a DOCTRINE formalisation of the constraint shipped in P012 + P013. The `parse_daily_cron` implementation in `src/core/register.rs` AND the parallel `parse_simple_cron` in `src/scheduler/macos.rs` are unchanged by P014. Future expansion to full 5-field cron is OUT OF SCOPE and would require a separate Tầng 1 phiếu that updates: (a) `parse_daily_cron` → renamed/replaced with `parse_cron_expression` (and same treatment for `parse_simple_cron`), (b) `RegisterIntent` extended with a `cron_expr: String` field OR a new variant enum, (c) `MacosScheduler::register` plist generation forced to error on non-daily expressions (launchd cannot represent them), (d) `CrontabScheduler::register` permitted to pass the full expression through, (e) `core::status::run` parser extended to render full-cron descriptors, (f) new INV-23 supersession entry documenting the Linux-extends-macOS-rejects asymmetry, (g) parser consolidation (unify `parse_daily_cron` + `parse_simple_cron` into one location — Phase 3.5+ candidate even without the full expansion).

**Why:** macOS launchd `StartCalendarInterval` does NOT have a native crontab parser — it accepts Hour + Minute (+ Weekday + Day, but advisory-cron Phase 1 chose daily form per INV-11 precedent). To preserve a single `RegisterIntent` cross-platform and avoid the foot-gun of "this expression works on Linux but errors on macOS", Phase 3 holds the daily-form line. The asymmetry is acknowledged + documented (this INV is the documentation) and explicitly deferred to a future phiếu when Sếp explicitly requests full 5-field. Sub-mechanism A "ship ≠ chạy": shipping full 5-field Linux WITHOUT the macOS rejection path = silent footgun (a Sếp dogfood expression works on the Linux box, then silently errors when Sếp tries the same config on a Mac).

**Implementation (Phase 3.2 — P013, formalised in P014):** Two parsers currently enforce this invariant via separate codepaths:

- `src/core/register.rs::parse_daily_cron` — caller-side / domain-layer parser. Accepts `&str`, splits on whitespace, asserts exactly 5 fields, asserts fields 3/4/5 are literal `*`, parses fields 1/2 as `u8`, bounds-checks. Returns `(hour: u8, minute: u8)` tuple (or `anyhow::Result` error chain). Called from `cli/register.rs` BEFORE `core::register::run` invocation; the validated tuple becomes `RegisterIntent.hour` + `.minute`.
- `src/scheduler/macos.rs::parse_simple_cron` — scheduler-internal parser (macOS-only). Same daily-form constraint, same bounds-check; used inside `MacosScheduler::*` to re-validate expressions before plist emission. Independent codepath from `parse_daily_cron` (both enforce the same INV-23, but a Worker changing one MUST verify the other still matches — Phase 3.5+ consolidation candidate per sub-rule 4 item (g)).

Linux scheduler emits `format!("{minute} {hour} * * *", ...)` — guaranteed daily-form by construction.

**Trust boundary:** the cron expression is user-controlled (CLI flag `--schedule` OR config file `[schedule]` block). All validation happens at parse time before any side effect (plist write OR crontab pipe). An invalid expression produces a parse error + exit 1 with NO file mutation. The parse is a pure function of input string + a constant grammar — no external service, no shell-out, no allocation beyond the parsed tuple.

**Trigger keywords:** `parse_daily_cron` call sites (`src/core/register.rs`), `parse_simple_cron` call sites (`src/scheduler/macos.rs` — parallel daily-form parser, same constraint, separate codepath), `parse_cron_expression` (NEW — future phiếu), new `RegisterIntent` fields touching cron representation, `StartCalendarInterval` plist key additions (Weekday, Day keys would expand macOS form), `cron::Schedule` or `croner` or `cron-parser` crate additions (would imply full 5-field — out of scope Phase 3).

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE (unit tests in `src/core/register.rs::tests` + `src/scheduler/macos.rs::tests` for daily-form acceptance + non-daily rejection + bounds-check). Giám sát soi PR diff for cron-expression changes; if PR adds a new cron parser, extends `RegisterIntent` cron shape, extends `StartCalendarInterval` plist keys, OR modifies `parse_daily_cron` / `parse_simple_cron` without an accompanying INV-23 supersession entry, flag as INV-23 violation.

---
```

**Lưu ý:**
- INV-23 H3 inserts directly after INV-22's closing `---`. No blank line between the `---` and the new H3 (standard markdown convention — `---` is a horizontal rule, H3 follows on the next line).
- The trailing `---` here precedes the existing `## How INV are checked` H2 (file line ~346). Sanity: after Task 1 + Task 2, structure is:
  ```
  ### INV-21 ... (existing)
  ---
  ### INV-22 ... (Task 1 NEW)
  ---
  ### INV-23 ... (Task 2 NEW)
  ---
  ## How INV are checked
  ```
- V2 wording change vs V1: Statement extended with `parse_simple_cron` mention; sub-rule 3 generalised to "Both parsers"; sub-rule 4 item (a) updated + item (g) added (consolidation candidate); Implementation paragraph restructured into 2 bullets covering both parsers; Trigger keywords extended with `parse_simple_cron` + scheduler.macos location; Giám sát PR-diff check extended with `parse_simple_cron`.

---

### Task 3 — CREATE `.github/workflows/ci.yml` (new file)

**File:** `.github/workflows/ci.yml` (NEW — does not exist per Anchor #1)

**Tìm:** N/A — new file creation.

**Thay bằng / Thêm (FULL FILE BODY — Worker copy verbatim):**

```yaml
# advisory-cron CI — cross-OS matrix
# Created by P014 (Phase 3.3). Runs on every push + PR against main.
#
# Jobs:
#   - macos-latest (Apple Silicon GHA runner — exercises `#[cfg(target_os = "macos")]`-gated
#     scheduler::macos unit tests; these use the in-module `NoopLaunchctl` test impl —
#     no real `launchctl bootstrap` shell-out per INV-10 / P012 design.
#     Integration tests in `tests/cli_register.rs` spawn the compiled binary and CANNOT
#     inject NoopScheduler (per the test file's own header docstring) — macOS sandbox
#     safety relies instead on integration tests gracefully degrading when
#     `launchctl bootstrap` is unavailable on the GHA runner. Observe-first per DP4;
#     if an integration test fails on GHA macOS, a Tầng 2 follow-up adds
#     `#[ignore]` or `if: matrix.os != 'macos-latest'` selectively.)
#   - ubuntu-latest (exercises `#[cfg(target_os = "linux")]`-gated scheduler::linux unit tests +
#     tests/cli_register_linux.rs integration tests via mock `crontab` binary in PATH).
#
# Sub-mechanism B (capability gap) — Linux job pre-step `which crontab` fails loud if `cron` package
# is missing from the ubuntu-latest runner image. If failure: DISCOVERY_REPORT + follow-up phiếu adds
# `sudo apt-get install -y cron` step.

name: ci

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    name: cargo test + clippy + fmt (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [macos-latest, ubuntu-latest]

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain (stable)
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Show Rust version
        run: |
          rustc --version
          cargo --version

      - name: Linux — verify `crontab` capability (Sub-mech B)
        if: matrix.os == 'ubuntu-latest'
        run: |
          which crontab
          crontab -l 2>&1 || echo "(no crontab for runner user — expected, exit 1 acceptable)"

      - name: cargo fmt --check
        run: cargo fmt --all -- --check

      - name: cargo build --release
        run: cargo build --release

      - name: cargo test --all
        run: cargo test --all

      - name: cargo clippy --all-targets -- -D warnings
        run: cargo clippy --all-targets -- -D warnings
```

**Lưu ý:**
- `fail-fast: false` — if one OS job fails, the other still completes so we see both signals per push.
- `dtolnay/rust-toolchain@stable` is the de-facto standard Rust toolchain installer for GitHub Actions (Anchor #12 `[unverified]` — Worker MAY swap to `actions-rust-lang/setup-rust-toolchain@v1` if context7 reveals it as current best practice; both acceptable for this minimal use).
- The Linux `which crontab` step uses `if: matrix.os == 'ubuntu-latest'` so the same job-step block runs on both OS without duplicating into 2 jobs. macOS skips this step.
- `cargo fmt --check` BEFORE build — fail fast on style.
- NO caching (`actions/cache@v4`) per Constraint #4 — Phase 4+ optimisation.
- NO `actions/upload-artifact` for the release binary — not a release workflow (Phase 6+ scope).
- NO `release` job, NO `tag-push` trigger, NO `cargo publish` step — Phase 6+ scope.
- File ends with a single trailing newline (yaml convention; `cargo fmt`-style cleanliness applies to yaml too).
- V2 wording change vs V1: macOS-job comment block rewritten — removes the false "tests/cli_register.rs integration tests via NoopScheduler injection" claim; replaces with accurate language separating (a) `scheduler::macos` unit tests using `NoopLaunchctl` from (b) integration tests that spawn the binary and rely on graceful sandbox degradation. Observe-first per DP4 explicit.

---

### Task 4 — EDIT `docs/ARCHITECTURE.md` §Phase status + APPEND §CI matrix subsection

**File:** `docs/ARCHITECTURE.md`

**Edit 4a — §Phase status row for Phase 3.3:**

**Tìm:** the line `  - ⏸️ **Phase 3.3** (P014): CI matrix (macOS + Linux parallel jobs). Deferred.` (file line 409 per Architect Read).

**Thay bằng:**

```markdown
  - ✅ **Phase 3.3** (P014): INV-22 (`crontab` shell-out boundary — 5 sub-rules parallel to INV-10/12/17) + INV-23 (cron expression daily-form invariant cross-platform) appended to `docs/security/INVARIANTS.md`. GitHub Actions CI workflow `.github/workflows/ci.yml` created — `matrix: os: [macos-latest, ubuntu-latest]` running `cargo fmt --check`, `cargo build --release`, `cargo test --all`, `cargo clippy --all-targets -- -D warnings` on each. Linux job pre-step `which crontab` (Sub-mechanism B capability smoke). No code change; doctrine + CI infra only.
```

**Edit 4b — APPEND new §CI matrix subsection (after §MCP surface, before §Heartbeat schema).**

**Tìm:** the `---` separator before `## Heartbeat schema` (file line ~319).

**Thay bằng (insert NEW subsection BEFORE the `---` separator; the `---` then closes the new section):**

```markdown
---

## CI matrix (Phase 3.3 — P014)

advisory-cron uses a 2-OS GitHub Actions matrix to guarantee cross-OS health on every push + PR against `main`:

| Job | Runner | Tests gated | Tests common |
|-----|--------|-------------|--------------|
| `test (macos-latest)` | macos-latest (Apple Silicon) | `scheduler::macos` unit tests (use in-module `NoopLaunchctl` — no real `launchctl`) + `tests/cli_register.rs` (`#[cfg(target_os = "macos")]`, spawn compiled binary) | `core::*`, `config`, `runner`, `heartbeat`, `alert`, `scheduler::mod` (cross-OS), `mcp::*` |
| `test (ubuntu-latest)` | ubuntu-latest | `scheduler::linux` unit tests + `tests/cli_register_linux.rs` (`#[cfg(target_os = "linux")]`) | same |

Each job runs (in order, fail-fast within the job):
1. `cargo fmt --all -- --check`
2. `cargo build --release`
3. `cargo test --all`
4. `cargo clippy --all-targets -- -D warnings`

`fail-fast: false` at the matrix level — if macOS fails we still see the Linux signal (and vice versa) per push.

**Sub-mechanism B capability check (Linux only):** before `cargo test`, the Linux job runs `which crontab` to verify `cron` package is present on the `ubuntu-latest` runner. If absent, the step fails loud — a follow-up phiếu would add `sudo apt-get install -y cron`. The macOS job has no equivalent pre-step because `launchctl` is part of macOS itself (no install step possible).

**macOS GHA sandbox safety model (two layers):** P012 design intent splits the macOS test surface into two:
1. **Unit tests inside `src/scheduler/macos.rs`** — use the in-module `NoopLaunchctl` test impl. These NEVER shell-out to real `launchctl` and run cleanly on any host (Sếp's Mac, GHA `macos-latest` sandbox, future Linux dev box building with `--target x86_64-apple-darwin`).
2. **Integration tests in `tests/cli_register.rs`** — spawn the compiled binary (per the test file's own header docstring, these CANNOT inject `NoopScheduler`). On the GHA `macos-latest` runner, `launchctl bootstrap` may be unavailable or restricted; integration tests rely on graceful degradation (exit non-zero with a sandbox-related error message rather than panic). DP4 (observe-first): the first CI run reveals which integration tests fail on the sandbox; a Tầng 2 follow-up adds `#[ignore]` or `if: matrix.os != 'macos-latest'` step skip selectively. The only real `launchctl` paths exercised end-to-end live in Sếp's dogfood + manual `advisory-cron register` on a real Mac, not in CI.

**Why no caching, no release artifact upload, no tag-triggered release:** advisory-cron is solo + small binary (~3.9 MB release). Per-job CI ~3 minutes is acceptable. Phase 4+ would add `actions/cache@v4` if CI cost matters; Phase 6+ would add release-on-tag workflow if `cargo publish` is in scope.
```

**Lưu ý:**
- The §CI matrix subsection sits BETWEEN §MCP surface (ends ~line 318) and §Heartbeat schema (starts ~line 320). The `---` separator at line 319 is preserved as the closing rule of the new §CI matrix section (Worker inserts the new section text BEFORE the existing `---`, so the existing `---` then naturally closes §CI matrix and precedes §Heartbeat schema).
- No other §Phase status entries edited (3.2 / 3.1 / 2.x / 1.x text untouched).
- No §Cron mechanism edits (split already done by P013 per Anchor #5).
- No §Modules table edits (P013 already updated `src/scheduler/linux.rs` row).
- V2 wording change vs V1: §CI matrix subsection — the macOS-safety paragraph is replaced with a 2-layer model accurately describing unit tests (`NoopLaunchctl` in-module) vs integration tests (spawn binary, graceful degradation, observe-first); removes false `NoopScheduler` injection claim. Table row for macOS-job also revised to mention `NoopLaunchctl` explicitly.

---

### Task 5 — EDIT `docs/CHANGELOG.md` — prepend P014 entry

**File:** `docs/CHANGELOG.md`

**Tìm:** the line `## 2026-05-28 — P013: Phase 3.2 — Linux cron-tab impl (sync stdlib, V2)` (file line 9 per Anchor #11).

**Thêm BEFORE that line (newest at top):**

```markdown
## 2026-05-28 — P014: Phase 3.3 — INV-22 + INV-23 formal entries + cross-OS CI matrix

**Phiếu:** P014 (Tầng 1 — docs/security/INVARIANTS.md doctrine surface + new GitHub Actions workflow file)

**Scope:** Formalise INV-22 (`crontab` shell-out boundary — 5 sub-rules) + INV-23 (cron expression daily-form invariant) in `docs/security/INVARIANTS.md`. Create `.github/workflows/ci.yml` 2-OS matrix (`macos-latest` + `ubuntu-latest`). ARCHITECTURE.md §Phase status 3.3 ⏸️ → ✅ + new §CI matrix subsection. No code change (zero `src/` diff).

**INV-22 (5 sub-rules):**
1. Discrete-arg shell-out (no `sh -c` interpolation against `crontab`).
2. Label allowlist 2-point — pre-flight in `core::*::run` (INV-12 existing, 3 inline copies at `src/core/{register,unregister,status}.rs`) + defense-in-depth `super::is_valid_label` first line of each `CrontabScheduler::*` method (P013 shipped at `src/scheduler/mod.rs`, scheduler-boundary single source). 4-location reality documented in sub-rule 2 Note + Phase 3.5+ consolidation deferred.
3. Tag-only filter idempotency (`# advisory-cron: <label>` substring match — preserve user's other crontab lines).
4. Sync `std::process::Command` only (V2 P013 lesson — no nested-runtime panic from `#[tokio::main]` context).
5. TOCTOU read-modify-write race acknowledged + deferred Phase 3.5+ (cross-ref ARCHITECTURE.md §Cron mechanism Linux).

**INV-23 (Option A — conservative):** Documents daily-form `M H * * *` cron expression invariant for BOTH platforms. Linux cron-tab native full 5-field support intentionally constrained to daily-form for cross-platform `RegisterIntent` shape parity. Both parsers (`src/core/register.rs::parse_daily_cron` + `src/scheduler/macos.rs::parse_simple_cron`) enforce daily-form via separate codepaths — Phase 3.5+ consolidation candidate. Future full 5-field expansion deferred to separate phiếu (would require `RegisterIntent` extension + 7 numbered scope items per INV-23 sub-rule 4).

**CI matrix:**
- `matrix: os: [macos-latest, ubuntu-latest]`, `fail-fast: false`.
- Each job: `cargo fmt --check` → `cargo build --release` → `cargo test --all` → `cargo clippy --all-targets -- -D warnings`.
- Linux job pre-step `which crontab` (Sub-mechanism B capability smoke — fails loud if `cron` package missing).
- macOS sandbox safety model: unit tests in `scheduler::macos` use `NoopLaunchctl` (no real `launchctl`); integration tests in `tests/cli_register.rs` spawn binary + rely on graceful degradation when `launchctl bootstrap` unavailable on GHA — observe-first DP4.
- Toolchain: `dtolnay/rust-toolchain@stable` with `clippy, rustfmt` components.
- Checkout: `actions/checkout@v4`.

**Doctrine surface counts:** INV total 21 → 23 (+INV-22 +INV-23).

**Debate Log:** 2 turns (V1 DRAFT → Worker CHALLENGE Turn 1 → Architect RESPOND Turn 2 → V2 final). 3 ACCEPT on factual-accuracy objections (O1.1 `parse_simple_cron` Trigger gap, O1.2 `is_valid_label` 4-location reality, O1.3 ci.yml false `NoopScheduler` claim). No scope/design change.

**Test count delta:** 143 (P013 baseline) → 143 (no new tests in P014 — doctrine + CI only). First successful CI run is the new "pass" signal.

**Surfaced debt item:** `[DEBT] core layer is_valid_label consolidation (Phase 3.5+ Tầng 2)` — 3 inline copies in `src/core/{register,unregister,status}.rs` predate P013's scheduler-layer consolidation. To be added to `docs/BACKLOG.md` "Open backlog" by Quản đốc post-merge.

**No code change, no `Cargo.toml` change, no dep change, no CLI / MCP / config schema change.** P014 is the doctrine + infra phiếu mirroring P007's role for Phase 1 (post-ship polish — but for Phase 3 it lands BEFORE Phase 3.4 README per BACKLOG sequencing).

---
```

**Lưu ý:**
- Date `2026-05-28` matches BACKLOG.md sprint start + P013 entry date. If Worker EXECUTE crosses midnight UTC, use the actual commit date.
- The trailing `---` separator follows existing CHANGELOG entries' convention (line 31 after P013 entry — Architect Read confirms).
- V2 wording change vs V1: INV-22 sub-rule 2 bullet updated to mention 4-location reality + Phase 3.5+ deferral; INV-23 paragraph extended with parallel parser note + 7 scope items (was 6); CI matrix bullet for macOS safety model rewritten; Debate Log summary added; surfaced debt item explicit.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `docs/security/INVARIANTS.md` | Task 1: APPEND INV-22 (5 sub-rules + 4-location Note in sub-rule 2) after INV-21. Task 2: APPEND INV-23 (daily-form invariant + both-parsers Implementation) after INV-22. ~115 LOC docs. |
| `.github/workflows/ci.yml` | Task 3: CREATE new file (2-OS matrix, 4 cargo steps, Linux `which crontab` smoke, accurate macOS sandbox comment). ~55 LOC yaml. |
| `docs/ARCHITECTURE.md` | Task 4a: edit Phase 3.3 status row. Task 4b: APPEND new §CI matrix subsection (with 2-layer macOS safety model). ~35 LOC docs. |
| `docs/CHANGELOG.md` | Task 5: prepend P014 entry (with V2 wording — 4-location reality + parallel parsers + Debate Log summary). ~35 LOC docs. |

**Estimated LOC total:** ~225 docs + ~55 yaml = ~280 LOC (V1 was ~270; +10 LOC absorbed by the 3 V2 wording revisions — within rounding). Zero `src/` LOC. Zero `Cargo.toml` LOC.

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/scheduler/linux.rs` | Task 0 step 5 grep — INV-22 references existing P013 code; verify `is_valid_label` first-line check pattern still present; ZERO edits. |
| `src/scheduler/mod.rs` | Task 0 step 5 grep — verify `pub fn is_valid_label` exists at line ~68; INV-22 cites this fn as scheduler-boundary source; ZERO edits. |
| `src/scheduler/macos.rs` | INV-12 + INV-22 share scheduler-layer helper; INV-23 cites `parse_simple_cron` at line ~111; verify both present; ZERO edits. |
| `src/core/register.rs` | Task 0 step 4 grep `parse_daily_cron` at line ~111 + line ~37 `is_valid_label` inline copy; INV-23 + INV-22 cite both; ZERO edits. |
| `src/core/unregister.rs` | Task 0 step 5 grep `is_valid_label` at line ~35 (1 of 3 inline copies, INV-22 sub-rule 2 Note); ZERO edits. |
| `src/core/status.rs` | Task 0 step 5 grep `is_valid_label` at line ~99 (1 of 3 inline copies, INV-22 sub-rule 2 Note); ZERO edits. |
| `tests/cli_register.rs` | Task 0 step 3 grep — verify `NoopScheduler` NOT used (header docstring forbids it); CI yaml comment + §CI matrix subsection cite this constraint; ZERO edits. |
| `Cargo.toml` | Task 0 step (none — no edits expected); Verification Trace E confirms `git diff Cargo.toml` empty. |
| `Cargo.lock` | Same as Cargo.toml — no edits, no diff. |
| All other `src/**/*.rs`, `tests/**/*.rs`, `phieu/**`, `.claude/agents/**`, `CLAUDE.md`, `docs/PROJECT.md`, `docs/SOUL.md`, `docs/CHARACTER*.md` | Out of scope — P014 is docs/infra only. |
| `docs/BACKLOG.md` | After phiếu ships: Worker moves P014 item from Active sprint "Phiếu phác" to "Recently shipped" — but this is BACKLOG maintenance per CLAUDE.md §Workflow rule 2, not a P014 EDIT task. Worker does this at sprint-close, not per-phiếu. P014 does NOT edit BACKLOG.md inline. Quản đốc adds `[DEBT] core layer is_valid_label consolidation` to "Open backlog" post-merge (separate operation, not in this phiếu). |
| `docs/RULES.md` | Does not exist yet per project file glob; INV-22/23 doctrine lives in INVARIANTS.md only. |

---

## Luật chơi (Constraints)

1. **Zero `src/` diff.** `git diff src/` after EXECUTE must be empty. P014 is docs + workflow file only. If Worker is tempted to "improve" any src/ code while in the file — STOP, write DISCOVERY_REPORT note, do NOT touch. (Includes the `is_valid_label` 4-copy consolidation — DP7 explicitly defers this.)
2. **Zero `Cargo.toml` + `Cargo.lock` diff.** No new dep, no new feature flag, no toolchain bump. `git diff Cargo.toml Cargo.lock` empty.
3. **INV-22 + INV-23 text verbatim** from Task 1 + Task 2 (V2 wording). Do NOT paraphrase or "tighten" the prose — the language is calibrated against INV-10/12/17/21 parallel format AND against Worker Turn 1 evidence (4-location reality, parallel parsers). Substitute only narrative line-number references if Task 0 step 4 or 5 reveals drift.
4. **CI workflow yaml verbatim** from Task 3 (V2 wording — macOS comment block accurate). `dtolnay/rust-toolchain@stable` + `actions/checkout@v4` are the chosen versions; Worker MAY swap to a newer minor (`@v4.1.1` etc.) if `actionlint` or context7 flags an explicit reason — note in Discovery. No caching, no release steps, no upload-artifact, no scheduled trigger (only push + pull_request to main).
5. **ARCHITECTURE.md §CI matrix subsection placed AFTER §MCP surface, BEFORE §Heartbeat schema.** Worker preserves the existing `---` separator at line ~319 (it becomes the closing rule of the new §CI matrix section). V2 macOS-safety 2-layer model verbatim. Other §Phase status entries (3.2 / 3.1 / 2.x / 1.x) unchanged.
6. **CHANGELOG.md P014 entry inserted at line 9 (BEFORE P013 entry).** Newest at top per file header convention. Date `2026-05-28` (adjust if Worker EXECUTE crosses UTC midnight). V2 wording verbatim.
7. **No new INV beyond INV-22, INV-23.** If Worker spots a candidate INV during EXECUTE (e.g. "crontab line max length boundary"), write to DISCOVERY_REPORT for future phiếu — do NOT inline-add to INVARIANTS.md.
8. **DRAFT-mode anchors `[needs Worker verify]` MUST flip to `[verified]` or `[unverified-CI-smoke]` in Discovery Report.** Specifically: Anchor #4 + #7 + #8 are now `[verified — Turn 1 V2 correction]` (Worker Turn 1 evidence absorbed into phiếu); Worker re-confirms in Task 0 steps 3, 4, 5. Anchor #10 remains `[needs Worker verify — first CI run]` — flip to `[verified-CI-smoke]` or `[failed-CI-smoke]` per first run + add to Discovery.
9. **First CI run observation block in Discovery Report.** When the first push to PR branch triggers `ci.yml`, Worker records:
   - macOS job exit (green/red + which step failed if red)
   - Linux job exit (same)
   - `which crontab` step output (Sub-mechanism B Anchor #9 verification)
   - macOS integration test behavior on GHA sandbox (graceful degradation vs panic — Anchor #10 verification)
   - Total wall-clock per job (informational, no SLA)
   - Any deprecation warnings from `actions/checkout` / `dtolnay/rust-toolchain` (informational, no action)
10. **No `unsafe { }` block.** Standing hard line per CLAUDE.md INV-6 — P014 docs+infra has no `unsafe` surface; this constraint is the standing rule reminder, not a P014-specific concern.
11. **No README edits.** P015 (Phase 3.4) covers README quick-start polish. If Worker spots a stale README claim during EXECUTE → DISCOVERY_REPORT note for P015 to pick up.
12. **No core-layer `is_valid_label` consolidation.** DP7 explicitly defers this to Phase 3.5+ Tầng 2. Worker MUST NOT consolidate the 3 inline copies in `src/core/{register,unregister,status}.rs` even if "while I'm here" tempts. Quản đốc tracks via `[DEBT]` BACKLOG entry post-merge.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings (sanity — no src/ changes; should pass trivially).
- [ ] `cargo test --all` — 143 pass (P013 baseline; no test count delta in P014).
- [ ] `cargo clippy --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — no diff.
- [ ] `git diff src/ Cargo.toml Cargo.lock` — empty.
- [ ] `git diff docs/security/INVARIANTS.md` — shows INV-22 + INV-23 additions only (no edits to INV-1..21).
- [ ] `grep -c "^### INV-" docs/security/INVARIANTS.md` — exactly 23.
- [ ] `grep -c "parse_simple_cron" docs/security/INVARIANTS.md` — at least 2 (Statement + Implementation + Trigger keywords). Confirms O1.1 fix landed.
- [ ] `grep -c "FOR THE SCHEDULER BOUNDARY\|scheduler boundary" docs/security/INVARIANTS.md` — at least 1 in INV-22 sub-rule 2. Confirms O1.2 fix landed.
- [ ] `grep -c "NoopScheduler" .github/workflows/ci.yml` — exactly 1 (only in the negative phrasing "CANNOT inject NoopScheduler"); ZERO occurrences of the "via NoopScheduler injection" V1 phrase. Confirms O1.3 fix landed.

### Manual Testing
- [ ] After push to PR branch, `gh run list --workflow=ci.yml --limit 1` shows 1 run.
- [ ] `gh run view <run-id>` shows 2 jobs (`test (macos-latest)`, `test (ubuntu-latest)`).
- [ ] Both jobs green ✅ — or DISCOVERY_REPORT documents failure mode + Architect/Sếp decides Tầng 2 follow-up.
- [ ] Linux job step `Linux — verify crontab capability` log line contains `/usr/bin/crontab` (or path equivalent).
- [ ] Render `docs/security/INVARIANTS.md` in GitHub markdown preview — INV-22 + INV-23 sections render with code fences + bullet structure intact (no broken table / list); 4-location Note in INV-22 sub-rule 2 renders as a sub-bullet under Point 2.

### Regression
- [ ] `cargo run --release -- register --label p014-smoke --schedule "0 9 * * *"` on Linux WSL2 (Worker dev box) — exit 0, 1 tagged line in `crontab -l`. (Re-verifies P013 Sub-mechanism A still holds — no doctrine entry breaks runtime.)
- [ ] `cargo run --release -- unregister --label p014-smoke` — exit 0, tag removed.
- [ ] `cargo run --release -- status --label p014-smoke --json` after unregister — `plist_loaded: false`.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P014 entry prepended (Task 5, V2 wording).
- [ ] `docs/ARCHITECTURE.md` — §Phase status 3.3 updated ✅ + §CI matrix subsection appended (Task 4a + 4b, V2 macOS 2-layer safety model).
- [ ] `docs/security/INVARIANTS.md` — INV-22 + INV-23 appended verbatim (Task 1 + Task 2, V2 wording).
- [ ] `README.md` — NOT touched (P015 owns).
- [ ] `docs-gate --all --verbose` — pass (or `docs-gate` MCP `check_all`).

### Discovery Report
- [ ] `docs/discoveries/P014.md` — full report written per CLAUDE.md DISCOVERY REPORT format.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry prepended (newest at top).
- [ ] Sub-mechanism A–E Verification Trace filled (table above).
- [ ] "First CI run observations" subsection in Discovery (Constraint #9).
- [ ] Anchor #4 + #7 + #8 marker flips recorded (V1 `[needs Worker verify]` → V2 `[verified — Turn 1 V2 correction]`, with Worker Turn 0 re-confirmation citations).
- [ ] Anchor #10 flip recorded (`[needs Worker verify — first CI run]` → `[verified-CI-smoke]` or `[failed-CI-smoke]` per first run).
- [ ] Debate Log surfaced debt item explicit: `[DEBT] core layer is_valid_label consolidation (Phase 3.5+ Tầng 2)`. Quản đốc confirms BACKLOG "Open backlog" entry added post-merge.

---

## Decision Log (Architect explicit choices for Worker CHALLENGE Turn 1 review)

| DP | Choice | Rationale | Worker CHALLENGE? |
|----|--------|-----------|-------------------|
| **DP1 — INV-22 sub-rule count** | **5 sub-rules** (discrete-arg + label-allowlist-2pt + tag-only-filter + sync-stdlib + TOCTOU-deferred) | Parallel to INV-21 (4 sub-rules) and INV-20 (4 sub-rules) density. Each sub-rule maps to a P013 enforcement point already shipped. TOCTOU explicitly named (DP6 — embedded as sub-rule 5, not separate ARCHITECTURE note). | Turn 1: APPROVE in principle (sub-rule 2 wording challenged → O1.2 → ACCEPT → V2 qualified). |
| **DP2 — INV-23 scope** | **Option A (conservative — daily-form for both platforms)** | Phase 3 BACKLOG acceptance criterion #4 (`parse "M H * * *" → "daily at HH:MM"`) is daily-form. Option B (Linux full 5-field) = scope creep into Tầng 1 design debate (`RegisterIntent` shape, parser rewrite, 6 new sub-tasks). P014 is docs+CI phiếu — code change rejected. | Turn 1: APPROVE. Trigger keyword gap on `parse_simple_cron` → O1.1 → ACCEPT → V2 extended. |
| **DP3 — CI workflow file** | **CREATE from scratch** | `.github/workflows/` directory does not exist (Anchor #1 ✅). P014 is the first CI workflow. | Turn 1: APPROVE. |
| **DP4 — macOS GHA runner: integration tests** | **Run all macOS-gated tests (observe first; no preemptive skip)** — relies on (a) unit tests using in-module `NoopLaunchctl` (no real `launchctl`) and (b) integration tests gracefully degrading when `launchctl bootstrap` unavailable on GHA sandbox | Per Anchor #4 (V2 corrected per Worker Turn 1: integration tests spawn binary, CANNOT inject `NoopScheduler`; macOS safety is 2-layer per Task 3 yaml comment + §CI matrix subsection). If integration tests don't gracefully degrade → DISCOVERY + Architect Tầng 2 follow-up adds `#[ignore]` or `if: matrix.os != 'macos-latest'` selectively. | Turn 1: APPROVE with note → O1.3 (yaml comment lied about mechanism) → ACCEPT → V2 yaml comment + §CI matrix subsection + Anchor #4 + Giải pháp prose all rewritten with accurate 2-layer model. |
| **DP5 — `ubuntu-latest` `crontab` install** | **Assume present (no `apt-get install` step)** + add `which crontab` smoke pre-step as the safety net | GHA `ubuntu-latest` runner image ships `cron` package per default image conventions (Anchor #9 `[unverified]`). If smoke fails at first run → 1-line follow-up phiếu adds install step. | Turn 1: APPROVE. |
| **DP6 — TOCTOU placement** | **INV-22 sub-rule 5** (1-sentence statement in INV doctrine) + **cross-reference** to existing ARCHITECTURE.md §Cron mechanism Linux paragraph for full discussion | Avoids ARCHITECTURE.md duplicating INVARIANTS.md. ARCHITECTURE.md describes *what is*; INVARIANTS.md describes *what must not break*. The 1-sentence sub-rule says "TOCTOU acknowledged, hardening deferred Phase 3.5+" — sufficient for audit purposes. | Turn 1: APPROVE. |
| **DP7 — core-layer `is_valid_label` consolidation** *(NEW in V2 — emerged from O1.2)* | **DEFER to Phase 3.5+ Tầng 2.** Document 4-location reality in INV-22 sub-rule 2 Note + flag as `[DEBT]` BACKLOG entry post-merge. P014 does NOT consolidate. | Worker Turn 1 evidence: 4 `is_valid_label` implementations exist (scheduler::mod.rs + 3 in core/*.rs). P013 consolidated only the scheduler layer. Consolidating core too in P014 would: (a) violate Zero src/ diff constraint, (b) expand a docs+CI phiếu into a refactor, (c) skip a Tầng 1 design debate on whether `scheduler::is_valid_label` should be the crate-wide source OR a new `validation::label` module should hold it. Deferral is the principled choice. | V2 only. Worker Turn 2 may challenge if doctrine wording on the 4-location Note is unclear. |

---

## Risk + rollback

**Risks (in decreasing priority):**

1. **CI macOS job fails on first run** (Anchor #10 false positive) — some `#[cfg(target_os = "macos")]` integration test in `tests/cli_register.rs` doesn't gracefully degrade when `launchctl bootstrap` is unavailable on GHA sandbox, it panics instead. Mitigation: Discovery + Tầng 2 follow-up phiếu adds `#[ignore]` selectively or `if: matrix.os != 'macos-latest'` step skip. P014 still ships; DISCOVERY documents.
2. **CI Linux job `which crontab` fails** (Anchor #9 false negative) — `ubuntu-latest` runner image variant doesn't ship `cron`. Mitigation: 1-line follow-up phiếu adds `sudo apt-get install -y cron` before the smoke step.
3. **INV-22 / INV-23 V2 wording catches a NEW corner case** Architect + Worker Turn 1 missed (e.g. INV-22 sub-rule 3 tag-filter "substring match" is too loose — accidentally matches a user line containing the substring as data; OR the 4-location Note inadvertently legitimises the inconsistency). Mitigation: Worker CHALLENGE Turn 2 reviews V2 text; if objection → V3 refine (within 3-turn cap). Worst case: post-ship Tầng 2 phiếu tightens wording.
4. **`dtolnay/rust-toolchain@stable` action gets deprecated mid-sprint** (Anchor #12 minor risk). Mitigation: Worker swaps to `actions-rust-lang/setup-rust-toolchain@v1` in Discovery follow-up; no rollback needed (CI workflow file is forward-only mutable per push).
5. **Surfaced [DEBT] item not tracked.** If Quản đốc forgets to add `[DEBT] core layer is_valid_label consolidation` to BACKLOG "Open backlog" post-merge → consolidation drifts further. Mitigation: Discovery Report explicitly notes the debt item + Worker reminds Quản đốc at sprint-close.

**Rollback:**

- `git revert <P014-commit>` undoes all 4 file changes atomically. CI workflow file deleted via revert → no CI runs on subsequent pushes until re-introduced. INV-22 + INV-23 entries removed from doctrine (audit surface returns to INV-1..21 baseline).
- No state file rollback needed (P014 touches no runtime state).
- No `Cargo.toml` / `Cargo.lock` rollback (zero diff per Constraint #2).
- BACKLOG.md sprint table entry for "Phase 3.3 P014" stays as "phiếu phác" (Worker did not move it to "Recently shipped" mid-sprint per Constraint #11).
- [DEBT] item also rolled back if added post-merge before revert; Quản đốc re-evaluates whether to add it back independently.

---

## Estimated effort

- Task 0 (Worker verify, V2 re-confirm): 15 min
- Task 1 + Task 2 (INV-22 + INV-23 V2 verbatim copy + line-number substitution): 20 min (V2 adds the 4-location Note + both-parsers Implementation paragraph — slight upward bump)
- Task 3 (CI workflow yaml V2 verbatim copy): 10 min
- Task 4 (ARCHITECTURE.md 2 edits, V2 macOS 2-layer safety model): 15 min (slight upward bump)
- Task 5 (CHANGELOG.md prepend, V2 wording): 5 min
- Local cargo passes + push + observe first CI run: 30 min
- Discovery Report write + DISCOVERIES.md index + [DEBT] flag to Quản đốc: 15 min

**Total: ~110 min (1.8h).** Pure docs+infra phiếu, no debugging surface. V2 adds ~10 min vs V1 estimate for the additional wording revisions absorbed.
