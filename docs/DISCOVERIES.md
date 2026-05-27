# DISCOVERIES — advisory-cron

> 1-line index per phiếu Discovery Report. Newest at top. Full reports in `docs/discoveries/P<NNN>.md`.
>
> **Soft cap:** 1000 lines. When exceeded, rotate older entries to `docs/Archive/DISCOVERIES_ARCHIVE.md`.

---

## P002 — Config schema shipped 2026-05-27

- 2026-05-27 P002: Config schema (TOML + serde, 3 blocks, `#[serde(untagged)]` ScheduleConfig enum confirmed working, zero new dep); key finding: `pub fn load` needs `#[allow(dead_code)]` in binary crate until Phase 1.3 calls it → see docs/discoveries/P002.md

## P001 — CLI scaffold shipped 2026-05-27

- 2026-05-27 P001: CLI scaffold (5 subcommand stubs, clap derive, zero new dep); key finding: tokio `rt-multi-thread` feature absent → fixed `#[tokio::main(flavor = "current_thread")]` → see docs/discoveries/P001.md
