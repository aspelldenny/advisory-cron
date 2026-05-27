# BACKLOG — advisory-cron

> **Mục đích:** Single source of truth cho "Sếp nên làm gì tiếp theo".
> Idea mới → vào đây trước (qua `/idea` skill). Phiếu → chỉ viết cho item trong Active sprint.
> Wave-based, KHÔNG time-based. Sprint kết thúc khi xong hoặc Sếp đổi hướng.
>
> **Quy tắc Architect (Rule 0):** Architect chỉ viết phiếu cho item nằm trong **Active sprint** hoặc Sếp explicit move từ "Next sprint" lên.

---

## 🔥 Active sprint: Phase 1 — MVP launchd plist fire + heartbeat + MCP wrapper

> **Mục tiêu:** Ship single binary `advisory-cron` chạy được trên macOS — register/unregister launchd plist, fire task on-demand, log heartbeat JSONL, show status, AND expose tất cả qua MCP server (stdio). Sếp dogfood end-to-end với `/advisory-scan` daily 09:00 ICT + Claude Desktop gọi được mọi tool qua MCP.
> **Kết thúc khi:** Acceptance criteria Phase 1 (PROJECT.md) tick xanh hết + Sếp confirm "đã quan sát launchd fire đúng lịch 3 ngày liên tiếp" + "đã gọi advisory-cron MCP tool từ Claude Desktop".
> **Started:** 2026-05-27
> **Scope expanded:** 2026-05-27 — Sếp re-defined ship-gate = CLI + MCP cùng ship. Phase 1.7 added.

- [ ] **[NEW]** **Phase 1.1 — Scaffold + CLI surface (clap derive).** Subcommands: `init`, `register`, `unregister`, `run`, `status`. Each subcommand returns proper exit code + help text. Empty implementations (panic with "not yet implemented") + happy-path test for `--help`. Tầng 1 (defines CLI contract for entire tool). ~150 LOC.

- [ ] **[NEW]** **Phase 1.2 — Config file (TOML + serde).** Schema: `[task]` block (command string, args list, working_dir), `[schedule]` block (cron expression OR launchd-friendly `{hour, minute}`), `[heartbeat]` block (log_path). `advisory-cron init` writes default config with placeholder Claude Code invocation. Validation on load: missing required fields → fail loud with helpful error. Tầng 1 (defines config schema — touched by every subcommand). ~200 LOC.

- [ ] **[NEW]** **Phase 1.3 — launchd plist generator.** Function `generate_plist(config) -> PlistContent`. Plist XML matches Apple's launchd schema (`ProgramArguments`, `StartCalendarInterval`, `StandardOutPath`, `StandardErrorPath`, `Label`). `register` subcommand writes plist to `~/Library/LaunchAgents/com.advisorycron.<label>.plist` then `launchctl bootstrap gui/$UID <path>`. `unregister` does inverse. Tầng 1 (touches user's LaunchAgents — must be careful). ~250 LOC + integration test using `tempfile`.

- [ ] **[NEW]** **Phase 1.4 — Task runner + heartbeat log.** Function `fire_task(config) -> RunResult { exit_code, stdout, stderr, duration }`. Uses `tokio::process::Command`. On completion, append 1 JSON line to `heartbeat.jsonl`: `{ts, label, exit_code, duration_ms, stdout_tail, stderr_tail}`. `run` subcommand invokes this once. Tầng 1 (defines heartbeat schema — durable contract for `status` + future Phase 2 alert). ~200 LOC.

- [ ] **[NEW]** **Phase 1.5 — Status reporter.** `status` subcommand: parse `launchctl print gui/$UID/<label>` for next fire time, read last N lines of `heartbeat.jsonl`, render to stdout (table or simple text). Handle "plist not loaded" + "no heartbeats yet" cases cleanly. Tầng 2 (no schema change, just rendering). ~100 LOC.

- [ ] **[NEW]** **Phase 1.7 — MCP server wrapper (stdio).** Subcommand `advisory-cron mcp` starts JSON-RPC 2.0 server over stdin/stdout. Exposes 5 tools 1-1 with CLI subcommands (`init`, `register`, `unregister`, `run`, `status`). Each tool's handler calls the SAME core function as its CLI counterpart (zero logic duplication — CLI layer + MCP layer both thin shells over `core::*` functions). MCP tool input schemas derived from clap args (or hand-written JSON schemas if `schemars` not pulled in). Includes README snippet for Claude Desktop `claude_desktop_config.json` registration. Architect MUST research Rust MCP SDK choice (likely `rmcp` official Anthropic crate — verify via context7 before specing). Tầng 1 (adds new dep + new public surface + may force refactor of phiếu 1.2-1.5 handlers to expose `core::*` instead of inline subcommand logic). ~300 LOC + handshake integration test.

- [ ] **[NEW]** **Phase 1.6 — README + ARCHITECTURE.md.** Update README quick-start with verified commands for BOTH CLI and MCP paths. Fill in ARCHITECTURE.md "Modules" section with per-module purpose + "Cron mechanism" section explaining launchd plist lifecycle + "MCP surface" section with tool schemas + Claude Desktop config example. Tầng 2 (docs only). ~90 min (raised from 60 to budget MCP coverage). **Runs AFTER 1.7** so docs reflect final shipped surface.

---

## 🎯 Next sprint: Phase 2 — Robust (Telegram alert + retry)

> **Trigger:** Phase 1 dogfood xanh 3 ngày liên tiếp.
> **Theme:** Resilience — fail-loud surface to Sếp's phone + retry transient errors.

- [ ] **Phase 2.1** — Telegram bot webhook POST on fail. Config `[alert.telegram]` block (bot_token, chat_id). Test with mock HTTP server. **Pre-req: ✅ secrets ready** at `~/.advisory-cron-secrets.env` chmod 600 (bot `@chiha_alert_bot`, chat_id `1184530337`, end-to-end test confirmed 2026-05-27 message_id=21).
- [ ] **Phase 2.2** — Retry policy. Config `[retry]` block (max_attempts, backoff_secs). Re-fire on transient failure (exit code 1-127 retryable; SIGTERM/SIGKILL not).
- [ ] **Phase 2.3** — State recovery. Crash-safe heartbeat write (write + fsync + rename). Recovery on next fire if previous run interrupted mid-write.

---

## 🌊 Future waves (cam kết level low)

- **Phase 3** — Linux support (systemd timer + cron-tab generation).
- **Phase 4** — sos-kit promotion (copy binary, bootstrap hook for new repos, add to `~/sos-kit/recipes/automation/`).
- **Phase 5** — `cargo publish` to crates.io (optional, if API stable + Sếp wants external users).

---

## 💡 Open backlog (chưa thuộc sprint)

(empty — populate via `/idea` skill or direct edit during dogfood)

---

## 🅿️ Park / nghĩ thêm

- **Web dashboard** — Sếp pre-empted reject in PROJECT.md non-goals. Move here if reconsidering 6 months later.

---

## ✅ Recently shipped

(empty until Phase 1 ships)

---

## ❌ Đã reject (lưu để khỏi nghĩ lại)

(empty)

---

## 📌 Quy tắc maintenance

1. **Idea mới** → `/idea` skill → tự append vào "Open backlog" hoặc "Active sprint" tùy phân loại.
2. **Phiếu xong** → move item từ Active sprint xuống "Recently shipped".
3. **Sprint xong** → tổng kết trong CHANGELOG.md, BACKLOG chỉ giữ 3 sprint gần nhất.
4. **Discovery debt** mới → từ DISCOVERIES.md → append vào "Open backlog" với prefix `[DEBT]`.
5. **Architect rule** (cứng): không viết phiếu cho item nằm ngoài "Active sprint".

---

*File này là LIVE. Sếp chỉnh trực tiếp được. Architect/Worker chỉ ĐỌC.*
