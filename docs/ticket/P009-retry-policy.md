# PHIẾU P009: Retry policy (Phase 2.2)

> **Loại:** Feature
> **Tầng:** 1
> **Ưu tiên:** P1
> **Ảnh hưởng:** `src/config.rs` (extend — `RetryConfig`), `src/core/run.rs` (retry loop wrapping `runner::fire_task`), `docs/security/INVARIANTS.md` (append INV-20), `docs/ARCHITECTURE.md` (Modules + Phase status + new §Retry section), `docs/CHANGELOG.md`, `README.md`, `tests/cli_run_retry.rs` (NEW integration)
> **Dependency:** P008 (Phase 2.1 alert — retry must integrate with alert path so only FINAL failure alerts, no per-attempt spam).

---

## Context

### Vấn đề hiện tại

BACKLOG.md "Phase 2 — Next sprint" item 2.2: Retry policy. Config `[retry]` block (max_attempts, backoff_secs). Re-fire on transient failure (exit code 1-127 retryable; SIGTERM/SIGKILL not).

Phase 2.1 (P008) đã ship: alert on `exit_code != 0`. Nhưng Sếp's task có thể fail vì network blip, transient API rate-limit, etc. — re-fire trong vài giây thường succeed. Hiện tại single-fire → 1 fail = 1 alert ngay. Phase 2.2 thêm retry loop wrapping `runner::fire_task`, alert chỉ khi all attempts exhausted.

PROJECT.md hard line #5 "Failure mode = noisy" yêu cầu surface failure — but only surface REAL failure (after retries), not transient blips.

### Giải pháp

1. **`src/config.rs` extend** — `RetryConfig { max_attempts: u32, backoff_secs: u64 }` + `pub retry: Option<RetryConfig>` trên `Config` (Option vì opt-in; old configs không có `[retry]` deserialize as `None`). Validation: `max_attempts ≥ 1` (1 = no retry, just 1 attempt — sensible boundary), `backoff_secs ≤ 3600` (sanity cap, prevent typo 86400 freezing launchd job for a day). `default_for_home` does NOT include `[retry]` block — opt-in.

2. **`src/core/run.rs` extend — retry loop wraps `runner::fire_task`** — Architect quyết: retry logic lives in `core/run.rs` (orchestration concern), NOT in `src/runner.rs` (single-fire primitive). `runner::fire_task` stays untouched — it remains the atomic "spawn once, capture result" function. `core/run.rs` calls it up to N times in a loop. Loop shape:
   ```
   for attempt in 1..=max_attempts:
       fire_result = runner::fire_task(config).await
       build HeartbeatRecord (with current attempt's exit_code etc.)
       heartbeat::append(record)   # 1 record per attempt — schema unchanged
       if exit_code == 0: break        # success
       if not is_retryable(exit_code): break  # SIGTERM/SIGKILL/spawn-fail — don't retry
       if attempt == max_attempts: break       # exhausted
       sleep(backoff_secs)
   # AFTER loop: send alert ONLY if final exit_code != 0 (one alert max per `run` invocation)
   ```

3. **Heartbeat schema unchanged — 1 record per FIRE attempt** — Architect ACCEPT Sếp's recommendation. If retry fires 3 times, 3 JSONL lines appended. Each line has its own `ts`, `exit_code`, `duration_ms`. Status reader (`advisory-cron status`) shows last N heartbeats — user naturally sees "task ran 3 times, last exit code = N". KHÔNG add `retry_attempt` field (would be Tầng 1 schema change, breaks INV-15 implicit contract).

4. **Retryable exit code rule** — per BACKLOG spec "exit code 1-127 retryable; SIGTERM/SIGKILL not":
   - `exit_code ∈ 1..=127` → retryable (normal process exit with error)
   - `exit_code ≥ 128` → NOT retryable (signal-killed: 130=SIGINT, 137=SIGKILL, 143=SIGTERM; convention `128 + signal_num`)
   - `exit_code == 0` → success, no retry needed
   - `exit_code == -1` (spawn failure — runner returns -1 per INV-14 + P004 contract) → NOT retryable. Spawn failure = command path missing / not executable = deploy/config bug. Retry won't help. Surface immediately.

5. **Alert wiring — only FINAL failure alerts (no per-attempt spam)** — Architect re-uses P008's alert call site in `core/run.rs`, BUT moves it OUTSIDE the retry loop. After all attempts done, evaluate FINAL `exit_code`: if != 0 → send 1 alert. If success on attempt 3 → no alert ever. This is the critical invariant — Sếp's chat gets EXACTLY 1 message per failed `advisory-cron run` invocation, regardless of retry count.

6. **Behavior when `[retry]` block absent** — backwards-compat: behave exactly as Phase 2.1 (single attempt, no retry, alert on fail). Achieved by treating `None` as `RetryConfig { max_attempts: 1, backoff_secs: 0 }` internally — loop body runs once, no sleep, no retry decision needed.

7. **No new dep.** `tokio::time::sleep` + `Duration` already pulled by `tokio` feature `time` (P008 Anchor #6 confirmed). `wiremock` dev-dep from P008 sufficient for integration test (test confirms alert called exactly once after retries).

8. **Docs**: INV-20 (retry-policy boundary — max_attempts cap + backoff respected + signal-exit not retried + 1-alert-per-run invariant), ARCHITECTURE.md new §Retry section + Modules table notes + Phase status 2.2 shipped, CHANGELOG, README brief snippet, Phase 2.2 ship.

### Scope

- **CHỈ sửa:**
  - `src/config.rs` (extend — add `RetryConfig`, `Config::retry` field, validation)
  - `src/core/run.rs` (extend — retry loop wrapping existing `runner::fire_task` + heartbeat append; move alert call OUTSIDE loop)
  - `docs/security/INVARIANTS.md` (append INV-20)
  - `docs/ARCHITECTURE.md` (§Modules row note for `core/run.rs` retry; new §Retry policy section; §Phase status update Phase 2.2)
  - `docs/CHANGELOG.md` (P009 entry)
  - `README.md` (brief Phase 2.2 mention + `[retry]` config snippet)
  - `tests/cli_run_retry.rs` (NEW integration test)

- **KHÔNG sửa:**
  - `src/cli/*` (retry là internal flow của `core::run::run` — không có subcommand mới, không có CLI flag mới)
  - `src/cli/mod.rs` (CONSTRAINT #1 re-instated post-P006 V2, honored P008 — KHÔNG touch dispatch)
  - `src/mcp/*` (MCP `run` tool tự động hưởng retry vì gọi `core::run::run` — không touch tool schema)
  - `src/runner.rs` (Architect decision — runner stays single-fire primitive; retry policy là orchestration, không là spawn mechanic)
  - `src/launchd.rs`, `src/heartbeat.rs`, `src/alert.rs` (retry orchestration ở `core/run.rs`, không xuyên vào các module low-level)
  - `src/core/init.rs`, `src/core/register.rs`, `src/core/unregister.rs`, `src/core/status.rs` (chỉ `core/run.rs` cần retry wire)
  - `Cargo.toml` (KHÔNG add runtime dep — `tokio::time::sleep` đã có; KHÔNG add dev-dep — wiremock từ P008 đủ)
  - Heartbeat schema (`HeartbeatRecord` struct in `src/heartbeat.rs`) — schema unchanged, 1 record per attempt

### Skills consulted

*(none — Architect sourced từ ARCHITECTURE.md, P008 phiếu/Discovery, BACKLOG.md, Sếp brief.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Architect KHÔNG có Bash/Grep — anchors sourced từ ARCHITECTURE.md, CHANGELOG.md P008 entry, P008 phiếu Worker Turn 1 evidence, DISCOVERIES.md. Worker BẮT BUỘC verify thực tế tại Task 0.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/core/run.rs` chứa `pub async fn run(args: RunArgs) -> Result<RunOutput>` (entry point cho retry loop wrap) | `grep -n "pub async fn run" src/core/run.rs` | `[unverified]` per ARCHITECTURE.md:48 + P008 Turn 1 evidence shows line 32 | ✅ Confirmed `src/core/run.rs:32` |
| 2 | `src/core/run.rs` gọi `runner::fire_task(...)` once và xử lý `fire_result` via `match` block binding `exit_code, stdout_tail, stderr_tail, duration_ms` | `grep -n "runner::fire_task\|match fire_result" src/core/run.rs` | `[unverified]` per P008 Anchor #2 V2 evidence (line 77 match binding) | ⚠️ TWO match blocks: `match &fire_result` (borrow, line 54 HeartbeatRecord build) then `match fire_result` (consume, line 77 quadruple extract). Task 3 `todo!()` must copy BOTH. See Turn 1. |
| 3 | `src/core/run.rs` gọi `heartbeat::append(&config.heartbeat.log_path, &record)` AFTER fire — current shape: ONE append per run | `grep -n "heartbeat::append" src/core/run.rs` | `[unverified]` per P008 Anchor #2 evidence (line 74) | ✅ Confirmed `src/core/run.rs:74` |
| 4 | `src/core/run.rs` chứa P008 alert wiring block (AFTER `match fire_result`, BEFORE `Ok(RunOutput)`) — must be MOVED outside retry loop | `grep -n "TelegramAlert\|send_with_base" src/core/run.rs` | `[unverified]` per CHANGELOG P008 line 22 | ✅ Confirmed `src/core/run.rs:95-127` (let-chain form `if exit_code != 0 && let Some(alert_cfg)`) |
| 5 | `Config` struct ở `src/config.rs` chưa có field `retry` | `grep -n "pub retry" src/config.rs` → expect empty | `[unverified]` | ✅ Confirmed empty — no conflict |
| 6 | `Config` đã derive `Serialize + Deserialize` (cho serde) — confirmed P008 Anchor #4 | `grep -n "#\[derive" src/config.rs` quanh struct Config | `[verified]` per P008 Anchor #4 result | ✅ Confirmed `src/config.rs:30` |
| 7 | `Cargo.toml` có `tokio` với feature `time` (cho `tokio::time::sleep` + `Duration`) — confirmed P008 Anchor #6 | `grep -A2 '^tokio' Cargo.toml` | `[verified]` per P008 Anchor #6 + CLAUDE.md tech stack | ✅ Confirmed `Cargo.toml:18` |
| 8 | `runner::fire_task` returns `Result<RunResult>` where `RunResult { exit_code: i32, stdout: String, stderr: String, duration: Duration }` — caller can call repeatedly with same config (no mutable state) | docs/ARCHITECTURE.md §Modules row `src/runner.rs`:55 + CHANGELOG P004:179 | `[unverified]` per ARCHITECTURE.md | ✅ Confirmed `src/runner.rs:36` — `pub async fn fire_task(config: &Config) -> Result<RunResult>`. RunResult fields: exit_code: i32, stdout: String, stderr: String, duration_ms: u64 (NOT Duration). Immutable &Config borrow — safe to call in loop. |
| 9 | `runner::fire_task` returns `exit_code = -1` on spawn failure per INV-14 + P004 contract (means: spawn-fail is distinct from any real process exit code which is ∈ 0..=255 or 128+signal) | docs/security/INVARIANTS.md INV-14 (P004 line 178-179) | `[unverified]` per CHANGELOG P004 line 184 `exit_code=-1 in heartbeat` | ⚠️ PARTIAL MISMATCH — see Turn 1. `runner::fire_task` does NOT return exit_code=-1 for spawn-fail; it returns `Result::Err`. The -1 sentinel is synthesized by `core/run.rs:66` in the `Err(spawn_err)` match arm. Signal-killed (no OS exit code) → `unwrap_or(-1)` in `runner.rs:54`. Two distinct -1 sources. |
| 10 | INVARIANTS.md max INV currently = 19 (slot for INV-20 free) — P008 added INV-19 | `grep -c "^### INV-" docs/security/INVARIANTS.md` | `[unverified]` per CHANGELOG P008 line 26 "INV-19 appended" | ✅ Confirmed count = 19 |
| 11 | `HeartbeatRecord` schema unchanged from P004 — fields `ts`, `label`, `exit_code`, `duration_ms`, `stdout_tail`, `stderr_tail` — Architect intentionally does NOT add `retry_attempt` field | docs/ARCHITECTURE.md §Heartbeat schema:270 + CHANGELOG P004:179 | `[verified]` per ARCHITECTURE.md | ✅ Schema preserved |
| 12 | CONSTRAINT #1 re-instated post-P006 V2, honored by P008: KHÔNG touch `src/cli/mod.rs` — P009 honors same | Sếp brief Heads-up #2 + docs/discoveries/P006.md + P008 phiếu Constraint #1 | `[verified]` per Sếp brief | ✅ Honored — `mod retry;` NOT created (retry logic stays in `core/run.rs`, no new module per Architect Decision §Giải pháp item 2) |
| 13 | CONSTRAINT #4: "all 5 core fns resolve env internally" — P009 không add config-path threading; `config` đã in scope trong `core::run::run` | docs/ARCHITECTURE.md §Modules "V2 internal-resolution pattern (P006)":62 | `[verified]` | ✅ `config` in scope, retry reads `config.retry` directly |
| 14 | CONSTRAINT #11 (P008): KHÔNG add env var reads in `src/alert.rs` — P009 doesn't touch `alert.rs` at all | docs/ticket/P008 Constraint #11 + CHANGELOG P008 line 46 | `[verified]` | ✅ N/A — `src/alert.rs` not in P009 scope |
| 15 | Phase 1 baseline + P008 = 116 tests passing — P009 adds new unit tests for retry + integration, expects ≥ 116 baseline preserved | CHANGELOG P008 line 33 "+22 new, total 116" | `[verified]` per CHANGELOG | ✅ Baseline 116 |
| 16 | `tokio::time::sleep(Duration::from_secs(N))` is the canonical async sleep — non-busy (yields to runtime) | tokio docs (standard async pattern) | `[verified]` per CLAUDE.md tech stack + tokio docs | ✅ Standard |
| 17 | `core::run::run` is `async` — `.await` on `tokio::time::sleep` works without restructure | docs/ARCHITECTURE.md §Modules `core/run.rs`:48 ("async run") + Anchor #1 above | `[verified]` per ARCHITECTURE.md | ✅ `async fn run` |

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND). Cap = 3 turns.

**Phiếu version:** V2 (Turn 1 Architect Response — Option A accepted, Task 3 Lưu ý two-match invariant codified)

### Turn 1 — Worker Challenge

**Date:** 2026-05-27
**Mode:** CHALLENGE (V1)

**Anchor verification summary:**

- Anchor #1: ✅ `pub async fn run` confirmed at `src/core/run.rs:32`
- Anchor #2: ⚠️ Layout differs from phiếu's single-match assumption — see O1.1 below
- Anchor #3: ✅ `heartbeat::append` confirmed at `src/core/run.rs:74`
- Anchor #4: ✅ Alert block confirmed `src/core/run.rs:95-127` (let-chain form — no structural blocker)
- Anchor #5: ✅ No `pub retry` field exists in `src/config.rs` — clean insertion
- Anchor #6: ✅ `#[derive(Debug, Clone, Serialize, Deserialize)]` at `src/config.rs:30`
- Anchor #7: ✅ `tokio = { ..., features = ["...", "time", ...] }` confirmed in `Cargo.toml`
- Anchor #8: ✅ `pub async fn fire_task(config: &Config) -> Result<RunResult>` at `src/runner.rs:36`. `RunResult.exit_code: i32`. Minor field name discrepancy: phiếu says `duration: Duration` but actual field is `duration_ms: u64` — no impact on retry logic.
- Anchor #9: ⚠️ Spawn-fail convention differs from phiếu description — see O1.2 below
- Anchor #10: ✅ `grep -c "^### INV-"` returns 19 — INV-20 slot free
- main.rs mods: ✅ No new `mod` needed — `mod core;` already present at `src/main.rs:9`

---

**Architect's 5 heads-up resolutions — Worker positions:**

1. **1 heartbeat per attempt (no schema change):** AGREE. `HeartbeatRecord` shape confirmed unchanged at `src/core/run.rs:55-70`. Two-match structure per iteration preserves the existing record-build pattern cleanly.

2. **Retry location = `core/run.rs` (not new module):** AGREE. The existing fire+heartbeat+alert pipeline in `core/run.rs` is exactly the right container. No new module needed; `mod core;` in `main.rs:9` already covers it.

3. **Retryable predicate 1-127 (excludes 0, ≥128 signal, -1 spawn-fail):** AGREE with one clarification (see O1.2) — the predicate `(1..=127).contains(&exit_code)` is correct. The -1 exclusion works because -1 is not in 1..=127. Worker just needs to understand WHERE -1 comes from (see O1.2) to implement the loop correctly.

4. **Backwards-compat `unwrap_or((1, 0))`:** AGREE. `Option<RetryConfig>` deserialized as `None` when `[retry]` block absent, then `.map(...).unwrap_or((1, 0))` yields single-fire. Clean.

5. **1 alert per run (Constraint #8 + INV-20):** AGREE. Alert block at `src/core/run.rs:95-127` moves cleanly outside the loop. Single call site enforced by Constraint #8 (`grep -c "send_with_base" src/core/run.rs` == 1).

---

**`heartbeat_appended: bool` — Tầng 2 Worker preview (Architect punted):**

Worker recommends keeping `bool` = last-attempt semantics. Rationale: (a) no consumer (`cli/run.rs`, `mcp/tools.rs`) currently displays this field in a way that misleads; (b) changing to `u32` count would require updating `RunOutput` consumers and integration tests — scope creep for minimal benefit; (c) a comment in code clarifying "last-attempt result" is sufficient for future maintainability. Worker will self-decide `bool` at EXECUTE and log to Discovery Report.

---

**Objections (Tầng 1):**

**[O1.1] — `src/core/run.rs` has TWO match blocks on `fire_result`, not one**

Evidence: `src/core/run.rs:54` — `match &fire_result` (borrow, builds `HeartbeatRecord`). `src/core/run.rs:77` — `match fire_result` (consumes, extracts `(exit_code, stdout_tail, stderr_tail, duration_ms)`). Phiếu's Task 3 snippet has ONE `match fire_result` with `todo!("WORKER: copy match arms from existing single-fire body")` for both the HeartbeatRecord build and the quadruple extract. This is actually correct as a directive (Worker copies both match blocks), but the loop design in the snippet has a structural implication:

In the single-fire code, `fire_result` is first borrowed (`match &fire_result`) to build `record`, then consumed (`match fire_result`) to extract variables. In the retry loop, `let fire_result = runner::fire_task(...).await` is called each iteration, and the same borrow-then-consume pattern works identically per iteration. **No blocking problem** — Worker copies both match blocks inside the loop body with `let fire_result = ...` fresh each iteration.

**Assessment:** Not a blocking objection — Task 3's `todo!()` approach is correct. Worker notes this for EXECUTE to avoid confusion. No phiếu text change required.

**[O1.2] — Spawn-fail produces `Result::Err` from `runner::fire_task`, NOT `Ok(RunResult { exit_code: -1 })`**

Evidence: `src/runner.rs:36-50` — spawn failure (binary not found / perm denied) propagates as `anyhow::Error` via `.with_context(...)?.` — it returns `Result::Err`, not `Ok`. The -1 sentinel in heartbeat comes from `src/core/run.rs:66` in the `Err(spawn_err)` arm of `match &fire_result`. Signal-killed (no OS exit code) uses `unwrap_or(-1)` at `src/runner.rs:54` — this IS returned as `Ok(RunResult { exit_code: -1 })`.

Phiếu's Task 3 snippet writes `let (exit_code, ...) = match fire_result { _ => todo!("copy match arms") }` as a flat destructuring, implying fire_result always produces a tuple. But `fire_result: Result<RunResult>` — `Err` arm must explicitly map to `(-1, String::new(), stderr_tail, duration_ms)` as the existing single-fire code does at `src/core/run.rs:84-89`.

**Impact on retry loop:** The `is_retryable(exit_code)` predicate is called AFTER both match blocks have resolved exit_code. Since the existing code at `src/core/run.rs:84-89` already maps `Err(_)` to `exit_code = -1`, and `-1` is excluded by `(1..=127).contains(&exit_code)`, the spawn-fail non-retry behavior is CORRECT — BUT only because Worker preserves the two-match structure (borrow for record, consume for variables). If Worker simplified to a single match (e.g. extracting from `Ok` only and early-returning on `Err`), the retry loop would skip heartbeat for spawn-fail iterations.

**This is a Tầng 1 clarification:** the retry loop must preserve the existing two-match pattern so that spawn-fail iterations still write a heartbeat (with exit_code=-1). Phiếu's `todo!("copy match arms")` directive handles this IF Worker copies verbatim — but the invariant should be explicit to prevent a Worker re-reading the snippet in isolation from getting it wrong.

**Proposed alternatives:**

A. (Recommended) **Architect adds 1 sentence to Task 3 Lưu ý:** "Worker must preserve the existing two-match structure per iteration: `match &fire_result` (borrow, build HeartbeatRecord including exit_code=-1 for Err) then `match fire_result` (consume, extract tuple). Spawn-fail MUST still append a heartbeat per iteration — do NOT short-circuit on `fire_result.is_err()` before the heartbeat::append call." No other phiếu changes needed. Worker proceeds to EXECUTE.

B. **Accept phiếu as-is** — Worker self-notes O1.2 as a Tầng 2 implementation caution and handles it at EXECUTE without phiếu text change. Risk: another Worker reading the phiếu in isolation might miss it.

Worker recommends **A** (explicit Lưu ý addition) given this is an invariant about heartbeat completeness (INV-15 adjacent) and misimplementation would produce silent data loss (no heartbeat for spawn-fail retries).

**Status:** ⚠️ O1.2 raised — AWAITING ARCHITECT RESPONSE on whether to add clarifying Lưu ý (Option A) or accept as-is (Option B). O1.1 is non-blocking (informational). All 5 heads-up resolutions: AGREE.

### Turn 1 — Architect Response (phiếu V2)

**Date:** 2026-05-27
**Mode:** RESPOND (V1 → V2)

- **[O1.1] — TWO match blocks (non-blocking, informational)** → **ACCEPT (acknowledge)**. Worker's evidence at `src/core/run.rs:54` (borrow) and `:77` (consume) is sound. Architect's V1 Task 3 snippet already directs Worker to "copy match arms VERBATIM from existing single-fire body" — the borrow-then-consume pattern works identically per iteration since `let fire_result = runner::fire_task(...).await` is fresh each loop iteration. No phiếu text change required for O1.1 alone (Worker self-confirmed non-blocking). The clarifying sentence added for O1.2 (below) covers the structural concern for both objections.

- **[O1.2] — Spawn-fail two-match invariant (Tầng 1 clarification)** → **ACCEPT Option A**. Worker's evidence is correct: `runner::fire_task` returns `Result::Err` for spawn-fail (NOT `Ok(exit_code=-1)`); the -1 sentinel is synthesized by `core/run.rs:66` in the `Err(spawn_err)` arm of `match &fire_result`. The risk Worker identifies is real — a future Worker reading Task 3's snippet in isolation could mis-implement by short-circuiting on `fire_result.is_err()` and skip the heartbeat::append for spawn-fail iterations, violating INV-15's implicit completeness contract. Action: Architect adds 1 invariant-codifying sentence to Task 3 Lưu ý (1st bullet, before the existing "DOES NOT replace entire file" sentence). Phiếu V2 ship.

- **5 heads-up resolutions** → all AGREED by Worker. No Architect action needed. Reaffirmed.

- **`heartbeat_appended: bool` Tầng 2 preview** → **REFRAME** (Tầng 2). Architect's V1 Task 3 Lưu ý already punted this to Worker self-decide at EXECUTE. Worker's preview rationale (no consumer misled, scope creep avoidance, comment-clarification sufficient) is sound. Worker self-decides `bool` at EXECUTE, logs to Discovery Report per existing punt. No phiếu text change.

**Changes applied to phiếu V2:**
- Task 3 Lưu ý — prepended new bullet codifying two-match heartbeat-completeness invariant (covers both O1.1 borrow-then-consume pattern + O1.2 spawn-fail heartbeat-append guarantee).
- `Phiếu version:` header updated V1 → V2.

**Status:** ✅ RESPONDED — phiếu bumped to V2. Both objections resolved (O1.1 ACCEPT-acknowledge, O1.2 ACCEPT Option A). No DEFER. Ready for Worker (CHALLENGE) verification of consensus OR proceed to approval gate per orchestrator routing.

### Turn 2 — Worker Challenge

**Date:** 2026-05-27
**Mode:** CHALLENGE (V2)

**V2 change verification:**

1. Task 3 Lưu ý lead bullet (two-match invariant): CONFIRMED present at start of Lưu ý block. Text codifies: two-match structure per iteration (`match &fire_result` borrow then `match fire_result` consume), `heartbeat::append` between the two matches, spawn-fail iterations must write heartbeat, no `fire_result.is_err()` short-circuit, `grep -c "heartbeat::append" src/core/run.rs` == 1.

2. Constraint #12 added: CONFIRMED present. Language mirrors Task 3 invariant; enforcement command `grep -c "heartbeat::append" src/core/run.rs` == 1 is explicit.

3. Acceptance Automated check for `grep -c "heartbeat::append" src/core/run.rs` exactly `1`: CONFIRMED present in Nghiệm thu §Automated, tied to Constraint #12.

4. `heartbeat_appended` bool Tầng 2 punt REFRAME: CONFIRMED. Turn 1 Architect Response (line 195) states no phiếu text change; Task 3 Lưu ý carries the existing punt language unchanged. Worker's Turn 1 preview (keep `bool`, last-attempt semantics, comment-clarify) is accepted. Worker self-decides at EXECUTE.

**Anchor verification summary (re-check not required for Turn 2 — all anchors were ✅/⚠️ resolved in Turn 1; V2 changes are text-only, no new code anchors added):** All Turn 1 anchor results remain valid. No new anchor discrepancies introduced by V2 text changes.

**Objections:** NONE. All four V2 changes are correctly and completely present. The two-match invariant codification (Task 3 Lưu ý + Constraint #12 + Acceptance check) comprehensively addresses Turn 1 O1.2. The Tầng 2 reframe is consistent across Architect Response and Task 3 Lưu ý.

**Worker accepted V2 — no new challenges.** Ready for Chủ nhà approval gate.

**Status:** ✅ CONSENSUS REACHED — V2 phiếu verified. Proceed to approval gate.

### Final consensus
- Phiếu version: V2
- Total turns: 2 (Turn 1 Worker objection O1.2 → Architect Option A → Turn 2 Worker accepted)
- Approved (autonomous narrate or Sếp gate): 2026-05-27 — code execution may begin

---

## Debug Log (advisory-cron specific)

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | N/A — no new launchd plist, retry fires from existing `run` path | — | | N/A |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test config::tests::` (new RetryConfig tests) | targeted pass | | |
| B (capability) | `cargo test --test cli_run_retry` (new integration) | pass | | |
| B (capability) | `cargo test --test cli_run_alert` (P008 regression) | still pass | | |
| C (migration) | Schema change: `Config` gains optional `retry` field. Backwards-compat: old config without `[retry]` block must still load. Verify: `cargo test config::tests::load_without_retry_block` | old configs deserialize OK with `config.retry == None` | | |
| C (migration) | Heartbeat schema UNCHANGED — verify by `git diff src/heartbeat.rs` empty | no diff | | |
| D (persistence) | `grep -l "INV-20" docs/security/INVARIANTS.md` | ≥1 hit | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (no new dep added) | | |
| E (env drift) | `cargo build --release` clean target | exit 0, binary still ≤7MB | | |

---

## Nhiệm vụ

### Task 0 — Anchor verification (BẮT BUỘC TRƯỚC mọi Task khác)

**Mục đích:** Architect không có Bash/Grep — anchor table dựa docs. Worker chạy grep commands, fill Kết quả column, escalate qua Debate Log Turn 1 nếu phát hiện sai lệch (e.g. `Config` đã có `retry` field — duplicate; or `core/run.rs` has different layout than P008 evidence suggests).

**Lệnh chạy (anchors 1-10, in order):**
```bash
grep -n "pub async fn run" src/core/run.rs                          # #1 expect ~line 32
grep -n "runner::fire_task\|match fire_result" src/core/run.rs      # #2 expect runner::fire_task call + match
grep -n "heartbeat::append" src/core/run.rs                          # #3 expect ~line 74
grep -n "TelegramAlert\|send_with_base" src/core/run.rs              # #4 expect P008 alert block present
grep -n "pub retry" src/config.rs                                    # #5 expect empty
grep -n "#\[derive" src/config.rs | head -5                          # #6 confirmation
grep -A2 '^tokio' Cargo.toml                                         # #7 confirmation
grep -n "pub fn fire_task\|RunResult\|exit_code" src/runner.rs       # #8 verify signature
grep -n "exit_code.*-1\|exit_code: -1\|exit_code: Some(-1)" src/runner.rs # #9 spawn-fail signal
grep -c "^### INV-" docs/security/INVARIANTS.md                       # #10 expect 19
grep -n "^mod " src/main.rs                                          # confirm no need for new mod
```

**Output:** fill table → if mọi anchor ✅ → proceed Task 1. If ⚠️/❌ → write Debate Log Turn 1 objection (Architect RESPOND mode required).

**Special focus for Worker:**
- Anchor #4 — confirm P008 alert block shape BEFORE Task 3 wraps loop around it. Architect's Task 3 below assumes alert call is INSIDE `if exit_code != 0` AFTER `match fire_result` block. If layout differs (e.g. P008 ship moved it elsewhere), challenge in Turn 1 with `file:line` evidence.
- Anchor #8 — confirm `runner::fire_task` returns `Result<RunResult>` AND that `RunResult.exit_code` is `i32`. If signature differs (e.g. returns `Result<i32>` directly or uses `u8`), challenge with evidence — affects `is_retryable` predicate signature in Task 3.
- Anchor #9 — confirm spawn-fail produces `exit_code = -1` (or equivalent sentinel). If runner uses different convention (e.g. `Result::Err` propagates, never produces `-1`), challenge — `is_retryable` predicate must handle correctly.

---

### Task 1: Extend `src/config.rs` — add `RetryConfig` + `Config::retry` field + validation

**File:** `src/config.rs`

**Tìm:** Cuối module (sau existing struct definitions; Worker grep `pub struct AlertConfig` or `pub struct TelegramConfig` để locate insertion point post-P008 alert structs — append retry block AFTER alert block for visual grouping).

**Thay bằng / Thêm:**
```rust
/// `[retry]` block. Optional — retry is opt-in. When absent, behavior is
/// single-fire (1 attempt, no retry), preserving Phase 2.1 semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of fire attempts. `1` = no retry (single attempt).
    /// `≥ 2` = retry up to (max_attempts - 1) times after initial failure.
    /// Validation: must be ≥ 1.
    pub max_attempts: u32,
    /// Seconds to sleep between attempts. `0` = retry immediately.
    /// Validation: must be ≤ 3600 (sanity cap, prevent freezing launchd
    /// job for a day via typo).
    pub backoff_secs: u64,
}
```

**Also Tìm:** struct `Config` definition (Worker grep `pub struct Config`).

**Thay bằng / Thêm:** add field (after existing `alert` field from P008):
```rust
pub struct Config {
    // ... existing fields (task, schedule, heartbeat, alert) ...
    #[serde(default)]
    pub retry: Option<RetryConfig>,
}
```

**Also Tìm:** `Config::validate` function (Worker grep `fn validate`).

**Thay bằng / Thêm:** add validation step (after existing alert validation block from P008):
```rust
// after existing validations
if let Some(retry) = &self.retry {
    if retry.max_attempts < 1 {
        anyhow::bail!("[retry].max_attempts must be ≥ 1 (got {})", retry.max_attempts);
    }
    if retry.backoff_secs > 3600 {
        anyhow::bail!(
            "[retry].backoff_secs sanity cap exceeded — got {} (max 3600 = 1 hour). \
             Use a shorter backoff or disable retry by removing the [retry] block.",
            retry.backoff_secs
        );
    }
}
```

**Also Tìm:** `default_for_home` function (Worker grep `pub fn default_for_home`).

**Thay bằng:** **KHÔNG MODIFY** — default config does NOT include `[retry]` block. Retry is strictly opt-in; `advisory-cron init` writes config without retry. Sếp manually adds `[retry]` to enable.

**Lưu ý:**
- `#[serde(default)]` on `retry` field — old configs (including P008-era ones with only `[alert]`) without `[retry]` deserialize as `None`. Backwards-compat preserved.
- `RetryConfig` does NOT derive `Default` — required fields `max_attempts` + `backoff_secs` mean no sensible default (Sếp must specify both explicitly when opting in).
- Add unit tests in `src/config.rs::tests`:
  - `test_load_without_retry_block` — config WITHOUT `[retry]` → `config.retry == None` (backwards-compat).
  - `test_load_with_retry_block` — `[retry] max_attempts=3 backoff_secs=5` → `Some(RetryConfig { 3, 5 })`.
  - `test_validate_retry_zero_attempts` — `max_attempts=0` → `Err`.
  - `test_validate_retry_excessive_backoff` — `backoff_secs=7200` → `Err`.
  - `test_load_with_retry_and_alert` — both blocks present, both deserialize correctly (no interference).

---

### Task 2: Add `is_retryable` helper (private fn in `src/core/run.rs`)

**File:** `src/core/run.rs`

**Tìm:** Top of module (after `use` imports, before `pub async fn run`). Worker grep `^use\|^pub async fn run` to locate insertion point.

**Thay bằng / Thêm:**
```rust
/// Retry decision predicate. Per BACKLOG Phase 2.2 spec:
/// - `exit_code ∈ 1..=127` → retryable (normal process error exit)
/// - `exit_code ≥ 128` → NOT retryable (signal-killed: 130=SIGINT, 137=SIGKILL,
///   143=SIGTERM; convention `128 + signal_num`). Signal kills are operator
///   actions or OOM, not transient errors — retry would fight the operator.
/// - `exit_code == 0` → success (caller checks before calling this fn)
/// - `exit_code == -1` (spawn failure sentinel per INV-14 + P004 contract) →
///   NOT retryable. Spawn failure = command path missing / not executable =
///   deploy/config bug. Retry won't help; surface immediately.
fn is_retryable(exit_code: i32) -> bool {
    (1..=127).contains(&exit_code)
}
```

**Lưu ý:**
- Pure function — easy to unit-test in isolation.
- Add unit tests in `src/core/run.rs::tests`:
  - `is_retryable_exit_1_true`
  - `is_retryable_exit_127_true`
  - `is_retryable_exit_0_false` (success, retry not needed — caller checks)
  - `is_retryable_exit_128_false` (signal boundary)
  - `is_retryable_exit_130_false` (SIGINT)
  - `is_retryable_exit_137_false` (SIGKILL)
  - `is_retryable_exit_143_false` (SIGTERM)
  - `is_retryable_exit_neg1_false` (spawn failure sentinel)

---

### Task 3: Rewire `core::run::run` to wrap retry loop around fire+heartbeat, move alert OUTSIDE loop

**File:** `src/core/run.rs`

**Tìm:** Worker uses Task 0 Anchor #1, #2, #3, #4 results to locate:
1. Start of `pub async fn run` (Anchor #1 — ~line 32 per P008 evidence)
2. `runner::fire_task(&config.task).await` call (Anchor #2)
3. `match fire_result` block binding `exit_code, stdout_tail, stderr_tail, duration_ms` (Anchor #2 — ~line 77 per P008 evidence)
4. `heartbeat::append(&config.heartbeat.log_path, &record)` call (Anchor #3 — ~line 74 per P008 evidence)
5. P008 alert block `if exit_code != 0 { ... TelegramAlert ... }` (Anchor #4)
6. Final `Ok(RunOutput { ... })` return (~line 92 per P008 evidence)

**Strategy:** rather than diff-edit the existing single-fire body (fragile against P008 actual layout), **rewrite the body of `pub async fn run` from the point where `runner::fire_task` is called to just before `Ok(RunOutput)` return**. Preserve all pre-fire setup (config load, label resolution, log_path resolution) and the final `RunOutput` construction.

**Thay bằng / Thêm:** Worker replaces the single-fire body (from "fire task once" through "alert call") with the retry-loop body below. Preserve variable names already used by the function (e.g. `config`, `label`, `start`, anything from setup phase). The loop yields the SAME final variables (`exit_code`, `stdout_tail`, `stderr_tail`, `duration_ms`) used to build `RunOutput`.

```rust
// Phase 2.2 — Retry loop (wraps single-fire `runner::fire_task` from Phase 1.4).
// max_attempts == 1 (or [retry] absent) = single-fire behavior preserved.
//
// Per BACKLOG Phase 2.2: exit code 1-127 retryable; ≥128 (signal) and -1 (spawn-fail) NOT.
// Heartbeat schema unchanged — 1 record per attempt (P009 Architect decision §Giải pháp item 3).
// Alert wiring moved OUTSIDE loop — 1 alert max per `run` invocation regardless of attempt count.
let (max_attempts, backoff_secs) = config
    .retry
    .as_ref()
    .map(|r| (r.max_attempts.max(1), r.backoff_secs))
    .unwrap_or((1, 0));

let mut final_exit_code: i32 = 0;
let mut final_stdout_tail = String::new();
let mut final_stderr_tail = String::new();
let mut final_duration_ms: u64 = 0;

for attempt in 1..=max_attempts {
    let fire_result = crate::runner::fire_task(&config.task).await;

    // Build HeartbeatRecord for THIS attempt — same shape as Phase 1.4.
    // Worker: re-use the existing record-building code from the single-fire
    // version. Variable bindings produced by the match below (`exit_code`,
    // `stdout_tail`, `stderr_tail`, `duration_ms`) are LOCAL to each iteration.
    let (exit_code, stdout_tail, stderr_tail, duration_ms) = match fire_result {
        // Worker: copy match arms VERBATIM from existing single-fire body.
        // (Architect cannot reproduce them exactly without reading runner.rs;
        // shape per P008 Worker Turn 1 evidence at src/core/run.rs:77 — the
        // `(i32, String, String, u64)` quadruple. Spawn failure → exit_code=-1.)
        _ => todo!("WORKER: copy match arms from existing single-fire body"),
    };

    // Append heartbeat per attempt (schema unchanged — 1 JSONL line per fire).
    // Worker: re-use the existing record construction + heartbeat::append call.
    let record = crate::heartbeat::HeartbeatRecord {
        // Worker: copy field assignments VERBATIM from existing single-fire body.
        // ts: chrono::Utc::now().to_rfc3339(),
        // label: label.clone(),
        // exit_code,
        // duration_ms,
        // stdout_tail: stdout_tail.clone(),
        // stderr_tail: stderr_tail.clone(),
        ..todo!("WORKER: copy from existing single-fire body")
    };
    let _ = crate::heartbeat::append(&config.heartbeat.log_path, &record);

    // Capture as "final" — overwritten by subsequent attempts; whatever
    // survives the loop exit is what gets returned in RunOutput + alert.
    final_exit_code = exit_code;
    final_stdout_tail = stdout_tail;
    final_stderr_tail = stderr_tail;
    final_duration_ms = duration_ms;

    // Loop exit decisions:
    if exit_code == 0 {
        // Success — stop retrying.
        break;
    }
    if !is_retryable(exit_code) {
        // Signal-killed (≥128) or spawn-failure (-1) — retry won't help.
        tracing::warn!(
            attempt,
            exit_code,
            "task fire produced non-retryable exit code, not retrying"
        );
        break;
    }
    if attempt == max_attempts {
        // Exhausted — stop and let post-loop alert fire.
        break;
    }
    // Transient failure — backoff then retry.
    tracing::info!(
        attempt,
        next_attempt = attempt + 1,
        backoff_secs,
        exit_code,
        "task fire failed with retryable exit code, sleeping before retry"
    );
    tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
}

// Phase 2.1 alert — re-located by P009 from INSIDE single-fire body to AFTER
// the retry loop. Invariant: EXACTLY 1 alert max per `advisory-cron run`
// invocation, regardless of how many retries fired. Per PROJECT.md hard line
// #5 "Failure mode = noisy" applied to FINAL failure only (transient blip
// that succeeded on retry 2 of 3 = no alert).
if final_exit_code != 0 {
    if let Some(alert_cfg) = config.alert.as_ref().and_then(|a| a.telegram.as_ref()) {
        match crate::alert::TelegramAlert::from_config(Some(alert_cfg)) {
            Ok(Some(alert)) => {
                // Env-var-at-call-site (P008 V2 — preserved by P009).
                let api_base = std::env::var("ADVISORY_CRON_TG_API_BASE")
                    .unwrap_or_else(|_| "https://api.telegram.org".to_string());
                let msg = crate::alert::format_failure_message(
                    &label,
                    final_exit_code,
                    final_duration_ms,
                    &final_stderr_tail,
                );
                if let Err(e) = alert.send_with_base(&api_base, &msg).await {
                    tracing::warn!(error = %e, "telegram alert send failed (best-effort, swallowing)");
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, "telegram alert config invalid (best-effort, swallowing)");
            }
        }
    }
}

// Build RunOutput from final-attempt values. (Worker: preserve existing
// RunOutput shape — Architect does not redefine it.)
// Ok(RunOutput {
//     exit_code: final_exit_code,
//     stdout_tail: final_stdout_tail,
//     stderr_tail: final_stderr_tail,
//     duration_ms: final_duration_ms,
//     heartbeat_appended: true,  // best-effort, may be inaccurate if retry attempt N failed to append
// })
```

**Lưu ý — critical Worker guidance:**

- **TWO-MATCH HEARTBEAT-COMPLETENESS INVARIANT (P009 V2 — codifies Turn 1 O1.2 resolution):** Worker MUST preserve the existing two-match structure per loop iteration: first `match &fire_result` (borrow, build `HeartbeatRecord` — including the `Err(spawn_err)` arm at `src/core/run.rs:66` which synthesizes `exit_code = -1` for spawn-fail), THEN `match fire_result` (consume, extract the `(exit_code, stdout_tail, stderr_tail, duration_ms)` quadruple per evidence at `src/core/run.rs:77`). The `heartbeat::append` call MUST run between the two matches (per current `src/core/run.rs:74` layout) so that spawn-fail iterations STILL write a heartbeat JSONL line (with `exit_code=-1`). DO NOT short-circuit on `fire_result.is_err()` to skip heartbeat — that would silently lose heartbeat data on spawn-fail retries and adjacent-violate INV-15 (heartbeat per fire). The Architect snippet's flat `let (exit_code, ...) = match fire_result { _ => todo!() }` is a SIMPLIFIED schematic — Worker's verbatim copy of the existing two-match pattern is what implements it correctly. Per Turn 1 Worker evidence: spawn-fail surfaces as `Result::Err` from `runner::fire_task`, the `-1` sentinel is synthesized in `core/run.rs`, NOT in `runner.rs`; signal-killed surfaces as `Ok(RunResult { exit_code: -1 })` via `unwrap_or(-1)` in `runner.rs:54`. Both -1 paths land in the same `is_retryable(-1) == false` → loop breaks → post-loop alert fires once. Constraint: `grep -c "heartbeat::append" src/core/run.rs` MUST be exactly `1` after edit (single call site, inside the loop body between the two matches).

- **Worker DOES NOT replace entire file** — only the body region from `let fire_result = runner::fire_task(...)` through the alert block, inclusive. Function signature, pre-fire setup (config load, label/log_path resolution), and the final `Ok(RunOutput { ... })` builder line are PRESERVED. Architect's snippet above has `todo!()` placeholders where Worker MUST copy from the existing single-fire body (match arms, HeartbeatRecord field assignments) — these are details Architect cannot reproduce verbatim without reading code (`[needs Worker verify]`).

- **`max_attempts.max(1)` defensive floor** — even though `Config::validate` rejects `max_attempts < 1`, the runtime floor here protects against `validate` being bypassed (defense-in-depth). `0` would cause `for 1..=0` → empty range → zero attempts, returning the initial `final_exit_code = 0` which would falsely look like success. The `.max(1)` ensures at least one attempt always runs.

- **`heartbeat::append` return value ignored** (matches P004 behavior — heartbeat write failure = warn-continue, never blocks task). Worker preserves `let _ =` discard pattern from existing code (or refactor to log-warn if existing code does so — match existing style).

- **`heartbeat_appended` field in `RunOutput`** — current single-fire returns `bool` based on last append result. With retry, the natural meaning is ambiguous (1 of 3 may have failed). Architect punts to Worker: use last-attempt's append result, OR change to `heartbeat_appends: u32` count. **Worker decides at EXECUTE — if changing, update `RunOutput` consumers (MCP tools.rs, cli/run.rs).** If Worker decides existing `bool` is fine (last-attempt semantics), no change needed. This is Tầng 2 — Worker self-decides, log to Discovery Report. (Turn 1 Worker preview: keep `bool` = last-attempt semantics, comment-clarify in code, no consumer change. Architect REFRAME-accepts.)

- **Variable shadowing inside the loop** — `(exit_code, stdout_tail, ...)` is bound EACH iteration via `let` (new binding per iteration, NOT mut). The "final_" outer variables capture the last iteration's values. This pattern is idiomatic Rust.

- **`tracing::info!` + `tracing::warn!`** — Worker verify `tracing` macros are accessible in scope (P008 used them in same file). If not in scope, qualify as `tracing::warn!(...)` or add `use tracing::{info, warn};` at top.

- **NO config-path threading** — `config` is already in scope (passed to `core::run::run`). Constraint #4 honored.

- **NO new dep** — `tokio::time::sleep` + `std::time::Duration` already pulled.

- **`alert.rs` env-free invariant (Constraint #11)** — unchanged. P009 keeps env-var-at-call-site in `core/run.rs`. `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` should still return empty.

- **Alert variable bindings** — alert block uses `final_exit_code`, `final_duration_ms`, `final_stderr_tail`, `&label`. `label` is the same `label` resolved pre-loop in setup phase (Worker preserves the existing line that resolves it from `config.task.label`).

---

### Task 4: Append INV-20 to `docs/security/INVARIANTS.md`

**File:** `docs/security/INVARIANTS.md`

**Tìm:** After INV-19 block (Worker grep `^### INV-19 ` then find end of that section before `^---` separator).

**Thay bằng / Thêm:**
```markdown
### INV-20 — Retry policy boundary: bounded attempts, backoff respected, signal-exits not retried, single-alert-per-invocation

**Statement:** PR introducing or modifying retry logic in `src/core/run.rs` (or any future retry mechanism) MUST satisfy ALL of:

1. **Bounded attempts (DOS prevention):** the retry loop MUST terminate in at most `max_attempts` iterations. `max_attempts` is read from `config.retry.max_attempts` (validated `≥ 1` at config load). Runtime defense-in-depth: apply `.max(1)` floor before loop. NO unbounded `loop { ... }` over `fire_task`.

2. **Backoff respected (no busy loop):** between attempts (and ONLY between attempts — never before the first or after the last), the loop MUST `tokio::time::sleep(Duration::from_secs(backoff_secs)).await`. `backoff_secs` is read from `config.retry.backoff_secs` (validated `≤ 3600` at config load). NO synchronous `std::thread::sleep` (would block tokio runtime). NO retry with zero sleep when `backoff_secs > 0` (every retry interval must honor the configured backoff).

3. **Signal exits NOT retried:** `is_retryable(exit_code)` predicate MUST return `false` for `exit_code ≥ 128` (signal-killed: 130 SIGINT, 137 SIGKILL, 143 SIGTERM, etc. — convention `128 + signal_num`) AND for `exit_code == -1` (spawn-failure sentinel per INV-14). Retrying a signal-killed process fights the operator (Ctrl+C, `launchctl kill`, OOM killer); retrying a spawn-fail (command path missing) is a deploy bug surface, not a transient blip. ONLY `exit_code ∈ 1..=127` is retryable per BACKLOG Phase 2.2 spec.

4. **Single alert per invocation (no per-attempt spam):** the Telegram alert call (`crate::alert::TelegramAlert::send_with_base`) MUST be invoked AT MOST ONCE per `core::run::run` call, regardless of retry count. Alert fires ONLY after the retry loop exits — never inside a per-attempt iteration. If `final_exit_code == 0` (task succeeded on some attempt) → ZERO alerts. If `final_exit_code != 0` (all retries exhausted OR signal-killed early) → ONE alert. Heartbeat append IS per-attempt (1 JSONL line per fire — that is the durable record); alert is per-invocation (that is the push channel).

**Why:** Retry is the difference between "transient blip silently recovered" and "operator-fightable infinite spawn loop". Without bounded attempts → process bombs the user's machine + spams Telegram (and runs up Claude API spend if task is `/advisory-scan`). Without backoff → busy-loop pegs CPU and DOSes whatever the task calls (Telegram, crates.io, Claude API). Without signal-exit exclusion → operator's `Ctrl+C` is fought by the program. Without single-alert discipline → Sếp's chat gets `max_attempts` messages per failed run, defeating the "noisy = useful" intent (3 messages saying the same thing = 0 actionable signal).

**Implementation (Phase 2.2):** `src/core/run.rs` — single `for attempt in 1..=max_attempts` loop. `is_retryable(exit_code: i32) -> bool` private fn returns `(1..=127).contains(&exit_code)`. Backoff via `tokio::time::sleep(Duration::from_secs(backoff_secs)).await`, placed AFTER the iteration's heartbeat append + retry-decision branches, ONLY when next attempt will run (skip sleep on success / non-retryable / last-attempt-exhausted exit paths). Alert block placed AFTER the loop, gated on `final_exit_code != 0`.

**Trust boundary:** retry loop runs INSIDE the same `tokio` runtime as the rest of `core::run::run`. No new process boundary. No new external service. Signal handling (Ctrl+C) propagates via tokio's signal handling on `fire_task`'s child process — child receives signal → exits with `≥128` → loop sees non-retryable code → exits cleanly (operator intent honored).

**Trigger keywords:** `for attempt in` (or `while`) loops over `runner::fire_task` / `crate::runner::fire_task`; `tokio::time::sleep` near retry-shape code; `is_retryable` predicate definitions; alert calls inside iteration bodies (forbidden — would violate single-alert rule); `max_attempts` / `backoff_secs` config reads.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE (4 unit tests for `is_retryable` + integration test asserting exactly 1 alert POST after N retries). Giám sát soi PR diff for retry-related changes; if PR adds a second `alert.send` / `send_with_base` call site outside `core::run::run`, flag as INV-20 violation.

---
```

**Lưu ý:**
- INV-20 follows INV-19 voice (P008) — explicit numbered statement, why, implementation, trust boundary, trigger keywords.
- Worker grep `INV-1..19` (or "max INV"/"19 invariants" strings) in docs/ and update if any reference total count.

---

### Task 5: Add integration test `tests/cli_run_retry.rs`

**File:** `tests/cli_run_retry.rs` (CREATE)

**Tìm:** N/A (new file).

**Thay bằng / Thêm:**
```rust
//! Integration: `advisory-cron run` with retry config and various task outcomes.
//! Subprocess invokes the binary; wiremock (when used) mocks Telegram endpoint.
//!
//! Test matrix per BACKLOG Phase 2.2 acceptance criteria + INV-20:
//! - Failing task with retry config retries up to max_attempts
//! - Successful task within retries → no alert
//! - Final failure after retries → exactly 1 alert (single-alert-per-invocation)
//! - SIGTERM-like exit (signal-killed) → no retry, single attempt
//! - Each retry attempt logs 1 heartbeat
//! - Backwards-compat: no [retry] block → single-fire behavior preserved

use std::process::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Worker scaffolds the 4 tests below. Architect supplies shape + assertions.

#[tokio::test]
async fn retry_succeeds_on_attempt_2_no_alert() {
    // Worker scaffolds:
    // 1. Start wiremock MockServer; install Mock matching POST /bot*/sendMessage → 200.
    //    (Will assert ZERO POSTs received at end.)
    // 2. Create tempdir + a small shell script "flaky.sh" that:
    //    - reads attempt count from a counter file
    //    - exit 1 on attempt 1, exit 0 on attempt 2+
    //    (Use `bash -c "if [ ! -f $COUNTER ]; then touch $COUNTER; exit 1; else exit 0; fi"`
    //     or an equivalent inline shell that flips state on second call.)
    // 3. Write tempdir config.toml:
    //    [task] command="bash" args=["-c", "<flaky script>"] working_dir=tempdir label="retry-flaky"
    //    [schedule] hour=9 minute=0
    //    [heartbeat] log_path=tempdir/heartbeat.jsonl
    //    [alert.telegram] chat_id="123" bot_token="testtoken"
    //    [retry] max_attempts=3 backoff_secs=0
    // 4. Spawn subprocess:
    //    Command::new(env!("CARGO_BIN_EXE_advisory-cron"))
    //        .env("ADVISORY_CRON_TG_API_BASE", mock_server.uri())
    //        .arg("run").arg("--config").arg(&config_path)
    //        .output().expect(...)
    // 5. Assert exit code 0 (final attempt succeeded).
    // 6. Assert 2 heartbeat JSONL lines (1 per attempt: attempt 1 exit_code=1, attempt 2 exit_code=0).
    // 7. Assert mock received ZERO POSTs (success → no alert).
}

#[tokio::test]
async fn retry_exhausts_max_attempts_single_alert() {
    // Worker scaffolds:
    // 1. Start wiremock; install Mock POST /bot*/sendMessage → 200, expect 1 call.
    // 2. Write tempdir config.toml:
    //    [task] command="false" args=[] working_dir=tempdir label="retry-always-fail"
    //    [schedule] hour=9 minute=0
    //    [heartbeat] log_path=tempdir/heartbeat.jsonl
    //    [alert.telegram] chat_id="123" bot_token="testtoken"
    //    [retry] max_attempts=3 backoff_secs=0
    // 3. Spawn subprocess as above with env var.
    // 4. Assert exit code 4 (task fire failed — same code as Phase 1.4 single-fire fail).
    // 5. Assert 3 heartbeat JSONL lines (1 per attempt, all exit_code=1).
    // 6. Assert mock received EXACTLY 1 POST (INV-20 single-alert-per-invocation).
}

#[tokio::test]
async fn signal_exit_not_retried_single_attempt() {
    // Worker scaffolds:
    // 1. Start wiremock; install Mock POST /bot*/sendMessage → 200, expect 1 call.
    // 2. Write tempdir config.toml with a command that exits 143 (SIGTERM convention):
    //    [task] command="bash" args=["-c", "exit 143"] working_dir=tempdir label="retry-sigterm"
    //    [schedule] hour=9 minute=0
    //    [heartbeat] log_path=tempdir/heartbeat.jsonl
    //    [alert.telegram] chat_id="123" bot_token="testtoken"
    //    [retry] max_attempts=3 backoff_secs=0
    // 3. Spawn subprocess as above.
    // 4. Assert exit code 4 (fire failed; non-zero treated as fail per P004 contract).
    // 5. Assert EXACTLY 1 heartbeat line (NO retry — signal-like exit ≥128 is non-retryable).
    // 6. Assert mock received EXACTLY 1 POST (final failure surfaces).
    // Note: this asserts is_retryable's boundary at 128. exit_code=143 is the value
    // a real SIGTERM-killed child would produce on Unix.
}

#[tokio::test]
async fn no_retry_block_preserves_phase21_single_fire() {
    // Worker scaffolds (regression test — Phase 2.1 P008 behavior preserved when
    // [retry] block absent):
    // 1. Start wiremock; expect 1 call.
    // 2. Write tempdir config.toml WITHOUT [retry] block (only [task], [schedule],
    //    [heartbeat], [alert.telegram]) — task exits 1.
    // 3. Spawn subprocess with env var.
    // 4. Assert exit code 4.
    // 5. Assert EXACTLY 1 heartbeat line (single-fire — no retry).
    // 6. Assert EXACTLY 1 POST to mock.
}
```

**Lưu ý:**
- Integration tests use `tempdir` for config + heartbeat (no real `$HOME` pollution).
- `wiremock::MockServer` instance per test — `Mock::expect(times)` for POST count assertion.
- `backoff_secs=0` keeps tests fast (no real sleep). Backoff timing correctness is covered by Layer 2 sub-mechanism B (unit test if Worker wants extra confidence — Architect leaves to Worker discretion since INV-20 implementation already mandates `tokio::time::sleep`).
- `env!("CARGO_BIN_EXE_advisory-cron")` provides cargo's path to the compiled test binary — same pattern as `tests/cli_run_alert.rs` (P008).
- These tests REQUIRE `bash` available on PATH (true on macOS/Linux dev machines + GHA Ubuntu/macOS runners). If CI uses a stripped image, Worker may need to swap `bash -c` for an inline binary; Architect leaves this as Worker EXECUTE-time call (Tầng 2).

---

### Task 6: Update `docs/ARCHITECTURE.md` — add §Retry policy section + Modules note + Phase status

**File:** `docs/ARCHITECTURE.md`

**Tìm change 1:** §Modules table row for `src/core/run.rs` (Worker grep `core/run.rs.*orchestration\|fire task once + write heartbeat`).

**Thay bằng:** update Purpose column to mention retry loop:
```markdown
| `src/core/run.rs` | `async run(RunArgs) -> Result<RunOutput>` — retry loop wraps `runner::fire_task` (Phase 2.2); 1 heartbeat per attempt; alert outside loop (1 max per invocation). Full runner logic extracted from `cli/run.rs`. | 1.7 ✅ + 2.2 retry ✅ |
```

**Tìm change 2:** §Modules comment after the table about Phase 2.2 (Worker grep `Phase 2.2 will add` — currently says "Phase 2.2 will add `src/retry.rs`").

**Thay bằng:** update to reflect P009 decision (no new module — retry lives in `core/run.rs`):
```markdown
*(Phase 2.2 ships retry policy inline in `src/core/run.rs` — no new module per P009 Architect decision. Phase 2.3 adds crash-safe heartbeat write.)*
```

**Tìm change 3:** §Heartbeat schema (Worker grep `## Heartbeat schema`).

**Thay bằng / Thêm:** append paragraph after "Schema versioning" section:
```markdown
**Retry semantics (Phase 2.2):** when `[retry]` config is present and a task fires multiple times in one `advisory-cron run` invocation, EACH attempt produces ONE heartbeat JSONL line (with its own `ts`, `exit_code`, `duration_ms`). The schema does NOT carry a `retry_attempt` field — Phase 2.2 explicitly preserves the Phase 1.4 schema. `advisory-cron status --last N` naturally shows the per-attempt trail.
```

**Tìm change 4:** §Config schema TOML block (Worker grep `\[heartbeat\]`).

**Thay bằng / Thêm:** append `[retry]` block to the TOML example after `[alert.telegram]`:
```toml
# (Phase 2.2 — optional)
[retry]
max_attempts = 3        # 1 = no retry; ≥2 = retry up to (max_attempts - 1) times after initial failure
backoff_secs = 30       # seconds to sleep between attempts (capped at 3600 by validate)
```

**Also Tìm:** §Config schema "Field reference" table.

**Thay bằng / Thêm:** append 2 rows:
```markdown
| `[retry]` | `max_attempts` | `u32` | yes (if block present) | Max fire attempts per `run` invocation (≥1) | — |
| `[retry]` | `backoff_secs` | `u64` | yes (if block present) | Seconds between attempts (0..=3600) | — |
```

**Tìm change 5:** §Error handling + alerting section (Worker grep `## Error handling`).

**Thay bằng / Thêm:** append new subsection after the Phase 2.1 alert paragraph:
```markdown
### Retry policy (Phase 2.2 — P009)

When `[retry]` block is configured, `core::run::run` wraps `runner::fire_task` in a bounded loop:

- Up to `max_attempts` total fires per `advisory-cron run` invocation
- `tokio::time::sleep(backoff_secs)` between attempts (skip before first, skip after last)
- `is_retryable(exit_code)`: retryable iff `exit_code ∈ 1..=127`. NOT retryable: `0` (success), `≥128` (signal-killed), `-1` (spawn-failure sentinel per INV-14)
- 1 heartbeat JSONL line per attempt (schema unchanged from Phase 1.4)
- Telegram alert fires AT MOST ONCE per invocation — after the loop, gated on final `exit_code != 0`. Successful retry → zero alerts. Exhausted retries → one alert. Signal-killed → one alert (immediate, no retry).

INV-20 enforces all four rules: bounded attempts, backoff respected, signal-exits not retried, single-alert-per-invocation.

When `[retry]` block is absent, behavior is Phase 2.1 single-fire (1 attempt, alert on fail) — backwards-compat preserved via `unwrap_or((1, 0))` default.
```

**Tìm change 6:** §Phase status section (Worker grep `## Phase status` → `Phase 2.2`).

**Thay bằng / Thêm:** update Phase 2 line:
```markdown
- 🚧 **Phase 2** — In progress. Phase 2.1 (Telegram alert) shipped per P008. Phase 2.2 (retry policy) shipped per P009 (`is_retryable` private fn + retry loop in `core/run.rs`; 1 heartbeat per attempt schema preserved; alert moved OUTSIDE loop per INV-20 single-alert-per-invocation; `[retry]` opt-in config block). Phase 2.3 (state recovery) pending.
```

---

### Task 7: Update `README.md` — brief Phase 2.2 mention + `[retry]` config snippet

**File:** `README.md`

**Tìm:** existing Phase 2.1 alert section (Worker grep `Phase 2.1\|alert.telegram` in README.md).

**Thay bằng / Thêm:** append a Phase 2.2 paragraph + config block after the existing alert snippet:
```markdown
### Phase 2.2 — Retry policy (opt-in)

When a task fails with a transient error (network blip, rate limit, etc.), advisory-cron can re-fire it before alerting. Add `[retry]` to your config:

```toml
[retry]
max_attempts = 3        # total fires per run; 1 = no retry
backoff_secs = 30       # seconds between attempts (0..=3600)
```

- Each attempt writes 1 heartbeat line (so `advisory-cron status` shows the full trail)
- Telegram alert fires AT MOST ONCE per `run` invocation (no per-attempt spam)
- Exit codes 1-127 are retryable; signal-killed (≥128) and spawn-failures (-1) are NOT retried
- Without a `[retry]` block, behavior is single-fire (Phase 2.1 baseline)
```

---

### Task 8: Update `docs/CHANGELOG.md` — P009 entry

**File:** `docs/CHANGELOG.md`

**Tìm:** top of file, immediately after the `---` separator following the header (insert as newest entry above P008).

**Thay bằng / Thêm:**
```markdown
## 2026-MM-DD — P009: Phase 2.2 — Retry policy

**Phiếu:** P009 (Tầng 1 — `[retry]` config block, retry loop in `core::run::run`, INV-20 appended, heartbeat schema preserved, alert call moved OUTSIDE loop)

**Config schema (src/config.rs):**
- Added `RetryConfig { max_attempts: u32, backoff_secs: u64 }`.
- `Config` gains `#[serde(default)] pub retry: Option<RetryConfig>` — old configs (Phase 1 + P008) without `[retry]` block deserialize as `None` (backwards-compat preserved).
- `Config::validate()` extended: `max_attempts ≥ 1`, `backoff_secs ≤ 3600` (sanity cap).
- `Config::default_for_home` does NOT include retry block — opt-in.

**Wiring (src/core/run.rs):**
- New private fn `is_retryable(exit_code: i32) -> bool` — `(1..=127).contains(&exit_code)` per BACKLOG Phase 2.2 spec.
- Retry loop wraps `runner::fire_task` + `heartbeat::append` for up to `max_attempts` iterations.
- Between attempts: `tokio::time::sleep(Duration::from_secs(backoff_secs)).await`.
- Loop exits early on: success (exit 0), non-retryable (signal-killed ≥128 or spawn-fail -1), exhausted attempts.
- P008 alert block MOVED from inside single-fire body to AFTER loop — fires AT MOST ONCE per `run` invocation, gated on final `exit_code != 0`. INV-20 single-alert-per-invocation invariant.
- When `[retry]` absent: `unwrap_or((1, 0))` → single-fire Phase 2.1 behavior preserved (1 attempt, alert on fail).

**Heartbeat schema unchanged:**
- 1 JSONL line per attempt (3 retries = 3 lines). `HeartbeatRecord` struct in `src/heartbeat.rs` untouched (verified via `git diff src/heartbeat.rs` empty). `advisory-cron status --last N` naturally shows per-attempt trail.

**INVARIANTS.md:**
- INV-20 appended (4 sub-rules: bounded attempts DOS prevention, backoff respected no busy loop, signal exits not retried, single alert per invocation).

**No new dep:**
- `tokio::time::sleep` + `std::time::Duration` already pulled by `tokio` feature `time` (P008 Anchor #6).
- `wiremock` dev-dep from P008 sufficient for integration test.

**Tests (+N new, total ~12N):**
- `src/config.rs` unit tests (5): load_without_retry_block (backwards-compat), load_with_retry_block, validate_retry_zero_attempts, validate_retry_excessive_backoff, load_with_retry_and_alert.
- `src/core/run.rs` unit tests (8): is_retryable boundaries (exit 1, 127, 0, 128, 130, 137, 143, -1).
- `tests/cli_run_retry.rs` integration (4): retry_succeeds_on_attempt_2_no_alert, retry_exhausts_max_attempts_single_alert, signal_exit_not_retried_single_attempt, no_retry_block_preserves_phase21_single_fire.
- All P008 + Phase 1 baseline tests preserved (116 → ~133).

**Docs updated (Tầng 1):**
- `docs/ARCHITECTURE.md` — §Modules `core/run.rs` row Purpose updated; comment after table updated (no new `src/retry.rs` module); §Heartbeat schema retry semantics paragraph added; §Config schema TOML block + Field reference rows for `[retry]`; new §Error handling subsection "Retry policy (Phase 2.2)"; §Phase status Phase 2.2 shipped.
- `docs/security/INVARIANTS.md` — INV-20 appended.
- `README.md` — Phase 2.2 section with `[retry]` config snippet.

**Acceptance (all ✅):**
- `cargo build --release` — zero warnings, binary ≤7MB budget
- `cargo test --all` — ~133/133 pass (116 baseline + 17 new)
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo fmt --check` — no diff
- `git diff src/cli/mod.rs` — empty (Constraint #1 re-instated, honored)
- `git diff src/heartbeat.rs` — empty (schema preserved)
- `git diff src/alert.rs` — empty (Constraint #11 alert.rs env-free preserved)
- `git diff src/runner.rs` — empty (runner stays single-fire primitive)

---
```

**Lưu ý:**
- Worker fills `2026-MM-DD` with actual ship date.
- Worker fills exact test count after running `cargo test --all`.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/config.rs` | Task 1: add `RetryConfig` struct, `Config::retry` field, validation |
| `src/core/run.rs` | Task 2: add `is_retryable` private fn; Task 3: rewire body with retry loop + move alert outside loop |
| `docs/security/INVARIANTS.md` | Task 4: append INV-20 |
| `tests/cli_run_retry.rs` | Task 5: NEW integration test file |
| `docs/ARCHITECTURE.md` | Task 6: §Modules note + §Heartbeat schema + §Config schema + §Error handling + §Phase status |
| `README.md` | Task 7: Phase 2.2 section with `[retry]` snippet |
| `docs/CHANGELOG.md` | Task 8: P009 entry |
| `docs/discoveries/P009.md` | Discovery Report (created at end per CLAUDE.md DOD) |
| `docs/DISCOVERIES.md` | 1-line index entry (newest at top, prepended above P008) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/cli/mod.rs` | `git diff` empty (CONSTRAINT #1 — newtype dispatch unchanged) |
| `src/heartbeat.rs` | `git diff` empty (HeartbeatRecord schema preserved — Architect decision §Giải pháp item 3) |
| `src/alert.rs` | `git diff` empty (alert module untouched — Constraint #11 env-free invariant preserved) |
| `src/runner.rs` | `git diff` empty (runner stays single-fire primitive — Architect decision §Giải pháp item 2) |
| `src/launchd.rs` | `git diff` empty (no launchd change) |
| `src/main.rs` | `git diff` empty (no new mod declarations — retry lives in existing `core/run.rs`) |
| `src/core/init.rs`, `register.rs`, `unregister.rs`, `status.rs` | `git diff` empty (only `core/run.rs` needs retry wire) |
| `src/mcp/tools.rs` | `git diff` empty (MCP `run` tool gets retry automatically via shared `core::run::run` — confirms layering invariant) |
| `Cargo.toml` | `git diff` empty (no new dep, no new dev-dep — `tokio time` + `wiremock` from P008 sufficient) |

---

## Luật chơi (Constraints)

1. **CONSTRAINT #1 re-instated (post-P006 V2, honored P008):** `git diff src/cli/mod.rs` MUST be empty. Retry is internal to `core::run::run` flow — no new CLI subcommand, no new CLI flag, no dispatch edit. Worker confirms with `git diff src/cli/mod.rs` returning empty before commit.

2. **No new dep:** `Cargo.toml` `[dependencies]` and `[dev-dependencies]` MUST be unchanged. `tokio::time::sleep` (feature `time`) + `wiremock` (dev-dep from P008) cover all P009 needs. If Worker thinks a new dep is needed → STOP, escalate per Hard Stops #2.

3. **No new module:** P009 does NOT create `src/retry.rs`. Architect decision per Giải pháp item 2 — retry is orchestration, lives in `core/run.rs` alongside the existing fire+heartbeat+alert pipeline. Hard Stops #1 applies: no new file in `src/` other than what phiếu explicitly creates (`tests/cli_run_retry.rs`).

4. **Heartbeat schema preserved (CONSTRAINT against P008-era expansion):** `git diff src/heartbeat.rs` MUST be empty. `HeartbeatRecord` struct shape (ts, label, exit_code, duration_ms, stdout_tail, stderr_tail) is the Phase 1.4 durable contract per ARCHITECTURE.md §Heartbeat schema. Adding `retry_attempt` field would be a Tầng 1 schema break — explicitly rejected here. Per-attempt trail is reconstructed by reading consecutive JSONL lines with same `label` close in time.

5. **`src/runner.rs` untouched:** `git diff src/runner.rs` MUST be empty. Runner is single-fire primitive; retry is orchestration. Worker confirms `runner::fire_task` is called inside the new loop, not modified.

6. **`src/alert.rs` env-free invariant (CONSTRAINT #11 from P008) PRESERVED:** `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` MUST return empty. P009 does not touch `src/alert.rs` — env var read stays at call site in `core/run.rs` (same line moved with the alert block from inside-loop to after-loop).

7. **Constraint #4 V2 internal-resolution preserved:** No config-path threading added. `config` is in scope in `core::run::run`; retry reads `config.retry` directly. No new env reads added to other `core::*` modules.

8. **Single-alert-per-invocation hard rule (INV-20 sub-rule 4):** Alert call site in `core/run.rs` MUST be exactly 1, located AFTER the retry loop, gated on `final_exit_code != 0`. Worker confirms with `grep -c "send_with_base" src/core/run.rs` returning exactly 1.

9. **No `unsafe { }`:** Per CLAUDE.md Hard Stop #7 + INV-6. Retry loop is pure safe Rust (tokio sleep, fire_task call, exit_code arithmetic).

10. **No new env var reads outside `core/run.rs`:** Retry adds zero env reads (config-driven). The ADVISORY_CRON_TG_API_BASE read from P008 is preserved as-is at its existing call site. `grep "std::env::var" src/` outside `core/run.rs` and existing-allowed sites MUST be unchanged.

11. **`max_attempts.max(1)` defense-in-depth floor:** even though `Config::validate` rejects `max_attempts < 1`, the runtime floor in Task 3 snippet is REQUIRED — defense against validate being bypassed (e.g. config struct constructed directly in test code without `Config::validate` call).

12. **TWO-MATCH HEARTBEAT-COMPLETENESS INVARIANT (P009 V2, INV-15 adjacent):** Worker MUST preserve the existing two-match structure (`match &fire_result` borrow for HeartbeatRecord build → `heartbeat::append` → `match fire_result` consume for tuple extract) per loop iteration. Spawn-fail iterations MUST still append a heartbeat (with synthesized `exit_code = -1`). `grep -c "heartbeat::append" src/core/run.rs` MUST be exactly `1` after edit (single call site inside loop body, between the two matches). DO NOT short-circuit on `fire_result.is_err()` to skip heartbeat — would silently lose heartbeat data on spawn-fail retries.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings, binary ≤7MB
- [ ] `cargo test --all` — all pass (116 P008 baseline + ~17 new = ~133)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `git diff src/cli/mod.rs` — empty (Constraint #1)
- [ ] `git diff src/heartbeat.rs` — empty (Constraint #4)
- [ ] `git diff src/runner.rs` — empty (Constraint #5)
- [ ] `git diff src/alert.rs` — empty (Constraint #6 P008-env-free preserved)
- [ ] `git diff Cargo.toml` — empty (Constraint #2)
- [ ] `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` — empty (Constraint #6)
- [ ] `grep -c "send_with_base" src/core/run.rs` — exactly `1` (Constraint #8 single-alert-per-invocation)
- [ ] `grep -c "heartbeat::append" src/core/run.rs` — exactly `1` (Constraint #12 two-match heartbeat-completeness invariant — single call site inside loop, between the two matches)

### Manual Testing
- [ ] `mkdir -p /tmp/p009 && cd /tmp/p009 && advisory-cron init` — generates default config (no `[retry]` block)
- [ ] Manually add `[retry]` block to config, set `command="false"` `args=[]` `max_attempts=2` `backoff_secs=1`, then `advisory-cron run --config /tmp/p009/config.toml` — observe ~1s pause between fires; check heartbeat shows 2 JSONL lines (both exit_code=1); confirm 1 alert sent (if `[alert.telegram]` configured with real bot)
- [ ] Same config but `command="true"` — observe 1 heartbeat line (exit 0, no retry needed), no alert
- [ ] Same config but `command="bash" args=["-c","exit 143"]` — observe 1 heartbeat line (exit 143, signal boundary, no retry), 1 alert
- [ ] Remove `[retry]` block entirely — `advisory-cron run` falls back to single-fire (Phase 2.1 behavior preserved)

### Regression
- [ ] `cargo test --test cli_run_alert` (P008 integration tests) — all pass (alert call site moved but behavior identical for single-attempt case)
- [ ] `cargo test --test cli_run` (P004 integration tests) — all pass (single-fire behavior preserved when no `[retry]`)
- [ ] `cargo test --test cli_mcp` (P006 MCP tests) — all pass (MCP `run` tool inherits retry via shared `core::run::run`)
- [ ] `advisory-cron mcp` smoke test (`echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | advisory-cron mcp`) — still returns serverInfo (no MCP regression)

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P009 entry at top
- [ ] `docs/ARCHITECTURE.md` — 6 changes per Task 6 (§Modules row, table comment, §Heartbeat schema retry para, §Config schema TOML+table, §Error handling new subsection, §Phase status 2.2)
- [ ] `docs/security/INVARIANTS.md` — INV-20 appended
- [ ] `README.md` — Phase 2.2 section with `[retry]` snippet
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P009.md` — full report written per CLAUDE.md DOD template
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top, above P008 line)
- [ ] Sub-mechanism A-E Verification Trace table filled
- [ ] Discovery includes: actual final test count, whether Worker chose `bool` vs `u32` for `heartbeat_appended` field (Tầng 2 decision per Task 3 Lưu ý), any anchor sai lệch found at Task 0, confirmation of all 11 Constraints
