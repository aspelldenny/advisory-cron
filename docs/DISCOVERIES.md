# DISCOVERIES — advisory-cron

> 1-line index per phiếu Discovery Report. Newest at top. Full reports in `docs/discoveries/P<NNN>.md`.
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/DISCOVERIES_ARCHIVE.md`.

---

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
