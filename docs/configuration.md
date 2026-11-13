# Configuration

`~/.claude/clauset/config.toml`, or the file `CLAUSET_CONFIG` names. Paths may start with `~/`. `clauset setup`, `add`, `use`, `warn` and `switch` write it, keeping its comments, and refuse a change that would leave it invalid. What clauset records (usage, MCP server files, move requests) and the shim's `bin/` sit in the same directory, which `link` leaves out of the accounts.

## Keys

| Key | Meaning |
|---|---|
| `default` | The account a session started from no clauset session runs on. `clauset switch` and `clauset use <account>` set it. |
| `warn_at` | Optional. The percent of a 5-hour or 7-day limit at which every account warns. `clauset warn 80` sets it. |
| `[accounts.<name>] dir` | The account's config directory. `~/.claude` is the hub, which runs with `CLAUDE_CONFIG_DIR` unset, as Claude Code expects of its default directory. |
| `[accounts.<name>] email` | Optional. Prefilled at `clauset login`, checked by `doctor`. |
| `[accounts.<name>] warn_at` | Optional. This account's own threshold, over the global one. `clauset warn 60 --account <name>` sets it. |
| `[accounts.<name>] org` | Optional. The organization UUID the sign-in must belong to, checked by `login` and `doctor`. One login can belong to several organizations, picked in the browser. |

## Which account `claude` starts on

1. `CLAUDE_CONFIG_DIR` names a directory no account owns: `claude` starts unchanged.
2. The process descends from a session clauset started (`CLAUSET_ACCOUNT` is set), or `CLAUDE_CONFIG_DIR` names an account: that session's account.
3. The default.

`clauset which` prints the answer and the rule that gave it.

## What an account shares

Everything in the hub except what Claude Code keeps per account or org: `.claude.json` and its backups, the Keychain item, policy limits, remote settings, `daemon*`, `state/`, `chrome/`, `cache/`, `telemetry/`, `statsig/`, `feedback/`, `stats-cache.json` and a few more, listed in `src/link.rs`. `plugins/` stays apart too, because `CLAUDE_CODE_PLUGIN_CACHE_DIR` points every account at the hub's, while Claude Code writes the plugins it syncs from claude.ai to `plugins/synced/<org>_<account>/` in each account's directory.

Shared on purpose: `projects/` (sessions and auto-memory), `history.jsonl` and `sessions/` (so `ListAgents` and `SendMessage` reach sessions on every account). A session started on one account resumes on another with `--resume <id>`.

MCP servers come from the hub: its user-scope ones and the directory's project-scope ones, written to `~/.claude/clauset/mcp/` in the shape `--mcp-config` reads and passed to every account beside the hub. `claude mcp add` in a session on such an account writes to that account's own `.claude.json`, so add servers from a session on the hub; `doctor` names user-scope servers only one account has.

`clauset link` links what the hub gained since; `doctor` reports a link Claude Code replaced with a file.

`clauset add <name> --dir <dir>` takes in a config dir that ran on its own. Signed in, it keeps its sign-in. A directory with files but no `.claude.json`, or one that holds your home directory, is refused before anything moves. Its own copies of shared directories move into the hub's wherever the hub has no entry of that name, so its sessions, memory and skills join the shared ones; what would clash, and its own copy of every shared file, stays in the directory as `<name>.before-clauset`.

## What `setup` and `add` write

`setup` asks before each step and leaves a step out when its files are already in place.

| Path | Written by | What |
|---|---|---|
| `~/.claude/clauset/config.toml` | `setup`, `add` | The accounts, the default and `warn_at`. |
| `~/.claude/clauset/bin/claude` | `setup` | A link to the clauset binary: the shim. |
| `~/.zshrc` or `~/.bashrc` | `setup` | Two lines at the end: a comment and `export PATH="<home>/.claude/clauset/bin:$PATH"`, so the shim comes before every other `claude`. Another shell gets the line printed to add by hand. |
| `~/.claude/settings.json` | `setup` | The `SessionStart` and `StopFailure` hooks below, and the `statusLine` when there's none. The file is written back whole, as JSON indented by two spaces, its keys in their order. |
| `~/.claude/skills/clauset/SKILL.md` | `setup` | The `/clauset` skill, as shipped in `skills/clauset/SKILL.md`. |
| `~/.claude/skills/accounts/SKILL.md` | `setup` | The `accounts` skill, as shipped in `skills/accounts/SKILL.md`. |
| `~/.claude-accounts/<name>/` | `add`, `login` | A new account's config directory, with a link into the hub for every shared entry. `add --dir` uses the directory you name. |
| `.claude.json` of an account beside the hub | `add`, `login`, `link`, `switch` | Onboarding, theme, the one-time dialogs the hub has answered, and the trust of every project the hub trusts. Keys the account has keep their values. |
| `~/.claude/` | `add --dir` | The entries the taken-in directory has and the hub lacks. |

The Keychain item and the account's sign-in in its `.claude.json` come from `claude auth login`, which `add` and `login` start.

## Moving a session

A Claude Code process keeps the config directory it started with, so a session moves to another account by ending and resuming there. At a terminal the shim and `clauset run` keep `claude` as a child under a supervisor, and `clauset switch <account>`, run inside the session as `/clauset switch <account>`, `! clauset switch <account>` or by Claude, asks that supervisor to move it. It also makes the account the default, so new sessions start there too.

1. The session finishes its turn. Once Claude Code's session registry says it's idle and it runs no background shell, the supervisor ends it.
2. The supervisor starts `claude --resume <session id>` on the other account in the same terminal, with the flags that shape the session (permissions, model, effort, agent, settings, system prompt, added directories, tools, name); its prompt and the flags that pick a session stay behind. An account beside the hub also gets the trust the first account gave the directory; the hub's `.claude.json` is left to the sessions that write it.

Background shells would end with the session, so the move waits until they have stopped: a watcher that never ends holds it until it's stopped through `/tasks`. `clauset switch` to the account the session runs on cancels a move it asked for and makes that account the default. A refused switch, from a session without the supervisor, leaves the default as it was. Background agents and teammates run inside the session and end with it. The first turn on the other account reads the whole conversation again, without the thinking written under the other organization. A session that ends on its own before it moves stays ended.

`clauset switch <account> --all` moves every session on the account being left, not only the one it runs in: the account the enclosing session runs on, or the one `--from <account>` names, which a terminal outside a session needs. Each supervisor notes the account its `claude` runs on in `~/.claude/clauset/supervised/<pid>.json` and removes the note when `claude` ends, since macOS keeps a process's environment from other processes. `--all` writes the request of every session whose supervisor runs it on that account, and each moves as above, once its own turn ends and its background shells have stopped; the output names the busy ones and those that wait for a shell. It makes the account the default once, and only when at least one session will move. A second `--all` writes the same requests again, so a session waiting to move keeps its place and gets only the newest target. A terminal session without a supervisor note, from a supervisor older than this version among others, stays where it is: the output lists it with the `clauset run <account> --resume <id>` that moves it by hand, since clauset can't tell its account. Sessions that Claude Code started for an SDK or another tool are left out.

MCP servers that sign in with OAuth, and the claude.ai connectors of an account's organization, keep their sign-in per account. On the account a session moves to they may need a sign-in through `/mcp`; clauset reads no token, so it can't tell beforehand.

A session that runs without the supervisor, because stdin was no terminal or it started before clauset, moves by hand: end it and run `clauset run <account> --resume <id>`. `/login` inside a session signs in the config directory itself, so it changes the account of every session that runs on it.

## Skills

`setup` writes two skills into the hub's `skills/`, so every account has them:

- `clauset`: `/clauset <args>` runs clauset in the session. Its body holds the line ``!`clauset $ARGUMENTS 2>&1 || true` ``, which Claude Code runs itself before the model reads the skill, since its frontmatter has `allowed-tools: Bash(clauset:*)`. Its `disable-model-invocation: true` keeps the model from invoking it with arguments of its own. It finds `clauset` on PATH; when it's missing there, the skill tells you how to install it.
- `accounts`: tells Claude which clauset commands it may run itself (`clauset`, `which`, `doctor`), which wait for your word, how to move a session, and how to install clauset when it's missing.

`doctor` warns about a missing skill and notes one that differs from the shipped one. `setup` writes a missing skill, and offers to replace one that differs, since you may have changed it on purpose.

The install command both skills name is `cargo install --locked --git https://github.com/mischasigtermans/clauset`; the source is at [github.com/mischasigtermans/clauset](https://github.com/mischasigtermans/clauset).

## Usage

`clauset` shows each account's use of its 5-hour and 7-day limits from two places Claude Code fills itself, whichever is fresher:

- `cachedUsageUtilization` in the account's `.claude.json`, written each time Claude Code fetches the account's usage.
- The `rate_limits` Claude Code hands a status line command, which `clauset statusline` keeps per account in `~/.claude/clauset/usage/`. Claude Code sends them for subscription plans, Team included.

An account shows numbers once a session has run on it; the overview says how old they are. A window Claude Code leaves out, as it does with a 5-hour window the API doesn't report, shows as `no 5h window`.

With `warn_at` set, `clauset statusline` puts up a macOS notification the first time a window passes it, once per window: after the reset it may warn again. The `StopFailure` hook adds one when an account hits its limit, with every other account's fullest window beside it. clauset never moves work to another account on a limit; the notification leaves that choice to you.

## Hooks and status line

In the hub's `settings.json`, shared by every account, with the path of the clauset binary `setup` ran from (`~/.cargo/bin/clauset` after `cargo install`):

```json
"SessionStart": [{"hooks": [{"type": "command", "command": "~/.cargo/bin/clauset hook context", "timeout": 5}]}],
"StopFailure": [{"matcher": "rate_limit", "hooks": [{"type": "command", "command": "~/.cargo/bin/clauset hook limit", "timeout": 5}]}]
```

`setup` gives a hook with another matcher, or one that runs a clauset installed elsewhere, the current one, and `doctor` reports it.

A status line command pipes the JSON Claude Code hands it through `clauset statusline`, which prints the account and keeps its usage. When there's no status line, `setup` writes `"statusLine": {"type": "command", "command": "~/.cargo/bin/clauset statusline"}`, and offers to point one that runs another clauset's `statusline` at its own. Any other status line it leaves alone and names the pipe to add.

## Solo

[Solo](https://soloterm.com) is a terminal app that starts agents from agent tools. One agent tool per account: command `~/.cargo/bin/clauset`, arguments `run <account> --dangerously-skip-permissions`, tool type `claude` set by hand so resume keeps working. The stock Claude Code tool starts on the default once the shim is on PATH.

## Undocumented behaviour clauset relies on

- The Keychain item of a config directory is `Claude Code-credentials-` and the first 8 hex digits of the SHA-256 of its resolved path. `doctor` checks each one exists.
- Claude Code gives every process its tools start `CLAUDE_PID` and `CLAUDE_CODE_SESSION_ID`, keeps each interactive session's `status` (`busy`, `idle`) in `sessions/<pid>.json`, starts each Bash tool shell from `<config dir>/shell-snapshots/snapshot-*`, and ends a session cleanly on SIGTERM. `clauset switch` rests on all four.
- Each entry in `sessions/<pid>.json` holds the `sessionId`, `name`, `cwd` and `entrypoint` (`cli` for a terminal session) of a running session, and `procStart`, its process's start time as `ps -o lstart=` prints it in UTC; clauset accepts it in the C locale or the inherited one. `clauset switch --all` finds the sessions to move there and leaves out an entry whose pid now belongs to a process started at another time.
- `.claude.json` keeps `cachedUsageUtilization`, and `oauthAccount.planDisplayName` for personal plans (`Ultra`); a Team organization has no display name.
