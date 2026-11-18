---
title: Which account a session starts on
description: "The rule the `claude` shim follows, and the commands that change the answer."
order: 1
---

`setup` puts a shim named `claude` before every other `claude` on PATH. Each time you start Claude Code, the shim picks the account and hands over to the real binary.

## The rule

1. `CLAUDE_CONFIG_DIR` names a directory no account owns: `claude` starts unchanged.
2. The process descends from a session Clauset started (`CLAUSET_ACCOUNT` is set), or `CLAUDE_CONFIG_DIR` names an account: that session's account.
3. The default.

So a session you start from a terminal runs on the default, and a `claude` started from inside a session stays on that session's account.

`clauset which` prints the answer for the directory you're in and the rule that gave it.

## Change the default

```sh
clauset use personal
```

`clauset use <account>` is the only command that sets the default. Moving a session with `clauset switch` leaves it alone. In the menu bar app, choosing an account does the same as `use`.

## Start on one account

```sh
clauset run work
clauset run work --resume <session id>
```

`clauset run <account> [args]` starts Claude Code on that account and passes every further argument on. It's for a terminal, or for a tool that starts agents: [For programs and agent tools](../reference/programs.md) has an example.

## The hub

`~/.claude` is the hub. It runs with `CLAUDE_CONFIG_DIR` unset, as Claude Code expects of its default directory, and every other account links what it shares from there. [What accounts share](what-accounts-share.md) lists it.
