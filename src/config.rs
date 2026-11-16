//! `~/.claude/clauset/config.toml` and the rule that picks an account.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Set on every Claude Code process clauset starts, so a `claude` started
/// inside it can tell an inherited account from a fresh terminal. The hub's
/// sessions run without `CLAUDE_CONFIG_DIR`, so that variable cannot say it.
pub const MARKER: &str = "CLAUSET_ACCOUNT";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub default: String,
    /// Percent of a 5-hour or 7-day limit at which to warn, for every account.
    pub warn_at: Option<f64>,
    /// The account whose claude.ai sign-in the Claude in Chrome extension
    /// uses; every other account reaches the browser through it.
    pub chrome: Option<String>,
    pub accounts: BTreeMap<String, Account>,
    #[serde(skip)]
    pub home: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub dir: PathBuf,
    pub email: Option<String>,
    /// The organization UUID the sign-in must belong to.
    pub org: Option<String>,
    /// This account's own `warn_at`.
    pub warn_at: Option<f64>,
}

/// The account a new `claude` gets, and why.
#[derive(Debug, PartialEq, Eq)]
pub enum Choice<'a> {
    /// `CLAUDE_CONFIG_DIR` names a directory no account owns: hands off.
    Foreign(PathBuf),
    Inherited(&'a str),
    Default(&'a str),
}

impl<'a> Choice<'a> {
    pub fn account(&self) -> Option<&'a str> {
        match *self {
            Choice::Foreign(_) => None,
            Choice::Inherited(name) | Choice::Default(name) => Some(name),
        }
    }
}

/// The account a running Claude Code process is signed in as.
#[derive(Debug, PartialEq, Eq)]
pub enum Session<'a> {
    Account(&'a str),
    Foreign(PathBuf),
    Unknown,
}

impl Config {
    pub fn path(home: &Path) -> PathBuf {
        std::env::var_os("CLAUSET_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude/clauset/config.toml"))
    }

    pub fn load(home: &Path) -> Result<Self> {
        let path = Self::path(home);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text, home).with_context(|| format!("in {}", path.display()))
    }

    /// The config, or `None` when there is no config file yet. A file that
    /// does not parse is still an error.
    pub fn load_if_present(home: &Path) -> Result<Option<Self>> {
        if !Self::path(home).exists() {
            return Ok(None);
        }
        Self::load(home).map(Some)
    }

    pub fn parse(text: &str, home: &Path) -> Result<Self> {
        let mut config: Config = toml::from_str(text)?;
        let home = &fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
        config.home = home.clone();
        for account in config.accounts.values_mut() {
            account.dir = expand(&account.dir, home);
        }
        if !config.accounts.contains_key(&config.default) {
            bail!(
                "`default` names `{}`, which is not an account",
                config.default
            );
        }
        if let Some(chrome) = &config.chrome
            && !config.accounts.contains_key(chrome)
        {
            bail!("`chrome` names `{chrome}`, which is not an account");
        }
        Ok(config)
    }

    pub fn hub(&self) -> PathBuf {
        self.home.join(".claude")
    }

    /// The hub account runs with `CLAUDE_CONFIG_DIR` unset: set to `~/.claude`,
    /// Claude Code would look for a Keychain item and a `.claude.json` that the
    /// hub's sign-in never wrote.
    pub fn is_hub(&self, account: &Account) -> bool {
        same(&account.dir, &self.hub())
    }

    pub fn state_file(&self, account: &Account) -> PathBuf {
        if self.is_hub(account) {
            self.home.join(".claude.json")
        } else {
            account.dir.join(".claude.json")
        }
    }

    /// The percent at which an account warns, if it does.
    pub fn warn_at(&self, name: &str) -> Option<f64> {
        self.accounts.get(name)?.warn_at.or(self.warn_at)
    }

    pub fn account(&self, name: &str) -> Result<&Account> {
        self.accounts
            .get(name)
            .with_context(|| format!("no account named `{name}`"))
    }

    pub fn session(&self, config_dir: Option<&OsStr>) -> Session<'_> {
        match config_dir.filter(|dir| !dir.is_empty()) {
            Some(dir) => {
                let dir = Path::new(dir);
                match self
                    .accounts
                    .iter()
                    .find(|(_, account)| same(&account.dir, dir))
                {
                    Some((name, _)) => Session::Account(name),
                    None => Session::Foreign(dir.to_path_buf()),
                }
            }
            None => match self
                .accounts
                .iter()
                .find(|(_, account)| self.is_hub(account))
            {
                Some((name, _)) => Session::Account(name),
                None => Session::Unknown,
            },
        }
    }

    /// A foreign config dir passes through untouched, then the account of the
    /// session that started this one, then the default.
    pub fn resolve(&self, config_dir: Option<&OsStr>, marked: bool) -> Choice<'_> {
        let config_dir = config_dir.filter(|dir| !dir.is_empty());
        let session = self.session(config_dir);
        if let Session::Foreign(dir) = session {
            return Choice::Foreign(dir);
        }
        if let Session::Account(name) = session
            && (marked || config_dir.is_some())
        {
            return Choice::Inherited(name);
        }
        Choice::Default(&self.default)
    }

    /// `path` with the home directory written as `~`.
    pub fn tilde(&self, path: &Path) -> String {
        tilde(path, &self.home)
    }
}

/// `path` with the home directory written as `~`.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .context("HOME is not set")
}

pub fn expand(path: &Path, home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// Two paths name the same directory: compared resolved where both exist, as
/// written otherwise, since an account's directory may not exist before login.
pub fn same(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/nonexistent/home";

    fn config(text: &str) -> Config {
        Config::parse(text, Path::new(HOME)).unwrap()
    }

    fn sample() -> Config {
        config(
            r#"
            default = "personal"

            [accounts.personal]
            dir = "~/.claude-accounts/personal"

            [accounts.team]
            dir = "~/.claude"

            [accounts.work]
            dir = "~/.claude-accounts/work"
            "#,
        )
    }

    fn p(path: &str) -> PathBuf {
        Path::new(HOME).join(path)
    }

    #[test]
    fn expands_the_home_directory() {
        let config = sample();
        assert_eq!(
            config.accounts["personal"].dir,
            p(".claude-accounts/personal")
        );
    }

    #[test]
    fn refuses_a_name_that_is_no_account() {
        let error = Config::parse(
            "default = \"x\"\n[accounts.a]\ndir = \"/a\"\n",
            Path::new(HOME),
        )
        .unwrap_err();
        assert!(error.to_string().contains("`default` names `x`"));

        let error = Config::parse(
            "default = \"a\"\nchrome = \"x\"\n[accounts.a]\ndir = \"/a\"\n",
            Path::new(HOME),
        )
        .unwrap_err();
        assert!(error.to_string().contains("`chrome` names `x`"));
    }

    #[test]
    fn a_foreign_config_dir_passes_through() {
        let config = sample();
        let choice = config.resolve(Some(OsStr::new("/elsewhere")), true);
        assert_eq!(choice, Choice::Foreign(PathBuf::from("/elsewhere")));
        assert_eq!(choice.account(), None);
    }

    #[test]
    fn a_new_session_inherits_a_marked_session() {
        let config = sample();
        let work = p(".claude-accounts/work");
        assert_eq!(
            config.resolve(Some(work.as_os_str()), false),
            Choice::Inherited("work")
        );
        // The hub runs without CLAUDE_CONFIG_DIR, so only the marker tells a
        // child of a hub session from a fresh terminal.
        assert_eq!(config.resolve(None, true), Choice::Inherited("team"));
        assert_eq!(config.resolve(None, false), Choice::Default("personal"));
    }

    #[test]
    fn the_session_without_config_dir_is_the_hub() {
        let config = sample();
        assert_eq!(config.session(None), Session::Account("team"));
        assert_eq!(
            config.session(Some(OsStr::new(""))),
            Session::Account("team")
        );
        let personal = p(".claude-accounts/personal");
        assert_eq!(
            config.session(Some(personal.as_os_str())),
            Session::Account("personal")
        );
    }

    #[test]
    fn writes_home_as_a_tilde() {
        let config = sample();
        assert_eq!(config.tilde(&p("code/app")), "~/code/app");
        assert_eq!(config.tilde(Path::new(HOME)), "~");
        assert_eq!(config.tilde(Path::new("/opt")), "/opt");
    }
}
