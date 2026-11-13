//! Links from an account's config dir into the hub.

use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;

use anyhow::{Context, Result};

/// Config-dir entries that belong to one account or org, after
/// larcane97/clausona's survey of Claude Code 2.1.299, rechecked on 2.1.318.
/// `plugins` stays apart because `CLAUDE_CODE_PLUGIN_CACHE_DIR` points every
/// account at the hub's, while Claude Code still writes the plugins it syncs
/// from claude.ai to `plugins/synced/<org>_<account>` in each config dir.
const OWN: &[&str] = &[
    ".claude.json",
    ".config.json",
    ".credentials.json",
    "active-time.json",
    "backups",
    "cache",
    "chrome",
    "feedback",
    "gh-pr-status-cache.json",
    "hfi-auth.json",
    "image-cache",
    "mcp-discovery-cache",
    "mcp-needs-auth-cache.json",
    "plugins",
    "state",
    "stats-cache.json",
    "statsig",
    "telemetry",
    ".last-update-result.json",
    ".session-stats.json",
    // The hub's own repository and editor files, and clauset's directory: a
    // linked `.git` would make the account's directory read as a work tree.
    "clauset",
    ".git",
    ".gitattributes",
    ".gitignore",
    ".idea",
    ".DS_Store",
    "LICENSE",
    "README.md",
];

const OWN_PREFIXES: &[&str] = &[
    ".claude.json.",
    ".config.json.",
    ".claude-custom-oauth.json",
    ".claude-local-oauth.json",
    ".claude-staging-oauth.json",
    ".session_ingress_token",
    "daemon",
    "policy-limits.json",
    "remote-settings",
];

pub fn is_own(name: &str) -> bool {
    OWN.contains(&name)
        || OWN_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        || name.contains(".tmp.")
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Hub entries without a link, made now when applied.
    pub added: Vec<String>,
    /// A file or directory of the account's own where a link belongs.
    pub copies: Vec<String>,
    /// A link that points somewhere other than the hub's entry.
    pub foreign: Vec<String>,
    /// A link to a hub entry that is gone, removed when applied.
    pub dangling: Vec<String>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.added.is_empty()
            && self.copies.is_empty()
            && self.foreign.is_empty()
            && self.dangling.is_empty()
    }
}

pub fn sync(hub: &Path, dir: &Path, apply: bool) -> Result<Report> {
    let mut report = Report::default();

    for entry in fs::read_dir(hub).with_context(|| format!("reading {}", hub.display()))? {
        let name = entry?.file_name();
        let Some(text) = name.to_str() else { continue };
        if is_own(text) {
            continue;
        }
        let target = hub.join(&name);
        let place = dir.join(&name);
        match fs::symlink_metadata(&place) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if apply {
                    symlink(&target, &place)
                        .with_context(|| format!("linking {}", place.display()))?;
                }
                report.added.push(text.to_owned());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("reading {}", place.display()));
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                if fs::read_link(&place)? != target {
                    report.foreign.push(text.to_owned());
                }
            }
            Ok(_) => report.copies.push(text.to_owned()),
        }
    }

    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_symlink() {
            continue;
        }
        let place = entry.path();
        let target = fs::read_link(&place)?;
        if target.starts_with(hub) && fs::symlink_metadata(&target).is_err() {
            if apply {
                fs::remove_file(&place).with_context(|| format!("removing {}", place.display()))?;
            }
            report
                .dangling
                .push(entry.file_name().to_string_lossy().into_owned());
        }
    }

    for list in [
        &mut report.added,
        &mut report.copies,
        &mut report.foreign,
        &mut report.dangling,
    ] {
        list.sort();
    }
    Ok(report)
}

/// Takes in a config dir that ran on its own. What it has that the hub lacks
/// moves into the hub, and its own copies of shared directories move into
/// the hub's wherever the hub has no entry of that name, so its sessions,
/// memory and skills join the shared ones. What stays, and every own copy of
/// a shared file, is renamed `<name>.before-clauset`. Then everything shared
/// is linked. Returns the names set aside.
pub fn adopt(hub: &Path, dir: &Path) -> Result<Vec<String>> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(text) = name.to_str() else { continue };
        if is_own(text) || text.ends_with(".before-clauset") || entry.file_type()?.is_symlink() {
            continue;
        }
        let shared = hub.join(&name);
        if fs::symlink_metadata(&shared).is_err() {
            fs::rename(entry.path(), &shared)
                .with_context(|| format!("moving {} into the hub", entry.path().display()))?;
        }
    }
    let mut aside = Vec::new();
    for name in sync(hub, dir, false)?.copies {
        let own = dir.join(&name);
        let shared = hub.join(&name);
        if own.is_dir() && shared.is_dir() {
            merge(&own, &shared)?;
            if fs::remove_dir(&own).is_ok() {
                continue;
            }
        }
        let parked = dir.join(format!("{name}.before-clauset"));
        fs::rename(&own, &parked).with_context(|| format!("moving {} aside", own.display()))?;
        aside.push(name);
    }
    sync(hub, dir, true)?;
    Ok(aside)
}

/// Moves what `from` holds into `into` where `into` has no entry of that
/// name, descending into directories both have. Nothing in `into` is touched.
fn merge(from: &Path, into: &Path) -> Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = into.join(entry.file_name());
        match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::rename(&source, &target)
                    .with_context(|| format!("moving {}", source.display()))?
            }
            Err(error) => {
                return Err(error).with_context(|| format!("reading {}", target.display()));
            }
            Ok(meta) if meta.is_dir() && entry.file_type()?.is_dir() => {
                merge(&source, &target)?;
                // Succeeds only once `source` is empty.
                let _ = fs::remove_dir(&source);
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adopting_moves_sessions_into_the_hub_and_parks_the_rest() {
        let root = tempfile::tempdir().unwrap();
        let hub = root.path().join("hub");
        let dir = root.path().join("old");
        fs::create_dir_all(hub.join("projects/-a")).unwrap();
        fs::create_dir_all(dir.join("projects/-a")).unwrap();
        fs::create_dir_all(dir.join("projects/-b/memory")).unwrap();
        fs::write(hub.join("projects/-a/s1.jsonl"), "hub").unwrap();
        fs::write(dir.join("projects/-a/s1.jsonl"), "old").unwrap();
        fs::write(dir.join("projects/-a/s2.jsonl"), "old").unwrap();
        fs::write(dir.join("projects/-b/memory/MEMORY.md"), "m").unwrap();
        fs::write(hub.join("settings.json"), "{}").unwrap();
        fs::write(dir.join("settings.json"), "{\"old\":1}").unwrap();
        fs::write(dir.join(".claude.json"), "{}").unwrap();
        fs::create_dir_all(dir.join("agents")).unwrap();
        fs::write(dir.join("agents/reviewer.md"), "r").unwrap();

        let aside = adopt(&hub, &dir).unwrap();

        assert!(
            hub.join("agents/reviewer.md").is_file(),
            "what the hub lacked moved in"
        );
        assert_eq!(
            fs::read_link(dir.join("agents")).unwrap(),
            hub.join("agents")
        );

        assert_eq!(aside, ["projects", "settings.json"]);
        assert_eq!(
            fs::read_to_string(hub.join("projects/-a/s1.jsonl")).unwrap(),
            "hub"
        );
        assert_eq!(
            fs::read_to_string(hub.join("projects/-a/s2.jsonl")).unwrap(),
            "old"
        );
        assert!(hub.join("projects/-b/memory/MEMORY.md").exists());
        assert_eq!(
            fs::read_to_string(dir.join("projects.before-clauset/-a/s1.jsonl")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(dir.join("settings.json.before-clauset")).unwrap(),
            "{\"old\":1}"
        );
        assert_eq!(
            fs::read_link(dir.join("projects")).unwrap(),
            hub.join("projects")
        );
        assert!(dir.join(".claude.json").is_file());
    }

    #[test]
    fn keeps_account_state_apart() {
        for name in [
            ".claude.json",
            ".claude.json.backup.1730000000",
            "policy-limits.json.signature-iat.json.lock",
            "remote-settings-consent.json",
            "daemon.lock",
            "plugins",
            "history.jsonl.tmp.123.abcdef012345",
            "clauset",
        ] {
            assert!(is_own(name), "{name}");
        }
        for name in [
            "settings.json",
            "projects",
            "sessions",
            "history.jsonl",
            "skills",
        ] {
            assert!(!is_own(name), "{name}");
        }
    }

    #[test]
    fn links_what_is_shared_and_reports_what_is_not_a_link() {
        let root = tempfile::tempdir().unwrap();
        let hub = root.path().join("hub");
        let dir = root.path().join("acct");
        fs::create_dir_all(hub.join("skills")).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(hub.join("settings.json"), "{}").unwrap();
        fs::write(hub.join("CLAUDE.md"), "").unwrap();
        fs::write(hub.join("history.jsonl"), "").unwrap();
        fs::write(hub.join("stats-cache.json"), "{}").unwrap();
        fs::write(dir.join("CLAUDE.md"), "own").unwrap();
        symlink("/somewhere", dir.join("history.jsonl")).unwrap();
        symlink(hub.join("gone"), dir.join("gone")).unwrap();

        let dry = sync(&hub, &dir, false).unwrap();
        assert_eq!(dry.added, ["settings.json", "skills"]);
        assert!(fs::symlink_metadata(dir.join("skills")).is_err());

        let applied = sync(&hub, &dir, true).unwrap();
        assert_eq!(applied.added, ["settings.json", "skills"]);
        assert_eq!(applied.copies, ["CLAUDE.md"]);
        assert_eq!(applied.foreign, ["history.jsonl"]);
        assert_eq!(applied.dangling, ["gone"]);
        assert_eq!(
            fs::read_link(dir.join("skills")).unwrap(),
            hub.join("skills")
        );
        assert!(fs::symlink_metadata(dir.join("gone")).is_err());
        assert!(fs::symlink_metadata(dir.join("stats-cache.json")).is_err());

        let again = sync(&hub, &dir, true).unwrap();
        assert!(again.added.is_empty() && again.dangling.is_empty());
    }
}
