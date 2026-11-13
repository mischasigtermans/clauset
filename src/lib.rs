//! Several Claude Code accounts on one Mac, sharing one `~/.claude`.
//!
//! Every account keeps its own config directory, so Claude Code keeps its own
//! sign-in, Keychain item and `.claude.json` for it. Everything else in that
//! directory is a link into the hub, `~/.claude`. clauset never reads a token
//! and makes no network call: only the `claude` binary talks to Anthropic.

#![deny(clippy::all)]

pub mod account;
pub mod config;
pub mod doctor;
pub mod edit;
pub mod hook;
pub mod launch;
pub mod link;
pub mod setup;
pub mod state;
pub mod status;
pub mod switch;
pub mod usage;
