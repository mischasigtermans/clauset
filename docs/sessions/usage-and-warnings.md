---
title: Usage and warnings
description: "Where the usage numbers come from, how to read the bars, and when Clauset warns you."
order: 2
---

`clauset` shows each account's use of its 5-hour and 7-day limits. Clauset never moves work to another account on a limit: a warning leaves that choice to you.

## Where the numbers come from

Two places Claude Code fills itself, whichever is fresher:

- `cachedUsageUtilization` in the account's `.claude.json`, written each time Claude Code fetches the account's usage.
- The `rate_limits` Claude Code hands a status line command, which `clauset statusline` keeps per account in `~/.claude/clauset/usage/`. Claude Code sends them for subscription plans, Team included.

An account shows numbers once a session has run on it. The overview says how old they are.

## Read the bars

A window Claude Code leaves out is left out of the overview too: a plan without a 5-hour limit shows its 7-day bar alone.

| Use | `clauset` in a terminal | The menu bar app |
|---|---|---|
| Below 70% | Blue | Blue |
| From 70% | Yellow | Orange |
| From 90% | Red | Red |

## A model's own weekly limit

A weekly limit that counts one model alone, such as Fable's, is only in `cachedUsageUtilization`: a status line gets no such limit. The overview shows it on the line under the bars with its own age, which is the time since Claude Code last fetched the account's usage, often a day or more.

## Warnings

```sh
clauset warn 80                  # every account warns at 80% of a limit
clauset warn 60 --account work   # this account's own threshold
clauset warn off                 # remove the threshold
```

With `warn_at` set, `clauset statusline` puts up a macOS notification the first time a window passes it, once per window. After the reset it may warn again.

The `StopFailure` hook adds a notification when an account hits its limit, with every other account's fullest window beside it.

Both notifications look at the 5-hour and 7-day windows alone: a model's own weekly limit is too old a number to warn on.

## How a notification reaches you

The menu bar app writes its process id to `app.pid`. While that process runs, Clauset leaves a notification as a file in `notify/`, a directory only you can write to, and the app posts it under its own name. Without the app, or when it has no permission to notify, AppleScript puts it up.

A notification names an account as the menu bar app shows it: `work-1` reads `Work 1`.
