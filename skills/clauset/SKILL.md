---
name: clauset
description: "Runs clauset in this session: `/clauset switch <account>` moves this session and makes the account the default, `/clauset switch <account> --all` moves every session on this account, `/clauset` shows the accounts and their usage, `/clauset which` shows where sessions start."
argument-hint: "[switch <account> [--all] | which | doctor]"
disable-model-invocation: true
allowed-tools: Bash(clauset:*)
---

# /clauset

!`clauset $ARGUMENTS 2>&1 || true`

The user typed `/clauset <args>`: the output above is `clauset <args>`, already run. Report it in a line or two and run nothing more for it. After a `switch`, end the turn right away: the session moves once the turn is over. The `accounts` skill holds the rest of what clauset does.

When the output says `command not found`, clauset isn't installed, or isn't on the PATH Claude Code has: tell the user to install it with `cargo install --locked --git https://github.com/mischasigtermans/clauset`, which needs Rust, and then run `clauset setup` in a terminal. The source and docs are at https://github.com/mischasigtermans/clauset.
