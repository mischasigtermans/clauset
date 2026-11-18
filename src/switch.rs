//! A Claude Code process keeps the config dir it started with, so a session
//! moves by ending once its turn is over and resuming on the other account,
//! in the same terminal, under the supervisor that started it.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, Session};
use crate::state;

/// Set on the `claude` a supervisor runs: the supervisor's pid.
pub const SUPERVISOR: &str = "CLAUSET_SUPERVISOR";

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub account: String,
    pub session_id: String,
}

fn request_path(config: &Config, pid: u32) -> Option<PathBuf> {
    Some(
        Config::path(&config.home)
            .parent()?
            .join("switch")
            .join(format!("{pid}.json")),
    )
}

fn parent_of(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()
        .filter(|ppid| *ppid > 1)
}

/// The child of `claude` this process runs under.
fn own_branch(claude: u32) -> Option<u32> {
    let mut pid = std::process::id();
    loop {
        let parent = parent_of(pid)?;
        if parent == claude {
            return Some(pid);
        }
        pid = parent;
    }
}

/// How many Bash tool shells `claude` runs besides `except`. Claude Code
/// starts each one from a shell snapshot in its config dir.
fn tool_shells(claude: u32, except: Option<u32>) -> usize {
    let Ok(output) = Command::new("ps")
        .args(["-A", "-ww", "-o", "pid=,ppid=,command="])
        .output()
    else {
        return 0;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            let mut fields = line
                .split_whitespace()
                .map(|field| field.parse::<u32>().ok());
            let (pid, ppid) = (fields.next().flatten(), fields.next().flatten());
            ppid == Some(claude) && pid != except && line.contains("/shell-snapshots/snapshot-")
        })
        .count()
}

fn registry_entry(hub: &Path, pid: u32) -> Option<Value> {
    state::read(&hub.join("sessions").join(format!("{pid}.json"))).ok()
}

/// What a supervisor notes about the `claude` it runs, so `switch --all`
/// can tell the account of another session: macOS keeps a process's
/// environment from other processes.
#[derive(Serialize, Deserialize)]
struct Supervised {
    account: String,
    supervisor: u32,
}

fn note_path(config: &Config, child: u32) -> Option<PathBuf> {
    Some(
        Config::path(&config.home)
            .parent()?
            .join("supervised")
            .join(format!("{child}.json")),
    )
}

pub fn note(config: &Config, child: u32, account: &str) -> Result<()> {
    let path = note_path(config, child).context("the config path has no directory")?;
    fs::create_dir_all(path.parent().expect("joined above"))?;
    let note = Supervised {
        account: account.to_owned(),
        supervisor: std::process::id(),
    };
    state::write(&path, &serde_json::to_value(&note)?)
}

pub fn forget(config: &Config, child: u32) {
    if let Some(path) = note_path(config, child) {
        let _ = fs::remove_file(path);
    }
}

fn supervised_on(config: &Config, pid: u32) -> Option<String> {
    let path = note_path(config, pid)?;
    let note: Supervised = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    (parent_of(pid) == Some(note.supervisor)).then_some(note.account)
}

/// When `pid` started, as Claude Code writes it in its registry entry: in
/// UTC, and in the C locale or the one inherited, which `ps` formats it in.
fn started(pid: u32, c_locale: bool) -> Option<String> {
    let mut ps = Command::new("ps");
    if c_locale {
        ps.env("LC_ALL", "C");
    }
    let output = ps
        .env("TZ", "UTC")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

struct Running {
    pid: u32,
    session_id: String,
    name: Option<String>,
    cwd: Option<String>,
    idle: bool,
    /// The account its supervisor runs it on; `None` without one.
    account: Option<String>,
}

/// An entry whose pid now belongs to a process started at another time is
/// left out.
fn running(config: &Config) -> Vec<Running> {
    let Ok(entries) = fs::read_dir(config.hub().join("sessions")) else {
        return Vec::new();
    };
    let mut sessions: Vec<Running> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension() != Some(OsStr::new("json")) {
                return None;
            }
            let pid: u32 = path.file_stem()?.to_str()?.parse().ok()?;
            let entry = state::read(&path).ok()?;
            let text = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);
            if let Some(start) = text("procStart") {
                let start = start.split_whitespace().collect::<Vec<_>>().join(" ");
                if ![true, false]
                    .into_iter()
                    .any(|c_locale| started(pid, c_locale).as_ref() == Some(&start))
                {
                    return None;
                }
            }
            if text("entrypoint").is_some_and(|entrypoint| entrypoint != "cli") {
                return None;
            }
            Some(Running {
                pid,
                session_id: text("sessionId")?,
                name: text("name"),
                cwd: text("cwd"),
                idle: text("status").as_deref() == Some("idle"),
                account: supervised_on(config, pid),
            })
        })
        .collect();
    sessions.sort_by_key(|session| session.pid);
    sessions
}

/// How many sessions each account's supervisors run: the ones `switch --all`
/// can move.
pub fn sessions_per_account(config: &Config) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for account in running(config)
        .into_iter()
        .filter_map(|session| session.account)
    {
        *counts.entry(account).or_default() += 1;
    }
    counts
}

/// How many sessions run on the hub, named `hub`: the ones its supervisors
/// run, and every session without a supervisor. clauset can't tell the
/// account of those, and the hub is where a `claude` started outside it runs.
pub fn sessions_on_hub(config: &Config, hub: &str) -> usize {
    running(config)
        .into_iter()
        .filter(|session| session.account.as_deref().is_none_or(|on| on == hub))
        .count()
}

/// What to say about new sessions after a move away from `leaving`. A move
/// touches running sessions alone, so leaving the default account leaves
/// new sessions starting there: `use` is what changes that.
fn new_sessions(default: &str, leaving: &str, account: &str) -> Option<String> {
    (default == leaving).then(|| {
        format!("New sessions still start on {leaving}: `clauset use {account}` changes that.")
    })
}

pub fn request(config: &Config, account: &str) -> Result<String> {
    config.account(account)?;
    // Claude Code gives the processes its tools start its own pid and session.
    let claude = std::env::var("CLAUDE_PID")
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok());
    let session_id = std::env::var("CLAUDE_CODE_SESSION_ID").ok();
    let (Some(claude), Some(session_id)) = (claude, session_id) else {
        bail!("run it inside a Claude Code session: `! clauset switch <account>`");
    };
    let session = config.session(std::env::var_os("CLAUDE_CONFIG_DIR").as_deref());
    let leaving = match session {
        Session::Account(name) => Some(name),
        _ => None,
    };
    if leaving == Some(account) {
        let cancelled =
            request_path(config, claude).is_some_and(|path| fs::remove_file(path).is_ok());
        return Ok(if cancelled {
            format!("This session already runs on {account}; the move it asked for is cancelled.")
        } else {
            format!("This session already runs on {account}.")
        });
    }
    let supervised = std::env::var(SUPERVISOR)
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok())
        .is_some_and(|supervisor| parent_of(claude) == Some(supervisor));
    if !supervised {
        bail!(
            "This session runs without clauset's supervisor, so it cannot move in place: end it and run `clauset run {account} --resume {session_id}`"
        );
    }
    let path = request_path(config, claude).context("the config path has no directory")?;
    fs::create_dir_all(path.parent().expect("joined above"))?;
    let request = Request {
        account: account.to_owned(),
        session_id,
    };
    state::write(&path, &serde_json::to_value(&request)?)?;
    // Background shells would end with the session, so the supervisor waits
    // for them; background agents and teammates run inside it, out of sight.
    let when = match tool_shells(claude, own_branch(claude)) {
        0 => "once the current turn ends".to_owned(),
        1 => "once the current turn ends and its background shell has stopped (`/tasks` stops it)"
            .to_owned(),
        shells => format!(
            "once the current turn ends and its {shells} background shells have stopped (`/tasks` stops them)"
        ),
    };
    let cancel = leaving
        .map(|name| format!(" `clauset switch {name}` cancels the move."))
        .unwrap_or_default();
    let new_sessions = leaving
        .and_then(|name| new_sessions(&config.default, name, account))
        .map(|line| format!(" {line}"))
        .unwrap_or_default();
    Ok(format!(
        "This session moves to {account} {when}: it closes here and resumes on {account} in this terminal.{cancel} Background agents and teammates of this session end with it. The first turn there reads the whole conversation again, without the thinking from the other organization.{new_sessions}"
    ))
}

/// The account being left is `from`, else the one the enclosing session runs
/// on. A session waiting to move gets its request written again, so a second
/// call changes nothing but the target.
pub fn request_all(config: &Config, account: &str, from: Option<&str>) -> Result<String> {
    config.account(account)?;
    let here = std::env::var("CLAUDE_PID")
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok());
    let leaving = match from {
        Some(from) => {
            config.account(from)?;
            from
        }
        None => match config.session(std::env::var_os("CLAUDE_CONFIG_DIR").as_deref()) {
            Session::Account(name) if here.is_some() => name,
            _ => bail!(
                "name the account to move the sessions from: `clauset switch {account} --all --from <account>`"
            ),
        },
    };
    if leaving == account {
        bail!("the sessions on {account} already run there");
    }
    let (moving, unknown): (Vec<_>, Vec<_>) = running(config)
        .into_iter()
        .filter(|session| session.account.as_deref().is_none_or(|on| on == leaving))
        .partition(|session| session.account.is_some());
    if moving.is_empty() && unknown.is_empty() {
        return Ok(format!(
            "No session runs on {leaving}. New sessions start on {}.",
            config.default
        ));
    }
    let label = |session: &Running| {
        let cwd =
            session
                .cwd
                .as_deref()
                .map(|cwd| match Path::new(cwd).strip_prefix(&config.home) {
                    Ok(rest) => format!("~/{}", rest.display()),
                    Err(_) => cwd.to_owned(),
                });
        let mut label = match (&session.name, cwd) {
            (Some(name), Some(cwd)) => format!("{name} in {cwd}"),
            (Some(name), None) => name.clone(),
            (None, Some(cwd)) => cwd,
            (None, None) => session.session_id.clone(),
        };
        if Some(session.pid) == here {
            label.push_str(" (this session)");
        }
        label
    };
    let mut lines = Vec::new();
    if !moving.is_empty() {
        lines.push(format!(
            "These move from {leaving} to {account}, each closing and resuming in its own terminal once its turn ends and its background shells have stopped:"
        ));
        for session in &moving {
            let path =
                request_path(config, session.pid).context("the config path has no directory")?;
            fs::create_dir_all(path.parent().expect("joined above"))?;
            let request = Request {
                account: account.to_owned(),
                session_id: session.session_id.clone(),
            };
            state::write(&path, &serde_json::to_value(&request)?)?;
            let except = (Some(session.pid) == here)
                .then(|| own_branch(session.pid))
                .flatten();
            let busy = if session.idle { "" } else { "busy, then " };
            let wait = match (session.idle, tool_shells(session.pid, except)) {
                (true, 0) => String::new(),
                (false, 0) => ", busy: it moves when its turn ends".to_owned(),
                (_, 1) => {
                    format!(", {busy}waiting for its background shell (`/tasks` stops it)")
                }
                (_, shells) => format!(
                    ", {busy}waiting for its {shells} background shells (`/tasks` stops them)"
                ),
            };
            lines.push(format!("- {}{wait}", label(session)));
        }
    }
    if !unknown.is_empty() {
        lines.push(format!(
            "These run without clauset's supervisor, so clauset can't tell their account or move them. One on {leaving} moves when you end it and run the command beside it:"
        ));
        for session in &unknown {
            lines.push(format!(
                "- {}: `clauset run {account} --resume {}`",
                label(session),
                session.session_id
            ));
        }
    }
    if moving.is_empty() {
        lines.push(format!(
            "New sessions start on {} as before.",
            config.default
        ));
    } else {
        lines.push(format!(
            "Background agents and teammates of a session end with it. The first turn on {account} reads each conversation again, without the thinking from the other organization."
        ));
        lines.extend(new_sessions(&config.default, leaving, account));
    }
    Ok(lines.join("\n"))
}

/// Ends `child` once a request for it is waiting and its session is idle
/// with no background shell; the thread returns whether it did. The request
/// has to name the session the registry gives for `child`, so a file left
/// from an earlier process with the same pid moves nothing.
pub fn watch(config: &Config, child: u32, done: Arc<AtomicBool>) -> Option<JoinHandle<bool>> {
    let path = request_path(config, child)?;
    let hub = config.hub();
    Some(thread::spawn(move || {
        while !done.load(Ordering::SeqCst) {
            let request = fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Request>(&text).ok());
            let idle = |session_id: &str| {
                registry_entry(&hub, child).is_some_and(|entry| {
                    entry.get("sessionId").and_then(Value::as_str) == Some(session_id)
                        && entry.get("status").and_then(Value::as_str) == Some("idle")
                })
            };
            // Read again after the shell check: a prompt sent while `ps`
            // runs turns the session busy.
            if let Some(request) = request
                && idle(&request.session_id)
                && tool_shells(child, None) == 0
                && idle(&request.session_id)
            {
                // SAFETY: kill(2) with a pid this process spawned and has not reaped.
                unsafe {
                    libc::kill(child as libc::pid_t, libc::SIGTERM);
                }
                return true;
            }
            thread::sleep(Duration::from_millis(500));
        }
        false
    }))
}

pub fn take(config: &Config, child: u32) -> Option<Request> {
    let path = request_path(config, child)?;
    let text = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    serde_json::from_str(&text).ok()
}

/// The arguments a resumed session keeps: the flags that shape a session.
/// A prompt, and every flag that picks or forks a session or runs `-p`, stay
/// behind, so only flags Claude Code 2.1.319 lists are kept.
pub fn carry(args: &[OsString]) -> Vec<OsString> {
    const SWITCHES: &[&str] = &[
        "--dangerously-skip-permissions",
        "--allow-dangerously-skip-permissions",
        "--verbose",
        "--chrome",
        "--no-chrome",
        "--ide",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--bare",
        "--brief",
    ];
    const WITH_VALUE: &[&str] = &[
        "--permission-mode",
        "--model",
        "--fallback-model",
        "--effort",
        "--agent",
        "--agents",
        "--append-system-prompt",
        "--append-system-prompt-file",
        "--system-prompt",
        "--system-prompt-file",
        "--settings",
        "--setting-sources",
        "--plugin-dir",
        "--autocompact",
        "--name",
        "-n",
    ];
    // These take every value up to the next flag, as Claude Code reads them.
    const WITH_VALUES: &[&str] = &[
        "--add-dir",
        "--allowedTools",
        "--allowed-tools",
        "--disallowedTools",
        "--disallowed-tools",
        "--tools",
        "--mcp-config",
        "--betas",
    ];
    let mut kept = Vec::new();
    let mut args = args.iter().peekable();
    while let Some(arg) = args.next() {
        let Some(text) = arg.to_str() else { continue };
        if SWITCHES.contains(&text) {
            kept.push(arg.clone());
        } else if WITH_VALUE.contains(&text) {
            if let Some(value) = args.next() {
                kept.extend([arg.clone(), value.clone()]);
            }
        } else if WITH_VALUES.contains(&text) {
            kept.push(arg.clone());
            while let Some(value) = args.next_if(|value| !value.to_string_lossy().starts_with('-'))
            {
                kept.push(value.clone());
            }
        } else if let Some((flag, _)) = text.split_once('=')
            && (WITH_VALUE.contains(&flag) || WITH_VALUES.contains(&flag))
        {
            kept.push(arg.clone());
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn says_where_new_sessions_start_only_when_the_default_is_left() {
        assert_eq!(
            new_sessions("work", "work", "personal").as_deref(),
            Some("New sessions still start on work: `clauset use personal` changes that.")
        );
        assert_eq!(new_sessions("client", "work", "personal"), None);
    }

    #[test]
    fn keeps_the_flags_that_shape_a_session() {
        let kept = carry(&args(&[
            "--add-dir",
            "/a",
            "/b",
            "--settings",
            "s.json",
            "--append-system-prompt",
            "be brief",
            "-n",
            "build",
            "--chrome",
            "--session-id",
            "u-1",
            "--fork-session",
            "--allowed-tools=Bash",
        ]));
        assert_eq!(
            kept,
            args(&[
                "--add-dir",
                "/a",
                "/b",
                "--settings",
                "s.json",
                "--append-system-prompt",
                "be brief",
                "-n",
                "build",
                "--chrome",
                "--allowed-tools=Bash",
            ])
        );
    }

    #[test]
    fn leaves_the_prompt_and_the_session_behind() {
        let kept = carry(&args(&[
            "--dangerously-skip-permissions",
            "--model",
            "opus",
            "--resume",
            "abc",
            "-c",
            "--effort=high",
            "-p",
            "fix the tests",
        ]));
        assert_eq!(
            kept,
            args(&[
                "--dangerously-skip-permissions",
                "--model",
                "opus",
                "--effort=high"
            ])
        );
    }
}
