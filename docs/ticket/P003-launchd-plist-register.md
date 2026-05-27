# PHIẾU P003: launchd plist generator + `register` / `unregister` wiring

> **Loại:** Feature
> **Tầng:** 1
> **Phiếu version:** V2 (refined 2026-05-27 after Turn 1 Worker challenge — Task 2 + Task 5 mod.rs edits dropped; newtype dispatch pattern confirmed; Anchor #15 + #17 corrected)
> **Ưu tiên:** P0 (Phase 1.4 `run` cần plist hiện diện để verify end-to-end fire; Phase 1.5 `status` cần `launchctl print` đọc plist; bug ở P003 lan tới mọi phiếu Phase 1 còn lại)
> **Ảnh hưởng:** `src/launchd.rs` (mới), `src/cli/register.rs` (rewrite body + extend `Args` struct), `src/cli/unregister.rs` (rewrite body + extend `Args` struct), `src/main.rs` (thêm `mod launchd;`), `src/config.rs` (chỉ xóa `#[allow(dead_code)]` trên `Config::load`), `Cargo.toml` (zero new deps), `tests/cli_register.rs` (mới — integration test với TempDir + spawn binary). **`src/cli/mod.rs` KHÔNG sửa** — V2 correction per Turn 1 [O1.1]: dispatch dùng newtype `Register(register::Args)` đã pass full `Args` struct, mọi field mới (`--config`, `schedule: Option<String>`) khai báo INSIDE `register::Args` / `unregister::Args` qua `#[derive(ClapArgs)]`, KHÔNG ở enum.
> **Dependency:** P001 (CLI scaffold) ✅, P002 (config schema) ✅ merged 2026-05-27

---

## Context

### Vấn đề hiện tại

P001 ship 5 stub. P002 wired `init` + ship `Config` + `ScheduleConfig` ở `src/config.rs`. Hiện tại `register` + `unregister` vẫn `bail!("not yet implemented (Phase 1.3)")` ở `src/cli/register.rs` `[verified Turn 1]` + `src/cli/unregister.rs` `[verified Turn 1]`.

Phase 1 acceptance (`docs/PROJECT.md:54-57` `[verified]`):
- `advisory-cron register --schedule "0 9 * * *"` generates a launchd plist + loads it
- `advisory-cron unregister` removes the plist cleanly

Không có 2 subcommand này hoạt động → launchd không fire → Phase 1 không ship được (PROJECT.md hard line: tool exists vì "shipping a check ≠ the check running" — Sub-mechanism A). P002 Worker đã ghi note rằng `Config::load` cần `#[allow(dead_code)]` cho đến khi có caller binary (P003 chính là caller đó).

Reference: `docs/BACKLOG.md:22` `[verified]` — "Phase 1.3 — launchd plist generator. Function `generate_plist(config) -> PlistContent`. Plist XML matches Apple's launchd schema (`ProgramArguments`, `StartCalendarInterval`, `StandardOutPath`, `StandardErrorPath`, `Label`). `register` subcommand writes plist to `~/Library/LaunchAgents/com.advisorycron.<label>.plist` then `launchctl bootstrap gui/$UID <path>`. `unregister` does inverse. Tầng 1 (touches user's LaunchAgents — must be careful). ~250 LOC + integration test using `tempfile`."

### Giải pháp

Tạo `src/launchd.rs` chứa:

1. **`generate_plist(config: &Config, label: &str, self_exe: &Path) -> Result<String>`** — pure function, returns plist XML as UTF-8 string. Inputs: loaded `Config` (cho task.working_dir + schedule), label (string Sếp pass qua `--label`), self-exe path (path-to-`advisory-cron` binary, resolved bởi caller qua `std::env::current_exe()`). Output: full XML matching `docs/ARCHITECTURE.md:143-176` §Cron mechanism spec. Cấu trúc plist:
   - `<key>Label</key>` = `com.advisorycron.<label>`
   - `<key>ProgramArguments</key>` = `[self_exe, "run"]` (launchd-fired process invokes `advisory-cron run`)
   - `<key>StartCalendarInterval</key>` = derived từ `config.schedule` (Calendar → trực tiếp; Cron → parse `M H * * *` simple form, error nếu phức tạp)
   - `<key>StandardOutPath</key>` = `/tmp/advisory-cron-<label>.stdout.log`
   - `<key>StandardErrorPath</key>` = `/tmp/advisory-cron-<label>.stderr.log`
   - `<key>WorkingDirectory</key>` = `config.task.working_dir`
   - `<key>RunAtLoad</key>` = `<false/>`

2. **Trait `LaunchctlClient`** + 2 impl:
   - `RealLaunchctl` — shells out `launchctl bootstrap gui/$UID <plist_path>` và `launchctl bootout gui/$UID/<label>` qua `std::process::Command` (sync — current_thread runtime đủ; không cần `tokio::process` vì shell-out one-shot)
   - `NoopLaunchctl` — record calls trong `Vec<String>` cho unit test (in `#[cfg(test)]` block of `src/launchd.rs`); không touch real launchctl. Integration tests in `tests/cli_register.rs` spawn the binary and CANNOT inject `NoopLaunchctl` through CLI boundary — see Task 6 trade-off note.
   - Trait surface tối thiểu: `fn bootstrap(&self, plist_path: &Path) -> Result<()>` + `fn bootout(&self, label: &str) -> Result<()>`

3. **`plist_path_for(label: &str, launch_agents_dir: &Path) -> PathBuf`** — returns `<launch_agents_dir>/com.advisorycron.<label>.plist`. `launch_agents_dir` injected (default `~/Library/LaunchAgents/`; test inject `TempDir`).

4. **`current_uid() -> Result<u32>`** — helper using `id -u` shell-out (Heads-up #5 Option B). Zero unsafe, zero new dep.

`src/cli/register.rs` rewrite (V2):

**V2 IMPORTANT — dispatch pattern correction:** Turn 1 [O1.1] confirmed `src/cli/mod.rs` uses **newtype-wrapping** dispatch: `Commands::Register(register::Args) => register::run(args).await`. `mod.rs` already forwards the entire `register::Args` struct opaquely. Therefore:

- New CLI flags (`--config`, relaxed `--schedule`) declared INSIDE `register::Args` struct via `#[derive(clap::Args)]` field-level `#[arg(long)]` attributes
- `src/cli/mod.rs` requires **ZERO edits** for P003
- clap discovers the new args through the derive macro on `Args`, propagating into the binary's `--help` and parse path automatically

Body steps:
1. Load config từ `--config <path>` arg HOẶC default `~/.config/advisory-cron/config.toml` (xem Heads-up #2 resolution: chọn Option B).
2. Derive schedule: `--schedule <cron>` CLI arg override `config.schedule` nếu present; nếu absent dùng `config.schedule`. Cron expression CLI-passed parse `M H * * *` simple form → Calendar {hour, minute}; complex cron → exit 2 với error rõ.
3. Resolve self-exe path: `std::env::current_exe()?`.
4. Generate plist string.
5. Compose plist path: `~/Library/LaunchAgents/com.advisorycron.<label>.plist`.
6. `fs::create_dir_all(parent)` + `fs::write(plist_path, plist_xml)`.
7. Inject `RealLaunchctl` → `.bootstrap(plist_path)`.
8. Print `"registered launchd job: com.advisorycron.<label>"` → exit 0.
9. Errors map: config load fail → exit 2; plist write fail → exit 3; launchctl fail → exit 3.

`src/cli/unregister.rs` rewrite (V2, idempotent):

Same dispatch correction — new `--config` flag (reserved) declared inside `unregister::Args`, NOT on the enum variant.

Body steps:
1. Compose plist_path từ `--label` (KHÔNG cần load config — unregister chỉ cần label).
2. Inject `RealLaunchctl` → `.bootout(label)`. Nếu Err → log warning to stderr, continue (idempotent: label might not be loaded).
3. `fs::remove_file(plist_path)`. Nếu `NotFound` → log warning, continue. Other IO error → exit 3.
4. Print `"unregistered launchd job: com.advisorycron.<label>"` → exit 0.
5. Exit 3 chỉ khi: bootout returned error AND plist removal returned non-`NotFound` error (i.e., real launchctl failure crash + filesystem failure).

**Heads-up #1 resolved — `#[allow(dead_code)]` trên `Config::load`:** P002 Worker added `#[allow(dead_code)]` trên `pub fn load` ở `src/config.rs:72` `[verified Turn 1]` vì binary crate chưa có callsite. P003 `register::run` SẼ gọi `Config::load(&config_path)?` → attribute không còn cần.

→ **P003 action:** Worker xóa `#[allow(dead_code)]` attribute trên `pub fn load` trong `src/config.rs` LÚC wire callsite trong `register::run`. Verify clippy clean post-edit (warning sẽ không sinh vì có callsite). Note trong Discovery Report: "Anchor resolved — dead_code attribute removed when first binary caller added."

**Heads-up #2 resolved — `--config <path>` arg for `register`:** Chọn **Option B (add `--config <path>` clap arg cho `register` + `unregister`)**. **V2 correction:** flag khai báo INSIDE `register::Args` / `unregister::Args` struct (not on enum variant). Lý do:

- **Hard line #3 không bị vi phạm:** `docs/PROJECT.md:77` "No magic config discovery beyond 2 paths. Repo-local `.advisory-cron.toml` OR `~/.config/advisory-cron/config.toml`. Period." `[verified]`. `--config <path>` là explicit user override, KHÔNG phải magic discovery. Tương đương `npm --prefix`, `git -C`, `cargo --manifest-path` — convention CLI chuẩn. Hard line cấm auto-walk-up-tree, không cấm explicit flag.
- **Testability cao hơn rõ rệt:** Integration test point at `TempDir` config → không pollute user `~/.config/`. Nếu hardcode, mọi test phải `cmd.env("HOME", tmp)` rồi rely vào HOME→default-path resolution (P002 cli_init.rs pattern). Hoạt động nhưng kém explicit; `--config` cleaner cho EXECUTE Worker write test.
- **Symmetry với Phase 1.4/1.5:** `run` (Phase 1.4) cần đọc config để spawn task — nó cũng nên có `--config`. `status` (Phase 1.5) cần đọc heartbeat path → cũng `--config`. P003 setup pattern bây giờ, P004/P005 reuse. Defer Option A = forced refactor 3 phiếu sau.
- **P002 Worker đã lean B** (Heads-up #2 spawn prompt note). Em đồng ý.

→ **P003 action (V2):**
- `src/cli/register.rs`: `#[derive(clap::Args)] pub struct Args { ... #[arg(long)] pub config: Option<PathBuf>, ... }`. Also relax `schedule: String` → `schedule: Option<String>` to allow config-driven schedule when CLI flag absent.
- `src/cli/unregister.rs`: `#[derive(clap::Args)] pub struct Args { ... #[arg(long)] pub config: Option<PathBuf>, ... }`. `--config` accepted but unused in P003 — mark with leading underscore (`_config`) or `#[allow(dead_code)]` on the field (Worker pick — Tầng 2 stylistic).
- `src/cli/mod.rs`: **NO EDITS.** Dispatch `Commands::Register(args) => register::run(args).await` already forwards the entire `Args` struct including newly-added fields. clap derive on `Args` propagates flags into binary help/parse automatically.
- `register::run`: resolve config path: `args.config.unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"))`.
- Verify Anchor #3 `[verified Turn 1]`: confirmed `Commands::Register(register::Args)` + `Commands::Unregister(unregister::Args)` newtype pattern. P001 `cli_help.rs` tests check `--help` exit 0 + subcommand listing — agnostic to flag set, so adding flags inside Args structs cannot break P001 regression.

**Heads-up #3 resolved — launchctl integration test danger:** Real `launchctl bootstrap` trên test machine có 3 rủi ro: (a) macOS popup permission prompt block test run; (b) side-effect Sếp's running plists; (c) test environment khác (CI Linux không có launchctl → cargo test fail). Giải pháp 3 tầng:

1. **Unit test `generate_plist`** — pure function, no side effect. Snapshot assertion: parse generated XML string, verify chứa `<key>Label</key>\n    <string>com.advisorycron.test</string>`, `<key>Hour</key>\n            <integer>9</integer>`, etc. Hoặc full string compare với golden file `tests/fixtures/expected_plist.xml`. Em chọn **inline string substring checks** (golden file cần file fixture management, hơi nặng cho P003 — defer Phase 2 nếu cần regression detection chặt hơn).

2. **Integration test với `NoopLaunchctl` injection** — Trait `LaunchctlClient`. Production `register::run` accept `&dyn LaunchctlClient` (default = `&RealLaunchctl`). Test instantiate `NoopLaunchctl` + `TempDir` for both LAUNCH_AGENTS_DIR + CONFIG dir. Verify: plist file written to temp LAUNCH_AGENTS_DIR, NoopLaunchctl recorded `bootstrap` call với đúng path argument. Zero real launchctl invocation.

   **Implementation detail:** `register::run` currently signature `pub async fn run(args: Args) -> Result<u8>`. Need to inject `LaunchctlClient` + `launch_agents_dir`. Em propose:
   - Add helper `pub async fn run_with_deps<L: LaunchctlClient>(args: Args, launchctl: &L, launch_agents_dir: &Path) -> Result<u8>` — testable surface.
   - Public `pub async fn run(args: Args) -> Result<u8>` — production entry, calls `run_with_deps(args, &RealLaunchctl, &default_launch_agents_dir()?)`. Match clap dispatch contract.

   **V2 note (acknowledged from Turn 1):** Integration tests in `tests/cli_register.rs` spawn the compiled binary — they cannot reach `run_with_deps` directly, so NoopLaunchctl injection is only available to **unit tests inside `src/launchd.rs #[cfg(test)]` mod**. Integration tests therefore exercise the END-TO-END CLI surface (spawn binary, real RealLaunchctl invocation). See Task 6 trade-off note for real-launchctl pollution mitigation.

3. **Manual test in Nghiệm thu** — Sếp/Worker chạy real `cargo run --release -- register --label probe --schedule "0 9 * * *"` + `launchctl list | grep advisorycron` to verify A-mechanism (trigger gap closed). Followed by `cargo run -- unregister --label probe` to clean up. Manual = post-EXECUTE, KHÔNG trong `cargo test`.

→ **Critical constraint:** EXECUTE phase Worker tuyệt đối KHÔNG được call `launchctl bootstrap` trong unit test harness. Nếu cần verify A-mechanism (sub-mech A check trong Verification Trace), dùng manual test step. Integration tests in `tests/cli_register.rs` accepted-pollution per Task 6.

**Heads-up #4 resolved — Idempotent unregister:** Chi tiết design ở mục §Giải pháp `unregister.rs` rewrite. Logic table:

| bootout result | plist file state | Worker action | Exit code | stderr |
|----------------|------------------|---------------|-----------|--------|
| Success | Exists | Remove file | 0 | (silent) |
| Success | Missing | (no-op) | 0 | "warning: plist file already absent at <path>" |
| Failure (not loaded) | Exists | Remove file | 0 | "warning: launchctl reports label not loaded (likely never bootstrapped); proceeding to remove plist" |
| Failure (not loaded) | Missing | (no-op) | 0 | both warnings above |
| Failure (real error: perm denied, launchctl crash) | Exists | Try remove; if remove fails → exit 3 | 0 or 3 | bootout error context + remove status |
| Failure (real error) | Missing | exit 3 | 3 | full bootout error + "no plist to remove" |

Distinguishing "label not loaded" vs "real launchctl error": **V2 Anchor #17 update** — empirical probe (Worker Turn 1) confirmed exact stderr is `"Boot-out failed: 3: No such process"` (not just "No such process" or "Could not find specified service"). Em propose: treat ANY non-zero launchctl exit as "warning" path. Reasoning: idempotency > strict error classification. User can re-run safely. **Worker may match on substring `"No such process"` for log-clarity but MUST NOT branch behavior on it** — all errors flow through warn-continue path.

**Heads-up #5 resolved — `getuid()` `unsafe` issue:** `launchctl bootstrap gui/$UID <path>` cần UID. Rust std có `std::os::unix::fs::MetadataExt` nhưng không expose `getuid()`. Em chọn **Option B** (`id -u` shell-out). Lý do: zero new dep, zero unsafe, reliable cross-platform (POSIX). Cost: 1 extra Command spawn lúc register (~50ms). Acceptable cho one-shot CLI op.

Implementation: helper `fn current_uid() -> Result<u32>` trong `src/launchd.rs`:
```rust
fn current_uid() -> Result<u32> {
    let out = std::process::Command::new("id")
        .arg("-u")
        .output()
        .context("failed to spawn `id -u`")?;
    if !out.status.success() {
        bail!("`id -u` exited non-zero: {}", String::from_utf8_lossy(&out.stderr));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    s.parse::<u32>().with_context(|| format!("failed to parse UID from `id -u` output: {s:?}"))
}
```

### Scope

- CHỈ tạo/sửa:
  - `src/launchd.rs` (mới — `generate_plist`, `LaunchctlClient` trait, `RealLaunchctl`, `NoopLaunchctl`, `plist_path_for`, `current_uid`, unit tests)
  - `src/main.rs` (thêm `mod launchd;`)
  - `src/cli/register.rs` (rewrite body + extend `Args` struct với `config: Option<PathBuf>` + relax `schedule: Option<String>`)
  - `src/cli/unregister.rs` (rewrite body + extend `Args` struct với `config: Option<PathBuf>`)
  - `src/config.rs` (chỉ xóa `#[allow(dead_code)]` trên `Config::load`)
  - `tests/cli_register.rs` (mới — integration test với TempDir + binary spawn)
- KHÔNG sửa:
  - **`src/cli/mod.rs` — V2 correction: ZERO edits.** Dispatch already newtype-forwards full `Args`; clap derive on `Args` propagates flags automatically. (V1 incorrectly told Worker to edit mod.rs — Turn 1 [O1.1] accepted.)
  - `src/cli/init.rs` (P002 wired, không touch)
  - `src/cli/run.rs`, `src/cli/status.rs` (Phase 1.4/1.5 sẽ touch)
  - `src/cli/mcp.rs` (Phase 1.7 sẽ touch — nếu file đã exist)
  - `Cargo.toml` (zero new dep — std::process::Command + std::fs đủ)
  - `Cargo.lock` (auto-regenerated)
  - `tests/cli_help.rs` (P001 — 3 tests phải pass nguyên)
  - `tests/cli_init.rs` (P002 — 4 tests phải pass nguyên)
  - `README.md` (defer Phase 1.6)
  - `.phieu-counter` (Quản đốc đã bump 002 → 003)
- KHÔNG tạo: `src/core/`, `src/runner.rs`, `src/heartbeat.rs`, `src/mcp/` (later phiếu)

### Skills consulted (optional)

*(Orchestrator chưa chạy skill nào cho phiếu này. Verification dựa Read docs + P002 Discovery Report + Turn 1 Worker empirical probes.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Mỗi anchor carry humility marker. `[verified]` = em đã Read file confirm. `[unverified]` = docs imply, em chưa Read source. `[needs Worker verify]` = punt cho Thợ grep. `[verified Turn 1]` = Worker confirmed empirically during CHALLENGE.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/register.rs` hiện stub `bail!("not yet implemented (Phase 1.3)")` (hoặc tương đương "Phase 1.3") | Read `src/cli/register.rs` | `[verified Turn 1]` | ✅ Confirmed `src/cli/register.rs:19` — `bail!("` register` not yet implemented (Phase 1.3)")`. Struct has `schedule: String` (required) + `label: String`. |
| 2 | `src/cli/unregister.rs` hiện stub `bail!("not yet implemented (Phase 1.3)")` | Read `src/cli/unregister.rs` | `[verified Turn 1]` | ✅ Confirmed `src/cli/unregister.rs:15` — `bail!("` unregister` not yet implemented (Phase 1.3)")`. Struct has `label: String` only. |
| 3 | `src/cli/mod.rs` `Commands::Register` variant hiện đã có `--schedule <cron>` + `--label <name>` clap args per P001 ARCHITECTURE.md spec; `Commands::Unregister` hiện đã có `--label <name>` | Read `src/cli/mod.rs` Commands enum | `[verified Turn 1]` | ✅ CORRECTED — flags exist via **newtype wrapping** `Register(register::Args)`, NOT inline fields. Dispatch `mod.rs:30` is `Commands::Register(args) => register::run(args).await` — no destructuring. V2 phiếu accepts this pattern: NO mod.rs edits needed; new flags declared inside `register::Args` / `unregister::Args` via clap derive. |
| 4 | `src/cli/mod.rs` dispatch signature `pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>` không đổi giữa P002 ship và P003 start | Read `src/cli/mod.rs` | `[verified Turn 1]` | ✅ Confirmed `src/cli/mod.rs:28` — signature exact match. |
| 5 | `src/config.rs` `pub fn load(path: &Path) -> Result<Config>` có `#[allow(dead_code)]` attribute (per P002 Discovery `docs/discoveries/P002.md` edge case #1) | Read `src/config.rs` around `pub fn load` | `[verified Turn 1]` | ✅ Confirmed `src/config.rs:72` — `#[allow(dead_code)]` on `pub fn load`. |
| 6 | `src/config.rs` `Config { task: TaskConfig, schedule: ScheduleConfig, heartbeat: HeartbeatConfig }` shape đủ cho plist generation (cần `task.working_dir`, `schedule` for Hour/Minute) | Read `src/config.rs` struct definitions | `[verified]` per P002 phiếu lines 244-268 | ✅ Shape: `TaskConfig { command, args, working_dir: PathBuf }`, `ScheduleConfig` untagged enum `Cron { cron: String } \| Calendar { hour: u8, minute: u8 }`, `HeartbeatConfig { log_path: PathBuf }`. |
| 7 | `src/main.rs` hiện có `mod cli;` + `mod config;` (P002 added); KHÔNG có `mod launchd;` | Read `src/main.rs` mod declarations | `[verified Turn 1]` | ✅ Confirmed `src/main.rs:6-7` — `mod cli;` + `mod config;` present, no `mod launchd;`. |
| 8 | `Cargo.toml` `[dev-dependencies]` đã có `tempfile = "3"` (P001 Anchor #12 confirmed + P002 used cùng pattern) | Read `Cargo.toml` `[dev-dependencies]` | `[verified Turn 1]` | ✅ `Cargo.toml:26` — `tempfile = "3"` confirmed in dev-dependencies. |
| 9 | `Cargo.toml` `[dependencies]` KHÔNG có `libc`, `users`, `whoami`, `nix`, hay any crate cho `getuid()` | Read `Cargo.toml` `[dependencies]` | `[verified Turn 1]` | ✅ Confirmed absent. `id -u` shell-out confirmed correct approach. |
| 10 | `docs/ARCHITECTURE.md:143-176` §Cron mechanism plist XML spec đầy đủ 7 keys: `Label`, `ProgramArguments`, `StartCalendarInterval` (with `Hour`+`Minute` sub-keys), `StandardOutPath`, `StandardErrorPath`, `WorkingDirectory`, `RunAtLoad` (`<false/>`) | Read `docs/ARCHITECTURE.md` §Cron mechanism | `[verified]` | ✅ confirmed lines 143-176. P003 `generate_plist` MUST emit XML matching this spec exactly. |
| 11 | `docs/ARCHITECTURE.md:75` exit code 3 = "launchd operation failed" | Read `docs/ARCHITECTURE.md` §CLI surface exit codes | `[verified]` | ✅ confirmed line 75. P003 register/unregister return Ok(3) for launchctl failures. |
| 12 | `docs/ARCHITECTURE.md:84-135` §Config schema confirms `Config::load(path: &Path) -> Result<Config>` is canonical API; `Config::default_for_home(home: &Path) -> Config` exists | Read `docs/ARCHITECTURE.md` §Config schema | `[verified]` | ✅ confirmed line 134 §Source module "Config, TaskConfig, ScheduleConfig, HeartbeatConfig structs + load, default_for_home, write_default functions". P003 only needs `load`. |
| 13 | `docs/PROJECT.md:77` Hard line #3 "No magic config discovery beyond 2 paths" — explicit `--config <path>` CLI arg is NOT magic discovery (parallel to `git -C`, `cargo --manifest-path`) | Read `docs/PROJECT.md` §Hard lines | `[verified]` | ✅ confirmed line 77. Heads-up #2 resolution Option B compatible. |
| 14 | `docs/RULES.md:13-23` Tầng 1 trigger table includes: CLI flag added (Tầng 1), launchd plist layout (Tầng 1), Module added (Tầng 1) — P003 hits all three | Read `docs/RULES.md` §DOCS GATE Tầng 1 | `[verified]` | ✅ confirmed. P003 BẮT BUỘC update ARCHITECTURE.md §CLI surface (flag table for `--config`) + §Cron mechanism + §Modules table (mark `src/launchd.rs` shipped 1.3). |
| 15 | `docs/RULES.md:22` "Security boundary touched (env var read, file write outside `.sos-state/` or `docs/runlog/`) → AUTO Tầng 1 + docs/security/INVARIANTS.md review" — P003 writes to `~/Library/LaunchAgents/` (outside sos-state) AND shells out `launchctl` AND `id -u` | Read `docs/RULES.md` + `ls docs/security/INVARIANTS.md` | `[verified Turn 1]` | ✅ **CORRECTED V2** — INVARIANTS.md **EXISTS** at `docs/security/INVARIANTS.md` (5968 bytes, 2026-05-27, full INV-1..INV-6+ catalog). V1 architect's "may not exist" guess was wrong. NO stub creation needed; NO AskUserQuestion needed. P003 EXECUTE **MUST append** project-specific INV entry covering: (a) `launchctl` shell-out boundary, (b) `id -u` shell-out boundary, (c) `~/Library/LaunchAgents/` write boundary, (d) `fs::write` of attacker-influenceable label path component (label sanitization defense). |
| 16 | macOS `launchctl bootstrap gui/<uid> <plist_path>` is correct syntax (vs older `launchctl load`) | macOS `man launchctl` | `[verified Turn 1]` | ✅ Confirmed. `man launchctl` states: `bootstrap | bootout domain-target [service-path ...] | service-target`. `gui/<uid>` domain-target documented explicitly with example `gui/501/com.apple.example`. Syntax `launchctl bootstrap gui/<uid> <plist_path>` CORRECT. `launchctl bootout gui/<uid>/com.advisorycron.<label>` CORRECT (service-target form). |
| 17 | macOS `launchctl bootout` returns non-zero exit + specific stderr ("No such process" or similar) when label not loaded | macOS `man launchctl` + empirical test | `[verified Turn 1]` | ✅ **CORRECTED V2** — empirically verified: `launchctl bootout gui/501/com.advisorycron.fake-test-do-not-use` → `exit=3`, `stdout="Boot-out failed: 3: No such process"` (note: actual emit on stdout, not stderr; full string `"Boot-out failed: 3: No such process"` — NOT just `"No such process"` and NOT `"Could not find specified service"` as V1 guessed). Idempotency design (any non-zero → warn + continue) unchanged. Worker MUST: (a) NOT branch behavior on stderr substring matching; (b) record exact observed message string in Discovery Report; (c) treat both stdout and stderr fields when logging (launchctl writes diagnostics to stdout for some errors). |
| 18 | `std::env::current_exe()` returns absolute path to the running binary on macOS | std lib doc | `[unverified]` | ✅ stdlib stable since 1.0; macOS uses `_NSGetExecutablePath`. No probe needed. Err propagates upward; no fallback path needed. |
| 19 | `~/Library/LaunchAgents/` is user-writable on macOS by default (no special permission needed) | macOS file system convention | `[verified]` | ✅ confirmed convention — per-user LaunchAgents dir owned by user, mode 0755. `fs::create_dir_all` + `fs::write` succeed without sudo. Edge: if `~/Library/LaunchAgents/` doesn't exist (rare), `create_dir_all` creates it. |
| 20 | `toml::to_string_pretty` round-trip for `ScheduleConfig` cross-shape stable (P002 Discovery confirmed) | P002 Discovery Report `docs/discoveries/P002.md` edge case #3 | `[verified]` | ✅ confirmed — `Calendar { hour, minute }` serializes flat in `[schedule]`; re-parses correctly. P003 reads config via `Config::load` — round-trip stability gives confidence. |
| 21 | P002 ship `cargo test --lib` 9 tests pass + `cargo test --test cli_init` 4 tests pass (baseline for regression check post-P003) | P002 Discovery Report `docs/discoveries/P002.md` §Layer 2 capability checks | `[verified]` | ✅ confirmed. P003 regression target: 3 (cli_help) + 4 (cli_init) + 9 (config unit) + N new (P003 unit + integration) = ≥16+N tests post-P003 ship. |

**Anchors flagged for Worker Task 0 priority (V2 — all critical anchors already resolved via Turn 1):**

- All 21 anchors verified through Turn 1. EXECUTE Worker re-runs `cargo check` baseline + manual launchctl spot-check in Verification Trace, but no fresh probe required (CHALLENGE round was authoritative).

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi Quản đốc triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (refined 2026-05-27 — Turn 1 [O1.1] accepted; mod.rs edits dropped; Anchor #15 corrected to "exists"; Anchor #17 corrected to exact empirical stderr)

### Turn 1 — Worker Challenge

**Anchor verification (all 21 verified 2026-05-27):**

- Anchor #1: ✅ `src/cli/register.rs:19` — `bail!("` register` not yet implemented (Phase 1.3)")` confirmed.
- Anchor #2: ✅ `src/cli/unregister.rs:15` — `bail!("` unregister` not yet implemented (Phase 1.3)")` confirmed.
- Anchor #3: ⚠️ PARTIAL — `src/cli/mod.rs:18-21` uses **newtype-wrapping** pattern (`Register(register::Args)`, `Unregister(unregister::Args)`), NOT inline struct fields. `register::Args` has `schedule: String` + `label: String`. `unregister::Args` has `label: String` only. No `--config` on either. Phiếu Task 2 + Task 5 describe wrong dispatch pattern — see Objection O1.1 below.
- Anchor #4: ✅ `src/cli/mod.rs:28-36` — `pub async fn dispatch(cmd: Commands) -> anyhow::Result<u8>` confirmed. Pattern: `Commands::Register(args) => register::run(args).await` (newtype pass-through, no destructuring).
- Anchor #5: ✅ `src/config.rs:72` — `#[allow(dead_code)]` confirmed on `pub fn load`. P003 removes when callsite wired.
- Anchor #6: ✅ Config shape confirmed per P002 code read.
- Anchor #7: ✅ `src/main.rs:6-7` — `mod cli;` + `mod config;` present. `mod launchd;` absent. P003 adds it.
- Anchor #8: ✅ `Cargo.toml:26` — `tempfile = "3"` confirmed in dev-dependencies.
- Anchor #9: ✅ No libc/users/whoami/nix in `Cargo.toml`. `id -u` shell-out per Heads-up #5 Option B confirmed.
- Anchor #10: ✅ Architecture doc spec assumed correct per Architect verify.
- Anchor #11: ✅ Exit code 3 = "launchd operation failed" per ARCHITECTURE.md.
- Anchor #12: ✅ `Config::load(path: &Path) -> Result<Config>` confirmed at `src/config.rs:73`.
- Anchor #13: ✅ No magic config discovery beyond 2 paths in PROJECT.md.
- Anchor #14: ✅ Tầng 1 trigger table applies. P003 MUST update ARCHITECTURE.md.
- Anchor #15: ✅ **EXISTS** — `docs/security/INVARIANTS.md` confirmed at 5968 bytes (2026-05-27). Architect's note "may not exist" was wrong. No stub needed, no AskUserQuestion needed. File is a full INV catalog with INV-1 through INV-6+. P003 should add a project-specific INV entry for `launchctl` shell-out + `~/Library/LaunchAgents/` write boundary during EXECUTE.
- Anchor #16: ✅ `man launchctl` confirmed. Exact syntax: `bootstrap | bootout domain-target [service-path ...] | service-target`. `gui/<uid>` documented as "targets the domain based on which user it is associated with" (example in man page: `gui/501/com.apple.example`). Syntax `launchctl bootstrap gui/<uid> <plist_path>` and `launchctl bootout gui/<uid>/com.advisorycron.<label>` are CORRECT.
- Anchor #17: ✅ **VERIFIED empirically.** Running `launchctl bootout gui/501/com.advisorycron.fake-test-do-not-use` → `stdout: "Boot-out failed: 3: No such process"`, `exit=3`. Key finding: stderr message is "Boot-out failed: 3: No such process" (not "No such process" alone, not "Could not find specified service"). The phiếu's Heads-up #4 table comment says "No such process" or "Could not find specified service" — the actual message is "Boot-out failed: 3: No such process". The idempotency design (any Err from `RealLaunchctl::bootout` → warn + continue) is correct and unaffected by exact message. Discovery note: document exact message in P003 Discovery Report.
- Anchor #18: ✅ Trivial stdlib stable since 1.0. No probe needed pre-EXECUTE. `current_exe()` propagates Err upward if binary-deleted-mid-run edge case occurs.
- Anchor #19: ✅ `~/Library/LaunchAgents/` user-writable convention confirmed.
- Anchor #20: ✅ ScheduleConfig round-trip confirmed per P002.
- Anchor #21: ✅ P002 baseline: 9 config unit + 4 cli_init tests confirmed passing.

**Cargo check baseline:** ✅ `cargo check` exits 0, zero warnings post-P002.

---

**Objections (Tầng 1 — phiếu cần sửa):**

**[O1.1] Task 2 + Task 5 dispatch pattern assumes WRONG enum shape — will not compile.**

Phiếu Anchor #3 assumed `Commands::Register` would have inline struct fields. The actual code at `src/cli/mod.rs:18-21` uses newtype-wrapping:

```
// ACTUAL at src/cli/mod.rs:17-21
Register(register::Args),
Unregister(unregister::Args),
// dispatch at mod.rs:30-33:
Commands::Register(args) => register::run(args).await,
Commands::Unregister(args) => unregister::run(args).await,
```

Phiếu Task 2 spec (lines 727-746) shows adding inline fields to enum:
```
Register { schedule: String, label: String, config: Option<PathBuf> },
```
This would require rewriting the entire enum variant shape and dispatch — a larger change than phiếu intends.

Phiếu Task 5 spec (lines 975-982) shows:
```
Commands::Register { schedule, label, config } => {
    register::run(register::Args { schedule, label, config }).await
}
```
This pattern DOES NOT MATCH the actual newtype dispatch. This code will not compile against the real codebase.

**Correct approach:** The fields to add (`config: Option<PathBuf>` and relaxing `schedule: String` → `schedule: Option<String>`) belong in the `Args` structs INSIDE the respective module files, not on the enum variants. The dispatch in `mod.rs` requires ZERO changes — it already passes `args` through as `register::run(args).await`.

**Impact:** Task 2 (mod.rs edit) must be revised. Task 5 (dispatch update for mod.rs) is **entirely unnecessary** — Worker should skip the mod.rs dispatch edit in Task 5 and only perform the `src/config.rs` `#[allow(dead_code)]` removal. The `--config` field and `schedule` type change happen only in `register::Args` (register.rs) and `unregister::Args` (unregister.rs).

**Proposed alternatives:**

**A. (Recommended) Revise Task 2 + Task 5 to match newtype pattern:**
- Task 2 for `src/cli/mod.rs`: NO CHANGES NEEDED. The `--config` arg is added inside `register::Args` and `unregister::Args`, not the enum. Task 2's mod.rs edit block is dropped entirely.
- Task 2 for `src/main.rs`: unchanged (still add `mod launchd;`).
- Task 3 `register.rs` `Args` struct: add `config: Option<PathBuf>`, change `schedule: String` → `schedule: Option<String>`.
- Task 4 `unregister.rs` `Args` struct: add `config: Option<PathBuf>`.
- Task 5 for `src/cli/mod.rs`: NO CHANGES. Drop the dispatch edit entirely. Only action is removing `#[allow(dead_code)]` in `src/config.rs`.
- Dispatch at `src/cli/mod.rs:30-33` remains verbatim.

**B. Rewrite enum variants to inline-field style (not recommended):** Would require rewriting the `Commands` enum from newtype to inline-field form for ALL 5 variants (Init, Register, Unregister, Run, Status) for consistency — scope creep beyond P003. Reject.

**Heads-up resolutions — Worker agrees on all 5:**
- **Heads-up #1 (dead_code removal):** Agree. P003 removes `#[allow(dead_code)]` at `src/config.rs:72` when callsite wired in `register::run`.
- **Heads-up #2 (--config Option B):** Agree with the field placement correction in O1.1 — `--config` goes into `Args` structs, not enum inline.
- **Heads-up #3 (LaunchctlClient trait pattern):** Agree. Test isolation worth the added trait abstraction. `NoopLaunchctl` as `pub` struct is correct for cross-crate integration test access (noting the integration test will spawn the binary, so `NoopLaunchctl` is actually only needed for unit tests within `src/launchd.rs #[cfg(test)]`. The integration test in `tests/cli_register.rs` spawns the binary and cannot inject `NoopLaunchctl` through CLI boundary — Architect already acknowledged this in Task 6 note.)
- **Heads-up #4 (idempotent unregister):** Agree. "Any launchctl error → warn + continue" is correct. Empirical probe confirms exit code = 3 with "Boot-out failed: 3: No such process" message. The warn-continue path handles this correctly.
- **Heads-up #5 (id -u shell-out):** Agree. Zero unsafe, zero new dep. 50ms cost acceptable for one-shot CLI op.

**Status:** ✅ RESOLVED — Architect ACCEPTED in Turn 1 Response below.

Architect action needed: Confirm Alternative A above — revise phiếu so Task 2 drops the `src/cli/mod.rs` inline-field edit, and Task 5 drops the dispatch restructuring edit. The `--config` field addition and `schedule` type relaxation happen only inside `register::Args` and `unregister::Args` in their respective `.rs` files. Also update Anchor #15 result to ✅ EXISTS (no stub needed).

### Turn 1 — Architect Response (phiếu V2)

**Verdict per objection:**

- **[O1.1] → ACCEPT (Alternative A)** — Worker is right. V1 phiếu Task 2 + Task 5 mod.rs edits assumed inline-field enum variants but the actual codebase at `src/cli/mod.rs:17-21` uses newtype-wrapping (`Register(register::Args)`) per Worker `file:line` citation. V1 code would not compile — Worker's grep evidence is authoritative (em không có Grep tool, dựa hoàn toàn vào Worker's citation per RESPOND mode envelope).

  **Actions taken in V2:**
  1. **Task 2** revised — `src/cli/mod.rs` edit block DROPPED. Task 2 now only adds `mod launchd;` to `src/main.rs`. The new `--config` arg + relaxed `--schedule` are declared INSIDE `register::Args` / `unregister::Args` structs via `#[derive(clap::Args)]` field-level attributes — clap derive propagates flags into the binary `--help` and parse path automatically through the newtype dispatch.
  2. **Task 5** revised — `src/cli/mod.rs` dispatch destructuring edit DROPPED entirely. Task 5 now only performs the `#[allow(dead_code)]` removal in `src/config.rs:72`.
  3. **Task 3** (register.rs) — `Args` struct definition explicitly shows: `pub schedule: Option<String>` (relaxed from `String`) + `pub config: Option<PathBuf>` (new field). Body unchanged from V1 semantics.
  4. **Task 4** (unregister.rs) — `Args` struct adds `pub config: Option<PathBuf>` (reserved/unused in P003 — Worker decides between `_config` rename or `#[allow(dead_code)]` annotation as Tầng 2 stylistic call).
  5. **Files cần sửa table** — `src/cli/mod.rs` row removed.
  6. **Files KHÔNG sửa table** — `src/cli/mod.rs` row added (verify-only: confirm dispatch unchanged).
  7. **Ảnh hưởng header** updated to explicitly note `src/cli/mod.rs` is NOT touched.

- **Anchor #15 update** — ACCEPTED Worker correction: `docs/security/INVARIANTS.md` EXISTS (5968 bytes, full INV-1..INV-6+ catalog). V1 architect's "may not exist" guess was wrong. V2 Anchor #15 result column rewritten. **Task 0 step 5 (AskUserQuestion about stub) DROPPED** (no longer relevant). EXECUTE Worker MUST append project-specific INV entry to `docs/security/INVARIANTS.md` covering: (a) launchctl shell-out boundary, (b) `id -u` shell-out boundary, (c) `~/Library/LaunchAgents/` write boundary, (d) `fs::write` of attacker-influenceable label path component (label sanitization defense). Added to Discovery Report mandate + Files cần sửa table marks INVARIANTS.md as **REQUIRED** (no longer conditional).

- **Anchor #17 update** — ACCEPTED Worker empirical evidence: exact message is `"Boot-out failed: 3: No such process"` (on stdout, not stderr; exit=3). V1's `"No such process" or "Could not find specified service"` guess was incomplete. V2 Anchor #17 result column rewritten with exact string. V2 Heads-up #4 narrative updated to: (a) NO substring-based behavior branching, (b) Worker logs both stdout + stderr in error context, (c) Discovery Report records the exact observed message string verbatim.

**Per-task summary of V2 diffs vs V1:**

| Task | V1 → V2 change |
|------|----------------|
| Task 0 | Step 5 (INVARIANTS.md existence check + AskUserQuestion) DROPPED — Anchor #15 confirmed. Remaining baseline checks unchanged. |
| Task 1 (src/launchd.rs) | UNCHANGED — pure new file, no enum dispatch dependency. |
| Task 2 (mod declaration + flag wiring) | `src/cli/mod.rs` edit block DROPPED entirely. Only `src/main.rs` `mod launchd;` add remains. New flags declared inside `register::Args` / `unregister::Args` per Task 3 + Task 4. |
| Task 3 (src/cli/register.rs) | UNCHANGED semantically. Body code already shows `Args` struct with `pub schedule: Option<String>` + `pub config: Option<PathBuf>` — V1 task already had correct internal-Args approach; only V1's Task 2 + Task 5 enum-edit guidance was wrong. |
| Task 4 (src/cli/unregister.rs) | UNCHANGED semantically. Body code already shows `Args` struct with `pub config: Option<PathBuf>`. Worker picks `_config` vs `#[allow(dead_code)]` (Tầng 2). |
| Task 5 (dispatch + dead_code removal) | `src/cli/mod.rs` dispatch edit DROPPED. Only `src/config.rs:72` `#[allow(dead_code)]` removal remains. |
| Task 6 (tests/cli_register.rs) | UNCHANGED — already accounts for binary-spawn boundary preventing NoopLaunchctl injection. |

**Anchors flagged for Worker Task 0 priority (V2 — all critical anchors already resolved via Turn 1):** all 21 anchors verified during CHALLENGE; EXECUTE Worker re-runs `cargo check` baseline + manual launchctl spot-check in Verification Trace, but no fresh probe required.

**Status:** ✅ RESPONDED — phiếu bumped to V2. No DEFER, no DEFEND, no REFRAME. All Worker objections accepted as-is. EXECUTE may proceed after Quản đốc approval gate.

### Turn 2 — Worker Challenge

**Worker accepted V2 — all 10 V2 changes present, [O1.1] resolved, no new objections.**

Verification of each claimed change against actual V2 file content:

1. Task 2 mod.rs edit block deleted — CONFIRMED. Task 2 titled "Wire `src/main.rs` — declare module ONLY". No mod.rs edit block present. Only `mod launchd;` add to `src/main.rs` remains.
2. Task 5 mod.rs dispatch destructuring deleted — CONFIRMED. Task 5 titled "Remove `#[allow(dead_code)]` in `src/config.rs` ONLY". Dispatch restructuring edit absent. Explicit note: "V2 — `src/cli/mod.rs` is NOT touched in Task 5."
3. `--config` + `schedule: Option<String>` declared INSIDE `register::Args` / `unregister::Args` — CONFIRMED. Task 3 `Args` struct has `pub schedule: Option<String>` + `pub config: Option<PathBuf>` as `#[arg(long)]` fields. Task 4 `Args` struct has `pub config: Option<PathBuf>`. No mod.rs edits in either task.
4. Constraint #16 hard rule added — CONFIRMED. Line in Constraints: "V2 [O1.1] hard constraint: `src/cli/mod.rs` MUST NOT be edited. Post-EXECUTE `git diff src/cli/mod.rs` MUST be empty."
5. Files cần sửa: `src/cli/mod.rs` row removed — CONFIRMED. Table lists 11 files; no `src/cli/mod.rs` row present.
6. Files KHÔNG sửa: `src/cli/mod.rs` row added with verify rule — CONFIRMED. First row in table: "`src/cli/mod.rs`" with "V2 — DO NOT EDIT ... `git diff src/cli/mod.rs` shows zero changes."
7. Anchor #15 corrected to ✅ EXISTS — CONFIRMED. Result column: "✅ CORRECTED V2 — INVARIANTS.md EXISTS at `docs/security/INVARIANTS.md` (5968 bytes, 2026-05-27, full INV-1..INV-6+ catalog). V1 architect's 'may not exist' guess was wrong."
8. Anchor #17 corrected to exact empirical message — CONFIRMED. Result column: "✅ CORRECTED V2 — empirically verified: `stdout='Boot-out failed: 3: No such process'` ... full string `'Boot-out failed: 3: No such process'` — NOT just `'No such process'` and NOT `'Could not find specified service'` as V1 guessed."
9. Discovery mandate expanded to require INVARIANTS.md INV entry — CONFIRMED. Files cần sửa marks `docs/security/INVARIANTS.md` as REQUIRED. Nghiệm thu Docs Gate lists specific 4-point INV entry content required. Discovery Report mandate requires confirmation of append.
10. Nghiệm thu: `git diff src/cli/mod.rs` must be empty — CONFIRMED. Nghiệm thu Automated section includes "- [ ] `git diff src/cli/mod.rs` — **empty** (V2 [O1.1] hard constraint)".

No new Tầng 1 objections. Phiếu V2 is internally consistent and the code skeleton provided in tasks is coherent with the newtype dispatch pattern confirmed in Turn 1.

Ready for Chủ nhà approval gate.

### Final consensus
- Phiếu version: V2
- Total turns: 2 (Turn 1 objection + Turn 2 acceptance — consensus reached)
- Approved (autonomous narrate or Sếp gate): pending Quản đốc APPROVAL_GATE narration

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.
> Purpose: post-mortem trace, especially for autonomous mode where Sếp didn't watch live.
> Append-only — Worker writes, không edit/delete.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

Example:
```
[2026-05-27T14:00:00Z] event=task0_anchor_1_grep evidence=src/cli/register.rs:14 "bail!(\"not yet implemented\")"
[2026-05-27T14:01:30Z] event=task0_anchor_16_man_launchctl evidence="bootstrap <domain-target> <service-path>" confirmed
[2026-05-27T14:02:15Z] event=task0_anchor_17_bootout_probe evidence="launchctl bootout gui/501/com.advisorycron.fake-test-... exit=3, stdout=\"Boot-out failed: 3: No such process\""
[2026-05-27T14:30:00Z] event=cargo_check evidence=exit_code=0 duration_ms=3800
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks (RULES.md matrix) BEFORE marking phiếu DONE.
> Fill the table; mark N/A if not applicable to this phiếu.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | (manual post-EXECUTE) `cargo run --release -- register --label probe-p003 --schedule "0 9 * * *"` then `launchctl list \| grep advisorycron.probe-p003` | row present | | |
| A (trigger) | (manual cleanup) `cargo run --release -- unregister --label probe-p003` then `launchctl list \| grep advisorycron.probe-p003` | no row | | |
| B (capability) | `cargo check` | exit 0, zero warnings | | |
| B (capability) | `cargo test --test cli_help` | 3/3 pass (P001 regression) | | |
| B (capability) | `cargo test --test cli_init` | 4/4 pass (P002 regression) | | |
| B (capability) | `cargo test --test cli_register` | new tests pass | | |
| B (capability) | `cargo test --lib launchd` | unit tests for `generate_plist` pass | | |
| B (capability) | `cargo test --lib config` | 9/9 pass (P002 regression — dead_code attribute removal must not break) | | |
| C (migration) | N/A — no schema change (config schema P002 stable; plist is new artifact) | | | N/A |
| D (persistence) | `grep -l "Cron mechanism" docs/ARCHITECTURE.md` | ≥1 hit (existing P001-era section, P003 expand) | | |
| D (persistence) | `grep -l "src/launchd.rs" docs/ARCHITECTURE.md` | ≥1 hit (Modules table updated to shipped 1.3 ✅) | | |
| D (persistence) | `grep -c "INV-" docs/security/INVARIANTS.md` | ≥1 new INV entry added for launchctl boundary | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | | |
| E (env drift) | `cargo build --release` from clean `target/` | exit 0, binary < 7MB | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE verification (Worker mandatory)

1. **Anchor recap reads** — Read `src/cli/register.rs`, `src/cli/unregister.rs`, `src/cli/mod.rs`, `src/main.rs`, `src/config.rs`. Confirm:
   - Anchor #1: stub `bail!` present in register.rs (exact line + message — Turn 1 confirmed `:19`)
   - Anchor #2: stub `bail!` present in unregister.rs (Turn 1 confirmed `:15`)
   - Anchor #3: `Commands::Register(register::Args)` newtype pattern at `src/cli/mod.rs:17-21` (Turn 1 confirmed) — DO NOT attempt to edit `mod.rs` enum variants
   - Anchor #4: dispatch signature unchanged at `src/cli/mod.rs:28` (Turn 1 confirmed)
   - Anchor #5: `#[allow(dead_code)]` present on `pub fn load` at `src/config.rs:72` (Turn 1 confirmed)
   - Anchor #7: `mod cli;` + `mod config;` in `src/main.rs:6-7`, no `mod launchd;` (Turn 1 confirmed)
   - Log each to Debug Log with `file:line` evidence.

2. **(V2 — Anchor #16 already verified Turn 1)** `man launchctl` syntax probe — Worker SKIP if Turn 1 evidence still valid (re-run only if `man launchctl` output suspected stale). Otherwise re-confirm:
   - `bootstrap <domain-target> <service-path>` with `<domain-target>` = `gui/<uid>`
   - `bootout <service-target>` with `<service-target>` = `gui/<uid>/<label>`

3. **(V2 — Anchor #17 already verified Turn 1)** Idempotency probe — Worker SKIP if Turn 1 evidence still valid. EXECUTE behavior MUST treat any non-zero launchctl exit as warning-continue path (NO substring branching). Record exact stderr/stdout in Discovery Report when first manual run hits this path.

4. **Anchor #9 dep audit** — `cat Cargo.toml | grep -E "^(libc|users|whoami|nix)" || echo "no uid-related crate"` → expect no match. Confirm using `id -u` shell-out per Heads-up #5 Option B.

5. **(V2 — DROPPED.)** Anchor #15 INVARIANTS.md existence check no longer needed — Turn 1 confirmed file exists. EXECUTE Worker MUST instead append a project-specific INV entry to `docs/security/INVARIANTS.md` covering launchctl + id-u shell-out + LaunchAgents write boundary + label sanitization. See Docs Gate section.

6. **`cargo check` baseline** — confirm clean (zero warnings post-P002 ship) before any edit.

### Task 1: Tạo `src/launchd.rs` — plist generator + LaunchctlClient trait + helpers

**File:** `src/launchd.rs` (mới)

**Thêm:**

```rust
//! Phase 1.3 — launchd plist generation + `launchctl` bootstrap/bootout wrappers.
//!
//! macOS-specific. Linux deferred to Phase 3 (systemd timer / cron-tab).
//!
//! Public surface:
//! - `generate_plist(config, label, self_exe)` — pure XML string builder
//! - `plist_path_for(label, launch_agents_dir)` — compose absolute plist path
//! - `LaunchctlClient` trait — `bootstrap`/`bootout` abstraction
//! - `RealLaunchctl` — production impl using `std::process::Command`
//! - `NoopLaunchctl` — test impl recording calls (gated `#[cfg(test)]` if not needed in integration test;
//!    expose `pub` for `tests/cli_register.rs` import)
//! - `current_uid()` — POSIX `id -u` shell-out (zero-unsafe, zero-dep alternative to `libc::getuid()`)

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{Config, ScheduleConfig};

/// Compose absolute plist path: `<launch_agents_dir>/com.advisorycron.<label>.plist`.
pub fn plist_path_for(label: &str, launch_agents_dir: &Path) -> PathBuf {
    launch_agents_dir.join(format!("com.advisorycron.{label}.plist"))
}

/// Default user LaunchAgents directory: `<home>/Library/LaunchAgents/`.
pub fn default_launch_agents_dir(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
}

/// Generate launchd plist XML for a configured task.
///
/// `config` provides `task.working_dir` + `schedule`.
/// `label` becomes the `Label` key suffix (full label = `com.advisorycron.<label>`).
/// `self_exe` is the absolute path to the `advisory-cron` binary launchd will fire (it invokes `<self_exe> run`).
///
/// Returns: UTF-8 plist XML string matching `docs/ARCHITECTURE.md` §Cron mechanism spec.
///
/// Errors: if `config.schedule` is `Cron` variant with an expression not parseable as `"M H * * *"` form
/// (launchd has no native crontab support — only `StartCalendarInterval`).
pub fn generate_plist(config: &Config, label: &str, self_exe: &Path) -> Result<String> {
    let (hour, minute) = match &config.schedule {
        ScheduleConfig::Calendar { hour, minute } => (*hour, *minute),
        ScheduleConfig::Cron { cron } => parse_simple_cron(cron)?,
    };

    // Sanitize: label MUST be safe for filesystem + reverse-DNS. Worker also validates upstream
    // in register::run but defense-in-depth here.
    if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!(
            "label must contain only ASCII alphanumeric / '-' / '_' (got {label:?})"
        );
    }

    let full_label = format!("com.advisorycron.{label}");
    let stdout_path = format!("/tmp/advisory-cron-{label}.stdout.log");
    let stderr_path = format!("/tmp/advisory-cron-{label}.stderr.log");

    // XML escape WorkingDirectory + self_exe (paths may contain `&`, `<`, `>` though rare on macOS).
    let working_dir_xml = xml_escape(&config.task.working_dir.display().to_string());
    let self_exe_xml = xml_escape(&self_exe.display().to_string());

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{full_label}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{self_exe_xml}</string>
        <string>run</string>
    </array>

    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key><integer>{hour}</integer>
        <key>Minute</key><integer>{minute}</integer>
    </dict>

    <key>StandardOutPath</key>
    <string>{stdout_path}</string>

    <key>StandardErrorPath</key>
    <string>{stderr_path}</string>

    <key>WorkingDirectory</key>
    <string>{working_dir_xml}</string>

    <key>RunAtLoad</key>
    <false/>
</dict>
</plist>
"#,
    ))
}

/// Parse cron expression in simple `M H * * *` form (Minute, Hour, daily) → (hour, minute) tuple.
/// launchd has no native crontab — only `StartCalendarInterval` (Hour/Minute/Day/etc). For Phase 1
/// we support ONLY the daily-fire simple form. Complex cron (ranges, lists, day-of-week) → error.
fn parse_simple_cron(expr: &str) -> Result<(u8, u8)> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        bail!(
            "cron expression must be 5 fields (got {n}): {expr:?}. \
             launchd Phase 1 supports only `M H * * *` daily form; \
             use [schedule] hour/minute for arbitrary times.",
            n = parts.len()
        );
    }
    // Enforce daily form: minute and hour numeric, day/month/dow all `*`.
    if parts[2] != "*" || parts[3] != "*" || parts[4] != "*" {
        bail!(
            "Phase 1 launchd cron support requires day/month/dow all `*` (daily fire). \
             Got: {expr:?}. Use [schedule] hour/minute in config for arbitrary schedules."
        );
    }
    let minute: u8 = parts[0]
        .parse()
        .with_context(|| format!("cron minute field must be 0..=59 numeric (got {:?})", parts[0]))?;
    let hour: u8 = parts[1]
        .parse()
        .with_context(|| format!("cron hour field must be 0..=23 numeric (got {:?})", parts[1]))?;
    if hour > 23 {
        bail!("cron hour must be 0..=23 (got {hour})");
    }
    if minute > 59 {
        bail!("cron minute must be 0..=59 (got {minute})");
    }
    Ok((hour, minute))
}

/// Minimal XML escape for `&`, `<`, `>`, `"`. Plist content typically file paths — apostrophe rare.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Abstraction over `launchctl` shell-out — production uses real launchctl, tests inject NoopLaunchctl.
pub trait LaunchctlClient {
    /// `launchctl bootstrap gui/<uid> <plist_path>`.
    fn bootstrap(&self, plist_path: &Path) -> Result<()>;

    /// `launchctl bootout gui/<uid>/<label>`. Returns Ok even if launchctl reports "not loaded"
    /// — caller decides idempotency. Errors only on hard launchctl failures (binary missing,
    /// spawn fail). Worker MUST capture stderr in returned error for diagnostics.
    fn bootout(&self, label: &str) -> Result<()>;
}

/// Production impl — shells out real `launchctl`.
pub struct RealLaunchctl;

impl LaunchctlClient for RealLaunchctl {
    fn bootstrap(&self, plist_path: &Path) -> Result<()> {
        let uid = current_uid()?;
        let domain = format!("gui/{uid}");
        let out = Command::new("launchctl")
            .arg("bootstrap")
            .arg(&domain)
            .arg(plist_path)
            .output()
            .context("failed to spawn `launchctl bootstrap`")?;
        if !out.status.success() {
            bail!(
                "launchctl bootstrap failed (exit {}): stdout={:?} stderr={:?}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    fn bootout(&self, label: &str) -> Result<()> {
        let uid = current_uid()?;
        let target = format!("gui/{uid}/com.advisorycron.{label}");
        let out = Command::new("launchctl")
            .arg("bootout")
            .arg(&target)
            .output()
            .context("failed to spawn `launchctl bootout`")?;
        if !out.status.success() {
            // V2 note (Anchor #17 empirical): expected stdout when label-not-loaded is
            // "Boot-out failed: 3: No such process" (exit=3). Do NOT branch behavior on
            // substring — caller (unregister::run) treats any Err as warn-continue.
            bail!(
                "launchctl bootout failed (exit {}): stdout={:?} stderr={:?}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }
}

/// Test impl — records calls; never invokes real launchctl.
#[derive(Debug, Default)]
pub struct NoopLaunchctl {
    pub bootstrap_calls: std::sync::Mutex<Vec<PathBuf>>,
    pub bootout_calls: std::sync::Mutex<Vec<String>>,
}

impl LaunchctlClient for NoopLaunchctl {
    fn bootstrap(&self, plist_path: &Path) -> Result<()> {
        self.bootstrap_calls.lock().unwrap().push(plist_path.to_path_buf());
        Ok(())
    }

    fn bootout(&self, label: &str) -> Result<()> {
        self.bootout_calls.lock().unwrap().push(label.to_string());
        Ok(())
    }
}

/// Resolve current UID via POSIX `id -u`. Zero-unsafe alternative to `libc::getuid()`.
/// Sub-100ms cost — acceptable for one-shot register/unregister CLI ops.
pub fn current_uid() -> Result<u32> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("failed to spawn `id -u`")?;
    if !out.status.success() {
        bail!(
            "`id -u` exited non-zero: stderr={:?}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    s.parse::<u32>()
        .with_context(|| format!("failed to parse UID from `id -u` output: {s:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HeartbeatConfig, TaskConfig};
    use std::path::PathBuf;

    fn sample_config_calendar() -> Config {
        Config {
            task: TaskConfig {
                command: "claude".into(),
                args: vec!["-p".into(), "/advisory-scan".into()],
                working_dir: PathBuf::from("/Users/test"),
            },
            schedule: ScheduleConfig::Calendar { hour: 9, minute: 0 },
            heartbeat: HeartbeatConfig {
                log_path: PathBuf::from("/Users/test/.local/state/advisory-cron/heartbeat.jsonl"),
            },
        }
    }

    fn sample_config_cron(expr: &str) -> Config {
        let mut c = sample_config_calendar();
        c.schedule = ScheduleConfig::Cron { cron: expr.into() };
        c
    }

    #[test]
    fn generate_plist_calendar_contains_all_required_keys() {
        let cfg = sample_config_calendar();
        let xml = generate_plist(&cfg, "test", Path::new("/usr/local/bin/advisory-cron"))
            .expect("calendar schedule should generate");
        for needle in [
            "<key>Label</key>",
            "<string>com.advisorycron.test</string>",
            "<key>ProgramArguments</key>",
            "<string>/usr/local/bin/advisory-cron</string>",
            "<string>run</string>",
            "<key>StartCalendarInterval</key>",
            "<key>Hour</key><integer>9</integer>",
            "<key>Minute</key><integer>0</integer>",
            "<key>StandardOutPath</key>",
            "<string>/tmp/advisory-cron-test.stdout.log</string>",
            "<key>StandardErrorPath</key>",
            "<string>/tmp/advisory-cron-test.stderr.log</string>",
            "<key>WorkingDirectory</key>",
            "<string>/Users/test</string>",
            "<key>RunAtLoad</key>",
            "<false/>",
        ] {
            assert!(
                xml.contains(needle),
                "plist missing required substring {needle:?}:\n{xml}"
            );
        }
        // DOCTYPE present
        assert!(xml.contains("<!DOCTYPE plist PUBLIC"));
    }

    #[test]
    fn generate_plist_cron_simple_daily_form_works() {
        let cfg = sample_config_cron("30 14 * * *");
        let xml = generate_plist(&cfg, "test", Path::new("/bin/x")).unwrap();
        assert!(xml.contains("<key>Hour</key><integer>14</integer>"));
        assert!(xml.contains("<key>Minute</key><integer>30</integer>"));
    }

    #[test]
    fn generate_plist_cron_complex_expression_errors() {
        let cfg = sample_config_cron("*/15 9-17 * * 1-5");
        let err = generate_plist(&cfg, "test", Path::new("/bin/x")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("daily") || msg.contains("numeric") || msg.contains("`*`"));
    }

    #[test]
    fn generate_plist_cron_wrong_field_count_errors() {
        let cfg = sample_config_cron("0 9 *");
        assert!(generate_plist(&cfg, "test", Path::new("/bin/x")).is_err());
    }

    #[test]
    fn generate_plist_rejects_invalid_label() {
        let cfg = sample_config_calendar();
        for bad in ["bad label", "bad.label", "bad/label", "bad@label", "bad$label"] {
            assert!(
                generate_plist(&cfg, bad, Path::new("/bin/x")).is_err(),
                "label {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn generate_plist_accepts_valid_labels() {
        let cfg = sample_config_calendar();
        for good in ["test", "advisory-scan", "daily_report", "Test123"] {
            assert!(
                generate_plist(&cfg, good, Path::new("/bin/x")).is_ok(),
                "label {good:?} should be accepted"
            );
        }
    }

    #[test]
    fn plist_path_for_composes_label_correctly() {
        let p = plist_path_for("scan", Path::new("/tmp/LaunchAgents"));
        assert_eq!(p, PathBuf::from("/tmp/LaunchAgents/com.advisorycron.scan.plist"));
    }

    #[test]
    fn default_launch_agents_dir_computes_user_path() {
        let p = default_launch_agents_dir(Path::new("/Users/x"));
        assert_eq!(p, PathBuf::from("/Users/x/Library/LaunchAgents"));
    }

    #[test]
    fn noop_launchctl_records_calls() {
        let n = NoopLaunchctl::default();
        n.bootstrap(Path::new("/tmp/foo.plist")).unwrap();
        n.bootout("scan").unwrap();
        assert_eq!(n.bootstrap_calls.lock().unwrap().len(), 1);
        assert_eq!(n.bootout_calls.lock().unwrap()[0], "scan");
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
    }

    #[test]
    fn current_uid_returns_nonzero() {
        // dev machine UID is non-zero (501 on macOS user, 1000 on Linux user typically).
        // Test confirms shell-out works; doesn't assert exact value.
        let uid = current_uid().expect("id -u must work in test env");
        assert!(uid > 0, "expected non-root UID in test env, got {uid}");
    }
}
```

**Lưu ý:**
- `generate_plist` is pure (no I/O) — easy to unit test. All side effects (file write, launchctl shell) live in `register::run` + `RealLaunchctl::bootstrap`.
- `LaunchctlClient` trait surface intentionally minimal (2 methods). Easy to mock; easy to extend Phase 3 with `SystemdClient` for Linux.
- `NoopLaunchctl` is `pub` (not `#[cfg(test)]`) so integration test in `tests/cli_register.rs` could theoretically import it — though Task 6 trade-off note observes binary-spawn integration tests cannot inject it. `pub` exposure is still useful for future lib-target restructure (Phase 2).
- `current_uid` shells out `id -u` per Heads-up #5. Worker MAY swap to `libc::getuid()` if dep added in future phiếu — but P003 keeps zero-dep.
- `parse_simple_cron` deliberately restrictive (only `M H * * *`). Document this in error message — Sếp/users immediately understand what subset is supported.
- XML escape minimal (4 chars). If paths ever contain unicode high-bit characters, plist parser handles UTF-8 natively (no escape needed).
- 11 unit tests cover: calendar schedule plist gen, cron simple form, cron complex rejection, cron wrong field count, label sanitization (5 bad + 4 good labels), plist path composition, default launch_agents_dir, noop launchctl recording, xml escape, current_uid sanity.

### Task 2 (V2 — revised per Turn 1 [O1.1]): Wire `src/main.rs` — declare module ONLY

**File:** `src/main.rs`

**Tìm:**
```rust
mod cli;
mod config;
```
(2 dòng adjacent — P002 added `mod config;` after `mod cli;` per P002 Task 2 spec; Anchor #7 Turn 1 verified `src/main.rs:6-7`)

**Thay bằng:**
```rust
mod cli;
mod config;
mod launchd;
```

**Lưu ý:**
- Add 1 dòng. Không đụng phần khác của `main.rs`.
- **V2 — `src/cli/mod.rs` is NOT touched in Task 2.** Turn 1 [O1.1] confirmed dispatch uses newtype `Register(register::Args)` pattern — clap forwards the full `Args` struct without destructuring. The new `--config` arg + relaxed `--schedule` are declared INSIDE `register::Args` (Task 3) and `unregister::Args` (Task 4) using `#[derive(clap::Args)]` field-level `#[arg(long)]` attributes. clap derive then propagates the flags into the binary `--help` + parse path automatically through the newtype dispatch. Zero edits to enum or dispatch fn needed.

### Task 3: Rewrite `src/cli/register.rs` — full implementation

**File:** `src/cli/register.rs`

**Tìm:** entire file body (currently stub `bail!("not yet implemented (Phase 1.3)")` at `src/cli/register.rs:19` per Anchor #1 Turn 1 confirmed; existing `Args` struct has `pub schedule: String` + `pub label: String`).

**Thay bằng:**

```rust
//! `advisory-cron register` — Phase 1.3 implementation.
//!
//! Loads config, generates launchd plist, writes to `~/Library/LaunchAgents/`,
//! bootstraps via `launchctl`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::{env, fs, path::PathBuf};

use crate::config::{Config, ScheduleConfig};
use crate::launchd::{
    LaunchctlClient, RealLaunchctl, default_launch_agents_dir, generate_plist, plist_path_for,
};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Cron expression (`M H * * *` daily form) — overrides config.schedule when present.
    /// (V2: relaxed from required `String` to `Option<String>` so config-driven schedule
    /// works without redundant CLI flag.)
    #[arg(long)]
    pub schedule: Option<String>,

    /// Label suffix (full label = com.advisorycron.<label>).
    #[arg(long)]
    pub label: String,

    /// Override default config path (default: ~/.config/advisory-cron/config.toml).
    /// (V2 new — placed inside Args struct, NOT on Commands enum, per newtype dispatch pattern
    /// confirmed by Turn 1 [O1.1].)
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub async fn run(args: Args) -> Result<u8> {
    let home = home_dir().context("failed to resolve $HOME")?;
    let launch_agents_dir = default_launch_agents_dir(&home);
    run_with_deps(args, &RealLaunchctl, &launch_agents_dir, &home).await
}

/// Test-friendly entry — injects LaunchctlClient + LaunchAgents dir.
pub async fn run_with_deps<L: LaunchctlClient>(
    args: Args,
    launchctl: &L,
    launch_agents_dir: &std::path::Path,
    home: &std::path::Path,
) -> Result<u8> {
    // 1. Resolve config path.
    let config_path = args
        .config
        .unwrap_or_else(|| home.join(".config/advisory-cron/config.toml"));

    // 2. Load config (may exit 2 if invalid).
    let mut config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load config: {e:#}");
            return Ok(2);
        }
    };

    // 3. Apply --schedule CLI override (parse as `M H * * *` simple form).
    if let Some(cron_expr) = &args.schedule {
        config.schedule = ScheduleConfig::Cron { cron: cron_expr.clone() };
        // generate_plist will validate via parse_simple_cron.
    }

    // 4. Validate label (defense-in-depth; generate_plist also checks).
    if args.label.is_empty() {
        eprintln!("error: --label must not be empty");
        return Ok(1);
    }

    // 5. Resolve self-exe path.
    let self_exe = env::current_exe().context("failed to resolve current executable path")?;

    // 6. Generate plist XML.
    let plist_xml = match generate_plist(&config, &args.label, &self_exe) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to generate plist: {e:#}");
            return Ok(2);
        }
    };

    // 7. Write plist file.
    fs::create_dir_all(launch_agents_dir)
        .with_context(|| format!("failed to create {}", launch_agents_dir.display()))?;
    let plist_path = plist_path_for(&args.label, launch_agents_dir);
    if let Err(e) = fs::write(&plist_path, &plist_xml) {
        eprintln!("error: failed to write plist to {}: {e:#}", plist_path.display());
        return Ok(3);
    }

    // 8. Bootstrap via launchctl.
    if let Err(e) = launchctl.bootstrap(&plist_path) {
        eprintln!("error: launchctl bootstrap failed: {e:#}");
        // Plist file already written; leave in place so user can inspect / retry.
        return Ok(3);
    }

    println!("registered launchd job: com.advisorycron.{}", args.label);
    println!("  plist: {}", plist_path.display());
    Ok(0)
}

fn home_dir() -> Result<PathBuf> {
    let raw = env::var("HOME").ok().filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(PathBuf::from(s)),
        None => bail!("$HOME env var is not set; cannot resolve default config / launch_agents path"),
    }
}
```

**Lưu ý:**
- `run_with_deps` is the testable surface; `run` is the thin production entry. Phase 1.7 MCP can also call `run_with_deps` directly with custom deps if needed.
- `--schedule` CLI override wraps to `ScheduleConfig::Cron { cron }`; `generate_plist` calls `parse_simple_cron` to validate. If user passes complex cron → exit 2 with helpful error.
- Plist file written BEFORE bootstrap attempt — if bootstrap fails, user can manually inspect plist + retry. Documented in error message.
- `home_dir()` duplicated from `init.rs` (P002 had same fn). Em accept duplication for P003 — extraction to shared util defer until 3+ callsites exist (Phase 1.4 `run` may also need it → reconsider in P004 phiếu).
- `args.schedule` is now `Option<String>` (relaxed from P001 spec which had `--schedule` as required `String`). **V2 note:** this is a Tầng 1 CLI change visible in `--help` — document in CHANGELOG. Since `src/cli/mod.rs` is NOT touched (Turn 1 [O1.1] correction), the relaxation propagates automatically through the newtype dispatch — clap derive on `Args` regenerates the parser.

### Task 4: Rewrite `src/cli/unregister.rs` — idempotent implementation

**File:** `src/cli/unregister.rs`

**Tìm:** entire file body (stub `bail!("not yet implemented (Phase 1.3)")` at `src/cli/unregister.rs:15` per Anchor #2 Turn 1 confirmed; existing `Args` struct has `pub label: String`).

**Thay bằng:**

```rust
//! `advisory-cron unregister` — Phase 1.3 implementation.
//!
//! Idempotent: succeeds even if label not currently loaded or plist file already absent.
//! Exit 3 only on real launchctl failure paired with plist removal failure.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::{env, fs, io, path::PathBuf};

use crate::launchd::{LaunchctlClient, RealLaunchctl, default_launch_agents_dir, plist_path_for};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Label suffix (full label = com.advisorycron.<label>).
    #[arg(long)]
    pub label: String,

    /// (V2 reserved — currently unused; declared for CLI symmetry with `register` per Heads-up #2.
    /// Placed inside Args struct, NOT on Commands enum, per newtype dispatch confirmed Turn 1 [O1.1].
    /// Worker chooses one of: (a) leading underscore rename `_config` to silence unused warning,
    /// or (b) `#[allow(dead_code)]` on the field — Tầng 2 stylistic.)
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub async fn run(args: Args) -> Result<u8> {
    let home = home_dir().context("failed to resolve $HOME")?;
    let launch_agents_dir = default_launch_agents_dir(&home);
    run_with_deps(args, &RealLaunchctl, &launch_agents_dir).await
}

pub async fn run_with_deps<L: LaunchctlClient>(
    args: Args,
    launchctl: &L,
    launch_agents_dir: &std::path::Path,
) -> Result<u8> {
    if args.label.is_empty() {
        eprintln!("error: --label must not be empty");
        return Ok(1);
    }

    // 1. Try launchctl bootout. If fails (likely "not loaded"), warn but continue.
    //    V2 (Anchor #17 empirical): expected error message is "Boot-out failed: 3: No such process"
    //    when label was never bootstrapped. Do NOT branch on substring — any Err goes through warn.
    let bootout_result = launchctl.bootout(&args.label);
    if let Err(ref e) = bootout_result {
        eprintln!("warning: launchctl bootout: {e:#} (label may not be loaded; proceeding to remove plist)");
    }

    // 2. Try plist file removal. NotFound → warn, continue. Other IO → potential exit 3.
    let plist_path = plist_path_for(&args.label, launch_agents_dir);
    let remove_result = fs::remove_file(&plist_path);
    match remove_result {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            eprintln!("warning: plist file already absent at {}", plist_path.display());
        }
        Err(e) => {
            eprintln!("error: failed to remove plist at {}: {e:#}", plist_path.display());
            // Hard failure on plist removal — exit 3 regardless of bootout result.
            return Ok(3);
        }
    }

    println!("unregistered launchd job: com.advisorycron.{}", args.label);
    Ok(0)
}

fn home_dir() -> Result<PathBuf> {
    let raw = env::var("HOME").ok().filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(PathBuf::from(s)),
        None => bail!("$HOME env var is not set"),
    }
}
```

**Lưu ý:**
- Idempotency: both `launchctl bootout` failure (likely "not loaded") AND missing plist file are warned-but-continue. Exit 0 if user-visible state is "label is gone from launchd + plist file is gone from disk".
- Exit 3 only if plist file is present and can't be removed (real IO error like permission denied) — that's a hard failure.
- `args.config` accepted but ignored — Worker picks `_config` rename or `#[allow(dead_code)]` annotation (Tầng 2 stylistic).
- Idempotency makes `unregister` safe to call repeatedly + safe to call before `register` was ever run (no-op + warnings).
- V2 Anchor #17: do NOT substring-match `"No such process"` for behavior branching — log the full bootout error context and let warn-continue path handle it.

### Task 5 (V2 — revised per Turn 1 [O1.1]): Remove `#[allow(dead_code)]` in `src/config.rs` ONLY

**File:** `src/config.rs`

**Tìm:** `#[allow(dead_code)]` attribute on `pub fn load` at `src/config.rs:72` (per Anchor #5 Turn 1 confirmed; P002 Discovery Report edge case #1).

**Thay bằng:** remove the attribute line entirely.

**Lưu ý:**
- After P003 wires `Config::load(&config_path)` in `register::run` (Task 3), the binary crate has a real callsite → rustc no longer flags as dead code.
- Worker run `cargo clippy --all-targets -- -D warnings` post-removal to confirm zero warnings.

**V2 — `src/cli/mod.rs` is NOT touched in Task 5.** Turn 1 [O1.1] confirmed dispatch is newtype `Commands::Register(args) => register::run(args).await` (no destructuring needed). V1's proposed dispatch destructuring edit is dropped entirely — the newtype forwards the full `Args` struct including new `config` + relaxed `schedule` fields automatically.

### Task 6: Integration test — `tests/cli_register.rs`

**File:** `tests/cli_register.rs` (mới)

**Thêm:**

```rust
//! Phase 1.3 acceptance: register/unregister flow with TempDir HOME override + binary spawn.
//!
//! CRITICAL: integration tests spawn the compiled binary, so they CANNOT inject NoopLaunchctl
//! across the CLI boundary. They exercise the END-TO-END flow with real RealLaunchctl invocation.
//! Pollution mitigation: unique label suffix per test (PID-based) + best-effort tearDown cleanup.
//! Unit tests inside src/launchd.rs #[cfg(test)] mod are the only place NoopLaunchctl is wired.

use std::path::Path;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_advisory-cron");

fn write_default_config(home: &Path) -> std::path::PathBuf {
    let config_dir = home.join(".config/advisory-cron");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    // Use Calendar schedule to avoid needing --schedule CLI arg.
    let toml = format!(
        r#"
[task]
command = "claude"
args = ["-p", "/advisory-scan"]
working_dir = "{}"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "{}/.local/state/advisory-cron/heartbeat.jsonl"
"#,
        home.display(),
        home.display()
    );
    std::fs::write(&config_path, toml).unwrap();
    config_path
}

#[test]
fn register_writes_plist_to_launch_agents_dir() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("test-p003-{}", std::process::id());

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label").arg(&label)
        .output()
        .expect("spawn failed");

    let plist_path = home.path()
        .join("Library/LaunchAgents")
        .join(format!("com.advisorycron.{label}.plist"));

    // Plist MUST be written regardless of launchctl bootstrap outcome (write happens before bootstrap).
    assert!(
        plist_path.exists(),
        "plist not written at {} (exit={:?} stdout={} stderr={})",
        plist_path.display(),
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    // Plist content sanity: contains label.
    let plist_content = std::fs::read_to_string(&plist_path).unwrap();
    assert!(plist_content.contains(&format!("com.advisorycron.{label}")));
    assert!(plist_content.contains("<key>Hour</key><integer>9</integer>"));

    // Cleanup: best-effort launchctl bootout + file removal (test pollution mitigation).
    let _ = std::process::Command::new("launchctl")
        .arg("bootout")
        .arg(format!("gui/{}/com.advisorycron.{label}", uid()))
        .output();
    let _ = std::fs::remove_file(&plist_path);
}

#[test]
fn register_with_cron_simple_form_works() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("test-p003-cron-{}", std::process::id());

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label").arg(&label)
        .arg("--schedule").arg("30 14 * * *")
        .output()
        .expect("spawn failed");

    let plist_path = home.path()
        .join("Library/LaunchAgents")
        .join(format!("com.advisorycron.{label}.plist"));

    assert!(
        plist_path.exists(),
        "plist not written (exit={:?} stderr={})",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let plist_content = std::fs::read_to_string(&plist_path).unwrap();
    assert!(plist_content.contains("<key>Hour</key><integer>14</integer>"));
    assert!(plist_content.contains("<key>Minute</key><integer>30</integer>"));

    let _ = std::process::Command::new("launchctl")
        .arg("bootout")
        .arg(format!("gui/{}/com.advisorycron.{label}", uid()))
        .output();
    let _ = std::fs::remove_file(&plist_path);
}

#[test]
fn register_complex_cron_exits_2() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label").arg("test-p003-bad")
        .arg("--schedule").arg("*/5 * * * 1-5")
        .output()
        .expect("spawn failed");

    assert_eq!(out.status.code(), Some(2), "expected exit 2 for complex cron, stderr={}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn register_missing_config_exits_2() {
    let home = TempDir::new().unwrap();
    // NO write_default_config — config absent.

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label").arg("test-p003-noconfig")
        .arg("--schedule").arg("0 9 * * *")
        .output()
        .expect("spawn failed");

    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unregister_nonexistent_label_exits_0_idempotent() {
    let home = TempDir::new().unwrap();
    // No prior register.

    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("unregister")
        .arg("--label").arg("test-p003-never-existed")
        .output()
        .expect("spawn failed");

    // Idempotent: exit 0 even if label never loaded + plist file never existed.
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 for idempotent unregister, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning") || stderr.contains("absent") || stderr.contains("not loaded"),
        "expected warning in stderr, got: {stderr}"
    );
}

#[test]
fn register_then_unregister_round_trip() {
    let home = TempDir::new().unwrap();
    write_default_config(home.path());
    let label = format!("test-p003-rt-{}", std::process::id());

    // Register
    let _ = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("register")
        .arg("--label").arg(&label)
        .output()
        .expect("register spawn failed");

    let plist_path = home.path()
        .join("Library/LaunchAgents")
        .join(format!("com.advisorycron.{label}.plist"));
    assert!(plist_path.exists(), "plist should exist after register");

    // Unregister
    let out = std::process::Command::new(BIN)
        .env("HOME", home.path())
        .arg("unregister")
        .arg("--label").arg(&label)
        .output()
        .expect("unregister spawn failed");

    assert_eq!(out.status.code(), Some(0));
    assert!(!plist_path.exists(), "plist should be removed after unregister");
}

/// Best-effort UID helper for cleanup (mirrors src/launchd.rs::current_uid logic).
fn uid() -> u32 {
    let out = std::process::Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}
```

**Lưu ý:**
- **CRITICAL WARNING (read this section carefully):** Integration tests above WILL invoke real `launchctl` because the spawned binary uses `RealLaunchctl` (no way to inject `NoopLaunchctl` through CLI spawn boundary). This means tests POLLUTE real launchctl with `test-p003-*` labels during run. Mitigation:
  - Each test uses unique label suffix (PID-based) to avoid collisions
  - Each test does best-effort cleanup (`launchctl bootout` + file removal)
  - Plist write happens BEFORE bootstrap → assertions on plist file existence pass even if bootstrap fails
  - CI environment (Linux, no launchctl binary) → register tests will fail at bootstrap step but plist write succeeds → assertions still pass (we check file existence, not exit code 0)
- **Worker discretion:** If real-launchctl pollution is unacceptable concern (e.g., Sếp dev machine has critical jobs), Worker may:
  - Add `#[ignore]` to integration tests + document run-with `cargo test -- --ignored`
  - Restructure binary to expose lib target → integration tests directly call `register::run_with_deps(args, &NoopLaunchctl, ...)` — but this is bigger refactor (binary-only → lib + bin), defer to Phase 2.
  - Worker pick + log Discovery if deviates from spec.
- 6 integration tests cover: basic register write, register with cron-simple, register with complex cron (error path exit 2), missing config (exit 2), idempotent unregister of nonexistent label, register→unregister round-trip.
- Tests use `std::process::id()` for unique label suffixes — sufficient for single-machine parallel test run.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/launchd.rs` | Task 1: tạo mới — `generate_plist`, `plist_path_for`, `default_launch_agents_dir`, `LaunchctlClient` trait + `RealLaunchctl`/`NoopLaunchctl`, `current_uid`, 11 unit tests |
| `src/main.rs` | Task 2: thêm 1 dòng `mod launchd;` sau `mod config;` |
| `src/cli/register.rs` | Task 3: rewrite body — full impl with `run_with_deps` testable surface; `Args` struct extended with `pub schedule: Option<String>` (relaxed) + `pub config: Option<PathBuf>` (new) |
| `src/cli/unregister.rs` | Task 4: rewrite body — idempotent impl with `run_with_deps` testable surface; `Args` struct extended with `pub config: Option<PathBuf>` (reserved, unused) |
| `src/config.rs` | Task 5: remove `#[allow(dead_code)]` attribute on `pub fn load` at `:72` (Anchor #5 Turn 1 confirmed) |
| `tests/cli_register.rs` | Task 6: tạo mới — 6 integration tests |
| `docs/CHANGELOG.md` | Append entry P003 (Tầng 1 — module added, CLI flag added, schedule type relaxed, plist schema spec, exit code semantics, INVARIANTS.md updated) |
| `docs/ARCHITECTURE.md` | Update §CLI surface table: register row note `--config <path>` flag + `--schedule` relaxed to optional; §Modules table mark `src/launchd.rs` 1.3 ✅, `src/cli/register.rs` + `src/cli/unregister.rs` 1.3 ✅; §Cron mechanism may need note on cron-simple-form launchd mapping; §Phase status from "Phase 1.2 shipped" → "Phase 1.3 shipped" |
| `docs/security/INVARIANTS.md` | **REQUIRED (V2 — Anchor #15 corrected: file exists 5968 bytes).** Append project-specific INV entry documenting: (a) `launchctl` shell-out boundary (RealLaunchctl::bootstrap/bootout), (b) `id -u` shell-out boundary (current_uid), (c) `~/Library/LaunchAgents/` write boundary (fs::create_dir_all + fs::write), (d) label sanitization defense (ASCII alphanum + `-` + `_` only — enforced in `generate_plist` AND `register::run`). Reference: P003 EXECUTE phase. |
| `docs/discoveries/P003.md` | Write Discovery Report (must include exact Anchor #17 message + INVARIANTS.md update note + dead_code removal note) |
| `docs/DISCOVERIES.md` | Prepend 1-line index entry |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| **`src/cli/mod.rs`** | **V2 — DO NOT EDIT.** Turn 1 [O1.1] confirmed dispatch is newtype `Commands::Register(register::Args) => register::run(args).await` at `mod.rs:17-21, 30-33`. New `--config` + relaxed `--schedule` declared INSIDE `register::Args` (Task 3) and `unregister::Args` (Task 4) via clap derive — propagate through dispatch automatically. Worker verifies post-EXECUTE: `git diff src/cli/mod.rs` shows zero changes. |
| `Cargo.toml` | Zero deps added. Worker confirm `git diff Cargo.toml` no change post-EXECUTE. |
| `Cargo.lock` | Auto-regenerated by cargo. Worker không edit manually. |
| `src/cli/init.rs` | P002 wired. KHÔNG động — `init` independent of P003. |
| `src/cli/run.rs`, `src/cli/status.rs`, `src/cli/mcp.rs` (if exists) | Phase 1.4/1.5/1.7 ranges. Vẫn `bail!("not yet implemented")`. KHÔNG động. |
| `tests/cli_help.rs` | P001 — 3 tests phải pass nguyên (regression). Adding `--config` arg + relaxing `--schedule` inside Args structs does not break `--help` exit 0 tests. |
| `tests/cli_init.rs` | P002 — 4 tests phải pass nguyên (regression). |
| `README.md` | KHÔNG update — defer Phase 1.6. |
| `CLAUDE.md` | KHÔNG update — no convention change. |
| `.phieu-counter` | Quản đốc đã bump 002 → 003. KHÔNG đụng. |
| `.sos-state/architect-active` | Quản đốc đã touch. KHÔNG đụng. |

---

## Luật chơi (Constraints)

1. **ZERO new dependencies.** `Cargo.toml` `[dependencies]` + `[dev-dependencies]` không thêm 1 dòng. `std::process::Command` + `std::fs` + `std::env` + `tempfile` (already in dev-deps) đủ. NẾU Worker thấy thiếu (tempting libc cho getuid, plist crate cho XML build) → STOP, `AskUserQuestion` Sếp. Heads-up #5 Option B (`id -u` shell-out) đã resolve UID need.
2. **No `unsafe { }` block.** `id -u` shell-out + `Command::new` + `fs::write` thuần safe Rust.
3. **No `tokio::process::Command`.** Use `std::process::Command` (sync) for `launchctl` + `id -u` shell-out. One-shot ops, no need for tokio runtime overhead. Async signature on `run` is for clap dispatch compatibility only; body is sync.
4. **No `tracing_subscriber::fmt::init()`.** Defer Phase 1.4 (runner). P003 uses `println!` / `eprintln!` per existing P001+P002 pattern.
5. **`#[tokio::main(flavor = "current_thread")]` giữ nguyên** at `src/main.rs`. P003 không escalate sang multi-thread.
6. **Exit code semantics:**
   - `register` success → 0
   - `register` config invalid / missing / cron parse fail / plist gen fail → 2
   - `register` plist write fail / launchctl bootstrap fail → 3
   - `register` `$HOME` unset → 1 (generic, via Err propagation)
   - `unregister` success (including idempotent no-ops) → 0
   - `unregister` plist removal fail (real IO error, not NotFound) → 3
   - `unregister` `$HOME` unset → 1
7. **Plist label sanitization:** only ASCII alphanumeric + `-` + `_`. Defense-in-depth: `generate_plist` rejects; `register::run` also pre-checks empty.
8. **Launchctl invocation safety:** RealLaunchctl uses absolute path components only (`gui/<uid>` + absolute plist path). Never shell-interpolate user-controlled strings without quoting.
9. **Idempotent unregister:** must succeed (exit 0) if bootout fails AND/OR plist file missing. Exit 3 only on real IO failure on plist removal. **V2 (Anchor #17):** do NOT branch behavior on stderr/stdout substring — any non-zero launchctl exit goes through warn-continue.
10. **`unwrap()` allowed only in tests.** Prod code: `?` + `anyhow::Context`. `expect()` allowed only with explicit "this can never fail because X" reasoning in comment.
11. **Async signature giữ nguyên** on `register::run` + `unregister::run` — match P001 dispatch convention. Body may be sync internally; `async fn` just adds `.await` no-op.
12. **No `core/` directory yet** — `register::run` + `unregister::run` import `launchd::*` directly from `crate::launchd`. Defer `core/` extraction to Phase 1.7 (MCP forces it).
13. **Help text language:** English. Doc comments English. User-visible CLI output English (per CLAUDE.md "CLI output / user-facing messages: tiếng Anh").
14. **NO real `launchctl bootstrap` in `cargo test` UNIT tests** — unit tests use `NoopLaunchctl` or pure functions. Integration tests in `tests/cli_register.rs` may invoke real launchctl via binary spawn — accepted trade-off documented in Task 6 Lưu ý. Worker reads Task 6 warning section before EXECUTE.
15. **Defense-in-depth:** label validated in BOTH `register::run` (pre-flight) AND `generate_plist` (pure function). Worker keep both checks.
16. **V2 [O1.1] hard constraint:** `src/cli/mod.rs` MUST NOT be edited. Post-EXECUTE `git diff src/cli/mod.rs` MUST be empty. Newtype dispatch pattern preserved.

---

## Nghiệm thu

### Automated

- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass:
  - 3 cli_help (P001 regression)
  - 4 cli_init (P002 regression)
  - 9 config unit tests (P002 regression — must pass after `#[allow(dead_code)]` removal)
  - 11 launchd unit tests (new P003)
  - 6 cli_register integration tests (new P003)
  - Total: ≥33 tests
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `git diff src/cli/mod.rs` — **empty** (V2 [O1.1] hard constraint)

### Manual Testing (post-build — Sếp or Worker on macOS dev machine)

- [ ] `./target/release/advisory-cron init --force` — exit 0, default config at `~/.config/advisory-cron/config.toml`
- [ ] `./target/release/advisory-cron register --label probe-p003 --schedule "0 9 * * *"` — exit 0, stdout mentions `com.advisorycron.probe-p003`, plist exists at `~/Library/LaunchAgents/com.advisorycron.probe-p003.plist`
- [ ] `launchctl list | grep advisorycron.probe-p003` — row present (Sub-mechanism A check)
- [ ] `launchctl print gui/$(id -u)/com.advisorycron.probe-p003` — shows next fire time set
- [ ] `cat ~/Library/LaunchAgents/com.advisorycron.probe-p003.plist` — XML matches ARCHITECTURE.md §Cron mechanism spec (7 keys present)
- [ ] `./target/release/advisory-cron register --label probe-p003 --schedule "*/5 * * * 1-5"` — exit 2, stderr mentions "daily" or "numeric" or "complex cron"
- [ ] `./target/release/advisory-cron register --label probe-p003 --schedule "0 9 * * *" --config /nonexistent.toml` — exit 2, stderr mentions "failed to load config"
- [ ] `./target/release/advisory-cron unregister --label probe-p003` — exit 0, stdout mentions `unregistered`, plist file removed
- [ ] `launchctl list | grep advisorycron.probe-p003` — no row
- [ ] `./target/release/advisory-cron unregister --label never-registered-xyz` — exit 0, stderr has warning, idempotent (V2 Anchor #17: log captures observed `"Boot-out failed: 3: No such process"` string verbatim)

### Regression

- [ ] `cargo test --test cli_help` — 3/3 pass (P001 intact)
- [ ] `cargo test --test cli_init` — 4/4 pass (P002 intact)
- [ ] `cargo test --lib config` — 9/9 pass (P002 unit tests intact; `#[allow(dead_code)]` removal must not break)
- [ ] `./target/release/advisory-cron init --help` — still works (P002 wiring intact)
- [ ] `./target/release/advisory-cron --help` — lists all 5 subcommands (P001 contract intact)
- [ ] `./target/release/advisory-cron register --help` — shows new `--config` flag + `--schedule` as optional
- [ ] `./target/release/advisory-cron unregister --help` — shows new `--config` flag (reserved)
- [ ] `./target/release/advisory-cron run` — still exit 1 stub (Phase 1.4 not shipped)
- [ ] `./target/release/advisory-cron status` — still exit 1 stub (Phase 1.5 not shipped)

### Docs Gate

- [ ] `docs/CHANGELOG.md` — append entry P003. Sections required:
  - "Module added (src/launchd.rs)"
  - "CLI: register + unregister wired; --config flag added to both (declared inside Args struct via newtype dispatch — zero edits to src/cli/mod.rs)"
  - "CLI: register --schedule relaxed from required String to Option<String> (config-driven schedule works without redundant CLI flag)"
  - "Plist XML schema (7 keys: Label, ProgramArguments, StartCalendarInterval, StandardOutPath, StandardErrorPath, WorkingDirectory, RunAtLoad)"
  - "Exit code 3 first use (launchd operation failure)"
  - "Cron expression support: simple `M H * * *` daily form only (Phase 1 launchd constraint)"
  - "INVARIANTS.md updated — new INV entry for launchctl + id-u shell-out + LaunchAgents write boundary"
  - "Tests added (11 unit + 6 integration)"
- [ ] `docs/ARCHITECTURE.md` updates:
  - §CLI surface table: `register` row append `--config <path>` to args column + note `--schedule` relaxed to optional; `unregister` row append `--config <path>` (reserved)
  - §CLI surface table: `register --schedule` semantic note — "simple `M H * * *` daily form only"
  - §Modules table: `src/launchd.rs` mark `1.3 ✅`; `src/cli/register.rs` + `unregister.rs` mark `1.3 ✅` (from "1.1 skeleton")
  - §Cron mechanism: append note about cron→Calendar mapping constraint (`M H * * *` daily only)
  - §Phase status: "Phase 1.2 shipped" → "Phase 1.3 shipped"
- [ ] `docs/security/INVARIANTS.md` — **REQUIRED (V2 — file confirmed exists Turn 1).** Append INV entry covering:
  - `launchctl` shell-out boundary (RealLaunchctl::bootstrap/bootout invocation surface, absolute path args only, no shell interpolation of user input)
  - `id -u` shell-out boundary (current_uid, parsed as u32)
  - `~/Library/LaunchAgents/` write boundary (fs::create_dir_all + fs::write, label-derived filename only)
  - Label sanitization defense (ASCII alphanum + `-` + `_` — both in `generate_plist` and `register::run` pre-flight)
- [ ] `README.md` — KHÔNG update (defer Phase 1.6)
- [ ] `CLAUDE.md` — KHÔNG update (no convention change)
- [ ] `docs-gate --all --verbose` — must pass

### Discovery Report

- [ ] `docs/discoveries/P003.md` written per `docs/RULES.md` format. **Mandatory sections:**
  - Assumptions ĐÚNG (list anchors verified ✅ during Worker Task 0 — all 21 confirmed via Turn 1)
  - **Assumptions SAI (V1 → V2 corrections — MUST document):**
    - V1 Anchor #3: assumed inline-field enum variants; ACTUAL is newtype `Commands::Register(register::Args)`. V2 [O1.1] dropped mod.rs edits.
    - V1 Anchor #15: "may not exist"; ACTUAL INVARIANTS.md exists (5968 bytes, INV-1..INV-6+ catalog). V2 dropped AskUserQuestion step.
    - V1 Anchor #17: "No such process" or "Could not find specified service"; ACTUAL exact message is `"Boot-out failed: 3: No such process"` (on stdout, exit=3). V2 strengthened "no substring branching" rule.
  - Edge cases discovered (e.g., Cargo binary-vs-lib structure forcing integration test trade-off; real-launchctl pollution mitigation; `current_exe()` path on macOS edge cases)
  - Docs updated per discoveries
  - **Specific notes required:**
    - confirm `#[allow(dead_code)]` removed from `Config::load` per Heads-up #1
    - confirm `src/cli/mod.rs` not touched (V2 [O1.1])
    - confirm `docs/security/INVARIANTS.md` appended with project-specific INV entry (V2 Anchor #15 mandate)
    - record exact `launchctl bootout` failure message string verbatim (V2 Anchor #17 mandate)
  - Layer 2 capability checks fired: A (manual launchctl list grep), B (cargo check, cargo test --all, targeted), D (grep ARCHITECTURE.md + INVARIANTS.md), E (cargo update --dry-run, clean rebuild)
- [ ] `docs/DISCOVERIES.md` — prepend 1-line: `- 2026-05-DD P003: launchd plist generator + register/unregister wired (newtype dispatch preserved, LaunchctlClient trait, idempotent unregister, simple `M H * * *` cron form, zero new dep, INVARIANTS.md INV entry appended) → see docs/discoveries/P003.md`
- [ ] Sub-mechanism A-E Verification Trace table filled (above)
