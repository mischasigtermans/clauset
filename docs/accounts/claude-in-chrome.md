---
title: Claude in Chrome on every account
description: "Name the account the browser extension is signed in to, and every other account reaches the same browser."
order: 4
---

Claude Code reaches the browser extension through a relay that pairs a session only with extensions signed in to its own claude.ai account. A session on another account finds no browser. The `chrome` key fixes that.

## Set it

```sh
clauset chrome personal          # the account the extension is signed in to
clauset chrome off               # remove the key
```

## What it does

With `chrome` set, Clauset starts a session on every other account with `--no-chrome` and Claude Code's own Chrome server, `claude --claude-in-chrome-mcp`, run on the named account's sign-in from `~/.claude/clauset/mcp/chrome.json`.

Claude Code keeps the name `claude-in-chrome` for its built-in server, so this one is called `chrome` and its tools `mcp__chrome__*`.

## What stays the same

- The session still applies its own organization's Claude in Chrome policy.
- Claude Code keeps the preferred browser in the named account's `.claude.json`, so a browser picked with `select_browser` from any account holds for all of them.
- The named account itself, a config without `chrome`, and a start with `--chrome`, `--no-chrome` or `--strict-mcp-config` keep Claude Code's own handling.

The key rests on behaviour Claude Code doesn't document. [Undocumented behaviour](../reference/undocumented-behaviour.md#claude-in-chrome) says what happens if an update changes it.
