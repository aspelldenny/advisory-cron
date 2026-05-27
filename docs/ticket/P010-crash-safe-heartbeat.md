# PHIẾU P010: Crash-safe heartbeat write (Phase 2.3 — State recovery)

> **Loại:** Feature
> **Tầng:** 1
> **Ưu tiên:** P1
> **Ảnh hưởng:** `src/heartbeat.rs` (refactor `append` to temp+fsync+rename; harden `read_last_n` against partial last line), `docs/security/INVARIANTS.md` (append INV-21), `docs/ARCHITECTURE.md` (Heartbeat section refinement + Phase status), `docs/CHANGELOG.md`, `README.md` (brief Phase 2.3 note), `tests/cli_run_crash_safe.rs` (NEW integration test — optional, mostly unit tests in `src/heartbeat.rs::tests`)
> **Dependency:** P009 (Phase 2.2 retry — adds 3+ heartbeat writes per `run` invocation, increases crash-surface area; this phiếu makes those writes durable). P008 (alert), P004 (heartbeat schema baseline) transitively.

---

## Context

### Vấn đề hiện tại

BACKLOG.md "Phase 2 — Next sprint" item 2.3 (explicitly promoted by Sếp per same pattern as P008/P009): **State recovery. Crash-safe heartbeat write (write + fsync + rename). Recovery on next fire if previous run interrupted mid-write.**

Current state post-P009 (per CHANGELOG, ARCHITECTURE.md, P009 Discovery, **revised V2 per Worker Turn 1 Anchor #2 verification**):

- `src/heartbeat.rs::append` (Phase 1.4) opens file via `OpenOptions::append(true)` + direct write of one JSONL line. **Not crash-safe.** If the process is killed mid-write (`launchctl kill`, OOM, power loss, panic at unfortunate moment), the heartbeat file can end with a **truncated/partial JSON line** — corrupt JSONL that breaks downstream readers.
- `src/heartbeat.rs::read_last_n` (current impl per Worker verification `src/heartbeat.rs:78-84`) **already skips malformed lines** via `eprintln!("warning: skipping malformed heartbeat line {i}: {err}")` and continues — it does NOT propagate parse errors. Existing test `read_last_n_skips_malformed_line` asserts this skip-all behavior. **However, three properties of the current skip behavior are wrong for INV-21:**
  1. **Wrong granularity** — current impl silently skips ALL malformed lines anywhere in the file (including mid-file). INV-21 sub-rule 2 requires only the LAST line to be tolerated; mid-file corruption MUST fail loud (PROJECT.md hard line #5 "Failure mode = noisy"). Mid-file corruption is impossible under the new atomic-write protocol and indicates external tampering or disk damage.
  2. **Wrong logging channel** — `eprintln!` bypasses the `tracing` subscriber, escaping JSON-formatted log capture and ignoring `RUST_LOG` env-filter. INV-13 (tracing-only logging) requires `tracing::warn!`.
  3. **Caller-side silent failure** — `src/core/status.rs:80` calls `heartbeat::read_last_n(...).unwrap_or_default()`. Under the new protocol, mid-file corruption will return `Err`; `unwrap_or_default` silently converts that to empty `Vec`, showing "no heartbeats" instead of an explicit error. This is a **pre-existing P005/P006-era design choice** in the KHÔNG sửa list — Worker logs in Discovery as a known limitation. INV-21 sub-rule 2 applies to the `read_last_n` return value (it returns the loudest possible `Err`); callers may choose to swallow or propagate per their own contract. Future hardening of the caller is BACKLOG candidate, NOT in P010 scope.
- P009 widened the exposure on the WRITE side: a single `advisory-cron run` invocation can now call `heartbeat::append` **3+ times** (one per retry attempt). Three times the crash surface area; each attempt's write is an independent atomicity event.

PROJECT.md hard line #4 "Heartbeat log is append-only" remains in force — Phase 2.3 does NOT compact, rotate, or restructure. It just makes each *append* atomic and makes the *read* path apply the correct last-vs-mid distinction.

PROJECT.md hard line #5 "Failure mode = noisy" applies inversely here: silent data loss from a corrupt last line that breaks `status --last N` rendering = exactly the bug we're not allowed to ship. Status reader must degrade gracefully (warn-and-skip the bad LAST line ONLY), not crash, and not silently absorb mid-file corruption.

### Giải pháp

**Approach decision — Architect: `temp+rename` (atomic POSIX rename), NOT `fsync-append`.**

Rationale (recorded here for posterity + Discovery cross-reference):

1. **POSIX rename is a hard atomicity guarantee.** `rename(2)` on the same filesystem is atomic by POSIX spec — at any instant another process observes either the old file or the new file, never a partial state. Works on macOS HFS+/APFS (Phase 1 target) and Linux ext4/btrfs/xfs (Phase 3 target). No platform-specific size limits.

2. **`fsync-append` (O_APPEND + fsync) depends on `PIPE_BUF` size for atomicity.** POSIX guarantees writes ≤ `PIPE_BUF` (typically 4 KiB on macOS/Linux but **not standardized across all filesystems / kernels**) are atomic when O_APPEND is set on a regular file. Heartbeat lines are usually < 4 KiB (label + small fields + ≤1 KiB each of stdout_tail / stderr_tail), but: (a) `serde_json` escape expansion of weird stderr bytes can blow up `stderr_tail` past 1 KiB; (b) future schema additions (Phase 3+) could push the record over the line; (c) the safety degrades silently — no compile-time or test-time signal when a write exceeds PIPE_BUF, just intermittent corruption on crash.

3. **Performance cost is negligible for Sếp's usage.** 1 fire/day (`/advisory-scan`) → ~1 line/day → file grows ~1 KB/day = ~365 KB/year. Rewriting 365 KB on every append = ~1 ms of disk I/O on SSD (microseconds for the in-memory part). At 10× usage (10 fires/day = 10 KB/day) over 10 years = 35 MB rewrite per append → still <100 ms on SSD. If a future user adopts advisory-cron for high-frequency cron (per-minute fires for years), this needs revisit — but that scenario is explicitly NOT in scope (Sếp solo + 1-3 daily tasks per CLAUDE.md AI Bias Warnings #1 "vấn đề Sếp ĐANG có"). Worker logs the per-append rewrite cost in Discovery Report if measured.

4. **Conservative = right for sprint-closing fault-tolerance phiếu.** Phase 2.3 is the LAST phiếu of the sprint. The acceptance criterion is "crash-safe append". Choosing the always-safe option (temp+rename) over the fast-but-conditional option (fsync-append) matches the conservative posture appropriate for closing a "resilience" sprint.

**Mechanical design — atomic write protocol (per-append):**

```
1. Read existing heartbeat.jsonl contents (or empty Vec<u8> if file missing).
2. Append the new JSONL line (with trailing newline) to the in-memory buffer.
3. Create temp file in SAME directory as heartbeat.jsonl (REQUIRED — atomic
   rename only works on same filesystem; cross-fs rename is a copy+delete and
   loses atomicity). Use `tempfile::NamedTempFile::new_in(parent_dir)`.
4. Write the full buffer to the temp file.
5. `temp_file.as_file().sync_all()` (fsync — flush kernel page cache to disk;
   defends against power loss after rename returns).
6. `temp_file.persist(target_path)` — atomic rename via `std::fs::rename` under
   the hood. Old heartbeat.jsonl replaced atomically.
7. If any step before step 6 fails: temp file auto-cleaned by Drop. Old
   heartbeat.jsonl untouched. Caller gets `Err(_)` — per existing P004 contract
   `core::run::run` warn-continues (does NOT fail the task).
```

**Recovery — partial last line tolerance in `read_last_n`:**

Pre-P010 heartbeat files may contain a partial last line from a past crash. The new `append` cannot corrupt the file going forward, but the read path must still handle legacy partial lines gracefully **AND tighten the existing skip-all-malformed behavior to skip-last-only-malformed**. Design:

- Iterate all lines from the file.
- For each line: try `serde_json::from_str::<HeartbeatRecord>`. On parse failure:
  - If it's the **last line of the file** (no trailing newline OR is the final element): `tracing::warn!` "partial heartbeat line detected (likely pre-P010 interrupted write), skipping" + continue to next iteration (skipping this corrupt line). **Replaces the current `eprintln!` skip-all behavior with last-only + proper tracing.**
  - If it's **any line other than the last**: this is unexpected (atomic-write protocol prevents mid-file corruption); propagate as `Err` — fail-loud per PROJECT.md hard line #5. Mid-file corruption indicates external tampering or disk failure that the user MUST see. **Tightens current behavior** — under the existing P004/Phase 1.4 impl, mid-file corruption was silently swallowed.
- Take the last `n` *successfully parsed* records.

**Existing test `read_last_n_skips_malformed_line` will need behavior update.** Currently asserts skip-ALL behavior (mid-file corrupt line silently skipped). Under new protocol, that scenario must FAIL loud — Worker updates the test to assert mid-file → Err, and adds a separate test asserting last-line → warn+skip+return-prior. See Task 2 + Task 5.

**Caller-side note (informational, NOT in scope):** `src/core/status.rs:80` uses `heartbeat::read_last_n(...).unwrap_or_default()` — this silently absorbs the new mid-file `Err` and shows empty heartbeats in `advisory-cron status` output. This is a pre-existing P005/P006-era design choice; `core/status.rs` is in the KHÔNG sửa list (Constraint #2 transitively — Worker MUST NOT change it). `read_last_n` itself returns the loudest possible signal; the caller may swallow per its own contract. Worker logs this in Discovery as a known limitation; future caller-side hardening (e.g. propagate the warning to user-facing stderr) is BACKLOG "Open backlog" candidate, NOT in P010 scope.

**Schema unchanged.** `HeartbeatRecord` struct in `src/heartbeat.rs` keeps the Phase 1.4 fields. No `schema_version` bump. No new fields. Constraint #4 from P009 preserved (Architect re-instates: `git diff` of `HeartbeatRecord` struct fields = empty).

**`append` signature unchanged.** `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>` stays exactly as P004 shipped. Constraint #12 from P009 holds: callers in `core/run.rs` (single call site inside retry loop, between the two matches) continue to work without source change. Worker confirms `grep -c "heartbeat::append" src/core/run.rs` STILL equals exactly `1` after this phiếu ships.

**`read_last_n` signature unchanged.** `pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>` stays. Only the internal parse-loop changes (skip-last-corrupt behavior added, mid-file tightened to fail-loud, `eprintln!` → `tracing::warn!`).

**No new runtime dep.** `tempfile` is already in `[dev-dependencies]` (Phase 1.4 P004 — used in heartbeat unit tests). For P010 we need it in `[dependencies]` (runtime) too — this IS a `Cargo.toml` change. **HEADS-UP: this is a `[dependencies]` add per Hard Stop #2 — explicitly sanctioned by this phiếu's Architect decision.** Rationale: `tempfile::NamedTempFile::new_in(parent_dir)` + `.persist(target)` is the canonical Rust idiom for atomic write; rolling our own (manual `OpenOptions` + UUID-suffixed filename + `std::fs::rename` + Drop cleanup) is ~40 LOC of error-prone reinvention. `tempfile` is already pulled by `[dev-dependencies]`, present in `Cargo.lock`, zero marginal compile time / binary size impact. The crate is widely used + maintained + audited (`cargo-audit` ✅ historically).

**Docs**: INV-21 (heartbeat append crash-safety boundary — temp+fsync+rename invariant), ARCHITECTURE.md §Heartbeat schema "Atomicity" subsection, CHANGELOG, README Phase 2.3 paragraph, Phase status update (**Phase 2 COMPLETE → ALL 10 PHIẾU SHIPPED**).

### Scope

- **CHỈ sửa:**
  - `src/heartbeat.rs` (refactor `append` body to temp+fsync+rename; refactor `read_last_n` body to skip-corrupt-last-line; new unit tests)
  - `Cargo.toml` (move `tempfile = "3"` from `[dev-dependencies]` to `[dependencies]` — see HEADS-UP above; if a separate `[dev-dependencies] tempfile` entry is still needed, keep both — Worker verify cargo accepts. Most idiomatic: single `[dependencies]` entry covers both contexts.)
  - `docs/security/INVARIANTS.md` (append INV-21)
  - `docs/ARCHITECTURE.md` (§Heartbeat schema atomicity subsection + §Phase status update Phase 2.3 SHIPPED, **Phase 2 COMPLETE**)
  - `docs/CHANGELOG.md` (P010 entry + sprint-close summary)
  - `README.md` (brief Phase 2.3 mention; emphasize "crash-safe" property)
  - `tests/cli_run_crash_safe.rs` (OPTIONAL NEW — Worker may consolidate into existing unit tests in `src/heartbeat.rs::tests` instead; Architect leaves the integration-vs-unit split to Worker's call as Tầng 2)

- **KHÔNG sửa:**
  - `src/core/run.rs` (heartbeat append call site signature unchanged; retry loop unchanged; Constraint #12 from P009 honored)
  - `src/cli/mod.rs` (Constraint #1 re-instated from P006 V2 / P008 / P009 — no dispatch edit)
  - `src/cli/run.rs` (run subcommand unchanged — `core/run.rs` is the only direct caller of `heartbeat::append`)
  - `src/cli/status.rs` (status subcommand uses `read_last_n` — signature unchanged, behavior gracefully extended; no source change needed)
  - `src/core/status.rs` (calls `read_last_n(...).unwrap_or_default()` at line 80 — pre-existing caller-side silent-swallow of `Err`; not in P010 scope to fix; Worker logs in Discovery as known limitation; future BACKLOG item)
  - `src/mcp/tools.rs` (MCP `status` tool uses `read_last_n` via shared `core::status::run`; behavior change is internal-positive, no source change)
  - `src/alert.rs` (Constraint #11 from P008/P009 — env-free alert module preserved)
  - `src/runner.rs` (P009 Constraint #5 preserved — runner stays single-fire primitive)
  - `src/config.rs` (no schema change — `HeartbeatConfig.log_path` field unchanged, no new fields)
  - `src/launchd.rs`, `src/main.rs`, `src/core/init.rs`, `src/core/register.rs`, `src/core/unregister.rs` (heartbeat refactor is isolated to `src/heartbeat.rs`; transitive callers via `read_last_n` get the better behavior for free)
  - `HeartbeatRecord` struct fields (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) — schema preserved per Constraint #4 from P009

### Skills consulted

*(none — Architect sourced từ ARCHITECTURE.md, P004/P008/P009 phiếu + Discovery, BACKLOG.md, Sếp brief, POSIX/tempfile crate doctrine.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Architect KHÔNG có Bash/Grep — anchors sourced từ ARCHITECTURE.md, CHANGELOG.md P004/P008/P009 entries, P004/P009 Discovery Reports, INVARIANTS.md. Worker BẮT BUỘC verify thực tế tại Task 0.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/heartbeat.rs` contains `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>` (Phase 1.4 signature) — current impl uses `OpenOptions::append(true)` direct write | ARCHITECTURE.md §Modules row `src/heartbeat.rs`:56 + CHANGELOG P004:179 + Sếp brief state description | `[unverified]` — Architect inferred from docs only | ✅ line 32: signature exact match. `OpenOptions::new().append(true)` at line 40-44. |
| 2 | `src/heartbeat.rs` contains `pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>` — current impl `BufReader.lines().enumerate()` parses all lines; **V1 premise "fails entire read on parse error" is INCORRECT — current impl already skips ALL malformed lines via `eprintln!`** | P004 Discovery Report:47 + ARCHITECTURE.md §Modules row | `[unverified]` (V1) → ✅ corrected V2 per Worker Turn 1 | ✅ Signature ✅ (line 63). Behavior corrected V2: current impl SKIPS malformed lines via `eprintln!` (lines 78-84), does NOT "fail entire read". Task 2 still needed and correct (eprintln→tracing::warn + skip-all→skip-last-only + fail-loud-mid-file). §Vấn đề wording updated V2 to accurately describe three things that need to change. |
| 3 | `HeartbeatRecord` struct fields = `ts: DateTime<Utc>`, `label: String`, `exit_code: i32`, `duration_ms: u64`, `stdout_tail: String`, `stderr_tail: String` — schema locked since P004 | ARCHITECTURE.md §Heartbeat schema:276-291 + P009 Discovery Anchor #11 | `[verified]` per ARCHITECTURE.md cross-reference | ✅ Schema documented |
| 4 | `core::run::run` calls `heartbeat::append` exactly ONCE (single call site inside retry loop between the two matches) per P009 Constraint #12 + Discovery line 73 | P009 phiếu Constraint #12 + P009 Discovery `grep -c "heartbeat::append" src/core/run.rs` = 1 | `[verified]` per P009 Discovery | ✅ Single call site preserved |
| 5 | `src/cli/status.rs` (or `src/core/status.rs` post-P006) calls `heartbeat::read_last_n` — read path consumer | ARCHITECTURE.md §Modules row `src/core/status.rs`:50 ("launchd query + heartbeat read") | `[unverified]` | ✅ `src/core/status.rs:80`: `heartbeat::read_last_n(...).unwrap_or_default()`. Called with `unwrap_or_default` — Err propagation from mid-file corruption will return default (empty vec), not panic. **Pre-existing P005/P006-era design choice; `core/status.rs` in KHÔNG sửa list; Worker logs in Discovery as known limitation; future BACKLOG candidate.** See §Giải pháp "Caller-side note". |
| 6 | `tempfile = "3"` already in `[dev-dependencies]` in `Cargo.toml` per P004 setup | P004 Discovery Anchor #6 line 17 ("`Cargo.toml` `[dev-dependencies]` has `tempfile = "3"` + `tokio-test = "0.4"`") | `[verified]` per P004 Discovery | ✅ Confirmed |
| 7 | `Cargo.lock` already pins `tempfile` (transitive via dev-deps) — moving to `[dependencies]` adds zero compile time | P004 Discovery + general Cargo behavior | `[verified]` per Cargo doctrine | ✅ Standard behavior |
| 8 | `tempfile::NamedTempFile::new_in(parent_dir)` + `.persist(target_path)` is the canonical atomic-rename idiom — `persist` uses `std::fs::rename` under the hood | `tempfile` crate documentation (well-established Rust ecosystem) | `[unverified]` — Architect did not Read tempfile crate source | ✅ Verified via tempfile-3.27.0 source. `new_in(dir)` at `src/file/mod.rs:621`. `persist(new_path)` at line 767 calls `TempPath::persist` which calls `std::fs::rename`. Note: `persist` returns `Result<File, PersistError<File>>` (not `Result<(), ...>`); phiếu snippet's `.map_err(...)` is correct. `PersistError<F>` impls `std::error::Error` so `.context()` also works. |
| 9 | `temp_file.as_file().sync_all()` is the fsync call — flushes write to disk before rename. `NamedTempFile::as_file() -> &File`, `File::sync_all() -> io::Result<()>`. | std lib + tempfile docs | `[unverified]` | ✅ `as_file() -> &F` at `src/file/mod.rs:880`. For default `NamedTempFile<File>`, returns `&std::fs::File`. `sync_all()` is correct (syncs data + metadata — file size durable; `sync_data` skips metadata). |
| 10 | INVARIANTS.md max INV currently = 20 (slot for INV-21 free) — P009 added INV-20 | P009 Discovery `grep -l "INV-20" docs/security/INVARIANTS.md` = 1 hit + INVARIANTS.md inspection | `[verified]` per P009 Discovery | ✅ Slot free for INV-21 |
| 11 | Heartbeat file is **append-only** per PROJECT.md hard line #4 — `append` semantics preserved by temp+rename (we still ADD one line per call; never compact, never rotate, never reorder) | PROJECT.md:78 hard line #4 | `[verified]` | ✅ Compatible |
| 12 | P004 contract "Heartbeat write fail → Log warning to stderr, do NOT fail the run (task already succeeded)" — caller in `core::run::run` warn-continues on `heartbeat::append` error | ARCHITECTURE.md §Error handling table line 330 + P004 Discovery line 26 | `[verified]` per ARCHITECTURE.md | ✅ Caller contract preserved — temp+rename can fail (disk full, permission), warn-continue still correct |
| 13 | P009 retry loop calls `heartbeat::append` once per attempt (3 attempts = 3 atomic writes) — atomicity of each write is now independent | P009 Discovery + ARCHITECTURE.md §Heartbeat schema retry semantics paragraph | `[verified]` per P009 docs | ✅ Each attempt's write is its own atomicity unit |
| 14 | `tracing` macro `warn!` available in `src/heartbeat.rs` scope (already used by other modules — `core/run.rs`, `alert.rs`) | P008 Anchor + P009 Discovery use of `tracing::warn!` | `[unverified]` for heartbeat.rs specifically | ✅ (action required) `grep -n "use tracing\|tracing::" src/heartbeat.rs` → ZERO hits. `tracing` is NOT currently imported in heartbeat.rs. Worker adds `use tracing::warn;` at top of file in Task 2 (anticipated by phiếu). `tracing = "0.1"` is in `[dependencies]` so import is valid. |
| 15 | Phase 2 baseline post-P009 = 133 tests passing | P009 Discovery sub-mechanism B "97/97" lib + "4/4" cli_run_retry + others; Sếp brief "Tests: 133/133" | `[verified]` per P009 Discovery + Sếp brief | ✅ Baseline 133 |
| 16 | `std::fs::rename` is atomic on POSIX (same filesystem) per POSIX `rename(2)` spec + Rust std docs guarantee | POSIX + Rust std `std::fs::rename` documentation | `[verified]` per POSIX standard | ✅ Atomic guarantee on same-fs |
| 17 | Phase 2 sprint = 10 phiếu total (P001-P010); P010 is the LAST → sprint closes after this ships | Sếp brief + BACKLOG.md Phase 2 items (2.1 ✅ P008, 2.2 ✅ P009, 2.3 = P010 this) | `[verified]` per Sếp brief | ✅ Sprint-closing phiếu |
| 18 | `parent_dir` of `log_path` exists by the time `append` is called — current `append` calls `fs::create_dir_all(parent)` per P004 + INV-15 | INV-15 "`fs::create_dir_all(parent)` call in `append` may create directories" + ARCHITECTURE.md | `[verified]` per INV-15 | ✅ Worker preserves `create_dir_all` call before temp file creation |

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND). Cap = 3 turns.

**Phiếu version:** V2 (Architect Turn 1 response — §Vấn đề wording corrected per Worker Anchor #2 finding; corollary status.rs note added; Task 2 spec unchanged)

### Turn 1 — Worker Challenge

**Date:** 2026-05-27

**Anchor verification results (all 18):**

- Anchor #1: ✅ `grep -n "pub fn append" src/heartbeat.rs` → line 32: `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>`. `OpenOptions::new()` + `.append(true)` confirmed at line 40-44 (direct-write impl, exactly as phiếu describes).
- Anchor #2: ⚠️ **PARTIAL MISMATCH — see O1.1 below.** `read_last_n` exists at line 63 with correct signature. BUT current body does NOT "fail entire read on parse error" — it already SKIPS malformed lines via `eprintln!` (lines 80-83). The phiếu's §Vấn đề hiện tại description ("if the last line is corrupt, the entire read fails") is inaccurate. Evidence: `src/heartbeat.rs:80-83` + existing test `read_last_n_skips_malformed_line` at line 181 confirms skip-all behavior. Task 2 is still needed (eprintln→tracing::warn, skip-all→skip-only-last + fail-loud mid-file), but the motivation premise in §Context is partially incorrect.
- Anchor #3: ✅ `HeartbeatRecord` struct confirmed at lines 17-25: ts/label/exit_code/duration_ms/stdout_tail/stderr_tail — schema locked.
- Anchor #4: ✅ `grep -c "heartbeat::append" src/core/run.rs` = 1. P009 Constraint #12 preserved.
- Anchor #5: ✅ `grep -rn "read_last_n" src/` → `src/core/status.rs:80`. Called as `heartbeat::read_last_n(...).unwrap_or_default()`. Note: `#[allow(dead_code)]` on `read_last_n` (line 62 of heartbeat.rs) — this attribute is stale since P005/P006 wired it; Worker should remove it in Task 2 (Tầng 2 cleanup, within scope).
- Anchor #6: ✅ `grep "^tempfile" Cargo.toml` → `tempfile = "3"` is in `[dev-dependencies]`. Confirmed.
- Anchor #7: ✅ `Cargo.lock` already pins `tempfile = "3.27.0"` — moving to `[dependencies]` adds zero compile time.
- Anchor #8: ✅ `NamedTempFile::new_in(dir: P)` confirmed at `tempfile-3.27.0/src/file/mod.rs:621`. Returns `io::Result<NamedTempFile>`. `persist(new_path)` at line 767 calls `std::fs::rename` via `TempPath::persist`. POSIX atomic rename confirmed.
- Anchor #9: ✅ `as_file() -> &F` confirmed at line 880 (returns `&File` for default `NamedTempFile<File>`). `File::sync_all()` is the correct method (syncs data + metadata; `sync_data` skips metadata which could leave file size non-durable). Both confirmed via std docs + tempfile source.
- Anchor #10: ✅ `grep -c "^### INV-" docs/security/INVARIANTS.md` = 20. Slot for INV-21 is free.
- Anchor #11: ✅ Append-only semantics preserved by temp+rename (adds one line per call, never compacts).
- Anchor #12: ✅ P004 warn-continue contract preserved — `core::run::run` `.unwrap_or_else(|e| warn!(...))` pattern unaffected by heartbeat internal change.
- Anchor #13: ✅ P009 retry loop confirmed: `grep -c "heartbeat::append" src/core/run.rs` = 1 (inside retry loop), each attempt is independent atomicity unit.
- Anchor #14: ✅ **`tracing` NOT in `src/heartbeat.rs` scope** — `grep -n "use tracing\|tracing::" src/heartbeat.rs` returns zero hits. Worker MUST add `use tracing::warn;` at top of `heartbeat.rs` in Task 2. This is anticipated by the phiếu ("If `tracing` not in scope in heartbeat.rs, Worker adds `use tracing::warn;`") — not a blocking objection, confirmed Tầng 2 action.
- Anchor #15: ✅ Baseline = 133 tests. Confirmed: `cargo test --all` returns 133 passed (97 lib + 3 + 4 + 7 + 6 + 4 + 3 + 4 + 5 = 133).
- Anchor #16: ✅ POSIX `rename(2)` atomic same-fs guarantee confirmed. `NamedTempFile::persist` uses `std::fs::rename` internally.
- Anchor #17: ✅ P010 is the 10th and final phiếu of the sprint. BACKLOG.md Phase 2 items = 2.1 (P008) + 2.2 (P009) + 2.3 (P010).
- Anchor #18: ✅ `grep -n "create_dir_all" src/heartbeat.rs` → line 36: `fs::create_dir_all(parent)`. Worker preserves this in Task 1's new body.

**Verification Anchors table updates:**
- Anchor #1: ✅ (was ⏳)
- Anchor #2: ⚠️ (was ⏳) — behavior mismatch in §Context description (current code skips-all, not fails-on-corrupt); Task 2 still needed and correct
- Anchor #5: ✅ (was ⏳) — called in `src/core/status.rs:80` via `unwrap_or_default`; `#[allow(dead_code)]` stale
- Anchor #8: ✅ (was ⏳) — `new_in` + `persist` confirmed via tempfile-3.27.0 source
- Anchor #9: ✅ (was ⏳) — `as_file() -> &F` confirmed; `sync_all` is correct (not `sync_data`)
- Anchor #14: ✅ (was ⏳) — tracing NOT in scope in heartbeat.rs; Worker adds `use tracing::warn;` in Task 2

**Objections (Tầng 1 only):**

- [O1.1] **Context description mismatch — `read_last_n` already skips, does not fail on parse error.** `src/heartbeat.rs:78-84` shows current `match` arm for `Err(err)` calls `eprintln!("warning: skipping malformed heartbeat line {i}: {err}")` and continues — NOT propagating error. The phiếu §Vấn đề hiện tại states "the entire read fails". This premise is wrong. The phiếu footnote on line 59 in the file doc comment CORRECTLY notes "Malformed lines are skipped with a stderr warning" — contradiction between the doc comment and §Context.

  **Impact assessment:** The Task 2 implementation IS still correct and needed for the RIGHT reasons: (a) current behavior skips ALL malformed lines silently (using `eprintln!` not `tracing::warn!`) — violates INV-21 sub-rule 2's requirement to fail loud on mid-file corruption; (b) current behavior doesn't distinguish last-vs-not-last line; (c) `eprintln!` must be replaced with `tracing::warn!`. The Task 2 work is valid. Only the §Vấn đề hiện tại framing needs a correction.

  **Architect action needed:** Confirm Worker may proceed to EXECUTE with the corrected understanding: Task 2 is needed not because `read_last_n` "fails on corrupt" (it doesn't) but because it (a) uses `eprintln!` instead of `tracing::warn!` and (b) silently skips ALL corrupt lines rather than applying the last-vs-mid distinction required by INV-21. Existing test `read_last_n_skips_malformed_line` passes a mid-file corrupt line — under the new protocol, this test behavior changes (mid-file corrupt line now fails loud). Worker updates the test.

**Heads-up resolutions (Architect's 5 items):**

1. **temp+rename approach (vs fsync-append):** ✅ Worker agrees. POSIX rename is a hard atomicity guarantee independent of write size. `fsync-append` has silent degradation above PIPE_BUF. Conservative choice is correct for a sprint-closing fault-tolerance phiếu.

2. **tempfile dev→runtime promotion:** ✅ Worker agrees. `tempfile::NamedTempFile::new_in` + `.persist` is exactly the canonical Rust atomic-write idiom. Rolling manual UUID+rename is ~40 LOC of error-prone reinvention. `tempfile` is already in lock file, zero marginal cost. Option A (single `[dependencies]` entry) confirmed correct — Cargo includes `[dependencies]` in test compilation, so dev-dep entry becomes redundant.

3. **Constraint #14 same-filesystem temp (`NamedTempFile::new_in` not `::new`):** ✅ Worker confirms. `new_in(log_path.parent())` is the ONLY acceptable pattern. `NamedTempFile::new()` uses `std::env::temp_dir()` which is commonly a different filesystem (`/private/var/folders/...` on macOS) — cross-fs rename degrades to copy+delete and loses POSIX atomicity. Worker hardcodes `new_in(parent_dir)` and adds the `grep "NamedTempFile::new(" src/heartbeat.rs` = ZERO hits check per Nghiệm thu.

4. **`heartbeat::append` signature preservation:** ✅ Worker confirms achievable. Signature `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>` needs zero change — the new body uses exactly the same two inputs. Single call site in `core/run.rs` at line confirmed by `grep -c "heartbeat::append" = 1`. No source change needed in callers.

5. **Sprint summary in CHANGELOG:** ✅ Worker agrees. Sprint summary table in Task 8 format is appropriate. Worker fills `2026-MM-DD` with actual ship date and exact test count after `cargo test --all`.

**Additional Tầng 2 findings (self-decided, no escalation needed):**

- `#[allow(dead_code)]` at `src/heartbeat.rs:62` is stale — `read_last_n` is called by `core/status.rs:80`. Worker removes this attribute in Task 2 (within scope of refactoring `read_last_n`, no new surface change).
- `sample_record()` helper in existing tests takes zero args (returns fixed record). Phiếu Task 5 proposes a `sample_record(i: u32, label: &str)` helper with two params. Worker will add a second helper with the two-param signature (or adapt the tests to use the existing helper where feasible) — Tầng 2 stylistic call.
- `PersistError<File>` implements `std::error::Error` (confirmed via tempfile-3.27.0 source `impl<F> error::Error for PersistError<F>`), so `.context(...)` from anyhow works as an alternative to `.map_err(|e| anyhow::anyhow!(...))`. Worker picks whichever matches existing heartbeat.rs error style (`.with_context(|| ...)` pattern).

**Additional informational finding (not blocking — O1.1 corollary):**

The phiếu Task 2 lưu ý asks Worker to verify P005 handles `read_last_n` returning `Err`. Confirmed: `src/core/status.rs:80` uses `.unwrap_or_default()` — Err is silently converted to empty Vec (shows "no heartbeats" in status output, no panic). This means mid-file corruption in the status path shows zero heartbeats rather than an explicit error message — partial violation of "fail loud" spirit from PROJECT.md hard line #5. However: (a) the phiếu's escalation trigger is "if P005 unwraps and panics" — it does not panic; (b) `core/status.rs` is in the KHÔNG sửa list (Worker cannot change it per Constraint); (c) `read_last_n` itself returns the loudest possible `Err` — the caller silencing it is a pre-existing design choice (P005 contract). Worker logs this in Discovery Report as a known limitation. The "fail loud" guarantee in INV-21 sub-rule 2 applies to the `read_last_n` return value — callers may choose to swallow or propagate per their own contract.

**Status:** Worker accepted V1 with one ⚠️ (Anchor #2 context mismatch — non-blocking, Task 2 still correct and needed). No Tầng 1 blockers requiring escalation. Ready for Chủ nhà approval gate (or Architect quick-RESPOND to confirm the ⚠️ does not invalidate the approach).

**Recommendation:** Architect RESPOND to confirm O1.1 is acknowledged (not blocking) and authorize EXECUTE. Alternatively, if Chủ nhà autonomous mode is active, Worker can proceed to EXECUTE with the understanding that §Vấn đề hiện tại description is partially inaccurate but Task 2 work is correct and needed.

### Turn 1 — Architect Response (phiếu V2)

**Date:** 2026-05-27

- **[O1.1] → ACCEPT.** Worker's verification of `src/heartbeat.rs:78-84` is authoritative — current `read_last_n` already skips ALL malformed lines via `eprintln!`, does not propagate parse errors. V1's §Vấn đề hiện tại framing ("the entire read fails") was Architect's incorrect inference from P004 Discovery wording (Architect cannot Read source per envelope; relied on doc summary which understated the existing skip behavior). The Task 2 implementation spec is **unchanged** — Worker confirmed it is still correct and needed for the RIGHT reasons (per Worker's impact assessment):
  1. **Wrong granularity** — current skips ALL malformed (including mid-file); INV-21 sub-rule 2 requires last-line-only tolerance + mid-file fails loud.
  2. **Wrong logging channel** — `eprintln!` bypasses tracing; must be `tracing::warn!` (INV-13).
  3. **Caller-side note added** (corollary, NOT new scope) — `src/core/status.rs:80` uses `.unwrap_or_default()` which silently absorbs the new mid-file `Err` into empty Vec; `core/status.rs` is in KHÔNG sửa list (added explicitly V2 for clarity); Worker logs as known limitation in Discovery; future BACKLOG candidate.

  **Actions taken in V2:**
  - **§Vấn đề hiện tại rewritten** to accurately describe current code (already skips all via `eprintln!`) AND enumerate the three properties that need to change (wrong granularity, wrong logging channel, caller-side silent failure as informational corollary).
  - **§Giải pháp "Recovery — partial last line tolerance in `read_last_n`" updated** to explicitly note this REPLACES the existing `eprintln!` skip-all behavior with last-only + tracing + mid-file-fails-loud, and TIGHTENS mid-file (previously silently swallowed).
  - **§Giải pháp added "Caller-side note (informational, NOT in scope)"** documenting the `core/status.rs:80` `.unwrap_or_default()` silent-absorb behavior + explicit BACKLOG punt + explicit Discovery log requirement.
  - **§Scope KHÔNG sửa list updated** to explicitly call out `src/core/status.rs` separately (previously bundled with other `core/*` modules) with the caller-side rationale spelled out.
  - **Anchor #2 row updated** in the Verification Anchors table — marker bumped from `[unverified]` to `[unverified] (V1) → ✅ corrected V2 per Worker Turn 1`, Kết quả column carries the corrected behavior description.
  - **Anchor #5 row updated** to mention the pre-existing P005/P006-era design choice + Discovery requirement + BACKLOG candidacy.
  - **Existing test `read_last_n_skips_malformed_line` semantic flip noted** — under new protocol that test (which passes a mid-file corrupt line and asserts skip) must FAIL loud; Worker updates the test per §Giải pháp + Task 2 + Task 5. This was implicit in V1 (new tests for last-line-skip + mid-file-fail were spec'd) but is now explicit.

- **Heads-up resolutions 1-5** — all ACCEPT per Worker confirmation. No changes needed.

- **Worker's Tầng 2 self-decisions** — all approved (stale `#[allow(dead_code)]` removal, `sample_record` helper signature evolution, `PersistError<File>` `.context()` alternative).

- **Task 2 spec UNCHANGED.** Worker confirmed Architect's implementation snippet is correct. Only the motivating prose in §Vấn đề hiện tại + §Giải pháp evolved V1 → V2.

- **All other Tasks (1, 3-8) UNCHANGED.**

- **All Constraints (1-16) UNCHANGED.**

**Status:** ✅ RESPONDED — phiếu bumped V1 → V2. No Tầng 1 blockers. No DEFER TO SẾP required. Ready for Worker (CHALLENGE) re-verification of consensus OR direct EXECUTE per autonomous mode.

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Turn 2 — Worker Challenge

**Date:** 2026-05-27

**Mode:** CHALLENGE — Turn 2 (last). Verifying V2 incorporates all 10 Architect changes declared in Turn 1 Architect Response.

**Anchor re-verification (spot-check — code unchanged since Turn 1):**

- Anchor #1: ✅ `grep -n "pub fn append" src/heartbeat.rs` → line 32 — signature exact match. Still direct-write (OpenOptions). Task 1 work still needed.
- Anchor #2: ✅ V2 §Vấn đề correctly describes skip-all via `eprintln!` at line 81. `grep -n "eprintln!" src/heartbeat.rs` = 1 hit (line 81). Task 2 work still needed and correct.
- Anchor #4: ✅ `grep -c "heartbeat::append" src/core/run.rs` = 1. P009 Constraint #12 preserved.
- Anchor #5: ✅ `grep -n "read_last_n\|unwrap_or_default" src/core/status.rs` → line 80 `.unwrap_or_default()`. `core/status.rs` still in KHÔNG sửa list (line 1016 of phiếu).
- Anchor #6: ✅ `grep -n "tempfile" Cargo.toml` → line 28 under `[dev-dependencies]` (section at line 27). Task 3 still needed.
- Anchor #10: ✅ `grep -c "^### INV-" docs/security/INVARIANTS.md` = 20. Slot for INV-21 still free.

**V2 changes present — verification (all 10):**

1. ✅ §Vấn đề hiện tại rewritten (lines 17-28) — accurately describes current skip-all via `eprintln!` + 3 reasons for change (granularity, logging channel, caller-side silent failure).
2. ✅ §Giải pháp Recovery subsection (lines 63-74) — explicit REPLACES `eprintln!` skip-all + TIGHTENS mid-file behavior note.
3. ✅ Caller-side note for `core/status.rs` `.unwrap_or_default()` present in both §Context (lines 73-75) and §Giải pháp "Caller-side note (informational, NOT in scope)" paragraph (lines 73-75 of §Giải pháp).
4. ✅ Anchor #2 result column updated: `[unverified] (V1) → ✅ corrected V2 per Worker Turn 1` with corrected behavior description (phiếu line 123). Anchor #5 result column updated with unwrap_or_default + BACKLOG candidacy note (phiếu line 126).
5. ✅ Task 2 "Tìm" section updated: "REVISED V2" label + accurate description of current skip-all via `eprintln!` behavior (phiếu lines 442-448).
6. ✅ Existing test `read_last_n_skips_malformed_line` semantic flip made explicit (phiếu lines 71-72, 547-548, 803-804) — update asserts mid-file → `is_err()`, recommend delete-old-name.
7. ✅ CHANGELOG Task 8 refined (phiếu lines 913-920) — `read_last_n` description says "Tightened prior P004 silent-skip-all-malformed behavior"; caller-side note with unwrap_or_default + Discovery reference included.
8. ✅ Files cần sửa + KHÔNG sửa tables updated (phiếu lines 997-1024) — `src/core/status.rs` explicitly listed in KHÔNG sửa (line 1016) with unwrap_or_default rationale.
9. ✅ Constraint #15 strengthened (phiếu line 1058) — references test update explicitly. Constraint #16 (phiếu line 1060) — explicitly names `core/status.rs:80` `.unwrap_or_default()` as off-limits for P010 scope.
10. ✅ Nghiệm thu checks present: `git diff src/core/status.rs` empty (line 1073); `grep "eprintln!" src/heartbeat.rs` = ZERO hits (line 1086); Discovery Report checklist includes mandatory `unwrap_or_default()` known-limitation note (line 1125).

**Objections (Tầng 1):** None. All V2 changes verified present and internally consistent. No new code-reality mismatches found. Codebase state unchanged since Turn 1 (no commits between Turn 1 and Turn 2 per `git status` clean).

**Additional observations (informational only — no escalation):**

- `Cargo.toml` tempfile at line 28 under `[dev-dependencies]` (confirmed; Task 3 move still needed).
- `INV` count still 20 (Task 4 still needed).
- No scope creep detected in phiếu V2 — all additions are corollary notes or explicit KHÔNG sửa constraints.

**Worker verdict:** V2 is consistent, accurate, and complete. Ready for Chủ nhà approval gate.

**Status:** ✅ ACCEPTED — Worker accepted V2. No Tầng 1 challenges. Ready for EXECUTE.

### Final consensus
- Phiếu version: V2
- Total turns: 2
- Approved (autonomous mode — Sếp brief authorized autonomous): 2026-05-27 — code execution may begin

---

## Debug Log (advisory-cron specific)

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | N/A — no new launchd plist, heartbeat write fires from existing `run` path | — | | N/A |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test --lib heartbeat` (new unit tests for atomic write + corrupt-line tolerance) | all pass | | |
| B (capability) | `cargo test --test cli_run` (P004 integration regression — heartbeat still written) | still pass | | |
| B (capability) | `cargo test --test cli_run_retry` (P009 integration regression — N attempts = N atomic writes) | still pass | | |
| B (capability) | `cargo test --test cli_run_alert` (P008 integration regression) | still pass | | |
| C (migration) | `git diff src/heartbeat.rs` shows `append` body refactored + `read_last_n` body refactored, BUT `HeartbeatRecord` struct fields unchanged | struct unchanged, bodies changed | | |
| C (migration) | Verify legacy heartbeat file with partial last line (manually crafted) is readable by new `read_last_n` (corrupt-line tolerance unit test) | partial last line warned + skipped, prior records returned | | |
| D (persistence) | `grep -l "INV-21" docs/security/INVARIANTS.md` | ≥1 hit | | |
| D (persistence) | `grep -l "crash-safe\|atomic rename\|temp+rename" docs/ARCHITECTURE.md` | ≥1 hit (in §Heartbeat schema atomicity subsection) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (only `tempfile` moves dev-dep → dep, version pin unchanged) | | |
| E (env drift) | `cargo build --release` clean target | exit 0, binary ≤7MB (tempfile already in lockfile per P004 — no size delta) | | |

---

## Nhiệm vụ

### Task 0 — Anchor verification (BẮT BUỘC TRƯỚC mọi Task khác)

**Mục đích:** Architect không có Bash/Grep — anchor table dựa docs. Worker chạy grep commands, fill Kết quả column, escalate qua Debate Log Turn 1 nếu phát hiện sai lệch (e.g. `append` already takes `&mut File` instead of `&Path`, or `read_last_n` doesn't exist, or `tracing` macros not in scope in heartbeat.rs).

**Lệnh chạy (anchors 1, 2, 5, 8, 9, 14 — others verified by Architect from docs):**

```bash
grep -n "pub fn append" src/heartbeat.rs                                      # #1 expect single signature line
grep -n "OpenOptions::append\|OpenOptions::new" src/heartbeat.rs              # #1 confirm current direct-write impl
grep -n "pub fn read_last_n" src/heartbeat.rs                                 # #2 expect single signature line
grep -n "BufReader\|lines()" src/heartbeat.rs                                 # #2 confirm current parse-all impl
grep -rn "read_last_n" src/                                                   # #5 list all callers (expect core::status or cli::status)
grep -c "heartbeat::append" src/core/run.rs                                   # #4 expect exactly 1 (P009 Constraint #12 preserved)
grep -n "use tracing\|tracing::" src/heartbeat.rs                             # #14 check if tracing already imported in heartbeat.rs
grep "^tempfile" Cargo.toml                                                   # #6 confirm current location (dev-dep)
grep -c "^### INV-" docs/security/INVARIANTS.md                                # #10 expect 20 (INV-21 slot free)
grep -n "fs::create_dir_all" src/heartbeat.rs                                 # #18 confirm current `append` creates parent dir
```

**Output:** fill Verification Anchors table → if mọi anchor ✅ → proceed Task 1. If ⚠️/❌ → write Debate Log Turn 1 objection (Architect RESPOND mode required).

**Special focus for Worker:**

- **Anchor #1 + #2:** verify current `append` and `read_last_n` shapes match Architect's understanding (single function each, `&Path` first arg, signatures as documented). If `append` already uses `tempfile` or `read_last_n` already has corrupt-line tolerance (e.g. P004 worker added it without doc update), the entire phiếu may be obsolete — escalate IMMEDIATELY with `file:line` evidence.

- **Anchor #5:** confirm `read_last_n` is called from at least one place — if it's `#[allow(dead_code)]` and unused (e.g. P005 didn't wire it), the recovery hardening is still required but the integration test plan in Task 5 should pivot accordingly. Architect expects P005 wired it per CHANGELOG; verify.

- **Anchor #4 (P009 Constraint #12 preservation):** confirm `grep -c "heartbeat::append" src/core/run.rs` is STILL exactly 1 BEFORE editing. If P010's setup somehow already broke it (shouldn't — P010 doesn't touch core/run.rs), STOP and escalate.

- **Anchor #14 (tracing scope):** if `tracing` is NOT imported in `src/heartbeat.rs`, Worker MUST add `use tracing::warn;` at the top of `heartbeat.rs` (Task 2 below). This is a 1-line addition, not scope creep — it's required for the corrupt-line warn path.

---

### Task 1: Refactor `src/heartbeat.rs::append` to temp+fsync+rename

**File:** `src/heartbeat.rs`

**Tìm:** Worker uses Task 0 Anchor #1 results to locate the `pub fn append` definition. Current body shape (per docs):
- Calls `fs::create_dir_all(log_path.parent().unwrap())` (or similar) to ensure parent dir exists.
- Opens file via `OpenOptions::new().create(true).append(true).open(log_path)?`.
- Calls `serde_json::to_string(record)?` to produce the JSONL line.
- Writes `<line>\n` directly to the open file.
- Returns `Ok(())`.

**Thay bằng / Thêm:** Replace the *body* of `pub fn append` (keep the signature exactly as-is — `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>`). New body:

```rust
pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()> {
    // Phase 2.3 — Crash-safe atomic-rename write protocol (P010, INV-21).
    //
    // Replaces the Phase 1.4 direct-append (`OpenOptions::append(true)` + write)
    // implementation. The previous impl was not crash-safe: a kill mid-write
    // could leave the JSONL file with a truncated final line. Under P009 retry
    // policy, a single `advisory-cron run` invocation may call this fn 3+ times
    // (once per retry attempt), tripling the crash-surface area. This impl
    // guarantees that each call either fully appends a well-formed line OR
    // leaves the file unchanged — there is no observable partial state.
    //
    // Protocol:
    //   1. Ensure parent dir exists (carry-over from Phase 1.4; INV-15).
    //   2. Read existing file contents into memory (empty if file absent).
    //   3. Serialize the new record to a JSONL line (record + `\n`).
    //   4. Append the new line to the in-memory buffer.
    //   5. Create a NamedTempFile in the SAME directory as the target file
    //      (atomic rename requires same filesystem).
    //   6. Write the full buffer to the temp file.
    //   7. fsync the temp file (sync_all — data + metadata, so file size is
    //      durable across power loss).
    //   8. Atomically persist (rename) the temp file over the target file.
    //
    // If any step fails before the persist call, the temp file is auto-cleaned
    // on Drop and the target file is untouched. Caller (`core::run::run`)
    // already log-warn-continues on `Err` per P004 contract — task is NOT
    // failed on heartbeat write failure (heartbeat is observability, not the
    // task outcome itself).

    // Step 1 — ensure parent dir (Phase 1.4 carry-over, INV-15).
    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating parent dir for {}", log_path.display()))?;
        }
    }

    // Step 2 — read existing contents (empty if file absent).
    let mut buffer = match std::fs::read(log_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", log_path.display())),
    };

    // Step 3 — serialize new record.
    let line = serde_json::to_string(record).context("serializing HeartbeatRecord to JSON")?;

    // Step 4 — append to in-memory buffer.
    buffer.extend_from_slice(line.as_bytes());
    buffer.push(b'\n');

    // Step 5 — create temp file in same directory (required for atomic rename).
    let parent_dir = log_path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent_dir)
        .with_context(|| format!("creating temp file in {}", parent_dir.display()))?;

    // Step 6 — write full buffer.
    use std::io::Write;
    temp.write_all(&buffer)
        .with_context(|| format!("writing to temp file {}", temp.path().display()))?;

    // Step 7 — fsync (sync_all = data + metadata so file size is durable).
    temp.as_file()
        .sync_all()
        .with_context(|| format!("fsyncing temp file {}", temp.path().display()))?;

    // Step 8 — atomic rename. `persist` calls `std::fs::rename` under the
    // hood, which is atomic on POSIX for same-filesystem renames.
    temp.persist(log_path)
        .map_err(|e| anyhow::anyhow!("atomic rename to {} failed: {}", log_path.display(), e))?;

    Ok(())
}
```

**Lưu ý:**

- **Imports needed at top of `heartbeat.rs`** (Worker verify what's already imported; add only what's missing):
  - `use std::path::Path;` (likely already present — signature uses `&Path`)
  - `use anyhow::{Context, Result};` (likely present — P004 used anyhow)
  - `use tracing::warn;` (for Task 2 corrupt-line warn; may already be present per Anchor #14)
  - `tempfile` crate available at runtime per Task 4 Cargo.toml change
  - `std::io::Write` for `write_all` (idiomatic to `use` inside the fn for hygiene — phiếu snippet does `use std::io::Write;` inline)

- **`temp.persist(log_path)` returns `Result<File, PersistError>`** — `PersistError` is NOT `std::io::Error`, hence the `.map_err(|e| anyhow::anyhow!(...))` instead of `.with_context(...)`. Worker MAY use `temp.persist(log_path).context(...)` if `PersistError` impls `std::error::Error` (it does — tempfile crate provides `From` impls); pick whichever style matches existing `heartbeat.rs` error idiom.

- **`anyhow::Context` import** — if not already imported, add `use anyhow::Context;`. Pre-P009 code likely uses this pattern; preserve existing style.

- **No `unsafe { }`** — entire protocol is safe Rust. Hard Stop #7 honored.

- **Performance note (Discovery candidate):** This implementation reads the entire heartbeat file into memory on every append. For Sếp's expected usage (~365 KB/year), this is microseconds. If `cargo test --test cli_run_retry` (running ~3 appends per test × multiple tests) shows measurable slowdown, log it in Discovery Report. Architect rejects optimization in P010 (Hard Stop #6 refactor-outside-scope, AI Bias Warnings #1 "vấn đề Sếp ĐANG có" — no scaling need).

- **`fs::create_dir_all` idempotency** — returns `Ok(())` if dir already exists (per std docs). Safe to call on every append.

- **TempFile cleanup on failure** — `NamedTempFile` impls `Drop` which `unlink`s the temp file. If any step 5-7 fails (returns `Err` via `?`), Drop fires and cleans up. Step 8 `.persist(...)` consumes the `NamedTempFile`, so successful rename also leaves no temp leftover. Worker confirms no `tmp.*` files accumulate by running `ls $(dirname heartbeat.jsonl)` after a test.

- **Same-filesystem assumption** — `log_path.parent()` is the temp file's parent, which IS the target file's parent. Same directory ⇒ same filesystem (under normal conditions). If a user bind-mounts a different fs at the heartbeat dir's parent (extremely unusual), this assumption could break — escalation: that's user error, not advisory-cron's concern. NOT a P010 scope item.

- **Add unit tests in `src/heartbeat.rs::tests`** (Task 5 below has the test matrix — keep here pointer-only to avoid duplication):
  - `append_creates_file_when_missing`
  - `append_preserves_existing_content`
  - `append_multiple_times_grows_file`
  - `append_atomic_rename_no_temp_leftover`
  - `append_concurrent_attempt_no_corruption` (smoke-only, see Task 5)

---

### Task 2: Harden `src/heartbeat.rs::read_last_n` against partial last line

**File:** `src/heartbeat.rs`

**Tìm:** Worker uses Task 0 Anchor #2 results to locate the `pub fn read_last_n` definition. Current body shape (per Worker Turn 1 verification, REVISED V2):
- Opens file via `File::open(log_path)?`.
- Wraps in `BufReader::new(file)`.
- Iterates `reader.lines().enumerate()` — for each line, attempts `serde_json::from_str::<HeartbeatRecord>(&line)`. On `Err`, **calls `eprintln!("warning: skipping malformed heartbeat line {i}: {err}")` and continues** (NOT `?` propagation). Currently skips ALL malformed lines silently regardless of position. Existing test `read_last_n_skips_malformed_line` asserts this.
- Collects into `Vec<HeartbeatRecord>`.
- Returns last `n` via `vec.into_iter().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect()` or similar.

**What needs to change** (per V2 §Vấn đề + §Giải pháp): (a) replace `eprintln!` with `tracing::warn!` (INV-13); (b) distinguish last-line (warn+skip) from mid-file (fail-loud, per INV-21 sub-rule 2); (c) update existing `read_last_n_skips_malformed_line` test (which passes a mid-file corrupt line) to assert mid-file → `Err`, and add a new test asserting last-line → warn+skip+return-prior.

**Thay bằng / Thêm:** Replace the *body* of `pub fn read_last_n` (keep the signature exactly as-is — `pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>`). Also remove the stale `#[allow(dead_code)]` attribute at line 62 (per Worker Turn 1 Tầng 2 finding). New body:

```rust
pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>> {
    // Phase 2.3 — Partial-last-line tolerance (P010, INV-21 read-path).
    //
    // Pre-P010 heartbeat files may contain a truncated last line from a
    // historical interrupted write (the Phase 1.4 direct-append impl was not
    // crash-safe — see Task 1 for the new atomic-write protocol). The read
    // path must tolerate ONE such partial line at the END of the file.
    //
    // Tolerance policy:
    //   - Last line parse failure → `tracing::warn!` + skip + continue. This
    //     is the recovery path for legacy partial writes.
    //   - Non-last line parse failure → `Err`. Mid-file corruption is
    //     unexpected (atomic-write prevents it going forward, and a partial
    //     line can only be the LAST line by construction of how truncation
    //     works). If we see mid-file corruption, something else is wrong
    //     (external tampering, disk failure) and we MUST surface it loud.
    //
    // V2 NOTE: This REPLACES the prior P004 behavior of silently skipping ALL
    // malformed lines (via `eprintln!`). The prior behavior masked mid-file
    // corruption — violation of PROJECT.md hard line #5. Existing test
    // `read_last_n_skips_malformed_line` is updated to assert the new
    // last-vs-mid distinction.

    use std::io::{BufRead, BufReader};

    let file = match std::fs::File::open(log_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("opening {}", log_path.display())),
    };

    let reader = BufReader::new(file);
    let raw_lines: Vec<String> = reader
        .lines()
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("reading lines from {}", log_path.display()))?;

    let mut records: Vec<HeartbeatRecord> = Vec::with_capacity(raw_lines.len());
    let last_idx = raw_lines.len().saturating_sub(1);

    for (idx, line) in raw_lines.iter().enumerate() {
        if line.trim().is_empty() {
            // Tolerate blank lines silently (could appear if a previous write
            // wrote ONLY the trailing newline and was interrupted before the
            // record bytes — extremely rare with the new atomic protocol but
            // defensive). Blank lines are never records.
            continue;
        }
        match serde_json::from_str::<HeartbeatRecord>(line) {
            Ok(rec) => records.push(rec),
            Err(parse_err) if idx == last_idx => {
                tracing::warn!(
                    log_path = %log_path.display(),
                    error = %parse_err,
                    "partial or corrupt last heartbeat line detected (likely pre-P010 interrupted write); skipping"
                );
                // Skip this line, do not propagate error.
            }
            Err(parse_err) => {
                // Mid-file corruption — propagate. Should not happen with the
                // atomic-write protocol; indicates external tampering or disk
                // damage.
                return Err(parse_err).with_context(|| {
                    format!(
                        "parsing heartbeat line {} of {} (mid-file corruption — non-last-line parse failure)",
                        idx + 1,
                        log_path.display()
                    )
                });
            }
        }
    }

    // Take last `n` records (preserve chronological order — oldest of the
    // returned slice first, newest last; matches the existing P004 contract
    // per P005 status reporter expectation).
    if records.len() > n {
        let skip = records.len() - n;
        Ok(records.into_iter().skip(skip).collect())
    } else {
        Ok(records)
    }
}
```

**Lưu ý:**

- **Return-order contract** — Worker verify the existing `read_last_n` returns records in CHRONOLOGICAL order (oldest-first, newest-last) by reading the P005 status reporter code OR running an existing unit test. If existing contract is REVERSED (newest-first), adjust the final `.skip(skip).collect()` line accordingly. Architect's snippet assumes chronological-order — if reversed, the snippet's last 4 lines flip to `.rev().take(n).collect::<Vec<_>>().into_iter().rev().collect()` style.

- **Empty file → `Ok(Vec::new())`** — current behavior likely matches; preserved by the `NotFound` early return + empty-records loop exit.

- **`tracing::warn!` requires `use tracing::warn;`** at top of file (Task 0 Anchor #14 — Worker confirmed tracing is NOT yet imported in heartbeat.rs). If `tracing` is not in `Cargo.toml` `[dependencies]`, escalate — but per CLAUDE.md tech stack, `tracing` + `tracing-subscriber` are core deps, so this should NOT happen.

- **Existing test `read_last_n_skips_malformed_line` semantic flip** (per Worker Turn 1 verification + V2 §Vấn đề / §Giải pháp): currently passes a mid-file corrupt line and asserts skip. Under the new protocol, mid-file → `Err`. Worker UPDATES this test to assert `result.is_err()` for the mid-file case AND adds a new test (per Task 5 matrix) for the last-line warn+skip case.

- **Remove stale `#[allow(dead_code)]`** at line 62 of heartbeat.rs (per Worker Turn 1 Tầng 2 finding) — `read_last_n` IS called by `core/status.rs:80` since P005/P006; the attribute is stale.

- **Caller-side `Err` behavior** — confirmed by Worker Turn 1: `src/core/status.rs:80` uses `.unwrap_or_default()`. New mid-file `Err` will be silently absorbed into empty `Vec` at the caller (shows "no heartbeats" in status output, no panic). `core/status.rs` is in KHÔNG sửa list — Worker logs this in Discovery Report as known limitation; future BACKLOG item (caller-side hardening to surface the warning to user-facing stderr). The `read_last_n` return value contract per INV-21 sub-rule 2 is satisfied — caller policy is independent.

- **Add unit tests in `src/heartbeat.rs::tests`** (per Task 5 matrix):
  - `read_last_n_with_corrupt_last_line_skips_it`
  - `read_last_n_with_corrupt_mid_line_fails`
  - `read_last_n_with_blank_lines_ignores_them`

---

### Task 3: Move `tempfile = "3"` from `[dev-dependencies]` to `[dependencies]` in `Cargo.toml`

**File:** `Cargo.toml`

**Tìm:** Worker uses Task 0 anchor `grep "^tempfile" Cargo.toml` result. Current location (per P004 Discovery): `[dev-dependencies] tempfile = "3"`.

**Thay bằng / Thêm:** Two equivalent approaches — Worker picks whichever produces a clean Cargo.toml:

**Option A (preferred — single entry):** Move `tempfile = "3"` to `[dependencies]`. Remove from `[dev-dependencies]`. Cargo treats `[dependencies]` as available to both lib/bin code AND tests/integration, so the dev-dep entry becomes redundant.

```toml
[dependencies]
# ... existing deps ...
tempfile = "3"   # P010: runtime use for atomic heartbeat write (was dev-dep only pre-P010)
```

**Option B (keep both — explicit dev separation):** Keep `[dev-dependencies] tempfile = "3"` AND add `[dependencies] tempfile = "3"`. Cargo accepts both; tests get the dev-deps too. Larger surface area but explicit intent.

**Architect recommends Option A** — single source of truth, less drift risk. If Worker hits a quirk where dev-only test code can't see `tempfile` after the move (extremely unlikely — Cargo includes `[dependencies]` in test compilation), fall back to Option B and note in Discovery.

**Lưu ý:**

- **Version pin unchanged** (`= "3"`). No major bump per Anchor #16 / INV-5. `cargo update --dry-run` should show NO surprise version bump.

- **`Cargo.lock` already pins `tempfile`** per P004 Discovery (transitive presence). Moving the entry section does NOT change the lock file (cargo doesn't add a new dep, just reclassifies an existing one).

- **Binary size impact: zero** — `tempfile` is already linked into the test binaries; moving it to runtime makes it part of the main binary too, but its code is small (~few KB) and most of it is shared with `std::fs` paths. Phase 1 binary size budget = 7 MB; current = 3.9 MB per P009 Discovery; ample headroom.

- **HEADS-UP per CLAUDE.md Hard Stop #2:** This phiếu EXPLICITLY sanctions the `[dependencies]` add (it's already in `[dev-dependencies]` per P004, so it's really a reclassification). Worker does NOT need separate escalation for this — Architect has decided per §Giải pháp.

---

### Task 4: Append INV-21 to `docs/security/INVARIANTS.md`

**File:** `docs/security/INVARIANTS.md`

**Tìm:** After INV-20 block (Worker grep `^### INV-20 ` then find end of that section before next `^---` separator).

**Thay bằng / Thêm:**

```markdown
### INV-21 — Heartbeat append crash-safety: temp+fsync+rename atomic protocol, partial-last-line read tolerance

**Statement:** PR introducing or modifying heartbeat write logic in `src/heartbeat.rs::append` (or any future heartbeat write path) MUST satisfy ALL of:

1. **Atomic write protocol (temp+fsync+rename):** `append` MUST NOT use `OpenOptions::append(true)` + direct `write_all` against the target heartbeat file. Instead: read existing contents → build new buffer (existing + new line) in memory → write buffer to a `tempfile::NamedTempFile` created via `NamedTempFile::new_in(parent_dir_of_target)` (REQUIRED — atomic rename only works on the same filesystem; cross-filesystem rename is a copy+delete that loses atomicity) → call `temp.as_file().sync_all()` (fsync data + metadata, durable across power loss) → call `temp.persist(target_path)` (atomic `std::fs::rename` under the hood; POSIX guarantees same-fs rename appears as a single atomic act to other observers). If ANY step fails before `persist`, the temp file MUST be auto-cleaned (NamedTempFile's `Drop` impl handles this — Worker MUST NOT bypass via `mem::forget` or manual unlink suppression).

2. **Partial-last-line read tolerance:** `read_last_n` (and any future heartbeat-read consumer) MUST tolerate ONE corrupt or truncated line at the END of the file by `tracing::warn!`-ing + skipping the line + continuing to return the prior parsed records. This handles legacy partial-write damage from pre-INV-21 heartbeat files (and defends against the edge case where a Phase 1.4 / pre-P010 binary version wrote the file). A parse failure on ANY line OTHER than the last MUST propagate as `Err` — mid-file corruption is impossible under the atomic-write protocol and indicates external tampering or disk damage that must surface loud (PROJECT.md hard line #5).

3. **Schema preserved across the boundary:** `HeartbeatRecord` struct fields (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail per Phase 1.4 + ARCHITECTURE.md §Heartbeat schema) MUST be unchanged by any crash-safety refactor. The atomicity boundary is a write-mechanism upgrade, NOT a schema upgrade. Adding fields (e.g. `schema_version`, `crash_marker`) requires a separate phiếu.

4. **`append` function signature preserved:** `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>`. Callers (`core::run::run` at exactly one call site per P009 Constraint #12) MUST NOT need source modification when the implementation switches to the atomic protocol. The signature is a public contract since P004.

**Why:** Heartbeat JSONL is the durable observability record advisory-cron exists to produce (PROJECT.md vision). A corrupt or partial JSONL line silently breaks the `status --last N` reader and Sếp's dogfood loop. P009's retry policy triples the crash-surface area (3+ writes per `run` invocation). Without atomic writes, every retry attempt is an independent corruption opportunity. The cost of the atomic protocol (read existing → write to temp → fsync → rename) is microseconds at Sếp's expected usage (~1 fire/day, ~365 KB/year). The cost of NOT having it is undebuggable silent data loss in the exact failure scenarios where the heartbeat is most needed.

**Why `temp+fsync+rename` instead of `fsync-append` (O_APPEND + fsync):** POSIX guarantees O_APPEND writes ≤ `PIPE_BUF` (typically 4 KiB) are atomic, but this depends on platform/filesystem and degrades silently when records exceed the limit (e.g. large `stderr_tail` with high JSON escape expansion). Atomic rename is a hard POSIX guarantee independent of size. Conservative posture for a sprint-closing fault-tolerance phiếu.

**Implementation (Phase 2.3):** `src/heartbeat.rs::append` — `tempfile::NamedTempFile::new_in(parent)` + `write_all` + `as_file().sync_all()` + `persist(target)`. `src/heartbeat.rs::read_last_n` — parse loop with last-line `match ... Err if idx == last_idx => warn+skip`. Both functions keep their P004 signatures unchanged.

**Trust boundary:** Atomic rename is filesystem-level — no new process boundary, no new external service. Same-filesystem constraint enforced by Worker creating the temp file in `log_path.parent()` (not `std::env::temp_dir()` — that is a different fs on many setups and would silently demote the rename to a copy+delete). User config controls `heartbeat.log_path` (INV-15) — atomicity holds regardless of which directory the user picks, as long as that directory and its parent are on the same filesystem (default `~/.local/state/advisory-cron/` satisfies this trivially).

**Trigger keywords:** `OpenOptions::append` + heartbeat file paths (forbidden in new code — must use temp+rename); `std::fs::rename` near heartbeat code (allowed only via `tempfile::NamedTempFile::persist`); new heartbeat writers outside `src/heartbeat.rs`; modifications to `HeartbeatRecord` struct (would require schema-version handling separately).

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE (unit tests for atomic-write protocol + corrupt-last-line tolerance). Giám sát soi PR diff for heartbeat-related changes; if PR reintroduces `OpenOptions::append(true)` against a heartbeat path OR removes the `tempfile::persist` call, flag as INV-21 violation.

---
```

**Lưu ý:**
- INV-21 follows INV-20 voice (P009) — explicit numbered statement, why, implementation, trust boundary, trigger keywords.
- Worker grep `INV-1..20` references in other docs (e.g. ARCHITECTURE.md) — if any reference "20 invariants" or "max INV", update to 21.

---

### Task 5: Add unit tests in `src/heartbeat.rs::tests`

**File:** `src/heartbeat.rs` (extend existing `#[cfg(test)] mod tests` block — Worker grep `#\[cfg\(test\)\]` to find insertion point).

**Tìm:** Existing `#[cfg(test)] mod tests { ... }` block at end of `src/heartbeat.rs`. **Also locate existing test `read_last_n_skips_malformed_line`** (per Worker Turn 1 finding at line 181) — this test must be **updated** to assert mid-file → `Err` rather than skip-all.

**Thay bằng / Thêm:** Append the following test cases AND update the existing `read_last_n_skips_malformed_line` per V2 semantics (mid-file corrupt line MUST now fail loud). Worker preserves all OTHER existing tests (P004 heartbeat tests for serde roundtrip etc.) — these are ADDITIONS + ONE UPDATE.

```rust
#[test]
fn append_creates_file_when_missing() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");
    assert!(!log_path.exists());

    let rec = sample_record(0, "test-create");
    append(&log_path, &rec).expect("append should create the file");
    assert!(log_path.exists(), "heartbeat file must exist after first append");
    let contents = std::fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "exactly 1 line after 1 append");
    let parsed: HeartbeatRecord = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed.label, "test-create");
}

#[test]
fn append_preserves_existing_content() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");

    let rec1 = sample_record(0, "test-first");
    let rec2 = sample_record(1, "test-second");
    append(&log_path, &rec1).unwrap();
    append(&log_path, &rec2).unwrap();

    let contents = std::fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "atomic append must preserve prior line");
    let p1: HeartbeatRecord = serde_json::from_str(lines[0]).unwrap();
    let p2: HeartbeatRecord = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(p1.label, "test-first");
    assert_eq!(p2.label, "test-second");
}

#[test]
fn append_multiple_times_grows_file_monotonically() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");

    let mut last_size: u64 = 0;
    for i in 0..5 {
        let rec = sample_record(i, &format!("test-{i}"));
        append(&log_path, &rec).unwrap();
        let size = std::fs::metadata(&log_path).unwrap().len();
        assert!(size > last_size, "file size must grow monotonically (iter {i}: {last_size} → {size})");
        last_size = size;
    }
    let lines: Vec<String> = std::fs::read_to_string(&log_path).unwrap().lines().map(|s| s.to_string()).collect();
    assert_eq!(lines.len(), 5, "5 appends = 5 lines");
}

#[test]
fn append_leaves_no_temp_file_in_parent_dir() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");

    append(&log_path, &sample_record(0, "noleak-1")).unwrap();
    append(&log_path, &sample_record(1, "noleak-2")).unwrap();
    append(&log_path, &sample_record(2, "noleak-3")).unwrap();

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    // Only the heartbeat.jsonl itself should remain — no `.tmp*` leftovers.
    assert_eq!(entries.len(), 1, "only heartbeat.jsonl should remain, found: {entries:?}");
    assert_eq!(entries[0], "heartbeat.jsonl");
}

#[test]
fn read_last_n_with_corrupt_last_line_skips_it_and_returns_prior() {
    use std::io::Write;
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");

    // Append 2 good records via the atomic protocol.
    append(&log_path, &sample_record(0, "good-1")).unwrap();
    append(&log_path, &sample_record(1, "good-2")).unwrap();

    // Manually append a corrupt trailing line (simulating pre-P010 partial write).
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&log_path).unwrap();
        f.write_all(b"{\"ts\":\"2026-05-27T00:00:00").unwrap(); // truncated JSON, no trailing newline
    }

    let records = read_last_n(&log_path, 10).expect("corrupt LAST line must be tolerated, not propagated");
    assert_eq!(records.len(), 2, "2 prior good records returned, corrupt last line skipped");
    assert_eq!(records[0].label, "good-1");
    assert_eq!(records[1].label, "good-2");
}

#[test]
fn read_last_n_with_corrupt_mid_line_fails_loud() {
    use std::io::Write;
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");

    // Manually craft a file with: good-line, corrupt-line, good-line.
    {
        let good1 = serde_json::to_string(&sample_record(0, "g1")).unwrap();
        let good2 = serde_json::to_string(&sample_record(1, "g2")).unwrap();
        let mut f = std::fs::File::create(&log_path).unwrap();
        writeln!(f, "{good1}").unwrap();
        writeln!(f, "{{this is not json}}").unwrap();
        writeln!(f, "{good2}").unwrap();
    }

    let result = read_last_n(&log_path, 10);
    assert!(result.is_err(), "mid-file corruption MUST fail loud per INV-21 sub-rule 2");
}

#[test]
fn read_last_n_returns_empty_on_missing_file() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("does-not-exist.jsonl");
    let records = read_last_n(&log_path, 5).expect("missing file should return Ok(empty)");
    assert!(records.is_empty());
}

#[test]
fn read_last_n_skips_blank_lines_silently() {
    use std::io::Write;
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("heartbeat.jsonl");
    {
        let good = serde_json::to_string(&sample_record(0, "g-blank")).unwrap();
        let mut f = std::fs::File::create(&log_path).unwrap();
        writeln!(f).unwrap(); // blank
        writeln!(f, "{good}").unwrap();
        writeln!(f).unwrap(); // blank in middle (tolerated, not last)
        writeln!(f, "{good}").unwrap();
    }
    let records = read_last_n(&log_path, 10).expect("blank lines must be tolerated silently");
    assert_eq!(records.len(), 2, "2 good records, blanks ignored");
}

// Helper — Worker may already have a similar helper from P004 tests.
// If so, reuse it; otherwise add this.
fn sample_record(i: u32, label: &str) -> HeartbeatRecord {
    HeartbeatRecord {
        ts: chrono::Utc::now(),
        label: label.to_string(),
        exit_code: i as i32,
        duration_ms: 100 + i as u64,
        stdout_tail: format!("stdout for {i}"),
        stderr_tail: String::new(),
    }
}
```

**Update existing test `read_last_n_skips_malformed_line`** (per Worker Turn 1 verification, currently at `src/heartbeat.rs:181`): The current test passes a mid-file corrupt line and asserts skip-all. Under V2 semantics, mid-file → `Err`. Worker rewrites the test to assert `result.is_err()` for the mid-file scenario (this test's contract semantically MERGES with `read_last_n_with_corrupt_mid_line_fails_loud` above — Worker may either delete `read_last_n_skips_malformed_line` entirely in favor of the new test, OR keep it with the assertion flipped — Tầng 2 stylistic call. Either way, the test name should reflect the new behavior; recommend deleting the old name and keeping the new descriptive `read_last_n_with_corrupt_mid_line_fails_loud`).

**Lưu ý:**

- **`sample_record` helper** — Worker checks if P004 left a similar helper in the existing `tests` block (Worker Turn 1 confirmed it exists with zero-args signature). Worker either: (a) renames the existing helper to `sample_record_default` and adds the new 2-param `sample_record(i, label)` here, OR (b) adapts the new tests to use the existing zero-arg helper plus manual field mutation — Tầng 2 stylistic call.

- **`HeartbeatRecord` import inside `mod tests`** — already present per P004 tests. Confirm with `grep "use super" src/heartbeat.rs`.

- **`#[cfg(test)] use chrono::Utc;`** — likely already present per P004 tests for `sample_record`. If not, add.

- **Concurrent-write smoke test (deferred to Worker Tầng 2 call):** A true atomicity stress test would spawn multiple tokio tasks/threads each calling `append` concurrently and assert the final file has exactly N lines (no corruption, no lost writes). Architect deems this OPTIONAL because: (a) the launchd plist for `advisory-cron` fires the binary in single-shot mode (one process per fire, no in-process concurrency among heartbeat writers); (b) atomicity guarantee comes from POSIX rename, not from advisory-cron code — POSIX is responsible for ordering. Worker MAY add the concurrent test if it's <30 LOC and Worker has confidence; otherwise punt to a future phiếu if dogfood reveals a need.

- **`tests/cli_run_crash_safe.rs` (Tầng 2 Worker call)** — Architect's §Scope leaves the integration-vs-unit split to Worker. If unit tests above cover the matrix completely (and they do for the documented acceptance criteria), no integration test file is needed. If Worker wants a single end-to-end "spawn binary, kill mid-write, re-spawn, verify recovery" test, add `tests/cli_run_crash_safe.rs` — but Architect estimates this is hard to make deterministic (timing-sensitive) and may add flakiness. Architect recommends UNIT TESTS ONLY — no new integration file. Worker logs choice to Discovery.

---

### Task 6: Update `docs/ARCHITECTURE.md` — Heartbeat atomicity subsection + Phase status

**File:** `docs/ARCHITECTURE.md`

**Tìm change 1:** §Modules table row for `src/heartbeat.rs` (Worker grep `src/heartbeat.rs` in ARCHITECTURE.md §Modules).

**Thay bằng:** update Purpose column to mention atomic-write protocol:

```markdown
| `src/heartbeat.rs` | JSONL atomic append (temp+fsync+rename per INV-21) + read-last-N with partial-last-line tolerance. `HeartbeatRecord` struct (durable schema, unchanged since P004). `tail_utf8` helper. | 1.4 ✅ + 2.3 crash-safe ✅ |
```

**Tìm change 2:** §Heartbeat schema section (Worker grep `## Heartbeat schema`).

**Thay bằng / Thêm:** insert a new "Atomicity" subsection AFTER the existing "Retry semantics (Phase 2.2)" paragraph:

```markdown
### Atomicity (Phase 2.3 — P010)

`heartbeat::append` uses a **temp+fsync+rename** protocol to guarantee that each call is crash-safe: the heartbeat file at any moment observable to another process is either the file as it was BEFORE the call, or the file as it would be after the call's full success. There is no observable partial state.

Protocol (per call):
1. Read existing heartbeat file contents into memory (empty if file missing).
2. Append the new JSONL line (with trailing `\n`) to the in-memory buffer.
3. Create a `NamedTempFile` in the **same directory** as the target heartbeat file (required — atomic rename only works on the same filesystem).
4. Write the full buffer to the temp file.
5. `fsync` the temp file via `sync_all()` (data + metadata — file size durable across power loss).
6. Atomically rename the temp file over the target via `NamedTempFile::persist(target)` (POSIX `rename(2)` — atomic on same-fs).

If any step before the rename fails, the temp file is auto-cleaned via `Drop`; the target file is untouched. The caller (`core::run::run`) log-warn-continues on `Err` per P004 contract — task is NOT failed on heartbeat write failure.

`heartbeat::read_last_n` tolerates ONE corrupt or truncated trailing line (likely from a pre-P010 interrupted write): `tracing::warn!` + skip. Corruption at any line OTHER than the last propagates as `Err` — mid-file corruption is impossible under the atomic-write protocol and must surface loud per PROJECT.md hard line #5.

Trade-off: atomic-rename rewrites the entire heartbeat file on every append. At Sếp's expected usage (1 fire/day, ~1 KB/day, ~365 KB/year), the per-append cost is microseconds. INV-21 documents the boundary in full.

**Why not `fsync-append` (O_APPEND + fsync)?** POSIX guarantees only writes ≤ `PIPE_BUF` (typically 4 KiB) are atomic with O_APPEND. Heartbeat records are usually well under this, but large `stderr_tail` content with high JSON-escape expansion could exceed it silently. Atomic rename is a hard POSIX guarantee independent of size — conservative choice for a fault-tolerance phiếu.
```

**Tìm change 3:** §Phase status section (Worker grep `## Phase status` → `Phase 2`).

**Thay bằng / Thêm:** update Phase 2 line to mark 2.3 SHIPPED + entire Phase 2 COMPLETE:

```markdown
- ✅ **Phase 2** — COMPLETE. Phase 2.1 (Telegram alert) shipped per P008. Phase 2.2 (retry policy) shipped per P009 (`is_retryable` private fn + retry loop in `core/run.rs`; 1 heartbeat per attempt schema preserved; alert moved OUTSIDE loop per INV-20 single-alert-per-invocation; `[retry]` opt-in config block). Phase 2.3 (state recovery) shipped per P010 (heartbeat `append` refactored to temp+fsync+rename atomic protocol; `read_last_n` tolerates corrupt last line; INV-21 added; no schema change). **All 10 phiếu of the sprint shipped — sprint closes 2026-MM-DD.**
- ⏸️ **Phase 3** — Deferred. Trigger: Phase 2 ship complete (now), need Linux support, OR Sếp picks from "Open backlog" debt items.
```

**Lưu ý:**
- Worker fills `2026-MM-DD` with actual ship date.
- All other ARCHITECTURE.md sections (CLI surface, Config schema, MCP surface, Cron mechanism, Error handling) are UNCHANGED — P010 is a heartbeat-internal hardening, not a surface change.

---

### Task 7: Update `README.md` — brief Phase 2.3 note

**File:** `README.md`

**Tìm:** existing Phase 2.2 retry section (Worker grep `Phase 2.2` in README.md).

**Thay bằng / Thêm:** append a Phase 2.3 paragraph after the Phase 2.2 section:

```markdown
### Phase 2.3 — Crash-safe heartbeat (state recovery)

Every heartbeat write is atomic — `advisory-cron` cannot leave a corrupt or truncated JSONL line in the heartbeat file, even if killed mid-write (OOM, `launchctl kill`, power loss). Each `append` uses the POSIX `temp+fsync+rename` protocol: write to a temp file in the same directory, `fsync`, then atomic rename over the target. If interrupted before the rename, the temp file is auto-cleaned and the target is untouched.

The read path (`advisory-cron status`) tolerates ONE legacy partial line at the end of the file (from a pre-Phase-2.3 binary that may have crashed mid-write) — it logs a warning and skips that line, returning the prior records. Corruption anywhere except the very last line fails loud (per PROJECT.md hard line #5).

No config change required — Phase 2.3 is fully transparent. Existing heartbeat files (and any pre-2.3 partial-write damage at their tail) read cleanly.
```

**Lưu ý:** Worker keep this paragraph short — README is user-facing, not architectural deep-dive. The technical detail lives in ARCHITECTURE.md + INV-21.

---

### Task 8: Update `docs/CHANGELOG.md` — P010 entry + sprint-close summary

**File:** `docs/CHANGELOG.md`

**Tìm:** top of file, immediately after the `---` separator following the header (insert as newest entry above P009).

**Thay bằng / Thêm:**

```markdown
## 2026-MM-DD — P010: Phase 2.3 — Crash-safe heartbeat (SPRINT COMPLETE)

**Phiếu:** P010 (Tầng 1 — `heartbeat::append` atomic temp+fsync+rename protocol, `read_last_n` partial-last-line tolerance, INV-21 appended, `HeartbeatRecord` schema preserved, function signatures preserved)

**Heartbeat write (src/heartbeat.rs::append):**
- Replaced Phase 1.4 direct `OpenOptions::append(true)` + write with atomic-rename protocol.
- Steps: read existing file → append new line in-memory buffer → write to `tempfile::NamedTempFile::new_in(parent_dir)` → `sync_all()` (fsync) → `persist(target)` (atomic `std::fs::rename`).
- Function signature `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>` UNCHANGED — single call site in `core::run::run` (P009 Constraint #12) preserved.
- If any step before rename fails, NamedTempFile's Drop auto-cleans the temp file; target file untouched. Caller continues to log-warn-continue on `Err` per P004 contract.

**Heartbeat read (src/heartbeat.rs::read_last_n):**
- **Tightened** prior P004 silent-skip-all-malformed behavior. Now: last-line parse failure → `tracing::warn!` + skip + return prior records; mid-file parse failure → propagate as `Err` (was silently swallowed pre-P010; unexpected under atomic protocol; must surface loud per PROJECT.md hard line #5).
- `eprintln!` replaced with `tracing::warn!` (INV-13 compliance).
- Blank lines tolerated silently anywhere (preserved).
- Existing test `read_last_n_skips_malformed_line` updated/replaced — mid-file corrupt line scenario flipped from skip-assertion to `is_err`-assertion.
- Function signature `pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>` UNCHANGED.
- Stale `#[allow(dead_code)]` attribute removed.
- **Caller-side note:** `src/core/status.rs:80` calls `read_last_n(...).unwrap_or_default()`, silently absorbing the new mid-file `Err` into empty Vec at the status output. Pre-existing P005/P006-era design choice; NOT in P010 scope; future BACKLOG candidate for caller-side hardening. See Discovery.

**Schema preserved:**
- `HeartbeatRecord` fields (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) UNCHANGED since P004.
- No `schema_version` bump, no new fields, no migration required for existing heartbeat files (the atomic protocol is additive in mechanism, preserves the JSONL format exactly).

**Cargo.toml:**
- `tempfile = "3"` moved from `[dev-dependencies]` to `[dependencies]` (was already in lock file per P004 — zero compile time / binary size delta).
- No other dep changes. `cargo update --dry-run` clean.

**INVARIANTS.md:**
- INV-21 appended (4 sub-rules: atomic temp+fsync+rename protocol, partial-last-line read tolerance, schema preservation, signature preservation).

**Tests (+N new, total ~133+N):**
- `src/heartbeat.rs::tests` unit (8): append_creates_file_when_missing, append_preserves_existing_content, append_multiple_times_grows_file_monotonically, append_leaves_no_temp_file_in_parent_dir, read_last_n_with_corrupt_last_line_skips_it_and_returns_prior, read_last_n_with_corrupt_mid_line_fails_loud, read_last_n_returns_empty_on_missing_file, read_last_n_skips_blank_lines_silently.
- 1 existing test updated/replaced (`read_last_n_skips_malformed_line` — semantic flip to mid-file → Err).
- All P009 + Phase 1 baseline tests preserved (133 → ~141 net after add + 1 flip).

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules row `src/heartbeat.rs` updated (atomic + tolerance); new §Heartbeat schema "Atomicity (Phase 2.3 — P010)" subsection; §Phase status Phase 2 marked COMPLETE.
- `docs/security/INVARIANTS.md` — INV-21 appended.
- `README.md` — Phase 2.3 paragraph appended after Phase 2.2 retry section.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, binary ≤7MB
- `cargo test --all` — ~141/141 pass (133 P009 baseline + ~8 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #1)
- `git diff src/core/run.rs` — empty (Constraint #2)
- `git diff src/core/status.rs` — empty (V2 KHÔNG sửa list — caller-side `.unwrap_or_default()` preserved)
- `git diff src/alert.rs` — empty (Constraint #6 alert.rs env-free preserved)
- `git diff src/runner.rs` — empty (Constraint #5)
- `git diff src/config.rs` — empty (no schema change)
- `grep -c "heartbeat::append" src/core/run.rs` — STILL exactly `1` (P009 Constraint #12 preserved)
- `grep -c "send_with_base" src/core/run.rs` — STILL exactly `1` (P009 Constraint #8 preserved)
- `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` — STILL empty (P008 Constraint #11 preserved)

---

### Sprint summary — Phase 1 + Phase 2 COMPLETE (P001-P010, 2026-05-27)

**10 phiếu shipped over the sprint:**

| Phiếu | Phase | Theme |
|-------|-------|-------|
| P001 | 1.1 | CLI scaffold (5 subcommand stubs, clap derive) |
| P002 | 1.2 | Config schema (TOML + serde) |
| P003 | 1.3 | launchd plist + register/unregister |
| P004 | 1.4 | Task runner + heartbeat JSONL |
| P005 | 1.5 | Status reporter |
| P006 | 1.7 | MCP server wrapper + `core::*` extraction (dual-surface parity) |
| P007 | 1.6 | README + ARCHITECTURE post-ship polish |
| P008 | 2.1 | Telegram alert on task failure |
| P009 | 2.2 | Retry policy (`is_retryable` + retry loop, single-alert-per-invocation) |
| P010 | 2.3 | Crash-safe heartbeat (temp+fsync+rename atomic protocol) |

**Cumulative state:**
- Binary size: ~3.9 MB release (well under 7 MB budget).
- Test count: ~141 (8 added by P010).
- INVARIANTS: 21 (INV-1..21).
- DISCOVERIES: 10 per-phiếu reports.
- Both CLI surface (5 subcommands) and MCP surface (5 tools via stdio) ship with full parity per layering invariant.

Sprint closes per BACKLOG.md acceptance pending: Sếp dogfood 3 ngày liên tiếp confirmation of `/advisory-scan` daily fire + at least 1 Claude Desktop MCP tool invocation.

---
```

**Lưu ý:**
- Worker fills `2026-MM-DD` with actual ship date.
- Worker fills exact test count after running `cargo test --all`.
- Worker DOES NOT mark BACKLOG.md "Recently shipped" / Active sprint update — that is Sếp's call (per BACKLOG.md §Quy tắc maintenance #2). Worker MAY suggest the diff in Discovery Report; Sếp applies.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/heartbeat.rs` | Task 1: refactor `append` body to temp+fsync+rename; Task 2: refactor `read_last_n` body to skip-corrupt-last-line + remove stale `#[allow(dead_code)]`; Task 5: append 8 unit tests + update existing `read_last_n_skips_malformed_line` to assert mid-file → Err |
| `Cargo.toml` | Task 3: move `tempfile = "3"` from `[dev-dependencies]` to `[dependencies]` (or duplicate per Option B) |
| `docs/security/INVARIANTS.md` | Task 4: append INV-21 |
| `docs/ARCHITECTURE.md` | Task 6: §Modules row + new §Heartbeat schema "Atomicity" subsection + §Phase status Phase 2 COMPLETE |
| `README.md` | Task 7: Phase 2.3 paragraph after Phase 2.2 section |
| `docs/CHANGELOG.md` | Task 8: P010 entry + sprint summary |
| `docs/discoveries/P010.md` | Discovery Report (created at end per CLAUDE.md DOD) — MUST include: caller-side `.unwrap_or_default()` known limitation note |
| `docs/DISCOVERIES.md` | 1-line index entry (newest at top, prepended above P009 line) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/cli/mod.rs` | `git diff` empty (Constraint #1 — newtype dispatch unchanged since P006 V2) |
| `src/core/run.rs` | `git diff` empty (P009 Constraint #12 preserved — `grep -c "heartbeat::append"` STILL = 1) |
| `src/core/status.rs` | `git diff` empty (V2 — explicit KHÔNG sửa per O1.1 corollary; caller-side `.unwrap_or_default()` at line 80 silently absorbs new mid-file `Err`; pre-existing P005/P006-era design choice; Worker logs as known limitation in Discovery; future BACKLOG candidate) |
| `src/core/init.rs`, `register.rs`, `unregister.rs` | `git diff` empty (only `core/run.rs` calls `heartbeat::append`) |
| `src/cli/run.rs`, `cli/status.rs`, `cli/init.rs`, `cli/register.rs`, `cli/unregister.rs`, `cli/mcp.rs` | `git diff` empty (CLI handlers unchanged) |
| `src/mcp/tools.rs`, `mcp/server.rs`, `mcp/mod.rs` | `git diff` empty (MCP tools inherit heartbeat changes via shared `core::*`) |
| `src/alert.rs` | `git diff` empty (Constraint #11 alert.rs env-free invariant preserved from P008) |
| `src/runner.rs` | `git diff` empty (P009 Constraint #5 — runner stays single-fire primitive) |
| `src/config.rs` | `git diff` empty (no schema change; `HeartbeatConfig.log_path` unchanged) |
| `src/launchd.rs`, `src/main.rs` | `git diff` empty (no module add, no dispatch change) |
| `HeartbeatRecord` struct fields | grep struct definition before/after, confirm fields unchanged (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) — Constraint #4 |

---

## Luật chơi (Constraints)

1. **CONSTRAINT #1 re-instated (post-P006 V2, honored P008+P009):** `git diff src/cli/mod.rs` MUST be empty. Heartbeat refactor is internal to `src/heartbeat.rs` — no new CLI subcommand, no dispatch edit. Worker confirms with `git diff src/cli/mod.rs` returning empty.

2. **`src/core/run.rs` UNTOUCHED:** `git diff src/core/run.rs` MUST be empty. P010 refactors heartbeat-INTERNAL implementation; signatures preserved per Constraints #4 + #5 below. The retry loop call site (`heartbeat::append(...)`) needs zero source change. If Worker is tempted to touch `core/run.rs` (e.g. to add a wrapper around `heartbeat::append`), STOP — that breaks Constraint #2 and indicates the signature changed (violation of Constraint #4).

3. **`Cargo.toml` change SANCTIONED (and bounded):** ONLY change permitted is moving (or duplicating) `tempfile = "3"` between `[dev-dependencies]` and `[dependencies]`. NO new crates added, NO version bumps, NO feature flag changes. `cargo update --dry-run` must show no surprise version bumps.

4. **`heartbeat::append` signature PRESERVED:** `pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>`. Worker confirms with `grep "pub fn append" src/heartbeat.rs` returning the exact same signature line as pre-P010. If implementation requires extra params (it does not — Architect's snippet uses only `log_path` + `record`), STOP and escalate.

5. **`heartbeat::read_last_n` signature PRESERVED:** `pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>`. Worker confirms with `grep "pub fn read_last_n" src/heartbeat.rs` returning the exact same signature line.

6. **`HeartbeatRecord` schema PRESERVED:** struct fields (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) UNCHANGED. No new fields, no removals, no renames, no type changes. `git diff src/heartbeat.rs` will be non-empty (function bodies + tests added) — Worker manually inspects the struct definition lines to confirm UNCHANGED. (Architect cannot script this exactly without Read access; Worker uses `git diff src/heartbeat.rs | grep -A20 "pub struct HeartbeatRecord"` to inspect.)

7. **No new module:** P010 does NOT create `src/heartbeat/` directory or `src/atomic_write.rs` or any other new file in `src/`. The atomic protocol lives inline in `src/heartbeat.rs::append`. Hard Stops #1 honored.

8. **`src/alert.rs` env-free invariant (CONSTRAINT #11 from P008/P009) PRESERVED:** `git diff src/alert.rs` MUST be empty. `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` MUST STILL return empty.

9. **`src/runner.rs` UNTOUCHED (P009 Constraint #5):** `git diff src/runner.rs` MUST be empty. (Exception: if a test helper in `runner.rs` needs to add a `Config` field initializer due to some unrelated struct evolution, Worker logs in Discovery as "structural necessity" — but no such change is expected in P010 since P010 does NOT modify `Config`.)

10. **No new env var reads:** P010 adds zero env reads. Heartbeat path comes from `config.heartbeat.log_path` (existing INV-15). No `std::env::var` calls added anywhere.

11. **No `unsafe { }`:** Per CLAUDE.md Hard Stop #7 + INV-6. The temp+rename protocol is pure safe Rust (std::fs, tempfile, std::io::Write).

12. **P009 Constraint #12 preservation (single heartbeat call site):** `grep -c "heartbeat::append" src/core/run.rs` MUST STILL be exactly `1` after P010 ships. P010 does NOT add new call sites — the entire change is internal to `heartbeat.rs`. Verifies that Constraint #2 (no `core/run.rs` edit) holds.

13. **P009 Constraint #8 preservation (single alert call site):** `grep -c "send_with_base" src/core/run.rs` MUST STILL be exactly `1`. P010 does not touch alert wiring.

14. **Same-filesystem temp file:** Worker MUST create the temp file in `log_path.parent()` directory (NOT `std::env::temp_dir()` or any other path). Cross-fs rename silently degrades to copy+delete and loses atomicity — this is THE critical correctness constraint of the entire phiếu. Architect's snippet shows `NamedTempFile::new_in(parent_dir)` — Worker MUST NOT change to `NamedTempFile::new()` (which uses `std::env::temp_dir()`).

15. **Mid-file corruption fails loud (INV-21 sub-rule 2):** Worker MUST propagate `Err` on parse failure for any line that is NOT the last line. If Worker is tempted to "be helpful" and silently skip all corrupt lines (preserving the prior P004 behavior), STOP — that violates PROJECT.md hard line #5 and INV-21 sub-rule 2. The existing `read_last_n_skips_malformed_line` test which asserts the OLD skip-all behavior MUST be updated to assert `is_err()` for the mid-file scenario.

16. **Sprint-closing posture — no scope creep:** P010 is the LAST phiếu of the sprint. Worker is tempted to "polish" adjacent areas (e.g. add `read_last_n` performance optimization, reorganize `heartbeat.rs` imports, refactor `tail_utf8`, fix `core/status.rs:80` `.unwrap_or_default()`). STOP. Per Hard Stops #6 — no refactor outside scope. The `core/status.rs:80` caller-side silent-swallow is a known limitation logged in Discovery, NOT a P010 fix. Any other cleanup item discovered goes to Discovery Report → BACKLOG.md "Open backlog" → future phiếu.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings, binary ≤7MB
- [ ] `cargo test --all` — all pass (~141 total: 133 P009 baseline + ~8 new P010 unit tests, with 1 existing test semantically flipped — net same baseline preserved + 8 add)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `git diff src/cli/mod.rs` — empty (Constraint #1)
- [ ] `git diff src/core/run.rs` — empty (Constraint #2)
- [ ] `git diff src/core/status.rs` — empty (V2 KHÔNG sửa — caller-side `.unwrap_or_default()` preserved per O1.1 corollary)
- [ ] `git diff src/alert.rs` — empty (Constraint #8 P008-env-free preserved)
- [ ] `git diff src/runner.rs` — empty (Constraint #9 P009-runner-untouched preserved)
- [ ] `git diff src/config.rs` — empty (Constraint #6 — no schema change)
- [ ] `git diff Cargo.toml` — non-empty BUT ONLY `tempfile` reclassification (Constraint #3)
- [ ] `grep "pub fn append(log_path: &Path, record: &HeartbeatRecord) -> Result<()>" src/heartbeat.rs` — single hit, signature unchanged (Constraint #4)
- [ ] `grep "pub fn read_last_n(log_path: &Path, n: usize) -> Result<Vec<HeartbeatRecord>>" src/heartbeat.rs` — single hit, signature unchanged (Constraint #5)
- [ ] `grep -A10 "pub struct HeartbeatRecord" src/heartbeat.rs` — fields ts/label/exit_code/duration_ms/stdout_tail/stderr_tail unchanged (Constraint #6)
- [ ] `grep -c "heartbeat::append" src/core/run.rs` — STILL exactly `1` (Constraint #12 — P009 invariant preserved)
- [ ] `grep -c "send_with_base" src/core/run.rs` — STILL exactly `1` (Constraint #13 — P009 invariant preserved)
- [ ] `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` — empty (Constraint #8 P008 invariant preserved)
- [ ] `grep "NamedTempFile::new_in" src/heartbeat.rs` — at least 1 hit (Constraint #14 — same-filesystem temp file)
- [ ] `grep "NamedTempFile::new(" src/heartbeat.rs` — ZERO hits (Constraint #14 — no `std::env::temp_dir` use)
- [ ] `grep "eprintln!" src/heartbeat.rs` — ZERO hits (V2 — `eprintln!` replaced with `tracing::warn!` per INV-13)
- [ ] `grep -c "^### INV-" docs/security/INVARIANTS.md` — exactly `21` (INV-21 appended)

### Manual Testing
- [ ] `cd /tmp/p010 && mkdir -p state && cat > config.toml <<EOF
[task]
command = "true"
args = []
working_dir = "/tmp/p010"
label = "p010-manual"
[schedule]
hour = 9
minute = 0
[heartbeat]
log_path = "/tmp/p010/state/heartbeat.jsonl"
EOF` — write a minimal config
- [ ] `cargo run --release -- run --config /tmp/p010/config.toml` — observe exit 0; `cat /tmp/p010/state/heartbeat.jsonl` shows 1 valid JSON line
- [ ] Repeat 3 times → 3 lines accumulated (atomic preservation)
- [ ] `ls /tmp/p010/state/` — only `heartbeat.jsonl`, NO `.tmp*` leftovers
- [ ] Manually corrupt last line: `echo '{"ts":"truncated' >> /tmp/p010/state/heartbeat.jsonl`
- [ ] `cargo run --release -- status --config /tmp/p010/config.toml --last 5` — observe warning in stderr about partial line, BUT 3 prior records displayed cleanly (no panic, no error exit)
- [ ] Cleanup: `rm -rf /tmp/p010`

### Regression
- [ ] `cargo test --test cli_run` (P004 integration) — all pass
- [ ] `cargo test --test cli_run_alert` (P008 integration) — all pass
- [ ] `cargo test --test cli_run_retry` (P009 integration) — all pass (now exercises atomic-write 3× per test)
- [ ] `cargo test --test cli_status` (P005 integration if exists) — all pass
- [ ] `cargo test --test cli_mcp` (P006 MCP tests) — all pass (MCP `status` tool gets corrupt-line tolerance for free)
- [ ] `advisory-cron mcp` smoke test (`echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | advisory-cron mcp`) — still returns serverInfo

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P010 entry at top + sprint summary appended
- [ ] `docs/ARCHITECTURE.md` — 3 changes (Modules row, new Heartbeat Atomicity subsection, Phase status Phase 2 COMPLETE)
- [ ] `docs/security/INVARIANTS.md` — INV-21 appended; total `### INV-` count = 21
- [ ] `README.md` — Phase 2.3 paragraph appended after Phase 2.2 section
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P010.md` — full report written per CLAUDE.md DOD template; include: actual final test count, whether Worker chose Option A or B for Cargo.toml `tempfile` reclassification, whether Worker added the optional concurrent-write smoke test, whether Worker added `tests/cli_run_crash_safe.rs` integration file or kept all tests as unit tests in `heartbeat.rs::tests`, any anchor sai lệch found at Task 0, confirmation of all 16 Constraints, **MANDATORY: known-limitation note re `src/core/status.rs:80` `.unwrap_or_default()` silently absorbing new mid-file `Err` — recommend BACKLOG "Open backlog" item for caller-side hardening (V2 corollary per Worker Turn 1 finding)**
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top, prepended above P009 line). Include note "SPRINT COMPLETE — Phase 1+2 all shipped"
- [ ] Sub-mechanism A-E Verification Trace table filled
- [ ] Discovery mentions: performance measurement (if Worker captured per-append rewrite cost as proposed in §Lưu ý Task 1); any new BACKLOG "Open backlog" debt items spotted during P010 (per Constraint #16 — log don't fix)
