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
