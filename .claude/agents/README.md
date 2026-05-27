# agents/ — canonical subagent definitions

> Source of truth for sos-kit's Claude Code subagents.

## Files

- `architect.md` — Kiến trúc sư subagent (Read/Write/Glob only — no code access)
- `worker.md` — Thợ subagent (full code tools — no vision docs access)

These are the **canonical** versions, English-neutral, "Chủ nhà" voice. When installing sos-kit into a project (per [`../INSTALL.md`](../INSTALL.md)), copy from here:

```bash
cp ~/sos-kit/agents/architect.md <your-project>/.claude/agents/
cp ~/sos-kit/agents/worker.md    <your-project>/.claude/agents/
```

## Why two copies exist in this repo

Inside the sos-kit repo itself, `.claude/agents/` is a regenerated copy with `Chủ nhà` swapped to `Sếp` — the maintainer (Denny) prefers Vietnamese pronoun in personal flow. The two are kept in sync by `scripts/sync-personal-agents.sh`:

```bash
bash scripts/sync-personal-agents.sh   # regenerates .claude/agents/ from agents/
```

**Workflow rule:** edit `agents/*.md` (canonical), then run sync. Never edit `.claude/agents/*.md` directly — your changes will be overwritten on next sync.

External users / Tarot / other projects copy from `agents/` and are unaffected by `.claude/agents/`.

## Invocation modes

Both subagents support 2 modes triggered by phrases in the orchestrator's spawn prompt — see body of each file for the table. Summary:

- **Architect:** `DRAFT` (write new phiếu) | `RESPOND` (respond to Worker challenge)
- **Worker:** `CHALLENGE` (verify phiếu against code, write Debate Log) | `EXECUTE` (code, test, commit)

The Architect ↔ Worker debate loop is documented in [`../docs/ORCHESTRATION.md`](../docs/ORCHESTRATION.md) and [`../docs/HANDOFF.md`](../docs/HANDOFF.md) (Handoff 2.5).
