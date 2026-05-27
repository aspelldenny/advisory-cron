#!/usr/bin/env bash
# architect-guard.sh — PreToolUse hook để chặn cứng Architect drift khỏi envelope
#
# How it works:
#   - Hook fires on Read / Glob / Write / Edit tool calls (matcher in settings.json)
#   - Reads JSON from stdin (Claude Code hook payload)
#   - Detects Architect via marker file .claude/.architect-active
#   - For Read/Glob: blocks src/, lib/, app/, prisma/, tests/, build dirs, code/sql/prisma extensions
#                    (Glob 'path' search root checked alongside 'pattern')
#   - For Write/Edit: strict allowlist — only docs/ticket/P*-*.md (phiếu files)
#                     per architect.md line 32 ("Em CHỈ được Write vào: docs/ticket/P*-*.md")
#
# Setup: wired in committed `.claude/settings.json` under hooks.PreToolUse so every
# collaborator/CI run gets structural enforcement. Architect agent must create
# marker file `.claude/.architect-active` on spawn.
#
# Note: NO external deps (no jq) — uses pure shell + sed/grep for cross-platform
# compatibility (esp. Windows msys2 bash where jq is not bundled).

set -euo pipefail

MARKER_FILE=".claude/.architect-active"

# If no marker → not running as Architect → allow everything
[ -f "$MARKER_FILE" ] || exit 0

# Read tool input JSON from stdin
INPUT_JSON=$(cat)

# Extract tool name (PreToolUse payload has top-level "tool_name")
TOOL_NAME=$(echo "$INPUT_JSON" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

# Extract candidate paths from tool_input
FILE_PATH=$(echo "$INPUT_JSON" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
GLOB_PATTERN=$(echo "$INPUT_JSON" | sed -n 's/.*"pattern"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
GLOB_PATH=$(echo "$INPUT_JSON" | sed -n 's/.*"path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

# Returns 0 (truthy) if path is in forbidden Read/Glob zone, 1 otherwise
is_forbidden_for_read() {
    local p="${1#./}"
    case "$p" in
        src/*|*/src/*|lib/*|*/lib/*|app/*|*/app/*|crates/*/src/*|pkg/*|*/pkg/*) return 0 ;;
        prisma/*|*/prisma/*) return 0 ;;
        tests/*|*/tests/*|test/*|*/test/*|__tests__/*) return 0 ;;
        node_modules/*|target/*|dist/*|build/*|.next/*|.nuxt/*|.svelte-kit/*) return 0 ;;
        *.rs|*.ts|*.tsx|*.js|*.jsx|*.py|*.go|*.java|*.cpp|*.c|*.h|*.hpp) return 0 ;;
        *.prisma|*.sql) return 0 ;;
    esac
    return 1
}

# Returns 0 (truthy) if path is allowed for Architect Write/Edit, 1 otherwise
# Architect handbook line 32: "Em CHỈ được Write vào: docs/ticket/P*-*.md"
is_allowed_for_write() {
    local p="${1#./}"
    case "$p" in
        docs/ticket/TICKET_TEMPLATE.md) return 1 ;;  # explicit deny — template is reference
        docs/ticket/P*-*.md|*/docs/ticket/P*-*.md) return 0 ;;
    esac
    return 1
}

block_read() {
    local violator="$1"
    cat >&2 <<EOF
🚫 Architect envelope violation (Read/Glob)

Architect cannot read source code: $violator

What to do instead: write a Task 0 anchor in the phiếu.
Example:
  | # | Assumption | Verify by | Result |
  | 1 | <claim about $violator> | grep ... $violator | ⏳ TO VERIFY |

Worker (separate subagent) will grep-verify it for you. The constraint IS the feature.
EOF
    exit 2
}

block_write() {
    local violator="$1"
    cat >&2 <<EOF
🚫 Architect envelope violation (Write/Edit)

Architect cannot Write/Edit: $violator

Architect's Write allowlist (per architect.md line 32):
  - docs/ticket/P*-*.md  (phiếu files only)

Everything else (src/, CLAUDE.md, BACKLOG.md, CHANGELOG.md, guides) belongs to Worker.
If a phiếu needs to update those files, encode it as a Worker Task in the phiếu.
EOF
    exit 2
}

case "$TOOL_NAME" in
    Read|Glob)
        # Check file_path (Read), pattern (Glob), and path (Glob search root)
        for candidate in "$FILE_PATH" "$GLOB_PATTERN" "$GLOB_PATH"; do
            [ -z "$candidate" ] && continue
            np="${candidate#./}"
            # .md early-allow — docs are Architect's read domain even if alongside code
            case "$np" in *.md) continue ;; esac
            if is_forbidden_for_read "$np"; then
                block_read "$candidate"
            fi
        done
        ;;
    Write|Edit)
        # No path → can't validate, allow (defensive — shouldn't happen for Write/Edit)
        [ -z "$FILE_PATH" ] && exit 0
        if ! is_allowed_for_write "$FILE_PATH"; then
            block_write "$FILE_PATH"
        fi
        ;;
esac

# Default: allow
exit 0
