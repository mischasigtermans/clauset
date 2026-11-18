---
name: accounts
description: "The user's Claude Code accounts, run by clauset on one shared ~/.claude. Use when the account matters: which account a session runs on, how much of each account's limits is used, moving this session to another account when one nears its limit, or starting work on a chosen account. Triggers on 'which account', 'usage', 'limit', 'how much do I have left', 'switch to <account>', 'move all sessions', 'run this on <account>', 'clauset'."
user-invocable: false
---

# Accounts

`claude` is a shim: it starts Claude Code on the account of the session that started it, else on the default, which `clauset use` sets. This session's account is in its start context and in `clauset whoami`. `clauset` lists every account with its plan. The config lives in `~/.claude/clauset/config.toml`, or the file `CLAUSET_CONFIG` names. The user runs clauset directly with `/clauset <args>`.

When `clauset` is `command not found`, it isn't installed, or isn't on the PATH Claude Code has: tell the user to install it with `cargo install --locked --git https://github.com/mischasigtermans/clauset`, which needs Rust, and then run `clauset setup` in a terminal. The source and docs are at https://github.com/mischasigtermans/clauset.

## Run these yourself

- `clauset` (or `clauset usage`): the account here and every account's plan and use of its 5-hour and 7-day limits, with how old the numbers are.
- `clauset which`: the account `claude` starts on here, and why.
- `clauset doctor`: the shim, hooks, status line, sign-ins, Keychain items and links.

## Only on the user's word

`clauset use <account>` (the default), `warn <percent|off>`, `chrome <account|off>`, `add`, `remove`, `login`, `link` and `switch` change the setup or the account work runs on. `clauset setup` and `clauset login` ask questions and open a browser: hand the user the command.

## Work on another account

1. This session: `clauset switch <account>` moves it once your turn ends. It leaves the default alone: new sessions start where they did, and only `clauset use <account>` changes that, so run `use` as well only when the user asks for new sessions to go there too. The session closes and resumes with `--resume` on that account in the same terminal, so end the turn right after it and say what happens. The move also waits for the session's background shells to stop, so a watcher that never ends holds it: stop them with `/tasks`. Background agents and teammates end with the session, so let them finish first. `clauset switch` to the account the session runs on cancels the move. A session started without clauset's supervisor gets a refusal that names `clauset run <account> --resume <session-id>`: hand the user that command for a new terminal.
2. Every session on this account: `clauset switch <account> --all` moves each supervised one once its own turn ends, this one included, and lists the ones it can't move with the command that moves them by hand. From outside a session it needs `--from <account>`, the account to leave. It moves the user's other sessions too, so run it only when the user asks for all of them.
3. A headless run: `clauset run <account> -p "<prompt>"`.

After a move the first turn reads the whole conversation again, without the thinking written under the other organization: it costs more than a turn on a warm cache. MCP servers that sign in with OAuth, and the claude.ai connectors of the account's organization, keep their sign-in per account: on the other account they may need a sign-in through `/mcp` before work that uses them goes on.

When an account nears or hits its limit, report the usage and leave the switch to the user: a move costs that cold first turn, and the user picks the account.
