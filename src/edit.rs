//! Changes to `config.toml` that keep its comments and order. Every change is
//! parsed as a whole config before it is written, so an edit never leaves a
//! config the shim cannot read.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, Table, value};

use crate::config::{self, Config};

pub struct ConfigFile {
    path: PathBuf,
    home: PathBuf,
    doc: DocumentMut,
}

impl ConfigFile {
    pub fn open(home: &Path) -> Result<Self> {
        let path = Config::path(home);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
        };
        let doc = text
            .parse()
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(ConfigFile {
            path,
            home: home.to_path_buf(),
            doc,
        })
    }

    pub fn has_account(&self, name: &str) -> bool {
        self.doc
            .get("accounts")
            .and_then(Item::as_table)
            .is_some_and(|accounts| accounts.contains_key(name))
    }

    pub fn has_default(&self) -> bool {
        self.doc.contains_key("default")
    }

    pub fn set_default(&mut self, name: &str) {
        self.doc["default"] = value(name);
    }

    pub fn add_account(&mut self, name: &str, dir: &Path, email: Option<&str>, org: Option<&str>) {
        if self.doc.get("accounts").is_none() {
            let mut accounts = Table::new();
            accounts.set_implicit(true);
            self.doc["accounts"] = Item::Table(accounts);
        }
        let mut account = Table::new();
        account["dir"] = value(config::tilde(dir, &self.home));
        if let Some(email) = email {
            account["email"] = value(email);
        }
        if let Some(org) = org {
            account["org"] = value(org);
        }
        self.doc["accounts"][name] = Item::Table(account);
    }

    /// Sets or, with `None`, removes `warn_at`, for one account or all.
    pub fn set_warn(&mut self, account: Option<&str>, percent: Option<u8>) {
        let table = match account {
            Some(name) => self.doc["accounts"][name].as_table_like_mut(),
            None => Some(self.doc.as_table_mut() as &mut dyn toml_edit::TableLike),
        };
        if let Some(table) = table {
            match percent {
                Some(percent) => {
                    table.insert("warn_at", value(i64::from(percent)));
                }
                None => {
                    table.remove("warn_at");
                }
            }
        }
    }

    pub fn set_org(&mut self, name: &str, org: &str) {
        self.doc["accounts"][name]["org"] = value(org);
    }

    /// Writes the config after checking it parses as one.
    pub fn save(&self) -> Result<Config> {
        let text = self.doc.to_string();
        let config = Config::parse(&text, &self.home)
            .context("the change would leave the config invalid")?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self
            .path
            .with_extension(format!("toml.{}", std::process::id()));
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path).with_context(|| format!("writing {}", self.path.display()))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> (tempfile::TempDir, ConfigFile) {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join(".claude/clauset/config.toml");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, text).unwrap();
        let file = ConfigFile {
            path: config,
            home: home.path().to_path_buf(),
            doc: text.parse().unwrap(),
        };
        (home, file)
    }

    const BASE: &str = "# my accounts\ndefault = \"a\"\n\n[accounts.a]\ndir = \"~/.claude\"\n";

    #[test]
    fn adds_an_account_keeping_comments() {
        let (home, mut file) = file(BASE);
        file.add_account(
            "b",
            &home.path().join(".claude-accounts/b"),
            Some("b@example.com"),
            None,
        );
        file.save().unwrap();
        let text = file.doc.to_string();
        assert!(text.starts_with("# my accounts\n"), "{text}");
        assert!(text.contains("dir = \"~/.claude-accounts/b\""), "{text}");
    }

    #[test]
    fn refuses_to_save_a_config_that_does_not_parse() {
        let (_home, mut file) = file(BASE);
        file.set_default("nobody");
        let error = file.save().unwrap_err();
        assert!(
            format!("{error:#}").contains("`default` names `nobody`"),
            "{error:#}"
        );
    }
}
