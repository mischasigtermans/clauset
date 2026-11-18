use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SignIn {
    pub email: String,
    pub org_name: String,
    pub org_uuid: String,
    pub org_type: String,
    pub plan_name: String,
}

impl SignIn {
    /// The plan's display name as Claude Code stores it for personal plans
    /// (`Ultra`), else named after the organization type. A Team
    /// organization stores no display name.
    pub fn plan(&self) -> &str {
        if !self.plan_name.is_empty() {
            return &self.plan_name;
        }
        match self.org_type.as_str() {
            "claude_max" => "Max",
            "claude_pro" => "Pro",
            "claude_team" => "Team",
            "claude_enterprise" => "Enterprise",
            other => other,
        }
    }

    /// The organization's name, or `personal` for the one Claude makes per
    /// login and names after its email.
    pub fn org(&self) -> &str {
        if self.org_name.ends_with("'s Organization") {
            "personal"
        } else {
            &self.org_name
        }
    }
}

pub fn read(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Replaces `path` whole, readable by its owner alone, as Claude Code keeps it.
pub fn write(path: &Path, value: &Value) -> Result<()> {
    let tmp = path.with_extension(format!("json.clauset.{}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

pub fn sign_in(state: &Value) -> Option<SignIn> {
    let account = state.get("oauthAccount")?;
    let field = |key: &str| {
        account
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    Some(SignIn {
        email: field("emailAddress"),
        org_name: field("organizationName"),
        org_uuid: field("organizationUuid"),
        org_type: field("organizationType"),
        plan_name: field("planDisplayName"),
    })
}

/// The user-scope MCP servers in a `.claude.json`, by name.
pub fn mcp_servers(state: &Value) -> Vec<String> {
    state
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(|servers| servers.keys().cloned().collect())
        .unwrap_or_default()
}

/// What a person answers once and Claude Code asks again on every account
/// that lacks it: onboarding, theme, and the one-time dialogs and notices of
/// 2.1.318. Keys tied to an account or organization stay out.
const ANSWERED_ONCE: &[&str] = &[
    "hasCompletedOnboarding",
    "lastOnboardingVersion",
    "theme",
    "hasCompletedClaudeInChromeOnboarding",
    "claudeInChromeDefaultEnabled",
    "effortCalloutDismissed",
    "effortCalloutV2Dismissed",
    "hasSeenAutoModeEntryWarning",
    "hasSeenAutoDefaultNudge",
    "hasSeenTasksHint",
    "hasIdeOnboardingBeenShown",
    "remoteDialogSeen",
    "resumeReturnDismissed",
];

/// Keys the account has keep their values. Returns whether anything changed.
pub fn seed(hub: &Value, state: &mut Value) -> bool {
    if !state.is_object() {
        *state = Value::Object(Default::default());
    }
    let mut changed = false;
    for key in ANSWERED_ONCE {
        if state.get(key).is_none()
            && let Some(value) = hub.get(key)
        {
            state[key] = value.clone();
            changed = true;
        }
    }
    if let Some(projects) = hub.get("projects").and_then(Value::as_object) {
        for (path, project) in projects {
            if project.get("hasTrustDialogAccepted") == Some(&Value::Bool(true)) {
                changed |= trust(state, path);
            }
        }
    }
    changed
}

/// Marks `path` trusted in an account's state; returns whether it was not.
pub fn trust(state: &mut Value, path: &str) -> bool {
    if state["projects"][path]["hasTrustDialogAccepted"] == Value::Bool(true) {
        return false;
    }
    state["projects"][path]["hasTrustDialogAccepted"] = Value::Bool(true);
    true
}

pub fn trusts(state: &Value, path: &str) -> bool {
    state["projects"][path]["hasTrustDialogAccepted"] == Value::Bool(true)
}

/// The Keychain service Claude Code signs a config dir in under: the bare
/// name for the hub, else suffixed with the first 8 hex digits of the SHA-256
/// of the resolved directory.
pub fn keychain_service(dir: &Path, hub: bool) -> String {
    const SERVICE: &str = "Claude Code-credentials";
    if hub {
        return SERVICE.into();
    }
    let dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    format!("{SERVICE}-{}", short_hash(&dir))
}

pub fn short_hash(path: &Path) -> String {
    Sha256::digest(path.as_os_str().as_encoded_bytes())[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Asks for the item's attributes only, never the secret.
pub fn keychain_has(service: &str) -> bool {
    Command::new("security")
        .args(["find-generic-password", "-s", service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn seeds_onboarding_theme_and_trust_once() {
        let hub = json!({
            "hasCompletedOnboarding": true,
            "lastOnboardingVersion": "2.1.91",
            "theme": "light",
            "hasCompletedClaudeInChromeOnboarding": true,
            "groveChoiceSeenAt": {"hub-account": 1},
            "oauthAccount": {"emailAddress": "hub@example.com"},
            "projects": {
                "/trusted": {"hasTrustDialogAccepted": true, "allowedTools": ["Bash"]},
                "/untrusted": {"hasTrustDialogAccepted": false}
            }
        });
        let mut state =
            json!({"oauthAccount": {"emailAddress": "own@example.com"}, "theme": "dark"});

        assert!(seed(&hub, &mut state));
        assert_eq!(state["theme"], "dark", "the account's own answer stays");
        assert_eq!(state["hasCompletedOnboarding"], true);
        assert_eq!(state["hasCompletedClaudeInChromeOnboarding"], true);
        assert!(
            state.get("groveChoiceSeenAt").is_none(),
            "per-account keys stay out"
        );
        assert_eq!(state["oauthAccount"]["emailAddress"], "own@example.com");
        assert_eq!(
            state["projects"]["/trusted"],
            json!({"hasTrustDialogAccepted": true})
        );
        assert!(state["projects"].get("/untrusted").is_none());

        assert!(!seed(&hub, &mut state));
    }

    #[test]
    fn reads_the_sign_in() {
        let state = json!({"oauthAccount": {
            "emailAddress": "a@b.c",
            "organizationName": "Org",
            "organizationUuid": "u",
            "organizationType": "claude_max"
        }});
        assert_eq!(sign_in(&state).unwrap().org_type, "claude_max");
        assert!(sign_in(&json!({})).is_none());
    }

    #[test]
    fn names_the_keychain_item_by_the_directory_hash() {
        assert_eq!(
            keychain_service(Path::new("/x"), true),
            "Claude Code-credentials"
        );
        // Claude Code 2.1.322 names the item of a login with this formula.
        assert_eq!(
            keychain_service(Path::new("/Users/me/.claude-accounts/work"), false),
            "Claude Code-credentials-1082021d"
        );
    }
}
