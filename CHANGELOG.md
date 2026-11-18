# Changelog

## [0.3.0] - 2026-11-18

Clauset gets a menu bar app, and `clauset switch` stops changing where new sessions start.

**Breaking**

- `clauset switch` moves sessions and leaves the default alone, with or without `--all`. It made the account it moved to the default. Now only `clauset use` sets where new sessions start, and a session that leaves the default account is told they still start there. Run `clauset use <account>` as well when new sessions should follow.

**Added**

- Clauset in `macos/`, a menu bar app that shows every account's usage, sets the default, the warning thresholds and `chrome`, and moves sessions between accounts. It reads and changes everything through `clauset`, which it carries inside it.
- The app as `Clauset.dmg` on each release, and as the Homebrew cask `mischasigtermans/tap/clauset`, which also puts `clauset` on PATH. The formula `clauset-cli` stays for the CLI alone.
- `clauset setup` links `clauset` beside the `claude` shim, so the CLI is on PATH wherever the shim is.
- The menu bar app updates itself through Sparkle. It asks the first time it opens. With your yes it fetches its release feed from GitHub once a week, shows a newer version in the menu and installs it when you choose to. `clauset` itself still makes no network call.
- While the menu bar app runs, notifications come through it, under its name.
- `clauset status --json` gives every account with its sign-in, usage, warning threshold and number of supervised sessions, plus the default and `chrome`, for a program to read.
- The overview shows an account's weekly limit per model, such as Fable's, with how old that number is: it comes from Claude Code's own cache alone.
- `clauset setup --json` says what each setup step still needs, and `clauset setup --apply <shim|hooks|statusline|skills>` carries one out without asking. `--apply skills --replace` also writes this version's skill over one that differs.
- Both JSON outputs carry `"version": 1`. A release that renames a field or changes its type raises that number.
- `clauset chrome <account|off>` sets or removes the `chrome` key.
- `clauset remove <account>` takes an account out of the config and leaves its directory and sign-in. It refuses the default, the `chrome` account, the hub and an account sessions run on.

**Changed**

- `clauset login` signs the hub in again once no session runs on it. It refused the hub outright, and left it to `claude auth login`.
- The overview leaves out a window an account doesn't have, where it wrote `no 5h window`.
- A usage bar is blue, yellow from 70% and red from 90%. It was green, yellow from 50% and red from 80%.
- A notification names an account as the menu bar app shows it: `work-1` reads `Work 1`. It gave the name as the config has it.
- `clauset add` refuses the name `off`, which `clauset chrome off` and `clauset warn off` read as a word of their own.

**Fixed**

- `clauset setup` writes the path `clauset` was started from into the shim, the hooks and the status line. It wrote the file behind that path, which for a Homebrew install lies in a folder the next `brew upgrade` deletes: the shim, the hooks and the status line then pointed at nothing.

**Migration**

Installed 0.2.0 with Homebrew: run `clauset setup` once after `brew upgrade`, to point the shim, the hooks and the status line at a path that lasts.

## [0.2.0] - 2026-11-16

**Added**

- A `chrome` key names the account the Claude in Chrome extension is signed in to. Every other account reaches that browser through it, as the `chrome` MCP server.

## [0.1.1] - 2026-11-13

**Fixed**

- `--strict-mcp-config` works on an account beside the hub. clauset added the hub's servers through `--mcp-config` on every start, so a caller who asked for its own servers alone still got all of the hub's.

## [0.1.0] - 2026-11-13

**Added**

- A `claude` shim that starts Claude Code on the account of the session that started it, else on the default.
- The shim leaves a `CLAUDE_CONFIG_DIR` that names no account untouched, and starts `claude` unchanged without a config.
- `clauset` (or `clauset usage`) shows the account here and every account's plan, sign-in and use of its 5-hour and 7-day limits, from Claude Code's own `cachedUsageUtilization` and the status line's `rate_limits`.
- `clauset setup` names the hub's account, suggesting a name after its organization.
- `clauset setup` signs in more accounts.
- `clauset setup` asks which account new sessions start on.
- `clauset setup` installs the shim, the hooks, the status line and the `clauset` and `accounts` skills, asking before each step.
- `clauset setup` points hooks and a status line that run a clauset installed elsewhere at its own, and replaces a skill that differs from the shipped one only when you say so.
- `clauset setup` asks for a warning threshold once the status line runs `clauset statusline`.
- `clauset use <account>` sets the default, keeping the config's comments.
- `clauset add` signs in a new account.
- `clauset add --dir` takes in an existing config dir, folding its sessions, memory and skills into the hub.
- Accounts beside the hub use the hub's plugins through `CLAUDE_CODE_PLUGIN_CACHE_DIR`.
- Accounts beside the hub get the hub's user-scope and project-scope MCP servers through `--mcp-config`, from files that hold only the servers.
- `clauset hook context` on `SessionStart` gives the model its account.
- `clauset statusline` prints the account and keeps its usage.
- `clauset warn <percent|off> [--account]` sets `warn_at`, and the status line notifies once per window when an account passes it.
- `clauset hook limit` on `StopFailure` notifies when an account hits its limit.
- `clauset switch <account>` moves a session the shim or `clauset run` started at a terminal to another account once its turn ends, resuming it with `--resume` in the same terminal with the flags that shape it.
- `clauset switch <account>` makes the account the default for new sessions.
- `clauset switch` waits for the session's background shells to stop before it moves it, and refuses a session without the supervisor, leaving the default as it was.
- `clauset switch` to the session's own account cancels a move.
- `clauset switch <account> --all [--from <account>]` moves every supervised session on the account being left, names the sessions it can't move with the command that moves them by hand, and makes the account the default once.
- The `/clauset <args>` skill runs clauset inside a session.
- The `accounts` skill tells Claude what each clauset command does and which ones wait for the user.
- Both skills name the install command when `clauset` isn't found.
- `clauset run <account>` starts Claude Code on one account.
- `clauset login <account>` signs an account in again.
- `clauset doctor` checks the shim, the hooks, the skills, the status line, and every account's sign-in, Keychain item and links.
- `clauset which` names the account `claude` starts on here, and why.
- `clauset link` links what the hub gained since into the accounts.
