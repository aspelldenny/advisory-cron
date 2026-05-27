# PHIẾU P007: Phase 1.6 — README + ARCHITECTURE post-ship docs polish

> **Filename:** `docs/ticket/P007-readme-architecture-polish.md`
> **Branch:** `docs/P007-readme-architecture-polish`

---

> **Loại:** docs
> **Tầng:** 2
> **Ưu tiên:** P1
> **Ảnh hưởng:** `README.md`, `docs/ARCHITECTURE.md`
> **Dependency:** P006 (đã ship — đây là phiếu post-ship, gate cuối của sprint Phase 1)

---

## Context

### Vấn đề hiện tại

BACKLOG.md item "Phase 1.6 — README + ARCHITECTURE.md" là gate cuối của Active sprint. P001–P006 đã ship lần lượt: scaffold CLI → config → launchd plist → runner+heartbeat → status → MCP wrapper. Mỗi phiếu Tầng 1 đã tự cập nhật docs theo scope của nó, nhưng:

1. **README.md** đã được P006 cập nhật với MCP section + smoke test. Hiện trạng đã có CLI quick-start + MCP quick-start + Claude Desktop config snippet + status banner ("Phase 1.7 shipped"). Tuy nhiên P006 viết docs trong cùng commit với code → chưa được "dogfooded": Worker phải CHẠY thực tế từng lệnh trong README để confirm output khớp, không có lỗi typo/sai flag/sai path.
2. **ARCHITECTURE.md** Modules table có cột "Phase ships" markup `1.7 ✅` etc. Section "Layering invariant (introduced Phase 1.7)" hiện viết "introduced" — sau khi ship, nên đổi thành "shipped". Phase status text dòng cuối: `Phase 1 — In progress (1.7 shipped; 1.6 docs remaining)` → sau P007 ship phải thành `Phase 1 — Code COMPLETE; docs polish (1.6) shipped; awaiting Sếp dogfood 3 ngày để close sprint`.
3. **MCP tool schemas** trong ARCHITECTURE.md §MCP surface table phải khớp chính xác hand-written schemas tại `src/mcp/tools.rs`. P006 ship docs cùng commit code nên về lý thuyết đã khớp; P007 Worker verify lại field-by-field bằng cách READ source.
4. **Sub-mech A verification step** chưa có docs hint cho Sếp về cách kiểm tra plist đã thực sự register sau `advisory-cron register` — README dừng ở "register" không dạy `launchctl list | grep com.advisorycron`. Thêm 1 dòng "verify" để Sếp dogfood không bị mù.

### Giải pháp

Worker dogfood-verify README + cross-check ARCHITECTURE vs source, sửa drift, KHÔNG đổi code. Phiếu này pure docs — Tầng 2 mềm.

**Workflow:**
- Worker đọc src/cli/*, src/core/*, src/mcp/* để biết shipped reality (Architect KHÔNG đọc source — đó là job Worker per envelope).
- Worker chạy các lệnh trong README (sau `cargo install --path .`) và verify output không lỗi.
- Worker đối chiếu MCP tool schemas trong ARCHITECTURE.md §MCP surface vs `src/mcp/tools.rs` JSON schema literals.
- Sửa bất cứ drift nào tìm thấy. Note vào Discovery Report nếu có sai lệch.

### Scope

- CHỈ sửa `README.md`, `docs/ARCHITECTURE.md`.
- KHÔNG sửa bất kỳ file `src/**`, `tests/**`, `Cargo.toml`, `Cargo.lock`.
- KHÔNG thêm test mới.
- KHÔNG đổi mục tiêu hoặc spec — chỉ docs polish theo shipped reality.

### Skills consulted (optional)

*(Không invoke skill — phiếu Tầng 2 docs polish, Architect đã có đủ context từ docs hiện có.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Phiếu Tầng 2 — anchor lighter (per orchestrator heads-up). Architect đọc docs (README.md, ARCHITECTURE.md, CHANGELOG.md, PROJECT.md) nhưng KHÔNG đọc source code. Mọi anchor về source là `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | README.md hiện đã có "Quick start (CLI)" section với 4 lệnh (`init`, `register`, `run`, `status`) | Read README.md L23-37 | `[verified]` | ✅ L23-37 |
| 2 | README.md hiện đã có "MCP server" section với Claude Desktop config snippet | Read README.md L39-79 | `[verified]` | ✅ L39-79 |
| 3 | README.md status banner ghi "Phase 1.7 shipped" | Read README.md L81-83 | `[verified]` | ✅ L83 |
| 4 | README.md THIẾU `advisory-cron unregister` example trong CLI quick-start | Read README.md L23-37 (chỉ có init/register/run/status — 4 lệnh, không có unregister) | `[verified]` | ✅ Cần thêm step 5 cho unregister |
| 5 | ARCHITECTURE.md §Modules có 17 hàng (cli/* x6 + core/* x7 + mcp/* x3 + supporting x1 = 17 — bao gồm `src/main.rs`, `src/config.rs`, `src/launchd.rs`, `src/runner.rs`, `src/heartbeat.rs`) | Đếm rows trong table L33-56 | `[verified]` | ✅ 22 rows (1 main + 6 cli + 7 core + 3 mcp + 5 supporting). Lưu ý orchestrator brief ghi "17 modules" nhưng đếm thực ARCHITECTURE = 22 rows. Worker đếm lại bằng wc + verify file count thật sự trong `src/`. |
| 6 | ARCHITECTURE.md §MCP surface table có 5 rows cho 5 tools (`init`/`register`/`unregister`/`run`/`status`) — line ~216-222 | Read ARCHITECTURE.md L216-222 | `[verified]` | ✅ 5 rows |
| 7 | ARCHITECTURE.md §Layering invariant hiện ghi "introduced Phase 1.7" (cần đổi thành "shipped") | Read ARCHITECTURE.md L60 | `[verified]` | ✅ L60 "Layering invariant (introduced Phase 1.7)" |
| 8 | ARCHITECTURE.md §Phase status dòng cuối ghi "In progress (1.7 shipped; 1.6 docs remaining)" | Read ARCHITECTURE.md L300 | `[verified]` | ✅ L300 |
| 9 | `src/mcp/tools.rs` chứa hand-written JSON schemas cho 5 tools, fields khớp với ARCHITECTURE.md L216-222 table | grep `inputSchema` hoặc `serde_json::json!` trong src/mcp/tools.rs | `[needs Worker verify]` | ⏳ Worker grep + so sánh field-by-field |
| 10 | `src/main.rs` declare modules: `mod cli; mod config; mod launchd; mod runner; mod heartbeat; mod core; mod mcp;` (7 top-level mods) | `grep "^mod " src/main.rs` | `[needs Worker verify]` | ⏳ Worker grep |
| 11 | Binary đã install ở `~/.cargo/bin/advisory-cron` (Worker cần `cargo install --path .` trước khi dogfood README quick-start) | `which advisory-cron` HOẶC `cargo install --path .` xong | `[needs Worker verify]` | ⏳ Worker run cargo install |
| 12 | `advisory-cron --help` output liệt kê 6 subcommands: `init`, `register`, `unregister`, `run`, `status`, `mcp` | `advisory-cron --help` exit 0 + grep từng tên | `[needs Worker verify]` | ⏳ Worker run + capture |
| 13 | `advisory-cron register --help` output có flags `--label <NAME>` + `--schedule <CRON>` + `--config <PATH>` | `advisory-cron register --help` | `[needs Worker verify]` | ⏳ Worker run + capture |
| 14 | `advisory-cron status --help` output có flags `--label <NAME>` + `--config <PATH>` + `--json` + `--last <N>` | `advisory-cron status --help` | `[needs Worker verify]` | ⏳ Worker run + capture |
| 15 | PROJECT.md Vision para có 1-câu mô tả tool: "local cron wrapper that fires periodic Claude Code tasks ... via macOS launchd (Phase 1) or Linux cron/systemd (Phase 2+)" | Read PROJECT.md L9 | `[verified]` | ✅ L9 — README hiện đã borrow phrasing tốt rồi (L5) |
| 16 | LICENSE file tồn tại tại repo root (README hiện reference `[LICENSE](LICENSE)` L87) | `Glob LICENSE` | `[verified]` | ✅ existing per CHANGELOG L343 bootstrap inventory |

---

## Debate Log

> Phiếu Tầng 2 — per orchestrator handbook autonomous mode, CHALLENGE round có thể skip nếu Worker không tìm thấy objection. Phiếu sẽ ship V1 nếu Worker accept.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

*(Worker fill phần này khi invoked CHALLENGE mode — chú ý: phiếu Tầng 2 thường accept luôn V1. Tuy nhiên nếu Worker phát hiện anchor #5 sai (số module ≠ 22) hoặc anchor #9 phát hiện MCP schema drift giữa ARCHITECTURE và src — RAISE objection để Architect refine V2.)*

**Anchor verification (recap từ Verification Anchors):**
- Anchor #N: ✅/⚠️/❌ + 1 dòng tóm tắt nếu ⚠️/❌

**Objections (Tầng 2 — Worker thường accept V1):**
- *(Worker tự ghi nếu có. Tầng 2 không cần raise objection cho việc tự quyết wording / typo — đó là Worker's call per Tầng 2 rule.)*

**Status:** ⏳ AWAITING WORKER CHALLENGE-OR-ACCEPT

### Turn 1 — Architect Response

*(Architect fill phần này khi invoked RESPOND mode — chỉ khi Worker raise objection. Nếu Worker accept V1, skip thẳng Final consensus.)*

**Status:** N/A nếu Worker accept V1

### Final consensus

- Phiếu version: V<N>
- Total turns: <count>
- Approved: [date] — execution may begin

---

## Debug Log

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

*(Worker fill during EXECUTE.)*

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | `launchctl list \| grep com.advisorycron` | row present after Worker dogfood-register test label | | |
| B (capability) | `cargo check` | exit 0 (no code change nhưng vẫn chạy để confirm repo clean) | | |
| B (capability) | `cargo install --path .` | exit 0; binary at `~/.cargo/bin/advisory-cron` | | |
| B (capability) | `advisory-cron --version` | prints `advisory-cron 0.1.0` | | |
| C (migration) | N/A | (no schema change) | | N/A |
| D (persistence) | `grep -l "Layering invariant" docs/ARCHITECTURE.md` | ≥1 hit (durable doctrine still findable) | | |
| E (env drift) | `cargo build --release` clean target | exit 0 | | |
| E (env drift) | docs-gate `--all --verbose` | pass | | |

---

## Nhiệm vụ

### Task 1: Worker pre-flight — install binary + capture --help output cho 6 subcommands

**File:** N/A (mechanical — không sửa file)

**Tìm:** N/A

**Làm:**
1. `cargo install --path . --force` để có binary tươi nhất ở `~/.cargo/bin/advisory-cron`.
2. Run + capture stdout (lưu trong head Debug Log) cho:
   - `advisory-cron --help`
   - `advisory-cron --version`
   - `advisory-cron init --help`
   - `advisory-cron register --help`
   - `advisory-cron unregister --help`
   - `advisory-cron run --help`
   - `advisory-cron status --help`
   - `advisory-cron mcp --help`
3. Compare với anchor #12-14 — nếu khớp, anchor → ✅. Nếu sai (e.g. flag rename phát hiện), anchor → ❌ và Worker phải hoặc (a) raise CHALLENGE Turn 1 để Architect refine, hoặc (b) Tầng 2 self-decide nếu sai khác chỉ là wording trong help text (KHÔNG nếu là flag name).

**Lưu ý:**
- Đây là Task 0 verification: phải làm TRƯỚC khi sửa README hoặc ARCHITECTURE để biết shipped reality.
- Nếu `cargo install` fail → STOP, escalate Sếp (binary build vỡ tức là phiếu không có cơ sở để dogfood — không phải scope P007).
- Heartbeat của Worker's smoke run nên trỏ vào tmp path (`HOME=$(mktemp -d)`) để không đụng config thật của Sếp.

---

### Task 2: README.md — bổ sung `unregister` step + verify step + license + verify CLI snippet output khớp

**File:** `README.md`

**Tìm 1 — CLI quick-start hiện 4 bước:** từ `## Quick start (CLI)` L23 đến cuối block code L37 (4 lệnh `init` / `register` / `run` / `status`).

**Thay bằng:** mở rộng thành 6 bước, thêm step verify-loaded sau register + step unregister cuối:

```bash
# 1. Write default config to ~/.config/advisory-cron/config.toml
advisory-cron init

# 2. Register a launchd plist that fires daily at 09:00
advisory-cron register --label advisory-scan-daily --schedule "0 9 * * *"

# 3. Verify the plist is loaded
launchctl list | grep com.advisorycron

# 4. Fire the configured task immediately (one-shot test)
advisory-cron run

# 5. Show launchd state + last 5 heartbeats
advisory-cron status --label advisory-scan-daily

# 6. Unregister when done testing
advisory-cron unregister --label advisory-scan-daily
```

**Lưu ý:**
- Step 3 (`launchctl list | grep`) là Sub-mechanism A verification — closes the trigger-gap doctrine từ CLAUDE.md.
- Label `advisory-scan-daily` là literal example — không cần shell var.
- Worker confirm từng lệnh chạy được sau `cargo install --path .` (lưu Debug Log).

---

### Task 3: README.md — verify MCP smoke test snippet thực sự trả về JSON khớp

**File:** `README.md`

**Tìm:** L72-79 block "Quick smoke test" — `echo '{"jsonrpc":"2.0",...}' | advisory-cron mcp`.

**Làm:**
1. Worker run lệnh đó từng-chữ-một.
2. Confirm stdout chứa `"serverInfo"` + `"name":"advisory-cron"`.
3. Nếu khớp: KHÔNG sửa README — anchor pass.
4. Nếu sai (e.g. server reply khác format do rmcp 1.7.0 nhả): sửa README expected-line cho khớp realität. Note vào Discovery Report — đây là drift Sếp dogfood sẽ cần.

**Lưu ý:**
- MCP server đọc 1 frame rồi block waiting more. `echo | ...` đóng stdin sau frame đầu tiên → server EOF → exit theo `Ok(0)` hoặc `Ok(5)` (V2 cli/mcp.rs contract). Worker confirm exit code = 0 hoặc = 5 (tùy rmcp interpret EOF) — không phải panic.

---

### Task 4: README.md — license pointer + bổ sung optional "What this fires" example

**File:** `README.md`

**Tìm 1:** L85-87 status + license block (đã có).

**Thay bằng — KHÔNG ĐỘNG license block (đã chính xác).**

**Tìm 2 (insert NEW section):** sau "Quick smoke test" block (L79), TRƯỚC `## Status` (L81), insert:

```markdown
## What advisory-cron fires

Out of the box, `advisory-cron init` writes a config that runs `claude -p /advisory-scan` (sos-kit's vulnerability scanner) daily at 09:00. To fire something else, edit `~/.config/advisory-cron/config.toml`:

```toml
[task]
command = "claude"
args = ["-p", "/my-slash-command"]
working_dir = "/Users/<YOU>/some-repo"
label = "my-task"

[schedule]
hour = 9
minute = 0

[heartbeat]
log_path = "/Users/<YOU>/.local/state/advisory-cron/heartbeat.jsonl"
```

Re-register after editing: `advisory-cron unregister --label my-task && advisory-cron register --label my-task`.
```

**Lưu ý:**
- Mục đích: Sếp / future kit consumer hiểu tool dùng cho gì ngoài `/advisory-scan`.
- Path placeholders dùng `<YOU>` — không hard-code `nguyenhuuanh`.
- Worker verify TOML block parse được: `advisory-cron init` hiện sinh ra block với cùng schema → so sánh field name từng dòng.

---

### Task 5: ARCHITECTURE.md — đổi "introduced Phase 1.7" → "shipped Phase 1.7"

**File:** `docs/ARCHITECTURE.md`

**Tìm:** L60 — `**Layering invariant (introduced Phase 1.7):**`

**Thay bằng:** `**Layering invariant (shipped Phase 1.7):**`

**Lưu ý:** 1-word swap. Mục đích: phản ánh shipped reality thay vì "introduced" (làm như đang trong quá trình).

---

### Task 6: ARCHITECTURE.md — Phase status text reflect "Phase 1 code COMPLETE, awaiting dogfood"

**File:** `docs/ARCHITECTURE.md`

**Tìm:** L300 — block `- ✅ **Phase 1** — In progress (1.7 shipped; 1.6 docs remaining). Phase 1.1 shipped: ...`

**Thay bằng:** giữ TOÀN BỘ phần liệt kê 1.1-1.7 (nội dung khoa học không đổi), chỉ thay tiền tố:

```
- ✅ **Phase 1** — Code COMPLETE (all 7 sub-phases shipped). Awaiting Sếp dogfood 3 ngày để close sprint per BACKLOG acceptance. <giữ nguyên phần phase-by-phase liệt kê 1.1 shipped: ... 1.7 shipped: ... Phase 1.6 (README + ARCHITECTURE docs polish) shipped per P007.>
```

**Lưu ý:**
- Chỉ đổi opening line + cuối thêm "Phase 1.6 shipped per P007".
- KHÔNG đụng nội dung 1.1-1.7 vì đó là log lịch sử ship.

---

### Task 7: ARCHITECTURE.md — verify MCP tool schemas khớp source

**File:** `docs/ARCHITECTURE.md`

**Tìm:** L216-222 block §MCP surface "Tool registry" table — 5 rows liệt kê input schema cho mỗi tool.

**Làm:**
1. Open `src/mcp/tools.rs`. Grep `serde_json::json!` hoặc `"inputSchema"` để locate 5 schema literals.
2. Field-by-field, so sánh ARCHITECTURE.md row vs source:
   - `init`: ARCHITECTURE = `{ force?: bool, config_path?: string }` → source phải có 2 optional fields cùng tên.
   - `register`: ARCHITECTURE = `{ label: string (required), schedule?: string, config_path?: string }` → source phải có `label` required + 2 optional.
   - `unregister`: ARCHITECTURE = `{ label: string (required), config_path?: string }`
   - `run`: ARCHITECTURE = `{ config_path?: string }`
   - `status`: ARCHITECTURE = `{ label?: string, config_path?: string, last?: int (default 5) }`
3. Nếu khớp: KHÔNG sửa ARCHITECTURE table — anchor #9 pass.
4. Nếu sai: sửa ARCHITECTURE row cho khớp source (source là ground truth — code đã ship). Note drift vào Discovery Report.

**Lưu ý:**
- Đây là PURE verification step — kết quả tích cực = không sửa file. Tích cực có giá trị: confirm docs khớp shipped state.
- Nếu drift phát hiện, đó là P006 đã ship docs hơi lệch — P007 sửa drift, ghi Discovery để Architect rút kinh nghiệm.

---

### Task 8: ARCHITECTURE.md — Modules table sanity check

**File:** `docs/ARCHITECTURE.md`

**Tìm:** L31-56 block §Modules table.

**Làm:**
1. `ls src/` + `ls src/cli/` + `ls src/core/` + `ls src/mcp/` để đếm thật sự bao nhiêu module files exist.
2. Verify mỗi file trong `src/**` đều có 1 row trong table (no missing entry).
3. Verify mỗi row trong table point tới 1 file thật (no phantom entry).
4. Nếu mismatch (phát hiện file mới quên add hoặc row dư): sửa table cho khớp `ls`. Note vào Discovery Report.

**Lưu ý:**
- Anchor #5 Architect đã verify rough count = 22 rows; orchestrator brief ghi 17. Worker phán quyết bằng `ls`-based ground truth.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `README.md` | Task 2 (CLI quick-start 4→6 bước + verify step), Task 3 (verify MCP smoke + có thể sửa expected output), Task 4 (insert "What advisory-cron fires" section) |
| `docs/ARCHITECTURE.md` | Task 5 ("introduced"→"shipped" L60), Task 6 (Phase status reflect code-COMPLETE L300), Task 7 (verify + có thể sửa MCP schemas L216-222), Task 8 (verify + có thể sửa Modules table L31-56) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/main.rs` | `mod` declarations match ARCHITECTURE Modules table |
| `src/cli/*.rs` | Existence + clap-derive args match README + ARCHITECTURE §CLI surface |
| `src/core/*.rs` | Existence match ARCHITECTURE Modules table |
| `src/mcp/tools.rs` | Hand-written JSON schemas match ARCHITECTURE §MCP surface Tool registry |
| `Cargo.toml` | No edit; verify version still `0.1.0` (for README `--version` example) |
| `LICENSE` | Exists at repo root (README references it) |

---

## Luật chơi (Constraints)

1. **No code change.** KHÔNG sửa bất kỳ file `src/**`, `tests/**`, `Cargo.toml`, `Cargo.lock`. Vi phạm = phiếu rejected. Phiếu Tầng 2 docs-only.
2. **No new dep.** Không thêm crate vào `Cargo.toml`.
3. **No test add/edit.** Test suite hiện tại 94 pass — không động.
4. **Dogfood from temp config dir.** Khi Worker run `advisory-cron init` / `register` / `run` để verify README, dùng `HOME=$(mktemp -d)` để không đụng config thật của Sếp tại `~/.config/advisory-cron/`. Sau verify, không cần xóa temp dir (mktemp tự cleanup khi reboot).
5. **No `launchctl bootstrap` against Sếp's session for verify.** Hard Stops rule 9 — không clobber existing labels. Nếu Worker cần verify launchctl shell-out hoạt động: dùng test label như `p007-verify-temp-do-not-use`, register → confirm `launchctl list` shows it → unregister NGAY → confirm `launchctl list` không còn nó. Discovery Report ghi rõ "test label used + cleaned up".
6. **Source is truth, docs reflect source.** Nếu Task 7 hoặc Task 8 phát hiện drift, source thắng — sửa docs cho khớp code. KHÔNG sửa code để khớp docs (đó là Tầng 1, ngoài scope phiếu).
7. **No "future-proofing" of README.** Đừng thêm Phase 2 / Phase 3 doc placeholder — README chỉ ship Phase 1 reality. Phase 2 sẽ có phiếu riêng.

---

## Nghiệm thu

### Automated

- [ ] `cargo build --release` — zero warnings (sanity check; không có code change nên không expected fail)
- [ ] `cargo test --all` — 94/94 pass (baseline maintained)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `cargo install --path .` — exit 0; binary at `~/.cargo/bin/advisory-cron`

### Manual Testing

- [ ] `advisory-cron --help` lists 6 subcommands (`init`, `register`, `unregister`, `run`, `status`, `mcp`)
- [ ] `advisory-cron --version` prints `advisory-cron 0.1.0`
- [ ] Run mỗi lệnh trong README "Quick start (CLI)" section (1-6) using `HOME=$(mktemp -d)` — mỗi lệnh exit 0
- [ ] Smoke test MCP snippet (README L72-79) — stdout chứa `"name":"advisory-cron"` JSON
- [ ] `launchctl list | grep com.advisorycron.p007-verify-temp-do-not-use` shows row sau register, không còn sau unregister

### Regression

- [ ] `cargo test --all` vẫn 94 pass (baseline)
- [ ] `git diff src/ tests/ Cargo.toml Cargo.lock` empty (Constraint #1 + #3 enforce)
- [ ] `git diff README.md docs/ARCHITECTURE.md` không-empty (chứng minh phiếu đã sửa cái gì)

### Docs Gate

- [ ] `docs/CHANGELOG.md` — entry P007 đã append (newest at top): title, summary 1 đoạn về docs polish + verification dogfood log
- [ ] `docs/ARCHITECTURE.md` — Tasks 5, 6, 7, 8 done
- [ ] `README.md` — Tasks 2, 3, 4 done
- [ ] `docs-gate --all --verbose` — PASS

### Discovery Report

- [ ] `docs/discoveries/P007.md` — full report written:
  - List anchor verification results (which were ✅, which ❌ require fix)
  - If any MCP schema drift / Modules table drift found, document the exact field/row
  - Note: this is the FIRST Tầng 2 phiếu in sprint — note Architect approach (lighter anchor count) worked / didn't
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top): `2026-MM-DD P007: README + ARCHITECTURE post-ship polish (6-step CLI quick-start; MCP smoke verified; <N> drift items fixed) → see docs/discoveries/P007.md`
- [ ] Sub-mechanism A-E Verification Trace filled (table above)

---

## Notes for Worker (Tầng 2 reminder)

- Phiếu Tầng 2 = Worker self-decides Tầng 2 details (wording, exact bullet count, placeholder format `<YOU>` vs `<USER>` vs `${USER}`). Architect prescribed structure; Worker can polish prose.
- Nếu phát hiện README có bug khác (e.g. broken link, outdated `Status:` banner referencing wrong phase) ngoài scope của 4 Task trên → fix nếu trivial (Tầng 2 self-decide), report Discovery. Nếu non-trivial (e.g. rebrand, restructure section) → STOP, escalate.
- Sprint close gate: P007 ship → Sếp dogfood 3 ngày → BACKLOG move Phase 1 items to "Recently shipped". Worker KHÔNG tự move BACKLOG items — đó là Sếp's action sau dogfood.
