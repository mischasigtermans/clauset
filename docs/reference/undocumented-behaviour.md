---
title: Undocumented behaviour Clauset relies on
description: "What Clauset takes from Claude Code without a documented promise, and what breaks if it changes."
order: 5
---

Clauset is tested on Claude Code 2.1.322. An update to Claude Code can change any of the behaviour below.

## What Clauset reads

- The Keychain item of a config directory is `Claude Code-credentials-` and the first 8 hex digits of the SHA-256 of its resolved path. `doctor` checks each one exists.
- Claude Code gives every process its tools start `CLAUDE_PID` and `CLAUDE_CODE_SESSION_ID`, keeps each interactive session's `status` (`busy`, `idle`) in `sessions/<pid>.json`, starts each Bash tool shell from `<config dir>/shell-snapshots/snapshot-*`, and ends a session cleanly on SIGTERM. `clauset switch` rests on all four.
- Each entry in `sessions/<pid>.json` holds the `sessionId`, `name`, `cwd` and `entrypoint` (`cli` for a terminal session) of a running session, and `procStart`, its process's start time as `ps -o lstart=` prints it in UTC. Clauset accepts it in the C locale or the inherited one. `clauset switch --all` finds the sessions to move there and leaves out an entry whose pid now belongs to a process started at another time.
- `.claude.json` keeps `cachedUsageUtilization`, and `oauthAccount.planDisplayName` for personal plans (`Ultra`). A Team organization has no display name.

## Claude in Chrome

Claude in Chrome reaches the extension only through a relay keyed on the session's claude.ai account, and its server is `claude --claude-in-chrome-mcp`, a switch `claude --help` doesn't list. `--mcp-config` refuses a local server named `claude-in-chrome` and accepts one named `chrome`. The `chrome` key rests on all three.

If an update drops the switch, `/mcp` lists the `chrome` server as failed. If it reserves `chrome` too, the server is left out with only a line in the debug log. Either way, sessions on accounts other than the one `chrome` names have no browser until they start with `--chrome` or the key is removed.
