---
title: The config file
description: "Where the config lives, which commands write it, and what each key means."
order: 1
---

The config is `~/.claude/clauset/config.toml`, or the file `CLAUSET_CONFIG` names. Paths may start with `~/`.

```toml
default = "personal"
warn_at = 80
chrome = "personal"

[accounts.personal]
dir = "~/.claude"
email = "me@example.com"

[accounts.work]
dir = "~/.claude-accounts/work"
warn_at = 60
```

## Who writes it

`clauset setup`, `add`, `remove`, `use`, `warn` and `chrome` write the file. They keep its comments, and refuse a change that would leave it invalid. `clauset switch` doesn't touch it.

## Keys

| Key | Meaning |
|---|---|
| `default` | The account a session started from no Clauset session runs on. `clauset use <account>` sets it. |
| `warn_at` | Optional. The percent of a 5-hour or 7-day limit at which every account warns. `clauset warn 80` sets it. |
| `chrome` | Optional. The account the Claude in Chrome extension is signed in to. Every other account reaches the browser through it. `clauset chrome <name>` sets it, `clauset chrome off` removes it. |
| `[accounts.<name>] dir` | The account's config directory. `~/.claude` is the hub, which runs with `CLAUDE_CONFIG_DIR` unset, as Claude Code expects of its default directory. |
| `[accounts.<name>] email` | Optional. Prefilled at `clauset login`, checked by `doctor`. |
| `[accounts.<name>] warn_at` | Optional. This account's own threshold, over the global one. `clauset warn 60 --account <name>` sets it. |
| `[accounts.<name>] org` | Optional. The organization UUID the sign-in must belong to, checked by `login` and `doctor`. One login can belong to several organizations, picked in the browser. |

## What sits beside it

What Clauset records (usage, MCP server files, move requests), the shim's `bin/` and the menu bar app's `app.pid` and `notify/` sit in the same directory, `~/.claude/clauset/`. `clauset link` leaves that directory out of the accounts. [What Clauset writes](files.md) lists the rest.
