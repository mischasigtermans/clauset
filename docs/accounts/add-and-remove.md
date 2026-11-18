---
title: Add, sign in and remove accounts
description: "A new account, a config directory you already use, signing in again and taking an account out."
order: 2
---

Each account keeps its own sign-in in its own config directory. A name is lowercase letters, digits, `-` and `_`.

## Add a new account

```sh
clauset add work --email me@work.com
```

`add` makes `~/.claude-accounts/work/`, links every shared entry of the hub into it and starts `claude auth login`, which opens your browser. `--email` prefills the address.

The Keychain item and the sign-in in the account's `.claude.json` come from `claude auth login`. Clauset reads no token.

## Take in a directory you already use

```sh
clauset add old --dir ~/.claude-old
```

`clauset add <name> --dir <dir>` takes in a config dir that ran on its own, from another tool or an alias. Signed in, it keeps its sign-in.

Its own copies of shared directories move into the hub's wherever the hub has no entry of that name, so its sessions, memory and skills join the shared ones. What would clash, and its own copy of every shared file, stays in the directory as `<name>.before-clauset`.

A directory with files but no `.claude.json`, or one that holds your home directory, is refused before anything moves.

## Sign in again

```sh
clauset login work
```

Claude Code keeps one sign-in per config directory, so a new one changes the account under every session running there. For the hub, where a `claude` started outside Clauset runs too, `login` therefore waits until no session runs on it.

`/login` inside a session signs in the config directory itself, so it changes the account of every session that runs on it.

## Remove an account

```sh
clauset remove old
```

`clauset remove <account>` takes an account out of the config file and drops its recorded usage. Its config directory and Keychain item stay, so `clauset add <account> --dir <dir>` takes it back.

It refuses the default, the `chrome` account, the hub and an account sessions run on. To remove the default, pick another first with `clauset use <account>`.

## Check an account

`clauset doctor` checks every account's sign-in, Keychain item and links. With `email` or `org` in the [config file](../reference/config-file.md#keys), it also checks that the sign-in matches.
