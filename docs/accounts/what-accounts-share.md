---
title: What accounts share
description: "What every account reads from the hub, what stays per account, and where MCP servers and plugins come from."
order: 3
---

Every account shares everything in the hub, `~/.claude`, except what Claude Code keeps per account or organization. Clauset links each shared entry from the account's own config directory into the hub.

## Shared

Settings, `CLAUDE.md`, rules, skills, plugins, MCP servers, sessions and auto-memory.

Three entries are shared on purpose: `projects/` (sessions and auto-memory), `history.jsonl` and `sessions/`, so `ListAgents` and `SendMessage` reach sessions on every account. A session started on one account resumes on another with `--resume <id>`.

## Per account

`.claude.json` and its backups, the Keychain item, policy limits, remote settings, `daemon*`, `state/`, `chrome/`, `cache/`, `telemetry/`, `statsig/`, `feedback/`, `stats-cache.json` and a few more, listed in `src/link.rs`.

## Plugins

Plugins come from the hub, but the `plugins/` directory isn't linked. `CLAUDE_CODE_PLUGIN_CACHE_DIR` points every account at the hub's, while Claude Code writes the plugins it syncs from claude.ai to `plugins/synced/<org>_<account>/` in each account's directory.

## MCP servers

MCP servers come from the hub: its user-scope ones and the directory's project-scope ones. Clauset writes them to `~/.claude/clauset/mcp/` in the shape `--mcp-config` reads and passes them to every account beside the hub.

- A start with `--strict-mcp-config` gets none of them: the flag asks Claude Code for the caller's own `--mcp-config` alone.
- `claude mcp add` in a session on such an account writes to that account's own `.claude.json`, so add servers from a session on the hub. `doctor` names user-scope servers only one account has.
- MCP servers that sign in with OAuth, and the claude.ai connectors of an account's organization, keep their sign-in per account.

## Keep the links current

`clauset link` links what the hub gained since, into one account's directory or all of them. `doctor` reports a link Claude Code replaced with a file.

For an account beside the hub, `add`, `login`, `link` and a move also copy onboarding, the theme, the one-time dialogs the hub has answered, and the trust of every project the hub trusts into its `.claude.json`. Keys the account has keep their values.
