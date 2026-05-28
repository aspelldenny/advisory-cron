# INVARIANTS — advisory-cron

> Project-local invariant catalog consumed by Giám sát (boundary-check) specialist subagent via `/security-review`.
> The 5 generic INV below are baked into `agents/boundary-check.md` rubric. Project-specific INV-6+ extend per advisory-cron's domain.

---

## Generic INV (baked into Giám sát rubric — P042 ship)

### INV-1 — New env var → env template update

**Statement:** PR thêm new env var read (`std::env::var("X")`, `env!("X")`, shell `${X}`) PHẢI update `.env.example` (or equivalent env-template doc) với key mới.

**Trigger keywords (Rust):** `std::env::var\(['\"]`, `env!\(['\"]`, `option_env!`.

**Status:** Active.

### INV-2 — New external service call → timeout + error handling

**Statement:** PR thêm new HTTP/external call PHẢI có explicit timeout AND error-handling.

**Trigger keywords (Rust):** `reqwest::`, `hyper::`, `surf::`, `tokio::net::`, `tokio::process::Command`.

**Per-call check:** `.timeout()` on client OR per-request `.timeout()`, AND `.await?` (or explicit `match` with `Err` arm).

**Status:** Active.

### INV-3 — Cross-user resource access → ownership binding

**Statement:** PR thêm API route/handler reading or mutating user-scoped data PHẢI có explicit ownership binding.

**Status:** ⚠️ **N/A for advisory-cron Phase 1** — no HTTP server, no multi-user surface. Will activate if Phase 2+ adds web UI or shared state.

### INV-4 — Webhook handler → signature verify + replay protection

**Statement:** PR thêm inbound webhook handler PHẢI verify signature/HMAC AND replay protection.

**Status:** ⚠️ **N/A for advisory-cron Phase 1** — outbound only (advisory-cron POSTS to Telegram, doesn't receive). Activates if Phase 2+ adds inbound webhook receiver.

### INV-5 — Dependency major bump → changelog/migration audit

**Statement:** PR bumps any `Cargo.toml` dependency's MAJOR version PHẢI cite changelog review trong PR description.

**Trigger keywords:** `Cargo.toml` `[dependencies]` diff showing MAJOR component change.

**Status:** Active.

---

## User-added INV (advisory-cron-specific)

### INV-6 — `unsafe { }` block requires explicit rationale

**Statement:** PR introducing any `unsafe { }` block PHẢI include a comment block above explaining:
1. Why safe Rust alternative was rejected
2. What invariants the `unsafe` code requires the caller to uphold
3. Reference to a `#[test]` that exercises the unsafe path

**Why:** advisory-cron is a local CLI tool — there's almost never a legitimate reason for `unsafe`. Standing rejection unless rationale is bulletproof.

**Trigger keywords:** `unsafe\s*{`, `unsafe fn`, `unsafe impl`.

**Status:** Active. Hard-stop in worker.md.

**Implemented in Giám sát:** No (project-local). Worker self-escalates if tempted.

### INV-7 — launchd plist write outside `~/Library/LaunchAgents/`

**Statement:** PR introducing code that writes a `.plist` file to any path OTHER than `~/Library/LaunchAgents/com.advisorycron.*.plist` MUST be flagged. advisory-cron does NOT write system-level (`/Library/LaunchAgents/`) or system daemon (`/Library/LaunchDaemons/`) plists — only user-session plists.

**Why:** Writing to `/Library/` paths requires root + can interfere with OS services. advisory-cron is a user tool, must stay in `~/Library/`.

**Trigger keywords:** path strings containing `/Library/LaunchAgents/` (not prefixed with `~/` or `$HOME/`) OR `/Library/LaunchDaemons/`.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-escalates.

### INV-8 — Heartbeat write outside XDG state path

**Statement:** PR introducing heartbeat write to path other than `$XDG_STATE_HOME/advisory-cron/` (default `~/.local/state/advisory-cron/`) MUST be flagged. Heartbeat is durable observability state — must follow XDG convention.

**Why:** Anti-littering. Sếp's home dir should not accumulate state files in unexpected locations.

**Trigger keywords:** `.write` or `OpenOptions::append` with path NOT prefixed by `xdg::BaseDirectories` or env-derived state dir.

**Status:** Active.

**Implemented in Giám sát:** No (project-local).

### INV-9 — No silent failure in task fire

**Statement:** PR introducing code that catches `tokio::process::Command` error or non-zero exit code WITHOUT logging to heartbeat OR returning Err to caller MUST be flagged. The "fail loud" hard line (PROJECT.md §Hard lines #5) is non-negotiable.

**Why:** advisory-cron exists BECAUSE silent failure is the bug. Reintroducing silent failure defeats the whole point.

**Trigger keywords:** `.unwrap_or_default()`, `.ok()`, `let _ = ...` on `Command::output()` / `Command::status()` results.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks via clippy + manual review.

---

### INV-10 — `launchctl` shell-out: absolute path args only, no user-string interpolation

**Statement:** PR touching `RealLaunchctl::bootstrap` or `RealLaunchctl::bootout` (or any future `launchctl` invocation) MUST pass only pre-validated, pre-sanitized strings to `Command::new("launchctl").arg(...)`. No `Command::new("sh").arg("-c").arg(format!("launchctl ... {user_label}"))` style. Shell interpolation of user-controlled input is PROHIBITED.

**Why:** `launchctl bootstrap gui/$UID <plist_path>` with an attacker-influenced path component could bootstrap arbitrary plists. advisory-cron accepts `--label` from the user; label sanitization (INV-12) is the first line of defense; no shell interpolation is the second.

**Implementation (Phase 1.3):** `src/launchd.rs` `RealLaunchctl` uses `Command::new("launchctl").arg("bootstrap").arg(&domain).arg(plist_path)` — each arg passed separately, no shell expansion. `domain` is `format!("gui/{uid}")` where `uid` is a `u32` (numeric, not user-controlled). `plist_path` is composed via `plist_path_for` from sanitized label.

**Trigger keywords:** `Command::new("sh")`, `.arg("-c")` combined with format strings containing user input, `std::process::Command` + `launchctl` in same expr.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-11 — `id -u` shell-out: result parsed as `u32`, no further interpolation

**Statement:** PR using `current_uid()` or any equivalent `id -u` invocation MUST parse the result as a plain `u32` before use. The raw string output of `id -u` MUST NOT be interpolated into shell commands or file paths without parsing.

**Why:** `id -u` output is expected to be a numeric UID string. Parsing to `u32` validates it is numeric and bounds-checked. Using the raw string could permit injection if (hypothetically) `id -u` were replaced or returns unexpected output.

**Implementation (Phase 1.3):** `src/launchd.rs::current_uid()` → `s.parse::<u32>()` — result is `u32`. Usage in `RealLaunchctl` is `format!("gui/{uid}")` where `uid: u32` — no shell injection surface.

**Trigger keywords:** `current_uid()` result used without `parse::<u32>()` step, or passed to `Command::new("sh").arg("-c")`.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-12 — Label sanitization: ASCII alphanumeric + `-` + `_` only; enforced at 2 points

**Statement:** PR accepting a `--label` CLI argument MUST validate the label against the ASCII alphanumeric + `-` + `_` allowlist at BOTH of:
1. Pre-flight check in `register::run` (before generating plist)
2. Inside `generate_plist` (defense-in-depth)

The label becomes part of a filesystem path (`~/Library/LaunchAgents/com.advisorycron.<label>.plist`) AND a launchd domain target string (`gui/<uid>/com.advisorycron.<label>`). Path traversal chars (`.`, `/`, `~`), shell meta-chars (`$`, `` ` ``, `&`, `;`), and whitespace are ALL PROHIBITED.

**Why:** Without sanitization, a label like `../../etc/cron.d/evil` or `foo; rm -rf ~` would either write plists to unexpected locations or (if shell-interpolated) execute arbitrary commands.

**Implementation (Phase 1.3):** `src/launchd.rs::generate_plist` — `label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')`. `src/cli/register.rs::run_with_deps` — early return exit 1 on empty label. Both checks active.

**Trigger keywords:** new `--label` parsing, `generate_plist` call sites, `plist_path_for` call sites.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-13 — Plist write boundary: `~/Library/LaunchAgents/com.advisorycron.*.plist` only

**Statement:** PR writing `.plist` files MUST write ONLY to `<launch_agents_dir>/com.advisorycron.<label>.plist` where `<launch_agents_dir>` defaults to `~/Library/LaunchAgents/` (or test-injected `TempDir`). Writing to:
- `/Library/LaunchAgents/` (system-level) — PROHIBITED (requires root)
- `/Library/LaunchDaemons/` (system daemons) — PROHIBITED (requires root)
- Arbitrary user-controlled path — PROHIBITED (label must be sanitized via INV-12 before path composition)

**Why:** System-level LaunchAgents/Daemons require root and can interfere with OS services. advisory-cron MUST stay user-scoped (`~/Library/LaunchAgents/`).

**Implementation (Phase 1.3):** `src/launchd.rs::plist_path_for(label, launch_agents_dir)` composes path as `launch_agents_dir.join(format!("com.advisorycron.{label}.plist"))`. `launch_agents_dir` is always either `default_launch_agents_dir(&home)` (= `<home>/Library/LaunchAgents`) or test-injected `TempDir`. Label is pre-sanitized (INV-12). `fs::write` target is the composed path — no free-form path override from user input.

**Trigger keywords:** `plist_path_for` call sites, `fs::write` + `.plist` extension, path containing `/Library/LaunchAgents/` or `/Library/LaunchDaemons/`.

**Status:** Active. Supersedes the shorter INV-7 (kept for reference; INV-13 is the authoritative expanded version).

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-14 — Child process spawn boundary: config-sourced command + args, no shell interpolation

**Statement:** PR introducing code that spawns child processes via `tokio::process::Command` or `std::process::Command` using values from user config (`config.task.command` + `config.task.args`) MUST pass command + args as discrete `.arg()` calls, NOT via `Command::new("sh").arg("-c").arg(format!("... {user_value}"))` shell interpolation.

**Why:** P004 `runner::fire_task` builds the command from user-controlled TOML (`task.command`, `task.args`). Phase 1 trusts the config file as user-controlled (same user who runs the tool). However, shell interpolation must still be avoided to prevent privilege escalation if config sourcing ever changes (e.g., remote config fetch in Phase 2+). `tokio::process::Command::new(cmd).args(args)` passes each value as a discrete OS-level argument — no shell expansion.

**Implementation (Phase 1.4):** `src/runner.rs::fire_task` — `Command::new(&config.task.command).args(&config.task.args).current_dir(...)`. Each arg is a discrete string. No shell wrapping. `config.task.command` is user-set in TOML — Phase 1 does NOT validate it against an allowlist (user owns config). Phase 2+ should add validation if config sourcing changes (remote config fetch, MCP tool invocation, etc.).

**Trust boundary note:** Phase 1: config file is user-writable, user-readable, local disk. The user who runs `advisory-cron run` owns the config. No third-party or elevated-privilege content in the command/args fields in Phase 1.

**Trigger keywords:** `runner::fire_task` call sites, `tokio::process::Command::new` + `config.task.command`, new spawn sites outside `runner.rs`.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-15 — Heartbeat file write boundary: user-configured path, `create_dir_all` creates ancestor dirs

**Statement:** PR introducing heartbeat write (`heartbeat::append`) MUST write to `config.heartbeat.log_path` (user-configured `PathBuf`). The `fs::create_dir_all(parent)` call in `append` may create directories at any path the user has write permission for — this is intentional (user owns config and chooses the path). Wildcard path override from non-config, non-user sources is PROHIBITED.

**Why:** `append` calls `fs::create_dir_all` on the parent of `log_path`. If `log_path` were ever sourced from an untrusted third party (rather than user's own TOML), this would allow arbitrary directory creation under the running user's permissions. Phase 1 trust boundary = user config only.

**Implementation (Phase 1.4):** `src/heartbeat.rs::append` — `log_path` comes from `config.heartbeat.log_path` (user-set in TOML). `fs::create_dir_all(parent)` is idempotent (returns Ok if dir already exists). No path sanitization in Phase 1 (user owns config). Phase 2+ should sanitize if config sourcing changes (e.g., remote config from MCP tool).

**INV-8 relationship:** INV-8 documents the XDG convention (`~/.local/state/advisory-cron/`). INV-15 is the mechanical boundary invariant — INV-8 is the convention guard.

**Trigger keywords:** `heartbeat::append` call sites, `fs::create_dir_all` + `log_path`, new write paths outside `heartbeat.rs`.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-16 — JSON serialization boundary: `serde_json::to_string` handles stdout/stderr_tail escape

**Statement:** PR introducing code that serializes `HeartbeatRecord` to JSON MUST use `serde_json::to_string` (or `serde_json::to_writer`) via serde derive — NOT hand-rolled JSON string assembly. `stdout_tail` and `stderr_tail` fields contain uncontrolled child process output; `serde_json` handles JSON character escaping correctly. Manual string assembly (`format!("{{\\"stdout_tail\\":\\"{val}\\"}}") where val = child output`) is PROHIBITED — it would produce invalid JSON on any child output containing `"` or `\`.

**Why:** `stdout_tail` / `stderr_tail` are derived from `String::from_utf8_lossy(&output.stdout)` — all bytes converted to UTF-8 String but content is uncontrolled (child process writes whatever it wants). `serde_json::to_string` on a `#[derive(Serialize)]` struct correctly escapes all JSON special characters. Manual assembly has no escape step and would produce malformed JSON (or worse — injection if the string is ever embedded in a larger JSON document).

**Additional guarantee:** `tail_utf8(s, max_bytes)` in `heartbeat.rs` snaps truncation to a UTF-8 char boundary. Combined with `from_utf8_lossy` upstream, the input to `serde_json` is always valid UTF-8. `serde_json::to_string` never receives invalid UTF-8 from these fields.

**Implementation (Phase 1.4):** `src/heartbeat.rs::append` — `serde_json::to_string(record)` on `#[derive(Serialize)] HeartbeatRecord`. `stdout_tail` / `stderr_tail` fields are `String` (valid UTF-8 guaranteed by `from_utf8_lossy` upstream + `tail_utf8` char-boundary snap). `serde_json` escapes internally.

**No shell interpretation of these strings in P004 code.** They are written to a JSONL file only. Phase 2 Telegram message formatting must re-escape for Telegram MarkdownV2 — that is a Phase 2 concern, not P004.

**Trigger keywords:** `HeartbeatRecord` field writes from external sources, new serialize call sites for fields containing uncontrolled bytes, manual JSON format! strings involving child process output.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-17 — `launchctl print` shell-out: label sanitization + discrete arg passing

**Statement:** PR introducing `launchctl print` shell-out (P005 `RealLaunchctl::print`) MUST:
1. Validate `label` against the INV-12 allowlist (ASCII alphanumeric + `-` + `_`) at BOTH (a) caller in `src/cli/status.rs::run` (pre-flight) and (b) inside `RealLaunchctl::print` impl (defense-in-depth).
2. Pass `label` as a component of `format!("gui/{uid}/com.advisorycron.{label}")` where `uid: u32` (numeric, parsed via `current_uid()`); the resulting target string is passed as a discrete `.arg()` to `Command::new("launchctl")`. NO `Command::new("sh").arg("-c").arg(format!("launchctl print ... {label}"))` — shell interpolation PROHIBITED.

**Why:** Same threat model as INV-10 / INV-12 — `launchctl print` accepts a target string that contains the label as a path component. Without sanitization, a label like `../foo` or `foo;evil` could either probe unintended services or (if shell-interpolated) inject. `launchctl print` is read-only (no side effect like `bootstrap`), so the impact is limited to information disclosure — still worth defending.

**Implementation (Phase 1.5):** `src/launchd.rs::RealLaunchctl::print` — `format!("gui/{uid}/com.advisorycron.{label}")` where `uid: u32` and `label` is pre-validated. `Command::new("launchctl").arg("print").arg(&target)` — discrete args, no shell. Caller in `src/cli/status.rs::run` validates label via `is_valid_label` helper before invocation.

**Trigger keywords:** `RealLaunchctl::print` call sites, `Command::new("launchctl").arg("print")`, new `launchctl <verb>` shell-outs.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks.

---

### INV-18 — MCP transport boundary: stdio JSON-RPC, label sanitization at tool entry

**Statement:** PR introducing MCP tool handlers (`src/mcp/tools.rs`) MUST:
1. Validate every `label` field at the MCP tool boundary BEFORE invoking `core::*::run` — ASCII alphanumeric + `-` + `_` allowlist (mirrors INV-12 pre-flight). This is a THIRD enforcement point in addition to INV-12's two points (CLI pre-flight + `generate_plist` defense-in-depth). MCP clients are external — never trust input.
2. Validate every `config_path` field is either absent (None → defaults) or a path with no `..` traversal sequences. (`PathBuf::components().any(|c| matches!(c, std::path::Component::ParentDir))` → reject.)
3. Tool result serialization via `serde_json::to_string` on `#[derive(Serialize)]` structs — NEVER hand-roll JSON (INV-16 generalization).
4. Tool handler errors propagate as MCP error objects (NOT process exit) — only transport-level errors (stdin EOF, malformed JSON-RPC frame) escape to `serve_stdio` and map to exit code 5 per ARCHITECTURE.md §CLI surface exit codes (via `return Ok(5)` in `src/cli/mcp.rs::run` per V2 P006 [O1.1]).

**Why:** MCP boundary IS the new attack surface in Phase 1.7. CLI was trusted (user owns their own keyboard). MCP tools are callable by any process Claude Desktop / Code talks to — the MCP server has no way to verify the upstream client's intentions. Defense at the boundary is non-negotiable.

**Implementation (Phase 1.7):** `src/mcp/tools.rs` — `validate_label` helper called at start of every tool handler that takes a `label`. `validate_config_path` for path inputs. `core::*::run` still validates internally (defense-in-depth). `serde_json::to_string_pretty(output)?` for all tool results.

**Trigger keywords:** new MCP tool handler additions, `rmcp::ServerHandler` impls, MCP server boot in any new transport beyond stdio.

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE; Giám sát soi PR diff for MCP-related additions.

---

### INV-19 — Telegram alert HTTP boundary: timeout + error handling, log-warn-not-bail on failure, env-free alert module

**Statement:** PR introducing or modifying `src/alert.rs::TelegramAlert::send_with_base` (or any future outbound HTTP alert) MUST:
1. Wrap the HTTP call in BOTH `reqwest::Client::builder().timeout(Duration)` (client-level) AND `tokio::time::timeout(Duration, ...)` (outer guard against pre-connect hangs / DNS hangs). Default `HTTP_TIMEOUT = 10s` (matches INV-2 generic baseline).
2. Return `Result<()>` from `send_with_base` — caller (currently `src/core/run.rs`) decides whether to log-warn-continue (best-effort) or bail. The current contract: `core::run::run` ALWAYS log-warn-continues — alert failure ≠ task failure (PROJECT.md hard line #5 "noisy" applies to task, not to alert delivery itself).
3. Bot token MUST come from either inline TOML `bot_token` (user-owned config file, chmod 600 responsibility on user) OR `bot_token_file` (path to `KEY=VAL` env-style file). The two are mutually exclusive at config validation time. No shell interpolation `${VAR}` pattern is supported — Worker MUST NOT add it.
4. Telegram API base URL is `https://api.telegram.org` (constant). `send_with_base(base, msg)` accepts an explicit base for production AND test-time override. **The API base test-seam env var (`ADVISORY_CRON_TG_API_BASE`) MUST be read at the call site in `src/core/run.rs`, NEVER inside `src/alert.rs`.** This keeps `alert.rs` a pure function of its inputs and unit-testable without env setup. Production code in `core/run.rs` reads the env var with `unwrap_or_else(|_| "https://api.telegram.org".to_string())` and passes the result to `send_with_base(&api_base, &msg)`.

**Why:** Telegram is the first outbound HTTP service in advisory-cron. INV-2 generic baseline ("external service call → timeout + error handling") applies but needs concrete teeth for this codebase. The log-warn-not-bail discipline is critical: silent failure is the bug advisory-cron exists to fix, but the failure we mean is *task* failure — not alert-delivery failure (network blip should not mask the underlying task failure that triggered the alert; heartbeat JSONL is the durable record, alert is the push channel). The env-free `alert.rs` rule (V2) keeps the library testable in isolation — unit tests don't need to set or unset env vars to exercise `send_with_base`.

**Implementation (Phase 2.1):** `src/alert.rs::TelegramAlert::send_with_base` — `reqwest::Client::builder().timeout(HTTP_TIMEOUT).build()?` + `tokio::time::timeout(HTTP_TIMEOUT, client.post(url).form(...).send()).await`. Caller in `src/core/run.rs` reads `std::env::var("ADVISORY_CRON_TG_API_BASE").unwrap_or_else(|_| "https://api.telegram.org".to_string())` and wraps `alert.send_with_base(&api_base, &msg).await` in `if let Err(e) = ... { tracing::warn!(...); }` — no `?` propagation.

**Trust boundary:** Bot token is a secret read from user config (chmod 600 responsibility on Sếp). advisory-cron does NOT log the token. POST body contains `chat_id` + `text` only — no token in body. URL contains token (Telegram API spec) — URL MUST NOT be logged at info/debug level. INV-19 forbids logging the full request URL.

**Trigger keywords:** `TelegramAlert::send_with_base` call sites, `reqwest::Client` + `api.telegram.org`, API base test-seam env var reads outside `core/run.rs` (forbidden — would violate env-free `alert.rs` rule), new alert backends (Slack, Discord, etc. would need parallel INV).

**Status:** Active.

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE; Giám sát soi PR diff for alert-related changes via INV-2 generic rubric + specific check that the API base env var does not appear in `src/alert.rs`.

---

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

**Implemented in Giám sát:** No (project-local). Worker self-checks during EXECUTE (8 unit tests for `is_retryable` + integration tests asserting exactly 1 alert POST after N retries). Giám sát soi PR diff for retry-related changes; if PR adds a second `alert.send` / `send_with_base` call site outside `core::run::run`, flag as INV-20 violation.

---

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

## How INV are checked

1. Worker pushes PR.
2. Quản đốc (or user) runs `/security-review <PR>` slash command.
3. Slash command captures diff via `gh pr diff` and spawns Giám sát.
4. Giám sát checks 5 generic INV (rubric baked in `.claude/agents/boundary-check.md`).
5. Project-local INV-6 → INV-9 are NOT auto-checked — they're documentation for human review during PR + Worker self-check during EXECUTE.
6. Slash command parses sentinel-wrapped verdict + posts as PR comment (silent if APPROVE + 0 FLAG).
7. **ADVISORY mode:** verdict does NOT block merge. Sếp/orchestrator gates.

## Why ADVISORY (not blocking)

- Generic INV at kit-level can over-flag (false positives in domain-specific code).
- Discipline > automation: Sếp reading the comment and deciding = stronger signal than CI-pass.
- Future: extend slash command to block on FLAGd INV — but kit ships ADVISORY default.

## Sentinel marker contract

Giám sát returns verdict wrapped in `<!-- security-review-start -->` ... `<!-- security-review-end -->`. These markers are LOAD-BEARING — slash command grep-extracts the block. DO NOT rename without phiếu.
