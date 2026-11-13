# clauset

[![Version](https://img.shields.io/github/v/release/mischasigtermans/clauset?label=version)](https://github.com/mischasigtermans/clauset/releases)
[![License](https://img.shields.io/github/license/mischasigtermans/clauset)](LICENSE)

Several Claude Code accounts on one Mac, sharing one `~/.claude`.

Each account keeps its own sign-in in its own config directory. Everything else is shared from `~/.claude`: settings, `CLAUDE.md`, rules, skills, plugins, MCP servers, sessions and auto-memory. A `claude` shim starts a new session on the default account, or on the account of the session that started it, and `clauset switch` moves a running session to another account when the one it runs on nears its limit. clauset reads no token and makes no network call. Only the official `claude` binary talks to Anthropic.

## Installation

```sh
cargo install --locked --git https://github.com/mischasigtermans/clauset
clauset setup
```

`cargo install` builds clauset from this repository and puts it in `~/.cargo/bin`, which has to be on PATH; rustup's installer puts it there. Run the same command again to upgrade. A clauset installed somewhere else than before takes over the shim, hooks and status line once you run `clauset setup` from it. `setup` names the account `~/.claude` is signed in to, signs in the others, asks which account new sessions start on, and puts the shim, the hooks, the status line and two skills in place. It asks before each step and skips what's already there. [docs/configuration.md](docs/configuration.md) lists every file it writes.

### Requires

- macOS: Claude Code keeps each sign-in in the login Keychain.
- Claude Code: tested on 2.1.322. clauset rests on behaviour Claude Code doesn't document, listed in [docs/configuration.md](docs/configuration.md#undocumented-behaviour-clauset-relies-on).
- Rust 1.88 or later to build, from [rustup](https://rustup.rs).

## Quick start

```sh
clauset                          # the account here, and every account's usage
clauset use personal             # the account new sessions start on
clauset add work --email me@work.com         # sign in another account
clauset add old --dir ~/.claude-old          # take in a config dir you already use
clauset switch work              # inside a session: move it there, and start new sessions there
clauset switch work --all        # move every session on this account there
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

## Features

- `claude` starts on the account of the session that started it, else on the default. A `CLAUDE_CONFIG_DIR` that names no account passes through untouched.
- `clauset add --dir` takes in a config dir from another tool or alias. Its sessions, memory and skills join the hub's, and what would clash stays beside them.
- Plugins come from the hub through `CLAUDE_CODE_PLUGIN_CACHE_DIR`; the hub's user-scope and project-scope MCP servers through `--mcp-config`. Their OAuth sign-ins, and claude.ai connectors, stay per account.
- `clauset warn 80` puts up a notification when an account passes 80% of a limit, and a hook adds one when it hits the limit. Which account to go on with stays your choice.
- `clauset switch <account>` moves a running session to another account: run inside the session, as `/clauset switch work`, `! clauset switch work` or by Claude when you ask, it ends the session once its turn is over and its background shells have stopped, and resumes it there in the same terminal. It also makes the account the default for new sessions. `--all` moves every session on the account you leave, each in its own terminal once its own turn is over.
- Two skills: `/clauset <args>` runs clauset inside a session, and `accounts` tells Claude what each command does and which ones wait for your word.
- `clauset run <account> [args]` starts Claude Code on one account, for a terminal or a tool that starts agents, such as [Solo](https://soloterm.com).
- `clauset doctor` checks the shim, hooks, skills, status line, and every account's sign-in, Keychain item and links.

## Documentation

- [docs/configuration.md](docs/configuration.md): the config file, how the account is picked, what's shared, what `setup` and `add` write, moving a session, usage and the hooks.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## Credits

- [Mischa Sigtermans](https://mischa.sigtermans.me)
- [larcane97/clausona](https://github.com/larcane97/clausona), for its survey of the state Claude Code keeps per account
- [vika2603/ccs](https://github.com/vika2603/ccs), for the shim on a single `~/.claude`

## License

MIT. See [LICENSE](LICENSE).
