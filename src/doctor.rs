//! `clauset doctor`: every check reads local state.

use std::env;
use std::process::Command;

use anyhow::Result;

use crate::config::Config;
use crate::{launch, link, state};

#[derive(Default)]
struct Out {
    failed: bool,
}

impl Out {
    fn ok(&self, line: impl AsRef<str>) {
        println!("  ✓ {}", line.as_ref());
    }
    fn warn(&self, line: impl AsRef<str>) {
        println!("  ! {}", line.as_ref());
    }
    fn fail(&mut self, line: impl AsRef<str>) {
        self.failed = true;
        println!("  ✗ {}", line.as_ref());
    }
}

/// Prints the checks; returns whether none failed.
pub fn run(config: &Config) -> Result<bool> {
    let mut out = Out::default();

    println!("claude");
    match launch::real_claude() {
        Ok(claude) => {
            let version = Command::new(&claude)
                .arg("--version")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                .unwrap_or_default();
            out.ok(format!("{} ({version})", config.tilde(&claude)));
        }
        Err(error) => out.fail(format!("{error:#}")),
    }
    match first_claude_is_clauset() {
        true => out.ok("the shim is the first `claude` on PATH"),
        false => out.warn("the shim is not the first `claude` on PATH: run `clauset setup`"),
    }
    let missing = crate::setup::missing_hooks(&config.home);
    match missing.is_empty() {
        true => out.ok("the context and limit hooks are in the hub's settings"),
        false => out.warn(format!(
            "hooks missing from the hub's settings, or set up with another matcher or clauset: {} (run `clauset setup`)",
            missing.join(", ")
        )),
    }
    let (missing, differ) = crate::setup::skills_state(&config.home);
    let differs = match differ.is_empty() {
        true => String::new(),
        false => format!(
            "; {} {} from this version's (`clauset setup` offers it)",
            differ.join(", "),
            if differ.len() == 1 {
                "differs"
            } else {
                "differ"
            }
        ),
    };
    match missing.is_empty() {
        true => out.ok(format!(
            "the clauset and accounts skills are in the hub's skills{differs}"
        )),
        false => out.warn(format!(
            "skills missing from the hub's skills: {} (run `clauset setup`){differs}",
            missing.join(", ")
        )),
    }
    match crate::setup::statusline_runs_clauset(&config.home) {
        true => out.ok("the status line runs `clauset statusline`"),
        false => out.warn("the status line does not run `clauset statusline`: usage stays stale and `warn_at` never fires"),
    }

    println!("accounts");
    for (name, account) in &config.accounts {
        let hub = config.is_hub(account);
        let mark = match (name == &config.default, hub) {
            (true, true) => " (default, hub)",
            (true, false) => " (default)",
            (false, true) => " (hub)",
            (false, false) => "",
        };
        println!(" {name}{mark}");
        if !account.dir.is_dir() {
            out.fail(format!(
                "{} does not exist: run `clauset login {name}`",
                config.tilde(&account.dir)
            ));
            continue;
        }
        let own_state = state::read(&config.state_file(account)).ok();
        match own_state.as_ref().and_then(state::sign_in) {
            None => out.fail(format!("not signed in: run `clauset login {name}`")),
            Some(sign_in) => {
                let line = crate::account::describe_sign_in(&sign_in);
                match (&account.org, &account.email) {
                    (Some(org), _) if org != &sign_in.org_uuid => out.fail(format!(
                        "{line}: org {} is not the configured {org}",
                        sign_in.org_uuid
                    )),
                    (_, Some(email)) if email != &sign_in.email => {
                        out.fail(format!("{line}: not the configured {email}"))
                    }
                    _ => out.ok(line),
                }
            }
        }
        let service = state::keychain_service(&account.dir, hub);
        match state::keychain_has(&service) {
            true => out.ok(format!("Keychain item {service}")),
            false => out.fail(format!(
                "no Keychain item {service}: run `clauset login {name}`"
            )),
        }
        if hub {
            continue;
        }
        let report = link::sync(&config.hub(), &account.dir, false)?;
        if report.is_clean() {
            out.ok("every shared entry is a link into the hub");
        }
        if !report.added.is_empty() {
            out.warn(format!(
                "not linked yet: {} (run `clauset link`)",
                report.added.join(", ")
            ));
        }
        if !report.copies.is_empty() {
            out.fail(format!(
                "own copies where a link belongs: {}",
                report.copies.join(", ")
            ));
        }
        if !report.foreign.is_empty() {
            out.fail(format!(
                "links that miss the hub: {}",
                report.foreign.join(", ")
            ));
        }
        if !report.dangling.is_empty() {
            out.warn(format!(
                "links to hub entries that are gone: {} (run `clauset link`)",
                report.dangling.join(", ")
            ));
        }
        if let Some(own_state) = &own_state {
            let servers = state::mcp_servers(own_state);
            if !servers.is_empty() {
                out.warn(format!(
                    "user-scope MCP servers only this account has: {}",
                    servers.join(", ")
                ));
            }
        }
    }

    Ok(!out.failed)
}

pub fn first_claude_is_clauset() -> bool {
    let me = env::current_exe().and_then(|exe| exe.canonicalize()).ok();
    env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("claude"))
        .find(|candidate| candidate.exists())
        .and_then(|first| first.canonicalize().ok())
        .is_some_and(|first| Some(first) == me)
}
