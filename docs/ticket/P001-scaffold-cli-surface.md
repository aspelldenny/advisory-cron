# PHIẾU P001: Scaffold + CLI surface (clap derive)

> **Loại:** Feature
> **Tầng:** 1
> **Ưu tiên:** P0 (foundation — every later phiếu depends)
> **Ảnh hưởng:** `src/main.rs`, `src/cli/` (new), `Cargo.toml` (no new deps), `tests/` (new)
> **Dependency:** Không (đây là phiếu đầu tiên của Phase 1)

---

## Context

### Vấn đề hiện tại

Repo mới bootstrap. `src/main.rs` hiện chỉ là `cargo new` placeholder (`println!("Hello, world!")` — `[verified]`). Cần dựng khung CLI 5 subcommands (`init`, `register`, `unregister`, `run`, `status`) để các phiếu Phase 1.2 → 1.5 có chỗ gắn logic. Không có khung → mọi phiếu sau sẽ phải tự debate naming/exit-code/clap shape, lãng phí.

Reference: BACKLOG.md dòng 18 — "Phase 1.1 — Scaffold + CLI surface (clap derive). Subcommands: init, register, unregister, run, status. Each subcommand returns proper exit code + help text. Empty implementations (panic with 'not yet implemented') + happy-path test for --help. Tầng 1 (defines CLI contract for entire tool). ~150 LOC."

### Giải pháp

Dùng `clap` 4 derive macro để define `Cli` struct + `Commands` enum. `main()` parse → match → dispatch sang handler functions trong `src/cli/<sub>.rs`. Mỗi handler hiện trả `anyhow::Result<i32>` với body `bail!("not yet implemented: <sub>")` — exit code 1 (generic error) khi gọi. `--help` (và `<sub> --help`) phải in help text liệt kê đủ 5 subcommands → exit 0.

**Layering decision (Sếp gợi ý trong spawn prompt):** P001 KHÔNG tạo `src/core/` module ngay. Lý do:
- Acceptance là 5 stub `panic`/`bail`, không có logic thật để tách core ↔ adapter.
- Tạo `core::init()` rỗng + `cli::init` gọi `core::init()` rỗng = 2 lớp indirection cho 0 logic → anti-pattern (over-engineer Phase 0).
- Khi Phase 1.2 viết config logic thật, phiếu đó sẽ extract `core::config::load()` natural; CLI handler co lại thành thin shell tại điểm có logic. Cost spread across 1.2–1.5, không upfront.
- Phase 1.7 (MCP wrapper) sẽ FORCE rà lại toàn bộ — refactor 5 file mỏng (mỗi file 5–15 dòng dispatch) DỄ HƠN refactor 5 file dày với core stub giả. Net: trì hoãn `core/` đến khi có logic thật để host là choice anti-completeness-bias.

→ **Hệ quả:** P001 chỉ tạo `src/cli/`. `src/core/` sẽ xuất hiện lần đầu trong P002 hoặc P003 (Worker của phiếu đó tự extract khi có logic).

### Scope

- CHỈ tạo: `src/main.rs` (rewrite), `src/cli/mod.rs`, `src/cli/init.rs`, `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/run.rs`, `src/cli/status.rs`, `tests/cli_help.rs`
- KHÔNG tạo: `src/core/` (defer — xem layering decision), `src/cli/mcp.rs` (Phase 1.7), `src/config.rs`, `src/launchd.rs`, `src/runner.rs`, `src/heartbeat.rs` (later phiếu)
- KHÔNG động: `Cargo.toml` (zero new deps — clap đã có), `Cargo.lock`, `.docs-gate.toml`, `.sos-stack.toml`, `.phieu-counter`

### Skills consulted (optional)

*(Orchestrator chưa chạy skill nào cho phiếu này. Verification thuần đọc docs + grep code thật.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/main.rs` hiện chỉ là `cargo new` stub (1 fn, `println!("Hello, world!")`) | Read `src/main.rs` | `[verified]` | ✅ 3 dòng, dòng 2 `println!("Hello, world!");` |
| 2 | `src/` chưa có module nào khác ngoài `main.rs` | `Glob("src/**/*")` | `[verified]` | ✅ chỉ 1 file `src/main.rs` |
| 3 | `Cargo.toml` đã có `clap = { version = "4", features = ["derive"] }` | Read `Cargo.toml` dòng 14 | `[verified]` | ✅ confirmed |
| 4 | `Cargo.toml` đã có `anyhow = "1"` + `thiserror = "1"` | Read `Cargo.toml` dòng 19-20 | `[verified]` | ✅ confirmed |
| 5 | `Cargo.toml` đã có `tracing` + `tracing-subscriber` (cho future logging init) | Read `Cargo.toml` dòng 21-22 | `[verified]` | ✅ confirmed (tracing 0.1, tracing-subscriber 0.3 env-filter+json) |
| 6 | `Cargo.toml` edition = `"2024"` (rustc MSRV 1.85) | Read `Cargo.toml` dòng 4 | `[verified]` | ✅ `edition = "2024"` |
| 7 | Exit code semantics: 0 success, 1 generic, 2 config not found, 3 launchd op fail, 4 task fire fail, 5 MCP transport, 130 SIGINT | Read `docs/ARCHITECTURE.md` §CLI surface, exit codes table | `[verified]` | ✅ ARCHITECTURE.md dòng 67-78 |
| 8 | CLI subcommand list = `init`, `register`, `unregister`, `run`, `status` (Phase 1.1 scope; `mcp` defer to 1.7) | Read `docs/ARCHITECTURE.md` §CLI surface table | `[verified]` | ✅ ARCHITECTURE.md dòng 58-65, `mcp` row explicit Phase 1.7 |
| 9 | Per-subcommand flags spec (ARCHITECTURE.md): `init --force`, `register --schedule <cron> --label <name>`, `unregister --label <name>`, `run` (no args), `status --json` | Read `docs/ARCHITECTURE.md` §CLI surface | `[verified]` | ✅ ARCHITECTURE.md dòng 60-64 |
| 10 | `tests/` directory chưa tồn tại (integration test mới) | `Glob("tests/**/*")` | `[needs Worker verify]` | ✅ Confirmed absent — no `tests/` dir exists; `tests/cli_help.rs` will create it fresh |
| 11 | Rust convention cho "not yet implemented" stub trong CLI handler: `anyhow::bail!()` ưu việt hơn `todo!()` vì (a) exit code controllable, (b) tracing context preserved, (c) `cargo build --release` không panic trong release path | Convention assertion từ ARCHITECTURE.md + anyhow doc | `[unverified]` | ✅ Worker grepped `~/sos-kit/bootstrap/sos-rs/src/`: 11 `bail!()`, 0 `todo!()`. Convention confirmed: use `bail!()` |
| 12 | Test pattern: integration test ở `tests/cli_help.rs` sử dụng `assert_cmd` crate KHÔNG khả dụng (chưa có trong dev-deps) → workaround: dùng `std::process::Command::new(env!("CARGO_BIN_EXE_advisory-cron"))` (Cargo built-in, zero new dep) | Read `Cargo.toml` `[dev-dependencies]` — chỉ có `tempfile` + `tokio-test` | `[verified]` | ✅ `assert_cmd` không có; `env!("CARGO_BIN_EXE_<bin-name>")` là built-in macro Cargo provides cho integration tests, không cần dep mới |

**Anchors flagged:**
- #10: `[needs Worker verify]` — trivial glob, Worker confirm trong Task 0.
- #11: `[unverified]` — Worker cross-check convention, Discovery Report nếu phải đảo.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND).
> Schema: 1 turn = Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no Tầng 1 objections.**

**Anchor verification (all 12):**
- Anchor #1: ✅ `src/main.rs` is 3 lines; line 2 = `println!("Hello, world!");`
- Anchor #2: ✅ Only `src/main.rs` found under `src/` — no other modules
- Anchor #3: ✅ `Cargo.toml` line 14: `clap = { version = "4", features = ["derive"] }`
- Anchor #4: ✅ `Cargo.toml` lines 19-20: `anyhow = "1"`, `thiserror = "1"`
- Anchor #5: ✅ `Cargo.toml` lines 21-22: `tracing = "0.1"`, `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }`
- Anchor #6: ✅ `Cargo.toml` line 4: `edition = "2024"`
- Anchor #7: ✅ `docs/ARCHITECTURE.md` lines 67-78: exit code table 0/1/2/3/4/5/130 matches spec
- Anchor #8: ✅ `docs/ARCHITECTURE.md` lines 58-65: CLI surface table lists init/register/unregister/run/status; `mcp` row shows Phase 1.7
- Anchor #9: ✅ `docs/ARCHITECTURE.md` lines 60-64: per-subcommand flags match phiếu (`--force`, `--schedule`/`--label`, `--label`, no-args, `--json`)
- Anchor #10: ✅ Worker verified via Glob: `tests/` directory does not exist. New file `tests/cli_help.rs` will create it fresh.
- Anchor #11: ✅ Worker grepped `~/sos-kit/bootstrap/sos-rs/src/` — 11 occurrences of `bail!()`, zero `todo!()` across all CLI command handlers (`apply.rs`, `blueprint.rs`, `contract.rs`, `launch.rs`, `recipe.rs`). Pattern: `bail!()` is the sos-kit convention. Phiếu spec confirmed. Use `bail!()`.
- Anchor #12: ✅ `Cargo.toml` `[dev-dependencies]` has only `tempfile = "3"` and `tokio-test = "0.4"`. No `assert_cmd`. `env!("CARGO_BIN_EXE_advisory-cron")` built-in macro is sufficient.

**Baseline check:** `cargo check` exits 0 (clean scaffold before rewrite).

**Addressing Architect's 4 declared risks:**

**Risk 1 — Layering decision (no `core/` yet):** AGREE. sos-kit Rust reference (`sos-rs`) uses identical pattern: `commands/mod.rs` dispatches directly to per-subcommand handlers with zero intermediate `core/` layer. All logic is in the command files themselves. Extracting `core/` before logic exists = over-engineering. Defer confirmed appropriate.

**Risk 2 — `bail!()` vs `todo!()` convention:** CONFIRMED `bail!()`. Grep evidence: `~/sos-kit/bootstrap/sos-rs/src/commands/apply.rs:10`, `:23`, `:51`; `blueprint.rs:9`; `contract.rs:13`, `:16`, `:43`; `launch.rs:11`, `:22`, `:39`; `recipe.rs:8`. All CLI stubs use `bail!()`. Zero `todo!()` in sos-kit Rust source. Phiếu reasoning is sound: `bail!()` propagates cleanly to `main()` → exit 1; `todo!()` would panic → exit 101 (SIGABRT), outside spec table.

**Risk 3 — Clap exit code 2 collision:** ACCEPT defer to Phase 1.2. The `unknown_subcommand_exits_nonzero` test in `tests/cli_help.rs` correctly asserts only `!success` (not `== 2`), which is the right conservative check for P001. Note: clap's exit 2 for parse errors (unknown sub, missing required arg) is distinct in timing from app-level exit 2 ("Config not found") — clap errors precede any app logic. A custom error handler (to remap clap's exit 2 to e.g. exit 3 for parse or emit contextual message) is appropriate for Phase 1.2 when `init` introduces actual config loading. Documented; no action in P001.

**Risk 4 — tokio overhead in stubs:** ACCEPT keep `#[tokio::main]`. sos-kit uses sync `fn main()` because it has no async operations. advisory-cron's Phase 1.4 (`runner.rs` uses `tokio::process::Command`) makes async the natural choice for the binary from the start. Changing `main()` signature and all 5 handler signatures at Phase 1.4 would be a non-trivial Tầng 1 touch. The overhead (~2ms startup for a once-per-day cron job) is irrelevant. Keep async throughout.

**Sub-mechanism E note:** `cargo update --dry-run` shows `displaydoc v0.2.5 -> v0.2.6` available. This is a transitive dep of clap. Since P001 does not run `cargo update`, `Cargo.lock` stays pinned at 0.2.5. Not a concern for this phiếu.

**Objections (Tầng 1 only):** None.

**Status:** Worker accepted V1 — ready for Chủ nhà approval gate.

### Turn 1 — Architect Response

*(Architect fill khi RESPOND. KHÔNG đọc source code — dựa Worker citation `file:line`.)*

- [O1.x] → ACCEPT / DEFEND / REFRAME / DEFER

**Status:** (pending Turn 1 challenge)

### Final consensus

- Phiếu version: V<N>
- Total turns: <count>
- Approved: [date] — code execution may begin

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE. Append-only.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output>
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | `launchctl list \| grep advisorycron` | (no row — phiếu này không register plist) | no row (expected) | N/A (Phase 1.3 verifies A) |
| B (capability) | `cargo check` | exit 0 | exit 0 | ✅ |
| B (capability) | `cargo test --test cli_help` | targeted test passes | 3/3 pass | ✅ |
| C (migration) | N/A (no schema) | | | N/A |
| D (persistence) | `grep -l "subcommands" docs/ARCHITECTURE.md` | ≥1 hit (CLI surface section) | 1 hit | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (clap stays 4.x) | `displaydoc v0.2.5 → v0.2.6` (transitive minor — non-breaking; clap stays 4.6.1) | ✅ |
| E (env drift) | `cargo build --release` from clean `target/` | exit 0, binary in `target/release/advisory-cron` | exit 0, binary confirmed | ✅ |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE verification (Worker mandatory)

1. Re-read Verification Anchors table. Re-grep Anchor #10 (`Glob("tests/**/*")`) — confirm `tests/` empty or absent.
2. Re-grep Anchor #11 — `grep -rn "todo!()" ~/tarot/src ~/sos-kit/` (best-effort, Worker has fs scope). Nếu tarot CLI handler dùng `todo!()` mainstream → ghi vào Debug Log, chốt pattern dùng `todo!()` thay vì `bail!()` cho consistency. NẾU không thấy pattern nào → dùng `anyhow::bail!()` như phiếu spec.
3. `cargo check` baseline — confirm clean trước khi sửa.

### Task 1: Rewrite `src/main.rs` thành clap entry point

**File:** `src/main.rs`

**Tìm:** toàn bộ file (3 dòng `println!("Hello, world!");`).

**Thay bằng:**

```rust
//! advisory-cron CLI entry point.
//!
//! Parses subcommand via clap derive, dispatches to handler in `cli::*`,
//! returns appropriate exit code per ARCHITECTURE.md §CLI surface exit codes.

mod cli;

use clap::Parser;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "advisory-cron",
    version,
    about = "Local cron wrapper for periodic Claude Code tasks (launchd-backed on macOS).",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: cli::Commands,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli::dispatch(cli.command).await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}
```

**Lưu ý:**
- `#[tokio::main]` mặc dù P001 stubs không cần async — chuẩn bị cho Phase 1.4 (`tokio::process::Command`). Acceptable overhead cho 1 binary CLI, không phải hot path.
- `ExitCode::from(u8)` thay vì `process::exit(i32)` để Rust runtime flush stdio buffers cleanly.
- `{err:#}` in alternate format = full anyhow context chain.

### Task 2: Tạo `src/cli/mod.rs` — Commands enum + dispatcher

**File:** `src/cli/mod.rs` (mới)

**Thêm:**

```rust
//! CLI subcommand definitions and dispatcher.
//!
//! Each subcommand module exposes `pub async fn run(args: <SubArgs>) -> anyhow::Result<u8>`
//! returning the exit code on success. Phase 1.1 stubs return `bail!()` (exit 1).

use clap::Subcommand;

pub mod init;
pub mod register;
pub mod run;
pub mod status;
pub mod unregister;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Write default config to ~/.config/advisory-cron/config.toml.
    Init(init::Args),
    /// Generate launchd plist + register with user session.
    Register(register::Args),
    /// Remove launchd plist + bootout from user session.
    Unregister(unregister::Args),
    /// Fire the configured task once and write heartbeat.
    Run(run::Args),
    /// Show next scheduled fire time + last heartbeat.
    Status(status::Args),
}

pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8> {
    match cmd {
        Commands::Init(args) => init::run(args).await,
        Commands::Register(args) => register::run(args).await,
        Commands::Unregister(args) => unregister::run(args).await,
        Commands::Run(args) => run::run(args).await,
        Commands::Status(args) => status::run(args).await,
    }
}
```

**Lưu ý:**
- Mỗi subcommand exposes `Args` struct của riêng nó + `pub async fn run(args: Args) -> anyhow::Result<u8>`. Convention duy nhất.
- `dispatch` return `Result<u8>` — `Ok(0)` = success, `Ok(<other>)` = success-with-nonzero (rare, e.g., `status` exit 2 nếu plist not loaded), `Err` = generic error (caller maps tới exit 1 hoặc unwrap context để extract specific code).
- `Args` empty cho `run` vẫn cần khai báo (consistency) — placeholder struct.

### Task 3: Tạo 5 handler stubs với clap args

**Files:** `src/cli/init.rs`, `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/run.rs`, `src/cli/status.rs` (mỗi file mới)

**Template chung — `src/cli/init.rs`:**

```rust
//! `advisory-cron init` — write default config to ~/.config/advisory-cron/config.toml.
//! Phase 1.1 stub. Implementation arrives in Phase 1.2.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Overwrite existing config file if present.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`init` not yet implemented (Phase 1.2)");
}
```

**`src/cli/register.rs`:**

```rust
//! `advisory-cron register` — generate launchd plist + bootstrap into user session.
//! Phase 1.1 stub. Implementation arrives in Phase 1.3.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Cron-style schedule expression (e.g., "0 9 * * *").
    #[arg(long)]
    pub schedule: String,

    /// Label for the launchd job (becomes plist filename component).
    #[arg(long)]
    pub label: String,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`register` not yet implemented (Phase 1.3)");
}
```

**`src/cli/unregister.rs`:**

```rust
//! `advisory-cron unregister` — remove launchd plist + bootout from user session.
//! Phase 1.1 stub. Implementation arrives in Phase 1.3.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Label of the launchd job to remove.
    #[arg(long)]
    pub label: String,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`unregister` not yet implemented (Phase 1.3)");
}
```

**`src/cli/run.rs`:**

```rust
//! `advisory-cron run` — fire the configured task once and write heartbeat.
//! Phase 1.1 stub. Implementation arrives in Phase 1.4.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`run` not yet implemented (Phase 1.4)");
}
```

**`src/cli/status.rs`:**

```rust
//! `advisory-cron status` — show next scheduled fire time + last heartbeat.
//! Phase 1.1 stub. Implementation arrives in Phase 1.5.

use anyhow::bail;
use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Emit machine-readable JSON instead of human text.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(_args: Args) -> anyhow::Result<u8> {
    bail!("`status` not yet implemented (Phase 1.5)");
}
```

**Lưu ý:**
- Mỗi `Args` doc comment trên field → clap derive auto-generates per-arg help text.
- `#[arg(long)]` (no `short`) — Sếp chưa spec single-char flags; defer cho phiếu sau nếu muốn (`-l` for `--label`, etc.). Tránh bikeshed bây giờ.
- Imports `clap::Args as ClapArgs` để tránh name clash với module-level `Args` struct.
- `pub async fn run(_args: Args)` — async chưa cần thiết cho stub, nhưng giữ signature thống nhất → phase sau không phải đổi signature gây touch lan.

### Task 4: Integration test — happy-path `--help` exits 0 + mentions 5 subcommands

**File:** `tests/cli_help.rs` (mới)

**Thêm:**

```rust
//! Phase 1.1 acceptance: top-level `--help` exits 0 and lists all 5 subcommands.
//! Per-subcommand `<sub> --help` also exits 0 (clap derived).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

#[test]
fn top_level_help_exits_zero_and_lists_all_subcommands() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run binary");

    assert!(
        out.status.success(),
        "expected exit 0, got {:?}; stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in ["init", "register", "unregister", "run", "status"] {
        assert!(
            stdout.contains(sub),
            "expected --help output to mention `{sub}`, got:\n{stdout}"
        );
    }
}

#[test]
fn each_subcommand_help_exits_zero() {
    for sub in ["init", "register", "unregister", "run", "status"] {
        let out = Command::new(BIN)
            .args([sub, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to run `{sub} --help`: {e}"));

        assert!(
            out.status.success(),
            "subcommand `{sub} --help` failed: exit={:?} stderr={}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let out = Command::new(BIN)
        .arg("definitely-not-a-subcommand")
        .output()
        .expect("failed to run binary");

    assert!(
        !out.status.success(),
        "expected nonzero exit for unknown subcommand, got success"
    );
}
```

**Lưu ý:**
- `env!("CARGO_BIN_EXE_advisory-cron")` — Cargo built-in macro available in integration tests. Resolves to `target/<profile>/advisory-cron`. Zero new dep.
- 3 test cases (top-level help, per-sub help, unknown sub) đủ cho acceptance Phase 1.1. KHÔNG thêm tests stub `run` returns nonzero — đó là Phase 1.2+ acceptance.
- Test sequence runs against debug binary mặc định. Acceptance row `cargo test --all` confirmed.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/main.rs` | Task 1: rewrite thành clap entry point + dispatcher |
| `src/cli/mod.rs` | Task 2: tạo mới — Commands enum + dispatch fn |
| `src/cli/init.rs` | Task 3: tạo mới — `Args { force }` + stub `bail!()` |
| `src/cli/register.rs` | Task 3: tạo mới — `Args { schedule, label }` + stub |
| `src/cli/unregister.rs` | Task 3: tạo mới — `Args { label }` + stub |
| `src/cli/run.rs` | Task 3: tạo mới — `Args {}` + stub |
| `src/cli/status.rs` | Task 3: tạo mới — `Args { json }` + stub |
| `tests/cli_help.rs` | Task 4: tạo mới — 3 happy-path integration tests |
| `docs/CHANGELOG.md` | Append entry P001 (Tầng 1 — module added) |
| `docs/ARCHITECTURE.md` | §Modules: mark `src/main.rs` shipped Phase 1.1 ✅, mark `src/cli/init.rs` ... `src/cli/status.rs` skeleton-shipped 1.1 (impl deferred per phase) |
| `docs/discoveries/P001.md` | Write Discovery Report |
| `docs/DISCOVERIES.md` | Prepend 1-line index entry |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `Cargo.toml` | Zero deps added — clap/tokio/anyhow already present. Worker confirm bằng `git diff Cargo.toml` (must show no change). |
| `Cargo.lock` | Auto-regenerated bởi `cargo build`. Worker không edit thủ công. Có thể đổi do `cargo build` re-resolve, OK. |
| `.docs-gate.toml` / `.sos-stack.toml` | Không động. |
| `.phieu-counter` | Quản đốc đã bump 000 → 001. KHÔNG đụng. |
| `README.md` | Phiếu 1.6 mới update quick-start. P001 không touch. |

---

## Luật chơi (Constraints)

1. **ZERO new dependencies.** `Cargo.toml` `[dependencies]` + `[dev-dependencies]` không thêm 1 dòng. Nếu Worker thấy thiếu dep (e.g., `assert_cmd` tempting) → STOP, escalate `AskUserQuestion`. Anchor #12 đã chứng minh `env!("CARGO_BIN_EXE_*")` đủ cho integration test.
2. **No `unsafe { }` block.** Stub CLI không có lý do gì cần unsafe.
3. **No `tracing_subscriber::fmt::init()` trong main.rs.** Logging init defer cho Phase 1.4 (khi `runner` cần). Phase 1.1 stub không cần log gì.
4. **Async signature dù chưa cần.** Mọi handler `pub async fn run(...) -> anyhow::Result<u8>` — giữ signature consistent để Phase 1.2-1.5 không phải đổi.
5. **`bail!()` không phải `todo!()` cho stub** (subject to Worker Task 0 cross-check — see Anchor #11). Lý do: `todo!()` panics → exit code 101 (SIGABRT) không nằm trong exit code table; `bail!()` propagates qua `?` → `main()` map sang exit 1 (generic error) đúng spec.
6. **No CLI logic in `main.rs` beyond parse+dispatch.** Mọi behavior trong `cli::<sub>::run()`. Giữ `main` 1-trang đọc được.
7. **Module declaration:** `src/main.rs` chỉ `mod cli;` — không `mod cli::init` (Rust idiomatic = parent module declares children).
8. **No `core/` directory yet.** Defer per Layering decision in Context. Nếu Worker muốn tạo `src/core/mod.rs` rỗng "for symmetry" → STOP, escalate (anti-completeness-bias).
9. **Help text language:** English (Rust CLI convention per CLAUDE.md). Doc comments → cargo doc-quality English.
10. **`unwrap()` allowed only in tests.** Prod code: `?` + `anyhow::Context`. Stubs use `bail!()` directly (Result return type).

---

## Nghiệm thu

### Automated

- [ ] `cargo build --release` — zero warnings (Constraint check via Cargo)
- [ ] `cargo test --all` — all pass (3 tests in `tests/cli_help.rs`)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (no `dead_code`, no `unused_imports`)
- [ ] `cargo fmt --check` — no diff

### Manual Testing

- [ ] `./target/release/advisory-cron --help` exits 0; stdout mentions `init`, `register`, `unregister`, `run`, `status`, `help`
- [ ] `./target/release/advisory-cron init --help` exits 0; mentions `--force`
- [ ] `./target/release/advisory-cron register --help` exits 0; mentions `--schedule` + `--label`
- [ ] `./target/release/advisory-cron unregister --help` exits 0; mentions `--label`
- [ ] `./target/release/advisory-cron status --help` exits 0; mentions `--json`
- [ ] `./target/release/advisory-cron init` (no args) — exits 1, stderr `error: \`init\` not yet implemented (Phase 1.2)`
- [ ] `./target/release/advisory-cron register --schedule "0 9 * * *" --label test` — exits 1, stderr `error: \`register\` not yet implemented (Phase 1.3)`
- [ ] `./target/release/advisory-cron --version` exits 0; prints `advisory-cron 0.1.0` (clap auto-derived from Cargo.toml)
- [ ] `./target/release/advisory-cron bogus-subcommand` exits 2 (clap default for unknown subcommand) — note: clap exit 2 OK here, không clash với spec exit 2 ("config not found") vì clap parse error precedes app exit semantics
- [ ] `./target/release/advisory-cron register` (missing required `--schedule`) — exits 2 (clap missing-arg)

### Regression

- [ ] N/A — this is the first phiếu shipping prod code.

### Docs Gate

- [ ] `docs/CHANGELOG.md` — append entry P001 dated 2026-MM-DD, sections: "Modules added", "CLI surface (5 stubs)", "Tests added", "Layering decision (no core/ yet)"
- [ ] `docs/ARCHITECTURE.md` §Modules table — update "Phase ships" column: `src/main.rs` 1.1 ✅, `src/cli/init.rs` 1.1 (stub) → impl 1.2, similar for 4 others
- [ ] `docs/ARCHITECTURE.md` §CLI surface table — no edit (matches spec)
- [ ] `README.md` — KHÔNG update (defer to 1.6)
- [ ] `docs-gate --all --verbose` — must pass

### Discovery Report

- [ ] `docs/discoveries/P001.md` written per `docs/RULES.md` format:
  - Assumptions ĐÚNG (list all 12 anchors verified ✅)
  - Assumptions SAI (e.g., if Anchor #11 flipped — `todo!()` chosen instead of `bail!()`)
  - Edge cases discovered (e.g., clap exit code for unknown sub = 2 conflicts with spec exit 2 "config not found" — note conflict, propose phiếu 1.2 disambiguate via custom error handler if needed)
  - Docs updated per discoveries
  - Layer 2 capability checks fired: B (cargo check, cargo test), D (grep ARCHITECTURE.md "5 subcommands"), E (cargo update --dry-run, clean rebuild)
- [ ] `docs/DISCOVERIES.md` — prepend 1-line: `- 2026-05-27 P001: CLI scaffold (5 subcommand stubs, clap derive, zero new dep), <key finding> → see docs/discoveries/P001.md`
- [ ] Sub-mechanism A-E Verification Trace table filled (above)
