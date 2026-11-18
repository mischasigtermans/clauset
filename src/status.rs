use std::ffi::OsStr;
use std::path::Path;

use chrono::{DateTime, Local, TimeZone};
use console::{Style, style};
use serde_json::{Value, json};

use crate::config::{Choice, Config};
use crate::usage::{self, Window};
use crate::{state, switch};

pub fn print(config: &Config, cwd: &Path, config_dir: Option<&OsStr>, marked: bool) {
    let now = Local::now();
    let choice = config.resolve(config_dir, marked);
    let here = choice.account();
    let why = match &choice {
        Choice::Foreign(dir) => format!(
            "CLAUDE_CONFIG_DIR={} is no clauset account",
            config.tilde(dir)
        ),
        Choice::Inherited(_) => "inherited from the session that started it".into(),
        Choice::Default(_) => "the default".into(),
    };
    match here {
        Some(name) => println!(
            "{} runs on {}  {}",
            config.tilde(cwd),
            style(name).bold(),
            style(format!("({why})")).dim()
        ),
        None => println!(
            "{} runs claude unchanged  {}",
            config.tilde(cwd),
            style(format!("({why})")).dim()
        ),
    }
    println!();

    let width = config
        .accounts
        .keys()
        .map(|name| name.len())
        .max()
        .unwrap_or(0)
        + 2;
    for (name, account) in &config.accounts {
        let arrow = if Some(name.as_str()) == here {
            "→"
        } else {
            " "
        };
        let default = if name == &config.default { "*" } else { " " };
        let sign_in = state::read(&config.state_file(account))
            .ok()
            .as_ref()
            .and_then(state::sign_in);
        let plan = sign_in.as_ref().map_or("", |s| s.plan());
        let head = format!(
            "{arrow} {}{default} {plan:<6}",
            style(format!("{name:<width$}")).bold()
        );

        let Some(sign_in) = sign_in else {
            println!(
                "{head} {}",
                style(format!("not signed in: clauset login {name}")).red()
            );
            continue;
        };
        match usage::latest(config, name) {
            Some(limits) => {
                println!(
                    "{head} {}",
                    windows(limits.five_hour, limits.seven_day, now)
                );
                println!(
                    "{:indent$}{}",
                    "",
                    style(format!(
                        "{}, {} | usage {}{}{}",
                        sign_in.email,
                        sign_in.org(),
                        age(limits.at_ms, now),
                        models(config, name, now),
                        config
                            .warn_at(name)
                            .map(|percent| format!(" | warns at {percent:.0}%"))
                            .unwrap_or_default()
                    ))
                    .dim(),
                    indent = width + 11
                );
            }
            None => {
                println!(
                    "{head} {}",
                    style("no usage seen yet: it shows after a session on this account").dim()
                );
                println!(
                    "{:indent$}{}",
                    "",
                    style(format!("{}, {}", sign_in.email, sign_in.org())).dim(),
                    indent = width + 11
                );
            }
        }
    }
    println!();
    println!(
        "{}",
        style("→ here   * default   clauset use <account> makes it the default").dim()
    );
}

/// The overview for a program to read, such as a menu bar app. A window's
/// percent is already 0 once its reset has passed.
pub fn json(config: &Config, config_dir: Option<&OsStr>, marked: bool) -> Value {
    let now = Local::now();
    let sessions = switch::sessions_per_account(config);
    let window = |window: Option<Window>| {
        window.map(|window| {
            let (percent, passed) = current(window, now);
            json!({"percent": percent, "resets_at": window.resets_at, "reset_passed": passed})
        })
    };
    let accounts: Vec<Value> = config
        .accounts
        .iter()
        .map(|(name, account)| {
            let sign_in = state::read(&config.state_file(account))
                .ok()
                .as_ref()
                .and_then(state::sign_in);
            json!({
                "name": name,
                "dir": config.tilde(&account.dir),
                "default": name == &config.default,
                "hub": config.is_hub(account),
                "signed_in": sign_in.is_some(),
                "email": sign_in.as_ref().map(|s| s.email.as_str()),
                "org": sign_in.as_ref().map(|s| s.org()),
                "plan": sign_in.as_ref().map(|s| s.plan()),
                "warn_at": config.warn_at(name),
                "own_warn_at": account.warn_at,
                "sessions": sessions.get(name).copied().unwrap_or(0),
                "usage": usage::latest(config, name).map(|limits| json!({
                    "at_ms": limits.at_ms,
                    "five_hour": window(limits.five_hour),
                    "seven_day": window(limits.seven_day),
                })),
                "models": usage::models(config, name)
                    .into_iter()
                    .map(|model| {
                        let (percent, passed) = current(model.window, now);
                        json!({
                            "name": model.name,
                            "at_ms": model.at_ms,
                            "percent": percent,
                            "resets_at": model.window.resets_at,
                            "reset_passed": passed,
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "version": 1,
        "here": config.resolve(config_dir, marked).account(),
        "default": config.default,
        "warn_at": config.warn_at,
        "chrome": config.chrome,
        "accounts": accounts,
    })
}

/// A window's percent as it stands now, and whether its reset has passed.
fn current(window: Window, now: DateTime<Local>) -> (f64, bool) {
    let passed = window.resets_at.is_some_and(|at| at <= now.timestamp());
    (if passed { 0.0 } else { window.percent }, passed)
}

/// A window Claude Code does not report, as with a plan that has no 5-hour
/// limit, is left out.
fn windows(five_hour: Option<Window>, seven_day: Option<Window>, now: DateTime<Local>) -> String {
    [("5h", five_hour), ("7d", seven_day)]
        .into_iter()
        .filter_map(|(label, limit)| limit.map(|limit| window(label, limit, now)))
        .collect::<Vec<_>>()
        .join("   ")
}

fn level(percent: f64) -> Style {
    match percent {
        p if p >= 90.0 => Style::new().red(),
        p if p >= 70.0 => Style::new().yellow(),
        _ => Style::new().blue(),
    }
}

fn window(label: &str, window: Window, now: DateTime<Local>) -> String {
    let (percent, passed) = current(window, now);
    let reset = window
        .resets_at
        .and_then(|at| Local.timestamp_opt(at, 0).single());
    let note = match reset {
        _ if passed => style("reset since").dim().to_string(),
        Some(reset) => style(format!("resets {}", when(reset, now)))
            .dim()
            .to_string(),
        None => String::new(),
    };
    format!(
        "{label} {} {:>3.0}%  {note}",
        level(percent).apply_to(bar(percent)),
        percent
    )
}

fn bar(percent: f64) -> String {
    let filled = ((percent / 10.0).round() as usize).min(10);
    format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
}

pub fn when(at: DateTime<Local>, now: DateTime<Local>) -> String {
    if at.date_naive() == now.date_naive() {
        at.format("%H:%M").to_string()
    } else if at - now < chrono::Duration::days(7) {
        at.format("%a %H:%M").to_string()
    } else {
        at.format("%-d %b").to_string()
    }
}

fn models(config: &Config, name: &str, now: DateTime<Local>) -> String {
    usage::models(config, name)
        .into_iter()
        .map(|model| {
            let (percent, passed) = current(model.window, now);
            if passed {
                format!(" | {} 7d reset since", model.name)
            } else {
                format!(
                    " | {} 7d {percent:.0}% ({})",
                    model.name,
                    age(model.at_ms, now)
                )
            }
        })
        .collect()
}

fn age(at_ms: i64, now: DateTime<Local>) -> String {
    let seconds = (now.timestamp_millis() - at_ms).max(0) / 1000;
    match seconds {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_a_bar_of_ten() {
        assert_eq!(bar(0.0), "░░░░░░░░░░");
        assert_eq!(bar(35.0), "████░░░░░░");
        assert_eq!(bar(140.0), "██████████");
    }

    #[test]
    fn colours_a_bar_by_how_full_it_is() {
        let painted = |percent: f64| level(percent).force_styling(true).apply_to("x").to_string();
        assert!(painted(0.0).contains("\u{1b}[34m"), "blue below 70");
        assert!(painted(69.0).contains("\u{1b}[34m"));
        assert!(painted(70.0).contains("\u{1b}[33m"), "yellow from 70");
        assert!(painted(89.0).contains("\u{1b}[33m"));
        assert!(painted(90.0).contains("\u{1b}[31m"), "red from 90");
    }

    #[test]
    fn leaves_out_a_window_claude_code_does_not_report() {
        let now = Local.timestamp_opt(1_800_000_000, 0).unwrap();
        let seven_day = Some(Window {
            percent: 40.0,
            resets_at: None,
        });
        let line = windows(None, seven_day, now);
        assert!(line.starts_with("7d "), "{line}");
        assert!(!line.contains("5h"), "{line}");
        let both = windows(seven_day, seven_day, now);
        assert!(both.starts_with("5h ") && both.contains("   7d "), "{both}");
    }

    #[test]
    fn says_how_old_the_numbers_are() {
        let now = Local.timestamp_opt(1_800_000_000, 0).unwrap();
        let ago = |seconds: i64| age((1_800_000_000 - seconds) * 1000, now);
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(600), "10 min ago");
        assert_eq!(ago(7200), "2 h ago");
        assert_eq!(ago(200_000), "2 d ago");
    }
}
