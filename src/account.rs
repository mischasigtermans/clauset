//! Adding accounts: signing a new one in, or taking in a config dir that
//! already exists.

use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::{Config, same};
use crate::edit::ConfigFile;
use crate::state::{self, SignIn};
use crate::{launch, link};

pub fn valid_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        bail!("`{name}` is no account name: use lowercase letters, digits, - and _");
    }
    Ok(())
}

/// Adds an account to the config. A directory that exists, other than the
/// hub, is taken in first: its own entries fold into the hub before they
/// become links. Without a sign-in it then gets one through `claude auth
/// login`.
pub fn add(home: &Path, name: &str, email: Option<&str>, dir: Option<&Path>) -> Result<()> {
    valid_name(name)?;
    let mut file = ConfigFile::open(home)?;
    if file.has_account(name) {
        bail!("`{name}` is already an account: `clauset login {name}` signs it in again");
    }
    let dir = match dir {
        Some(dir) => {
            let dir = env::current_dir()?.join(dir);
            dir.canonicalize().unwrap_or(dir)
        }
        None => home.join(".claude-accounts").join(name),
    };
    let hub = home.join(".claude");
    let is_hub = same(&dir, &hub);
    let state_file = if is_hub {
        home.join(".claude.json")
    } else {
        dir.join(".claude.json")
    };
    let signed_in = state::read(&state_file)
        .ok()
        .as_ref()
        .and_then(state::sign_in);

    if is_hub && signed_in.is_none() {
        bail!("~/.claude is not signed in: run `claude auth login` first");
    }
    // Taking a directory in moves its entries into the hub, so it must be a
    // config dir: Claude Code writes `.claude.json` into every one it ran
    // with, and keeps it through a logout.
    if !is_hub {
        let home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
        if home.starts_with(&dir) {
            bail!(
                "{} holds your home directory: pass a Claude config dir",
                dir.display()
            );
        }
        let has_files = fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_some());
        if has_files && !state_file.exists() {
            bail!(
                "{} is no Claude config dir: it holds files but no .claude.json",
                dir.display()
            );
        }
    }
    let email = signed_in.as_ref().map(|s| s.email.as_str()).or(email);
    let org = signed_in.as_ref().map(|s| s.org_uuid.as_str());
    file.add_account(name, &dir, email, org);
    if !file.has_default() {
        file.set_default(name);
    }
    let config = file.save()?;
    if !is_hub && dir.is_dir() {
        let aside = link::adopt(&hub, &dir)?;
        seed(&config, name)?;
        if !aside.is_empty() {
            println!(
                "{name}: its own {} stay beside the links, renamed <name>.before-clauset",
                aside.join(", ")
            );
        }
    }
    if let Some(sign_in) = signed_in {
        println!("{name}: {}", describe_sign_in(&sign_in));
        return Ok(());
    }
    let sign_in = login(&config, name)?;
    let mut file = ConfigFile::open(home)?;
    file.set_org(name, &sign_in.org_uuid);
    file.save()?;
    Ok(())
}

pub fn describe_sign_in(sign_in: &SignIn) -> String {
    format!("{}, {} ({})", sign_in.email, sign_in.org(), sign_in.plan())
}

pub fn describe(report: &link::Report) -> String {
    let mut parts = Vec::new();
    if !report.added.is_empty() {
        parts.push(format!("linked {}", report.added.join(", ")));
    }
    if !report.dangling.is_empty() {
        parts.push(format!("removed dangling {}", report.dangling.join(", ")));
    }
    if !report.copies.is_empty() {
        parts.push(format!(
            "own copies where a link belongs: {}",
            report.copies.join(", ")
        ));
    }
    if !report.foreign.is_empty() {
        parts.push(format!(
            "links that miss the hub: {}",
            report.foreign.join(", ")
        ));
    }
    if parts.is_empty() {
        "every shared entry is a link into the hub".into()
    } else {
        parts.join("; ")
    }
}

/// Fills in what the hub has answered and the account lacks; returns whether
/// the account's state changed.
pub fn seed(config: &Config, name: &str) -> Result<bool> {
    let account = config.account(name)?;
    if config.is_hub(account) {
        return Ok(false);
    }
    let hub_state = state::read(&config.home.join(".claude.json"))?;
    let path = config.state_file(account);
    let mut own = if path.exists() {
        state::read(&path)?
    } else {
        serde_json::json!({})
    };
    let changed = state::seed(&hub_state, &mut own);
    if changed {
        state::write(&path, &own)?;
    }
    Ok(changed)
}

/// Prepares an account's directory and signs it in. Signing the hub in again
/// would change the account under every session running on it, so the hub is
/// left to `claude auth login`.
pub fn login(config: &Config, name: &str) -> Result<SignIn> {
    let account = config.account(name)?;
    if config.is_hub(account) {
        bail!(
            "{name} is the hub: signing it in again changes the account of every session running on it. \
             Run `claude auth login` yourself if that is what you want."
        );
    }
    fs::create_dir_all(&account.dir)
        .with_context(|| format!("creating {}", account.dir.display()))?;
    let report = link::sync(&config.hub(), &account.dir, true)?;
    println!("{name}: {}", describe(&report));
    if seed(config, name)? {
        println!("{name}: copied onboarding, answered dialogs and project trust from the hub");
    }

    let claude = launch::real_claude()?;
    let mut command = launch::command(config, name, &claude, None, false)?;
    command.args(["auth", "login"]);
    if let Some(email) = &account.email {
        command.args(["--email", email]);
    }
    if !command
        .status()
        .context("starting claude auth login")?
        .success()
    {
        bail!("claude auth login failed");
    }
    let path = config.state_file(account);
    let sign_in = state::read(&path)
        .ok()
        .as_ref()
        .and_then(state::sign_in)
        .with_context(|| format!("claude auth login wrote no sign-in to {}", path.display()))?;
    println!("{name}: signed in as {}", describe_sign_in(&sign_in));
    if let Some(org) = &account.org
        && org != &sign_in.org_uuid
    {
        bail!("that is not the configured org {org}: sign in again and pick it");
    }
    Ok(sign_in)
}

/// Links the shim to this binary; returns the directory it lives in.
pub fn init(home: &Path) -> Result<PathBuf> {
    let bin = Config::path(home)
        .parent()
        .context("the config path has no directory")?
        .join("bin");
    fs::create_dir_all(&bin).with_context(|| format!("creating {}", bin.display()))?;
    let exe = env::current_exe()?.canonicalize()?;
    let shim = bin.join("claude");
    if fs::read_link(&shim).ok().as_deref() != Some(exe.as_path()) {
        if fs::symlink_metadata(&shim).is_ok() {
            fs::remove_file(&shim).with_context(|| format!("replacing {}", shim.display()))?;
        }
        symlink(&exe, &shim).with_context(|| format!("linking {}", shim.display()))?;
    }
    Ok(bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lowercase_words() {
        for name in ["ultra-work", "team_2", "a"] {
            assert!(valid_name(name).is_ok(), "{name}");
        }
        for name in ["", "Ultra", "-x", "a b", "a/b"] {
            assert!(valid_name(name).is_err(), "{name}");
        }
    }
}
