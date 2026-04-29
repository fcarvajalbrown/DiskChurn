# Hooks — How They Work

Hooks are shell commands Claude Code runs automatically at specific lifecycle events.
They live in `settings.json` under the `hooks` key, at user (`~/.claude/settings.json`)
or project (`.claude/settings.json`) scope. Project scope is loaded on top of user scope.

## Structure

```json
{
  "hooks": {
    "EVENT": [
      {
        "matcher": "ToolName",
        "hooks": [
          {
            "type": "command",
            "command": "your-shell-command",
            "shell": "bash"
          }
        ]
      }
    ]
  }
}
```

- `EVENT` — when the hook fires (see list below)
- `matcher` — optional, filters by tool name (e.g. `"Write|Edit"`)
- `type` — `"command"` (shell), `"prompt"` (LLM check), or `"agent"` (full agent)
- `shell` — `"bash"` or `"powershell"`

## Events

| Event | When it fires |
|---|---|
| `Stop` | Every time Claude finishes a response |
| `PreToolUse` | Before a tool runs — can block it |
| `PostToolUse` | After a tool succeeds |
| `SessionStart` | When the session begins |
| `PreCompact` | Before context is compacted |
| `UserPromptSubmit` | When you hit enter on a prompt |

## Hook Input

For tool events, Claude Code pipes JSON to the hook via stdin:

```json
{
  "tool_name": "Write",
  "tool_input": { "file_path": "/some/file.rs", "content": "..." }
}
```

Read it with `jq`: `jq -r '.tool_input.file_path'`

## Hook Output

If the command prints JSON with a `systemMessage` key, Claude shows it in the UI:

```bash
echo '{"systemMessage": "your message here"}'
```

Set `"continue": false` to block the action (only works on `PreToolUse`).

## The Hook in This Project

`.claude/settings.json` has a `Stop` hook that reminds you to update `CLAUDE.md`
at the end of every session. It fires after every Claude response — no matcher
needed since `Stop` is not a tool event.

Settings load order: `~/.claude/settings.json` -> `.claude/settings.json` -> `.claude/settings.local.json`
Later files win on conflicts. Hooks from all files are merged and all fire.
