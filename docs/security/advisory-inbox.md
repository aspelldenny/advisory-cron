# Advisory Inbox

> **Queue file for security advisories surfaced by Trinh sát (advisory-watch).**
> Generated rows are appended between the sentinel markers below by the `/advisory-scan` slash command (orchestrator-side).
> Chủ nhà reviews each row and either marks status `dismissed` or creates a follow-on phiếu to patch.
>
> **Schema (8 columns):** `date | advisory ID | URL | name@version-range | file:line or "indirect" | severity | status | notes`
>
> **Severity:** must reflect upstream official source (GHSA reviewed badge / vendor official page). NO third-party rescore inflation. See `agents/advisory-watch.md` Bước 5 "Severity sourcing rule" for full rationale.
>
> **Status values:** `open` (new, awaiting Chủ nhà review) | `dismissed` (reviewed, not actionable — e.g. unaffected code path, false positive) | `phieu-P<NNN>` (follow-on phiếu created — link the row to its tracking phiếu).

## Rows

<!-- advisory-start -->
<!-- advisory-end -->

## Example row (commented — for schema reference, NOT counted in scans)

<!--
| 2026-05-25 | GHSA-xxxx-yyyy | https://github.com/advisories/GHSA-xxxx-yyyy | next@<=15.5.17 | src/middleware.ts:42 | High (GHSA reviewed) | open | - |
-->

## How rows flow

1. User runs `/advisory-scan` (or cron triggers it).
2. Slash command verifies `.sos-stack.toml` exists; if missing → prompts user to run `sos init security` first.
3. Slash command spawns Trinh sát subagent.
4. Trinh sát runs each `[[stack]]` parser via scoped Bash (`python3 <parser> <lock_file>`), collects deps.
5. Trinh sát queries GHSA + vendor pages, greps codebase, returns sentinel-wrapped rows in final report.
6. Slash command extracts block between `<!-- advisory-start -->` and `<!-- advisory-end -->`, appends rows in this file BETWEEN those markers (preserve marker positions).
7. Chủ nhà reviews, gates per-row.

## Notes

- Empty `<!-- advisory-start --> / <!-- advisory-end -->` block = no advisories matched (clean scan). Re-run later as deps evolve.
- Multiple scans append cumulatively. Chủ nhà handles dedup by status updates (no auto-dedup in P041; state-file dedup deferred to follow-on phiếu).
- **Do NOT delete the sentinel markers.** They are the slash command's append point.
- **Do NOT rename this file's location randomly.** Default expected at `docs/security/advisory-inbox.md` in user project — copy this template there on first run.
