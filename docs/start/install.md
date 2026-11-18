---
title: Install Clauset
description: "Homebrew or Cargo, what Clauset needs, and how to upgrade."
order: 1
---

Clauset is a menu bar app and a CLI, `clauset`. The app carries the CLI and runs setup in a window. Without the app, install the CLI and run `clauset setup` in a terminal. [Set up your accounts](set-up.md) says what that asks.

## The app

Download [Clauset.dmg](https://github.com/mischasigtermans/clauset/releases/latest/download/Clauset.dmg), drag Clauset to Applications and open it from there. Or with Homebrew:

```sh
brew install --cask mischasigtermans/tap/clauset
```

The cask also puts `clauset` on PATH. From the disk image, the app's setup puts it beside the `claude` shim. The app needs macOS 14.4 or later, and [The menu bar app](menu-bar-app.md) says what it shows.

## The CLI, with Homebrew

```sh
brew install mischasigtermans/tap/clauset-cli
clauset setup
```

Homebrew builds the latest release from source and brings its own Rust. Installing by the formula's full name adds the tap and tells Homebrew you trust that formula, so you need no `brew tap` or `brew trust` first.

## The CLI, with Cargo

```sh
cargo install --locked clauset
clauset setup
```

`cargo install` builds Clauset from crates.io and puts it in `~/.cargo/bin`, which has to be on PATH. rustup's installer puts it there. It needs Rust 1.88 or later, from [rustup](https://rustup.rs).

## What it needs

- macOS: Claude Code keeps each sign-in in the login Keychain.
- Claude Code: tested on 2.1.322. Clauset rests on behaviour Claude Code doesn't document, listed in [Undocumented behaviour](../reference/undocumented-behaviour.md).

## Upgrade

The app updates itself: it shows a newer version in its menu and installs it when you choose to. With Homebrew, `brew upgrade` does it; coming from 0.2.0, run `clauset setup` once after that upgrade. With Cargo, run the install command again. A Clauset installed somewhere else than before takes over the shim, the hooks and the status line once you run `clauset setup` from it.

`clauset doctor` notes a skill that differs from the one this version ships, and `clauset setup` offers to replace it.

## Tokens and the network

Clauset reads no token, and the CLI makes no network call. Only the official `claude` binary talks to Anthropic.

The menu bar app asks before it goes online. With your yes it fetches its release feed from GitHub once a week, and it downloads a newer version from there when you choose to install it. [The menu bar app](menu-bar-app.md#updates) says what GitHub sees.
