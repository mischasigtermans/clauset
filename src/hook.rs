//! A hook that cannot decide prints nothing: Claude Code then goes on as if
//! no hook ran.

use chrono::{DateTime, Local, TimeZone};
use serde_json::{Value, json};
use std::ffi::OsStr;

use crate::config::{Config, Session};
use crate::notify;
use crate::usage::{self, Window};

/// `SessionStart`: tells the model which account it runs on.
pub fn context(config: &Config, config_dir: Option<&OsStr>) -> Option<String> {
    let Session::Account(session) = config.session(config_dir) else {
        return None;
    };
    let text = format!("This Claude Code session runs on the {session} account (clauset).");
    Some(
        json!({"hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": text,
        }})
        .to_string(),
    )
}

/// `StopFailure` on `rate_limit`. Choosing where to go on is left to the
/// person: clauset never switches an account on a limit.
pub fn limit(
    config: &Config,
    input: &str,
    config_dir: Option<&OsStr>,
    now: DateTime<Local>,
) -> Option<String> {
    let input: Value = serde_json::from_str(input).ok()?;
    if input.get("error").and_then(Value::as_str) != Some("rate_limit") {
        return None;
    }
    let Session::Account(session) = config.session(config_dir) else {
        return None;
    };
    let mut text = format!("{} hit its usage limit", notify::title(session));
    let fullest = usage::latest(config, session).and_then(|limits| {
        [limits.five_hour, limits.seven_day]
            .into_iter()
            .flatten()
            .max_by(|a, b| a.percent.total_cmp(&b.percent))
    });
    if let Some(reset) = fullest
        .and_then(|window| window.resets_at)
        .and_then(|at| Local.timestamp_opt(at, 0).single())
        .filter(|reset| *reset > now)
    {
        text.push_str(&format!(", resets {}", crate::status::when(reset, now)));
    }
    let others: Vec<String> = config
        .accounts
        .keys()
        .filter(|name| name.as_str() != session)
        .map(|name| {
            let fullest = usage::latest(config, name).and_then(|limits| {
                [("5h", limits.five_hour), ("7d", limits.seven_day)]
                    .into_iter()
                    .filter_map(|(label, window)| Some((label, current(window?, now))))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
            });
            let name = notify::title(name);
            match fullest {
                Some((label, percent)) => format!("{name} {percent:.0}% {label}"),
                None => format!("{name} no usage seen"),
            }
        })
        .collect();
    if !others.is_empty() {
        text.push_str(&format!(". Elsewhere: {}", others.join(", ")));
    }
    Some(text)
}

fn current(window: Window, now: DateTime<Local>) -> f64 {
    match window.resets_at {
        Some(at) if at <= now.timestamp() => 0.0,
        _ => window.percent,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const HOME: &str = "/nonexistent/home";

    fn config() -> Config {
        Config::parse(
            r#"
            default = "personal"
            [accounts.personal]
            dir = "~/.claude-accounts/personal"
            [accounts.team]
            dir = "~/.claude"
            "#,
            Path::new(HOME),
        )
        .unwrap()
    }

    #[test]
    fn names_the_account() {
        let config = config();
        let personal = format!("{HOME}/.claude-accounts/personal");
        let out = context(&config, Some(OsStr::new(&personal))).unwrap();
        assert!(out.contains("runs on the personal account"));
        let out = context(&config, None).unwrap();
        assert!(out.contains("runs on the team account"));
    }

    #[test]
    fn names_the_account_that_hit_its_limit() {
        let config = config();
        let now = Local.timestamp_opt(1_800_000_000, 0).unwrap();
        let input = json!({"cwd": HOME, "error": "rate_limit"}).to_string();
        let personal = format!("{HOME}/.claude-accounts/personal");
        let text = limit(&config, &input, Some(OsStr::new(&personal)), now).unwrap();
        assert!(text.starts_with("Personal hit its usage limit"), "{text}");
        assert!(text.contains("Elsewhere: Team no usage seen"), "{text}");

        let overloaded = json!({"cwd": HOME, "error": "overloaded"}).to_string();
        assert!(limit(&config, &overloaded, Some(OsStr::new(&personal)), now).is_none());
    }
}
