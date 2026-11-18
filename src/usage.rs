//! clauset asks Anthropic for nothing: the freshest of an account's
//! `cachedUsageUtilization` and the last status line seen on it wins.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::state;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub percent: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub at_ms: i64,
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
}

impl Limits {
    fn new(at_ms: i64, five_hour: Option<Window>, seven_day: Option<Window>) -> Option<Self> {
        (five_hour.is_some() || seven_day.is_some()).then_some(Limits {
            at_ms,
            five_hour,
            seven_day,
        })
    }
}

/// `cachedUsageUtilization`, which Claude Code writes to an account's
/// `.claude.json` each time it fetches that account's usage.
pub fn from_state(state: &Value) -> Option<Limits> {
    let cached = state.get("cachedUsageUtilization")?;
    let utilization = cached.get("utilization")?;
    let window = |key: &str| {
        let window = utilization.get(key)?;
        Some(Window {
            percent: window.get("utilization")?.as_f64()?,
            resets_at: window
                .get("resets_at")
                .and_then(Value::as_str)
                .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                .map(|time| time.timestamp()),
        })
    };
    Limits::new(
        cached.get("fetchedAtMs")?.as_i64()?,
        window("five_hour"),
        window("seven_day"),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelLimit {
    /// The model's name as Claude Code shows it, such as `Fable`.
    pub name: String,
    pub at_ms: i64,
    pub window: Window,
}

/// The weekly limits per model in `cachedUsageUtilization`. A status line
/// gets no such limit, so these are as old as Claude Code's last fetch.
pub fn models_from_state(state: &Value) -> Vec<ModelLimit> {
    let Some(cached) = state.get("cachedUsageUtilization") else {
        return Vec::new();
    };
    let Some(at_ms) = cached.get("fetchedAtMs").and_then(Value::as_i64) else {
        return Vec::new();
    };
    cached
        .pointer("/utilization/limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|limit| limit.get("kind").and_then(Value::as_str) == Some("weekly_scoped"))
        .filter_map(|limit| {
            Some(ModelLimit {
                name: limit
                    .pointer("/scope/model/display_name")?
                    .as_str()?
                    .to_owned(),
                at_ms,
                window: Window {
                    percent: limit.get("percent")?.as_f64()?,
                    resets_at: limit
                        .get("resets_at")
                        .and_then(Value::as_str)
                        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                        .map(|time| time.timestamp()),
                },
            })
        })
        .collect()
}

pub fn models(config: &Config, account: &str) -> Vec<ModelLimit> {
    config
        .account(account)
        .ok()
        .and_then(|acct| state::read(&config.state_file(acct)).ok())
        .map(|state| models_from_state(&state))
        .unwrap_or_default()
}

/// The `rate_limits` Claude Code hands a status line command. It sends them
/// for subscription plans, Team included, and leaves out a window the API
/// does not report.
pub fn from_statusline(input: &Value, now_ms: i64) -> Option<Limits> {
    let rate = input.get("rate_limits")?;
    let window = |key: &str| {
        let window = rate.get(key)?;
        Some(Window {
            percent: window.get("used_percentage")?.as_f64()?,
            resets_at: window
                .get("resets_at")
                .and_then(|value| value.as_i64().or_else(|| value.as_f64().map(|f| f as i64))),
        })
    };
    Limits::new(now_ms, window("five_hour"), window("seven_day"))
}

fn recorded_path(config: &Config, account: &str) -> Option<PathBuf> {
    Some(
        Config::path(&config.home)
            .parent()?
            .join("usage")
            .join(format!("{account}.json")),
    )
}

fn read_recorded(path: &Path) -> Option<Limits> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Keeps what a status line saw, rewriting the file when the numbers move or
/// a minute has passed, not on every refresh.
pub fn record(config: &Config, account: &str, limits: &Limits) -> Result<()> {
    let Some(path) = recorded_path(config, account) else {
        return Ok(());
    };
    if let Some(old) = read_recorded(&path)
        && old.five_hour == limits.five_hour
        && old.seven_day == limits.seven_day
        && limits.at_ms - old.at_ms < 60_000
    {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    state::write(&path, &serde_json::to_value(limits)?)
}

pub fn forget(config: &Config, account: &str) {
    if let Some(path) = recorded_path(config, account) {
        let _ = fs::remove_file(path.with_extension("warned.json"));
        let _ = fs::remove_file(path);
    }
}

/// The `resets_at` of each window already warned about.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Warned {
    five_hour: Option<i64>,
    seven_day: Option<i64>,
}

/// The windows that reached `threshold` since the last warning, each once
/// per window: a window that resets may warn again.
pub fn newly_crossed(
    config: &Config,
    account: &str,
    limits: &Limits,
    threshold: f64,
) -> Vec<(&'static str, Window)> {
    let Some(path) = recorded_path(config, account).map(|path| path.with_extension("warned.json"))
    else {
        return Vec::new();
    };
    let mut warned: Warned = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut crossed = Vec::new();
    for (label, window, slot) in [
        ("5-hour", limits.five_hour, &mut warned.five_hour),
        ("7-day", limits.seven_day, &mut warned.seven_day),
    ] {
        if let Some(window) = window
            && window.percent >= threshold
            && *slot != Some(window.resets_at.unwrap_or(0))
        {
            *slot = Some(window.resets_at.unwrap_or(0));
            crossed.push((label, window));
        }
    }
    if !crossed.is_empty()
        && let Ok(value) = serde_json::to_value(&warned)
    {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = state::write(&path, &value);
    }
    crossed
}

pub fn latest(config: &Config, account: &str) -> Option<Limits> {
    let from_cache = config
        .account(account)
        .ok()
        .and_then(|acct| state::read(&config.state_file(acct)).ok())
        .and_then(|state| from_state(&state));
    let recorded = recorded_path(config, account).and_then(|path| read_recorded(&path));
    [from_cache, recorded]
        .into_iter()
        .flatten()
        .max_by_key(|limits| limits.at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_claude_codes_cache() {
        let state = json!({"cachedUsageUtilization": {
            "fetchedAtMs": 1793864219200_i64,
            "utilization": {
                "five_hour": {"utilization": 24, "resets_at": "2026-11-05T12:30:00.121412+00:00"},
                "seven_day": {"utilization": 35.5, "resets_at": "2026-11-08T09:00:00+00:00"},
                "seven_day_opus": null
            }
        }});
        let limits = from_state(&state).unwrap();
        assert_eq!(limits.at_ms, 1793864219200);
        assert_eq!(limits.five_hour.unwrap().percent, 24.0);
        assert_eq!(limits.five_hour.unwrap().resets_at, Some(1793881800));
        assert_eq!(limits.seven_day.unwrap().percent, 35.5);
        assert!(from_state(&json!({})).is_none());
    }

    #[test]
    fn reads_the_weekly_limit_of_a_model() {
        let state = json!({"cachedUsageUtilization": {
            "fetchedAtMs": 7,
            "utilization": {"limits": [
                {"kind": "weekly_all", "percent": 42, "resets_at": "2026-11-08T09:00:00+00:00", "scope": null},
                {"kind": "weekly_scoped", "percent": 30, "resets_at": "2026-11-08T09:00:00+00:00",
                 "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null}},
                {"kind": "weekly_scoped", "percent": 5, "resets_at": null,
                 "scope": {"model": null, "surface": "cowork"}}
            ]}
        }});
        assert_eq!(
            models_from_state(&state),
            vec![ModelLimit {
                name: "Fable".into(),
                at_ms: 7,
                window: Window {
                    percent: 30.0,
                    resets_at: Some(1794128400),
                },
            }]
        );
        assert!(models_from_state(&json!({})).is_empty());
    }

    #[test]
    fn reads_the_status_line_input() {
        let input = json!({"rate_limits": {
            "five_hour": {"used_percentage": 12.5, "resets_at": 1793881800},
            "seven_day": {"used_percentage": 40, "resets_at": 1794142800.0}
        }});
        let limits = from_statusline(&input, 5).unwrap();
        assert_eq!(limits.at_ms, 5);
        assert_eq!(limits.seven_day.unwrap().resets_at, Some(1794142800));
        assert!(from_statusline(&json!({"model": {}}), 5).is_none());
    }

    #[test]
    fn warns_once_per_window() {
        let home = tempfile::tempdir().unwrap();
        let config = Config::parse(
            "default = \"a\"\nwarn_at = 80\n[accounts.a]\ndir = \"/a\"\n",
            home.path(),
        )
        .unwrap();
        let at = |percent: f64, resets_at: i64| Limits {
            at_ms: 0,
            five_hour: Some(Window {
                percent,
                resets_at: Some(resets_at),
            }),
            seven_day: None,
        };
        assert_eq!(config.warn_at("a"), Some(80.0));
        assert!(newly_crossed(&config, "a", &at(79.0, 100), 80.0).is_empty());
        assert_eq!(newly_crossed(&config, "a", &at(81.0, 100), 80.0).len(), 1);
        assert!(newly_crossed(&config, "a", &at(95.0, 100), 80.0).is_empty());
        assert_eq!(newly_crossed(&config, "a", &at(85.0, 200), 80.0).len(), 1);
    }
}
