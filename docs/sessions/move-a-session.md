---
title: Move a session to another account
description: "`clauset switch` ends a running session once its turn is over and resumes it on another account, in the same terminal."
order: 1
---

A Claude Code process keeps the config directory it started with, so a session moves to another account by ending and resuming there. At a terminal the shim and `clauset run` keep `claude` as a child under a supervisor, and `clauset switch` asks that supervisor to move it.

## Move this session

Run it inside the session, as a skill or as a shell line:

```
/clauset switch <account>
! clauset switch <account>
```

Or ask Claude to switch. The session finishes its turn first.

## What happens

1. The session finishes its turn. Once Claude Code's session registry says it's idle and it runs no background shell, the supervisor ends it.
2. The supervisor starts `claude --resume <session id>` on the other account in the same terminal, with the flags that shape the session (permissions, model, effort, agent, settings, system prompt, added directories, tools, name). Its prompt and the flags that pick a session stay behind.

An account beside the hub also gets the trust the first account gave the directory. The hub's `.claude.json` is left to the sessions that write it.

## What a move costs

- Background agents and teammates run inside the session and end with it. Let them finish first.
- The first turn on the other account reads the whole conversation again, without the thinking written under the other organization.
- MCP servers that sign in with OAuth, and the claude.ai connectors of an account's organization, keep their sign-in per account. On the account a session moves to they may need a sign-in through `/mcp`. Clauset reads no token, so it can't tell beforehand.

## Waiting and cancelling

Background shells would end with the session, so the move waits until they have stopped. A watcher that never ends holds it until you stop it through `/tasks`.

`clauset switch` to the account the session runs on cancels a move it asked for. A session that ends on its own before it moves stays ended.

## Move every session

```sh
clauset switch work --all                    # inside a session: every session on its account
clauset switch work --all --from personal    # from any terminal
```

`clauset switch <account> --all` moves every session on the account being left, not only the one it runs in. That account is the one the enclosing session runs on, or the one `--from <account>` names, which a terminal outside a session needs. In the menu bar app, 'Move Sessions' does the same.

Each session moves as above, once its own turn ends and its background shells have stopped. The output names the busy ones and those that wait for a shell. A second `--all` writes the same requests again, so a session waiting to move keeps its place and gets only the newest target.

Each supervisor notes the account its `claude` runs on in `~/.claude/clauset/supervised/<pid>.json` and removes the note when `claude` ends, since macOS keeps a process's environment from other processes. `--all` writes the request of every session whose supervisor runs it on that account.

## Sessions Clauset can't move

A session runs without the supervisor when stdin was no terminal or it started before Clauset. It moves by hand: end it and run `clauset run <account> --resume <id>`.

With `--all`, a terminal session without a supervisor note, from a supervisor older than this version among others, stays where it is. The output lists it with the `clauset run <account> --resume <id>` that moves it by hand, since Clauset can't tell its account. Sessions that Claude Code started for an SDK or another tool are left out.

## The default stays

A move touches running sessions alone. New sessions start where they did, and a session that leaves the default account is told so:

```
New sessions still start on personal: `clauset use work` changes that.
```

[Which account a session starts on](../accounts/which-account.md#change-the-default) has `clauset use`.
