# PHIẾU P008: Telegram alert on fail (Phase 2.1)

> **Loại:** Feature
> **Tầng:** 1
> **Ưu tiên:** P1
> **Ảnh hưởng:** `src/alert.rs` (NEW), `src/config.rs`, `src/core/run.rs`, `Cargo.toml` (dev-dep only), `docs/ARCHITECTURE.md`, `docs/security/INVARIANTS.md`, `README.md`
> **Dependency:** P007 (Phase 1 ship). Pre-req `~/.advisory-cron-secrets.env` chmod 600 đã sẵn (bot `@chiha_alert_bot`, chat_id `1184530337`, end-to-end test 2026-05-27 message_id=21).

---

## Context

### Vấn đề hiện tại

BACKLOG.md "Phase 2 — Next sprint" item 2.1: Telegram bot webhook POST on fail. Phase 1 đã ship: heartbeat ghi exit_code != 0 vào JSONL, nhưng Sếp không biết task fail trừ khi tự `advisory-cron status`. PROJECT.md §Hard line #5 "Failure mode = noisy" yêu cầu surface lên điện thoại Sếp. Phase 2.1 thêm Telegram alert best-effort khi `core::run::run` thấy exit_code != 0.

Sếp promote Phase 2.1 vào active sprint hôm nay (autonomous cuốn chiếu 10 phiếu — Phase 1 + Phase 2). Secrets đã staged ở `~/.advisory-cron-secrets.env` (TG_BOT_TOKEN, TG_CHAT_ID, TG_BOT_USERNAME), chmod 600, e2e verified.

### Giải pháp

1. **`src/alert.rs` (NEW module)** — `TelegramAlert { bot_token, chat_id }` struct + `from_config` constructor + `async fn send(&self, message: &str) -> Result<()>`. POST `https://api.telegram.org/bot<token>/sendMessage` qua `reqwest` (đã có trong deps, rustls-tls + json features), wrap `tokio::time::timeout(Duration::from_secs(10), ...)`. Trả lỗi qua `Result` — caller quyết log-and-continue vs fail. **`src/alert.rs` is env-free** — no `std::env::var` reads inside this module. API base override (test seam) is read at the call site in `core::run::run` and passed in via `send_with_base(base, msg)`.
2. **`src/config.rs` extend** — `AlertConfig { telegram: Option<TelegramConfig> }`, `TelegramConfig { bot_token: String, chat_id: String, bot_token_file: Option<PathBuf> }`. Add `pub alert: Option<AlertConfig>` vào `Config`. Validation: nếu `[alert.telegram]` có thì PHẢI có `chat_id` AND (`bot_token` HOẶC `bot_token_file`) — không cả hai, không thiếu.
3. **Secrets resolution — Option C (Architect quyết, xem Heads-up #3)**: config dùng `bot_token_file = "~/.advisory-cron-secrets.env"` — `TelegramAlert::from_config` đọc file `KEY=VAL` lines, extract `TG_BOT_TOKEN=...`. Hoặc inline `bot_token = "..."` cho test/dev (config file chmod 600 trách nhiệm Sếp). KHÔNG implement env var interpolation `${VAR}` (Option B) — too magic, không cần.
4. **`src/core/run.rs` wire** — AFTER the `match fire_result` block (so local bindings `exit_code`, `stderr_tail`, `duration_ms` are in scope), BEFORE `Ok(RunOutput ...)` return: NẾU `exit_code != 0` AND `config.alert.telegram` Some → build message + read optional `ADVISORY_CRON_TG_API_BASE` env var at call site → spawn alert send via `send_with_base(base, &msg)` wrapped trong existing `tokio::time::timeout` inside `alert.rs` + `tracing::warn!` khi alert fail (KHÔNG bubble error — best-effort per Hard line #1 acceptance "Alert failure does NOT fail the task"). **Env var read happens at the call site, NOT inside `alert.rs`** — keeps `alert.rs` env-free for testability (per Worker Turn 1 design recommendation, Architect ACCEPT).
5. **Tests**: dev-dep `wiremock = "0.6"` (Architect quyết, xem Heads-up #2). Unit test `TelegramAlert::send_with_base` happy path + 500 retry-not-implemented path + timeout path. `from_config` cases: None / Some(inline) / Some(file). Integration test `tests/cli_run_alert.rs` (subprocess `advisory-cron run` with `false` command + mock TG endpoint via wiremock + `ADVISORY_CRON_TG_API_BASE` env var set to mock URL — verify POST body shape).
6. **Docs**: INV-19 (Telegram HTTP boundary), ARCHITECTURE.md §Alert section + §Phase status, CHANGELOG, README brief snippet, Phase 2.1 ship.

### Scope

- **CHỈ sửa:**
  - `src/alert.rs` (NEW)
  - `src/config.rs` (extend — add `AlertConfig`, `TelegramConfig`, `Config::alert` field, validation)
  - `src/core/run.rs` (extend — alert call after match-fire_result block, before Ok return)
  - `src/main.rs` (declare `mod alert;`)
  - `Cargo.toml` ([dev-dependencies] add `wiremock`)
  - `docs/security/INVARIANTS.md` (append INV-19)
  - `docs/ARCHITECTURE.md` (§Modules row for alert.rs, §Error handling + alerting expand Phase 2 section, §Config schema add `[alert.telegram]` block, §Phase status update Phase 2.1)
  - `docs/CHANGELOG.md` (P008 entry)
  - `README.md` (brief Phase 2 mention + config snippet)
  - `tests/cli_run_alert.rs` (NEW integration test)

- **KHÔNG sửa:**
  - `src/cli/*` (alert là internal flow của `core::run::run` — không có subcommand mới, không có CLI flag mới)
  - `src/cli/mod.rs` (CONSTRAINT #1 re-instated post-P006 V2 — KHÔNG touch dispatch)
  - `src/mcp/*` (MCP `run` tool tự động hưởng alert vì gọi `core::run::run` — không touch tool schema)
  - `src/launchd.rs`, `src/runner.rs`, `src/heartbeat.rs` (alert hook ở `core/run.rs`, không xuyên vào các module low-level)
  - `src/core/init.rs`, `src/core/register.rs`, `src/core/unregister.rs`, `src/core/status.rs` (chỉ `core/run.rs` cần alert wire)
  - Cargo.toml `[dependencies]` (KHÔNG add runtime dep — reqwest + tokio + serde đã đủ)

### Skills consulted

*(none — Architect sourced từ docs hiện có + Sếp brief.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Architect KHÔNG có Bash/Grep — anchors sourced từ ARCHITECTURE.md, INVARIANTS.md, DISCOVERIES.md, Sếp brief. Worker BẮT BUỘC verify thực tế.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/core/run.rs` tồn tại + chứa `async fn run(args: RunArgs) -> Result<RunOutput>` resolving env nội tại | `grep -n "pub async fn run" src/core/run.rs` | `[unverified]` | ✅ Confirmed `src/core/run.rs:32` |
| 2 | `src/core/run.rs` gọi `heartbeat::append` AFTER `runner::fire_task` returns | `grep -n "heartbeat::append" src/core/run.rs` | `[unverified]` | ✅ Confirmed at line 74. V2 update: alert injection point moved AFTER `match fire_result` block (after line 90), before `Ok(RunOutput ...)`. At that point `exit_code`, `stderr_tail`, `duration_ms`, `label` are all in scope. |
| 3 | `Config` struct ở `src/config.rs` chưa có field `alert` | `grep -n "pub alert" src/config.rs` → expect empty | `[unverified]` | ✅ Empty — no conflict |
| 4 | `Config` đã derive `Serialize + Deserialize` (cho serde) | `grep -n "#\[derive" src/config.rs` quanh struct Config | `[unverified]` | ✅ Confirmed `src/config.rs:30` |
| 5 | `Cargo.toml` có `reqwest` với features `rustls-tls`, `json`, `default-features = false` | `grep -A2 'reqwest' Cargo.toml` | `[verified]` per Sếp brief tech stack section CLAUDE.md | ✅ Confirmed `Cargo.toml:23` |
| 6 | `Cargo.toml` có `tokio` với feature `time` (cho `tokio::time::timeout`) | `grep -A2 'tokio' Cargo.toml` | `[verified]` per CLAUDE.md tech stack (rt, macros, process, time, fs, io-std) | ✅ Confirmed `Cargo.toml:18` |
| 7 | INVARIANTS.md max INV currently = 18 (slot for INV-19 free) | `grep -c "^### INV-" docs/security/INVARIANTS.md` | `[verified]` | ✅ Confirmed 18 total |
| 8 | INV-2 generic baseline "external service call → timeout + error handling" đã active | `grep -A3 "INV-2" docs/security/INVARIANTS.md` | `[verified]` | ✅ Confirmed `INVARIANTS.md:18-25` |
| 9 | Heartbeat schema (HeartbeatRecord) chứa `exit_code`, `label`, `duration_ms`, `stderr_tail` — đủ để Architect format alert message | docs/ARCHITECTURE.md §Heartbeat schema | `[verified]` | ✅ All fields confirmed in `src/heartbeat.rs:18-24` and ARCHITECTURE.md:260 |
| 10 | `~/.advisory-cron-secrets.env` chmod 600, format `KEY=VAL` lines, chứa TG_BOT_TOKEN + TG_CHAT_ID + TG_BOT_USERNAME | Sếp brief — verified end-to-end 2026-05-27 msg_id=21 | `[verified]` | ✅ Confirmed `-rw-------` (chmod 600) live |
| 11 | `wiremock = "0.6"` crate là dev-dep choice — Anthropic-friendly async, no native deps | crates.io / wiremock-rs docs | `[needs Worker verify]` | ✅ `cargo search wiremock` → `wiremock = "0.6.5"`. `"0.6"` resolves to 0.6.5 via semver. |
| 12 | `core::run::run` returns `Result<RunOutput>` — alert call inserted BEFORE return Ok, AFTER match fire_result block | docs/ARCHITECTURE.md §Modules row `core/run.rs` | `[unverified]` | ✅ Return type ✅ (`src/core/run.rs:32,92`). V2 insertion location: after `match fire_result` block (~line 90), before `Ok(RunOutput ...)` at line 92. |
| 13 | CONSTRAINT #1 re-instated post-P006 V2: KHÔNG touch `src/cli/mod.rs` | Sếp brief Heads-up #4 + docs/discoveries/P006.md | `[unverified]` | ✅ Confirmed. `mod alert;` goes in `main.rs`. `cli/mod.rs` untouched. |
| 14 | CONSTRAINT #4: "all 5 core fns resolve env internally" — Architect honors bằng cách KHÔNG add config-path threading; alert reads secrets file path từ config struct (path is config-data, not env) | docs/ARCHITECTURE.md §Modules "V2 internal-resolution pattern (P006)" | `[verified]` | ✅ Confirmed `ARCHITECTURE.md:62` |
| 15 | `src/main.rs` declares modules via `mod alert;` style (need to add new line) | typical Rust binary layout — Worker check | `[needs Worker verify]` | ✅ Confirmed. `main.rs:6-12` uses flat `mod <name>;` style. `mod alert;` addition is straightforward. |

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND). Cap = 3 turns.

**Phiếu version:** V2 (Architect responded to Turn 1: O1.1 ACCEPT Option A + env-var-at-call-site ACCEPT)

### Turn 1 — Worker Challenge — 2026-05-27

**Anchor verification:**

- Anchor #1 ✅ — `pub async fn run(args: RunArgs) -> Result<RunOutput>` at `src/core/run.rs:32`
- Anchor #2 ⚠️ — `heartbeat::append` at `src/core/run.rs:74`. CONFIRMED injection point exists. BUT see Objection O1.1 below — variable names `exit_code`, `stderr_tail`, `duration_ms` do NOT exist at line 74. They are declared at line 77 inside the `match fire_result` block. The `record` struct (built lines 54-71) IS in scope at line 74 and contains all the same values. Alert snippet must read from `record` or Worker restructures variable ordering.
- Anchor #3 ✅ — `pub alert` absent from `src/config.rs` (grep returned empty — no conflict)
- Anchor #4 ✅ — `Config` derives `Serialize + Deserialize` at `src/config.rs:30`
- Anchor #5 ✅ — `reqwest = { version = "0.12", features = ["rustls-tls", "json"], default-features = false }` at `Cargo.toml:23`
- Anchor #6 ✅ — `tokio = { ..., features = [..., "time", ...] }` confirmed at `Cargo.toml:18`
- Anchor #7 ✅ — `grep -c "^### INV-"` → 18. INV-19 slot free.
- Anchor #8 ✅ — INV-2 confirmed active at `docs/security/INVARIANTS.md:18-25`
- Anchor #9 ✅ — `HeartbeatRecord` fields confirmed at `src/heartbeat.rs:18-24`: `label: String`, `exit_code: i32`, `duration_ms: u64`, `stdout_tail: String`, `stderr_tail: String`. ARCHITECTURE.md line 260 example also matches.
- Anchor #10 ✅ — `~/.advisory-cron-secrets.env` exists, permissions `-rw-------` (chmod 600), verified live.
- Anchor #11 ✅ — `cargo search wiremock` returns `wiremock = "0.6.5"`. Latest stable is **0.6.5** (not 0.6.0). Phiếu specifies `"0.6"` which resolves to 0.6.5 via semver — correct.
- Anchor #12 ✅ — `pub async fn run` at line 32, `Ok(RunOutput` at line 92. Insertion point BEFORE `Ok(RunOutput` is achievable. (Variable name issue noted in Anchor #2 / O1.1 below.)
- Anchor #13 ✅ — `src/main.rs` uses flat `mod <name>;` style: lines 6-12 (`mod cli;`, `mod config;`, `mod core;`, `mod heartbeat;`, `mod launchd;`, `mod mcp;`, `mod runner;`). Adding `mod alert;` at line 13 is straightforward.
- Anchor #14 ✅ (docs-only) — ARCHITECTURE.md line 62 confirms V2 internal-resolution pattern: no config-path threading.
- Anchor #15 ✅ — confirmed by Anchor #13 result. `mod alert;` will work.

**Tempfile dev-dep confirmed:** `tempfile = "3"` already in `[dev-dependencies]` at `Cargo.toml:28`.

---

**Objections (Tầng 1 — code reality differs from phiếu assumption):**

**[O1.1] — CRITICAL: Task 3 alert snippet uses variables that don't exist yet at the injection point.**

Evidence: `src/core/run.rs:74-90`

```
74:    let heartbeat_appended = heartbeat::append(&config.heartbeat.log_path, &record).is_ok();
75:
76:    // 6. Build RunOutput.
77:    let (exit_code, stdout_tail, stderr_tail, duration_ms) = match fire_result {
...
```

The Task 3 snippet references `&label`, `exit_code`, `duration_ms`, `&stderr_tail`. At line 74 (right after `heartbeat::append`), the local bindings `exit_code`, `stderr_tail`, `duration_ms` do NOT exist yet — they're first bound on line 77 inside the `match fire_result` block. `label` IS in scope (line 44). The `record` struct IS in scope (line 54) and already contains `record.exit_code`, `record.stderr_tail`, `record.duration_ms`.

This is a Tầng 1 objection because it affects insertion location and variable naming in Task 3 — affects how Worker writes the wiring code.

**Two clean alternatives:**

A. (Recommended) Insert alert block AFTER the `match fire_result` block (after line 90, before `Ok(RunOutput ...)`). At that point `exit_code`, `stderr_tail`, `duration_ms`, `label` are all bound. Task 3 snippet is used as-is verbatim. `fire_result` is consumed by then — no borrow conflict (the match on line 77 consumes it). This is the minimal change.

B. Insert alert block BEFORE the `match fire_result` block (between lines 74 and 77), reading from `record.exit_code`, `record.stderr_tail`, `record.duration_ms`, `&label`. Requires renaming variables in Task 3 snippet to `record.exit_code` etc. Slightly more verbose but keeps alert before "build output" step. Worker can self-decide between A and B as Tầng 2 IF Architect agrees both are acceptable.

Architect: confirm A or B (or propose C). If A, Task 3 snippet works verbatim — no other changes needed.

---

**Heads-up resolutions — Worker position:**

1. **INV-19 + 10s timeout double-guard:** Worker agrees. `reqwest::Client::builder().timeout(Duration)` + outer `tokio::time::timeout` guards against both connection-established-but-body-hang AND pre-connect/DNS hang. 10s reasonable for solo tool. No objection.

2. **wiremock 0.6 vs httpmock:** Worker agrees with wiremock 0.6.5. Cargo search confirms it's current, pure-async, no native deps. `cargo search` shows no httpmock dependency concerns. No objection.

3. **Option C (bot_token_file, no env var interpolation):** Worker agrees. `${VAR}` interpolation would require shell-like parsing — extra complexity, potential injection vector. `bot_token_file` pointing to a `KEY=VAL` file is simpler and already e2e verified by Sếp. No objection.

4. **CONSTRAINT #1 (mod.rs untouched):** Worker confirms achievable. `mod alert;` goes in `src/main.rs` (line 13 area), NOT in `src/cli/mod.rs`. Alert is `crate::alert::TelegramAlert` — called from `src/core/run.rs` directly as `crate::alert::...`. `cli/mod.rs` dispatch is untouched. Zero edits to `src/cli/mod.rs`. Confirmed.

5. **CONSTRAINT #4 (alert reads config.alert directly):** Worker agrees. `config` is already in scope in `core::run::run`. No threading needed. `expand_home` inline in `alert.rs` (not borrowing from `core/config_path`) is correct for module isolation. No objection.

6. **API base override (env var `ADVISORY_CRON_TG_API_BASE`):** Worker position — prefer env var seam over `TelegramConfig::api_base` field, for same reason Architect stated: no config surface added, test-only seam is invisible to users. However, env var should be read at `send_with_base` call site in `core/run.rs` (or in `TelegramAlert::from_config`), NOT inside `TelegramAlert::send`. This way `send` stays pure (no env reads inside library fn), and the caller decides which base to use. Worker recommends: read `std::env::var("ADVISORY_CRON_TG_API_BASE")` in `core/run.rs` Task 3 wiring (after O1.1 insertion point is resolved) → pass to `alert.send_with_base(base, &msg)`. This keeps `src/alert.rs` testable without env coupling. Architect confirm or override.

---

**Summary:** 1 Tầng 1 objection (O1.1 — variable availability at injection point). Architect must choose A or B. All other heads-up resolutions accepted. Wiremock version confirmed 0.6.5 (phiếu `"0.6"` resolves correctly).

**Status:** ✅ RESPONDED IN TURN 1 ARCHITECT RESPONSE BELOW

---

### Turn 1 — Architect Response (phiếu V2) — 2026-05-27

**[O1.1] → ACCEPT Option A** (insert alert block AFTER `match fire_result` block, before `Ok(RunOutput ...)`)

- **Rationale:** Worker recommend + Quản đốc lean + minimal diff. Task 3 snippet works verbatim — no rename required. `fire_result` is already consumed by line 77 match, so no borrow conflict. `label`, `exit_code`, `duration_ms`, `stderr_tail` all bound at insertion point. Option B would require renaming 3 variable references to `record.*` — strictly more changes for zero benefit (alert-before vs alert-after-output-build has no semantic impact; `RunOutput` is local, not yet returned).
- **Phiếu changes:**
  - Task 3 "Tìm" updated: locate insertion point as "AFTER the `match fire_result` block (around line 90), BEFORE `Ok(RunOutput { ... })` return". Worker grep `^    Ok(RunOutput` to anchor.
  - Task 3 snippet body unchanged (uses `&label`, `exit_code`, `duration_ms`, `&stderr_tail` — all in scope post-match).
  - Anchor #2 Kết quả updated: ✅ (insertion point shifted in V2, all vars in scope).
  - Anchor #12 Kết quả updated: ✅ (V2 location annotated).

**[API base override — env var at call site] → ACCEPT Worker recommendation**

- **Rationale:** `alert.rs` env-free = strictly more testable. `send` / `send_with_base` become pure functions of their inputs — no hidden global state to set up in unit tests. Env var read is a 2-line block at `core::run::run`, trivially auditable. This pattern matches CONSTRAINT #4 spirit (env resolution happens at one layer, not threaded silently through library calls).
- **Phiếu changes:**
  - Task 3 snippet updated: insert env var read at call site:
    ```rust
    let api_base = std::env::var("ADVISORY_CRON_TG_API_BASE")
        .unwrap_or_else(|_| "https://api.telegram.org".to_string());
    // ... then ...
    if let Err(e) = alert.send_with_base(&api_base, &msg).await {
    ```
    (No call to `alert.send(...)` from `core/run.rs` — always go through `send_with_base` with explicit base. `send` remains as a thin convenience wrapper for any future non-test call site.)
  - Task 1 `alert.rs` snippet updated: REMOVE any env-reading code from `alert.rs`. `send(msg)` simply forwards to `send_with_base(TELEGRAM_API_BASE, msg)` — pure delegation, no `std::env::var`. (Architect Task 1 original draft already had this shape; this change is mostly clarifying / re-stating, no code edit beyond removing the inline doc comment that hinted at builder/env alternative.)
  - Task 6 integration test updated: test sets `ADVISORY_CRON_TG_API_BASE=<mock_server.uri()>` via `Command::env(...)` BEFORE spawning the subprocess. Subprocess inherits the env var → `core::run::run` reads it → passes mock URL to `send_with_base`.
  - INV-19 (Task 4) wording reviewed: section §Trust boundary / §Implementation already say "env var read at call site, `alert.rs` stays env-free" implicitly. V2 makes this explicit — adds 1 sentence: "The `ADVISORY_CRON_TG_API_BASE` env var (test seam) MUST be read at the call site in `src/core/run.rs`, NEVER inside `src/alert.rs`. This keeps `alert.rs` a pure function of its inputs and unit-testable without env setup."

**Heads-up resolutions [1-5]:** All Worker AGREE positions accepted. No phiếu changes — phiếu V1 already encoded these directions; Worker just re-confirmed.

**Phiếu version bump:** V1 → V2.

**Status:** ✅ RESPONDED — phiếu V2 ready. All objections resolved. No DEFER to Sếp. Quản đốc may proceed to spawn Worker (CHALLENGE) again for V2 verification, or — given the changes are narrow (insertion location shift + env-read site clarification, both explicitly Worker-recommended) — proceed directly to approval gate / EXECUTE per orchestrator discretion.

### Turn 2 — Worker Challenge — 2026-05-27

**Worker accepted V2 — no challenges.**

**V2 change verification (5/5 confirmed):**

- [V2.1] O1.1 Option A (alert AFTER match fire_result): Task 3 header reads "V2 — Option A"; insertion location documented as "between end of `match fire_result` block (~line 90) and `Ok(RunOutput ...)` return (line 92)"; snippet uses `exit_code`, `stderr_tail`, `duration_ms`, `label` verbatim — all bound post-match. ✅
- [V2.2] Env var at call site in core/run.rs: Task 3 snippet contains `let api_base = std::env::var("ADVISORY_CRON_TG_API_BASE").unwrap_or_else(...)` in `core/run.rs`; `send_with_base` docstring says "No env reads inside this function". ✅
- [V2.3] alert.rs env-free contract docstring: Module-level doc at top of Task 1 snippet explicitly states "**Env-free contract (V2 — per Worker Turn 1 recommendation):** This module MUST NOT call `std::env::var` for any reason." ✅
- [V2.4] send_with_base used, not send(): Task 3 snippet line `if let Err(e) = alert.send_with_base(&api_base, &msg).await`; Task 3 Notes section states "Production call site MUST go through `send_with_base(&api_base, &msg)`". ✅
- [V2.5] INV-19 + Constraint #11: INV-19 paragraph 4 explicitly forbids `ADVISORY_CRON_TG_API_BASE` inside `src/alert.rs`; Constraint #11 added with mandatory grep check `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` → expect empty; same grep in Automated nghiệm thu. ✅

**Anchor verification:** All 15 anchors already resolved in Turn 1 (anchors #1–#15 ✅). V2 changes did not introduce new anchors requiring re-verification.

**No new Tầng 1 objections.** Ready for Chủ nhà approval gate.

### Final consensus
- Phiếu version: V2
- Total turns: 2
- Approved: [date]

---

## Debug Log

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (Sub-mechanism A-E)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | N/A — no new launchd plist, alert hook fires from existing `run` path | — | | N/A |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test alert` (new unit tests) | targeted pass | | |
| B (capability) | `cargo test cli_run_alert` (integration) | pass | | |
| C (migration) | Schema change: `Config` gains optional `alert` field. Backwards-compat: old config without `[alert]` block must still load. Verify: `cargo test config_load_without_alert` | old configs deserialize OK | | |
| D (persistence) | `grep -l "INV-19" docs/security/INVARIANTS.md` | ≥1 hit | | |
| E (env drift) | `cargo update --dry-run` after adding `wiremock` dev-dep | no surprise major bump in existing deps | | |
| E (env drift) | `cargo build --release` clean target | exit 0, binary still ≤7MB | | |

---

## Nhiệm vụ

### Task 0 — Anchor verification (BẮT BUỘC TRƯỚC mọi Task khác)

**Mục đích:** Architect không có Bash/Grep — anchor table dựa docs. Worker chạy 15 grep commands trong table Verification Anchors, fill Kết quả column, escalate qua Debate Log nếu phát hiện sai lệch nghiêm trọng (e.g. `Config` đã có `alert` field — duplicate definition).

**Lệnh chạy** (anchors 1-15, theo thứ tự):
```bash
grep -n "pub async fn run" src/core/run.rs                  # #1
grep -n "heartbeat::append" src/core/run.rs                  # #2
grep -n "pub alert" src/config.rs                            # #3 expect empty
grep -n "#\[derive" src/config.rs | head -5                  # #4
grep -A2 'reqwest' Cargo.toml                                # #5
grep -A2 '^tokio' Cargo.toml                                 # #6
grep -c "^### INV-" docs/security/INVARIANTS.md              # #7 expect 18
grep -A3 "^### INV-2 " docs/security/INVARIANTS.md           # #8
grep -n "ts.*label.*exit_code" docs/ARCHITECTURE.md          # #9
ls -la ~/.advisory-cron-secrets.env                          # #10 expect -rw-------
cargo search wiremock | head -3                              # #11
grep -n "^pub async fn run\|^    Ok(RunOutput" src/core/run.rs # #12 — V2 insertion ABOVE Ok(RunOutput
grep -n "^mod " src/main.rs                                  # #13, #15
# #14 — docs-only verify, skip command
```

**Output:** fill table → if mọi anchor ✅ → proceed Task 1. If ⚠️/❌ → write Debate Log Turn 2 objection.

---

### Task 1: Thêm module `src/alert.rs` (NEW)

**File:** `src/alert.rs` (CREATE)

**Tìm:** N/A (new file).

**Thay bằng / Thêm:**
```rust
//! Telegram alert sender (Phase 2.1).
//!
//! Best-effort outbound POST to Telegram Bot API. Per PROJECT.md hard line #5
//! ("Failure mode = noisy"), advisory-cron surfaces task failures to Sếp's
//! phone via Telegram. Alert failure does NOT fail the task (logged via
//! `tracing::warn!`, swallowed at the caller).
//!
//! INV-19 governs this module: explicit timeout + error handling.
//!
//! **Env-free contract (V2 — per Worker Turn 1 recommendation):**
//! This module MUST NOT call `std::env::var` for any reason. The
//! `ADVISORY_CRON_TG_API_BASE` test seam is read at the call site in
//! `src/core/run.rs` and passed in via `send_with_base(api_base, msg)`.
//! This keeps `alert.rs` a pure function of its inputs and unit-testable
//! without env setup.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `[alert.telegram]` block (mirrors config schema). Re-exported here so
/// `from_config` can take `&TelegramConfig` without circular deps.
pub use crate::config::TelegramConfig;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TelegramAlert {
    bot_token: String,
    chat_id: String,
}

impl TelegramAlert {
    /// Build from config. Returns `Ok(None)` if `[alert.telegram]` block absent.
    /// Returns `Err` if config malformed (e.g. `bot_token_file` set but file
    /// unreadable, or neither `bot_token` nor `bot_token_file` provided).
    pub fn from_config(cfg: Option<&TelegramConfig>) -> Result<Option<Self>> {
        let Some(tg) = cfg else { return Ok(None); };
        let token = resolve_token(tg)?;
        Ok(Some(Self {
            bot_token: token,
            chat_id: tg.chat_id.clone(),
        }))
    }

    /// Construct from raw token + chat_id (test helper / explicit override).
    pub fn new(bot_token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self { bot_token: bot_token.into(), chat_id: chat_id.into() }
    }

    /// Convenience wrapper — forwards to `send_with_base` with the production
    /// Telegram API URL. Production call sites in `core::run::run` MUST use
    /// `send_with_base(api_base, msg)` directly (with `api_base` resolved from
    /// the optional `ADVISORY_CRON_TG_API_BASE` env var at the call site).
    /// This `send` shim exists for any future non-test call site that does
    /// not need the env override.
    pub async fn send(&self, message: &str) -> Result<()> {
        self.send_with_base(TELEGRAM_API_BASE, message).await
    }

    /// Send a message. `api_base` is the URL scheme+host (no trailing slash),
    /// e.g. `"https://api.telegram.org"` (prod) or the wiremock URL (tests).
    /// No env reads inside this function — `api_base` is the explicit seam.
    pub async fn send_with_base(&self, api_base: &str, message: &str) -> Result<()> {
        let url = format!("{api_base}/bot{}/sendMessage", self.bot_token);
        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("build reqwest client")?;
        let resp = tokio::time::timeout(
            HTTP_TIMEOUT,
            client
                .post(&url)
                .form(&[("chat_id", self.chat_id.as_str()), ("text", message)])
                .send(),
        )
        .await
        .context("telegram POST timed out")?
        .context("telegram POST transport error")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("telegram API error: status={status} body={body}");
        }
        Ok(())
    }
}

fn resolve_token(tg: &TelegramConfig) -> Result<String> {
    match (&tg.bot_token, &tg.bot_token_file) {
        (Some(t), None) => Ok(t.clone()),
        (None, Some(p)) => read_token_from_file(p),
        (Some(_), Some(_)) => {
            anyhow::bail!(
                "[alert.telegram]: provide either `bot_token` or `bot_token_file`, not both"
            )
        }
        (None, None) => anyhow::bail!(
            "[alert.telegram]: missing both `bot_token` and `bot_token_file`"
        ),
    }
}

/// Read `KEY=VAL` lines from `path`. Extract `TG_BOT_TOKEN=...` value.
/// Expand leading `~/` to `$HOME` (Architect uses std::env::var for HOME only,
/// which is a process-level invariant, NOT a feature env var — this is the
/// one allowed env read in alert.rs because HOME path expansion is a unix
/// filesystem primitive, not a test seam).
fn read_token_from_file(path: &Path) -> Result<String> {
    let expanded = expand_home(path)?;
    let content = std::fs::read_to_string(&expanded)
        .with_context(|| format!("read bot_token_file {}", expanded.display()))?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(rest) = line.strip_prefix("TG_BOT_TOKEN=") {
            // strip optional surrounding quotes
            let val = rest.trim_matches(|c| c == '"' || c == '\'');
            if val.is_empty() {
                anyhow::bail!("TG_BOT_TOKEN is empty in {}", expanded.display());
            }
            return Ok(val.to_string());
        }
    }
    anyhow::bail!("TG_BOT_TOKEN not found in {}", expanded.display())
}

fn expand_home(path: &Path) -> Result<PathBuf> {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        let home = std::env::var("HOME")
            .context("HOME env var unset — required to expand `~/` in bot_token_file")?;
        if home.is_empty() {
            anyhow::bail!("HOME is empty");
        }
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(path.to_path_buf())
}

// Format the alert message body. Caller pre-truncates stderr_tail to ~500 bytes
// to keep total message under Telegram's 4096-char limit.
pub fn format_failure_message(
    label: &str,
    exit_code: i32,
    duration_ms: u64,
    stderr_tail: &str,
) -> String {
    let tail = if stderr_tail.is_empty() { "<no stderr>".to_string() } else {
        // Truncate to ~500 bytes at UTF-8 char boundary (mirrors heartbeat::tail_utf8).
        // Worker: if `tail_utf8` is `pub` in heartbeat.rs, reuse it; else hand-truncate.
        truncate_chars(stderr_tail, 500)
    };
    format!(
        "❌ advisory-cron failed\nlabel={label}\nexit_code={exit_code}\nduration_ms={duration_ms}\n\nstderr_tail:\n{tail}"
    )
}

fn truncate_chars(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes { return s.to_string(); }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) { end -= 1; }
    let mut out = s[..end].to_string();
    out.push_str("…");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests Worker fleshes out. Architect supplies skeleton:
    // - test_from_config_none — Option None → Ok(None)
    // - test_from_config_inline_token — TelegramConfig with bot_token Some → Ok(Some)
    // - test_from_config_file_token — write tempfile, point bot_token_file → Ok(Some)
    // - test_from_config_both_set — both bot_token and bot_token_file Some → Err
    // - test_from_config_neither_set — both None → Err
    // - test_send_with_base_happy_wiremock — mock returns 200 → Ok(())
    // - test_send_with_base_500_returns_err — mock returns 500 → Err with body
    // - test_send_with_base_timeout — mock delay > HTTP_TIMEOUT → Err with "timed out"
    // - test_format_failure_message — assert label/exit_code/duration/stderr visible
    // - test_truncate_chars_utf8_boundary — multi-byte char at boundary → no panic
}
```

**Lưu ý:**
- **Env-free contract (V2):** `alert.rs` MUST NOT call `std::env::var` for the API base override. The `ADVISORY_CRON_TG_API_BASE` env var (test seam) is read at the call site in `src/core/run.rs` (Task 3). The only env read in `alert.rs` is `HOME` inside `expand_home` — that's a unix filesystem primitive (not a test seam), kept inline for module isolation.
- **API base override:** Architect chose `send_with_base(api_base, message)` over builder. `send(msg)` is now a thin shim forwarding to `send_with_base(TELEGRAM_API_BASE, msg)` for any future non-test call site that doesn't need the env override. Worker may refactor if cleaner pattern emerges — stay within ≤200 LOC scope.
- **Token expansion:** `~/` expansion is inline (KHÔNG depend on `core/config_path::home_dir` — that module is for default config path resolution, alert is config-data path). Worker may reuse `home_dir()` if doable without circular dep — Architect leans inline for module isolation.
- **`tracing::warn!` happens at CALLER (core/run.rs Task 3), KHÔNG inside `send_with_base`** — `send_with_base` returns `Result`, caller decides log-and-continue.
- **INV-19 wording (Task 4)** mirrors INV-2 generic baseline + module-specific implementation note, plus explicit "env-var-at-call-site" rule. KHÔNG copy INV-17/INV-18 verbosity — alert is outbound, no inbound surface to defend.

---

### Task 2: Extend `src/config.rs` — add `AlertConfig` + `TelegramConfig`

**File:** `src/config.rs`

**Tìm:** Cuối module (sau existing struct definitions; Worker grep `pub struct HeartbeatConfig` để locate insertion point post-existing structs).

**Thay bằng / Thêm:**
```rust
/// `[alert]` block. Optional — alert is opt-in.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertConfig {
    pub telegram: Option<TelegramConfig>,
}

/// `[alert.telegram]` block. Either `bot_token` (inline) OR `bot_token_file`
/// (path to KEY=VAL file with TG_BOT_TOKEN=...). Validated at load time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub chat_id: String,
    pub bot_token: Option<String>,
    pub bot_token_file: Option<std::path::PathBuf>,
}
```

**Also Tìm:** struct `Config` definition (Worker grep `pub struct Config`).

**Thay bằng / Thêm:** add field
```rust
pub struct Config {
    // ... existing fields ...
    #[serde(default)]
    pub alert: Option<AlertConfig>,
}
```

**Also Tìm:** `Config::validate` function (Worker grep `pub fn validate\|fn validate`).

**Thay bằng / Thêm:** add validation step
```rust
// after existing validations
if let Some(alert) = &self.alert {
    if let Some(tg) = &alert.telegram {
        if tg.chat_id.trim().is_empty() {
            anyhow::bail!("[alert.telegram].chat_id is empty");
        }
        match (&tg.bot_token, &tg.bot_token_file) {
            (Some(_), Some(_)) => anyhow::bail!(
                "[alert.telegram]: specify either `bot_token` or `bot_token_file`, not both"
            ),
            (None, None) => anyhow::bail!(
                "[alert.telegram]: missing both `bot_token` and `bot_token_file`"
            ),
            (Some(t), None) if t.trim().is_empty() => anyhow::bail!(
                "[alert.telegram].bot_token is empty"
            ),
            _ => {}
        }
    }
}
```

**Also Tìm:** `default_for_home` function (Worker grep `pub fn default_for_home`).

**Thay bằng:** **KHÔNG MODIFY** — default config does NOT include `[alert]` block. Alert is strictly opt-in; `advisory-cron init` writes config without alert. Sếp manually adds `[alert.telegram]` block to enable.

**Lưu ý:**
- `#[serde(default)]` on `alert` field — old configs without `[alert]` deserialize as `None`, backwards-compat preserved.
- `AlertConfig::default()` derives None for `telegram` — `#[derive(Default)]` works.
- `TelegramConfig` does NOT derive `Default` — required field `chat_id` means no sensible default.
- Add unit tests in `src/config.rs::tests`:
  - `test_load_without_alert_block` — old config loads with `config.alert == None`.
  - `test_load_with_alert_inline_token` — `[alert.telegram] chat_id=... bot_token=...` → Some.
  - `test_load_with_alert_file_token` — `[alert.telegram] chat_id=... bot_token_file=...` → Some.
  - `test_validate_alert_both_set` — both bot_token and bot_token_file → Err.
  - `test_validate_alert_neither_set` → Err.
  - `test_validate_alert_empty_chat_id` → Err.

---

### Task 3: Wire alert into `src/core/run.rs` (V2 — Option A, env-var-at-call-site)

**File:** `src/core/run.rs`

**Tìm:** AFTER the `match fire_result` block (which binds `exit_code`, `stdout_tail`, `stderr_tail`, `duration_ms` — Worker grep `^    let (exit_code, stdout_tail, stderr_tail, duration_ms) = match fire_result` to confirm its end), BEFORE `Ok(RunOutput { ... })` return. Worker grep `^    Ok(RunOutput` to locate the return.

**Insertion location (V2):** Between the end of the `match fire_result` block (~line 90 per Worker's Turn 1 evidence) and the `Ok(RunOutput { ... })` return statement (line 92). At this point all of `label`, `exit_code`, `duration_ms`, `stderr_tail` are bound. `fire_result` is already consumed by the match — no borrow conflict.

**Thay bằng / Thêm:** insert the following block BEFORE `Ok(RunOutput { ... })`:
```rust
// Phase 2.1 — Telegram alert on fail (best-effort, INV-19 boundary).
// Insertion point V2: AFTER `match fire_result` (vars in scope), BEFORE `Ok(RunOutput)`.
if exit_code != 0 {
    if let Some(alert_cfg) = config.alert.as_ref().and_then(|a| a.telegram.as_ref()) {
        match crate::alert::TelegramAlert::from_config(Some(alert_cfg)) {
            Ok(Some(alert)) => {
                // Env-var-at-call-site (V2 — Worker Turn 1 recommendation, Architect ACCEPT).
                // `ADVISORY_CRON_TG_API_BASE` is a TEST-ONLY seam (set by integration tests
                // to redirect POST to wiremock). Production: env var unset → default base.
                // `alert.rs` itself stays env-free for unit-testability.
                let api_base = std::env::var("ADVISORY_CRON_TG_API_BASE")
                    .unwrap_or_else(|_| "https://api.telegram.org".to_string());
                let msg = crate::alert::format_failure_message(
                    &label,           // bound from line 44
                    exit_code,        // bound from match fire_result
                    duration_ms,      // bound from match fire_result
                    &stderr_tail,     // bound from match fire_result
                );
                if let Err(e) = alert.send_with_base(&api_base, &msg).await {
                    tracing::warn!(error = %e, "telegram alert send failed (best-effort, swallowing)");
                }
            }
            Ok(None) => {} // unreachable given outer Some check, but defensive
            Err(e) => {
                tracing::warn!(error = %e, "telegram alert config invalid (best-effort, swallowing)");
            }
        }
    }
}
```

**Lưu ý:**
- **Variable names** (V2 — confirmed by Worker Turn 1): `label` is bound at line 44 (already in scope). `exit_code`, `stderr_tail`, `duration_ms` are bound at line 77 inside the `match fire_result` block; at our V2 insertion point (after that block ends, before `Ok(RunOutput)`), all four are in scope. Use them as-is.
- **`send_with_base` not `send`:** Production call site MUST go through `send_with_base(&api_base, &msg)`. `api_base` defaults to the production URL when the env var is unset; the env var is read here at the call site (not inside `alert.rs`).
- **Best-effort discipline (Hard line #5 alignment):**
  - Alert send fail → `warn!` log + continue. Task already wrote heartbeat (durable record) before alert. Failure trail not lost.
  - Alert config invalid → same: warn + continue. We do NOT bail the run because alert config is misconfigured — that's a deploy bug, surface via log not exit.
  - NOTE this is the ONE intentional "silent" path (well, log-warn-only) in advisory-cron. INV-9 forbids silent failure in task fire — alert failure ≠ task fire failure. Distinction documented in INV-19.
- **NO additional config-path threading** — `config` is already in scope (passed to `core::run::run`). Constraint #4 honored.
- **NO new async runtime feature flags** — `tokio::time::timeout` already pulled by `time` feature (Anchor #6).
- **`tracing::warn!` macro** — Worker verify `tracing` imported at file top; if not, add `use tracing::warn;` or qualify path.
- **Env-free invariant for `alert.rs`** — see INV-19 §Implementation: `std::env::var("ADVISORY_CRON_TG_API_BASE")` MUST be read here in `core/run.rs`, NEVER inside `src/alert.rs`. Worker check `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` returns empty.

---

### Task 4: Append INV-19 to `docs/security/INVARIANTS.md`

**File:** `docs/security/INVARIANTS.md`

**Tìm:** After INV-18 block (Worker grep `^### INV-18 ` then find end of that section before `^## How INV`).

**Thay bằng / Thêm:**
```markdown
### INV-19 — Telegram alert HTTP boundary: timeout + error handling, log-warn-not-bail on failure, env-free alert module

**Statement:** PR introducing or modifying `src/alert.rs::TelegramAlert::send_with_base` (or any future outbound HTTP alert) MUST:
1. Wrap the HTTP call in BOTH `reqwest::Client::builder().timeout(Duration)` (client-level) AND `tokio::time::timeout(Duration, ...)` (outer guard against pre-connect hangs / DNS hangs). Default `HTTP_TIMEOUT = 10s` (matches INV-2 generic baseline).
2. Return `Result<()>` from `send_with_base` — caller (currently `src/core/run.rs`) decides whether to log-warn-continue (best-effort) or bail. The current contract: `core::run::run` ALWAYS log-warn-continues — alert failure ≠ task failure (PROJECT.md hard line #5 "noisy" applies to task, not to alert delivery itself).
3. Bot token MUST come from either inline TOML `bot_token` (user-owned config file, chmod 600 responsibility on user) OR `bot_token_file` (path to `KEY=VAL` env-style file). The two are mutually exclusive at config validation time. No shell interpolation `${VAR}` pattern (Option B in Heads-up #3) is supported — Worker MUST NOT add it.
4. Telegram API base URL is `https://api.telegram.org` (constant). `send_with_base(base, msg)` accepts an explicit base for production AND test-time override. **The `ADVISORY_CRON_TG_API_BASE` env var (test seam) MUST be read at the call site in `src/core/run.rs`, NEVER inside `src/alert.rs`.** This keeps `alert.rs` a pure function of its inputs and unit-testable without env setup. Production code in `core/run.rs` reads the env var with `unwrap_or_else(|_| "https://api.telegram.org".to_string())` and passes the result to `send_with_base(&api_base, &msg)`.

**Why:** Telegram is the first outbound HTTP service in advisory-cron. INV-2 generic baseline ("external service call → timeout + error handling") applies but needs concrete teeth for this codebase. The log-warn-not-bail discipline is critical: silent failure is the bug advisory-cron exists to fix, but the failure we mean is *task* failure — not alert-delivery failure (network blip should not mask the underlying task failure that triggered the alert; heartbeat JSONL is the durable record, alert is the push channel). The env-free `alert.rs` rule (V2) keeps the library testable in isolation — unit tests don't need to set or unset env vars to exercise `send_with_base`.

**Implementation (Phase 2.1):** `src/alert.rs::TelegramAlert::send_with_base` — `reqwest::Client::builder().timeout(HTTP_TIMEOUT).build()?` + `tokio::time::timeout(HTTP_TIMEOUT, client.post(url).form(...).send()).await`. Caller in `src/core/run.rs` reads `std::env::var("ADVISORY_CRON_TG_API_BASE").unwrap_or_else(|_| "https://api.telegram.org".to_string())` and wraps `alert.send_with_base(&api_base, &msg).await` in `if let Err(e) = ... { tracing::warn!(...); }` — no `?` propagation.

**Trust boundary:** Bot token is a secret read from user config (chmod 600 responsibility on Sếp). advisory-cron does NOT log the token. POST body contains `chat_id` + `text` only — no token in body. URL contains token (Telegram API spec) — URL MUST NOT be logged at info/debug level. INV-19 forbids logging the full request URL.

**Trigger keywords:** `TelegramAlert::send_with_base` call sites, `reqwest::Client` + `api.telegram.org`, `ADVISORY_CRON_TG_API_BASE` env var reads outside `core/run.rs` (forbidden — would violate env-free `alert.rs` rule), new alert backends (Slack, Discord, etc. would need parallel INV).

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE; Giám sát soi PR diff for alert-related changes via INV-2 generic rubric + specific check that `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` is empty.

---
```

**Lưu ý:**
- Bumps "Generic INV count" reference if present elsewhere — Worker grep `INV-1..18` or `max INV` strings in docs/ and update if any reference total count (likely just CHANGELOG / Discovery Report).
- V2 INV-19 wording explicitly mentions env-free `alert.rs` rule (Worker Turn 1 design recommendation, Architect ACCEPT).

---

### Task 5: Add `wiremock` dev-dep to `Cargo.toml`

**File:** `Cargo.toml`

**Tìm:** `[dev-dependencies]` section. If absent, append after `[dependencies]` block.

**Thay bằng / Thêm:**
```toml
[dev-dependencies]
# ... existing dev-deps if any ...
wiremock = "0.6"
```

**Lưu ý:**
- `wiremock` 0.6.x is the current major. Worker Turn 1 confirmed `cargo search wiremock` → `wiremock = "0.6.5"`; `"0.6"` resolves to 0.6.5 via semver — correct.
- Pure-Rust async mock — works with `tokio` runtime, no native deps (chosen over `httpmock` for tokio-native and over hand-rolled mock for test ergonomics).
- KHÔNG add to `[dependencies]` — alert sender uses real `reqwest`, mock only intercepted in tests via `send_with_base(mock_url, ...)`.

---

### Task 6: Add integration test `tests/cli_run_alert.rs`

**File:** `tests/cli_run_alert.rs` (CREATE)

**Tìm:** N/A (new file).

**Thay bằng / Thêm:**
```rust
//! Integration: `advisory-cron run` with a failing task + Telegram alert configured.
//! Subprocess invokes the binary; wiremock mocks Telegram endpoint; assert POST received.
//!
//! V2: API base override via `ADVISORY_CRON_TG_API_BASE` env var. Test sets this
//! env var on the subprocess via `Command::env(...)` so the child `core::run::run`
//! reads it at the call site and routes POST to the mock server. `alert.rs` itself
//! is env-free — no env setup needed for unit tests of `send_with_base`.

use std::process::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn run_failing_task_posts_to_telegram() {
    // Worker scaffolds:
    // 1. Start wiremock MockServer (gets a URL like http://127.0.0.1:PORT).
    // 2. Configure Mock to match POST /bot<token>/sendMessage → 200.
    // 3. Write tempdir config.toml with:
    //    [task] command="false" args=[] working_dir="."  label="test-alert"
    //    [schedule] hour=9 minute=0
    //    [heartbeat] log_path=tempdir/heartbeat.jsonl
    //    [alert.telegram] chat_id="123" bot_token="testtoken"
    // 4. Spawn:
    //      Command::new(env!("CARGO_BIN_EXE_advisory-cron"))
    //          .env("ADVISORY_CRON_TG_API_BASE", mock_server.uri())
    //          .arg("run").arg("--config").arg(&config_path)
    //          .output()
    //    (env var is read inside the spawned child's `core::run::run` and
    //     passed to `send_with_base`. Parent test process does not need to
    //     set or unset its own env.)
    // 5. Assert exit code 4 (task fire failed).
    // 6. Assert mock received exactly 1 POST.
    // 7. Assert heartbeat JSONL line written with exit_code != 0.
}

#[tokio::test]
async fn run_failing_task_without_alert_config_no_post() {
    // Same as above minus [alert.telegram] block. Still set env var to mock URL —
    // verifies that absence of [alert.telegram] config short-circuits before any
    // env read / HTTP call (no spurious POST even with env set).
    // Assert mock received 0 POSTs.
    // Assert heartbeat still written.
}

#[tokio::test]
async fn run_successful_task_no_post() {
    // [task] command="true" args=[] + [alert.telegram] configured.
    // Env var set to mock URL.
    // Assert mock received 0 POSTs (success path doesn't alert — exit_code == 0 short-circuit).
}
```

**Lưu ý:**
- **API base override mechanism (V2 — FINALIZED):** Env var `ADVISORY_CRON_TG_API_BASE` is read at the call site in `src/core/run.rs` (Task 3), NOT inside `alert.rs`. Test sets the env var on the SUBPROCESS via `Command::env(KEY, VAL)` — the subprocess inherits the env, parent test process is unaffected. This avoids any cross-test env contamination AND keeps `alert.rs` unit tests env-free.
- **`alert.rs` unit tests** (Task 1 skeleton: `test_send_with_base_happy_wiremock`, `test_send_with_base_500_returns_err`, `test_send_with_base_timeout`) instantiate `TelegramAlert::new("testtoken", "123")` and call `alert.send_with_base(&mock_server.uri(), "msg").await` directly — no env var involved. This is the testability win from "env-var-at-call-site".
- **`tempfile` crate** — already in dev-deps per CLAUDE.md (used by P003 plist tests). Worker Turn 1 confirmed `tempfile = "3"` at `Cargo.toml:28`.
- **Integration tests run on `cargo test --all`** by default (no `--ignored` flag needed).

---

### Task 7: Update `docs/ARCHITECTURE.md` — §Modules, §Config schema, §Error handling + alerting, §Phase status

**File:** `docs/ARCHITECTURE.md`

**Tìm 1:** `### Modules` table (Phase 2 placeholder line: `*(Phase 2 adds src/alert.rs for Telegram + src/retry.rs for retry policy.)*`).

**Thay bằng:** add row for `src/alert.rs` IN the table (between `heartbeat.rs` row and the Phase 2 italic note), and update italic note:
```markdown
| `src/alert.rs` | `TelegramAlert::send_with_base` outbound POST to Telegram Bot API. Best-effort (alert fail ≠ task fail). Env-free module (API base override env var `ADVISORY_CRON_TG_API_BASE` is read at the call site in `core::run::run`, NOT here). INV-19 boundary (timeout + error handling, env-free). | 2.1 ✅ |

*(Phase 2.2 will add `src/retry.rs` for retry policy. Phase 2.3 adds crash-safe heartbeat write.)*
```

**Tìm 2:** `### Full schema` TOML block under `## Config schema (Phase 1.2)`.

**Thay bằng:** Add `[alert.telegram]` block at end of TOML example with comment "(Phase 2.1 — optional)":
```toml
# (Phase 2.1 — optional)
[alert.telegram]
chat_id = "1184530337"
# Choose ONE of:
bot_token_file = "~/.advisory-cron-secrets.env"  # path to KEY=VAL file with TG_BOT_TOKEN=...
# bot_token = "8678210414:AAGN..."  # inline (less secure — config file must be chmod 600)
```

**Tìm 3:** Field reference table (after the TOML block).

**Thay bằng:** Append 3 rows:
```markdown
| `[alert.telegram]` | `chat_id` | `string` | yes (if block present) | Telegram chat ID (numeric string or `@channelname`) | — |
| `[alert.telegram]` | `bot_token` | `string (one-of)` | one-of bot_token/bot_token_file | Inline bot token (config file should be chmod 600) | — |
| `[alert.telegram]` | `bot_token_file` | `path (one-of)` | one-of bot_token/bot_token_file | Path to `KEY=VAL` file containing `TG_BOT_TOKEN=...` | — |
```

**Tìm 4:** `## Error handling + alerting` section (around line 278).

**Thay bằng:** Update Phase 2 paragraph:
```markdown
Phase 2 ships `src/alert.rs` (P008):
- Telegram bot POST on `exit_code != 0` (best-effort).
- Configurable via `[alert.telegram]` block (chat_id + bot_token OR bot_token_file).
- INV-19 boundary: 10s timeout (reqwest client + `tokio::time::timeout` outer guard), error returned to caller as `Result<()>`.
- `alert.rs` is env-free; the test-only API base override env var `ADVISORY_CRON_TG_API_BASE` is read at the call site in `core::run::run` and passed to `send_with_base(base, msg)`. This keeps `alert.rs` unit-testable in isolation.
- Caller in `core::run::run` log-warn-continues on alert send error — alert failure does NOT fail the task. Heartbeat JSONL is the durable failure record; Telegram is the push channel.
```

**Tìm 5:** `## Phase status`.

**Thay bằng:** Update Phase 2 bullet:
```markdown
- 🚧 **Phase 2** — In progress. Phase 2.1 (Telegram alert) shipped per P008. Phase 2.2 (retry) + 2.3 (state recovery) pending.
```

**Lưu ý:**
- Tầng 1 docs updates — Worker MUST update before commit per Definition of Done #7.
- KHÔNG add `src/retry.rs` row (Phase 2.2 future).
- V2 updates: §Modules row + §Error handling paragraph both note env-free `alert.rs` rule.

---

### Task 8: CHANGELOG + README

**File:** `docs/CHANGELOG.md`

**Tìm:** Top of file (most recent entry up top).

**Thay bằng / Thêm:** prepend entry
```markdown
## P008 — Telegram alert on fail (Phase 2.1) — 2026-05-MM

- Added `src/alert.rs` — `TelegramAlert` with `send_with_base(api_base, msg)` (10s timeout, reqwest + tokio::time::timeout double guard). Env-free module: API base override env var `ADVISORY_CRON_TG_API_BASE` is read at the call site in `core::run::run`, NOT inside `alert.rs`, keeping the module unit-testable in isolation.
- Extended `src/config.rs` — `AlertConfig`, `TelegramConfig` (chat_id + one-of bot_token/bot_token_file), validation, backwards-compat (`#[serde(default)]`).
- Wired `src/core/run.rs` — AFTER `match fire_result` block and BEFORE `Ok(RunOutput)` return: on `exit_code != 0`, best-effort POST to Telegram via `send_with_base(&api_base, &msg)`; `tracing::warn!` on failure, never bail.
- Added INV-19 (Telegram HTTP boundary: timeout + error handling, log-warn-not-bail, env-free `alert.rs` rule).
- Added `wiremock = "0.6"` dev-dep + `tests/cli_run_alert.rs` integration (failing task → POST; success → no POST; no alert config → no POST). Tests set `ADVISORY_CRON_TG_API_BASE` on the subprocess via `Command::env(...)` — parent process env unaffected.
- Updated ARCHITECTURE.md (Modules table, Config schema TOML block + field reference, Error handling + alerting Phase 2 paragraph, Phase status to Phase 2.1 SHIPPED).
- Updated README brief Phase 2 mention + config snippet.
- All 94 existing tests still pass; +N new (Worker count).
```

**File:** `README.md`

**Tìm:** Section after Phase 1 quick-start (Worker grep `## ` heading for natural insertion).

**Thay bằng / Thêm:**
```markdown
## Phase 2.1 — Telegram alert on failure (optional)

When a fire fails (`exit_code != 0`), advisory-cron can POST to a Telegram bot, so you see the failure on your phone without checking the heartbeat log.

Add to your config file (`~/.config/advisory-cron/config.toml`):

```toml
[alert.telegram]
chat_id = "<your chat id>"
bot_token_file = "~/.advisory-cron-secrets.env"  # KEY=VAL file with TG_BOT_TOKEN=...
```

Create the secrets file `~/.advisory-cron-secrets.env` (chmod 600):
```
TG_BOT_TOKEN=<token from @BotFather>
```

Alternatively, inline the token (config file must be chmod 600):
```toml
[alert.telegram]
chat_id = "<your chat id>"
bot_token = "<token from @BotFather>"
```

Alert is best-effort: a network blip will not fail the task. The heartbeat JSONL remains the durable failure record. INV-19 governs the alert HTTP boundary.
```

**Lưu ý:**
- README example matches Sếp's actual setup so dogfood test instructions are accurate.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/alert.rs` | **NEW** — Task 1 (TelegramAlert + from_config + send + send_with_base + format_failure_message + tests; env-free module) |
| `src/config.rs` | Task 2 — AlertConfig + TelegramConfig + Config::alert field + validation + tests |
| `src/core/run.rs` | Task 3 — best-effort alert call AFTER match fire_result block, BEFORE Ok return; env var read at call site |
| `src/main.rs` | Task 1/3 — add `mod alert;` (Worker verify mod declaration style) |
| `Cargo.toml` | Task 5 — add `[dev-dependencies] wiremock = "0.6"` |
| `tests/cli_run_alert.rs` | **NEW** — Task 6 (3 integration tests via subprocess + wiremock + `Command::env` for API base override) |
| `docs/security/INVARIANTS.md` | Task 4 — append INV-19 (includes env-free `alert.rs` rule) |
| `docs/ARCHITECTURE.md` | Task 7 — Modules row (env-free note), Config schema, Error handling + alerting (env-free note), Phase status |
| `docs/CHANGELOG.md` | Task 8 — P008 entry |
| `README.md` | Task 8 — Phase 2.1 brief section |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/cli/mod.rs` | **CONSTRAINT #1 re-instated** — Worker confirms ZERO edits to dispatch |
| `src/cli/run.rs` | Thin shell over `core::run::run` already — gets alert behavior for free, no edits |
| `src/cli/init.rs`, `register.rs`, `unregister.rs`, `status.rs`, `mcp.rs` | No edits — alert is internal to run flow |
| `src/mcp/tools.rs` | `run` MCP tool delegates to `core::run::run` — automatically gets alert path, no schema change |
| `src/mcp/server.rs` | No edits |
| `src/launchd.rs` | No edits — alert is post-task, plist generation unchanged |
| `src/runner.rs` | No edits — `fire_task` returns `RunResult` as before; alert decision in `core::run` |
| `src/heartbeat.rs` | No edits — alert reads `exit_code`/`label`/`stderr_tail` from existing values |
| `src/core/init.rs`, `register.rs`, `unregister.rs`, `status.rs` | No edits — alert is `core::run`-specific |
| `src/core/config_path.rs` | No edits — alert uses inline `expand_home` helper (module isolation) |
| `Cargo.toml [dependencies]` | Verify NO runtime dep added — reqwest, tokio, serde, serde_json all already present |

---

## Luật chơi (Constraints)

1. **CONSTRAINT #1 (re-instated post-P006 V2):** KHÔNG edit `src/cli/mod.rs` newtype dispatch. Alert is internal to `core::run::run` — no new subcommand, no new CLI flag. If Worker discovers a need to touch dispatch → STOP, escalate.

2. **CONSTRAINT #4 (V2 internal-resolution pattern):** No config-path or env-var threading through call stacks. `core::run::run` already receives `config: &Config` — alert uses `config.alert.as_ref()`. `bot_token_file` path expansion happens INSIDE `alert.rs::expand_home`, NOT threaded from caller. The `ADVISORY_CRON_TG_API_BASE` env read happens at the `core/run.rs` call site (one layer above `alert.rs`), passed explicitly as `&str` to `send_with_base` — consistent with "resolve env at one boundary, not threaded".

3. **No new runtime dep.** Cargo.toml `[dependencies]` unchanged. `reqwest`, `tokio (time + macros + rt)`, `serde`, `serde_json` already present (Anchors #5 #6). New `[dev-dependencies] wiremock` is test-only.

4. **No `unsafe { }` block** (INV-6 hard-stop). HTTP + std + serde — no FFI surface needed.

5. **INV-2 + INV-19 enforcement at every alert POST site.** Future Slack/Discord/etc. backends MUST follow same pattern (10s timeout double-guard, `Result<()>` to caller, log-warn-not-bail at caller, env-free module + env-var-at-call-site for test base override).

6. **No `tokio::spawn` for alert.** Sequential `.await` — task already completed (exit_code captured). +10s worst-case latency is acceptable for solo CLI (no concurrent fires; user-visible delay only on failure path which is rare).

7. **No silent log of bot token.** INV-19 §Trust boundary — full request URL contains token; MUST NOT appear in tracing logs above debug level (and ideally never). Use `tracing::warn!("telegram POST failed: {}", error_kind)` without printing URL.

8. **Telegram message format is fixed by `format_failure_message`** (Task 1). KHÔNG let caller hand-build message strings — central format = central truncation = consistent Sếp UX.

9. **Backwards compat:** Old configs (Phase 1 — no `[alert]` block) MUST load without error. `#[serde(default)]` on `Config::alert` field. Test `test_load_without_alert_block` MANDATORY.

10. **Hard line #5 alignment:** Alert log-warn-continue is the ONE intentional "log-not-bail" path for an outbound side effect. INV-9 ("no silent failure in task fire") still holds — alert delivery ≠ task fire. Distinction documented in INV-19 §Why.

11. **CONSTRAINT #11 (V2 — new):** `src/alert.rs` MUST NOT call `std::env::var` for the API base override. Only `HOME` env read (inside `expand_home`) is allowed — that's a unix filesystem primitive, not a test seam. The `ADVISORY_CRON_TG_API_BASE` env var MUST be read at the call site in `src/core/run.rs`. Verify: `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` returns EMPTY. This constraint exists because env-free `alert.rs` is what makes its unit tests fast and isolated (no env setup/teardown).

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings; binary ≤7MB (per PROJECT.md acceptance criterion)
- [ ] `cargo test --all` — all pass (94 existing + new alert unit tests + 3 new integration tests in `cli_run_alert.rs`)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `grep "ADVISORY_CRON_TG_API_BASE" src/alert.rs` — empty (CONSTRAINT #11)

### Manual Testing
- [ ] **Failing task + alert configured** — Sếp writes `[alert.telegram]` block (real bot token via `bot_token_file = "~/.advisory-cron-secrets.env"`, chat_id `1184530337`) + sets `[task] command = "false"`. Runs `advisory-cron run`. Expects: exit code 4, heartbeat written with exit_code != 0, Telegram message arrives at @chiha_alert_bot DM within ~10s. Message body contains `label`, `exit_code`, `duration_ms`, stderr (empty for `false`).
- [ ] **Successful task + alert configured** — `[task] command = "true"`. Run. Expect: exit 0, heartbeat written, NO Telegram message.
- [ ] **Failing task + alert NOT configured** — remove `[alert.telegram]`. Run with `false`. Expect: exit 4, heartbeat written, NO Telegram message, no warn log about alert.
- [ ] **Invalid alert config** — set `bot_token` AND `bot_token_file` both — `advisory-cron run` should still execute task (since validation is at config load); but `Config::validate` should reject at load → exit 2 with clear error. Worker verifies validation order.
- [ ] **Alert timeout simulation** — point `bot_token_file` to file with junk token + `chat_id` valid. Telegram API returns 401. `run` exits with task's exit code (4 for `false`, 0 for `true`), warn log shows "telegram API error: status=401". Task not failed by alert.

### Regression
- [ ] `advisory-cron init` still writes default config (without `[alert]` block) — Phase 1 behavior preserved.
- [ ] `advisory-cron register --label foo --schedule "0 9 * * *"` still generates plist + bootstraps.
- [ ] `advisory-cron unregister --label foo` still removes plist (idempotent).
- [ ] `advisory-cron status --label foo` still reads launchctl print + heartbeat.
- [ ] `advisory-cron mcp` MCP `run` tool gets alert path automatically (test via Claude Desktop OR integration test if Worker adds one).
- [ ] All 94 pre-existing tests pass.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P008 entry prepended.
- [ ] `docs/ARCHITECTURE.md` — Modules row (env-free note), Config schema, Error handling + alerting (env-free note), Phase status all updated (Tầng 1 requirement).
- [ ] `docs/security/INVARIANTS.md` — INV-19 appended (includes env-free `alert.rs` rule).
- [ ] `README.md` — Phase 2.1 brief section added with config snippet.
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P008.md` — full report written (assumptions correct/incorrect, edge cases, docs updated). MUST note the V1→V2 changes: insertion point shift (O1.1 Option A) + env-var-at-call-site decision.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry prepended (newest at top).
- [ ] Sub-mechanism A-E Verification Trace filled in table above.

### AI Bias câu vàng (Rule 11 — Architect declares)
- "Cái này giải vấn đề Sếp ĐANG có, hay GIẢ ĐỊNH có?" — Sếp ĐANG có:
  - Phase 1 đã ship, daily `/advisory-scan` đang fire (Sếp dogfood). Nếu Claude Code crash hoặc `claude -p` hang silently mid-fire → Sếp chỉ biết qua `advisory-cron status` (pull model). Push model qua Telegram là vấn đề THẬT.
  - Pre-req secrets staged + e2e verified hôm nay (msg_id=21) → Sếp đã commit invest setup, scope tight = ship.
  - KHÔNG over-spec: 1 backend (Telegram), 1 trigger (exit_code != 0), 1 message format. Không thêm Slack/Discord/email (chưa cần), không retry-on-alert-fail (best-effort là design), không alert-on-success/skip (yêu cầu rõ ràng = on fail).
