# BACKLOG — advisory-cron

> **Mục đích:** Single source of truth cho "Sếp nên làm gì tiếp theo".
> Idea mới → vào đây trước (qua `/idea` skill). Phiếu → chỉ viết cho item trong Active sprint.
> Wave-based, KHÔNG time-based. Sprint kết thúc khi xong hoặc Sếp đổi hướng.
>
> **Quy tắc Architect (Rule 0):** Architect chỉ viết phiếu cho item nằm trong **Active sprint** hoặc Sếp explicit move từ "Next sprint" lên.

---

## 🔥 Active sprint: TBD — pending Sếp Linux dogfood + Phase 3.5/4/5 decision

> **Status (2026-05-28):** Phase 3 sprint **code COMPLETE** — P012 → P015 đã merge main. Awaiting:
> 1. Sếp dogfood Linux 1 ngày trên WSL2 box hiện tại (register `/advisory-scan` daily 09:00, observe heartbeat append on fire, unregister cleanly), HOẶC
> 2. Sếp green-light next sprint (Phase 3.5 systemd timer / Phase 4 sos-kit promotion / Phase 5 Windows native / Phase 6 cargo publish — xem Future waves).
>
> First CI run pending observation per P014 DP4/DP5 (macOS GHA launchctl sandbox + Ubuntu crontab smoke). Tầng 2 follow-up nếu break.

---

## 🎯 Next sprint: TBD — pending Sếp pick

> Phase 3.5 (Linux systemd timer) / Phase 4 (sos-kit promotion) / Phase 5 (Windows native) / Phase 6 (cargo publish) — Sếp pick post-dogfood. Xem Future waves cho commitment level low.

---

## 🌊 Future waves (cam kết level low)

- **Phase 3.5** — Linux systemd timer impl (pick chỉ nếu Phase 3 dogfood lộ nhu cầu journald log / sandboxing / RandomizedDelaySec). Sẽ extend `Scheduler` trait existing — không refactor lại.
- **Phase 5** — Windows native (Task Scheduler / schtasks.exe XML). Pick chỉ khi Sếp chính thức dev/dogfood trên Windows host (KHÔNG phải WSL2). Sẽ extend `Scheduler` trait existing.
- **Phase 6** — `cargo publish` to crates.io (optional, if API stable + Sếp wants external users).

---

## 💡 Open backlog (chưa thuộc sprint)

- **[DEBT] `fire_task` no process timeout** (from PR#4 security review advisory note 2026-05-27). `runner::fire_task` uses `Command::new(...).output().await` without `tokio::time::timeout` wrapper — a hung child process (e.g. `claude -p` waiting on input) blocks the launchd job indefinitely. INV-14 in INVARIANTS.md already notes Phase 2+ deferral. Promote to phiếu only if dogfood reveals real hung-run incidents. Tầng 1 when picked up (adds config field `[task].timeout_secs` + tokio::time::timeout wiring).

- **[DEBT] core layer `is_valid_label` consolidation** (Phase 3.5+ Tầng 2) — from P014 V2 Worker CHALLENGE Turn 1 discovery (PR#14, 2026-05-28). `src/core/{register,unregister,status}.rs` each carry inline `is_valid_label` copy predating P013's scheduler-layer consolidation. 4 implementations kept consistent today (1 shared `scheduler/mod.rs::is_valid_label` + 3 inline core copies) but a Worker changing allowlist chars in only one location would silently desync 4 callsites. Refactor candidates: (a) import `scheduler::is_valid_label` from core layer, OR (b) extract to new `validation::label` module. INV-22 sub-rule 2 (INVARIANTS.md) explicitly documents 4-location reality + Phase 3.5+ deferral. Promote to phiếu when Phase 3.5 sprint opens.

---

## 🅿️ Park / nghĩ thêm

- **Web dashboard** — Sếp pre-empted reject in PROJECT.md non-goals. Move here if reconsidering 6 months later.

---

## ✅ Recently shipped

- **2026-05-28 — Sprint Phase 3 (P012-P015) shipped (4 phiếu total).** Linux support via cron-tab. See CHANGELOG.md 2026-05-28 entries for per-phiếu detail.
  - P012 — Phase 3.1 — Scheduler trait abstract (`LaunchctlClient` → cross-OS `Scheduler` trait, `cfg(target_os)` dispatch, Linux stub for P013)
  - P013 — Phase 3.2 — Linux cron-tab impl (`CrontabScheduler` sync `std::process::Command` per V2 pivot — Worker CHALLENGE Turn 1 caught nested-runtime panic + missing `io-util`)
  - P014 — Phase 3.3 — INV-22 + INV-23 formal entries + `.github/workflows/ci.yml` CREATE from scratch (2-OS matrix macos-latest + ubuntu-latest)
  - P015 — Phase 3.4 — README Linux quick-start (2-OS sections, Linux dogfood smoke verified WSL2 end-to-end)
  - **Cumulative state:** 143+ tests Linux (macOS-gated 11 lib + 4 integration skip on Linux), 23 INVs, 4.8MB Linux release binary, CI matrix shipped (first observation pending).
  - **Discoveries surfaced:** core-layer `is_valid_label` 4-location reality → `[DEBT]` logged in Open backlog (Phase 3.5+ Tầng 2 consolidation).

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
