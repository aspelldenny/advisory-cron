# PHIẾU P013: Linux crontab impl — replace `CrontabScheduler` stub with real `crontab -l/-` injection

> **Loại:** feat
> **Tầng:** 1
> **Phase:** 3.2
> **Ưu tiên:** P1
> **Branch:** `feat/P013-linux-crontab-impl`
> **Ảnh hưởng:** `src/scheduler/linux.rs` (stub → real impl), `src/scheduler/mod.rs` (shared `is_valid_label` helper), `src/cli/register.rs` (empty-plist-path render gate — P012 watch-item closure), `tests/cli_register_linux.rs` (new — mock `crontab` binary integration), `docs/ARCHITECTURE.md` §Cron mechanism (split into "macOS launchd" + "Linux crontab" subsections), `docs/CHANGELOG.md`
> **Dependency:** P012 (merged 2026-05-28 — trait surface + Linux stub shipped)
> **Tier-1 reason:** Real impl of cross-OS scheduler boundary surface. Touches NEW security boundary (`crontab` shell-out + stdin pipe = INV-22 new) AND extends existing CLI render path (closes P012 watch-item). Per RULES.md "Security boundary touched → AUTO Tầng 1". INV-22 documents in P014 (deferred), but enforcement ships here.

---

## Context

### Vấn đề hiện tại

P012 (Phase 3.1) extracted `Scheduler` trait + landed Linux `CrontabScheduler` as a stub: every method `bail!("Phase 3.2 — P013 chưa ship")`. Linux WSL2 builds cleanly (4.7MB binary, 0 warnings) but `advisory-cron register` / `unregister` / `status` exit non-zero with the stub message — Phase 3 acceptance gate (BACKLOG: `crontab -l | grep advisory-cron` after register = 1 line) cannot be satisfied.

Phase 3.2 fills the stub with the real `crontab -l/-` injection flow + introduces a new security boundary (`crontab` shell-out, stdin-pipe write) that requires INV-22-grade label allowlist defense-in-depth mirroring INV-10/12/17 (the macOS `launchctl` boundary).

### Giải pháp

Replace the 3 `bail!` bodies in `src/scheduler/linux.rs` with real impls following the macOS pattern (parallel to `RealLaunchctl::{bootstrap, bootout, print}` shell-out wrappers):

- **`CrontabScheduler::register(&intent)`** — `crontab -l` (read user crontab via `std::process::Command::output()`; tolerate "no crontab for <user>" stderr by treating exit 1 with that stderr as empty input) → split stdout into lines → filter OUT any existing `# advisory-cron: <label>` tagged line (idempotent re-register replaces) → append new managed line `<minute> <hour> * * * <self_exe> run # advisory-cron: <label>` → pipe combined back via `crontab -` (stdin write, daemon path). Returns `RegisterReport { plist_path: None }` — Linux has no plist concept.

- **`CrontabScheduler::unregister(&label)`** — same `crontab -l` → filter out tagged line → `crontab -` pipe back. `UnregisterReport { was_registered: <whether-line-was-found> }`. Idempotent: missing tag → `was_registered: false`, exit Ok.

- **`CrontabScheduler::status(&label)`** — `crontab -l` → grep for `# advisory-cron: <label>` line → `SchedulerStatus { is_registered: <found>, raw_descriptor: Some(<matched line>) | None }`. Linux-specific `parse_next_fire` extension to read `M H * * *` from the descriptor is OUT OF SCOPE for P013 — `core::status::run` uses the macOS-format parser (`parse_next_fire` in `core/status.rs`); on Linux this returns `None` for `next_fire`. P014 INV-23 adds Linux-format parser. See "Out of scope" + "Edge cases" sections.

**Sync shell-out (V2 pivot per Worker Turn 1):** All 3 methods use `std::process::Command` (sync, blocking) — NOT `tokio::process::Command`. Rationale: `Scheduler` trait methods are sync (P012-shipped); blocking shell-out is the natural fit for ~3 short crontab calls (~10ms each). Zero new tokio feature flags (`io-util` absent in `Cargo.toml`), zero nested-runtime panic risk inside `#[tokio::main]` context. See DP6 + Debate Log Turn 1+2.

**INV-22 (2-point defense-in-depth) enforcement** — shipped here, **documented as new INV in P014**:

1. **Point 1 (pre-flight, caller side):** `core::register::run` / `core::unregister::run` / `core::status::run` already validate via existing `is_valid_label` helpers per INV-12. No new code needed at caller — existing macOS allowlist (ASCII alphanumeric + `-` + `_`) is a tight superset of any crontab metachar blacklist. Anchor #14 confirms call-site validation already exists pre-P013.

2. **Point 2 (defense-in-depth, inside each `CrontabScheduler` method):** new shared helper `scheduler::is_valid_label(&str) -> bool` in `src/scheduler/mod.rs` (cross-OS — covers both macOS allowlist AND defends Linux from any caller that forgot pre-flight). Each Linux method's first line: `if !crate::scheduler::is_valid_label(label) { bail!(...) }`. **macOS `scheduler::macos::is_valid_label_inline` is REPLACED by call to the new shared helper** — single source of truth, zero behavioral diff for macOS (same allowlist).

**P012 watch-item closure (Task 6):** `src/cli/register.rs` currently prints `"  plist: {plist_path}"` from `RegisterOutput.plist_path: PathBuf`. On Linux, `MacosScheduler` is not used → `core::register::run` does `report.plist_path.unwrap_or_default()` → empty `PathBuf` → renders `"  plist: "` blank. Fix: gate the render `if !output.plist_path.as_os_str().is_empty() { print "  plist: {...}" }`. Mirror for `cli/unregister.rs` if symmetric concern exists (verify in Task 0). This is a 1–2 line CLI render gate, NOT a config/schema/API change — closes the P012 P013 watch-item proactively here.

### Scope

**CHỈ sửa:**

1. **EDIT** `src/scheduler/mod.rs` — ADD shared `pub fn is_valid_label(label: &str) -> bool` (ASCII alphanumeric + `-` + `_`, non-empty). REMOVE `#[allow(dead_code)]` on `NoopScheduler` + `RegisterIntent` if Linux real impl exercises them (verify in Task 0 — Linux impl will use `RegisterIntent` actively).

2. **EDIT** `src/scheduler/macos.rs` — REPLACE local `is_valid_label_inline(label)` (added in P012 Task 2, used inside `MacosScheduler::unregister`) with call to `super::is_valid_label(label)`. Delete the local helper. Behavior: identical (same allowlist). Tests unaffected (defense-in-depth path still triggered).

3. **REPLACE** `src/scheduler/linux.rs` body — stub `bail!` → real **sync** impl per "Giải pháp" above (uses `std::process::Command`). Add `#[cfg(test)] mod tests` with 5+ unit tests covering register/unregister/status happy paths + invalid label rejection + idempotent re-register. (Mock `crontab` binary integration tests live in `tests/cli_register_linux.rs` — see Task 5.)

4. **EDIT** `src/cli/register.rs` — gate the `plist_path` print with `if !output.plist_path.as_os_str().is_empty()`. (Closes P012 P013 watch-item — 1-line cosmetic gate, no behavior change on macOS where path is always non-empty.) Verify `cli/unregister.rs` doesn't have symmetric render of an empty field — if yes, mirror; if no, skip.

5. **CREATE** `tests/cli_register_linux.rs` — new integration test file gated `#[cfg(target_os = "linux")]`. Mock `crontab` binary in tempdir + prepend to `PATH`. 5+ test cases (see Task 5 detail).

6. **EDIT** `docs/ARCHITECTURE.md` — split §Cron mechanism into 2 subsections: "macOS — launchd plist" (existing content verbatim) + "Linux — crontab injection" (new — 1 short subsection: command form `<minute> <hour> * * * <self_exe> run # advisory-cron: <label>`, idempotency via tag-filter, "no crontab for user" graceful path, INV-22 ref). Update `src/scheduler/linux.rs` row in §Modules from "stub" → "real impl". Update Phase status: Phase 3.2 ⏸️ → ✅.

7. **EDIT** `docs/CHANGELOG.md` — append P013 entry under 2026-05-XX (date Worker fills on commit day).

**KHÔNG sửa (OUT OF SCOPE — cứng, reject creep):**

- KHÔNG document INV-22 formally trong `docs/security/INVARIANTS.md` — P014 batch with INV-23 (cron expression validation) + CI matrix. P013 implements enforcement; P014 documents both INV.
- KHÔNG add GitHub Actions CI matrix — P014.
- KHÔNG update README Linux quick-start — P015 (Phase 3.4).
- KHÔNG đổi macOS behavior beyond replacing 1 local helper with shared helper (Task 2 — same allowlist, zero diff).
- KHÔNG đổi `heartbeat.rs` / `runner.rs` / `config.rs` / `alert.rs` / `core::run.rs` / `core::init.rs` / `core::config_path.rs`.
- KHÔNG đổi `Scheduler` trait surface (`register`/`unregister`/`status` signatures, `RegisterIntent`/`RegisterReport`/`UnregisterReport`/`SchedulerStatus` shapes ALL preserved).
- KHÔNG đổi MCP tool schema (`mcp/tools.rs` unchanged — schemas already cross-OS via trait abstraction).
- KHÔNG đổi CLI subcommand / flag / exit code.
- **KHÔNG add new dep, KHÔNG add new tokio feature flag.** `Cargo.toml` ZERO diff. V2 pivot to `std::process::Command` (sync) avoids needing `io-util` tokio feature (absent in current `Cargo.toml`). `std::process::{Command, Stdio}` + `std::io::Write` are stdlib (zero crate-feature flags). Verify Task 0 #5.
- KHÔNG support systemd timer — Phase 3.5 future wave.
- KHÔNG support full 5-field cron on Linux — P014 INV-23 + `RegisterIntent` extension.
- KHÔNG add Linux-format `parse_next_fire` parser — `next_fire: None` on Linux in P013 acceptable. P014 INV-23 adds `parse_cron_descriptor` parallel parser when full cron lands.
- KHÔNG modify `RegisterOutput`/`UnregisterOutput`/`StatusReport` public field set (JSON schema stability per MCP).

### Skills consulted

*(none — refactor + real impl, mock-binary pattern already in `tests/cli_run.rs` per Phase 1.4 P004. No new external library question. `std::process::Command::stdin(Stdio::piped())` + `std::io::Write::write_all` are stable stdlib API since Rust 1.0; Worker confirms via Task 0 anchor #7 + may invoke context7 if friction — expected zero friction.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/scheduler/linux.rs` currently has stub bodies `bail!("Phase 3.2 — P013 chưa ship")` for `register`/`unregister`/`status` | P012 phiếu Task 3 spec verbatim (lines 437–500) + P012 Discovery Report Sub-mech C row "0 functional refs" | `[unverified]` | ⚠️ Architect did not Read `src/scheduler/linux.rs` (envelope: no Read on src/). Worker `grep -c "bail!" src/scheduler/linux.rs` → expect 3 (one per method) before edit. P012 Discovery Sub-mech B row "129/129 pass" confirms stub is wired + tests pass — strong indirect evidence. |
| 2 | `RegisterIntent` shape: `{ label: String, hour: u8, minute: u8, self_exe: PathBuf, working_dir: PathBuf }` — passed as `&RegisterIntent` to `Scheduler::register` | P012 phiếu Task 1 spec lines 226–238 + P012 Discovery anchor #2 ✅ | `[verified via P012]` | ✅ Confirmed in P012 Discovery + ARCHITECTURE.md §Scheduler trait |
| 3 | `RegisterReport`, `UnregisterReport`, `SchedulerStatus` shapes (per P012 mod.rs) | P012 phiếu lines 240–262 | `[verified via P012]` | ✅ Confirmed |
| 4 | `RegisterOutput.plist_path: PathBuf` (NOT `Option<PathBuf>`) — populated via `report.plist_path.unwrap_or_default()` in `core::register::run` | P012 phiếu anchor #14 ✅ + Task 5 body lines 574–578 verbatim | `[verified via P012]` | ✅ Confirmed |
| 5 | `src/cli/register.rs:51` prints the plist_path (rendering blank on empty path is the P012 watch-item) | P012 Discovery "Edge cases" §`P013 watch-item` lines 42 verbatim | `[verified via P012]` | ✅ Confirmed — exact line cited |
| 6 | `tests/cli_run.rs` uses mock binary in `PATH` pattern (mock script in TempDir prepended to PATH) | P012 brief says "per `runner.rs` Phase 1.4 pattern" + BACKLOG Phase 3.2 phiếu phác references same | `[needs Worker verify]` | ⏳ Worker Turn 1 confirmed: `TempDir` used at lines 48/95/117/156 but **PATH-override pattern NOT in existing cli_run.rs**. Approach `std::process::Command::env("PATH", ...)` is still sound — no precedent in tree, Worker accepts (DP4 review). ✅ (verified, no precedent but pattern sound) |
| 7 | `std::process::Command::stdin(Stdio::piped())` + `child.stdin.unwrap().write_all(buf)` (sync stdlib pattern) is stable + canonical | `std::process::Stdio::piped` stable since Rust 1.0 + `std::io::Write` trait in `std::prelude` (no import needed for trait dispatch but explicit `use std::io::Write` clearer) | `[needs Worker verify]` | ⏳ Worker `cargo check` after typing the body — compiler is the verify. Zero feature flag concern (stdlib). |
| 8 | `crontab` binary present at `/usr/bin/crontab` on WSL2 Linux (BACKLOG Decision log 2026-05-28) | BACKLOG.md line 20 verbatim | `[unverified]` | ✅ Worker Turn 1 confirmed: `command -v crontab` → `/usr/bin/crontab`. |
| 9 | `crontab -l` exit code: 0 if user has crontab, 1 + stderr "no crontab for <user>" if not | POSIX convention + `man crontab` standard | `[needs Worker verify]` | ✅ Worker Turn 1 confirmed: exit 1, stderr `"no crontab for sep"`, substring `"no crontab"` matches lowercased. |
| 10 | `crontab -` reads stdin and replaces user crontab atomically (single-user-level atomicity; not multi-process) | `man crontab` standard POSIX | `[unverified]` | ⏳ Worker dogfood smoke (Task 0 final block — write 1 line via `crontab -`, then `crontab -l` to verify; cleanup with empty stdin: `printf "" \| crontab -`). |
| 11 | `is_valid_label` allowlist (ASCII alphanumeric + `-` + `_`) used by macOS = tight superset of `# `/`\n`/`;`/`|`/`&`/`$`/backtick/`'`/`"` blacklist — Linux can adopt SAME allowlist with zero security gap | INV-12 statement (`docs/security/INVARIANTS.md:137–143`) + cron metachars are subset of "non-alphanumeric/non-`-`/non-`_`" by construction | `[verified]` | ✅ Confirmed by inspection of INV-12 (`docs/security/INVARIANTS.md:141`) — allowlist excludes ALL whitespace, `.`, `/`, `~`, `$`, `` ` ``, `&`, `;`, plus `#`/`'`/`"`/`|`/`\n` are all non-alphanumeric/non-`-`/non-`_`. Single allowlist suffices both macOS + Linux. |
| 12 | `src/scheduler/macos.rs::is_valid_label_inline` is local helper used inside `MacosScheduler::unregister` (P012 Task 2 line 420–423) | P012 phiếu Task 2 spec lines 420–423 + P012 Discovery anchor "INV-12 preserved" lines 50 | `[verified via P012]` | ✅ Worker Turn 1 confirmed: defined at `src/scheduler/macos.rs:291`, called at `src/scheduler/macos.rs:356`. Pre-flight `is_valid_label` at `src/core/register.rs:37`. |
| 13 | `NoopScheduler` + `RegisterIntent` have `#[allow(dead_code)]` on Linux (P012 Discovery noted: `pub` but only constructed on macOS targets) | P012 Discovery "macos module gating" + "NoopScheduler / RegisterIntent dead-code warnings on Linux" lines 37–38 | `[verified via P012]` | ✅ Confirmed. P013 will exercise both on Linux — `#[allow(dead_code)]` becomes removable. Worker verifies + removes if Linux compile produces zero warnings without the allow. |
| 14 | `core::register::run` / `core::unregister::run` / `core::status::run` already validate `label` pre-flight via `is_valid_label` helper (INV-12 point 1) | INV-12 statement + P012 Discovery INV-12 audit lines 50 verbatim | `[verified via P012]` | ✅ Confirmed — no caller-side change needed for INV-22 Point 1. |
| 15 | `tests/cli_register.rs` is gated `#[cfg(target_os = "macos")]` (P012 Discovery Edge cases lines 43) — Linux integration tests need a separate file | P012 Discovery "Test count delta" lines 43 verbatim | `[verified via P012]` | ✅ Confirmed. P013 Task 5 creates `tests/cli_register_linux.rs` (new file, parallel-gated `#[cfg(target_os = "linux")]`). |
| 16 | `Cargo.toml` `tokio` features include `process` (Phase 1.4 — runner.rs) AND `io-util` is NOT required by V2 (sync stdlib pivot) | Phase 1.4 ships `runner::fire_task` using `tokio::process::Command` per ARCHITECTURE.md §Modules `src/runner.rs` row | `[verified]` | ✅ Worker Turn 1 confirmed: tokio features = `["rt", "macros", "process", "time", "fs", "io-std"]`. `process` ✅ present. **`io-util` ABSENT — V2 sidesteps via `std::process::Command` sync (no tokio async I/O needed for crontab shell-out).** Cargo.toml diff = ZERO. |
| 17 | `src/cli/register.rs:51` is the EXACT line that prints `plist: {plist_path}` (per P012 phiếu anchor #14) | P012 phiếu anchor #14 verbatim + Discovery anchor "Anchor #14 confirmed" | `[unverified]` | ✅ Worker Turn 1 confirmed: `src/cli/register.rs:51` → `println!("  plist: {}", output.plist_path.display())`. Single consumer. |
| 18 | `parse_next_fire` in `core/status.rs` parses macOS descriptor format (`descriptor = { "Hour" => N "Minute" => M }`) — Linux crontab line format NOT parsed | ARCHITECTURE.md §Cron mechanism "Lifecycle status" para "P005 Discovery" + P012 phiếu line 28 explicit "macOS descriptor format... Phase 3.2 P013 adds parallel `parse_cron_next_fire` cho Linux line — out of scope P012" — and **out of scope P013** per OUT-OF-SCOPE list above | `[verified]` | ✅ Confirmed `next_fire: None` is expected Linux behavior in P013; documented in this phiếu Edge cases. |
| 19 | The integration test pattern (mock binary in PATH) requires `assert_cmd` crate OR manual `Command::env("PATH", ...)` setup | `tests/cli_*.rs` existing pattern uses `assert_cmd` per Phase 1 history | `[needs Worker verify]` | ⏳ Worker `grep -l "assert_cmd" tests/` → expect ≥1 hit. If absent: use stdlib `std::process::Command::new(BIN).env("PATH", ...)`. Either works; Worker picks per existing test file style. |
| 20 | `src/main.rs` uses `#[tokio::main]` — CLI handler call paths execute INSIDE active tokio runtime | Phase 1 P001 spec + Worker Turn 1 verify | `[verified]` | ✅ Worker Turn 1 confirmed: `#[tokio::main(flavor = "current_thread")]` in `src/main.rs`. **V1 spec used `tokio::runtime::Runtime::new().block_on(...)` inside this context → nested-runtime PANIC.** V2 sync `std::process::Command` eliminates the panic entirely (no nested runtime needed). |

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.**

⚠️ Anchor #1 — Architect did not Read `src/scheduler/linux.rs` (envelope: cannot Read src/). Strong indirect evidence from P012 phiếu spec + Discovery. Worker grep-verify in Task 0 BEFORE editing.
⚠️ Anchor #7, #10, #19 — `[needs Worker verify]` remaining for Task 0. All mechanical (stdlib API verify via `cargo check`; dogfood smoke; grep for `assert_cmd`). None are blockers.
⚠️ Anchor #16, #20 (V2 RESOLVES V1 incorrect anchors) — Worker Turn 1 caught two `[verified]` claims in V1 that would have caused runtime failures. V2 corrects via sync `std::process::Command` pivot.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns.

**Phiếu version:** V2 (Architect responded to Worker Turn 1 — sync stdlib pivot for shell-out)

### Turn 1 — Worker CHALLENGE (2026-05-28)

**Anchors verified (8 deferred + 1 blocker found):**

1. **crontab capability (Anchor #8):** `command -v crontab` → `/usr/bin/crontab`. `crontab -l 2>&1` → stderr: `"no crontab for sep"`, exit 1. Substring `"no crontab"` confirmed. ✅
2. **tokio features (Anchor #16):** `grep -A5 "^tokio = " Cargo.toml` → features: `["rt", "macros", "process", "time", "fs", "io-std"]`. `process` ✅ confirmed. `rt` ✅ confirmed. **`io-util` ❌ ABSENT.** The phiếu Task 3 code imports `use tokio::io::AsyncWriteExt;` which requires the `io-util` feature. This is NOT present in `Cargo.toml`. Per Constraint #1 + Hard Stop rule 2: adding a tokio feature = `Cargo.toml` change = escalation required. See Objection #1 below.
3. **is_valid_label source location (Anchor #12 / DP1):** `grep -rn "fn is_valid_label\|is_valid_label_inline" src/scheduler/ src/core/register.rs` → 3 hits: `src/core/register.rs:37` (pre-flight `is_valid_label`), `src/scheduler/macos.rs:291` (local `is_valid_label_inline` def), `src/scheduler/macos.rs:356` (call site). Baseline for DP1 confirmed. ✅
4. **Test fixture pattern (Anchor #6):** `grep -n "TempDir\|PATH\|mock" tests/cli_run.rs` → `TempDir` used at lines 48, 95, 117, 156 via `tempfile::TempDir`. **PATH injection NOT present** in `cli_run.rs` — existing tests use `TempDir` for tmp dirs but inject the mock binary path differently (direct `Command::new(BIN)` + config file, no `PATH` override). Phiếu's mock-crontab-in-PATH pattern has no exact precedent in existing test files. Acceptable — the pattern is sound (`std::process::Command::env("PATH", ...)` is standard); Worker flags no precedent for awareness, not as a blocker. ⚠️ (no precedent, but approach is valid)
5. **Stdio::piped precedent (Anchor #7):** `grep -rn "Stdio::piped" src/` → 0 hits. New API surface, no existing precedent. ✅ (expected absent — not a blocker)
6. **Runtime::new precedent (Anchor DP6):** `grep -rn "Runtime::new\|tokio::runtime" src/` → 0 hits. New pattern. **CRITICAL:** `src/main.rs` uses `#[tokio::main]` (Phase 1 P001 ships `#[tokio::main(flavor = "current_thread")]` per ARCHITECTURE.md). All CLI handler call paths including `cli::register::run`, `cli::unregister::run`, `cli::status::run` execute INSIDE the existing tokio runtime. Calling `tokio::runtime::Runtime::new()` + `block_on` from within an already-running tokio context panics at runtime: `"Cannot start a runtime from within a runtime"`. This is a **Tầng 1 blocker**. See Objection #2 below. ❌
7. **plist_path consumers (Anchor #17 / DP5):** `grep -rn "plist_path" src/cli/ src/core/status.rs src/mcp/tools.rs` → exactly 1 hit: `src/cli/register.rs:51` → `println!("  plist: {}", output.plist_path.display())`. Confirmed single consumer. DP5 "2-line gate" scope is accurate. ✅
8. **advisory-cron: tag conflict (Anchor tag):** `grep -rn "advisory-cron:" src/ tests/ docs/` → hits only in `src/scheduler/linux.rs` (stub doc comment), `src/scheduler/mod.rs` (trait doc comment), `docs/BACKLOG.md`, `docs/ticket/P012*.md`, `docs/ticket/P013*.md`. No hit in any existing crontab file or test crontab fixture. Tag `# advisory-cron: <label>` is unique within crontab semantic, no conflict. ✅

**Decision point review:**

- **DP1 (shared `is_valid_label` allowlist in `mod.rs`):** APPROVE. Single source of truth for INV-12 + INV-22. Allowlist (alphanumeric + `-` + `_`) excludes all crontab metacharacters including `#`, `\n`, whitespace, `;`, `|`, `&`, `$`, backtick, `.`, `/` — correct and tighter than a blacklist. macOS inline copy deleted, call redirected to `super::is_valid_label`. Sanity-check passed: crontab field separator is whitespace; allowlist excludes whitespace → label injection into cron line via tag is safe. Single source is architecturally sound.
- **DP2 (`plist_path: None` on Linux):** APPROVE. `RegisterReport.plist_path: Option<PathBuf>` None is semantically correct — Linux has no plist. `unwrap_or_default()` in `core::register::run` yields empty `PathBuf`. Task 6 gate fixes the render. No alternative (path-to-cache marker) is warranted for P013 scope.
- **DP3 ("no crontab for user" silent fallback in `status`):** APPROVE. Mirrors macOS bootout idempotent pattern. `is_registered: false` is the correct observable result when the user has zero crontab. Confirmed stderr substring `"no crontab"` present on WSL2 (Anchor #1 verified).
- **DP4 (per-test `TempDir` mock crontab binary):** APPROVE. Parallel-safe, no shared fixtures to race. Note: existing test files do NOT use the PATH-override pattern — the approach is new but sound. Each test owns its own `mock_crontab` script + state file. Boilerplate is accepted per phiếu DP4 rationale.
- **DP5 (tackle P012 watch-item in P013, Task 6):** APPROVE. The +2-line render gate in `src/cli/register.rs:51` is cosmetic and closes a confirmed P012 open item. Single consumer confirmed at exact line 51. Scope expansion is minimal and appropriate.
- **DP6 (sync→async bridge via local `Runtime::new()`):** **OBJECT — Tầng 1 blocker.** See Objection #2 below.

**Constraint #7 (no `--config` interpolation in cron line):**
ACCEPT with watch-item acknowledged. Parity with macOS plist `["<self_exe>", "run"]` shape is correct for P013. The limitation (Sếp using non-default config path would be silently ignored by cron-fired runs) is real but deferred to Phase 3.5+ per Architect. No objection for P013 scope. Recommend watch-item already noted in phiếu is sufficient.

**Mock shell portability concern (POSIX `sh` vs WSL2 bash):**
APPROVE. WSL2 `/bin/sh` is `dash` on Ubuntu-based distros (most WSL2 default installs) — POSIX-compliant, no bash-isms needed for the mock script. The mock body uses only `case`/`;;`/`cat`/`echo` — all POSIX primitives. No concern.

**Out-of-decision-point objections:**

**Objection #1 — `io-util` tokio feature MISSING from `Cargo.toml` (Tầng 1 Hard Stop):**
`use tokio::io::AsyncWriteExt;` in phiếu Task 3 requires the `io-util` tokio feature. Current `Cargo.toml` tokio features: `["rt", "macros", "process", "time", "fs", "io-std"]`. `io-util` is NOT present. Adding it to `Cargo.toml` = dependency change = Hard Stop per CLAUDE.md rule 2 + phiếu Constraint #1 explicitly states this must escalate. The phiếu states "No new dep added" in Files KHÔNG sửa `Cargo.toml` row, but this claim is **incorrect** — the impl as spec'd requires `io-util`.

Proposed resolutions (Worker proposes 2 Tầng 1 alternatives for Architect):
- **Alt A (preferred):** Add `io-util` to tokio features in `Cargo.toml`. This is a minor feature addition to an existing dep (tokio is already in tree), not a new crate. Phiếu Constraint #1 + Hard Stop wording says "no new dep" — whether a feature flag counts as "new dep" is Sếp/Architect judgment. Worker treats it as Tầng 1 to be safe. If Architect/Sếp approves, Worker adds `"io-util"` to the tokio features list and removes it from KHÔNG sửa table.
- **Alt B:** Replace `tokio::io::AsyncWriteExt` usage with `std::process::Stdio::piped()` + synchronous write via `std::io::Write` on the child's `stdin` handle — i.e., replace `tokio::process::Command` in `write_user_crontab` with `std::process::Command` (blocking). This avoids the tokio async write path entirely and also sidesteps Objection #2 below. `write_user_crontab` becomes a sync function. `read_user_crontab` can similarly use `std::process::Command::output()` (sync). The `Scheduler` trait methods are already sync — sync shell-out throughout is the natural fit, eliminates the `io-util` + `Runtime::new` problems in one move. **This is the Worker's recommended solution** — avoids all nested-runtime risk and the missing feature flag simultaneously.

**Objection #2 — `Runtime::new()` nested-runtime panic (Tầng 1 Hard Stop):**
The phiếu spec (Task 3) creates a new `tokio::runtime::Runtime` inside each `CrontabScheduler` method. However, `src/main.rs` uses `#[tokio::main]`, which means ALL CLI command handlers (`register`, `unregister`, `status`) run inside an active tokio runtime. Calling `tokio::runtime::Runtime::new().block_on(...)` from within an existing runtime causes a panic at runtime: `"Cannot start a runtime from within a runtime. This happens because a function (like \`block_on\`) attempted to block the current thread while the thread is being used to drive asynchronous tasks."` This is not a compile-time error — it compiles fine but panics on the first `advisory-cron register` invocation on Linux. The phiếu as written would pass `cargo build` but fail at runtime dogfood smoke.

Proposed resolutions:
- **Alt A (strongly preferred, aligns with Objection #1 Alt B):** Use `std::process::Command` (sync, blocking) for both `crontab -l` and `crontab -` shell-outs. The `Scheduler` trait methods are sync; there is no need for async here. `std::process::Command::output()` reads crontab; `std::process::Command::new("crontab").arg("-").stdin(Stdio::piped()).spawn()` with `child.stdin.write_all(...)` (using `std::io::Write` from stdlib) writes crontab. Zero new tokio features needed, zero nested-runtime risk, same behavior. This is clean and idiomatic for sync code.
- **Alt B:** Use `tokio::task::block_in_place(|| { ... })` instead of `Runtime::new()`. `block_in_place` is safe inside an existing multi-threaded tokio runtime and allows running blocking code without spawning a new runtime. However, it requires the `rt-multi-thread` feature (not just `rt`), which is another feature-flag concern. Current tokio feature set only has `rt` (current-thread flavor per Phase 1 P001). This alt is more complex than Alt A with no benefit.

The Worker's strong recommendation: **Architect revise Task 3 to use `std::process::Command` (sync) for both `read_user_crontab` and `write_user_crontab`**. This resolves both Objection #1 and Objection #2 in a single change, keeps `Cargo.toml` frozen, eliminates nested-runtime panic, and is idiomatic for a sync trait surface. The cron management path is not performance-critical (~1 call/day); blocking I/O is fully acceptable.

**Status:** ✅ RESPONDED — phiếu bumped to V2

### Turn 2 — Architect RESPOND (2026-05-28, phiếu V2)

**Objection #1 (io-util feature absent):** **ACCEPT.** V1 spec was incorrect — `tokio::io::AsyncWriteExt` requires the `io-util` tokio feature, which is not present in `Cargo.toml`. Worker's verification (Anchor #16 grep) is conclusive. V2 pivots to `std::io::Write::write_all` on the child stdin handle — zero feature addition, zero `Cargo.toml` diff. Constraint #1 ("no new dep / no feature change") is now accurate (V1's claim was false).

**Objection #2 (Runtime::new nested panic):** **ACCEPT.** V1 spec contained a fatal runtime bug: `tokio::runtime::Runtime::new().block_on(...)` called from inside `#[tokio::main]` context (per Anchor #20 verified by Worker) would panic on first invocation. Worker's diagnosis is correct. V2 replaces `tokio::process::Command` + nested `Runtime::new` bridge with `std::process::Command` (sync). The `Scheduler` trait is sync (P012-shipped); blocking shell-out is the natural fit for ~3 short crontab calls per day (~10ms each). MCP runtime context preserved without nested-runtime panic. Alt B (`block_in_place`) rejected — requires `rt-multi-thread` feature (also absent) for more complexity than Alt A.

**Net change V1 → V2:**
- `tokio::process::Command` → `std::process::Command` (3 method bodies in `CrontabScheduler` + 2 helpers `read_user_crontab` / `write_user_crontab`)
- `tokio::io::AsyncWriteExt::write_all` → `std::io::Write::write_all` (1 stdin write site in `write_user_crontab`)
- Remove `tokio::runtime::Runtime::new()?.block_on(async { ... })` wrappers from all 3 methods — direct sync body
- Remove `.await` from all `output()` / `spawn()` / `wait_with_output()` calls — sync equivalents (`Command::output()`, `Command::spawn()`, `Child::wait_with_output()`)
- Imports update: `use std::process::{Command, Stdio};` + `use std::io::Write;` (remove `use tokio::io::AsyncWriteExt;` + `use tokio::process::Command;`)
- Helper `read_user_crontab` / `write_user_crontab` signatures lose `async fn` → `fn`; callers drop `.await`
- Cargo.toml diff: **ZERO** (corrected from V1's incorrect claim)
- Estimated LOC: unchanged ~635 total (sync-vs-async is line-for-line near-equivalent)
- Other 5 DP (1–5) preserved unchanged from V1
- Other anchors preserved
- Anchor #7 marker changed: `tokio::process::Command::stdin(Stdio::piped())` → `std::process::Command::stdin(Stdio::piped())` (stdlib stable since Rust 1.0 — zero feature flag concern)
- Anchor #16 updated: now `[verified]` ✅ (V2 confirms `io-util` ABSENT is fine; sync pivot needs no tokio I/O features at all)
- Anchor #20 added: `#[tokio::main]` context verified by Worker; sync `std::process::Command` is safe inside it (no nested-runtime panic possible)
- Constraint #9 (`Scheduler` trait surface frozen + sync→async bridge) — REWRITTEN: bridge removed entirely; Linux impl is sync end-to-end. Constraint #16 (tokio bridge cost) removed (no bridge in V2).
- Files KHÔNG sửa table: `Cargo.toml` row now stronger ("ZERO diff" — V2 pivot eliminates feature add)

**Counter-revisions:** None — Worker's single-fix proposal (sync `std::process::Command` throughout) is optimal. No alternative entertained.

**Sub-issue discovered mid-revision (worth flagging for Worker Turn 2 review):**
- `std::process::Child::stdin` is `Option<ChildStdin>` (same as Tokio). The sync pattern `child.stdin.take().expect(...)` or `child.stdin.as_mut().expect(...)` works. V2 uses `.take()` to move the handle out of the `Option` and drop it after `write_all` (closes the pipe → signals EOF to crontab). The Tokio pattern in V1 was identical structure — only the trait dispatched (`std::io::Write` vs `tokio::io::AsyncWriteExt`) and the `.await` differ.
- No second `unsafe` concern, no platform-specific stdlib gotcha. Sync `std::process::Command` is fully portable on Linux + macOS (P013 only ships Linux impl, but the helpers compile cleanly on either target if `#[cfg]` were relaxed in a future phase).

**Status:** ✅ RESPONDED — phiếu bumped to V2

### Turn 2 — Worker CHALLENGE (2026-05-28)

**Mechanical scrub (tokio::process / tokio::io::Async / Runtime::new / block_on):**
- grep hits: lines 32, 101, 105, 129, 133, 144, 155, 159, 161-162, 166, 174, 176, 179-183, 189, 198, 232, 683-686, 1024, 1093, 1112, 1172, 1209
- All surviving refs framed in Debate Log (V1 incorrect → V2 corrects to sync): YES. Zero hits instruct EXECUTE to use async tokio I/O. Every occurrence is one of: (a) historical Debate Log Turn 1 objection text, (b) Debate Log Turn 2 "ACCEPT — V2 corrects to std::process", (c) V2 body text contrasting "NOT `tokio::process::Command`", (d) Nghiệm thu / Constraint / Risk table citing "N/A in V2". No stray prescriptive instruction to EXECUTE worker survives.

**Cargo.toml zero-diff invariant:**
- V2 Acceptance criteria (Nghiệm thu line 1111): `git diff Cargo.toml` after Task 3 — empty. ✅ Present.
- Constraint #1 (line 1085): "No new dep, no new feature flag. `Cargo.toml [dependencies]` ZERO diff." ✅ Strengthened from V1's incorrect claim.
- Files KHÔNG sửa table (line 1076): "`Cargo.toml` row now stronger ('ZERO diff' — V2 pivot eliminates feature add)." ✅ Confirmed.

**std::process::Command stdin pattern sanity:**
- APPROVE. Task 3 `write_user_crontab` (lines 578-611) follows canonical pattern exactly:
  1. `.stdin(Stdio::piped())` on `Command::new("crontab").arg("-")` — correct pipe setup.
  2. `child.stdin.take().context(...)` — correct idiom: moves `ChildStdin` out of `Option`, leaving `None`; `.context()` is strictly better than `.expect()` for production (returns `Err` vs panic).
  3. `stdin.write_all(content.as_bytes())` — `use std::io::Write` imported at line 436. Correct.
  4. Scoped drop via `{ ... }` block — stdin dropped at block exit, closes pipe, signals EOF to crontab before `wait_with_output()` is called. Correct sequencing: close-then-wait avoids stdin-blocking deadlock.
  5. `child.wait_with_output()` called AFTER stdin scope exits — correct. If reversed, crontab blocks on stdin while caller blocks on wait → deadlock. Sequence is sound.

**Sub-issue review (Child::stdin transposition):**
- CONFIRM 1:1 structural transposition. `std::process::Child::stdin: Option<std::process::ChildStdin>` shape is identical to `tokio::process::Child::stdin: Option<tokio::process::ChildStdin>`. `.take()` idiom works on both. Only the trait dispatch differs (`std::io::Write::write_all` sync vs `tokio::io::AsyncWriteExt::write_all(...).await`). No corner cases identified.

**Verdict:** APPROVE_AS_IS

**Net change estimate:** unchanged ~635 LOC (Architect's V2 estimate confirmed — sync-vs-async is line-for-line near-equivalent; scoped-drop replaces `.await` but adds no bulk).

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
| A (trigger) | (Linux dogfood smoke — Task 0 final block) `crontab -l \| grep "# advisory-cron: p013-smoke"` after `advisory-cron register --label p013-smoke --schedule "0 9 * * *"` | exactly 1 matched line | | |
| A (trigger) | After `advisory-cron unregister --label p013-smoke`, `crontab -l \| grep "# advisory-cron"` | 0 lines | | |
| B (capability) | `command -v crontab` on Linux WSL2 | `/usr/bin/crontab` (or other absolute path) | | |
| B (capability) | `crontab -l; echo "exit=$?"` (when user has no crontab) | exit 1 + stderr containing `"no crontab"` substring | | |
| B (capability) | `cargo check` (Linux host) after edits | exit 0, 0 errors | | |
| B (capability) | `cargo test --all` (Linux host) | all pass + ≥5 new tests in `tests/cli_register_linux.rs` + ≥3 new in `scheduler::linux::tests` | | |
| B (capability) | `cargo clippy --all-targets -- -D warnings` | clean | | |
| C (migration completeness) | `grep -c "bail!" src/scheduler/linux.rs` BEFORE | exactly 3 (one per stub method) | | |
| C (migration completeness) | `grep -c "bail!.*P013" src/scheduler/linux.rs` AFTER | 0 (P013 ship message removed) | | |
| C (migration completeness) | `grep -c "is_valid_label_inline" src/scheduler/macos.rs` AFTER | 0 (replaced with `super::is_valid_label`) | | |
| C (migration completeness) | `grep -c "scheduler::is_valid_label\|super::is_valid_label" src/scheduler/` AFTER | ≥2 (macos.rs + linux.rs) | | |
| C (V2 sanity) | `grep -c "tokio::process\|tokio::io::Async\|Runtime::new\|block_on" src/scheduler/linux.rs` AFTER | 0 (V2 sync pivot — no tokio runtime in scheduler) | | |
| C (V2 sanity) | `grep -c "^tokio = " Cargo.toml` BEFORE vs AFTER | identical line (zero diff) | | |
| D (persistence) | N/A (no doctrine rotation) | — | — | N/A |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | | |
| E (env drift) | `cargo build --release` from clean target/ (Linux) | exit 0, 0 warnings, binary ≤7MB | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability + assumption verify (Sub-mechanism B + C baseline)

**Mục đích:** Verify all `[needs Worker verify]` and `[unverified]` anchors BEFORE touching code. Capture all results into Debug Log section + Discovery Report later.

> **V2 note:** Worker Turn 1 already completed many of these checks. Anchors #8, #9, #12, #16, #17, #20 marked ✅ from Turn 1 evidence — Worker may skip re-verify in EXECUTE phase but must STILL run mechanical compile-time checks #3, #5 (Cargo.toml diff sanity), #6, #7, #8 below.

Run all of (record output for each in Debug Log):

1. **B-capability — `crontab` binary present (Turn 1 ✅):**
   ```bash
   command -v crontab
   ```
   Expected: absolute path `/usr/bin/crontab`. Already confirmed Turn 1. Re-run for fresh evidence in Discovery.

2. **B-capability — `crontab -l` empty-user behavior (Turn 1 ✅ — substring `"no crontab"` confirmed):**
   ```bash
   crontab -l 2>&1; echo "exit=$?"
   ```
   Expected: exit 1 + stderr containing substring `"no crontab"`. Re-run + capture verbatim.

3. **C-migration baseline — stub state confirmation:**
   ```bash
   grep -c "bail!" src/scheduler/linux.rs
   ```
   Expected: exactly 3. If different → STOP, re-read P012 ship state.

4. **C-migration baseline — current `is_valid_label_inline` location (Turn 1 ✅ — lines 291/356):**
   ```bash
   grep -n "is_valid_label_inline\|is_valid_label" src/scheduler/macos.rs src/scheduler/mod.rs
   ```
   Expected: 1+ hit `is_valid_label_inline` in macos.rs; 0 hits `is_valid_label` in mod.rs (P013 adds it).

5. **V2 Cargo.toml sanity — `tokio` features unchanged + `io-util` still absent:**
   ```bash
   grep -A5 "^tokio = " Cargo.toml
   ```
   Expected: features list = `["rt", "macros", "process", "time", "fs", "io-std"]` (Turn 1 ✅). **V2 invariant: this line MUST be identical after EXECUTE — zero diff. No tokio feature additions required by the sync pivot.** Re-grep AFTER editing `src/scheduler/linux.rs` to confirm Cargo.toml untouched.

6. **Anchor #17 — exact `plist_path` print line in CLI register (Turn 1 ✅ — line 51):**
   ```bash
   grep -n "plist" src/cli/register.rs
   ```
   Expected: line 51 prints `output.plist_path`. Used for Task 6 surgical edit.

7. **Anchor #19 — `assert_cmd` crate available for integration tests:**
   ```bash
   grep -A3 "^assert_cmd" Cargo.toml || grep -l "assert_cmd" tests/
   ```
   Expected: dev-dep present OR existing `tests/*.rs` import. If absent: use stdlib `std::process::Command::new(BIN).env("PATH", ...)` pattern in `tests/cli_register_linux.rs`. Either acceptable.

8. **A-trigger Linux dogfood smoke (one-time mechanical — run AFTER cargo build --release succeeds, BEFORE commit):**
   ```bash
   # Snapshot existing crontab
   crontab -l > /tmp/before.txt 2>/dev/null || echo "" > /tmp/before.txt

   # Register with smoke label
   ./target/release/advisory-cron register --label p013-smoke --schedule "0 9 * * *"

   # Verify exactly 1 tagged line added
   crontab -l > /tmp/after.txt
   diff /tmp/before.txt /tmp/after.txt | grep "advisory-cron: p013-smoke" | wc -l
   # Expected: 1

   # Unregister
   ./target/release/advisory-cron unregister --label p013-smoke

   # Verify cleanup
   crontab -l > /tmp/cleanup.txt 2>/dev/null || echo "" > /tmp/cleanup.txt
   diff /tmp/before.txt /tmp/cleanup.txt | wc -l
   # Expected: 0 (no diff)
   ```
   Record results into Discovery Report Sub-mech A row.

**Nếu bất kỳ check nào fail expected:** STOP, escalate via AskUserQuestion + DISCOVERY_REPORT.

---

### Task 1 — EDIT `src/scheduler/mod.rs`: add shared `is_valid_label` helper

**File:** `src/scheduler/mod.rs`

**Tìm:** end of existing `pub trait Scheduler { ... }` block. (After the `compile_error!` and before `NoopScheduler` would be tidy; Worker decides exact placement.)

**Thêm:**

```rust
/// Cross-OS label allowlist — ASCII alphanumeric + `-` + `_`, non-empty.
///
/// Used by:
/// - `scheduler::macos::MacosScheduler::unregister` (INV-12 defense-in-depth point 2 — same allowlist as `generate_plist`).
/// - `scheduler::linux::CrontabScheduler::{register, unregister, status}` (INV-22 defense-in-depth point 2 — pre-flight at `core::*` is point 1).
///
/// **Why a tight allowlist instead of a metachar blacklist:** the allowlist excludes ALL whitespace,
/// path separators (`.`, `/`, `~`), shell meta-chars (`$`, `` ` ``, `&`, `;`, `|`, `#`), quote chars (`'`, `"`),
/// AND newlines — covers both launchd domain-target injection (INV-10/12/17) AND crontab tag-line injection
/// (INV-22) without enumeration. Single source of truth; less to forget.
pub fn is_valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::is_valid_label;

    #[test]
    fn accepts_alphanumeric_hyphen_underscore() {
        assert!(is_valid_label("advisory-scan_daily"));
        assert!(is_valid_label("foo"));
        assert!(is_valid_label("F00"));
        assert!(is_valid_label("a-b_c-d"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_valid_label(""));
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for label in ["foo;bar", "foo$bar", "foo|bar", "foo&bar", "foo`bar`",
                      "foo'bar", "foo\"bar", "foo#bar", "foo bar", "foo\nbar",
                      "foo/bar", "foo.bar", "foo~bar", "../etc"] {
            assert!(!is_valid_label(label), "expected rejection for {label:?}");
        }
    }

    #[test]
    fn rejects_unicode() {
        assert!(!is_valid_label("café"));
        assert!(!is_valid_label("日本語"));
    }
}
```

**Lưu ý:**

- This is the **canonical INV-12 + INV-22 enforcement point** going forward. macOS `is_valid_label_inline` deleted in Task 2 → replaced with `super::is_valid_label`. No behavioral change on macOS (same allowlist).
- Test module `#[cfg(test)] mod tests` is at the `mod.rs` level, NOT inside `macos`/`linux` submodules — cross-OS, runs on both.
- **P012 Discovery noted `NoopScheduler` + `RegisterIntent` have `#[allow(dead_code)]` on Linux.** Verify in Task 0 #4 + Task 4 spec — Linux real impl exercises `RegisterIntent` actively → `#[allow(dead_code)]` MAY become removable. Worker checks: after Linux impl ships, does `cargo clippy --all-targets -- -D warnings` on Linux pass WITHOUT the `#[allow]`? If yes → remove. If no (e.g. `NoopScheduler` still Linux-unused) → keep `#[allow]` on the specific struct that needs it. Document in Discovery.

---

### Task 2 — EDIT `src/scheduler/macos.rs`: replace local `is_valid_label_inline` with shared `super::is_valid_label`

**File:** `src/scheduler/macos.rs`

**Tìm:** the local `fn is_valid_label_inline(label: &str) -> bool { ... }` (P012 Task 2 spec lines 420–423 — body uses same allowlist).

**Thay bằng:** delete the local helper. Replace its single call site inside `MacosScheduler::unregister` (the line `if !is_valid_label_inline(label) { anyhow::bail!(...) }`) with:

```rust
if !super::is_valid_label(label) {
    anyhow::bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
}
```

**Lưu ý:**

- Behavior identical (same allowlist, same error message). Zero regression. macOS unit tests in `scheduler::macos::tests` unchanged.
- This is the only macOS-side edit. `generate_plist` defense-in-depth inside `MacosScheduler::register` already inlines its own allowlist check (per P012 Task 2 + INV-12 second point) — leave it untouched OR optionally migrate to `super::is_valid_label` if Worker sees value in single-source-of-truth there too. Architect leans: leave `generate_plist` untouched (mechanical-move discipline carried from P012 — `generate_plist` body is auto-migrated, not refactored). Worker self-decides whether to also migrate `generate_plist`'s check — both options preserve INV-12.

---

### Task 3 — REPLACE `src/scheduler/linux.rs`: real `CrontabScheduler` impl (V2 sync stdlib)

**File:** `src/scheduler/linux.rs` (full body replace — stub gone)

**Tìm:** entire current stub body (3 `bail!` methods + 3 stub tests per P012 Task 3 spec).

**Thay bằng (V2 — sync `std::process::Command` throughout, NO tokio I/O):**

```rust
//! Phase 3.2 — Linux crontab scheduler (P013).
//!
//! `register`: `crontab -l` (tolerate "no crontab for <user>" stderr) → filter existing tagged line →
//! append `<minute> <hour> * * * <self_exe> run # advisory-cron: <label>` →
//! pipe back via `crontab -` (stdin).
//!
//! `unregister`: same flow, omit append; report whether tag was found.
//!
//! `status`: `crontab -l` → grep for tagged line → return raw line in `raw_descriptor`.
//!
//! **INV-22 defense-in-depth point 2**: each method validates `label` via `super::is_valid_label` first.
//! Point 1 lives at `core::*::run` (pre-flight per INV-12 — same allowlist; covers INV-22 transitively).
//!
//! **Cron form constraint**: P013 builds only daily form `<min> <hour> * * *` (mirrors macOS Phase 1 `M H * * *`
//! constraint per ARCHITECTURE.md §Cron mechanism). Full 5-field cron deferred to P014 INV-23.
//!
//! **V2 sync stdlib (Debate Log Turn 1+2)**: `Scheduler` trait methods are sync; this module uses
//! `std::process::Command` (blocking) for `crontab -l` and `crontab -` shell-outs. No tokio runtime,
//! no `io-util` feature, no nested-runtime panic. Blocking I/O is acceptable for ~3 crontab calls/day (~10ms each).

use anyhow::{Context, Result, bail};
use std::io::Write;
use std::process::{Command, Stdio};

use super::{
    RegisterIntent, RegisterReport, Scheduler, SchedulerStatus, UnregisterReport, is_valid_label,
};

/// Tag prefix used to mark advisory-cron-managed lines in user crontab.
/// Format: `<cron_expr> <command> # advisory-cron: <label>`.
const TAG_PREFIX: &str = "# advisory-cron: ";

#[derive(Debug, Default)]
pub struct CrontabScheduler;

impl Scheduler for CrontabScheduler {
    fn register(&self, intent: &RegisterIntent) -> Result<RegisterReport> {
        // INV-22 defense-in-depth point 2.
        if !is_valid_label(&intent.label) {
            bail!("invalid label {:?} — must be ASCII alphanumeric + '-' + '_'", intent.label);
        }

        // Read existing crontab (tolerate "no crontab for <user>" stderr).
        let existing = read_user_crontab()?;

        // Filter out any prior tagged line for this label (idempotent re-register).
        let tag = format!("{TAG_PREFIX}{}", intent.label);
        let mut lines: Vec<&str> = existing
            .lines()
            .filter(|line| !line.contains(&tag))
            .collect();

        // Build new managed line.
        let new_line = format!(
            "{} {} * * * {} run # advisory-cron: {}",
            intent.minute,
            intent.hour,
            intent.self_exe.display(),
            intent.label,
        );
        lines.push(&new_line);

        // Pipe combined output back via `crontab -`.
        let combined = format!("{}\n", lines.join("\n"));
        write_user_crontab(&combined)?;

        Ok(RegisterReport { plist_path: None })
    }

    fn unregister(&self, label: &str) -> Result<UnregisterReport> {
        // INV-22 defense-in-depth point 2.
        if !is_valid_label(label) {
            bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }

        let existing = read_user_crontab()?;

        let tag = format!("{TAG_PREFIX}{label}");
        let mut found = false;
        let kept: Vec<&str> = existing
            .lines()
            .filter(|line| {
                let is_tagged = line.contains(&tag);
                if is_tagged {
                    found = true;
                }
                !is_tagged
            })
            .collect();

        if found {
            let combined = if kept.is_empty() {
                String::new()
            } else {
                format!("{}\n", kept.join("\n"))
            };
            write_user_crontab(&combined)?;
        }

        Ok(UnregisterReport { was_registered: found })
    }

    fn status(&self, label: &str) -> Result<SchedulerStatus> {
        // INV-22 defense-in-depth point 2.
        if !is_valid_label(label) {
            bail!("invalid label {label:?} — must be ASCII alphanumeric + '-' + '_'");
        }

        let existing = match read_user_crontab() {
            Ok(s) => s,
            Err(_) => {
                // "no crontab for user" or other crontab read failure → silent fallback.
                // Mirrors macOS bootout idempotent "Boot-out failed: 3: No such process" pattern.
                return Ok(SchedulerStatus {
                    is_registered: false,
                    raw_descriptor: None,
                });
            }
        };

        let tag = format!("{TAG_PREFIX}{label}");
        let matched = existing.lines().find(|line| line.contains(&tag));

        Ok(SchedulerStatus {
            is_registered: matched.is_some(),
            raw_descriptor: matched.map(String::from),
        })
    }
}

/// Read user crontab via `crontab -l` (sync). Tolerates "no crontab for <user>" stderr by returning empty string.
///
/// **Worker verify in Task 0**: exact substring observed on the dev host. Phiếu uses lowercase `"no crontab"`.
fn read_user_crontab() -> Result<String> {
    let output = Command::new("crontab")
        .arg("-l")
        .output()
        .context("failed to invoke `crontab -l`")?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    // Non-zero exit: check if it's the benign "no crontab" case.
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("no crontab") {
        // User has no crontab — treat as empty input for register flow.
        return Ok(String::new());
    }

    // Other non-zero exit: surface the error.
    bail!(
        "`crontab -l` failed (exit {:?}): {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Write user crontab via `crontab -` (sync stdin pipe).
///
/// **Race condition note (Architect-acknowledged)**: between `read_user_crontab` and `write_user_crontab`,
/// another process could modify the user's crontab. P013 accepts last-writer-wins. Future hardening
/// (advisory locking via `flock(2)` on a sentinel file) deferred — out of scope per BACKLOG Phase 3 acceptance.
fn write_user_crontab(content: &str) -> Result<()> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn `crontab -` for stdin write")?;

    {
        let mut stdin = child
            .stdin
            .take()
            .context("failed to acquire stdin handle for `crontab -`")?;
        stdin
            .write_all(content.as_bytes())
            .context("failed to write content to `crontab -` stdin")?;
        // Drop stdin to close pipe → signal EOF to crontab.
    }

    let output = child
        .wait_with_output()
        .context("failed to wait for `crontab -` to complete")?;

    if !output.status.success() {
        bail!(
            "`crontab -` failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for `CrontabScheduler` — invalid-label rejection. Full integration
    //! (mock `crontab` binary in PATH + happy-path flows) lives in `tests/cli_register_linux.rs`.

    use super::*;

    fn make_intent(label: &str) -> RegisterIntent {
        RegisterIntent {
            label: label.into(),
            hour: 9,
            minute: 0,
            self_exe: std::path::PathBuf::from("/usr/local/bin/advisory-cron"),
            working_dir: std::path::PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn register_rejects_empty_label() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn register_rejects_label_with_semicolon() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("foo;evil")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn register_rejects_label_with_hash() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("foo#bar")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn register_rejects_label_with_newline() {
        let s = CrontabScheduler;
        let err = s.register(&make_intent("foo\nbar")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn unregister_rejects_invalid_label() {
        let s = CrontabScheduler;
        let err = s.unregister("foo$bar").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }

    #[test]
    fn status_rejects_invalid_label() {
        let s = CrontabScheduler;
        let err = s.status("foo|bar").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid label"), "got: {msg}");
    }
}
```

**Lưu ý:**

- **V2 sync stdlib pivot (per Debate Log Turn 1+2 — Worker Objections #1 + #2 ACCEPTED):**
  - `std::process::Command` (blocking) for both `crontab -l` and `crontab -` shell-outs. NOT `tokio::process::Command`.
  - `std::io::Write::write_all` on `Child::stdin` (taken from `Option<ChildStdin>` via `.take()`). NOT `tokio::io::AsyncWriteExt`.
  - Helpers `read_user_crontab` / `write_user_crontab` are `fn` (not `async fn`). No `.await` anywhere in this module.
  - **Zero tokio runtime nesting** — methods run inside `#[tokio::main]` context without spawning a child runtime. No `Runtime::new`, no `block_on`, no `block_in_place`.
  - **Zero Cargo.toml diff** — no `io-util` / `rt-multi-thread` feature additions. Stdlib `std::process` + `std::io::Write` are zero-feature.
  - Blocking cost (~10ms per `crontab` shell-out) is acceptable for ~1 register/day. Performance not a concern.
- **`write_user_crontab` empty-content case** (when unregister removes the last managed line and `kept.is_empty()`) — passes empty string to `crontab -`. POSIX behavior: `crontab -` with empty stdin replaces user crontab with empty crontab (or removes it on some distros). Acceptable for P013 — Sếp's crontab is single-line use case anyway. Worker dogfood smoke (Task 0 #8) confirms behavior on WSL2.
- **Idempotency**: register filters by exact `# advisory-cron: <label>` tag — re-register with same label replaces; with different label appends. unregister with missing tag → `was_registered: false`, no crontab modification (skip `write_user_crontab` call entirely — preserves user crontab unmodified when there's nothing to remove).
- **Tag location: end-of-line comment** — `<cron_expr> <command> # advisory-cron: <label>`. POSIX crontab supports `#` comments. cron line parser ignores the comment but `contains(&tag)` substring match finds it. This is the same pattern other tools (`crontab-l` style helpers, fcrontab) use.
- **No `--config <path>` interpolated into the cron line** — Architect dropped this from the schema. **Rationale:** `advisory-cron run` resolves config via `core::config_path::default_config_path()` (per ARCHITECTURE.md §Config schema "Default path: `~/.config/advisory-cron/config.toml`"). Embedding `--config <intent.config_path>` would (a) require adding `config_path: PathBuf` field to `RegisterIntent` (trait surface change → Tầng 1 cascade) and (b) leak the config path into the user's crontab (privacy + cleanliness concern). Worker may push back: if Sếp uses non-default config path, `advisory-cron run` from cron won't find it. **Mitigation:** if non-default config path needed, Worker can extend `RegisterIntent` with `Option<PathBuf>` later (P014+). For P013 the cron line invokes plain `advisory-cron run` — same as macOS plist `ProgramArguments` which is `["<self_exe>", "run"]` per ARCHITECTURE.md §Cron mechanism plist example (no `--config` either). Architect verifies parity: macOS plist invokes `run` with no `--config` → Linux cron line should match. ✅ Parity preserved.
- **Stdout/stderr redirect (BACKLOG mentioned cron line redirects to `~/.local/state/advisory-cron/heartbeat-cron-<label>.log` as debug safety net)**: Architect OMITS this from P013 v1 cron line. Reason: (a) it adds shell redirect operators (`>>`) that are parsed by the shell launched by cron — additional surface; (b) `advisory-cron run` already writes its own heartbeat JSONL (the durable record); (c) the "debug safety net" justification is speculative — defer until Sếp dogfood reveals need. **Watch-item for P015 README**: document that cron stdout/stderr discards by default (cron behavior); if user wants `> /path/log` redirect they can edit the crontab line manually post-register (advisory-cron will preserve their edit on next register only if the tag-line ALSO matches their manual edits — currently the filter is `contains(&tag)` so manual additions after the tag would NOT be preserved on re-register; this is a Phase 3.5+ polish concern).

---

### Task 4 — Verify `#[allow(dead_code)]` cleanup eligibility for `NoopScheduler` + `RegisterIntent` on Linux

**File:** `src/scheduler/mod.rs`

**Tìm:** any `#[allow(dead_code)]` annotation added by P012 on `NoopScheduler` / `RegisterIntent` / other structs (per P012 Discovery line 38).

**Action:**

1. After Task 3 ships, run `cargo clippy --all-targets -- -D warnings` on Linux.
2. If clippy still warns about `NoopScheduler` being unused on Linux (it's used by macOS tests gated `#[cfg(target_os = "macos")]` only): KEEP the `#[allow(dead_code)]` on `NoopScheduler`.
3. If clippy warns about `RegisterIntent` being constructed only on macOS: since `CrontabScheduler::register` (Linux) now actively reads `&RegisterIntent` fields → `#[allow(dead_code)]` SHOULD be removable on `RegisterIntent`. Try removing → re-run clippy → if clean, keep removed.
4. Document outcome in Discovery Report (which `#[allow]` annotations survived, which got removed).

**Lưu ý:**

- This is mechanical cleanup, not behavioral. Tầng 2 work folded into Tầng 1 phiếu because it's a direct consequence of Task 3. Skip if no `#[allow(dead_code)]` exists in current code (Task 0 #4 grep would have caught).

---

### Task 5 — CREATE `tests/cli_register_linux.rs`: mock `crontab` binary integration tests

**File:** `tests/cli_register_linux.rs` (NEW)

**Pattern:** mock `crontab` binary in `tempfile::TempDir`, prepend to `PATH` via `Command::env("PATH", ...)`. The mock supports:

- `crontab -l` → reads from `$MOCK_CRONTAB_STATE` file (env-set by test), echoes to stdout; if file missing, exit 1 with stderr `"no crontab for user"`.
- `crontab -` → reads stdin, writes to `$MOCK_CRONTAB_STATE` file.

**Mock binary** — emit as a shell script (`mock_crontab.sh`) in TempDir via `std::fs::write`, chmod 0755, prepend its dir to PATH.

**Test cases (minimum 5+):**

```rust
//! Integration tests for `CrontabScheduler` happy-path flows.
//! Gated `#[cfg(target_os = "linux")]`. Mock `crontab` binary in PATH.

#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

/// Create a mock `crontab` binary that proxies state via a file.
/// Returns (mock_dir, state_file_path).
fn make_mock_crontab(td: &TempDir) -> (PathBuf, PathBuf) {
    let state = td.path().join("mock_crontab.state");
    let script = td.path().join("crontab");

    // POSIX shell mock: handles `crontab -l` and `crontab -`.
    let body = format!(
        r#"#!/bin/sh
case "$1" in
  -l)
    if [ -f "{state}" ]; then
      cat "{state}"
    else
      echo "no crontab for $USER" >&2
      exit 1
    fi
    ;;
  -)
    cat > "{state}"
    ;;
  *)
    echo "mock_crontab: unknown arg $1" >&2
    exit 2
    ;;
esac
"#,
        state = state.display(),
    );
    fs::write(&script, body).expect("write mock crontab");
    let mut perm = fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&script, perm).unwrap();
    (td.path().to_path_buf(), state)
}

/// Prepend `dir` to PATH for a Command invocation.
fn with_mock_path(cmd: &mut Command, mock_dir: &std::path::Path) {
    let existing = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{}:{}", mock_dir.display(), existing));
}

fn write_minimal_config(td: &TempDir, label: &str) -> PathBuf {
    // Write a minimal valid config so `advisory-cron register` can load it.
    let cfg_path = td.path().join("config.toml");
    let cfg = format!(
        r#"
[task]
command = "echo"
args = ["hello"]
working_dir = "{wd}"
label = "{label}"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{hb}"
"#,
        wd = td.path().display(),
        hb = td.path().join("heartbeat.jsonl").display(),
        label = label,
    );
    fs::write(&cfg_path, cfg).expect("write test config");
    cfg_path
}

#[test]
fn register_writes_one_tagged_line() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test1");

    let mut cmd = Command::new(BIN);
    cmd.args(["register", "--label", "p013-test1", "--config"])
        .arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    let out = cmd.output().expect("spawn advisory-cron");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let written = fs::read_to_string(&state).expect("state written");
    let tagged: Vec<&str> = written.lines().filter(|l| l.contains("# advisory-cron: p013-test1")).collect();
    assert_eq!(tagged.len(), 1, "expected exactly 1 tagged line, got: {written}");
}

#[test]
fn unregister_removes_tagged_line() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test2");

    // Pre-populate state with a tagged line + an unrelated user line.
    fs::write(
        &state,
        "0 12 * * * /usr/bin/echo user-line\n0 9 * * * /bin/advisory-cron run # advisory-cron: p013-test2\n",
    ).unwrap();

    let mut cmd = Command::new(BIN);
    cmd.args(["unregister", "--label", "p013-test2"])
        .arg("--config")
        .arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    let out = cmd.output().expect("spawn advisory-cron");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let after = fs::read_to_string(&state).expect("state written");
    assert!(!after.contains("p013-test2"), "tagged line still present: {after}");
    assert!(after.contains("user-line"), "user line was clobbered: {after}");
}

#[test]
fn idempotent_re_register_replaces_not_duplicates() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test3");

    // Register twice with same label.
    for _ in 0..2 {
        let mut cmd = Command::new(BIN);
        cmd.args(["register", "--label", "p013-test3"]).arg("--config").arg(&cfg);
        with_mock_path(&mut cmd, &mock_dir);
        let out = cmd.output().expect("spawn");
        assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    }

    let written = fs::read_to_string(&state).expect("state written");
    let tagged: Vec<&str> = written.lines().filter(|l| l.contains("# advisory-cron: p013-test3")).collect();
    assert_eq!(tagged.len(), 1, "expected exactly 1 tagged line after 2 registers, got: {written}");
}

#[test]
fn status_reports_loaded_after_register() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test4");

    // Register first.
    let mut reg = Command::new(BIN);
    reg.args(["register", "--label", "p013-test4"]).arg("--config").arg(&cfg);
    with_mock_path(&mut reg, &mock_dir);
    reg.output().expect("register");

    // Confirm state file has the line (sanity).
    assert!(fs::read_to_string(&state).unwrap().contains("# advisory-cron: p013-test4"));

    // Now run status --json.
    let mut st = Command::new(BIN);
    st.args(["status", "--label", "p013-test4", "--config"]).arg(&cfg).arg("--json");
    with_mock_path(&mut st, &mock_dir);
    let out = st.output().expect("status");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["plist_loaded"], serde_json::json!(true), "got: {stdout}");
}

#[test]
fn status_reports_unloaded_when_crontab_empty() {
    let td = TempDir::new().unwrap();
    let (mock_dir, _state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test5");

    // State file does NOT exist → mock returns "no crontab for user" stderr + exit 1.

    let mut st = Command::new(BIN);
    st.args(["status", "--label", "p013-test5", "--config"]).arg(&cfg).arg("--json");
    with_mock_path(&mut st, &mock_dir);
    let out = st.output().expect("status");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["plist_loaded"], serde_json::json!(false), "got: {stdout}");
}

#[test]
fn invalid_label_rejected_preflight_before_crontab_spawned() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "valid-cfg-label");

    // Mock state initially empty — if mock IS invoked, state would change.
    // We assert state is still absent (= mock never invoked) = pre-flight rejected before shell-out.

    let mut cmd = Command::new(BIN);
    cmd.args(["register", "--label", "foo;evil"]).arg("--config").arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    let out = cmd.output().expect("spawn advisory-cron");
    assert!(!out.status.success(), "expected non-zero exit for invalid label");
    assert!(!state.exists(), "mock crontab was invoked despite invalid label — INV-22 point 1 violated");
}

#[test]
fn preserves_unrelated_user_cron_lines() {
    let td = TempDir::new().unwrap();
    let (mock_dir, state) = make_mock_crontab(&td);
    let cfg = write_minimal_config(&td, "p013-test7");

    // Pre-populate with user's own cron entries.
    fs::write(&state, "# user comment\n0 12 * * * /usr/bin/backup\n30 * * * * /usr/local/bin/poll\n").unwrap();

    let mut cmd = Command::new(BIN);
    cmd.args(["register", "--label", "p013-test7"]).arg("--config").arg(&cfg);
    with_mock_path(&mut cmd, &mock_dir);
    cmd.output().expect("register");

    let after = fs::read_to_string(&state).unwrap();
    assert!(after.contains("# user comment"), "user comment lost");
    assert!(after.contains("/usr/bin/backup"), "user backup line lost");
    assert!(after.contains("/usr/local/bin/poll"), "user poll line lost");
    assert!(after.contains("# advisory-cron: p013-test7"), "managed line missing");
}
```

**Lưu ý:**

- Per-test `TempDir` (DP4 chosen — no shared `tests/fixtures/mock_crontab.sh`). Parallel-safe: each test owns its own mock binary + state file.
- 7 test cases above (exceeds "5+" minimum). Worker may add more during EXECUTE if Task 0 #2 reveals distro-specific `crontab -l` stderr text needing dedicated coverage.
- `BIN = env!("CARGO_BIN_EXE_advisory-cron")` — Cargo provides this env var for integration tests pointing to the test-built binary. Standard pattern (used by existing `tests/cli_*.rs` per Phase 1 history).
- **Worker verify Task 0 #7**: if `assert_cmd` crate is used elsewhere, may simplify some boilerplate. Either pattern OK.

---

### Task 6 — EDIT `src/cli/register.rs`: gate empty `plist_path` render (closes P012 P013 watch-item)

**File:** `src/cli/register.rs`

**Tìm:** the line (per Task 0 #6 — Worker confirms exact line number) that prints `output.plist_path`. Expected to look like:

```rust
println!("  plist: {}", output.plist_path.display());
```

**Thay bằng:**

```rust
if !output.plist_path.as_os_str().is_empty() {
    println!("  plist: {}", output.plist_path.display());
}
```

**Lưu ý:**

- 2-line cosmetic gate. macOS behavior unchanged (path always non-empty there). Linux: skips the line entirely when `RegisterReport.plist_path: None` → `unwrap_or_default()` → empty PathBuf.
- **Symmetric check in `src/cli/unregister.rs`**: Worker grep for any analogous "print empty field" pattern (Task 0 — quick visual scan). Architect best-guess: `UnregisterOutput` fields are `bool`/`String` — no empty-path render risk. If Worker finds symmetric concern → mirror the fix; if not → skip.
- **`tests/cli_register.rs` is `#[cfg(target_os = "macos")]` gated** (per Anchor #15) → macOS test path always exercises non-empty branch → no regression risk. New `tests/cli_register_linux.rs` Task 5 implicitly tests the empty-branch via `register --label ... ` smoke (success exit + no panic on empty path render).

---

### Task 7 — UPDATE `docs/ARCHITECTURE.md`: split §Cron mechanism + update §Modules + Phase status

**File:** `docs/ARCHITECTURE.md`

**Tìm + Thay:**

1. **§Modules table** — update `src/scheduler/linux.rs` row description from "stub: `impl Scheduler` bails ... Real implementation lands P013" → "Real impl: `CrontabScheduler` uses `crontab -l` (read, tolerate `no crontab for user` stderr) + `crontab -` (stdin pipe write) — **sync `std::process::Command`** (zero tokio runtime nesting, zero new feature flag). Tag-line idempotency `# advisory-cron: <label>`. INV-22 defense-in-depth via `super::is_valid_label`. Gated `#[cfg(target_os = "linux")]`. Phase 3.2 (P013)." Phase ships column: `3.1 ✅ (stub)` → `3.2 ✅`.

2. **§Modules table** — update `src/scheduler/mod.rs` description to mention the new `is_valid_label` shared helper: append " Shared `is_valid_label` helper (P013) used by both `macos.rs` defense-in-depth AND `linux.rs` defense-in-depth (single source of truth for INV-12 + INV-22 allowlist)."

3. **§Cron mechanism** — split into 2 subsections:
   - Rename the existing section to `### Cron mechanism — macOS (launchd plist)` and keep its body verbatim.
   - Add new subsection AFTER it:

   ```markdown
   ### Cron mechanism — Linux (crontab injection)

   `advisory-cron register` on Linux invokes `crontab -l` to read the user's existing crontab, filters out any prior advisory-cron-managed line tagged `# advisory-cron: <label>` (idempotent re-register), appends a new managed line, and pipes the result back via `crontab -` (stdin):

   ```
   <minute> <hour> * * * <self_exe> run # advisory-cron: <label>
   ```

   Example: `register --label scan --schedule "0 9 * * *"` writes:

   ```
   0 9 * * * /home/sep/.cargo/bin/advisory-cron run # advisory-cron: scan
   ```

   **Sync shell-out (P013 V2):** the Linux impl uses sync `std::process::Command` for both `crontab -l` and `crontab -`. The `Scheduler` trait is sync (P012), and the CLI entry runs under `#[tokio::main]` — using `tokio::process::Command` would either require `io-util` feature (absent) or a nested `tokio::runtime::Runtime` (panics inside an existing runtime). Stdlib `std::process` + `std::io::Write` is the natural fit: zero new feature flag, zero nested-runtime risk, ~10ms blocking cost per call is negligible at ~1 register/day.

   **Cron form constraint (Phase 3.2):** daily form `M H * * *` only (parity with macOS launchd `StartCalendarInterval` per Phase 1 INV-11). Full 5-field cron (ranges, lists, steps, DOW) deferred to P014 INV-23.

   **"No crontab" graceful path:** when the user has no crontab yet, `crontab -l` exits 1 with stderr `"no crontab for <user>"`. `CrontabScheduler::register` treats this as empty input and proceeds normally. `CrontabScheduler::status` returns `is_registered: false` silently (mirrors macOS `bootout` idempotent fallback).

   **Last-writer-wins race:** between `crontab -l` (read) and `crontab -` (write), another process modifying the user crontab races. P013 accepts last-writer-wins. Hardening (advisory `flock(2)` on sentinel) deferred — Phase 3.5+ if dogfood reveals need.

   **No `--config` interpolation:** the managed line invokes `advisory-cron run` with no `--config` flag, mirroring the macOS plist `ProgramArguments = ["<self_exe>", "run"]` pattern. `run` resolves config via `core::config_path::default_config_path()`. If Sếp uses non-default config path, Phase 3.5+ `RegisterIntent` extension would be required.

   **No stdout/stderr redirect:** the cron line omits redirect operators (`> /path`). cron discards stdout/stderr by default (or mails on some distros). `advisory-cron run` writes its own heartbeat JSONL via `core::run::run` — the durable observability record. Manual `> /path` redirect can be added post-register by editing the crontab line directly (NOT preserved on re-register — Phase 3.5+ concern).

   **`next_fire` parsing:** Phase 3.2 (P013) does NOT parse `M H * * *` from the Linux descriptor — `core::status::run` calls `parse_next_fire` which is macOS-format-specific. On Linux `next_fire` renders `None`. P014 INV-23 adds a parallel `parse_cron_descriptor` parser when full 5-field cron support lands.
   ```

4. **Phase status** at the bottom of ARCHITECTURE.md:
   - `⏸️ **Phase 3.2** (P013): ...Deferred.` → `✅ **Phase 3.2** (P013): CrontabScheduler real impl shipped — sync std::process::Command for crontab -l/- (no tokio feature add), INV-22 defense-in-depth via shared scheduler::is_valid_label, Linux WSL2 dogfood smoke verified, X new tests, binary <YMB>. P012 watch-item closed (empty plist_path render gated in cli/register.rs).`

**Lưu ý:** Worker fills the X/Y placeholders post-build (test count from `cargo test --all 2>&1 | grep "test result"`; binary size from `ls -lh target/release/advisory-cron`).

---

### Task 8 — UPDATE `docs/CHANGELOG.md`: P013 entry

**File:** `docs/CHANGELOG.md`

**Thêm:** entry per existing CHANGELOG style (Worker mirrors P010/P011/P012 format). Date = Worker's commit day. Include: `CrontabScheduler` real impl using **sync `std::process::Command`** (V2 pivot — Debate Log Turn 1+2 — keeps Cargo.toml frozen, avoids nested-runtime panic), INV-22 defense-in-depth (documented in P014), P012 watch-item closure (CLI render gate), 7+ new Linux integration tests, dogfood smoke verified, Phase 3.2 ✅.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/scheduler/mod.rs` | Task 1: add `is_valid_label` shared helper + `mod tests` for it. Task 4: remove `#[allow(dead_code)]` where Linux real impl now exercises the struct. |
| `src/scheduler/macos.rs` | Task 2: replace local `is_valid_label_inline` with `super::is_valid_label` call. |
| `src/scheduler/linux.rs` | Task 3: full body replace — stub `bail!` → real **sync** `CrontabScheduler` impl (`std::process::Command` + `std::io::Write`, no tokio I/O) + 6 unit tests for label rejection. |
| `src/cli/register.rs` | Task 6: gate `plist_path` print on `!is_empty()`. |
| `tests/cli_register_linux.rs` | Task 5: NEW file — 7+ integration tests via mock `crontab` binary in PATH. |
| `docs/ARCHITECTURE.md` | Task 7: split §Cron mechanism + update §Modules + Phase status. |
| `docs/CHANGELOG.md` | Task 8: P013 entry. |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/scheduler/macos.rs` (`MacosScheduler::register` body, `generate_plist`, `RealLaunchctl`, etc.) | Unchanged behavior. macOS test suite passes unchanged on `target_os = "macos"`. |
| `src/core/register.rs`, `src/core/unregister.rs`, `src/core/status.rs` | Unchanged — already trait-generic per P012. `RegisterIntent` builder unchanged. |
| `src/mcp/tools.rs` | Unchanged — calls `PlatformScheduler::default()` per P012 (Linux now resolves to real `CrontabScheduler`, transparently). MCP tool schemas unchanged. |
| `src/cli/unregister.rs`, `src/cli/status.rs` | Task 6 spec-check: confirm no symmetric "print empty field" risk. If present → mirror Task 6 fix. If absent → skip. |
| `src/main.rs` | **Unchanged.** `#[tokio::main(flavor = "current_thread")]` preserved. V2 sync `std::process::Command` inside this context is safe (no nested-runtime concern). |
| `src/runner.rs`, `src/heartbeat.rs`, `src/alert.rs`, `src/config.rs`, `src/core/run.rs`, `src/core/init.rs`, `src/core/config_path.rs` | Untouched. |
| `Cargo.toml` | **ZERO diff.** Verified Task 0 #5: `tokio` features = `["rt", "macros", "process", "time", "fs", "io-std"]`. NO new dep added, NO new tokio feature added. V2 sync pivot avoids `io-util` requirement. Worker confirms via `git diff Cargo.toml` post-edit = empty. |
| `docs/security/INVARIANTS.md` | INV-22 NOT documented here in P013 — P014 batches with INV-23. |
| `README.md` | NOT updated — P015 (Phase 3.4). |
| `.github/workflows/ci.yml` | NOT updated — P014 (Phase 3.3). |

---

## Luật chơi (Constraints)

1. **No new dep, no new feature flag.** `Cargo.toml [dependencies]` ZERO diff. V2 sync pivot avoids requiring `io-util` tokio feature (which is absent — Worker Turn 1 ✅ verified). Any feature add at EXECUTE time = HARD STOP per CLAUDE.md Hard Stop rule 2.
2. **No `unsafe { }` block.** Mock binary script + sync stdlib shell-out are safe-Rust. Per INV-6, any `unsafe` requires Sếp escalation.
3. **macOS behavior unchanged.** `cargo test` on macOS (or CI matrix in P014) must pass with zero diff vs P012-shipped state. `MacosScheduler::register/unregister/status` body identical post-P013 except `is_valid_label_inline` → `super::is_valid_label` rename (same allowlist).
4. **INV-12 + INV-22 single source of truth.** All label allowlist checks go through `scheduler::is_valid_label`. `MacosScheduler::unregister` + all 3 `CrontabScheduler` methods + `generate_plist` (optional, Worker's call per Task 2 Lưu ý) all call this helper. NO duplicated allowlist constants.
5. **Tag format frozen.** `# advisory-cron: <label>` is THE tag prefix. Used as substring match in register/unregister/status. Changing the tag = phiếu of its own (breaks existing user crontabs from prior P013 register runs — backwards-compat concern).
6. **Idempotency mandatory.** Re-register with same label = exactly 1 tagged line (no duplicates). Unregister with no matching tag = `was_registered: false` + 0 crontab modification (don't call `crontab -` when nothing to do). Test cases #3 + #5 cover.
7. **No `--config` in cron line.** Match macOS plist's `["<self_exe>", "run"]` arg shape. Adding `--config` requires extending `RegisterIntent` (out of scope P013).
8. **No stdout/stderr redirect in cron line.** Defer to user manual edit + P015 README documentation.
9. **`Scheduler` trait surface frozen.** No async-fy of trait methods (would cascade to all `core::*::run` signatures). **V2 Linux impl is SYNC end-to-end** — uses `std::process::Command` blocking, no tokio runtime, no `Runtime::new`, no `block_on`, no `block_in_place`. The trait being sync IS the natural fit; no bridge needed.
10. **Linux integration tests gated `#[cfg(target_os = "linux")]`.** Run only on Linux host (WSL2 + future P014 CI Linux job). macOS host runs `cargo test --all` without these tests.
11. **CLI surface frozen.** No subcommand/flag/exit-code change. Task 6 is render-side cosmetic gate (no semantic change).
12. **JSON schema frozen.** `RegisterOutput`/`UnregisterOutput`/`StatusReport` fields unchanged. MCP consumers see identical JSON output.
13. **Race condition acknowledged, not fixed.** Read-modify-write of user crontab is last-writer-wins. Documented in §Cron mechanism Linux subsection. Hardening deferred Phase 3.5+.
14. **No security boundary change without INV documentation:** P013 implements INV-22 enforcement; P014 documents the INV. Architect commits to documenting INV-22 in P014 phiếu. Worker DOES NOT need to write `docs/security/INVARIANTS.md` in P013.
15. **`Scheduler::register` may NOT include `--config <path>` argument until trait extension** (Constraint #7 mechanical consequence — re-stated for clarity).
16. **Sync I/O cost acceptable.** ~10ms per `crontab` shell-out × ~3 calls/register × ~1 register/day = ~30ms/day. Performance not a concern. Any future async migration is out of scope P013 and would require trait-level coordination.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` (Linux WSL2) — zero warnings, binary ≤7MB
- [ ] `cargo test --all` (Linux WSL2) — all pass + ≥7 new tests in `tests/cli_register_linux.rs` + ≥6 new unit tests in `scheduler::linux::tests` + ≥4 new in `scheduler::tests` (`is_valid_label` allowlist)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] **`git diff Cargo.toml` after Task 3 — empty (zero diff). V2 invariant.**
- [ ] **`grep -c "tokio::process\|tokio::io::Async\|Runtime::new\|block_on" src/scheduler/linux.rs` after Task 3 — 0. V2 invariant.**
- [ ] On macOS (or CI matrix when P014 ships): identical pre-P013 test count + zero behavior diff for `MacosScheduler` tests

### Manual Testing (Linux WSL2 — Sub-mech A trigger gap)
- [ ] **Dogfood smoke** (Task 0 #8 — full sequence): register + verify exactly 1 tagged line + unregister + verify cleanup. Record results in Debug Log + Discovery Report.
- [ ] `advisory-cron register --label p013-real --schedule "0 9 * * *"` → exit 0, prints success message (no blank `plist:` line), NO `"Cannot start a runtime from within a runtime"` panic.
- [ ] `crontab -l | grep "# advisory-cron: p013-real"` → exactly 1 line, contains `9 * * *`
- [ ] `advisory-cron status --label p013-real --json` → JSON with `"plist_loaded": true`, `"next_fire": null` (Phase 3.2 limitation — documented)
- [ ] `advisory-cron unregister --label p013-real` → exit 0
- [ ] `crontab -l | grep "advisory-cron"` → 0 lines
- [ ] Idempotency: run `register --label p013-idem` twice → `crontab -l | grep "p013-idem" | wc -l` → 1 (not 2)
- [ ] Invalid label rejection: `advisory-cron register --label "foo;evil"` → exit ≠ 0, stderr contains "invalid label", `crontab -l` unchanged (mock `crontab` NEVER invoked — INV-22 point 1 hold)

### Regression (Sub-mech B capability — cross-OS)
- [ ] macOS path (if available, else punt to P014 CI matrix): `MacosScheduler::{register,unregister,status}` round-trip works as in P012 ship state. `is_valid_label` rename is sole macOS-side change; same allowlist = zero diff.
- [ ] MCP smoke (Linux): `advisory-cron mcp` + JSON-RPC `tools/call register {label: "mcp-test"}` → success, `crontab -l` shows new tagged line. Confirms MCP layer routes through new `CrontabScheduler` transparently. **V2 invariant: no nested-runtime panic in MCP context either (MCP server also runs under `#[tokio::main]`; sync `std::process::Command` is safe there).**

### Docs Gate (Tầng 1)
- [ ] `docs/ARCHITECTURE.md` §Cron mechanism split (macOS + Linux subsections) + Linux subsection documents V2 sync `std::process::Command` rationale
- [ ] `docs/ARCHITECTURE.md` §Modules — `src/scheduler/linux.rs` row updated + `src/scheduler/mod.rs` row mentions shared helper
- [ ] `docs/ARCHITECTURE.md` Phase status — Phase 3.2 ⏸️ → ✅
- [ ] `docs/CHANGELOG.md` — P013 entry (Worker-formatted, mirroring P012 style)
- [ ] `README.md` — NOT touched in P013 (P015 scope)
- [ ] `docs/security/INVARIANTS.md` — NOT touched in P013 (P014 scope)
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P013.md` — full report written, including:
   - Anchor verifications (which were `[verified]`, which Worker confirmed/refuted)
   - `crontab -l` stderr substring observed on dev host (Task 0 #2)
   - `tokio` feature set confirmed (Task 0 #5) — `io-util` absent, V2 sync pivot avoids needing it
   - Sub-mech A dogfood smoke results (Task 0 #8)
   - `#[allow(dead_code)]` cleanup outcome (Task 4)
   - **V2 pivot rationale captured (Debate Log Turn 1+2 cross-reference)**
   - Any deviation from phiếu spec + rationale
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top) per CLAUDE.md doctrine format
- [ ] Sub-mechanism A-E Verification Trace filled (table above)

### P013 watch-items inherited from P012 (status check)
- [ ] **P012 watch-item "`plist_path` empty Linux render"**: CLOSED in this phiếu Task 6.
- [ ] **P012 watch-item "`CrontabScheduler::status()` error swallowed by `core::status::run`"**: ACCEPTED behavior — Linux now returns `is_registered: false` for "no crontab" case directly (silent fallback inside `CrontabScheduler::status`), not via core swallow. P013 Discovery documents the explicit behavior.

### P013 new watch-items for future phiếu
- [ ] **P014 INV-22 + INV-23 formal documentation**: confirm P013 Discovery Report flags this in the "Edge cases / limitations" section so P014 Architect picks up.
- [ ] **`--config <path>` interpolation in cron line**: if Sếp's dogfood reveals need for non-default config path, document as backlog item (Phase 3.5+).
- [ ] **`flock(2)` advisory locking for crontab read-modify-write**: defer Phase 3.5+ unless dogfood reveals race.
- [ ] **Async-fy `Scheduler` trait**: defer; V2 sync `std::process::Command` works fine for ~1 register/day. Revisit only if Sếp opens a phiếu specifically.
- [ ] **`next_fire` parsing on Linux**: P014 INV-23 adds parallel `parse_cron_descriptor`.

---

## Risk + rollback

**Risk levels:**

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| `crontab -` stdin pipe pattern fails on WSL2 (Anchor #7, #10) | Low | High (no Linux register possible) | Task 0 #8 dogfood smoke runs BEFORE merging. V2 uses sync stdlib (zero feature flag concern); pattern `child.stdin.take().unwrap().write_all(...)` is canonical Rust since 1.0. |
| Distro-specific `crontab -l` stderr text (Anchor #9) | Very Low (✅ Turn 1 confirmed `"no crontab for sep"` on WSL2) | Medium (graceful-fallback path broken) | Substring `"no crontab"` matched lowercased — robust against `"no crontab for sep"` / `"no crontab found"` / `"no crontab installed"` variants. |
| `tokio` feature set missing `rt` or `process` (Anchor #16) | None — Turn 1 ✅ verified `rt` + `process` both present | High (would block compile) | N/A — already verified. V2 needs neither `io-util` nor `rt-multi-thread`. |
| **V1 `Runtime::new` nested-runtime panic** | **N/A in V2** — sync stdlib pivot eliminates risk entirely | (was: High) | V2 removes `tokio::runtime::Runtime::new()` + `block_on` from all 3 methods. No nested-runtime possible. |
| macOS test regression from `is_valid_label_inline` → `super::is_valid_label` rename | Very Low | Medium | Same allowlist; same error message text. macOS test suite unchanged. CI matrix (P014) catches if any. |
| Race condition during read-modify-write of user crontab | Low (solo use) | Low (last-writer-wins) | Acknowledged in Constraint #13 + ARCHITECTURE.md §Cron mechanism Linux subsection. |
| User's non-managed crontab lines clobbered | Very Low | High | Test case "preserves_unrelated_user_cron_lines" + dogfood smoke `/tmp/before.txt` vs `/tmp/after.txt` diff. |

**Rollback procedure (if EXECUTE blows up):**

```bash
# 1. Discard all uncommitted edits
git checkout -- src/ tests/ docs/

# 2. Delete new test file
rm tests/cli_register_linux.rs

# 3. Counter rollback (013 → 012)
echo "012" > .phieu-counter

# 4. Branch teardown
git checkout main
git branch -D feat/P013-linux-crontab-impl

# 5. If anything committed already: git revert <hash>
```

If a dogfood smoke partially modifies the user's actual crontab during testing:
```bash
# Restore from /tmp/before.txt snapshot
crontab /tmp/before.txt
crontab -l  # verify
```

---

## Estimated effort

- **LOC**: ~290 impl (src/scheduler/linux.rs full rewrite — sync stdlib, slightly shorter than V1's async equivalent) + ~250 integration tests (tests/cli_register_linux.rs) + ~30 helper (src/scheduler/mod.rs is_valid_label + tests) + ~5 cosmetic gate (src/cli/register.rs) + ~55 docs (ARCHITECTURE.md split — V2 adds sync rationale paragraph). Total: ~630 LOC. **Zero new dep, zero new feature flag, zero Cargo.toml diff.**
- **Time estimate (Worker effort)**: half-day (4–6h) — Task 0 verification (~15min, partially covered by Turn 1), Task 1+2 (1h), Task 3 (~1.5h — sync `std::process::Command` is simpler than V1 async — no runtime bridge), Task 4 (~15min), Task 5 (~1.5h — mock binary + 7 test cases), Task 6 (~10min), Task 7+8 (~30min), dogfood smoke + Discovery Report (~30min).
- **DP chốt**: DP1 = shared `is_valid_label` in mod.rs (allowlist, not blacklist) | DP2 = `plist_path: None` Linux | DP3 = silent fallback "no crontab" → `is_registered: false` | DP4 = per-test `TempDir` mock binary | DP5 = tackle P012 watch-item HERE (Task 6) | **DP6 V2 = sync `std::process::Command` (NOT tokio async + Runtime::new) — Worker Turn 1 ACCEPT both objections; sync stdlib is the natural fit for sync `Scheduler` trait, zero feature flag, zero nested-runtime risk**
- **Verification anchors**: 20 total (5 `[verified]` directly + 6 `[verified via P012]` cross-phiếu + 6 `[verified]` by Worker Turn 1 + 3 `[needs Worker verify]` remaining — all mechanical). V1 fatal anchors #16, #20 corrected in V2.

