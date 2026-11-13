//! The overview `clauset` prints: the account here, then every account with
//! its plan, sign-in and use of its limits.

use std::ffi::OsStr;
use std::path::Path;

use chrono::{DateTime, Local, TimeZone};
use console::{Style, style};

use crate::config::{Choice, Config};
use crate::state;
use crate::usage::{self, Limits, Window};

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
                    "{head} {}   {}",
                    window("5h", limits.five_hour, now),
                    window("7d", limits.seven_day, now)
                );
                println!(
                    "{:indent$}{}",
                    "",
                    style(format!(
                        "{}, {} | usage {}{}",
                        sign_in.email,
                        sign_in.org(),
                        age(&limits, now),
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

fn window(label: &str, window: Option<Window>, now: DateTime<Local>) -> String {
    let Some(window) = window else {
        return style(format!("no {label} window")).dim().to_string();
    };
    let reset = window
        .resets_at
        .and_then(|at| Local.timestamp_opt(at, 0).single());
    let (percent, note) = match reset {
        Some(reset) if reset <= now => (0.0, style("reset since").dim().to_string()),
        Some(reset) => (
            window.percent,
            style(format!("resets {}", when(reset, now)))
                .dim()
                .to_string(),
        ),
        None => (window.percent, String::new()),
    };
    let color = match percent {
        p if p >= 80.0 => Style::new().red(),
        p if p >= 50.0 => Style::new().yellow(),
        _ => Style::new().green(),
    };
    format!(
        "{label} {} {:>3.0}%  {note}",
        color.apply_to(bar(percent)),
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

fn age(limits: &Limits, now: DateTime<Local>) -> String {
    let seconds = (now.timestamp_millis() - limits.at_ms).max(0) / 1000;
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
    fn says_how_old_the_numbers_are() {
        let now = Local.timestamp_opt(1_800_000_000, 0).unwrap();
        let limits = |ago_s: i64| Limits {
            at_ms: (1_800_000_000 - ago_s) * 1000,
            five_hour: None,
            seven_day: None,
        };
        assert_eq!(age(&limits(5), now), "just now");
        assert_eq!(age(&limits(600), now), "10 min ago");
        assert_eq!(age(&limits(7200), now), "2 h ago");
        assert_eq!(age(&limits(200_000), now), "2 d ago");
    }
}
