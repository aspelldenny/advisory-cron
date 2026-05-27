# PHIẾU P<NNN>: <Tên phiếu>

> **ID format:** `P` + 3 chữ số (P001, P042, P123). Số tiếp theo đọc từ `.phieu-counter`.
> **Filename:** `docs/ticket/P<NNN>-<slug>.md` (khớp tên branch, bỏ prefix `<type>/`).
> **Branch:** `<type>/P<NNN>-<slug>` với `<type>` ∈ {feat, fix, chore, docs, infra}.

---

> **Loại:** Feature / Bugfix / Hotfix / chore / docs / infra
> **Tầng:** 1 / 2
> **Ưu tiên:** P0 / P1 / P2
> **Ảnh hưởng:** [modules / files chính bị ảnh hưởng]
> **Dependency:** [phiếu nào phải xong trước, hoặc "Không"]

---

## Context

### Vấn đề hiện tại
[Mô tả vấn đề hoặc feature cần làm. Reference BACKLOG.md item.]

### Giải pháp
[Mô tả approach. Module-level — không deep implementation detail.]

### Scope
- CHỈ sửa [liệt kê files]
- KHÔNG sửa [liệt kê files không được động]

### Skills consulted (optional)
*(Orchestrator runs skills, captures output, embeds here. Worker reads FROM this section — not invokes Skill.)*

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> **BẮT BUỘC:** Kiến trúc sư PHẢI grep/verify code thật trước khi viết assumption.
> Thợ đọc bảng này để biết assumption nào đã verify, assumption nào chưa.
> Mỗi anchor PHẢI carry humility marker `[verified]` / `[unverified]` / `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | [Function X tồn tại ở file Y] | `grep "fn X" src/...` | `[verified]` | ✅ Dòng 123 |
| 2 | [Type Z = struct với field A,B] | `grep "struct Z" src/...` | `[needs Worker verify]` | ⏳ TO VERIFY |
| 3 | [No dep `foo` in Cargo.toml] | `grep "^foo = " Cargo.toml` | `[verified]` | ✅ Không có |

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.**

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge
*(Worker fill phần này khi invoked CHALLENGE mode. Nếu không có objection: ghi "Worker accepted V1 — no challenges." rồi nhảy Final consensus.)*

**Anchor verification (recap từ Verification Anchors):**
- Anchor #N: ✅/⚠️/❌ + 1 dòng tóm tắt nếu ⚠️/❌

**Objections (Tầng 1 only — phiếu cần sửa):**
- [O1.1] Phiếu giả định X tại file Y, code thật là Z (cite `file:line`). Tác động: …
- [O1.2] …

**Proposed alternatives** (Worker recommend 1):
- A. … (Worker lean — vì …)
- B. …

**Status:** ⏳ AWAITING ARCHITECT RESPONSE

### Turn 1 — Architect Response
*(Architect fill phần này khi invoked RESPOND mode. KHÔNG đọc source code — dựa vào Worker `file:line` citation.)*

- [O1.1] → ACCEPT / DEFEND / REFRAME (Tầng 2) / DEFER TO SẾP → action taken
- [O1.2] → …

**Status:** ✅ RESPONDED — phiếu bumped to V2

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus
- Phiếu version: V<N>
- Total turns: <count>
- Approved (autonomous narrate or Sếp gate): [date] — code execution may begin

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
[2026-05-27T11:30:00Z] event=task0_anchor_1_grep evidence=src/main.rs:42 "fn parse_args"
[2026-05-27T11:32:15Z] event=cargo_check evidence=exit_code=0 duration_ms=4200
[2026-05-27T11:35:00Z] event=clippy_warning evidence=src/launchd.rs:88 "function never used"
[2026-05-27T11:35:30Z] event=clippy_fix evidence=removed unused fn at src/launchd.rs:88
```

---

## Verification Trace (advisory-cron specific — Sub-mechanism A-E checks)

> Worker MUST run applicable Layer 2 capability checks (RULES.md matrix) BEFORE marking phiếu DONE.
> Fill the table; mark N/A if not applicable to this phiếu.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | `launchctl list \| grep <label>` | row present | | |
| B (capability) | `cargo check` | exit 0 | | |
| B (capability) | `cargo test <module>` | targeted tests pass | | |
| C (migration) | (only if schema change) | counts match | | |
| D (persistence) | `grep -l "<rule>" CLAUDE.md docs/RULES.md` | ≥1 hit | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | | |
| E (env drift) | `cargo build --release` clean target | exit 0 | | |

---

## Nhiệm vụ

### Task 1: [Tên task]

**File:** `src/path/to/file.rs`

**Tìm:** [mô tả chính xác đoạn code cần sửa — dùng nội dung text, KHÔNG dùng tên type/function nếu chưa verify]

**Thay bằng / Thêm:**
```rust
[nội dung mới]
```

**Lưu ý:** [edge cases, điều kiện, tương tác cross-module]

### Task 2: [...]

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/path/file.rs` | Task 1: mô tả ngắn |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/path/other.rs` | [fn X tự động hoạt động với thay đổi mới] |

---

## Luật chơi (Constraints)

1. [Constraint 1 — e.g. "Stay within current `Cargo.toml` deps; do NOT add new crate"]
2. [Constraint 2 — e.g. "Match existing tracing span pattern in src/runner.rs"]
3. [Constraint 3 — e.g. "No `unsafe { }` block — escalate if tempted"]

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] [Test case 1 — concrete command + expected output]
- [ ] [Test case 2]

### Regression
- [ ] [Existing subcommand X still works — concrete invocation]

### Docs Gate
- [ ] `docs/CHANGELOG.md` — entry cho phiếu này
- [ ] `docs/ARCHITECTURE.md` — [section nào cần update, nếu Tầng 1]
- [ ] `README.md` — quick-start updated (nếu touch CLI)
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P<NNN>.md` — full report written
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top)
- [ ] Sub-mechanism A-E Verification Trace filled (table above)
