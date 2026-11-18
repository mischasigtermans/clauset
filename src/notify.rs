//! Desktop notifications. While the menu bar app runs it posts them, under
//! its own name; otherwise AppleScript does.

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

pub fn send(config: &Config, text: &str) {
    if app_runs(config) && leave(config, text) {
        return;
    }
    // The text goes in as an argument, so it needs no quoting. It follows
    // `--`, or a text starting with `-e` would be read as a line of script.
    let _ = Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 1 of argv) with title \"clauset\"",
            "-e",
            "end run",
            "--",
            text,
        ])
        .status();
}

/// An account's name as a notification says it, and as the menu bar app
/// shows it: `work-1` reads `Work 1`.
pub fn title(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut letters = part.chars();
            letters
                .next()
                .map(|first| first.to_uppercase().chain(letters).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A notification left for an app that isn't running would wait there unseen.
fn app_runs(config: &Config) -> bool {
    let Some(dir) = Config::path(&config.home).parent().map(ToOwned::to_owned) else {
        return false;
    };
    fs::read_to_string(dir.join("app.pid"))
        .ok()
        .and_then(|text| text.trim().parse::<libc::pid_t>().ok())
        // SAFETY: signal 0 sends nothing; it only checks the process exists.
        .is_some_and(|pid| pid > 0 && unsafe { libc::kill(pid, 0) } == 0)
}

/// A file and not a URL the app opens: a URL scheme answers every web page,
/// and this directory its owner alone. The file gets its name only once it
/// is whole, so the app never reads half a text.
fn leave(config: &Config, text: &str) -> bool {
    let Some(dir) = Config::path(&config.home)
        .parent()
        .map(|dir| dir.join("notify"))
    else {
        return false;
    };
    if fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .is_err()
    {
        return false;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let name = format!("{stamp}-{}", std::process::id());
    let unfinished = dir.join(format!(".{name}"));
    fs::write(&unfinished, text)
        .and_then(|()| fs::rename(&unfinished, dir.join(format!("{name}.txt"))))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_an_account_name() {
        assert_eq!(title("personal"), "Personal");
        assert_eq!(title("work-1"), "Work 1");
        assert_eq!(title("old_work"), "Old Work");
    }
}
