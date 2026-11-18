use std::env;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use console::style;

use clauset::config::{self, Choice, Config, MARKER, Session};
use clauset::edit::ConfigFile;
use clauset::{account, doctor, hook, launch, link, notify, setup, status, switch, usage};

#[derive(Parser)]
#[command(
    name = "clauset",
    version,
    about = "Several Claude Code accounts on one Mac, sharing one ~/.claude",
    after_help = "Without a command, clauset shows the account here and every account's usage."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// The account here, and every account with its plan and usage
    #[command(visible_alias = "usage")]
    Status {
        /// As JSON, for a program to read
        #[arg(long)]
        json: bool,
    },
    /// Make an account the one new sessions start on
    Use { account: String },
    /// Warn when an account passes a percent of its 5-hour or 7-day limit, or `off`
    Warn {
        /// A percent from 1 to 100, or `off`
        percent: String,
        /// Only for this account
        #[arg(long)]
        account: Option<String>,
    },
    /// Name the account the Claude in Chrome extension is signed in to, which every other account reaches it through, or `off`
    Chrome { account: String },
    /// Move this Claude Code session to another account once its turn ends: `! clauset switch <account>`
    Switch {
        account: String,
        /// Move every session on the account this one runs on, not only this one
        #[arg(long)]
        all: bool,
        /// With --all: the account to move the sessions from
        #[arg(long, requires = "all")]
        from: Option<String>,
    },
    /// Start Claude Code on an account, passing every further argument on
    Run {
        account: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// Add an account: sign a new one in, or take in a config dir with --dir
    Add {
        name: String,
        /// The email to sign in with
        #[arg(long)]
        email: Option<String>,
        /// A config dir that already exists, signed in or not
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Take an account out of the config, leaving its directory and sign-in
    Remove { account: String },
    /// Sign an account in again
    Login { account: String },
    /// Set everything up, asking about what is missing
    Setup {
        /// What each step still needs, as JSON, for a program to read
        #[arg(long, conflicts_with = "apply")]
        json: bool,
        /// Carry out one step without asking
        #[arg(long, value_enum)]
        apply: Option<setup::Step>,
        /// With `--apply skills`: write this version's over a skill that differs, too
        #[arg(long, requires = "apply")]
        replace: bool,
    },
    /// Check the accounts, their sign-ins and links
    Doctor,
    /// Which account `claude` starts on here, and why
    Which,
    /// The account this Claude Code session runs on
    #[command(hide = true)]
    Whoami,
    /// Link what the hub gained since into one account's directory, or all
    Link { account: Option<String> },
    /// For a status line command: reads its JSON on stdin, keeps the usage, prints the account
    #[command(hide = true)]
    Statusline,
    /// Claude Code hook entry points; they read the hook's JSON on stdin
    #[command(hide = true)]
    Hook {
        #[arg(value_enum)]
        event: HookEvent,
    },
    /// Put the `claude` shim in place and print the PATH line it needs
    #[command(hide = true)]
    Init,
}

#[derive(Clone, Copy, ValueEnum)]
enum HookEvent {
    /// SessionStart: the account, as context for the model
    Context,
    /// StopFailure on rate_limit: a desktop notification naming the account
    Limit,
}

fn main() -> ExitCode {
    // Rust starts with SIGPIPE ignored, which turns `clauset | head` into a
    // panic on the next print.
    // SAFETY: called before any thread starts; restores the default action.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let mut args = env::args_os();
    let called = args.next().unwrap_or_default();
    if Path::new(&called).file_name() == Some("claude".as_ref()) {
        return match launch::shim(args.collect()) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("clauset: {error:#}");
                ExitCode::FAILURE
            }
        };
    }
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("clauset: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let home = config::home()?;
    let config_dir = env::var_os("CLAUDE_CONFIG_DIR");
    let marked = env::var_os(MARKER).is_some();
    let command = cli.command.unwrap_or(Cmd::Status { json: false });

    // These run inside Claude Code: without a config file they stay silent.
    if matches!(command, Cmd::Statusline | Cmd::Hook { .. } | Cmd::Whoami) {
        let mut input = String::new();
        if !matches!(command, Cmd::Whoami) {
            io::stdin().read_to_string(&mut input)?;
        }
        let Some(config) = Config::load_if_present(&home)? else {
            return Ok(ExitCode::SUCCESS);
        };
        let output = match command {
            Cmd::Hook {
                event: HookEvent::Context,
            } => hook::context(&config, config_dir.as_deref()),
            Cmd::Hook {
                event: HookEvent::Limit,
            } => {
                if let Some(text) =
                    hook::limit(&config, &input, config_dir.as_deref(), chrono::Local::now())
                {
                    notify::send(&config, &text);
                }
                None
            }
            Cmd::Statusline => statusline(&config, &input, config_dir.as_deref()),
            _ => match config.session(config_dir.as_deref()) {
                Session::Account(name) => Some(name.to_owned()),
                _ => None,
            },
        };
        if let Some(output) = output {
            println!("{output}");
        }
        return Ok(ExitCode::SUCCESS);
    }

    match command {
        Cmd::Setup {
            json,
            apply,
            replace,
        } => {
            if replace && !matches!(apply, Some(setup::Step::Skills)) {
                anyhow::bail!("--replace goes with `--apply skills`");
            }
            match apply {
                Some(step) => println!("{}", setup::apply(&home, step, replace)?),
                None if json => println!("{}", setup::state(&home)?),
                None => setup::run(&home)?,
            }
            return Ok(ExitCode::SUCCESS);
        }
        Cmd::Init => {
            let bin = account::init(&home)?;
            println!(
                "{} → {}",
                bin.join("claude").display(),
                account::exe(&home)?.display()
            );
            println!(
                "Put this line last in ~/.zshrc, so the shim comes before every other `claude` on PATH:"
            );
            println!("  export PATH=\"{}:$PATH\"", bin.display());
            return Ok(ExitCode::SUCCESS);
        }
        Cmd::Add { name, email, dir } => {
            account::add(&home, &name, email.as_deref(), dir.as_deref())?;
            return Ok(ExitCode::SUCCESS);
        }
        Cmd::Use { account } => {
            let mut file = ConfigFile::open(&home)?;
            if !file.has_account(&account) {
                anyhow::bail!("no account named `{account}`");
            }
            file.set_default(&account);
            file.save()?;
            println!("New sessions start on {account}");
            return Ok(ExitCode::SUCCESS);
        }
        Cmd::Warn { percent, account } => {
            let percent = match percent.as_str() {
                "off" => None,
                text => Some(
                    text.trim_end_matches('%')
                        .parse::<u8>()
                        .ok()
                        .filter(|p| (1..=100).contains(p))
                        .with_context(|| {
                            format!("`{text}` is no percent from 1 to 100, nor `off`")
                        })?,
                ),
            };
            let mut file = ConfigFile::open(&home)?;
            if let Some(name) = &account
                && !file.has_account(name)
            {
                anyhow::bail!("no account named `{name}`");
            }
            file.set_warn(account.as_deref(), percent);
            file.save()?;
            let whom = account.as_deref().unwrap_or("every account");
            match percent {
                Some(percent) => println!("{whom} warns at {percent}% of a limit"),
                None => println!("{whom}: no warning of its own"),
            }
            if percent.is_some() && !setup::statusline_runs_clauset(&home) {
                println!(
                    "The status line does not run `clauset statusline`, which is what warns: until it does, only the limit hook notifies, at 100%. `clauset setup` sets it up."
                );
            }
            return Ok(ExitCode::SUCCESS);
        }
        Cmd::Chrome { account } => {
            let account = (account != "off").then_some(account);
            let mut file = ConfigFile::open(&home)?;
            if let Some(name) = &account
                && !file.has_account(name)
            {
                anyhow::bail!("no account named `{name}`");
            }
            file.set_chrome(account.as_deref());
            file.save()?;
            match account {
                Some(name) => println!(
                    "Every account reaches Claude in Chrome through {name}, in sessions started from now on"
                ),
                None => println!(
                    "Each account uses Claude in Chrome on its own sign-in, in sessions started from now on"
                ),
            }
            return Ok(ExitCode::SUCCESS);
        }
        _ => {}
    }

    let Some(config) = Config::load_if_present(&home)? else {
        if matches!(command, Cmd::Status { json: true }) {
            anyhow::bail!("no accounts yet: run `clauset setup`");
        }
        println!("No accounts yet. Run `clauset setup`.");
        return Ok(ExitCode::SUCCESS);
    };

    match command {
        Cmd::Status { json: false } => {
            status::print(&config, &directory(None)?, config_dir.as_deref(), marked);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Status { json: true } => {
            println!("{}", status::json(&config, config_dir.as_deref(), marked));
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Run { account, args } => {
            config.account(&account)?;
            let claude = launch::real_claude()?;
            launch::start(&config, &account, &claude, &directory(None)?, args)
        }
        Cmd::Switch { account, all, from } => {
            let said = if all {
                switch::request_all(&config, &account, from.as_deref())?
            } else {
                switch::request(&config, &account)?
            };
            println!("{said}");
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Remove { account } => {
            println!("{}", remove(&config, &account)?);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Login { account } => {
            account::login(&config, &account)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Doctor => Ok(if doctor::run(&config)? {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }),
        Cmd::Which => {
            let choice = config.resolve(config_dir.as_deref(), marked);
            match &choice {
                Choice::Foreign(other) => println!(
                    "unchanged  (CLAUDE_CONFIG_DIR={} is no clauset account)",
                    config.tilde(other)
                ),
                Choice::Inherited(name) => {
                    println!("{name}  (inherited from the session that started it)")
                }
                Choice::Default(name) => println!("{name}  (default)"),
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Link { account } => {
            let accounts = match &account {
                Some(name) => vec![(name, config.account(name)?)],
                None => config.accounts.iter().collect(),
            };
            for (name, account) in accounts {
                if !config.is_hub(account) {
                    let report = link::sync(&config.hub(), &account.dir, true)?;
                    println!("{} {}", style(name).bold(), account::describe(&report));
                    if account::seed(&config, name)? {
                        println!(
                            "{} copied answered dialogs and project trust from the hub",
                            style(name).bold()
                        );
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => unreachable!("handled above"),
    }
}

fn remove(config: &Config, name: &str) -> Result<String> {
    let account = config.account(name)?;
    if config.default == name {
        anyhow::bail!("{name} is the default: `clauset use <account>` picks another first");
    }
    if config.chrome.as_deref() == Some(name) {
        anyhow::bail!("`chrome` names {name}: `clauset chrome <account|off>` changes it first");
    }
    if config.is_hub(account) {
        anyhow::bail!(
            "{name} is the hub, the account ~/.claude is signed in to and every other account shares from: it stays"
        );
    }
    if let Some(&sessions) = switch::sessions_per_account(config).get(name) {
        let run = if sessions == 1 {
            "1 session runs".to_owned()
        } else {
            format!("{sessions} sessions run")
        };
        anyhow::bail!(
            "{run} on {name}: end them, or move them with `clauset switch <account> --all --from {name}`"
        );
    }
    let dir = config.tilde(&account.dir);
    let mut file = ConfigFile::open(&config.home)?;
    file.remove_account(name);
    file.save()?;
    usage::forget(config, name);
    Ok(format!(
        "{name} is no longer an account. Its directory {dir} and its sign-in stay: `clauset add {name} --dir {dir}` takes it back."
    ))
}

fn directory(dir: Option<PathBuf>) -> Result<PathBuf> {
    let dir = env::current_dir()?.join(dir.unwrap_or_default());
    dir.canonicalize()
        .with_context(|| format!("reading {}", dir.display()))
}

fn statusline(
    config: &Config,
    input: &str,
    config_dir: Option<&std::ffi::OsStr>,
) -> Option<String> {
    let Session::Account(name) = config.session(config_dir) else {
        return None;
    };
    if let Ok(value) = serde_json::from_str(input)
        && let Some(limits) = usage::from_statusline(&value, chrono::Utc::now().timestamp_millis())
    {
        let _ = usage::record(config, name, &limits);
        if let Some(threshold) = config.warn_at(name) {
            let now = chrono::Local::now();
            for (label, window) in usage::newly_crossed(config, name, &limits, threshold) {
                let reset = window
                    .resets_at
                    .and_then(|at| chrono::TimeZone::timestamp_opt(&chrono::Local, at, 0).single())
                    .map(|reset| format!(", resets {}", status::when(reset, now)))
                    .unwrap_or_default();
                notify::send(
                    config,
                    &format!(
                        "{} is at {:.0}% of its {label} limit{reset}",
                        notify::title(name),
                        window.percent
                    ),
                );
            }
        }
    }
    Some(name.to_owned())
}
