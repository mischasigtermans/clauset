---
title: Hooks, status line and skills
description: "What `setup` adds to Claude Code: two hooks, a status line command and two skills."
order: 3
---

All three live in the hub, so every account has them. `clauset doctor` checks each one.

## Hooks

In the hub's `settings.json`, shared by every account, with the path of the `clauset` binary `setup` ran from (`~/.cargo/bin/clauset` after `cargo install`):

```json
"SessionStart": [{"hooks": [{"type": "command", "command": "~/.cargo/bin/clauset hook context", "timeout": 5}]}],
"StopFailure": [{"matcher": "rate_limit", "hooks": [{"type": "command", "command": "~/.cargo/bin/clauset hook limit", "timeout": 5}]}]
```

`SessionStart` tells the session which account it runs on. `StopFailure` posts a notification when an account hits its limit.

`setup` gives a hook with another matcher, or one that runs a Clauset installed elsewhere, the current one, and `doctor` reports it.

## Status line

A status line command pipes the JSON Claude Code hands it through `clauset statusline`, which prints the account and keeps its usage.

When there's no status line, `setup` writes this one:

```json
"statusLine": {"type": "command", "command": "~/.cargo/bin/clauset statusline"}
```

It offers to point a status line that runs another Clauset's `statusline` at its own. Any other status line it leaves alone and names the pipe to add.

## Skills

`setup` writes two skills into the hub's `skills/`:

- `clauset`: `/clauset <args>` runs Clauset in the session. Its body holds the line ``!`clauset $ARGUMENTS 2>&1 || true` ``, which Claude Code runs itself before the model reads the skill, since its frontmatter has `allowed-tools: Bash(clauset:*)`. Its `disable-model-invocation: true` keeps the model from invoking it with arguments of its own. It finds `clauset` on PATH. When it's missing there, the skill tells you how to install it.
- `accounts`: tells Claude which `clauset` commands it may run itself (`clauset`, `which`, `doctor`), which wait for your word, how to move a session, and how to install Clauset when it's missing.

`doctor` warns about a missing skill and notes one that differs from the shipped one. `setup` writes a missing skill, and offers to replace one that differs, since you may have changed it on purpose.

The install command both skills name is `cargo install --locked --git https://github.com/mischasigtermans/clauset`. The source is at [github.com/mischasigtermans/clauset](https://github.com/mischasigtermans/clauset).
