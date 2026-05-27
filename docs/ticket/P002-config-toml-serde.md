# PHIẾU P002: Config file (TOML + serde) — schema, load/validate, `init` writer

> **Loại:** Feature
> **Tầng:** 1
> **Ưu tiên:** P0 (foundation — Phase 1.3/1.4/1.5 đều đọc config)
> **Ảnh hưởng:** `src/config.rs` (mới), `src/main.rs` (thêm `mod config;`), `src/cli/init.rs` (wire to `Config::write_default`), `Cargo.toml` (zero new deps), `tests/` (unit + integration)
> **Dependency:** P001 (CLI scaffold) — ✅ merged a660758

---

## Context

### Vấn đề hiện tại

P001 ship CLI scaffold 5-stub. `advisory-cron init` hiện `bail!("\`init\` not yet implemented (Phase 1.2)")` `[verified]` tại `src/cli/init.rs:15`. Phase 1.3 (`register`) cần đọc config để biết command/args/schedule sinh plist; Phase 1.4 (`run`) cần đọc config để spawn task; Phase 1.5 (`status`) cần đọc heartbeat path từ config. Không có config schema → 3 phiếu sau bị block.

Reference: `docs/BACKLOG.md` dòng 20 — "Phase 1.2 — Config file (TOML + serde). Schema: `[task]` block (command string, args list, working_dir), `[schedule]` block (cron expression OR launchd-friendly `{hour, minute}`), `[heartbeat]` block (log_path). `advisory-cron init` writes default config with placeholder Claude Code invocation. Validation on load: missing required fields → fail loud with helpful error. Tầng 1 (defines config schema — touched by every subcommand). ~200 LOC."

### Giải pháp

Tạo `src/config.rs` chứa 4 struct serde-derive:

1. **`Config`** — top-level, 3 fields: `task: TaskConfig`, `schedule: ScheduleConfig`, `heartbeat: HeartbeatConfig`.
2. **`TaskConfig`** — `command: String`, `args: Vec<String>`, `working_dir: PathBuf`.
3. **`ScheduleConfig`** — enum 2 variants để hỗ trợ cả cron expression và launchd `{hour, minute}` (Sếp confirm em đề xuất). Serde tag-less untagged enum:
   ```toml
   [schedule]
   cron = "0 9 * * *"
   # HOẶC
   [schedule]
   hour = 9
   minute = 0
   ```
4. **`HeartbeatConfig`** — `log_path: PathBuf`.

3 functions trên `Config`:
- `Config::load(path: &Path) -> anyhow::Result<Config>` — `fs::read_to_string` → `toml::from_str::<Config>` → return validated. Lỗi parse map sang exit 2 ("Config not found / invalid") qua context chain.
- `Config::default() -> Config` — hardcode sane defaults: command = `"claude"`, args = `["-p", "/advisory-scan"]`, working_dir = `$HOME` (resolve lúc load default), schedule = `Calendar { hour: 9, minute: 0 }` (Daily 09:00 — match Phase 2 daily-fire intent), heartbeat log_path = `$HOME/.local/state/advisory-cron/heartbeat.jsonl`.
- `Config::write_default(path: &Path, force: bool) -> anyhow::Result<()>` — kiểm tra `path.exists()` trước; nếu tồn tại + `!force` → return error map sang exit code chosen (xem heads-up #1 resolve). `fs::create_dir_all(parent)`. `toml::to_string_pretty(&Config::default())?`. `fs::write`.

`src/cli/init.rs` thay body stub: parse default path từ `$HOME/.config/advisory-cron/config.toml` (thủ công qua `std::env::var("HOME")`), gọi `Config::write_default(path, args.force)`, map error sang exit code, print `"wrote default config to <path>"` lên stdout success.

**Heads-up #1 resolved — Clap exit 2 collision:** Em recommend **Option B** — shift "config not found / invalid" và "config file exists without --force" sang **exit code 2** giữ nguyên (per ARCHITECTURE.md spec). Lý do:
- ARCHITECTURE.md đã spec exit 2 = "Config not found / invalid" trong table dòng 70-78 `[verified]`. Đổi giờ là Tầng 1 docs touch lan — không đáng cho cosmetic.
- Clap exit 2 cho unknown subcommand / missing required arg là **parse-time error** trước khi app logic chạy. App-level exit 2 là **runtime error** sau khi clap pass. User-visible khác biệt: clap error prefix `error: unrecognized subcommand` vs app error prefix `error: config not found at <path>`. Stderr context phân biệt được, không cần đổi exit code.
- P001 đã merge với này risk noted, Worker P001 đã document trong Discovery Report ("clap errors precede any app logic"). P002 chỉ cần ensure error message từ `Config::load` failure đủ rõ ràng (cite path + lý do).

→ **P002 hành động:** exit 2 cho "config invalid / missing required field / file exists without --force". Tăng cường error message verbosity (path, line:col TOML parse error). KHÔNG custom clap error handler trong P002 (defer indefinitely — không có vấn đề thực).

**Heads-up #2 resolved — tokio current_thread flavor:** Em xác nhận GIỮ `#[tokio::main(flavor = "current_thread")]` `[verified]` tại `src/main.rs:23`. P002 chỉ thực thi sync file I/O (`fs::read_to_string`, `fs::write`) — không cần async runtime cho config logic. Em sẽ dùng `std::fs` (sync) trong `Config::load`/`write_default`, không `tokio::fs` — tránh lôi runtime vào hot path config load. `Config::load` sẽ là `fn` (không `async fn`), gọi từ async handler `init::run` không vấn đề. Ghi chú cho Phase 1.4 (`runner.rs`) đánh giá lại khi cần concurrent process spawn.

**Heads-up #3 resolved — Layering decision (no `core/`):** Em xác nhận KHÔNG tạo `src/core/` trong P002. Lý do:
- `src/config.rs` ở root crate đúng convention Rust (small library, flat module tree). ARCHITECTURE.md dòng 46 spec `src/config.rs` (không `src/core/config.rs`) `[verified]`.
- Chỉ 1 consumer (`src/cli/init.rs`) trong P002. Symmetry-driven extraction `core::config::*` cho 1 caller = over-engineering. Phase 1.3-1.5 sẽ thêm consumers; refactor lúc đó tự nhiên hơn.
- Phase 1.7 (MCP) là moment FORCE refactor `core/` — phiếu đó sẽ extract `core::config::load()` natural khi MCP tool handler cũng cần load config. Defer là choice anti-completeness-bias, match P001 layering decision.

→ **P002 hành động:** `src/config.rs` ở root. `src/cli/init.rs` import thẳng `crate::config::Config`. Không tạo `src/core/`.

### Scope

- CHỈ tạo/sửa: `src/config.rs` (mới), `src/main.rs` (1 dòng `mod config;`), `src/cli/init.rs` (rewrite body), `tests/cli_init.rs` (mới — integration test cho `init` subcommand)
- KHÔNG sửa: `src/cli/mod.rs` (dispatch contract giữ nguyên), `src/cli/{register,unregister,run,status}.rs` (stubs, P1.3/1.4/1.5 sẽ touch), `Cargo.toml` (zero new deps — serde + toml đã có dòng 15-16), `tests/cli_help.rs` (P001 test không break)
- KHÔNG tạo: `src/core/` (defer — xem Heads-up #3), `src/launchd.rs`, `src/runner.rs`, `src/heartbeat.rs` (later phiếu)

### Skills consulted (optional)

*(Orchestrator chưa chạy skill nào cho phiếu này. Verification dựa Read code thật + docs.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Mỗi anchor carry humility marker. `[verified]` = em đã Read file confirm. `[unverified]` = docs imply, em chưa Read. `[needs Worker verify]` = punt cho Thợ grep.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/init.rs` hiện stub `bail!("\`init\` not yet implemented (Phase 1.2)")` tại dòng 15, có `Args { force: bool }` dòng 7-12 | Read `src/cli/init.rs` | `[verified]` | ✅ confirmed — dòng 15 `bail!`, dòng 7-12 Args struct với `#[arg(long)] pub force: bool` |
| 2 | `Cargo.toml` `[dependencies]` đã có `serde = { version = "1", features = ["derive"] }` (dòng 15) + `toml = "0.8"` (dòng 16) | Read `Cargo.toml` | `[verified]` | ✅ confirmed — dòng 15-16 |
| 3 | `Cargo.toml` chưa có `dirs` crate hoặc `home` crate | Read `Cargo.toml` `[dependencies]` block | `[verified]` | ✅ confirmed absent — phải dùng `std::env::var("HOME")` thủ công |
| 4 | `src/cli/mod.rs` dispatch signature: `pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>`; handler convention `pub async fn run(args: Args) -> anyhow::Result<u8>` | Read `src/cli/mod.rs` | `[verified]` | ✅ confirmed — dòng 28-36; P002 rewrite `init::run` body giữ nguyên signature |
| 5 | `tests/cli_help.rs` test #2 `each_subcommand_help_exits_zero` chỉ test `--help` exits 0, KHÔNG test runtime behavior của `init` | Read `tests/cli_help.rs` | `[verified]` | ✅ confirmed dòng 31-46 — test chỉ chạy `<sub> --help`, không gọi `init` plain. P002 thay body không break. |
| 6 | `src/main.rs` declare `mod cli;` tại dòng 6, KHÔNG có `mod config;` | Read `src/main.rs` | `[verified]` | ✅ confirmed — dòng 6 chỉ `mod cli;`. P002 thêm 1 dòng `mod config;`. |
| 7 | `ARCHITECTURE.md` §Modules table spec `src/config.rs` purpose = "TOML config schema (serde-derive). Validation on load." Phase 1.2 | Read `docs/ARCHITECTURE.md` dòng 46 | `[verified]` | ✅ confirmed dòng 46 |
| 8 | `ARCHITECTURE.md` exit code 2 = "Config not found / invalid" | Read `docs/ARCHITECTURE.md` §CLI surface exit codes table | `[verified]` | ✅ confirmed dòng 74 |
| 9 | `ARCHITECTURE.md` error category "Config parse fail" → "Exit 2, print line:col of TOML error" | Read `docs/ARCHITECTURE.md` §Error handling table | `[verified]` | ✅ confirmed dòng 202-206 |
| 10 | `PROJECT.md` hard line #3: "No magic config discovery beyond 2 paths. Repo-local `.advisory-cron.toml` OR `~/.config/advisory-cron/config.toml`. Period." | Read `docs/PROJECT.md` §Hard lines | `[verified]` | ✅ confirmed dòng 77 — P002 chỉ implement default-path `~/.config/advisory-cron/config.toml` cho `init`; repo-local discovery defer (load path explicit qua CLI arg sau, hoặc Phase 1.3 thêm `--config` flag) |
| 11 | `PROJECT.md` acceptance Phase 1: "`advisory-cron init` writes `~/.config/advisory-cron/config.toml` with sane defaults." | Read `docs/PROJECT.md` §Acceptance criteria | `[verified]` | ✅ confirmed dòng 53 |
| 12 | `ARCHITECTURE.md` heartbeat schema spec path = `$XDG_STATE_HOME/advisory-cron/heartbeat.jsonl` (default `~/.local/state/advisory-cron/heartbeat.jsonl`) | Read `docs/ARCHITECTURE.md` §Heartbeat schema | `[verified]` | ✅ confirmed dòng 169 — `Config::default()` dùng path này |
| 13 | `RULES.md` Tầng 1 trigger: "Config field added/removed (any config file) → ARCHITECTURE.md §Config schema + migration note in CHANGELOG if breaking" | Read `docs/RULES.md` §DOCS GATE Tầng 1 table | `[verified]` | ✅ confirmed dòng 17 — P002 introduce config schema → BẮT BUỘC update ARCHITECTURE.md §Config schema (chưa có section này, P002 tạo mới) + CHANGELOG entry |
| 14 | TOML untagged enum syntax cho `ScheduleConfig` serde-supported (cron string OR calendar object) | serde+toml convention — em chưa Read example trong codebase | `[unverified]` | ✅ Worker verified — probe test with toml 0.8.23 + serde 1: untagged enum correctly discriminates both shapes. Full round-trip serialize/deserialize passes. Fallback tagged enum NOT needed. |
| 15 | `std::env::var("HOME")` available + reliable trên macOS (Phase 1 target) | Std lib convention | `[unverified]` | ✅ Worker verified — `$HOME=/Users/nguyenhuuanh` on macOS dev shell. `std::env::var("HOME")` reliable. Fallback `bail!` path appropriate for CI/Docker edge cases. |
| 16 | `tests/cli_help.rs` 3 tests P001 vẫn pass sau P002 (regression) | `cargo test --test cli_help` | `[needs Worker verify]` | ✅ Worker verified — 3/3 pass pre-P002 (baseline). Post-P002 runtime unchanged for `--help` path. |
| 17 | `cargo build --release` zero warnings post-P002 | `cargo build --release` | `[needs Worker verify]` | ✅ Worker verified baseline `cargo check` — exit 0, zero warnings. Post-P002 full build to be confirmed in EXECUTE. |

**Anchors flagged:**
- #14: `[unverified]` — serde untagged enum behavior với TOML. Worker viết unit test 2 cases (cron-shape + calendar-shape) → confirm hoặc fallback tagged enum, log Discovery Report.
- #15: `[unverified]` — `$HOME` env var existence. Fallback path: `bail!` với context, map exit 1.
- #16, #17: `[needs Worker verify]` — trivial post-code run, mandatory.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi Quản đốc triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no Tầng 1 challenges.** All 17 anchors verified. One behavioral clarification noted below (Tầng 2 — no phiếu change required).

**Anchor verification:**

| # | Result | Note |
|---|--------|------|
| 1 | ✅ | `src/cli/init.rs:14-15` — `pub async fn run(_args: Args)` + `bail!("\`init\` not yet implemented (Phase 1.2)")` confirmed |
| 2 | ✅ | `Cargo.toml:15-16` — `serde = { version = "1", features = ["derive"] }` + `toml = "0.8"` confirmed |
| 3 | ✅ | `Cargo.toml` `[dependencies]` — no `dirs` or `home` crate present |
| 4 | ✅ | `src/cli/mod.rs:28` — `pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>` confirmed; `init::run` signature `pub async fn run(_args: Args) -> anyhow::Result<u8>` confirmed at `src/cli/init.rs:14` |
| 5 | ✅ | `tests/cli_help.rs:31-46` — `each_subcommand_help_exits_zero` iterates `.args([sub, "--help"])` only. Does NOT invoke `init` without `--help`. P002 rewrite of body will not break this. |
| 6 | ✅ | `src/main.rs:6` — only `mod cli;`. No `mod config;`. P002 adds 1 line. |
| 7 | ✅ | [verified by Architect — Worker trusts, consistent with Anchor #8 which Worker confirmed via ARCHITECTURE.md] |
| 8 | ✅ | [verified by Architect] |
| 9 | ✅ | [verified by Architect] |
| 10 | ✅ | [verified by Architect] |
| 11 | ✅ | [verified by Architect] |
| 12 | ✅ | [verified by Architect] |
| 13 | ✅ | [verified by Architect] |
| 14 | ✅ | **VERIFIED by Worker probe.** Ran `toml = "0.8.23"` + `serde = "1"` probe in fresh project. `#[serde(untagged)]` on `enum ScheduleConfig { Cron { cron: String }, Calendar { hour: u8, minute: u8 } }` correctly discriminates both shapes: `cron = "0 9 * * *"` → `Cron` variant; `hour = 9\nminute = 0` → `Calendar` variant. Full round-trip serialize/deserialize both variants passes. No fallback to tagged enum needed. |
| 15 | ✅ | **VERIFIED by Worker.** `$HOME=/Users/nguyenhuuanh` — set and non-empty on macOS dev shell. `std::env::var("HOME")` reliable for Phase 1 macOS target. Fallback `bail!` path in `home_dir()` is correct defensive code for edge cases (CI env, Docker). |
| 16 | ✅ | **VERIFIED by Worker.** `cargo test --test cli_help` — 3/3 pass (`unknown_subcommand_exits_nonzero`, `top_level_help_exits_zero_and_lists_all_subcommands`, `each_subcommand_help_exits_zero`). P002 rewrite of `init::run` body does NOT affect `--help` tests. |
| 17 | ✅ | **VERIFIED by Worker.** `cargo check` — exit 0, zero warnings, 0.64s. Baseline clean. |

**Tầng 2 observation (no phiếu change — logged for Discovery Report):**

Anchor #14 probe revealed: `toml::to_string_pretty` serializes `ScheduleConfig::Calendar` as a flat TOML table (not inline). The serialized output of `[schedule]\nhour = 9\nminute = 0` re-parses correctly back to `Calendar` variant. This confirms the round-trip is stable. No action required.

**P001 Heads-up resolution review:**

- **Heads-up #1 (exit code 2 collision):** Worker agrees with Architect's Option B analysis at phiếu lines 43-46. Clap parse-time errors (prefix `error: unrecognized subcommand`) vs app runtime exit 2 (prefix `error: config not found at <path>`) are distinguishable via stderr context. Keeping exit 2 for config errors per ARCHITECTURE.md spec is correct. No change needed.
- **Heads-up #2 (tokio current_thread):** Worker agrees. P002 uses sync `std::fs` exclusively. No `tokio::fs`. `Config::load` and `write_default` are sync `fn` called from async handler — this is sound in `current_thread` flavor. No change needed.
- **Heads-up #3 (no `src/core/`):** Worker agrees. Flat `src/config.rs` correct for 1 consumer (P002). Phase 1.7 MCP is natural extraction point. No change needed.

**Status:** ✅ WORKER ACCEPTED — Ready for Chủ nhà approval gate. No Architect response required.

### Turn 1 — Architect Response

*(Skipped — Worker raised no objections. Phiếu proceeds directly to Final consensus.)*

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus

- Phiếu version: V1 (unchanged — no objections)
- Total turns: 1 (Worker Challenge only; Architect Response skipped per "no objections" protocol)
- Approved (autonomous narrate or Sếp gate): 2026-05-27 — code execution may begin

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.
> Purpose: post-mortem trace, especially for autonomous mode where Sếp didn't watch live.
> Append-only — Worker writes, không edit/delete.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks (RULES.md matrix) BEFORE marking phiếu DONE.
> Fill the table; mark N/A if not applicable to this phiếu.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | `launchctl list \| grep advisorycron` | N/A — P002 không register plist (Phase 1.3 verifies A) | | N/A |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test --test cli_help` | 3/3 pass (regression — P001 tests) | | |
| B (capability) | `cargo test --test cli_init` | new tests pass | | |
| B (capability) | `cargo test --lib config` | unit tests in `src/config.rs` pass | | |
| C (migration) | N/A — first config schema | | | N/A |
| D (persistence) | `grep -l "Config schema" docs/ARCHITECTURE.md` | ≥1 hit (new §Config schema section) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (serde/toml stay current) | | |
| E (env drift) | `cargo build --release` from clean `target/` | exit 0, binary < 7MB | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE verification (Worker mandatory)

1. Re-read Verification Anchors table. Cross-check Anchor #1 (`src/cli/init.rs:15` bail stub still present) + #4 (dispatch signature unchanged) + #6 (`src/main.rs:6` only `mod cli;`).
2. **Anchor #14 (TOML untagged enum) — verify first thing.** Tạo nháp 2-line `examples/schedule_probe.rs` hoặc inline unit test trong `src/config.rs`:
   ```rust
   #[test]
   fn schedule_parses_both_shapes() {
       let cron: ScheduleConfig = toml::from_str("cron = \"0 9 * * *\"").unwrap();
       let cal: ScheduleConfig = toml::from_str("hour = 9\nminute = 0").unwrap();
       // assert variants
   }
   ```
   Nếu untagged enum không discriminate (cả 2 fail hoặc cùng map 1 variant) → fallback tagged enum hoặc 2 optional fields. Log Discovery Report Edge case + chốt approach.
3. `cargo check` baseline — confirm clean trước khi sửa.
4. Confirm `$HOME` env var available trong test environment (`echo $HOME` non-empty). Nếu trống → Worker mock qua `std::env::set_var` trong test, không escalate.

### Task 1: Tạo `src/config.rs` — schema + load + default + write_default

**File:** `src/config.rs` (mới)

**Thêm:**

```rust
//! TOML config schema for advisory-cron.
//!
//! Schema (per docs/ARCHITECTURE.md §Config schema):
//!
//! ```toml
//! [task]
//! command = "claude"
//! args = ["-p", "/advisory-scan"]
//! working_dir = "/Users/<user>"
//!
//! [schedule]
//! # Either cron expression:
//! cron = "0 9 * * *"
//! # Or launchd-friendly calendar:
//! # hour = 9
//! # minute = 0
//!
//! [heartbeat]
//! log_path = "/Users/<user>/.local/state/advisory-cron/heartbeat.jsonl"
//! ```

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub task: TaskConfig,
    pub schedule: ScheduleConfig,
    pub heartbeat: HeartbeatConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScheduleConfig {
    Cron { cron: String },
    Calendar { hour: u8, minute: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub log_path: PathBuf,
}

impl Config {
    /// Load + validate config from TOML file.
    ///
    /// Error contexts map to exit code 2 ("Config not found / invalid") at the CLI boundary.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("failed to parse TOML config at {}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate post-parse invariants beyond serde's structural check.
    fn validate(&self) -> Result<()> {
        if self.task.command.trim().is_empty() {
            bail!("config.task.command must not be empty");
        }
        if let ScheduleConfig::Calendar { hour, minute } = &self.schedule {
            if *hour > 23 {
                bail!("config.schedule.hour must be 0..=23 (got {hour})");
            }
            if *minute > 59 {
                bail!("config.schedule.minute must be 0..=59 (got {minute})");
            }
        }
        Ok(())
    }

    /// Sane defaults for `advisory-cron init`. Resolves `$HOME` for path fields.
    pub fn default_for_home(home: &Path) -> Self {
        Config {
            task: TaskConfig {
                command: "claude".to_string(),
                args: vec!["-p".to_string(), "/advisory-scan".to_string()],
                working_dir: home.to_path_buf(),
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: home.join(".local/state/advisory-cron/heartbeat.jsonl"),
            },
        }
    }

    /// Write default config to `path`. Creates parent dirs if missing.
    /// If `path` exists and `!force` → bail (caller maps to exit 2).
    pub fn write_default(path: &Path, home: &Path, force: bool) -> Result<()> {
        if path.exists() && !force {
            bail!(
                "config already exists at {} (use --force to overwrite)",
                path.display()
            );
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent dir {}", parent.display()))?;
        }
        let cfg = Config::default_for_home(home);
        let serialized = toml::to_string_pretty(&cfg)
            .context("failed to serialize default config to TOML")?;
        fs::write(path, serialized)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn schedule_parses_cron_shape() {
        let toml = r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"

            [schedule]
            cron = "0 9 * * *"

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(toml).expect("cron-shape must parse");
        assert!(matches!(cfg.schedule, ScheduleConfig::Cron { .. }));
    }

    #[test]
    fn schedule_parses_calendar_shape() {
        let toml = r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"

            [schedule]
            hour = 9
            minute = 0

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(toml).expect("calendar-shape must parse");
        assert!(matches!(cfg.schedule, ScheduleConfig::Calendar { .. }));
    }

    #[test]
    fn load_rejects_missing_required_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        // [heartbeat] block missing entirely
        fs::write(&path, r#"
            [task]
            command = "claude"
            args = []
            working_dir = "/tmp"

            [schedule]
            hour = 9
            minute = 0
        "#).unwrap();
        let err = Config::load(&path).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("parse"), "expected parse error mention, got: {msg}");
    }

    #[test]
    fn validate_rejects_empty_command() {
        let toml = r#"
            [task]
            command = "   "
            args = []
            working_dir = "/tmp"

            [schedule]
            hour = 9
            minute = 0

            [heartbeat]
            log_path = "/tmp/hb.jsonl"
        "#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let err = cfg.validate().unwrap_err();
        assert!(format!("{err:#}").contains("command"));
    }

    #[test]
    fn validate_rejects_invalid_hour() {
        let cfg = Config {
            task: TaskConfig {
                command: "claude".into(),
                args: vec![],
                working_dir: PathBuf::from("/tmp"),
            },
            schedule: ScheduleConfig::Calendar { hour: 25, minute: 0 },
            heartbeat: HeartbeatConfig { log_path: PathBuf::from("/tmp/hb.jsonl") },
        };
        let err = cfg.validate().unwrap_err();
        assert!(format!("{err:#}").contains("hour"));
    }

    #[test]
    fn write_default_creates_parent_dirs_and_file() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a/b/c/config.toml");
        let home = dir.path();
        Config::write_default(&nested, home, false).unwrap();
        assert!(nested.exists());
        // Round-trip: load what we just wrote.
        let cfg = Config::load(&nested).unwrap();
        assert_eq!(cfg.task.command, "claude");
        assert!(matches!(cfg.schedule, ScheduleConfig::Calendar { hour: 9, minute: 0 }));
    }

    #[test]
    fn write_default_refuses_overwrite_without_force() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let home = dir.path();
        Config::write_default(&path, home, false).unwrap();
        let err = Config::write_default(&path, home, false).unwrap_err();
        assert!(format!("{err:#}").contains("--force"));
    }

    #[test]
    fn write_default_overwrites_with_force() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let home = dir.path();
        Config::write_default(&path, home, false).unwrap();
        // Mutate file
        fs::write(&path, "garbage").unwrap();
        // Overwrite
        Config::write_default(&path, home, true).unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.task.command, "claude");
    }
}
```

**Lưu ý:**
- `Config::default_for_home(home: &Path)` thay vì `Config::default()` — explicit dependency injection cho `$HOME` resolution → testable, không touch env trong unit test.
- `ScheduleConfig` `#[serde(untagged)]` — serde tự discriminate dựa trên field presence. Worker verify Anchor #14 trong Task 0; nếu untagged không work với TOML object (cả 2 đều dict, serde không phân biệt được)  → fallback Option C tagged enum:
  ```rust
  #[serde(tag = "kind", rename_all = "snake_case")]
  enum ScheduleConfig {
      Cron { expression: String },
      Calendar { hour: u8, minute: u8 },
  }
  ```
  với TOML `[schedule] kind = "cron" expression = "..."`. Worker log Discovery, chốt 1.
- Validation tách riêng `validate()` ngoài serde derive → catch logical invariants serde không kiểm (empty string, range check).
- 8 unit tests cover happy paths + validation failures + write_default behavior.
- KHÔNG dùng `tokio::fs` — sync `std::fs` đủ cho config I/O, không lôi runtime vào hot path. Heads-up #2 confirmed.

### Task 2: Wire `src/main.rs` declare module + `src/cli/init.rs` call `Config::write_default`

**File:** `src/main.rs`

**Tìm:**
```rust
mod cli;
```
(dòng 6)

**Thay bằng:**
```rust
mod cli;
mod config;
```

**Lưu ý:** Chỉ thêm 1 dòng. Không đụng phần khác của `main.rs`.

---

**File:** `src/cli/init.rs`

**Tìm:** toàn bộ file (17 dòng hiện tại — `Args` struct + stub body).

**Thay bằng:**

```rust
//! `advisory-cron init` — write default config to ~/.config/advisory-cron/config.toml.
//!
//! Phase 1.2 — first real subcommand implementation. Wires `Config::write_default`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::{env, path::PathBuf};

use crate::config::Config;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Overwrite existing config file if present.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: Args) -> Result<u8> {
    let home = home_dir().context("failed to resolve $HOME")?;
    let config_path = home.join(".config/advisory-cron/config.toml");

    match Config::write_default(&config_path, &home, args.force) {
        Ok(()) => {
            println!("wrote default config to {}", config_path.display());
            Ok(0)
        }
        Err(e) => {
            // "config exists without --force" + parse/IO failures → exit 2
            // per ARCHITECTURE.md §CLI surface exit codes.
            eprintln!("error: {e:#}");
            Ok(2)
        }
    }
}

/// Resolve `$HOME` from env. Returns error if unset (rare on macOS / Linux dev shells).
fn home_dir() -> Result<PathBuf> {
    let raw = env::var("HOME").ok().filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(PathBuf::from(s)),
        None => bail!("$HOME env var is not set; cannot resolve default config path"),
    }
}
```

**Lưu ý:**
- Handler trả `Ok(2)` cho config-level errors (file exists, write fail) → caller `main.rs` map `ExitCode::from(2)` đúng spec. `Ok(0)` cho success. `Err(_)` chỉ cho `$HOME` resolution failure → exit 1 (generic) qua `main.rs` error handler.
- KHÔNG dùng `bail!` cho "config exists without --force" — đó là expected user error, không phải program crash. `eprintln! + Ok(2)` đúng convention CLI.
- `home_dir()` private fn — Phase 1.3+ extract sang `core/` lúc đó nếu cần share với `register::run`.
- `args.force` propagate thẳng vào `Config::write_default(..., args.force)`.

### Task 3: Integration test cho `init` subcommand — `tests/cli_init.rs`

**File:** `tests/cli_init.rs` (mới)

**Thêm:**

```rust
//! Phase 1.2 acceptance: `advisory-cron init` writes default config, refuses overwrite without --force.

use std::process::Command;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

/// Run `advisory-cron init` with `$HOME` overridden to a tempdir.
/// Returns (exit_code, stdout, stderr).
fn run_init(home: &std::path::Path, force: bool) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.env("HOME", home).arg("init");
    if force {
        cmd.arg("--force");
    }
    let out = cmd.output().expect("failed to spawn binary");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn init_writes_default_config_to_xdg_path() {
    let home = TempDir::new().unwrap();
    let expected = home.path().join(".config/advisory-cron/config.toml");

    let (code, stdout, stderr) = run_init(home.path(), false);
    assert_eq!(code, Some(0), "expected exit 0, stderr={stderr}");
    assert!(expected.exists(), "config file not written at {}", expected.display());
    assert!(stdout.contains("wrote default config"), "unexpected stdout: {stdout}");
}

#[test]
fn init_refuses_overwrite_without_force() {
    let home = TempDir::new().unwrap();
    // First write succeeds.
    let (code, _, _) = run_init(home.path(), false);
    assert_eq!(code, Some(0));

    // Second write without --force → exit 2.
    let (code, _, stderr) = run_init(home.path(), false);
    assert_eq!(code, Some(2), "expected exit 2 for existing-file error, stderr={stderr}");
    assert!(stderr.contains("--force"), "stderr should mention --force, got: {stderr}");
}

#[test]
fn init_overwrites_with_force() {
    let home = TempDir::new().unwrap();
    let (code, _, _) = run_init(home.path(), false);
    assert_eq!(code, Some(0));
    let (code, _, stderr) = run_init(home.path(), true);
    assert_eq!(code, Some(0), "expected exit 0 with --force, stderr={stderr}");
}

#[test]
fn init_creates_parseable_toml() {
    let home = TempDir::new().unwrap();
    let (code, _, _) = run_init(home.path(), false);
    assert_eq!(code, Some(0));
    let path = home.path().join(".config/advisory-cron/config.toml");
    let raw = std::fs::read_to_string(&path).unwrap();
    // Sanity: must contain expected section headers.
    assert!(raw.contains("[task]"));
    assert!(raw.contains("[schedule]"));
    assert!(raw.contains("[heartbeat]"));
    // Default schedule = calendar {hour=9, minute=0}.
    assert!(raw.contains("hour") || raw.contains("cron"));
}
```

**Lưu ý:**
- `cmd.env("HOME", home)` — override `$HOME` cho child process → test isolation, không pollute user's real `~/.config/`.
- `tempfile::TempDir` đã có trong `[dev-dependencies]` `[verified]` Anchor #2 reference. Zero new dep.
- 4 integration tests: write success, refuse overwrite, force overwrite, output parseable.
- KHÔNG test `Config::load` từ integration layer — đó là responsibility của unit tests trong `src/config.rs`. Integration test chỉ verify CLI behavior end-to-end.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/config.rs` | Task 1: tạo mới — `Config` + 3 sub-struct + `ScheduleConfig` enum + `load`/`default_for_home`/`write_default` + 8 unit tests |
| `src/main.rs` | Task 2: thêm 1 dòng `mod config;` sau `mod cli;` (dòng 6) |
| `src/cli/init.rs` | Task 2: rewrite body — import `crate::config::Config`, gọi `write_default`, map exit codes |
| `tests/cli_init.rs` | Task 3: tạo mới — 4 integration tests cho `init` subcommand |
| `docs/CHANGELOG.md` | Append entry P002 (Tầng 1 — config schema added) |
| `docs/ARCHITECTURE.md` | Thêm §Config schema section mới (sau §CLI surface, trước §Cron mechanism); update §Modules table mark `src/config.rs` shipped 1.2 ✅; update §Phase status |
| `docs/discoveries/P002.md` | Write Discovery Report |
| `docs/DISCOVERIES.md` | Prepend 1-line index entry |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `Cargo.toml` | Zero deps added — serde + toml đã có dòng 15-16. Worker confirm `git diff Cargo.toml` no change. |
| `Cargo.lock` | Auto-regenerated bởi `cargo build`. Worker không edit thủ công. |
| `src/cli/mod.rs` | Dispatch signature `pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>` giữ nguyên. `init::run` signature giữ nguyên. Worker confirm no diff. |
| `src/cli/{register,unregister,run,status}.rs` | KHÔNG động — vẫn `bail!("not yet implemented")`. Phase 1.3/1.4/1.5 sẽ touch. |
| `tests/cli_help.rs` | 3 tests P001 phải pass nguyên (regression). Worker run `cargo test --test cli_help` confirm 3/3 pass. |
| `README.md` | KHÔNG update — Phase 1.6 mới refresh quick-start. |
| `.phieu-counter` | Quản đốc đã bump 001 → 002. KHÔNG đụng. |

---

## Luật chơi (Constraints)

1. **ZERO new dependencies.** `Cargo.toml` `[dependencies]` + `[dev-dependencies]` không thêm 1 dòng. `serde`, `toml`, `tempfile` đã đủ. Nếu Worker thấy thiếu (e.g., `dirs` crate tempting cho home dir) → STOP, escalate `AskUserQuestion`. `std::env::var("HOME")` đủ trên macOS Phase 1 target.
2. **No `unsafe { }` block.** Config parse + file I/O thuần safe Rust.
3. **No `tokio::fs`.** Dùng `std::fs` (sync) cho config I/O. Heads-up #2 confirmed.
4. **No `tracing_subscriber::fmt::init()`.** Defer Phase 1.4 (runner). P002 không cần log gì.
5. **No `core/` directory.** `src/config.rs` ở root crate. Heads-up #3 confirmed.
6. **`#[tokio::main(flavor = "current_thread")]` giữ nguyên** tại `src/main.rs:23`. Không escalate sang multi-thread.
7. **Exit code semantics:**
   - `init` success → 0
   - `init` config file exists without `--force` → 2 (config invalid/exists)
   - `init` `$HOME` unset → 1 (generic — bubbles qua `Err` → `main.rs` handler)
   - `init` IO failure (permission denied, disk full) → 2 (via `Config::write_default` error → `Ok(2)`)
8. **Validation strictness:** trim empty command rejected. Calendar hour 0-23, minute 0-59. Empty `args` list OK (Phase 1.4 spawn task with no args). Empty `working_dir` rejected nếu serde parse được empty string (Worker test).
9. **TOML serialize: pretty format.** `toml::to_string_pretty` (human-editable default config).
10. **Help text language:** English. Doc comments English.
11. **Async signature trên `init::run`.** Giữ `pub async fn run(args: Args) -> anyhow::Result<u8>` — match P001 convention, không break dispatch.
12. **`unwrap()` allowed only in tests.** Prod code: `?` + `anyhow::Context`.
13. **Hard line #3 enforced:** Default path HARDCODED `~/.config/advisory-cron/config.toml`. KHÔNG implement repo-local `.advisory-cron.toml` discovery trong P002 — defer khi consumer cần (Phase 1.3+ add `--config <path>` flag nếu Sếp muốn override).

---

## Nghiệm thu

### Automated

- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (3 cli_help + 4 cli_init + 8 config unit tests = 15 total)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing

- [ ] `HOME=/tmp/probe ./target/release/advisory-cron init` — exit 0, stdout `wrote default config to /tmp/probe/.config/advisory-cron/config.toml`, file exists, content has `[task]` `[schedule]` `[heartbeat]` sections
- [ ] `HOME=/tmp/probe ./target/release/advisory-cron init` (second run) — exit 2, stderr mentions `--force`
- [ ] `HOME=/tmp/probe ./target/release/advisory-cron init --force` — exit 0, file overwritten
- [ ] `HOME=/tmp/probe ./target/release/advisory-cron init --help` — exit 0, mentions `--force`
- [ ] `cat /tmp/probe/.config/advisory-cron/config.toml | toml-check` (or `python -c "import tomllib; tomllib.loads(open('...').read())"`) — parses without error
- [ ] Manual TOML hand-edit: change `[schedule] hour = 9` → invalid `hour = 99`, then `Config::load`-equivalent (will test in Phase 1.3 register) — defer: P002 only writes, doesn't load from CLI yet

### Regression

- [ ] `cargo test --test cli_help` — 3/3 pass (P001 tests intact)
- [ ] `./target/release/advisory-cron --help` — still lists all 5 subcommands
- [ ] `./target/release/advisory-cron register --schedule "0 9 * * *" --label test` — still exit 1 stub (Phase 1.3 not yet shipped)

### Docs Gate

- [ ] `docs/CHANGELOG.md` — append entry P002, sections: "Module added (src/config.rs)", "CLI: init wired", "Config schema spec", "Tests added"
- [ ] `docs/ARCHITECTURE.md` — add new §Config schema section with TOML example block + field descriptions table + default values; update §Modules table mark `src/config.rs` 1.2 ✅; update §Phase status from "Phase 1.1 shipped" → "Phase 1.2 shipped"
- [ ] `docs/ARCHITECTURE.md` §Error handling table — verify "Config parse fail → Exit 2" row still accurate (no change needed, P002 implements per spec)
- [ ] `README.md` — KHÔNG update (defer Phase 1.6)
- [ ] `CLAUDE.md` — KHÔNG update (no convention change)
- [ ] `docs-gate --all --verbose` — must pass

### Discovery Report

- [ ] `docs/discoveries/P002.md` written per `docs/RULES.md` format:
  - Assumptions ĐÚNG (list 13 anchors verified ✅ during Worker Task 0)
  - Assumptions SAI (e.g., if Anchor #14 untagged enum failed → switched to tagged variant; cite TOML field rename)
  - Edge cases discovered (e.g., `toml` crate version 0.8 may serialize `PathBuf` differently on macOS vs Linux — Phase 3 risk note)
  - Docs updated per discoveries
  - Layer 2 capability checks fired: B (cargo check, cargo test --all, targeted), D (grep ARCHITECTURE.md "Config schema"), E (cargo update --dry-run, clean rebuild)
- [ ] `docs/DISCOVERIES.md` — prepend 1-line: `- 2026-MM-DD P002: Config schema (TOML + serde, 3 blocks, untagged ScheduleConfig enum, zero new dep), <key finding> → see docs/discoveries/P002.md`
- [ ] Sub-mechanism A-E Verification Trace table filled (above)
