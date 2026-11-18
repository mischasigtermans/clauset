---
title: For programs and agent tools
description: "Start Claude Code on a chosen account, and read or carry out Clauset's state from a program."
order: 4
---

The menu bar app reads `clauset status --json` and carries out setup steps through `clauset setup --apply`. Another program can do the same.

## Start on an account

```sh
clauset run work
clauset run work -p "<prompt>"
```

`clauset run <account> [args]` starts Claude Code on one account and passes every further argument on. At a terminal it keeps `claude` under the supervisor, so the session can [move](../sessions/move-a-session.md) later.

## Read the overview

```sh
clauset status --json
```

`clauset status --json` gives the overview to a program: every account with its sign-in, usage, weekly limits per model, warning threshold and number of supervised sessions, plus the default and `chrome`.

## Carry out a setup step

```sh
clauset setup --json
clauset setup --apply skills
```

`clauset setup --json` says which of the shim, the hooks, the status line and the skills are in place. `clauset setup --apply <shim|hooks|statusline|skills>` puts one in place without asking. It leaves a status line of your own alone, and writes over a skill that differs only with `--replace`, which goes with `--apply skills` alone.

Both JSON outputs carry `"version": 1`. A release that renames a field or changes its type raises that number; one that adds a field leaves it.

## Solo

[Solo](https://soloterm.com) is a terminal app that starts agents from agent tools. One agent tool per account: command `~/.cargo/bin/clauset`, arguments `run <account> --dangerously-skip-permissions`, tool type `claude` set by hand so resume keeps working.

The stock Claude Code tool starts on the default once the shim is on PATH.
