---
description: Capture `gh pr diff` + spawn `@agent-boundary-check` (giám sát) + post advisory comment với sentinel block. ADVISORY mode — KHÔNG block merge. Argument BẮT BUỘC — PR number (vd "12").
---

# /security-review <PR> — Boundary check shortcut (advisory-cron)

Vai `@agent-boundary-check` (Giám sát) soi 5 generic INV trên PR diff. Slash command này wrap workflow: capture diff → spawn agent → post comment if NEEDS_REVIEW.

## Nhiệm vụ

1. **Validate `$ARGUMENTS`** — must be a PR number. Fail loud if empty.
2. **Capture diff:**
   ```bash
   gh pr diff $ARGUMENTS --name-only > /tmp/pr-${ARGUMENTS}-files.txt
   gh pr diff $ARGUMENTS > /tmp/pr-${ARGUMENTS}-diff.patch
   gh pr view $ARGUMENTS --json body -q .body > /tmp/pr-${ARGUMENTS}-body.txt
   ```
3. **Security surface check** (paths: `src/`, `Cargo.toml`, `.docs-gate.toml`, `.sos-stack.toml`, `scripts/`, `.claude/hooks/`, `.github/workflows/`, `.env*`):
   - Touch ≥1 path → proceed (mandatory).
   - Touch 0 path → narrate "no security surface touched, skipping" + exit.
4. **Spawn `@agent-boundary-check`** với:
   - PR number
   - Diff content (or `/tmp/` path if > 100KB)
   - File list
   - PR body (for INV-5 changelog check)
5. **Parse verdict** from sentinel block `<!-- security-review-start -->` ... `<!-- security-review-end -->`.
6. **Silent-when-clean rule:** APPROVE + 0 FLAG → exit silently. NEEDS_REVIEW (≥1 FLAG) → post comment.
7. **Post comment** (NEEDS_REVIEW only):
   ```bash
   gh pr comment $ARGUMENTS --body "$(cat <<EOF
   [agent-boundary-check verdict]
   <sentinel block content>
   EOF
   )"
   ```

## Output

- PR number
- Files touched count
- Security surface match: yes/no
- Verdict: APPROVE / NEEDS_REVIEW
- Comment posted: yes/no (silent-when-clean rule)
- FLAG count + brief evidence summary
