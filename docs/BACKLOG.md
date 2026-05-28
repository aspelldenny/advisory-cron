# BACKLOG — advisory-cron

> **Mục đích:** Single source of truth cho "Sếp nên làm gì tiếp theo".
> Idea mới → vào đây trước (qua `/idea` skill). Phiếu → chỉ viết cho item trong Active sprint.
> Wave-based, KHÔNG time-based. Sprint kết thúc khi xong hoặc Sếp đổi hướng.
>
> **Quy tắc Architect (Rule 0):** Architect chỉ viết phiếu cho item nằm trong **Active sprint** hoặc Sếp explicit move từ "Next sprint" lên.

---

## 🔥 Active sprint: Phase 3 — Linux support (cron-tab)

> **Mục tiêu:** Ship `advisory-cron register/unregister/run/status` chạy nguyên si trên Linux qua `crontab -l/-` injection. Heartbeat / runner / config / MCP / CLI surface KHÔNG đổi. Single binary giữ ≤7MB. Compile-time dispatch theo `#[cfg(target_os)]` — macOS path zero regression.
> **Kết thúc khi:** Phase 3 acceptance gate xanh (xem ngay dưới) + Sếp dogfood 1 ngày trên Linux box hiện tại (WSL2 `/usr/bin/crontab` đã sẵn, no setup).
> **Started:** 2026-05-28
> **Out of scope (deferred):** Windows native (Task Scheduler — Phase 5+ nếu Sếp dev Windows host), systemd timer (Phase 3.5 nếu dogfood lộ nhu cầu journald/sandboxing), Linux distro packaging (deb/rpm — Phase 4+).
>
> **Decision log (2026-05-28):**
> - Windows: defer (anh đang ở WSL2 = Linux, Windows native là AI-completeness-bias trừ khi anh thực sự dev/dogfood trên Windows host).
> - Linux scheduler: cron-tab only. WSL2 probe xác nhận `/usr/bin/crontab` sẵn + `systemd-pid1=NO` (systemd path require `/etc/wsl.conf` toggle + restart — quá nhiều setup tax cho zero benefit ở solo scope).

**Acceptance criteria (Phase 3 ship gate):**
- [ ] `advisory-cron register` trên Linux inject 1 dòng cron tagged `# advisory-cron: <label>` vào user crontab (no shell metachar leak — INV-22 mới).
- [ ] `advisory-cron unregister` xóa đúng dòng tagged, không động dòng cron khác của user (idempotent — chạy 2 lần = exit 0 cả 2).
- [ ] `advisory-cron run` + `heartbeat.jsonl` hoạt động y hệt macOS (cùng `runner.rs` + `heartbeat.rs`, zero diff).
- [ ] `advisory-cron status` trên Linux đọc next-fire từ cron expression (parse `M H * * *` → "daily at HH:MM") + last N heartbeats — render giống macOS.
- [ ] `advisory-cron mcp` trên Linux: 5 tools handshake + register/run/status callable từ Claude Desktop (config snippet update README).
- [ ] `cargo build --release` trên macOS: zero behavior change (NoopLaunchctl tests still pass, plist generation untouched).
- [ ] `cargo build --release` trên Linux: zero warnings, binary ≤7MB.
- [ ] `cargo test --all` trên Linux: pass (cross-OS test matrix — CI runner thêm Linux job).
- [ ] README quick-start có 2 paths (macOS launchd + Linux cron) với verification command (`crontab -l | grep advisory-cron` = Sub-mechanism A trigger gap check).

**Phiếu phác (Architect sẽ chốt khi DRAFT):**

- [ ] **Phase 3.1 — Scheduler trait abstract.** Tầng 1. Extract `LaunchctlClient` (`src/launchd.rs`) → `Scheduler` trait (`src/scheduler/mod.rs`) với methods `register(label, schedule, command) -> Result<RegisterReport>` / `unregister(label) -> Result<UnregisterReport>` / `print_status(label) -> Result<StatusReport>`. macOS impl move sang `src/scheduler/macos.rs` (re-export `LaunchctlClient` for backwards compat). Compile-time dispatch: `#[cfg(target_os = "macos")] use macos::MacosScheduler as PlatformScheduler;`. Zero behavior change macOS. Update `core::register::run` / `core::unregister::run` / `core::status::run` để inject `&dyn Scheduler` thay vì `&L: LaunchctlClient`. ~250 LOC refactor + 0 dep change.

- [ ] **Phase 3.2 — Linux cron-tab impl.** Tầng 1. New `src/scheduler/linux.rs`. `CrontabScheduler::register`: shell `crontab -l` → parse → append managed line `<cron_expr> <self_exe> run --config <path> # advisory-cron: <label>` → pipe back qua `crontab -` (using `tokio::process::Command` + stdin write). `unregister`: same flow, filter out tag line. `print_status`: grep tag from `crontab -l`. Heartbeat: cron line redirects stdout/stderr to `~/.local/state/advisory-cron/heartbeat-cron-<label>.log` (raw capture; `advisory-cron run` itself writes heartbeat JSONL — cron-level redirect chỉ là debug safety net). INV-22 label allowlist defense-in-depth: pre-flight reject `#`, `\n`, `'`, `"`, `;`, `|`, `&`, `$`, backtick. ~300 LOC + integration test using temp HOME + mock `crontab` binary in PATH (per `runner.rs` Phase 1.4 pattern).

- [ ] **Phase 3.3 — INVARIANTS + cross-OS CI matrix.** Tầng 1. INV-22 (crontab shell-out boundary — label allowlist 9-char blacklist enforced 2-point: pre-flight in `core::register::run` + defense-in-depth inside `CrontabScheduler::register`). INV-23 (cron expression validation — Linux accept full 5-field `M H DOM MON DOW`, macOS keep `M H * * *` daily form only per Phase 1.3 INV-11). GitHub Actions workflow `.github/workflows/ci.yml` extended với `os: [macos-latest, ubuntu-latest]` matrix; mỗi job chạy `cargo test --all`. ARCHITECTURE.md §Cron mechanism section split thành "macOS launchd" + "Linux cron-tab" subsections; new §Scheduler trait section. ~150 LOC docs + ~30 LOC INV.

- [ ] **Phase 3.4 — README + Quick-start Linux.** Tầng 2. README quick-start có 2 OS columns (hoặc 2 sequential sections). Linux quick-start verified end-to-end trên WSL2: `crontab -l | grep advisory-cron` after register → expect exactly 1 line; `advisory-cron unregister` → expect 0 lines. MCP server section ghi nhận Claude Desktop config nguyên bản (path absolute, OS-agnostic). Status banner Phase 3 ✅. ~90 min.

---

## 🎯 Next sprint: Phase 4 — sos-kit promotion + Linux packaging (DEFERRED)

> **Trigger:** Phase 3 dogfood xanh + Sếp confirm muốn share advisory-cron qua sos-kit recipes (2-3 repos khác).
> **Theme:** Distribution — copy binary vào `~/sos-kit/bin/`, bootstrap hook for new repos, `.deb` / `.rpm` Linux packaging optional.

---

## 🌊 Future waves (cam kết level low)

- **Phase 3.5** — Linux systemd timer impl (pick chỉ nếu Phase 3 dogfood lộ nhu cầu journald log / sandboxing / RandomizedDelaySec). Sẽ extend `Scheduler` trait existing — không refactor lại.
- **Phase 5** — Windows native (Task Scheduler / schtasks.exe XML). Pick chỉ khi Sếp chính thức dev/dogfood trên Windows host (KHÔNG phải WSL2). Sẽ extend `Scheduler` trait existing.
- **Phase 6** — `cargo publish` to crates.io (optional, if API stable + Sếp wants external users).

---

## 💡 Open backlog (chưa thuộc sprint)

- **[DEBT] `fire_task` no process timeout** (from PR#4 security review advisory note 2026-05-27). `runner::fire_task` uses `Command::new(...).output().await` without `tokio::time::timeout` wrapper — a hung child process (e.g. `claude -p` waiting on input) blocks the launchd job indefinitely. INV-14 in INVARIANTS.md already notes Phase 2+ deferral. Promote to phiếu only if dogfood reveals real hung-run incidents. Tầng 1 when picked up (adds config field `[task].timeout_secs` + tokio::time::timeout wiring).

---

## 🅿️ Park / nghĩ thêm

- **Web dashboard** — Sếp pre-empted reject in PROJECT.md non-goals. Move here if reconsidering 6 months later.

---

## ✅ Recently shipped

- **2026-05-27 — Sprint Phase 1 + Phase 2 (P001-P011) shipped (11 phiếu total).** See CHANGELOG.md 2026-05-27 sprint summary table for per-phiếu detail. Cumulative: 144 tests, 22 modules, 21 INVs, ~3.9MB release binary.
  - **Phase 1 — MVP launchd + MCP (7 phiếu):**
    - P001 — Phase 1.1 — CLI scaffold (5 subcommand stubs, clap derive)
    - P002 — Phase 1.2 — Config schema (TOML + serde, 3 blocks)
    - P003 — Phase 1.3 — launchd plist + `register`/`unregister`
    - P004 — Phase 1.4 — Task runner + heartbeat JSONL
    - P005 — Phase 1.5 — Status reporter (`launchctl print` parse + heartbeat tail)
    - P006 — Phase 1.7 — MCP server (stdio, rmcp SDK) + `core::*` extraction (CLI/MCP layering)
    - P007 — Phase 1.6 — README + ARCHITECTURE post-ship polish
  - **Phase 2 — Robust (3 phiếu):**
    - P008 — Phase 2.1 — Telegram alert on task failure (INV-19 outbound HTTP boundary)
    - P009 — Phase 2.2 — Retry policy (`is_retryable` + backoff loop, single-alert-per-invocation INV-20)
    - P010 — Phase 2.3 — Crash-safe heartbeat (temp+fsync+rename atomic protocol INV-21)
  - **Sprint debt cleanup (1 phiếu):**
    - P011 — Tầng 2 — INV-12 label sanitization 2-point enforcement tests + DISCOVERIES hook regex align

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
