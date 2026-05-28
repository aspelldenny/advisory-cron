# PHIẾU P015: README Linux quick-start — 2-OS paths + Phase 3 status banner

> **Phiếu:** P015
> **Phase:** 3.4 (final phiếu của Phase 3 sprint)
> **Branch:** `feat/P015-readme-linux-quickstart`
> **Filename:** `docs/ticket/P015-readme-linux-quickstart.md`

---

> **Loại:** docs
> **Tầng:** 2 (docs only — README.md update; tier routing: DRAFT → APPROVAL_GATE → EXECUTE, Worker CHALLENGE skipped per orchestrator handbook)
> **Ưu tiên:** P1
> **Ảnh hưởng:** `README.md` (single file)
> **Dependency:** P012 (Phase 3.1), P013 (Phase 3.2), P014 (Phase 3.3) đã merge — Linux cron-tab impl + INV-22/23 + CI matrix đã ship

---

## Context

### Vấn đề hiện tại

Phase 3 sprint xong 3/4 phiếu (P012 trait extract, P013 Linux impl, P014 INV docs + CI matrix). `README.md` hiện tại (post-P010 polish) chỉ mô tả **macOS launchd path** + status banner "Phase 1 + Phase 2 COMPLETE". Linux user clone repo sẽ:

- Đọc Quick start → nghĩ tool chỉ chạy macOS → bỏ qua
- Đọc Status banner → nghĩ Phase 3 chưa ship → không biết Linux đã usable
- Đọc MCP section → thấy `~/Library/Application Support/Claude/...` macOS-specific path → không biết Linux Claude Desktop ở đâu

Phase 3 sprint **acceptance criterion** cuối (BACKLOG.md dòng 31):
> README quick-start có 2 paths (macOS launchd + Linux cron) với verification command (`crontab -l | grep advisory-cron` = Sub-mechanism A trigger gap check).

Phải đóng acceptance này TRƯỚC khi Sếp dogfood Linux 1 ngày → sprint close.

### Giải pháp

Edit `README.md` only:

1. **Split Quick start section** thành 2 sequential subsections — macOS (existing, preserved verbatim except heading rename) + Linux (new, mirror sequence with `crontab` verification commands).
2. **MCP section** thêm 1-sentence note OS-agnostic về binary path (`~/.cargo/bin/advisory-cron` cho cả 2 OS nếu `cargo install`).
3. **Status banner** bump "Phase 1 + Phase 2 COMPLETE" → "Phase 1 + Phase 2 + Phase 3 COMPLETE — macOS launchd + Linux cron-tab dual-platform shipped" + 1-line stats (single binary, 2-OS, 23 INVs, ~145 tests cross-OS).
4. Zero touch `src/`, `Cargo.toml`, `INVARIANTS.md`, `ARCHITECTURE.md` (P014 đã ship Phase 3.3 status + §CI matrix).

### Scope
- CHỈ sửa `README.md`
- KHÔNG sửa: `src/**`, `Cargo.toml`, `Cargo.lock`, `docs/ARCHITECTURE.md`, `docs/security/INVARIANTS.md`, `docs/PROJECT.md`, `docs/BACKLOG.md` (Quản đốc owns post-merge BACKLOG move)

### Skills consulted (optional)
Không (Tầng 2 docs, không cần skill).

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Tầng 2 docs phiếu — Anchors mostly `[verified]` vì Architect đã Read `README.md` + `CHANGELOG.md` + `ARCHITECTURE.md` + 2 Discovery Reports. Worker EXECUTE chạy end-to-end Linux quick-start trên WSL2 thật → mỗi command có stdout/exit_code → Discovery Report.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `README.md` hiện có heading `## Quick start (CLI)` (line 23) | Architect Read README dòng 23 | `[verified]` | ✅ Dòng 23 |
| 2 | `README.md` hiện có heading `## MCP server (Claude Desktop / Claude Code)` (line 45) | Architect Read README dòng 45 | `[verified]` | ✅ Dòng 45 |
| 3 | `README.md` hiện có heading `## Status` (line 159) với content "Phase 1 + Phase 2 COMPLETE (all 10 phiếu shipped)" | Architect Read README dòng 159-161 | `[verified]` | ✅ Dòng 159-161 |
| 4 | `README.md` hiện có heading `## What advisory-cron fires` (line 87) — P007-shipped, vẫn còn valid cho both OS | Architect Read README dòng 87-106 | `[verified]` | ✅ TOML snippet dùng `/Users/<YOU>/...` placeholder — Linux user phải chỉnh tay |
| 5 | `crontab -l \| grep "# advisory-cron:"` là verification command đúng (tag format match `src/scheduler/linux.rs` TAG_PREFIX) | P013 Discovery Report dòng 14 confirms tag = `"# advisory-cron: <label>"` substring match | `[verified per P013 Discovery]` | ✅ |
| 6 | Linux dogfood smoke P013: `advisory-cron register --label p013-smoke` → exit 0 + 1 tagged line; `unregister` → exit 0 + 0 lines; INV-22 `foo;evil` reject → exit 2 | P013 Discovery edge case #7 documents exact commands + observed outputs | `[verified per P013 Discovery]` | ✅ Worker EXECUTE re-runs với label `p015-smoke` |
| 7 | `advisory-cron status --label <label>` Linux render = `plist_loaded: true, next_fire: null` (P013 limitation, không phải bug) | P013 Discovery edge case #7 + ARCHITECTURE.md §Cron mechanism Linux dòng 268 confirm `next_fire` = None on Linux | `[verified per P013 Discovery + ARCHITECTURE]` | ✅ README phải note "next fire shows N/A on Linux Phase 3 — P014 INV-23 deferred" |
| 8 | `cargo install --path .` lands binary tại `~/.cargo/bin/advisory-cron` cả macOS lẫn Linux (cargo default `CARGO_HOME`) | Architect không Read shell config — đây là cargo upstream behavior | `[unverified]` | ⏳ Worker confirm tại EXECUTE bằng `ls ~/.cargo/bin/advisory-cron` post-install |
| 9 | Claude Desktop Linux config path khác macOS (`~/.config/Claude/claude_desktop_config.json` thường, không phải `~/Library/Application Support/Claude/...`) | Architect không có anchor doc trong repo về Linux Claude Desktop path | `[needs Worker verify]` | ⏳ Worker EXECUTE — nếu Sếp không dùng Claude Desktop Linux, Worker skip + ghi Discovery; OR Worker bỏ Linux Claude Desktop path detail, chỉ note "OS-specific config dir" |
| 10 | Phase 3 sprint cumulative stats: 23 INVs (INV-1..23), ~145 tests (Linux WSL2 chạy ~5 cross-OS-shared, macOS chạy đủ ~145) | P014 CHANGELOG dòng 32 ghi "INV total 21 → 23"; P013 CHANGELOG dòng 60 ghi "143 total"; P011 baseline 144 | `[verified per CHANGELOG]` | ✅ Số chính xác: 23 INVs, 143 tests baseline (P013 + P014 zero new tests) — Linux test count varies do `#[cfg(target_os = "...")]` gating |
| 11 | Phase 3.4 phiếu phác trong BACKLOG.md (dòng 41) — "README quick-start có 2 OS columns (hoặc 2 sequential sections)" | Architect Read BACKLOG dòng 41 | `[verified]` | ✅ DP1 chọn sequential sections (em recommend, không phải columns vì markdown column support kém) |

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.**

Tất cả anchors ✅ hoặc ⏳ — không có ❌.

---

## Debate Log

> Tầng 2 phiếu — Worker CHALLENGE skipped per orchestrator tier routing (xem `docs/ORCHESTRATION.md`). Debate Log chỉ initialize với V1 và Worker EXECUTE ghi observation block (không phải objection).

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge
*(Tầng 2 — Worker EXECUTE direct sau APPROVAL_GATE; nếu Worker phát hiện anchor sai khi đọc README hoặc khi chạy dogfood smoke, Worker ESCALATE qua Discovery Report thay vì CHALLENGE.)*

**Status:** N/A — Tầng 2 phiếu, CHALLENGE skipped per tier routing.

### Final consensus
- Phiếu version: V1
- Total turns: 0 (Tầng 2 — direct EXECUTE)
- Approved (autonomous narrate): pending APPROVAL_GATE

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.
> Worker BẮT BUỘC log: (a) mỗi shell command Linux quick-start dry-run (exit_code + stdout snippet), (b) `cargo install --path .` + binary path verify, (c) trước/sau crontab diff, (d) Phase 3 status banner edit.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

Example template (Worker fill):
```
[2026-05-28T...Z] event=cargo_install evidence=exit_code=0 duration=12s binary=/home/sep/.cargo/bin/advisory-cron
[2026-05-28T...Z] event=p015_smoke_register evidence=exit_code=0 stdout="registered: p015-smoke" crontab_diff=+1_line_tagged
[2026-05-28T...Z] event=p015_smoke_status evidence=exit_code=0 json={"plist_loaded":true,"next_fire":null,...}
[2026-05-28T...Z] event=p015_smoke_unregister evidence=exit_code=0 crontab_diff=-1_line_tagged_total_0
[2026-05-28T...Z] event=invalid_label_reject evidence=exit_code=2 stderr="invalid label" crontab_unchanged
[2026-05-28T...Z] event=readme_quick_start_macos_preserved evidence=git_diff=heading_rename_only
[2026-05-28T...Z] event=readme_quick_start_linux_added evidence=git_diff=+~45_lines_new_section
[2026-05-28T...Z] event=readme_status_banner_bumped evidence=git_diff=Phase_3_COMPLETE_line
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks BEFORE marking phiếu DONE. Tầng 2 docs phiếu — Sub-mechanism focus = A (trigger gap doc), B (capability — `cargo install` works + binary executable), E (environment drift — `cargo install` clean idempotent).

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | `crontab -l \| grep "# advisory-cron: p015-smoke"` post-`register` | exactly 1 line | | |
| A (trigger) | `crontab -l \| grep "# advisory-cron: p015-smoke"` post-`unregister` | 0 lines | | |
| B (capability) | `cargo install --path .` from clean working tree | exit 0, binary at `~/.cargo/bin/advisory-cron` | | |
| B (capability) | `~/.cargo/bin/advisory-cron --version` | prints `advisory-cron 0.1.0` exit 0 | | |
| B (capability) | `~/.cargo/bin/advisory-cron init --force` | exit 0, config written | | |
| B (capability) | `cargo build --release` | exit 0, zero warnings (sanity — README change shouldn't touch code) | | |
| B (capability) | `cargo test --all` | all pass (sanity — same as above) | | |
| C (migration) | N/A — no schema change | — | | N/A |
| D (persistence) | `grep -c "Phase 3 COMPLETE" README.md` | ≥1 (durable banner location) | | |
| E (env drift) | `git diff Cargo.toml Cargo.lock` | empty (Constraint #2) | | |
| E (env drift) | `git diff src/` | empty (Constraint #1) | | |

---

## Nhiệm vụ

### Task 1: Rename existing macOS quick-start heading

**File:** `README.md`

**Tìm:** dòng 23
```markdown
## Quick start (CLI)
```

**Thay bằng:**
```markdown
## Quick start — macOS (launchd)
```

**Lưu ý:** Giữ NGUYÊN toàn bộ content dưới heading này (dòng 24-43 hiện tại — 6 bash steps đã verified bởi P007 dogfood). KHÔNG đụng tới flow, KHÔNG đổi label `advisory-scan-daily`, KHÔNG đụng `launchctl list | grep com.advisorycron` verification step (Sub-mech A check macOS).

### Task 2: Add Linux quick-start section sau macOS quick-start

**File:** `README.md`

**Tìm:** sau dòng 43 (kết thúc macOS quick-start block với `advisory-cron unregister --label advisory-scan-daily`), TRƯỚC heading `## MCP server (Claude Desktop / Claude Code)` (dòng 45 hiện tại).

**Thêm:**
```markdown
## Quick start — Linux (cron-tab)

```bash
# 1. Write default config to ~/.config/advisory-cron/config.toml
advisory-cron init

# 2. Register a cron-tab line that fires daily at 09:00
advisory-cron register --label advisory-scan-daily --schedule "0 9 * * *"

# 3. Verify the cron-tab line is present (Sub-mechanism A — trigger gap check)
crontab -l | grep "# advisory-cron:"
# Expected: 0 9 * * * /home/<YOU>/.cargo/bin/advisory-cron run # advisory-cron: advisory-scan-daily

# 4. Fire the configured task immediately (one-shot test)
advisory-cron run

# 5. Show cron schedule + last 5 heartbeats
advisory-cron status --label advisory-scan-daily
# Note: on Linux Phase 3, `next_fire` renders `N/A` — full cron-expression parsing
# deferred to Phase 3.5+ (INV-23). Schedule is still active; verify via `crontab -l`.

# 6. Unregister when done testing
advisory-cron unregister --label advisory-scan-daily

# 7. Verify the cron-tab line is removed
crontab -l | grep "# advisory-cron:"
# Expected: (no output, exit 1) — other user crontab lines preserved
```

**Note:** `advisory-cron register` parses `--schedule` as a 5-field cron expression but currently only accepts the daily form `M H * * *` (parity with macOS `StartCalendarInterval`). Full 5-field cron support is deferred to Phase 3.5+ per INV-23. See [`docs/security/INVARIANTS.md`](docs/security/INVARIANTS.md) for the formal invariant.
```

**Lưu ý:**
- Bash code block phải khớp chính xác với commands Worker chạy end-to-end trên WSL2.
- Step 3 verification: stdout sample dòng (`0 9 * * * /home/<YOU>/...`) là expected từ P013 Discovery edge case #7 + ARCHITECTURE.md §Cron mechanism Linux dòng 253. Worker EXECUTE confirm exact text rendering, nếu khác → update README line + log Discovery.
- Step 5 note về `next_fire: N/A` per P013 Discovery + ARCHITECTURE dòng 268. ESSENTIAL — anh không document → user mở `advisory-cron status` thấy thiếu next_fire nghĩ là bug.
- Step 7 verification: `grep` exit 1 khi no match là expected (chuẩn POSIX). README dùng "no output, exit 1" để user không nghĩ "không có output = bị treo".
- `<YOU>` placeholder pattern khớp với existing macOS quick-start "What advisory-cron fires" section (dòng 95, 103) — consistent.

### Task 3: MCP section OS-agnostic note

**File:** `README.md`

**Tìm:** đoạn sau code block JSON (kết thúc dòng 64 hiện tại `}`), TRƯỚC dòng 66 hiện tại (`Replace <YOUR_USERNAME> with your macOS username. Confirm binary path with which advisory-cron.`).

Cụ thể em đề xuất Worker xử như sau — REPLACE dòng 66 hiện tại:
```markdown
Replace `<YOUR_USERNAME>` with your macOS username. Confirm binary path with `which advisory-cron`.
```

**Thay bằng:**
```markdown
Replace `<YOUR_USERNAME>` with your username and adjust the path for your OS:
- **macOS:** `/Users/<YOUR_USERNAME>/.cargo/bin/advisory-cron` + config at `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Linux:** `/home/<YOUR_USERNAME>/.cargo/bin/advisory-cron` + config at `~/.config/Claude/claude_desktop_config.json` `[needs Worker verify]`

Confirm binary path with `which advisory-cron`.
```

**Lưu ý:**
- Linux Claude Desktop config path `~/.config/Claude/claude_desktop_config.json` là `[needs Worker verify]` — Architect không có anchor trong repo. Worker EXECUTE: nếu confirm được (Sếp có Claude Desktop Linux installed) → bỏ marker, ghi Discovery. Nếu Sếp KHÔNG dùng Claude Desktop Linux + Worker không verify được → Worker thay text Linux bullet thành: "**Linux:** Claude Desktop / Claude Code MCP client config path is OS-specific — consult your client documentation. Binary path: `/home/<YOUR_USERNAME>/.cargo/bin/advisory-cron`" và ghi Discovery + log.
- KHÔNG đụng JSON code block hiện tại (dòng 55-64) — giữ macOS-style example đúng.

### Task 4: Bump Status banner Phase 3 ✅

**File:** `README.md`

**Tìm:** dòng 161 hiện tại
```markdown
Phase 1 + Phase 2 COMPLETE (all 10 phiếu shipped). Track progress in [`docs/BACKLOG.md`](docs/BACKLOG.md).
```

**Thay bằng:**
```markdown
Phase 1 + Phase 2 + Phase 3 COMPLETE — macOS launchd + Linux cron-tab dual-platform shipped. Single Rust binary (~5 MB), 23 invariants, cross-OS CI matrix (macos-latest + ubuntu-latest). Track progress in [`docs/BACKLOG.md`](docs/BACKLOG.md).
```

**Lưu ý:**
- "Single Rust binary (~5 MB)" — P013 CHANGELOG dòng 89 ghi 4.7MB Linux build; P012 dòng 89 ghi 4.7MB; estimate ~5MB safe round-up. Worker EXECUTE verify lại bằng `ls -lh target/release/advisory-cron` after `cargo build --release` → nếu lệch xa (e.g. >7MB tạo lo về INV-3 budget) → log Discovery + update text.
- "23 invariants" — verified per P014 CHANGELOG dòng 32 ("INV total 21 → 23"). Confirm bằng `grep -c "^### INV-" docs/security/INVARIANTS.md`.
- "cross-OS CI matrix (macos-latest + ubuntu-latest)" — verified per P014 + ARCHITECTURE.md §CI matrix dòng 322.
- KHÔNG thêm test count (varies by OS gated tests — gây confuse).
- KHÔNG thêm Phase 4/5 roadmap (out of scope; BACKLOG ownership).

### Task 5: Verify §What advisory-cron fires vẫn accurate cho Linux

**File:** `README.md` (read-only verify)

**Tìm:** §What advisory-cron fires hiện tại dòng 87-106. TOML example dùng:
```toml
working_dir = "/Users/<YOU>/some-repo"

[heartbeat]
log_path = "/Users/<YOU>/.local/state/advisory-cron/heartbeat.jsonl"
```

**Quyết định:** GIỮ NGUYÊN, không edit. Lý do:
- Section là **example** không phải spec — placeholder `<YOU>` tells user replace.
- Linux user reading section sẽ thay `/Users/` → `/home/` tự nhiên (cùng với `<YOU>` → username thật).
- Edit-mở-rộng-2-OS section sẽ kéo dài README + duplicate placeholder spam.

**Lưu ý:** Nếu Worker EXECUTE thấy Sếp confused về `/Users/` vs `/home/` khi đọc README → log Discovery, đề xuất P016+ note. KHÔNG fix in this phiếu.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `README.md` | Task 1 (rename macOS heading), Task 2 (add Linux quick-start ~50 LOC), Task 3 (MCP OS-agnostic note ~5 LOC), Task 4 (status banner bump 1 line) |

**Estimated LOC delta:** ~+50 to +60 lines net (Linux section + small additions).

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/**/*.rs` | `git diff src/` empty (Constraint #1) |
| `Cargo.toml`, `Cargo.lock` | `git diff Cargo.toml Cargo.lock` empty (Constraint #2) |
| `docs/ARCHITECTURE.md` | `git diff docs/ARCHITECTURE.md` empty (P014 đã ship Phase 3.3 status + §CI matrix; P015 Tầng 2 không touch ARCHITECTURE) |
| `docs/security/INVARIANTS.md` | `git diff docs/security/INVARIANTS.md` empty (P014 đã ship INV-22/23) |
| `docs/PROJECT.md` | `git diff docs/PROJECT.md` empty (no scope/non-goals change) |
| `docs/BACKLOG.md` | `git diff docs/BACKLOG.md` empty (Quản đốc owns post-merge BACKLOG sprint close move; Worker không touch) |

---

## Luật chơi (Constraints)

1. **Chỉ sửa `README.md`** — `git diff --name-only` chỉ list `README.md` + the 3 docs-gate-required files (`docs/CHANGELOG.md`, `docs/discoveries/P015.md`, `docs/DISCOVERIES.md`). Nothing else.
2. **Zero `src/` + Cargo touch** — `git diff src/ Cargo.toml Cargo.lock` empty. Tầng 2 không bao giờ touch code.
3. **Linux quick-start commands phải verified end-to-end trên WSL2 thật** — Worker EXECUTE chạy mỗi command, capture exit_code + stdout snippet vào Discovery Report. KHÔNG copy-paste từ phiếu mà không chạy.
4. **`<YOU>` / `<YOUR_USERNAME>` placeholder consistency** — Linux bullets dùng `<YOUR_USERNAME>` (match existing MCP section pattern) hoặc `<YOU>` (match §What advisory-cron fires pattern). Worker chọn 1 cho Linux quick-start nhất quán; em đề xuất `<YOUR_USERNAME>` cho MCP section (Task 3) + `<YOU>` cho Linux quick-start (Task 2) — match adjacent existing patterns.
5. **macOS quick-start preserved verbatim** — `git diff README.md` cho thấy macOS section chỉ thay đổi 1 dòng (heading rename). All 6 bash steps preserved exactly.
6. **No screenshots, GIFs, asciinema, badge changes** — pure text edit.
7. **No Windows quick-start** — Phase 5+ future waves. KHÔNG mention "Windows support" anywhere.
8. **No Phase 4/5 roadmap text** — BACKLOG.md ownership. Status banner chỉ ghi Phase 3 ✅, không tease tương lai.
9. **No INV-22/23 detail explanation** — link ra `docs/security/INVARIANTS.md` thôi, không duplicate doctrine vào README.
10. **`[needs Worker verify]` markers** trong Task 3 Linux Claude Desktop path PHẢI được Worker resolve (verify-then-remove-marker, hoặc rewrite as OS-agnostic prose) — KHÔNG ship `[needs Worker verify]` text trong final README.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings (sanity — README change shouldn't affect)
- [ ] `cargo test --all` — all pass (sanity)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (sanity)
- [ ] `cargo fmt --check` — no diff (sanity)
- [ ] `git diff src/ Cargo.toml Cargo.lock docs/ARCHITECTURE.md docs/security/INVARIANTS.md docs/PROJECT.md docs/BACKLOG.md` — empty

### Manual Testing
- [ ] **Linux quick-start dry-run end-to-end on WSL2** — Worker run từng command theo Task 2 sequence, expect exit_code + crontab state khớp expected:
  - `advisory-cron init` → exit 0, config tại `~/.config/advisory-cron/config.toml`
  - `advisory-cron register --label p015-smoke --schedule "0 9 * * *"` → exit 0
  - `crontab -l | grep "# advisory-cron:"` → exactly 1 line matching `0 9 * * * .* run.* # advisory-cron: p015-smoke`
  - `advisory-cron status --label p015-smoke --json` → exit 0, JSON contains `"plist_loaded": true` + `"next_fire": null`
  - `advisory-cron unregister --label p015-smoke` → exit 0
  - `crontab -l | grep "# advisory-cron:"` → 0 lines, exit 1
- [ ] **MCP section binary path verify** — Worker run `which advisory-cron` → confirm `/home/<sep>/.cargo/bin/advisory-cron` matches Task 3 Linux bullet.
- [ ] **Status banner accuracy** — `grep -c "^### INV-" docs/security/INVARIANTS.md` → 23 (matches "23 invariants" claim). `ls -lh target/release/advisory-cron` → size reasonable for "~5 MB" claim (4-6 MB range OK).

### Regression
- [ ] **macOS quick-start preserved** — `git diff README.md` shows existing macOS quick-start 6 bash steps unchanged (only heading rename on Task 1).
- [ ] **MCP smoke test snippet preserved** — `echo '{"jsonrpc":"2.0"...}' | advisory-cron mcp` block at dòng 80-83 hiện tại untouched.
- [ ] **§What advisory-cron fires** untouched per Task 5 decision.
- [ ] **Phase 2.1/2.2/2.3 paragraphs** (dòng 108-157 hiện tại) untouched.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P015 entry prepended (newest at top) ghi Tầng 2 README polish, 4 task summary, dogfood smoke exit codes.
- [ ] `docs/ARCHITECTURE.md` — KHÔNG cần update (Tầng 2; P014 đã ship Phase 3.3 status + §CI matrix; P015 không touch architecture).
- [ ] `README.md` — Đây IS the file being edited; verify rendering Markdown đúng (no broken links, no malformed code blocks).
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P015.md` — full report written, BẮT BUỘC ghi:
  - Anchor #8 (binary path `~/.cargo/bin/advisory-cron`) verify result
  - Anchor #9 (Linux Claude Desktop config path) verify result + cách Worker resolved `[needs Worker verify]` marker
  - Linux dogfood smoke output (all 7 steps Task 2 sequence) — exit codes + stdout snippets
  - Phase 3 sprint closure notes — Worker confirm acceptance criteria 9/9 từ BACKLOG.md Phase 3 (sau P015 ship) đã satisfy
- [ ] `docs/DISCOVERIES.md` — 1-line index entry prepended (newest at top): `- 2026-05-28 P015: README Linux quick-start (Phase 3.4 — sprint close) → see docs/discoveries/P015.md`
- [ ] Sub-mechanism A-E Verification Trace filled (table above)

---

## Estimated effort

**~30-45 phút** Worker EXECUTE:
- Task 1 (heading rename): 1 phút
- Task 2 (Linux section + smoke dry-run on WSL2): 15-20 phút (mostly running commands, capturing output)
- Task 3 (MCP OS-agnostic note + Linux config path resolve): 5-10 phút (depends on whether Sếp's Linux Claude Desktop exists)
- Task 4 (status banner): 1 phút
- Task 5 (verify-only): 1 phút
- Docs gate + Discovery Report + CHANGELOG entry: 10 phút

**Risk: LOW.** Tầng 2 docs-only. Worst case: Anchor #9 Linux Claude Desktop path không verify được → Worker dùng fallback OS-agnostic prose, log Discovery. Không block Phase 3 sprint close.

**Rollback:** `git checkout main -- README.md` revert hoàn toàn. No DB, no schema, no migration. Reversible 100%.
