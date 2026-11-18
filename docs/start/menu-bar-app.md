---
title: The menu bar app
description: "What the app shows, what its menus do, and how it updates."
order: 3
---

`macos/` holds Clauset, a menu bar app on top of `clauset status --json`. It runs `clauset` for every read and change. It reads the overview with a bare environment, so it shows what a new terminal would. While the app runs, Clauset's notifications come through it, under its name. [Usage and warnings](../sessions/usage-and-warnings.md#how-a-notification-reaches-you) says how.

## Install or build

[Install Clauset](install.md#the-app) has the download and the Homebrew cask. To build it yourself:

```sh
macos/bundle.sh
cp -R macos/build/Clauset.app ~/Applications/
open ~/Applications/Clauset.app
```

`bundle.sh` builds `macos/build/Clauset.app`, signed ad hoc. It needs macOS 14.4 and Xcode, and the first build downloads Sparkle 2.10.1 from GitHub and checks its hash.

The released app carries its own `clauset` and uses that, unless it runs from its disk image. A build of your own runs the `clauset` that `setup` linked the `claude` shim to, else the one Homebrew or Cargo installed. A menu bar manager such as Ice or Bartender may put the new item in its hidden section.

## The menu bar

The menu bar shows one account, the default until you pick another, as a meter: a thick bar for its 5-hour window over a thin one for its 7-day window, beside the higher of the two percentages.

'Menu Bar' under 'Settings' picks the account the menu bar follows, the default or a fixed one, and which of its bars, percentage and name show there.

## The menu

The menu is a standard macOS menu. Every account is an item with a bar for each limit it has: its 5-hour window, its 7-day window and the weekly limit of one model, such as Fable. A bar is blue, orange from 70% and red from 90%.

The percentages stand under the account's name. A limit past the warning threshold shows its reset there, and the tooltip has every reset. A name shows in title case, so `steddle-2` reads `Steddle 2`.

- Choosing an account makes it the default.
- 'Move Sessions' moves every session from one account to another, and first says what a move does. Where new sessions start stays as it is.
- 'Accounts' holds what concerns one account: its own warning threshold, signing it in again and removing it. It also adds an account.
- 'Settings' holds the warning threshold for every account, the `chrome` account, opening at login and checking for updates.

## Windows

Adding an account, signing in again, `doctor` and `link` run inside the app: a window shows what `clauset` prints, and a sign-in opens your browser. `doctor` and `link` first say what they do, and run on your word.

A new account asks for a name, a label for the menu. It refuses an email address, lowers capitals and writes a space as a hyphen.

'Set Up…' lists the setup steps, each with what it writes and a button that carries it out. It opens by itself on a Mac Clauset isn't set up on, and the menu shows 'Finish Setting Up…' while a step waits, such as a skill a newer version changed.

## Updates

The first time it opens, the app asks two things before it does either: whether to open at login, and whether to check for updates.

With 'Check for Updates Automatically' on, the app fetches its release feed from GitHub once a week. GitHub sees what any download shows it: your IP address, the app's version and your preferred languages. The app sends nothing about your accounts.

A newer version shows in the menu as 'Update to 0.4.0…' until it's installed, and no window opens by itself. 'Check for Updates…' asks now. Installing downloads the version from GitHub, checks its signature, replaces the app and opens it again. Updates run on [Sparkle](https://sparkle-project.org).
