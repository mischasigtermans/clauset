---
title: Set up your accounts
description: "What `clauset setup` asks, and the first commands to run after it."
order: 2
---

`clauset setup` puts everything in place in one run, in a terminal. It asks before each step and leaves a step out when its files are already there, so you can run it again after an upgrade.

## What setup does

- It names the account `~/.claude` is signed in to. That directory is the hub: the one every account shares its settings, skills and sessions from.
- It signs in the other accounts you name, each in a config directory of its own.
- It asks which account new sessions start on. That's the default.
- It puts the `claude` shim, the hooks, the status line and two skills in place.

[What Clauset writes](../reference/files.md) lists every file, and [Hooks, status line and skills](../reference/hooks-status-line-and-skills.md) shows what goes into Claude Code's settings.

## Look around

```sh
clauset                          # the account here, and every account's usage
clauset which                    # which account `claude` starts on here, and why
clauset doctor                   # checks the shim, hooks, skills, status line and every account
```

```
~/code/app runs on personal  (the default)

→ personal  * Ultra  5h █░░░░░░░░░  12%  resets 14:00   7d ██░░░░░░░░  18%  resets Tue 09:00
                     me@example.com, personal | usage 4 min ago
  work        Team   5h ███░░░░░░░  31%  resets 13:00   7d ██████░░░░  61%  resets Sun 10:00
                     me@work.com, Acme | usage just now

→ here   * default   clauset use <account> makes it the default
```

The usage comes from what Claude Code stores and hands its status line, so an account shows numbers once a session has run on it.

`clauset doctor` checks the shim, the hooks, the skills, the status line, and every account's sign-in, Keychain item and links.

## From here

- `claude` now starts on the default account. [Which account a session starts on](../accounts/which-account.md) has the rule.
- [Add, sign in and remove accounts](../accounts/add-and-remove.md) covers a new account and a config directory you already use.
- [Move a session to another account](../sessions/move-a-session.md) is for when a limit nears.
- `clauset warn 80` warns at 80% of a limit. [Usage and warnings](../sessions/usage-and-warnings.md) has the rest.
