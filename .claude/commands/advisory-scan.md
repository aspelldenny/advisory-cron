---
description: Trigger @agent-advisory-watch quét stack + query advisory database. Caller (slash command Bash) parse rows từ report + append vào docs/security/advisory-inbox.md. Argument optional — nếu pass tên dep cụ thể (vd "tokio" / "clap") thì scan focused; nếu rỗng scan full stack.
---

# /advisory-scan — Advisory watch shortcut (advisory-cron)

CLAUDE.md có vai bounded `@agent-advisory-watch` — **read-only thật** (KHÔNG cầm Write tool) soi CVE/GHSA thế giới ngoài đối chiếu stack mình. Slash command này shortcut invoke agent + append inbox.

## Nhiệm vụ

Spawn `@agent-advisory-watch` với input `$ARGUMENTS`:

1. **Nếu `$ARGUMENTS` rỗng** → agent scan **full stack** (đọc `.sos-stack.toml` → run parser per stack).
2. **Nếu `$ARGUMENTS` có giá trị** (vd `tokio`, `clap`, `reqwest`) → agent scan **focused** dep đó.

## Caller flow

Slash command body chạy flow:

1. **Invoke** `@agent-advisory-watch` với `$ARGUMENTS` → capture report markdown.
2. **Parse rows** trong block `<!-- advisory-start -->` ... `<!-- advisory-end -->` (sentinel comments).
3. **Dedup check** với state file `docs/security/.advisory-scan-state` (`seen_advisories[]` JSON array).
4. **Append rows** mới vào `docs/security/advisory-inbox.md` (insert sau heading `## Rows`, mới nhất trên cùng).
5. **Re-display** top 3 rows by severity + show count open rows.
6. **Update state file** — JSON: `{last_scan_at, seen_advisories, agent_version}`.

## Output

- Number deps scanned
- Number advisories found / chạm stack / new vs dedup
- Top 3 rows by severity (preview)
- Link to `docs/security/advisory-inbox.md`
- Open rows total

## Edge case

- Inbox > 20 open rows → flag warning + chỉ append max 5 row mới.
- Network fail → agent return graceful error, slash KHÔNG append.
- Sentinel missing → fail loud (KHÔNG silent assume "0 advisory").
