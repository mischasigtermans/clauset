---
title: What Clauset writes
description: "Every file `setup`, `add` and a running session put on your Mac, and which command writes it."
order: 2
---

`setup` asks before each step and leaves a step out when its files are already in place. It leaves a status line of your own alone.

## Files

| Path | Written by | What |
|---|---|---|
| `~/.claude/clauset/config.toml` | `setup`, `add` | The accounts, the default and `warn_at`. |
| `~/.claude/clauset/bin/claude` | `setup` | A link to the `clauset` binary: the shim. |
| `~/.claude/clauset/bin/clauset` | `setup` | A link to the same binary, so the CLI is on PATH wherever the shim is. |
| `~/.zshrc` or `~/.bashrc` | `setup` | Two lines at the end: a comment and `export PATH="<home>/.claude/clauset/bin:$PATH"`, so the shim comes before every other `claude`. Another shell gets the line printed to add by hand. |
| `~/.claude/settings.json` | `setup` | The `SessionStart` and `StopFailure` hooks, and the `statusLine` when there's none. The file is written back whole, as JSON indented by two spaces, its keys in their order. |
| `~/.claude/skills/clauset/SKILL.md` | `setup` | The `/clauset` skill, as shipped in `skills/clauset/SKILL.md`. |
| `~/.claude/skills/accounts/SKILL.md` | `setup` | The `accounts` skill, as shipped in `skills/accounts/SKILL.md`. |
| `~/.claude-accounts/<name>/` | `add`, `login` | A new account's config directory, with a link into the hub for every shared entry. `add --dir` uses the directory you name. |
| `.claude.json` of an account beside the hub | `add`, `login`, `link`, `switch` | Onboarding, theme, the one-time dialogs the hub has answered, and the trust of every project the hub trusts. Keys the account has keep their values. |
| `~/.claude/` | `add --dir` | The entries the taken-in directory has and the hub lacks. |

[Hooks, status line and skills](hooks-status-line-and-skills.md) shows the lines that go into `settings.json`.

## While Clauset runs

All of it sits in `~/.claude/clauset/`:

| Path | What |
|---|---|
| `usage/` | Each account's usage, as `clauset statusline` last got it. |
| `mcp/` | The hub's MCP servers in the shape `--mcp-config` reads, and `chrome.json` for the `chrome` key. |
| `switch/` | A move a session asked for, until its supervisor carries it out. |
| `supervised/<pid>.json` | The account a supervised `claude` runs on, removed when it ends. |
| `app.pid` | The menu bar app's process id. |
| `notify/` | Notifications waiting for the menu bar app, in a directory only you can write to. |

## Sign-ins

The Keychain item and the account's sign-in in its `.claude.json` come from `claude auth login`, which `add` and `login` start.

`clauset remove <account>` takes an account out of the config and drops its recorded usage. Its config directory and Keychain item stay.
