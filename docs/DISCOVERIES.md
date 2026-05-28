# DISCOVERIES — advisory-cron

> 1-line index per phiếu Discovery Report. Newest at top. Full reports in `docs/discoveries/P<NNN>.md`.
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/DISCOVERIES_ARCHIVE.md`.

---

- 2026-05-28 P013: Linux cron-tab sync stdlib impl shipped; V2 pivot saved nested-runtime panic + missing io-util feature (Worker CHALLENGE Turn 1 catch); mock crontab TempDir+PATH pattern for integration tests; `working_dir` still dead on Linux (`RegisterIntent` allow(dead_code) stays); dogfood smoke WSL2 verified register/unregister/idempotency/invalid-label all clean → see docs/discoveries/P013.md

- 2026-05-28 P012: Scheduler trait extracted (src/launchd.rs deleted → src/scheduler/{mod,macos,linux}.rs; PlatformScheduler compile-time alias; Linux WSL2 build 4.7MB clean; 129 tests Linux (11 macos-gated moved to macos.rs + 4 integration gated + 3 new linux stub tests); P013 watch-item: plist_path empty render on Linux when CrontabScheduler ships; INV-10/11/12/13/17 preserved; no schema/dep/CLI change) → see docs/discoveries/P012.md

- 2026-05-27 P011: Sprint debt cleanup shipped (INV-12 label sanitization pre-flight confirmed already in place in core::register::run + core::unregister::run — BACKLOG debt items 1+2 were stale; +3 named attack-class tests added for register pre-flight; .git/hooks/pre-commit DISCOVERIES regex aligned with CLAUDE.md doctrine list-item form, legacy H2 kept for backwards-compat; 141→144 tests; item 3 fire_task no-timeout stays deferred; no INV/schema/dep change) → see docs/discoveries/P011.md

## P010 — Crash-safe heartbeat (Phase 2.3) shipped 2026-05-27 — SPRINT COMPLETE

- 2026-05-27 P010: Crash-safe heartbeat Phase 2.3 shipped (temp+fsync+rename atomic protocol in `append`; `read_last_n` last-line-tolerate + mid-file-fail-loud; INV-21; tempfile dev→runtime; 8 new tests; 141 total; binary 3.9MB; sprint closes — Phase 1+2 all 10 phiếu shipped) → see docs/discoveries/P010.md

## P009 — Retry policy (Phase 2.2) shipped 2026-05-27

- 2026-05-27 P009: Retry policy Phase 2.2 shipped (RetryConfig + Config::retry field; is_retryable(exit_code) private fn; for-loop retry in core/run.rs; two-match heartbeat-completeness invariant preserved; alert moved outside loop (1 per invocation INV-20); heartbeat schema unchanged; 133 tests; 3.9MB binary; runner.rs test helper only structural change) → see docs/discoveries/P009.md

## P008 — Telegram alert on task failure shipped 2026-05-27

- 2026-05-27 P008: Telegram alert Phase 2.1 shipped (src/alert.rs env-free; AlertConfig + TelegramConfig config schema; core::run wired with env-var-at-call-site seam; INV-19; wiremock dev-dep; 116 tests total +22 new; binary 3.9MB; Constraint #1 + #11 satisfied) → see docs/discoveries/P008.md

## P007 — README + ARCHITECTURE post-ship docs polish 2026-05-27

- 2026-05-27 P007: README + ARCHITECTURE post-ship polish (6-step CLI quick-start; Sub-mechanism A verify step added; MCP smoke verified exit 0; "What advisory-cron fires" section; 0 schema drift; 22 modules confirmed; Phase 1 Code COMPLETE status) → see docs/discoveries/P007.md

## P006 — MCP server wrapper + core/* extraction shipped 2026-05-27

- 2026-05-27 P006: MCP server wrapper (rmcp 1.7.0 stdio; 5 tools; INV-18; core/* extraction for dual-surface parity; 94 tests; 2.1MB binary; ServerInfo/Implementation non-exhaustive structs require constructors; no lib.rs means integration tests are subprocess-only) → see docs/discoveries/P006.md

## P005 — status reporter shipped 2026-05-27

- 2026-05-27 P005: Status reporter shipped (status --label/--config/--json/--last; LaunchctlClient trait + print method; macOS 15 launchctl exposes NO next-fire timestamp — parse_next_fire pivoted to descriptor Hour/Minute → "daily at HH:MM"; INV-17 launchctl print shell-out boundary; 70 tests total, +19 new) → see docs/discoveries/P005.md

## P004 — task runner + heartbeat shipped 2026-05-27

- 2026-05-27 P004: Task runner + heartbeat shipped (serde_json explicit dep, runner::fire_task + heartbeat::append/read_last_n, run --config flag + bail-on-$HOME-unset, task.label optional config field, INV-14/15/16 appended; 51 tests total) → see docs/discoveries/P004.md

## P003 — launchd plist generator + register/unregister shipped 2026-05-27

- 2026-05-27 P003: launchd plist generator + register/unregister wired (newtype dispatch preserved — zero mod.rs edits; LaunchctlClient trait + NoopLaunchctl; idempotent unregister exit 0; simple `M H * * *` cron form only; zero new dep; dead_code annotation removed from Config::load; INVARIANTS.md INV-10..INV-13 appended) → see docs/discoveries/P003.md

## P002 — Config schema shipped 2026-05-27

- 2026-05-27 P002: Config schema (TOML + serde, 3 blocks, `#[serde(untagged)]` ScheduleConfig enum confirmed working, zero new dep); key finding: `pub fn load` needs `#[allow(dead_code)]` in binary crate until Phase 1.3 calls it → see docs/discoveries/P002.md

## P001 — CLI scaffold shipped 2026-05-27

- 2026-05-27 P001: CLI scaffold (5 subcommand stubs, clap derive, zero new dep); key finding: tokio `rt-multi-thread` feature absent → fixed `#[tokio::main(flavor = "current_thread")]` → see docs/discoveries/P001.md
