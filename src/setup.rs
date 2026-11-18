//! Every step reads what is there first and asks only about what is missing.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use cliclack::{confirm, input, intro, log, note, outro, select};
use console::{Term, style};
use serde_json::{Value, json};

use crate::account;
use crate::config::{self, Config};
use crate::edit::ConfigFile;
use crate::state::{self, SignIn};

pub fn run(home: &Path) -> Result<()> {
    if !Term::stdout().is_term() {
        bail!("setup asks questions: run it in a terminal");
    }
    intro(style(" clauset setup ").reverse())?;

    hub_account(home)?;
    more_accounts(home)?;
    default_account(home)?;
    shim(home)?;
    hooks(home)?;
    statusline(home)?;
    skills(home)?;
    warn(home)?;

    outro(
        "Done. `clauset` shows the accounts and their usage, `clauset doctor` checks everything.",
    )?;
    Ok(())
}

/// A step that needs no answer, so a program can carry it out as the
/// questions do. The other steps are commands of their own: `add`, `use`
/// and `warn`.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Step {
    /// The `claude` shim, first on PATH
    Shim,
    /// The Claude Code hooks
    Hooks,
    /// Claude Code's status line
    Statusline,
    /// The `clauset` and `accounts` skills
    Skills,
}

/// `replace` lets `Skills` write this version's over a skill that differs.
pub fn apply(home: &Path, step: Step, replace: bool) -> Result<String> {
    match step {
        Step::Shim => apply_shim(home),
        Step::Hooks => apply_hooks(home),
        Step::Statusline => apply_statusline(home),
        Step::Skills => apply_skills(home, replace),
    }
}

/// What each step still needs, for a program to read, such as the menu bar
/// app. It changes nothing.
pub fn state(home: &Path) -> Result<Value> {
    let config = Config::load(home).ok();
    let hub = config.as_ref().and_then(|config| {
        config
            .accounts
            .iter()
            .find(|(_, account)| config.is_hub(account))
            .map(|(name, _)| name.as_str())
    });
    let sign_in = state::read(&home.join(".claude.json"))
        .ok()
        .as_ref()
        .and_then(state::sign_in);
    let done = |done: bool| if done { "done" } else { "todo" };

    let shimmed = crate::doctor::first_claude_is_clauset();
    let missing_hooks = missing_hooks(home);
    let (status_line, command) = match status_line(home)? {
        StatusLine::Ours => ("done", None),
        StatusLine::Unset => ("todo", None),
        StatusLine::Elsewhere(program) => ("todo", Some(format!("{program} statusline"))),
        StatusLine::Own(command) => ("manual", Some(command)),
    };
    let (missing_skills, differ) = skills_state(home);
    let skills = match (missing_skills.is_empty(), differ.is_empty()) {
        (false, _) => "todo",
        (true, false) => "differs",
        (true, true) => "done",
    };
    Ok(json!({
        "version": 1,
        "claude": crate::launch::real_claude().is_ok(),
        "hub": {
            "account": hub,
            "signed_in": sign_in.is_some(),
            "suggestion": sign_in.as_ref().map(suggest),
        },
        "steps": {
            "shim": {
                "state": done(shimmed),
                "file": shell_rc(home).map(|rc| config::tilde(&rc, home)),
            },
            "hooks": {"state": done(missing_hooks.is_empty()), "missing": missing_hooks},
            "statusline": {"state": status_line, "command": command},
            "skills": {"state": skills, "missing": missing_skills, "differ": differ},
        },
    }))
}

fn hub_account(home: &Path) -> Result<()> {
    if let Ok(config) = Config::load(home)
        && let Some((name, _)) = config
            .accounts
            .iter()
            .find(|(_, account)| config.is_hub(account))
    {
        log::info(format!("~/.claude is the {name} account"))?;
        return Ok(());
    }
    let Some(sign_in) = state::read(&home.join(".claude.json"))
        .ok()
        .as_ref()
        .and_then(state::sign_in)
    else {
        log::warning(
            "~/.claude is not signed in. Run `claude auth login` first to keep it as an account.",
        )?;
        return Ok(());
    };
    let name: String = input(format!(
        "~/.claude is signed in as {}. Name this account",
        account::describe_sign_in(&sign_in)
    ))
    .default_input(&suggest(&sign_in))
    .validate(|name: &String| account::valid_name(name).map_err(|error| error.to_string()))
    .interact()?;
    account::add(home, &name, None, Some(&home.join(".claude")))
}

fn more_accounts(home: &Path) -> Result<()> {
    loop {
        let count = Config::load(home)
            .map(|config| config.accounts.len())
            .unwrap_or(0);
        if !confirm("Add another Claude account?")
            .initial_value(count < 2)
            .interact()?
        {
            return Ok(());
        }
        let email: String = input("The email it signs in with").interact()?;
        let name: String = input("A name for it")
            .placeholder("work")
            .validate(|name: &String| account::valid_name(name).map_err(|error| error.to_string()))
            .interact()?;
        log::step("Sign in in the browser, and pick the organization this account is for.")?;
        if let Err(error) = account::add(home, &name, Some(&email), None) {
            log::error(format!("{error:#}"))?;
        }
    }
}

fn default_account(home: &Path) -> Result<()> {
    let Some(config) = Config::load_if_present(home)? else {
        return Ok(());
    };
    if config.accounts.len() < 2 {
        return Ok(());
    }
    note(
        "Default",
        "A new session starts on the default account, or on the account of the session that started it.\n\nLater: `clauset use <account>` changes the default, and `clauset switch <account>` inside a session moves that session there.",
    )?;
    let mut prompt = select("The account new sessions start on");
    for name in config.accounts.keys() {
        prompt = prompt.item(name.clone(), name, plan_hint(&config, name));
    }
    let chosen = prompt.initial_value(config.default.clone()).interact()?;
    if chosen != config.default {
        let mut file = ConfigFile::open(home)?;
        file.set_default(&chosen);
        file.save()?;
    }
    Ok(())
}

fn warn(home: &Path) -> Result<()> {
    let Some(config) = Config::load_if_present(home)? else {
        return Ok(());
    };
    if config.warn_at.is_some()
        || config
            .accounts
            .values()
            .any(|account| account.warn_at.is_some())
    {
        return Ok(());
    }
    if !statusline_runs_clauset(home) {
        log::remark(
            "A warning threshold fires from the status line, which does not run `clauset statusline`. Once it does, `clauset warn 80` sets one.",
        )?;
        return Ok(());
    }
    let answer: String = input(
        "Warn when an account passes what percent of its 5-hour or 7-day limit? Empty: never",
    )
    .placeholder("80")
    .required(false)
    .validate(|text: &String| match text.trim() {
        "" => Ok(()),
        text => text
            .parse::<u8>()
            .ok()
            .filter(|percent| (1..=100).contains(percent))
            .map(|_| ())
            .ok_or("a percent from 1 to 100"),
    })
    .interact()?;
    if let Ok(percent) = answer.trim().parse::<u8>() {
        let mut file = ConfigFile::open(home)?;
        file.set_warn(None, Some(percent));
        file.save()?;
    }
    Ok(())
}

fn shell_rc(home: &Path) -> Option<PathBuf> {
    match std::env::var("SHELL")
        .unwrap_or_default()
        .rsplit('/')
        .next()
    {
        Some("zsh") => Some(home.join(".zshrc")),
        Some("bash") => Some(home.join(".bashrc")),
        _ => None,
    }
}

fn apply_shim(home: &Path) -> Result<String> {
    let bin = account::init(home)?;
    let Some(rc) = shell_rc(home) else {
        return Ok(format!(
            "Put `export PATH=\"{}:$PATH\"` last in your shell's startup file.",
            bin.display()
        ));
    };
    let text = fs::read_to_string(&rc).unwrap_or_default();
    if !text.contains("clauset/bin") {
        let mut file = fs::OpenOptions::new().append(true).create(true).open(&rc)?;
        writeln!(
            file,
            "\n# clauset shim: stays below every other line that changes PATH, so `claude` resolves to it first"
        )?;
        writeln!(file, "export PATH=\"{}:$PATH\"", bin.display())?;
    }
    Ok("Open a new terminal, and restart Solo or any app that starts `claude`, to use it.".into())
}

fn shim(home: &Path) -> Result<()> {
    if crate::doctor::first_claude_is_clauset() {
        return Ok(());
    }
    let Some(rc) = shell_rc(home) else {
        log::info(apply_shim(home)?)?;
        return Ok(());
    };
    let shown = rc
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !confirm(format!(
        "Put the `claude` shim first on PATH, with a line at the end of ~/{shown}?"
    ))
    .initial_value(true)
    .interact()?
    {
        return Ok(());
    }
    log::success(apply_shim(home)?)?;
    Ok(())
}

/// (event, matcher, hook, what it does): each is added on its own, so a hook
/// a later version brings is offered without repeating the others.
const HOOKS: [(&str, Option<&str>, &str, &str); 2] = [
    (
        "SessionStart",
        None,
        "context",
        "the session's account as context for Claude",
    ),
    (
        "StopFailure",
        Some("rate_limit"),
        "limit",
        "a notification when an account hits its limit",
    ),
];

fn hook_entry<'a>(settings: &'a mut Value, event: &str, hook: &str) -> Option<&'a mut Value> {
    let command = format!("clauset hook {hook}");
    settings["hooks"][event]
        .as_array_mut()?
        .iter_mut()
        .find(|entry| {
            entry["hooks"].as_array().is_some_and(|hooks| {
                hooks
                    .iter()
                    .any(|h| h["command"].as_str().is_some_and(|c| c.ends_with(&command)))
            })
        })
}

fn has_hook(
    settings: &mut Value,
    event: &str,
    matcher: Option<&str>,
    hook: &str,
    exe: &str,
) -> bool {
    let command = format!("{exe} hook {hook}");
    hook_entry(settings, event, hook).is_some_and(|entry| {
        entry["matcher"].as_str() == matcher
            && entry["hooks"]
                .as_array()
                .is_some_and(|hooks| hooks.iter().any(|h| h["command"] == command.as_str()))
    })
}

fn hooks(home: &Path) -> Result<()> {
    let path = home.join(".claude/settings.json");
    let mut settings = read_settings(&path)?;
    let exe = exe_command(home)?;
    let missing = missing_in(&mut settings, &exe);
    if missing.is_empty() {
        return Ok(());
    }
    let list: Vec<&str> = HOOKS
        .iter()
        .filter(|(_, _, hook, _)| missing.contains(hook))
        .map(|(_, _, _, what)| *what)
        .collect();
    if !confirm(format!("Add the Claude Code hooks: {}?", list.join("; ")))
        .initial_value(true)
        .interact()?
    {
        return Ok(());
    }
    apply_hooks(home)?;
    Ok(())
}

fn apply_hooks(home: &Path) -> Result<String> {
    let path = home.join(".claude/settings.json");
    let mut settings = read_settings(&path)?;
    let exe = exe_command(home)?;
    let missing = missing_in(&mut settings, &exe);
    if missing.is_empty() {
        return Ok("The hooks are in ~/.claude/settings.json already.".into());
    }
    put_hooks(&mut settings, &exe, &missing);
    write_settings(&path, &settings)?;
    Ok(format!(
        "Added to ~/.claude/settings.json: the {} hook{}.",
        missing.join(" and "),
        if missing.len() == 1 { "" } else { "s" }
    ))
}

fn put_hooks(settings: &mut Value, exe: &str, names: &[&str]) {
    for (event, matcher, hook, _) in HOOKS.iter().filter(|(_, _, hook, _)| names.contains(hook)) {
        // An entry with an older matcher, or another clauset, gets the
        // current one: a clauset installed elsewhere takes over its hooks.
        if let Some(entry) = hook_entry(settings, event, hook) {
            match matcher {
                Some(matcher) => entry["matcher"] = json!(matcher),
                None => {
                    if let Some(object) = entry.as_object_mut() {
                        object.remove("matcher");
                    }
                }
            }
            let suffix = format!("clauset hook {hook}");
            for h in entry["hooks"].as_array_mut().into_iter().flatten() {
                if h["command"].as_str().is_some_and(|c| c.ends_with(&suffix)) {
                    h["command"] = json!(format!("{exe} hook {hook}"));
                }
            }
            continue;
        }
        let mut entry = json!({"hooks": [{"type": "command", "command": format!("{exe} hook {hook}"), "timeout": 5}]});
        if let Some(matcher) = matcher {
            entry["matcher"] = json!(matcher);
        }
        push(settings, event, entry);
    }
}

enum StatusLine {
    /// Nothing: clauset can be it.
    Unset,
    /// This clauset, in its command or in the script the command names.
    Ours,
    /// A clauset installed elsewhere, at this path.
    Elsewhere(String),
    /// A command of the person's own, which clauset leaves to them.
    Own(String),
}

fn status_line(home: &Path) -> Result<StatusLine> {
    let settings = read_settings(&home.join(".claude/settings.json"))?;
    let exe = exe_command(home)?;
    let Some(command) = settings
        .get("statusLine")
        .and_then(|line| line.get("command"))
        .and_then(Value::as_str)
    else {
        return Ok(StatusLine::Unset);
    };
    let words: Vec<&str> = command.split_whitespace().collect();
    if let [program, "statusline"] = words[..]
        && program.ends_with("clauset")
        && program != exe
    {
        return Ok(StatusLine::Elsewhere(program.to_owned()));
    }
    Ok(if statusline_runs_clauset(home) {
        StatusLine::Ours
    } else {
        StatusLine::Own(command.to_owned())
    })
}

fn own_status_line(command: &str, exe: &str) -> String {
    format!(
        "Your status line runs `{command}`. Pipe the JSON it gets through `{exe} statusline`: that prints the account and keeps its usage for `clauset`."
    )
}

/// A status line of the person's own is refused: only they can pipe it
/// through clauset.
fn apply_statusline(home: &Path) -> Result<String> {
    let path = home.join(".claude/settings.json");
    let exe = exe_command(home)?;
    match status_line(home)? {
        StatusLine::Ours => Ok("The status line runs `clauset statusline` already.".into()),
        StatusLine::Own(command) => bail!("{}", own_status_line(&command, &exe)),
        StatusLine::Unset | StatusLine::Elsewhere(_) => {
            let mut settings = read_settings(&path)?;
            match settings
                .get_mut("statusLine")
                .filter(|line| line.is_object())
            {
                Some(line) => line["command"] = json!(format!("{exe} statusline")),
                None => {
                    settings["statusLine"] =
                        json!({"type": "command", "command": format!("{exe} statusline")});
                }
            }
            write_settings(&path, &settings)?;
            Ok(format!(
                "The status line in ~/.claude/settings.json runs `{exe} statusline`."
            ))
        }
    }
}

fn statusline(home: &Path) -> Result<()> {
    let exe = exe_command(home)?;
    let question = match status_line(home)? {
        StatusLine::Ours => return Ok(()),
        StatusLine::Own(command) => {
            log::info(own_status_line(&command, &exe))?;
            return Ok(());
        }
        StatusLine::Elsewhere(program) => format!(
            "Your status line runs `{program} statusline`. Run this clauset's instead, `{exe} statusline`?"
        ),
        StatusLine::Unset => "Show the account in Claude Code's status line?".to_owned(),
    };
    if confirm(question).initial_value(true).interact()? {
        apply_statusline(home)?;
    }
    Ok(())
}

const SKILLS: [(&str, &str); 2] = [
    ("clauset", include_str!("../skills/clauset/SKILL.md")),
    ("accounts", include_str!("../skills/accounts/SKILL.md")),
];

fn skill_path(home: &Path, name: &str) -> PathBuf {
    home.join(".claude/skills").join(name).join("SKILL.md")
}

/// The skills the hub lacks, and those it holds in another version, by name.
/// A skill that differs may be one the person changed, so it is never
/// replaced without asking.
pub fn skills_state(home: &Path) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut missing = Vec::new();
    let mut differ = Vec::new();
    for (name, text) in SKILLS {
        match fs::read_to_string(skill_path(home, name)) {
            Err(_) => missing.push(name),
            Ok(held) if held != text => differ.push(name),
            Ok(_) => {}
        }
    }
    (missing, differ)
}

fn write_skills(home: &Path, names: &[&str]) -> Result<()> {
    for (name, text) in SKILLS.iter().filter(|(name, _)| names.contains(name)) {
        let path = skill_path(home, name);
        fs::create_dir_all(path.parent().expect("joined above"))?;
        fs::write(&path, text)?;
    }
    Ok(())
}

fn skill_paths(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("~/.claude/skills/{name}/SKILL.md"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn apply_skills(home: &Path, replace: bool) -> Result<String> {
    let (missing, differ) = skills_state(home);
    let mut written = missing.clone();
    if replace {
        written.extend(&differ);
    }
    write_skills(home, &written)?;
    let mut said = if written.is_empty() {
        "The skills are in ~/.claude/skills already.".to_owned()
    } else {
        format!("Wrote {}.", skill_paths(&written))
    };
    if !replace && !differ.is_empty() {
        said.push_str(&format!(
            " Left as found, differing from this version's: {}. `--replace` writes this version's there.",
            skill_paths(&differ)
        ));
    }
    Ok(said)
}

fn skills(home: &Path) -> Result<()> {
    let (missing, differ) = skills_state(home);
    let paths = skill_paths;
    if !missing.is_empty()
        && confirm(format!(
            "Write the skills that let you run `/clauset <args>` and tell Claude what clauset does: {}?",
            paths(&missing)
        ))
        .initial_value(true)
        .interact()?
    {
        write_skills(home, &missing)?;
    }
    if !differ.is_empty()
        && confirm(format!(
            "{} differ from this version's, perhaps because you changed them. Replace them?",
            paths(&differ)
        ))
        .initial_value(false)
        .interact()?
    {
        write_skills(home, &differ)?;
    }
    Ok(())
}

/// Whether Claude Code's status line runs `clauset statusline`, in its
/// command or in the script that command names last.
pub fn statusline_runs_clauset(home: &Path) -> bool {
    let Ok(settings) = read_settings(&home.join(".claude/settings.json")) else {
        return false;
    };
    let Some(command) = settings
        .get("statusLine")
        .and_then(|line| line.get("command"))
        .and_then(Value::as_str)
    else {
        return false;
    };
    command.contains("clauset statusline")
        || command
            .split_whitespace()
            .last()
            .map(|word| config::expand(Path::new(word), home))
            .and_then(|script| fs::read_to_string(script).ok())
            .is_some_and(|text| text.contains("clauset statusline"))
}

/// The clauset hooks the hub's settings lack, or run with another matcher or
/// from another clauset, by name.
pub fn missing_hooks(home: &Path) -> Vec<&'static str> {
    let mut settings = read_settings(&home.join(".claude/settings.json")).unwrap_or_default();
    missing_in(&mut settings, &exe_command(home).unwrap_or_default())
}

fn missing_in(settings: &mut Value, exe: &str) -> Vec<&'static str> {
    HOOKS
        .iter()
        .filter(|(event, matcher, hook, _)| !has_hook(settings, event, *matcher, hook, exe))
        .map(|(_, _, hook, _)| *hook)
        .collect()
}

fn read_settings(path: &Path) -> Result<Value> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(error.into()),
    }
}

fn write_settings(path: &Path, settings: &Value) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(settings)? + "\n")?;
    Ok(())
}

fn push(settings: &mut Value, event: &str, entry: Value) {
    let list = &mut settings["hooks"][event];
    if !list.is_array() {
        *list = json!([]);
    }
    list.as_array_mut().expect("set above").push(entry);
}

fn exe_command(home: &Path) -> Result<String> {
    Ok(config::tilde(&account::exe(home)?, home))
}

fn suggest(sign_in: &SignIn) -> String {
    let org: String = sign_in
        .org()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    org.trim_matches('-').to_owned()
}

fn plan_hint(config: &Config, name: &str) -> String {
    config
        .account(name)
        .ok()
        .and_then(|account| state::read(&config.state_file(account)).ok())
        .as_ref()
        .and_then(state::sign_in)
        .map(|sign_in| account::describe_sign_in(&sign_in))
        .unwrap_or_else(|| "not signed in".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_with_another_matcher_or_clauset_counts_as_missing() {
        let exe = "~/.cargo/bin/clauset";
        let entry = |command: &str, matcher: Option<&str>| {
            let mut entry = json!({"hooks": [{"type": "command", "command": command}]});
            if let Some(matcher) = matcher {
                entry["matcher"] = json!(matcher);
            }
            entry
        };
        let mut settings = json!({"hooks": {
            "SessionStart": [entry("~/.cargo/bin/clauset hook context", None)],
            "StopFailure": [entry("~/.cargo/bin/clauset hook limit", Some("api_error"))],
        }});
        assert_eq!(missing_in(&mut settings, exe), ["limit"]);

        let mut settings = json!({"hooks": {
            "SessionStart": [entry("~/.local/bin/clauset hook context", None)],
            "StopFailure": [entry("~/.cargo/bin/clauset hook limit", Some("rate_limit"))],
        }});
        assert_eq!(missing_in(&mut settings, exe), ["context"]);
    }

    #[test]
    fn a_clauset_installed_elsewhere_takes_over_its_hooks() {
        let exe = "~/.cargo/bin/clauset";
        let mut settings = json!({"hooks": {
            "SessionStart": [{"matcher": "startup", "hooks": [{"type": "command", "command": "~/.local/bin/clauset hook context", "timeout": 5}]}],
        }});
        let missing = missing_in(&mut settings, exe);
        assert_eq!(missing, ["context", "limit"]);
        put_hooks(&mut settings, exe, &missing);
        assert_eq!(
            settings,
            json!({"hooks": {
                "SessionStart": [{"hooks": [{"type": "command", "command": "~/.cargo/bin/clauset hook context", "timeout": 5}]}],
                "StopFailure": [{"matcher": "rate_limit", "hooks": [{"type": "command", "command": "~/.cargo/bin/clauset hook limit", "timeout": 5}]}],
            }})
        );
        assert!(missing_in(&mut settings, exe).is_empty());
    }

    #[test]
    fn tells_missing_skills_from_changed_ones() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        assert_eq!(skills_state(home), (vec!["clauset", "accounts"], vec![]));

        fs::create_dir_all(home.join(".claude/skills/accounts")).unwrap();
        fs::write(skill_path(home, "accounts"), "a version of my own").unwrap();
        write_skills(home, &["clauset"]).unwrap();
        assert_eq!(skills_state(home), (vec![], vec!["accounts"]));
        assert_eq!(
            fs::read_to_string(home.join(".claude/skills/clauset/SKILL.md")).unwrap(),
            include_str!("../skills/clauset/SKILL.md")
        );

        write_skills(home, &["accounts"]).unwrap();
        assert_eq!(skills_state(home), (vec![], vec![]));
    }

    #[test]
    fn suggests_a_name_from_the_org() {
        let sign_in = |org_name: &str, org_type: &str, plan_name: &str| SignIn {
            org_name: org_name.into(),
            org_type: org_type.into(),
            plan_name: plan_name.into(),
            ..Default::default()
        };
        assert_eq!(suggest(&sign_in("Acme", "claude_team", "")), "acme");
        assert_eq!(
            suggest(&sign_in("a@b.c's Organization", "claude_max", "Ultra")),
            "personal"
        );
        assert_eq!(
            suggest(&sign_in("Acme Labs", "claude_team", "")),
            "acme-labs"
        );
    }
}
