use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::IsTerminal;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::config::{Config, MARKER};
use crate::{state, switch};

/// The first `claude` on PATH that is not clauset itself.
pub fn real_claude() -> Result<PathBuf> {
    let me = env::current_exe().and_then(|exe| exe.canonicalize()).ok();
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let candidate = dir.join("claude");
        let Ok(resolved) = candidate.canonicalize() else {
            continue;
        };
        if Some(&resolved) != me.as_ref() && is_executable(&resolved) {
            return Ok(candidate);
        }
    }
    bail!("no `claude` on PATH besides clauset's own shim")
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The `--mcp-config` flags go first and in their `=` form: the option takes
/// several values, and a separate value would swallow a prompt.
pub fn command(
    config: &Config,
    name: &str,
    claude: &Path,
    servers: Option<&Path>,
    chrome: bool,
) -> Result<Command> {
    let account = config.account(name)?;
    let mut command = Command::new(claude);
    command.arg0("claude").env(MARKER, name);
    if config.is_hub(account) {
        command
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CLAUDE_CODE_PLUGIN_CACHE_DIR");
    } else {
        command
            .env("CLAUDE_CONFIG_DIR", &account.dir)
            .env("CLAUDE_CODE_PLUGIN_CACHE_DIR", config.hub().join("plugins"));
    }
    if !config.is_hub(account)
        && let Some(cwd) = servers
        && let Ok(hub_state) = state::read(&config.home.join(".claude.json"))
    {
        let files = [
            user_servers(config, &hub_state),
            project_servers(config, &hub_state, cwd),
        ];
        for file in files.into_iter().flatten() {
            let mut flag = OsString::from("--mcp-config=");
            flag.push(file);
            command.arg(flag);
        }
    }
    if chrome && let Some(file) = chrome_server(config, name, claude) {
        let mut flag = OsString::from("--mcp-config=");
        flag.push(file);
        command.arg("--no-chrome").arg(flag);
    }
    Ok(command)
}

/// The directory whose MCP servers the hub shares with this start: none once
/// the caller asks Claude Code for its own `--mcp-config` alone.
fn shared_servers<'a>(cwd: &'a Path, args: &[OsString]) -> Option<&'a Path> {
    (!args.iter().any(|arg| arg == "--strict-mcp-config")).then_some(cwd)
}

/// Whether the `chrome` account's connection replaces the session's own: not
/// once the caller picks Chrome itself or asks for its own `--mcp-config`
/// alone.
fn shared_chrome(args: &[OsString]) -> bool {
    !args
        .iter()
        .any(|arg| arg == "--chrome" || arg == "--no-chrome" || arg == "--strict-mcp-config")
}

/// Claude Code's own Chrome server, started on the sign-in of the account
/// `chrome` names, for a session on any other account. Claude Code reaches
/// Chrome through a relay that pairs a session with the extensions signed in
/// to the same claude.ai account, so a session on another account finds
/// none. The server runs as `command` runs that account, keeps its flag in
/// sight, where the session's check of its organization's Claude in Chrome
/// policy looks for it, and takes another name than `claude-in-chrome`, which
/// `--mcp-config` refuses for a local server.
fn chrome_server(config: &Config, name: &str, claude: &Path) -> Option<PathBuf> {
    let owner = config.chrome.as_deref().filter(|owner| *owner != name)?;
    let account = config.account(owner).ok()?;
    let mut args: Vec<Value> = if config.is_hub(account) {
        [
            "-u",
            "CLAUDE_CONFIG_DIR",
            "-u",
            "CLAUDE_CODE_PLUGIN_CACHE_DIR",
        ]
        .map(Value::from)
        .into()
    } else {
        vec![
            format!("CLAUDE_CONFIG_DIR={}", account.dir.to_str()?).into(),
            format!(
                "CLAUDE_CODE_PLUGIN_CACHE_DIR={}",
                config.hub().join("plugins").to_str()?
            )
            .into(),
        ]
    };
    args.extend([claude.to_str()?.into(), "--claude-in-chrome-mcp".into()]);
    let servers = json!({
        "chrome": { "type": "stdio", "command": "/usr/bin/env", "args": args }
    });
    servers_file(mcp_dir(config)?.join("chrome.json"), Some(&servers))
}

fn mcp_dir(config: &Config) -> Option<PathBuf> {
    Some(Config::path(&config.home).parent()?.join("mcp"))
}

fn user_servers(config: &Config, hub_state: &Value) -> Option<PathBuf> {
    servers_file(
        mcp_dir(config)?.join("user.json"),
        hub_state.get("mcpServers"),
    )
}

/// The hub's project-scope MCP servers for `cwd`, which Claude Code keeps
/// under `projects` in `.claude.json`.
fn project_servers(config: &Config, hub_state: &Value, cwd: &Path) -> Option<PathBuf> {
    let servers = hub_state
        .get("projects")
        .and_then(|projects| projects.get(cwd.to_str()?))
        .and_then(|project| project.get("mcpServers"));
    servers_file(
        mcp_dir(config)?.join(format!("{}.json", state::short_hash(cwd))),
        servers,
    )
}

/// `servers` written to `file` in the shape `--mcp-config` reads, rewritten
/// when they change and never read back as truth; without servers the file
/// goes.
fn servers_file(file: PathBuf, servers: Option<&Value>) -> Option<PathBuf> {
    let Some(servers) = servers.filter(|servers| {
        servers
            .as_object()
            .is_some_and(|servers| !servers.is_empty())
    }) else {
        let _ = fs::remove_file(&file);
        return None;
    };
    let content = json!({ "mcpServers": servers });
    if state::read(&file).ok().as_ref() != Some(&content) {
        fs::create_dir_all(file.parent()?).ok()?;
        state::write(&file, &content).ok()?;
    }
    Some(file)
}

/// Replaces this process with `command`; returns only when that fails.
pub fn exec(mut command: Command) -> anyhow::Error {
    let error = command.exec();
    anyhow::Error::new(error).context(format!(
        "starting {}",
        Path::new(command.get_program()).display()
    ))
}

/// What runs when clauset is started as `claude`. Without a config it starts
/// `claude` unchanged, and with one it cannot read it warns first, so clauset
/// never stops work.
pub fn shim(args: Vec<OsString>) -> Result<ExitCode> {
    let claude = real_claude()?;
    let plain = |claude: &Path, args: Vec<OsString>| {
        let mut command = Command::new(claude);
        command.arg0("claude").args(args);
        Err(exec(command))
    };
    let Ok(home) = crate::config::home() else {
        return plain(&claude, args);
    };
    if !Config::path(&home).exists() {
        return plain(&claude, args);
    }
    let config = match Config::load(&home) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("clauset: {error:#}; starting claude unchanged");
            return plain(&claude, args);
        }
    };
    let cwd = env::current_dir().unwrap_or_default();
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    let config_dir = env::var_os("CLAUDE_CONFIG_DIR");
    let marked = env::var_os(MARKER).is_some();
    match config.resolve(config_dir.as_deref(), marked).account() {
        None => plain(&claude, args),
        Some(name) => start(&config, name, &claude, &cwd, args),
    }
}

/// Starts `claude` on an account: at a terminal under a supervisor, so the
/// session can move to another account; anywhere else in this process's place.
pub fn start(
    config: &Config,
    name: &str,
    claude: &Path,
    cwd: &Path,
    args: Vec<OsString>,
) -> Result<ExitCode> {
    if std::io::stdin().is_terminal() {
        return supervise(config, name, claude, cwd, args);
    }
    let mut command = command(
        config,
        name,
        claude,
        shared_servers(cwd, &args),
        shared_chrome(&args),
    )?;
    command.args(args);
    Err(exec(command))
}

static CHILD: AtomicI32 = AtomicI32::new(0);

extern "C" fn forward(signal: libc::c_int) {
    let child = CHILD.load(Ordering::SeqCst);
    if child > 0 {
        // SAFETY: kill(2) is async-signal-safe.
        unsafe {
            libc::kill(child, signal);
        }
    }
}

fn supervise(
    config: &Config,
    name: &str,
    claude: &Path,
    cwd: &Path,
    mut args: Vec<OsString>,
) -> Result<ExitCode> {
    // The terminal sends Ctrl+C and Ctrl+\ to the whole foreground group, and
    // the child handles them; a stop for this process is passed on to it.
    // SAFETY: plain disposition changes before any other thread exists.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGQUIT, libc::SIG_IGN);
        libc::signal(libc::SIGTERM, forward as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, forward as *const () as libc::sighandler_t);
    }
    let mut account = name.to_owned();
    loop {
        let mut command = command(
            config,
            &account,
            claude,
            shared_servers(cwd, &args),
            shared_chrome(&args),
        )?;
        command
            .args(&args)
            .env(switch::SUPERVISOR, std::process::id().to_string());
        // An ignored signal stays ignored across exec, so the child starts
        // with the defaults again.
        // SAFETY: signal(2) is async-signal-safe.
        unsafe {
            command.pre_exec(|| {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                Ok(())
            });
        }
        let mut child = command.spawn().context("starting claude")?;
        let pid = child.id();
        CHILD.store(pid as i32, Ordering::SeqCst);
        // Without its note the session still moves on its own request;
        // only `switch --all` from elsewhere misses it.
        let _ = switch::note(config, pid, &account);
        let done = Arc::new(AtomicBool::new(false));
        let watcher = switch::watch(config, pid, done.clone());
        let status = child.wait()?;
        switch::forget(config, pid);
        done.store(true, Ordering::SeqCst);
        CHILD.store(0, Ordering::SeqCst);
        let moved = watcher.is_some_and(|watcher| watcher.join().unwrap_or(false));
        // A session that ended on its own stays ended, request or not.
        match switch::take(config, pid) {
            Some(request) if moved => {
                eprintln!("clauset: this session continues on {}", request.account);
                carry_trust(config, &account, &request.account, cwd);
                account = request.account;
                args = switch::carry(&args);
                args.extend([
                    OsString::from("--resume"),
                    OsString::from(request.session_id),
                ]);
            }
            _ => return Ok(exit_code(status)),
        }
    }
}

/// Gives the account a session moves to the trust its source account had in
/// `cwd`, and what the hub has answered, so it resumes without dialogs.
fn carry_trust(config: &Config, from: &str, to: &str, cwd: &Path) {
    let _ = crate::account::seed(config, to);
    let (Ok(from), Ok(to)) = (config.account(from), config.account(to)) else {
        return;
    };
    let (source, target) = (config.state_file(from), config.state_file(to));
    let Some(dir) = cwd.to_str() else { return };
    if config.is_hub(to) || !state::read(&source).is_ok_and(|state| state::trusts(&state, dir)) {
        return;
    }
    if let Ok(mut state) = state::read(&target)
        && state::trust(&mut state, dir)
    {
        let _ = state::write(&target, &state);
    }
}

fn exit_code(status: ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) => ExitCode::from(code as u8),
        None => ExitCode::from(128 + status.signal().unwrap_or(0) as u8),
    }
}
