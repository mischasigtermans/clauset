use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const FAKE_CLAUDE: &str = r#"#!/bin/sh
echo "ARGS:$*"
echo "CONFIG_DIR:${CLAUDE_CONFIG_DIR-unset}"
echo "PLUGINS:${CLAUDE_CODE_PLUGIN_CACHE_DIR-unset}"
echo "MARKER:${CLAUSET_ACCOUNT-unset}"
"#;

struct World {
    root: TempDir,
}

impl World {
    fn new() -> Self {
        let world = World {
            root: tempfile::tempdir().unwrap(),
        };
        let home = world.home();
        for dir in [
            ".claude/skills",
            ".claude-accounts/personal",
            "work/app",
            "misc",
            "fake",
            "shim",
        ] {
            fs::create_dir_all(home.join(dir)).unwrap();
        }
        fs::write(home.join(".claude.json"), r#"{"mcpServers":{}}"#).unwrap();
        let fake = home.join("fake/claude");
        fs::write(&fake, FAKE_CLAUDE).unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        symlink(env!("CARGO_BIN_EXE_clauset"), home.join("shim/claude")).unwrap();
        world.config(
            r#"
            default = "personal"
            [accounts.personal]
            dir = "~/.claude-accounts/personal"
            [accounts.team]
            dir = "~/.claude"
            "#,
        );
        world
    }

    fn home(&self) -> PathBuf {
        self.root.path().canonicalize().unwrap().join("home")
    }

    fn config(&self, text: &str) {
        let dir = self.home().join(".claude/clauset");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), text).unwrap();
    }

    fn command(&self, program: &Path, cwd: &str) -> Command {
        let home = self.home();
        let mut command = Command::new(program);
        command
            .current_dir(home.join(cwd))
            .env_clear()
            .env("HOME", &home)
            .env(
                "PATH",
                format!(
                    "{}:{}:/usr/bin:/bin",
                    home.join("shim").display(),
                    home.join("fake").display()
                ),
            );
        command
    }

    fn clauset(&self, cwd: &str) -> Command {
        self.command(Path::new(env!("CARGO_BIN_EXE_clauset")), cwd)
    }

    fn shim(&self, cwd: &str) -> Command {
        self.command(&self.home().join("shim/claude"), cwd)
    }

    /// The flags that give an account beside the hub the hub's Chrome.
    fn chrome(&self) -> String {
        format!(
            "--no-chrome --mcp-config={}",
            self.home()
                .join(".claude/clauset/mcp/chrome.json")
                .display()
        )
    }

    fn personal_dir(&self) -> String {
        self.home()
            .join(".claude-accounts/personal")
            .display()
            .to_string()
    }
}

fn stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn run_leaves_the_hubs_servers_out_when_the_caller_asks_for_its_own_alone() {
    let world = World::new();
    fs::write(
        world.home().join(".claude.json"),
        r#"{"mcpServers":{"bear":{"command":"bearcli"}}}"#,
    )
    .unwrap();
    let out = stdout(
        world
            .clauset("misc")
            .args(["run", "personal", "--strict-mcp-config", "-p", "hi there"])
            .output()
            .unwrap(),
    );
    assert!(
        out.contains("ARGS:--strict-mcp-config -p hi there"),
        "{out}"
    );
}

#[test]
fn run_starts_another_account_with_its_dir_plugins_and_the_hubs_servers() {
    let world = World::new();
    let home = world.home();
    fs::write(
        home.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"hub@example.com"},"mcpServers":{"bear":{"command":"bearcli"}}}"#,
    )
    .unwrap();
    let out = stdout(
        world
            .clauset("misc")
            .args(["run", "personal", "-p", "hi there"])
            .output()
            .unwrap(),
    );
    let user = home.join(".claude/clauset/mcp/user.json");
    assert!(
        out.contains(&format!("ARGS:--mcp-config={} -p hi there", user.display())),
        "{out}"
    );
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&user).unwrap()).unwrap();
    assert_eq!(written["mcpServers"]["bear"]["command"], "bearcli");
    assert!(written.get("oauthAccount").is_none(), "{written}");
    assert!(
        out.contains(&format!("CONFIG_DIR:{}", world.personal_dir())),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "PLUGINS:{}",
            home.join(".claude/plugins").display()
        )),
        "{out}"
    );
    assert!(out.contains("MARKER:personal"), "{out}");
}

#[test]
fn run_adds_the_hubs_project_servers_for_the_directory() {
    let world = World::new();
    let home = world.home();
    let misc = home.join("misc");
    let hub_state = format!(
        r#"{{"mcpServers":{{}},"projects":{{"{}":{{"mcpServers":{{"stripe":{{"type":"http","url":"https://mcp.stripe.com"}}}}}}}}}}"#,
        misc.display()
    );
    fs::write(home.join(".claude.json"), hub_state).unwrap();

    let out = stdout(
        world
            .clauset("misc")
            .args(["run", "personal"])
            .output()
            .unwrap(),
    );
    let file = fs::read_dir(home.join(".claude/clauset/mcp"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        out.contains(&format!("--mcp-config={}", file.display())),
        "{out}"
    );
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(
        written["mcpServers"]["stripe"]["url"],
        "https://mcp.stripe.com"
    );

    let elsewhere = stdout(
        world
            .clauset("work")
            .args(["run", "personal"])
            .output()
            .unwrap(),
    );
    assert_eq!(elsewhere.matches("--mcp-config=").count(), 0, "{elsewhere}");

    fs::write(home.join(".claude.json"), r#"{"mcpServers":{}}"#).unwrap();
    let removed = stdout(
        world
            .clauset("misc")
            .args(["run", "personal"])
            .output()
            .unwrap(),
    );
    assert_eq!(removed.matches("--mcp-config=").count(), 0, "{removed}");
    assert!(!file.exists(), "the copy of a removed server stays on disk");
}

#[test]
fn run_reaches_chrome_through_the_account_chrome_names() {
    let world = World::new();
    let home = world.home();
    let file = home.join(".claude/clauset/mcp/chrome.json");
    let run = |args: &[&str]| stdout(world.clauset("misc").args(args).output().unwrap());
    let server = || -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&file).unwrap()).unwrap()
            ["mcpServers"]["chrome"]
            .clone()
    };
    let claude = home.join("fake/claude").display().to_string();

    let unset = run(&["run", "personal", "-p", "hi"]);
    assert!(unset.contains("ARGS:-p hi"), "{unset}");

    world.config(
        r#"
        default = "personal"
        chrome = "team"
        [accounts.personal]
        dir = "~/.claude-accounts/personal"
        [accounts.team]
        dir = "~/.claude"
        "#,
    );
    let out = run(&["run", "personal", "-p", "hi"]);
    assert!(
        out.contains(&format!("ARGS:{} -p hi", world.chrome())),
        "{out}"
    );
    assert_eq!(
        server(),
        serde_json::json!({
            "type": "stdio",
            "command": "/usr/bin/env",
            "args": [
                "-u", "CLAUDE_CONFIG_DIR",
                "-u", "CLAUDE_CODE_PLUGIN_CACHE_DIR",
                claude,
                "--claude-in-chrome-mcp",
            ],
        })
    );
    for args in [
        &["run", "team", "-p", "hi"][..],
        &["run", "personal", "--chrome", "-p", "hi"],
        &["run", "personal", "--no-chrome", "-p", "hi"],
        &["run", "personal", "--strict-mcp-config", "-p", "hi"],
    ] {
        let out = run(args);
        assert!(!out.contains("chrome.json"), "{args:?}: {out}");
        assert_eq!(
            out.matches("--no-chrome").count(),
            args.contains(&"--no-chrome") as usize,
            "{args:?}: {out}"
        );
    }

    world.config(
        r#"
        default = "personal"
        chrome = "personal"
        [accounts.personal]
        dir = "~/.claude-accounts/personal"
        [accounts.team]
        dir = "~/.claude"
        "#,
    );
    let hub = run(&["run", "team", "-p", "hi"]);
    assert!(
        hub.contains(&format!("ARGS:{} -p hi", world.chrome())),
        "{hub}"
    );
    assert_eq!(
        server()["args"],
        serde_json::json!([
            format!("CLAUDE_CONFIG_DIR={}", world.personal_dir()),
            format!(
                "CLAUDE_CODE_PLUGIN_CACHE_DIR={}",
                home.join(".claude/plugins").display()
            ),
            claude,
            "--claude-in-chrome-mcp",
        ])
    );
    let own = run(&["run", "personal", "-p", "hi"]);
    assert!(own.contains("ARGS:-p hi"), "{own}");
}

#[test]
fn run_starts_the_hub_without_a_config_dir_even_when_one_is_inherited() {
    let world = World::new();
    let out = stdout(
        world
            .clauset("misc")
            .env("CLAUDE_CONFIG_DIR", world.personal_dir())
            .args(["run", "team", "--resume", "abc"])
            .output()
            .unwrap(),
    );
    assert!(out.contains("ARGS:--resume abc"), "{out}");
    assert!(out.contains("CONFIG_DIR:unset"), "{out}");
    assert!(out.contains("PLUGINS:unset"), "{out}");
    assert!(out.contains("MARKER:team"), "{out}");
}

#[test]
fn the_shim_starts_on_the_default() {
    let world = World::new();
    let out = stdout(world.shim("work/app").arg("auth").output().unwrap());
    assert!(
        out.contains(&format!("CONFIG_DIR:{}", world.personal_dir())),
        "{out}"
    );
    assert!(out.contains("ARGS:auth"), "{out}");
}

#[test]
fn the_shim_keeps_the_account_of_a_hub_session_it_started() {
    let world = World::new();
    let out = stdout(
        world
            .shim("misc")
            .env("CLAUSET_ACCOUNT", "team")
            .output()
            .unwrap(),
    );
    assert!(
        out.contains("CONFIG_DIR:unset") && out.contains("MARKER:team"),
        "{out}"
    );
}

#[test]
fn the_shim_leaves_a_foreign_config_dir_alone() {
    let world = World::new();
    let out = stdout(
        world
            .shim("work")
            .env("CLAUDE_CONFIG_DIR", "/elsewhere")
            .arg("x")
            .output()
            .unwrap(),
    );
    assert!(
        out.contains("ARGS:x") && out.contains("CONFIG_DIR:/elsewhere"),
        "{out}"
    );
    assert!(out.contains("MARKER:unset"), "{out}");
}

#[test]
fn the_shim_starts_claude_unchanged_when_the_config_is_broken() {
    let world = World::new();
    world.config("default = ");
    let output = world.shim("misc").arg("y").output().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains("starting claude unchanged"));
    let out = stdout(output);
    assert!(
        out.contains("ARGS:y") && out.contains("MARKER:unset"),
        "{out}"
    );
}

#[test]
fn which_and_whoami_name_the_account() {
    let world = World::new();
    let which = stdout(world.clauset("misc").arg("which").output().unwrap());
    assert_eq!(which, "personal  (default)\n");

    let whoami = stdout(world.clauset("misc").arg("whoami").output().unwrap());
    assert_eq!(whoami, "team\n");
    let whoami = stdout(
        world
            .clauset("misc")
            .env("CLAUDE_CONFIG_DIR", world.personal_dir())
            .arg("whoami")
            .output()
            .unwrap(),
    );
    assert_eq!(whoami, "personal\n");
}

#[test]
fn use_sets_the_default_keeping_the_config_valid() {
    let world = World::new();
    let refused = world
        .clauset("misc")
        .args(["use", "nobody"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("no account named `nobody`"));

    let out = stdout(
        world
            .clauset("misc")
            .args(["use", "team"])
            .output()
            .unwrap(),
    );
    assert_eq!(out, "New sessions start on team\n");
    let which = stdout(world.clauset("misc").arg("which").output().unwrap());
    assert_eq!(which, "team  (default)\n");
}

#[test]
fn the_overview_shows_usage_a_status_line_recorded() {
    let world = World::new();
    let home = world.home();
    let signed_in = |email: &str| {
        format!(
            r#"{{"oauthAccount":{{"emailAddress":"{email}","organizationName":"{email}'s Organization","organizationType":"claude_max","planDisplayName":"Ultra"}}}}"#
        )
    };
    fs::write(
        home.join(".claude-accounts/personal/.claude.json"),
        signed_in("p@example.com"),
    )
    .unwrap();

    let mut child = world
        .clauset("misc")
        .env("CLAUDE_CONFIG_DIR", world.personal_dir())
        .arg("statusline")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"rate_limits":{"five_hour":{"used_percentage":42,"resets_at":4102444800},"seven_day":{"used_percentage":7,"resets_at":4102444800}}}"#)
        .unwrap();
    assert_eq!(stdout(child.wait_with_output().unwrap()), "personal\n");

    let out = stdout(world.clauset("misc").output().unwrap());
    assert!(
        out.starts_with("~/misc runs on personal  (the default)"),
        "{out}"
    );
    assert!(out.contains("Ultra"), "{out}");
    assert!(out.contains(" 42%") && out.contains("  7%"), "{out}");
    assert!(
        out.contains("p@example.com, personal | usage just now"),
        "{out}"
    );
    assert!(out.contains("not signed in: clauset login team"), "{out}");
}

#[test]
fn status_json_gives_every_account_with_its_usage_and_settings() {
    let world = World::new();
    let home = world.home();
    world.config(
        r#"
        default = "personal"
        warn_at = 90
        chrome = "team"
        [accounts.personal]
        dir = "~/.claude-accounts/personal"
        warn_at = 70
        [accounts.team]
        dir = "~/.claude"
        "#,
    );
    fs::write(
        home.join(".claude-accounts/personal/.claude.json"),
        r#"{"oauthAccount":{"emailAddress":"p@example.com","organizationName":"p@example.com's Organization","organizationType":"claude_max","planDisplayName":"Ultra"},"cachedUsageUtilization":{"fetchedAtMs":500,"utilization":{"five_hour":null,"seven_day":null,"limits":[{"kind":"weekly_all","percent":42,"resets_at":"2099-01-01T00:00:00+00:00","scope":null},{"kind":"weekly_scoped","percent":30,"resets_at":"2099-01-01T00:00:00+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null}}]}}}"#,
    )
    .unwrap();
    let usage = home.join(".claude/clauset/usage");
    fs::create_dir_all(&usage).unwrap();
    fs::write(
        usage.join("personal.json"),
        r#"{"at_ms":1000,"five_hour":{"percent":42.0,"resets_at":4102444800},"seven_day":{"percent":99.0,"resets_at":1000}}"#,
    )
    .unwrap();

    let out = stdout(
        world
            .clauset("misc")
            .args(["status", "--json"])
            .output()
            .unwrap(),
    );
    let status: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(status["version"], 1);
    assert_eq!(status["default"], "personal");
    assert_eq!(status["here"], "personal");
    assert_eq!(status["warn_at"], 90.0);
    assert_eq!(status["chrome"], "team");

    let personal = &status["accounts"][0];
    assert_eq!(personal["name"], "personal");
    assert_eq!(personal["default"], true);
    assert_eq!(personal["hub"], false);
    assert_eq!(personal["signed_in"], true);
    assert_eq!(personal["email"], "p@example.com");
    assert_eq!(personal["org"], "personal");
    assert_eq!(personal["plan"], "Ultra");
    assert_eq!(personal["warn_at"], 70.0);
    assert_eq!(personal["own_warn_at"], 70.0);
    assert_eq!(personal["sessions"], 0);
    assert_eq!(personal["usage"]["at_ms"], 1000);
    assert_eq!(personal["usage"]["five_hour"]["percent"], 42.0);
    assert_eq!(personal["usage"]["five_hour"]["resets_at"], 4102444800_i64);
    assert_eq!(personal["usage"]["five_hour"]["reset_passed"], false);
    assert_eq!(personal["usage"]["seven_day"]["percent"], 0.0);
    assert_eq!(personal["usage"]["seven_day"]["reset_passed"], true);
    assert_eq!(personal["models"][0]["name"], "Fable");
    assert_eq!(personal["models"][0]["percent"], 30.0);
    assert_eq!(personal["models"][0]["resets_at"], 4070908800_i64);
    assert_eq!(personal["models"][0]["reset_passed"], false);
    assert_eq!(personal["models"][0]["at_ms"], 500);

    let team = &status["accounts"][1];
    assert_eq!(team["name"], "team");
    assert_eq!(team["default"], false);
    assert_eq!(team["hub"], true);
    assert_eq!(team["signed_in"], false);
    assert_eq!(team["warn_at"], 90.0);
    assert!(team["own_warn_at"].is_null(), "{team}");
    assert!(team["usage"].is_null(), "{team}");
    assert_eq!(team["models"].as_array().map(Vec::len), Some(0));

    let overview = stdout(world.clauset("misc").output().unwrap());
    assert!(overview.contains("Fable 7d 30% ("), "{overview}");
}

#[test]
fn status_json_fails_without_accounts() {
    let world = World::new();
    fs::remove_file(world.home().join(".claude/clauset/config.toml")).unwrap();
    let output = world
        .clauset("misc")
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("clauset setup"));
}

#[test]
fn chrome_names_the_account_or_clears_it() {
    let world = World::new();
    let config = || -> toml::Table {
        fs::read_to_string(world.home().join(".claude/clauset/config.toml"))
            .unwrap()
            .parse()
            .unwrap()
    };
    let out = stdout(
        world
            .clauset("misc")
            .args(["chrome", "team"])
            .output()
            .unwrap(),
    );
    assert!(
        out.starts_with("Every account reaches Claude in Chrome through team"),
        "{out}"
    );
    assert_eq!(config()["chrome"].as_str(), Some("team"));

    let refused = world
        .clauset("misc")
        .args(["chrome", "nobody"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(config()["chrome"].as_str(), Some("team"));

    stdout(
        world
            .clauset("misc")
            .args(["chrome", "off"])
            .output()
            .unwrap(),
    );
    assert!(!config().contains_key("chrome"));
}

#[test]
fn a_warning_goes_through_the_menu_bar_app_while_it_runs() {
    let world = World::new();
    let home = world.home();
    world.config(
        r#"
        default = "personal"
        warn_at = 50
        [accounts.personal]
        dir = "~/.claude-accounts/personal"
        [accounts.team]
        dir = "~/.claude"
        "#,
    );
    for tool in ["open", "osascript"] {
        let fake = home.join("fake").join(tool);
        fs::write(&fake, "#!/bin/sh\necho \"$0 $*\" >> \"$HOME/notified\"\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let cross = |resets_at: i64| {
        let mut child = world
            .clauset("misc")
            .env("CLAUDE_CONFIG_DIR", world.personal_dir())
            .arg("statusline")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = format!(
            r#"{{"rate_limits":{{"five_hour":{{"used_percentage":60,"resets_at":{resets_at}}}}}}}"#
        );
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        stdout(child.wait_with_output().unwrap());
        fs::read_to_string(home.join("notified")).unwrap()
    };

    let said = cross(4102444800);
    assert!(
        said.contains("/osascript ") && !said.contains("/open "),
        "{said}"
    );
    assert!(
        said.contains("end run -- Personal is at 60%"),
        "the text follows `--`, so osascript never reads it as an option: {said}"
    );

    fs::write(
        home.join(".claude/clauset/app.pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    let said = cross(4102444801);
    assert_eq!(said.matches("/osascript ").count(), 1, "{said}");
    assert!(
        !said.contains("/open "),
        "nothing opens a URL: a web page could open one too: {said}"
    );
    let inbox = home.join(".claude/clauset/notify");
    let left: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(
        fs::read_to_string(&left[0])
            .unwrap()
            .starts_with("Personal is at 60% of its 5-hour limit"),
        "{left:?}"
    );
    assert_eq!(
        fs::metadata(&inbox).unwrap().permissions().mode() & 0o777,
        0o700,
        "only its owner leaves notifications there"
    );
}

#[test]
fn remove_takes_an_account_out_of_the_config_and_keeps_its_directory() {
    let world = World::new();
    let home = world.home();
    world.config(
        r#"
        default = "work"
        chrome = "browser"
        [accounts.personal]
        dir = "~/.claude-accounts/personal"
        [accounts.work]
        dir = "~/.claude-accounts/work"
        [accounts.browser]
        dir = "~/.claude-accounts/browser"
        [accounts.team]
        dir = "~/.claude"
        "#,
    );
    let usage = home.join(".claude/clauset/usage");
    fs::create_dir_all(&usage).unwrap();
    for file in ["personal.json", "personal.warned.json"] {
        fs::write(usage.join(file), "{}").unwrap();
    }
    let refused = |account: &str| {
        let output = world
            .clauset("misc")
            .args(["remove", account])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{account}");
        String::from_utf8_lossy(&output.stderr).into_owned()
    };
    assert!(refused("work").contains("is the default"));
    assert!(refused("browser").contains("`chrome` names"));
    assert!(refused("team").contains("is the hub"));
    assert!(refused("nobody").contains("no account named"));

    let out = stdout(
        world
            .clauset("misc")
            .args(["remove", "personal"])
            .output()
            .unwrap(),
    );
    assert!(
        out.contains("clauset add personal --dir ~/.claude-accounts/personal"),
        "{out}"
    );
    let config: toml::Table = fs::read_to_string(home.join(".claude/clauset/config.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let accounts = config["accounts"].as_table().unwrap();
    assert!(!accounts.contains_key("personal"));
    assert_eq!(accounts.len(), 3);
    assert!(home.join(".claude-accounts/personal").is_dir());
    assert_eq!(usage.read_dir().unwrap().count(), 0);
}

#[test]
fn login_signs_the_hub_in_again_once_no_session_runs_on_it() {
    let world = World::new();
    let home = world.home();
    fs::write(
        home.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"hub@example.com","organizationName":"Acme","organizationUuid":"org-1","organizationType":"claude_team"},"mcpServers":{}}"#,
    )
    .unwrap();
    let out = stdout(
        world
            .clauset("misc")
            .args(["login", "team"])
            .output()
            .unwrap(),
    );
    assert!(out.contains("ARGS:auth login"), "{out}");
    assert!(out.contains("CONFIG_DIR:unset"), "{out}");
    assert!(out.contains("team: signed in as hub@example.com"), "{out}");

    fs::write(home.join("fake/claude"), REGISTERED_CLAUDE).unwrap();
    let session = start_session(&world, "busy", "team", "idle");
    let pids = registered(&world, 1);
    let output = world
        .clauset("misc")
        .args(["login", "team"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("1 session runs on team, the hub"), "{said}");

    end(pids["busy"]);
    session.wait_with_output().unwrap();
}

#[test]
fn remove_refuses_an_account_sessions_run_on() {
    let world = World::new();
    let home = world.home();
    world.config(
        r#"
        default = "team"
        [accounts.personal]
        dir = "~/.claude-accounts/personal"
        [accounts.team]
        dir = "~/.claude"
        "#,
    );
    fs::write(home.join("fake/claude"), REGISTERED_CLAUDE).unwrap();
    let session = start_session(&world, "busy", "personal", "idle");
    let pids = registered(&world, 1);

    let output = world
        .clauset("misc")
        .args(["remove", "personal"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("1 session runs on personal"), "{said}");

    end(pids["busy"]);
    session.wait_with_output().unwrap();
}

#[test]
fn add_takes_in_a_signed_in_config_dir_without_a_login() {
    let world = World::new();
    let home = world.home();
    let old = home.join("old-profile");
    fs::create_dir_all(old.join("skills/mine")).unwrap();
    fs::write(
        old.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"o@example.com","organizationName":"Acme","organizationUuid":"org-1","organizationType":"claude_team"}}"#,
    )
    .unwrap();

    let out = stdout(
        world
            .clauset("misc")
            .args(["add", "acme", "--dir", "../old-profile"])
            .output()
            .unwrap(),
    );
    assert!(out.contains("acme: o@example.com, Acme (Team)"), "{out}");
    assert!(
        home.join(".claude/skills/mine").is_dir(),
        "the profile's skill moved into the hub"
    );
    assert_eq!(
        fs::read_link(old.join("skills")).unwrap(),
        home.join(".claude/skills")
    );
    let config = fs::read_to_string(home.join(".claude/clauset/config.toml")).unwrap();
    assert!(
        config.contains(
            "[accounts.acme]\ndir = \"~/old-profile\"\nemail = \"o@example.com\"\norg = \"org-1\""
        ),
        "{config}"
    );
}

#[test]
fn setup_reports_its_steps_and_applies_one_without_asking() {
    let world = World::new();
    let home = world.home();
    let state = || -> serde_json::Value {
        let out = stdout(
            world
                .clauset("misc")
                .args(["setup", "--json"])
                .output()
                .unwrap(),
        );
        serde_json::from_str(&out).unwrap()
    };
    let apply = |args: &[&str]| {
        stdout(
            world
                .clauset("misc")
                .arg("setup")
                .args(args)
                .output()
                .unwrap(),
        )
    };

    let before = state();
    assert_eq!(before["claude"], true);
    assert_eq!(before["hub"]["account"], "team");
    assert_eq!(before["hub"]["signed_in"], false);
    assert_eq!(before["steps"]["shim"]["state"], "done");
    assert_eq!(before["steps"]["hooks"]["state"], "todo");
    assert_eq!(
        before["steps"]["hooks"]["missing"],
        serde_json::json!(["context", "limit"])
    );
    assert_eq!(before["steps"]["statusline"]["state"], "todo");
    assert_eq!(before["steps"]["skills"]["state"], "todo");
    assert_eq!(
        before["steps"]["skills"]["missing"],
        serde_json::json!(["clauset", "accounts"])
    );

    apply(&["--apply", "hooks"]);
    apply(&["--apply", "statusline"]);
    apply(&["--apply", "skills"]);
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join(".claude/settings.json")).unwrap())
            .unwrap();
    let command = |value: &serde_json::Value| value.as_str().unwrap_or_default().to_owned();
    assert!(command(&settings["statusLine"]["command"]).ends_with("clauset statusline"));
    assert!(
        command(&settings["hooks"]["SessionStart"][0]["hooks"][0]["command"])
            .ends_with("clauset hook context")
    );
    let after = state();
    for step in ["hooks", "statusline", "skills"] {
        assert_eq!(after["steps"][step]["state"], "done", "{step}");
    }

    let skill = home.join(".claude/skills/accounts/SKILL.md");
    fs::write(&skill, "a version of my own").unwrap();
    let changed = state();
    assert_eq!(changed["steps"]["skills"]["state"], "differs");
    assert_eq!(
        changed["steps"]["skills"]["differ"],
        serde_json::json!(["accounts"])
    );
    apply(&["--apply", "skills"]);
    assert_eq!(fs::read_to_string(&skill).unwrap(), "a version of my own");
    apply(&["--apply", "skills", "--replace"]);
    assert_eq!(state()["steps"]["skills"]["state"], "done");
}

#[test]
fn setup_applies_the_shim_with_one_line_in_the_shell_startup_file() {
    let world = World::new();
    let home = world.home();
    let setup = |args: &[&str]| {
        stdout(
            world
                .clauset("misc")
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", home.join("fake").display()),
                )
                .env("SHELL", "/bin/zsh")
                .arg("setup")
                .args(args)
                .output()
                .unwrap(),
        )
    };
    let state: serde_json::Value = serde_json::from_str(&setup(&["--json"])).unwrap();
    assert_eq!(state["version"], 1);
    assert_eq!(state["steps"]["shim"]["state"], "todo");
    assert_eq!(state["steps"]["shim"]["file"], "~/.zshrc");

    let refused = world
        .clauset("misc")
        .args(["setup", "--apply", "shim", "--replace"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("--replace goes with `--apply skills`")
    );

    setup(&["--apply", "shim"]);
    setup(&["--apply", "shim"]);
    assert!(
        home.join(".claude/clauset/bin/claude")
            .symlink_metadata()
            .is_ok()
    );
    assert_eq!(
        fs::read_link(home.join(".claude/clauset/bin/clauset")).unwrap(),
        fs::read_link(home.join(".claude/clauset/bin/claude")).unwrap(),
        "the command stands beside the shim, for an install that put it nowhere on PATH"
    );
    let rc = fs::read_to_string(home.join(".zshrc")).unwrap();
    assert_eq!(rc.matches("clauset/bin").count(), 1, "{rc}");
}

#[test]
fn setup_writes_the_path_clauset_was_started_from() {
    let world = World::new();
    let home = world.home();
    fs::create_dir_all(home.join("brew/bin")).unwrap();
    let linked = home.join("brew/bin/clauset");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_clauset"), &linked).unwrap();
    for step in ["shim", "hooks"] {
        let applied = world
            .command(&linked, "misc")
            .env("SHELL", "/bin/zsh")
            .args(["setup", "--apply", step])
            .output()
            .unwrap();
        assert!(applied.status.success(), "{step}");
    }

    assert_eq!(
        fs::read_link(home.join(".claude/clauset/bin/claude")).unwrap(),
        linked,
        "Homebrew deletes the folder a link like this points into on its next upgrade"
    );
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(
        settings.contains("~/brew/bin/clauset hook context"),
        "{settings}"
    );

    let through_the_link = world
        .command(&home.join(".claude/clauset/bin/clauset"), "misc")
        .env("SHELL", "/bin/zsh")
        .args(["setup", "--apply", "shim"])
        .output()
        .unwrap();
    assert!(through_the_link.status.success());
    assert_eq!(
        fs::read_link(home.join(".claude/clauset/bin/claude")).unwrap(),
        linked,
        "started through its own link, clauset keeps what that link points to"
    );
}

#[test]
fn setup_leaves_a_status_line_of_your_own_to_you() {
    let world = World::new();
    let home = world.home();
    fs::write(
        home.join(".claude/settings.json"),
        r#"{"statusLine":{"type":"command","command":"~/bin/line.sh"}}"#,
    )
    .unwrap();
    let out = stdout(
        world
            .clauset("misc")
            .args(["setup", "--json"])
            .output()
            .unwrap(),
    );
    let state: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(state["steps"]["statusline"]["state"], "manual");
    assert_eq!(state["steps"]["statusline"]["command"], "~/bin/line.sh");

    let output = world
        .clauset("misc")
        .args(["setup", "--apply", "statusline"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("statusline"));
    let settings = fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("~/bin/line.sh"), "{settings}");
}

#[test]
fn without_a_config_the_shim_is_silent_and_clauset_points_at_setup() {
    let world = World::new();
    fs::remove_file(world.home().join(".claude/clauset/config.toml")).unwrap();
    let output = world.shim("misc").arg("z").output().unwrap();
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout(output);
    assert!(
        out.contains("ARGS:z") && out.contains("MARKER:unset"),
        "{out}"
    );

    let out = stdout(world.clauset("misc").output().unwrap());
    assert_eq!(out, "No accounts yet. Run `clauset setup`.\n");
}

#[test]
fn add_takes_in_a_signed_out_dir_before_the_login() {
    let world = World::new();
    let home = world.home();
    let old = home.join("signed-out");
    fs::create_dir_all(old.join("skills/mine")).unwrap();
    fs::write(old.join("settings.json"), "{\"own\":true}").unwrap();
    fs::write(old.join(".claude.json"), "{}").unwrap();
    fs::write(home.join(".claude/settings.json"), "{}").unwrap();

    // The fake `claude auth login` writes no sign-in, so the login fails
    // after the directory was taken in.
    let output = world
        .clauset("misc")
        .args(["add", "old", "--dir", "../signed-out"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("wrote no sign-in"));
    assert!(home.join(".claude/skills/mine").is_dir());
    assert_eq!(
        fs::read_link(old.join("settings.json")).unwrap(),
        home.join(".claude/settings.json")
    );
    assert_eq!(
        fs::read_to_string(old.join("settings.json.before-clauset")).unwrap(),
        "{\"own\":true}"
    );
}

#[test]
fn hooks_report_a_broken_config_and_stay_silent_without_one() {
    let world = World::new();
    world.config("default = ");
    let output = world
        .clauset("misc")
        .args(["hook", "context"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());

    fs::remove_file(world.home().join(".claude/clauset/config.toml")).unwrap();
    let output = world
        .clauset("misc")
        .args(["hook", "context"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success() && output.stdout.is_empty() && output.stderr.is_empty());
}

#[test]
fn add_refuses_a_directory_that_is_no_config_dir() {
    let world = World::new();
    let home = world.home();
    let project = home.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("Cargo.toml"), "[package]").unwrap();

    let output = world
        .clauset("project")
        .args(["add", "work", "--dir", "."])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no Claude config dir"));
    assert!(project.join("src").is_dir() && project.join("Cargo.toml").is_file());
    assert!(!home.join(".claude/src").exists());

    let output = world
        .clauset("misc")
        .args(["add", "home", "--dir", ".."])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("holds your home directory"));

    let config = fs::read_to_string(home.join(".claude/clauset/config.toml")).unwrap();
    assert!(
        !config.contains("[accounts.work]") && !config.contains("[accounts.home]"),
        "{config}"
    );
}

#[test]
fn warn_sets_a_threshold_for_all_or_one_account() {
    let world = World::new();
    let config = || -> toml::Table {
        fs::read_to_string(world.home().join(".claude/clauset/config.toml"))
            .unwrap()
            .parse()
            .unwrap()
    };

    let out = stdout(world.clauset("misc").args(["warn", "80"]).output().unwrap());
    assert!(
        out.starts_with("every account warns at 80% of a limit\n"),
        "{out}"
    );
    assert!(out.contains("does not run `clauset statusline`"), "{out}");
    fs::write(
        world.home().join(".claude/settings.json"),
        r#"{"statusLine":{"type":"command","command":"~/.local/bin/clauset statusline"}}"#,
    )
    .unwrap();
    let out = stdout(world.clauset("misc").args(["warn", "80"]).output().unwrap());
    assert_eq!(out, "every account warns at 80% of a limit\n");
    assert_eq!(config()["warn_at"].as_integer(), Some(80));
    stdout(
        world
            .clauset("misc")
            .args(["warn", "60%", "--account", "team"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        config()["accounts"]["team"]["warn_at"].as_integer(),
        Some(60)
    );

    let refused = world
        .clauset("misc")
        .args(["warn", "120"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let refused = world
        .clauset("misc")
        .args(["warn", "50", "--account", "nobody"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&refused.stderr).contains("no account named `nobody`"));

    stdout(
        world
            .clauset("misc")
            .args(["warn", "off"])
            .output()
            .unwrap(),
    );
    stdout(
        world
            .clauset("misc")
            .args(["warn", "off", "--account", "team"])
            .output()
            .unwrap(),
    );
    assert!(!config().contains_key("warn_at"));
    assert!(config()["accounts"]["team"].get("warn_at").is_none());
}

/// A fake `claude` that registers a session, asks clauset to move it, and
/// waits to be ended; resumed, it prints what it was started with.
const SWITCHING_CLAUDE: &str = r#"#!/bin/sh
case "$*" in
  *--resume*)
    echo "RESUMED:$*"
    echo "CONFIG_DIR:${CLAUDE_CONFIG_DIR-unset}"
    echo "MARKER:$CLAUSET_ACCOUNT"
    exit 0;;
esac
mkdir -p "$HOME/.claude/sessions"
printf '{"pid":%s,"sessionId":"s-1","status":"%s"}' $$ "${STATUS:-idle}" > "$HOME/.claude/sessions/$$.json"
CLAUDE_PID=$$ CLAUDE_CODE_SESSION_ID=s-1 "$CLAUSET_BIN" switch personal
[ "$STATUS" = busy ] && exit 0
exec sleep 20
"#;

/// A session that asks to move and then to stay.
const CANCELLING_CLAUDE: &str = r#"#!/bin/sh
case "$*" in *--resume*) echo "RESUMED:$*"; exit 0;; esac
mkdir -p "$HOME/.claude/sessions"
entry="$HOME/.claude/sessions/$$.json"
printf '{"pid":%s,"sessionId":"s-1","status":"busy"}' $$ > "$entry"
CLAUDE_PID=$$ CLAUDE_CODE_SESSION_ID=s-1 "$CLAUSET_BIN" switch personal
CLAUDE_PID=$$ CLAUDE_CODE_SESSION_ID=s-1 "$CLAUSET_BIN" switch team
printf '{"pid":%s,"sessionId":"s-1","status":"idle"}' $$ > "$entry"
sleep 2
"#;

/// A session that asks to move, then starts a background shell that runs
/// for the rest of its life.
const LATE_SHELL_CLAUDE: &str = r#"#!/bin/sh
case "$*" in *--resume*) echo "RESUMED:$*"; exit 0;; esac
mkdir -p "$HOME/.claude/sessions" "$HOME/.claude/shell-snapshots"
entry="$HOME/.claude/sessions/$$.json"
printf '{"pid":%s,"sessionId":"s-1","status":"busy"}' $$ > "$entry"
CLAUDE_PID=$$ CLAUDE_CODE_SESSION_ID=s-1 "$CLAUSET_BIN" switch personal
echo 'sleep 20' > "$HOME/.claude/shell-snapshots/snapshot-zsh-1.sh"
/bin/sh "$HOME/.claude/shell-snapshots/snapshot-zsh-1.sh" &
printf '{"pid":%s,"sessionId":"s-1","status":"idle"}' $$ > "$entry"
sleep 2
printf '{"pid":%s,"sessionId":"s-1","status":"busy"}' $$ > "$entry"
kill $!
"#;

/// An idle session with a background Bash shell, which Claude Code starts
/// from a shell snapshot in its config dir, that stops after two seconds.
const SHELL_CLAUDE: &str = r#"#!/bin/sh
case "$*" in *--resume*) echo "RESUMED:$*"; exit 0;; esac
mkdir -p "$HOME/.claude/sessions" "$HOME/.claude/shell-snapshots"
printf '{"pid":%s,"sessionId":"s-1","status":"idle"}' $$ > "$HOME/.claude/sessions/$$.json"
echo 'sleep 2' > "$HOME/.claude/shell-snapshots/snapshot-zsh-1.sh"
/bin/sh "$HOME/.claude/shell-snapshots/snapshot-zsh-1.sh" &
CLAUDE_PID=$$ CLAUDE_CODE_SESSION_ID=s-1 "$CLAUSET_BIN" switch personal
exec sleep 20
"#;

fn default_account(world: &World) -> String {
    let text = fs::read_to_string(world.home().join(".claude/clauset/config.toml")).unwrap();
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("default = "))
        .unwrap()
        .trim_matches('"')
        .to_owned()
}

#[test]
fn a_session_moves_to_another_account_in_the_same_terminal() {
    let world = World::new();
    let home = world.home();
    let config = home.join(".claude/clauset/config.toml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        text.replace(r#"default = "personal""#, r#"default = "team""#),
    )
    .unwrap();
    let fake = home.join("fake/claude");
    fs::write(&fake, SWITCHING_CLAUDE).unwrap();

    // `script` gives the shim a terminal, which is where it supervises.
    let mut command = world.command(Path::new("/usr/bin/script"), "work");
    command
        .args(["-q", "/dev/null"])
        .arg(home.join("shim/claude"))
        .arg("--dangerously-skip-permissions")
        .env("CLAUSET_BIN", env!("CARGO_BIN_EXE_clauset"))
        .env("CLAUSET_ACCOUNT", "team")
        .stdin(Stdio::null());
    let out = stdout(command.output().unwrap());

    assert!(out.contains("This session moves to personal"), "{out}");
    assert!(out.contains("RESUMED:"), "{out}");
    assert!(
        out.contains("--dangerously-skip-permissions --resume s-1"),
        "{out}"
    );
    assert!(
        out.contains(&format!("CONFIG_DIR:{}", world.personal_dir())),
        "{out}"
    );
    assert!(out.contains("MARKER:personal"), "{out}");
    assert!(
        out.contains("New sessions still start on team: `clauset use personal` changes that."),
        "a session that leaves the default account is told the default stayed: {out}"
    );
    assert_eq!(
        default_account(&world),
        "team",
        "moving a session leaves the default alone"
    );
    assert!(
        !home
            .join(".claude/clauset/switch")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
}

#[test]
fn a_session_that_ends_itself_before_it_moves_stays_ended() {
    let world = World::new();
    let home = world.home();
    fs::write(home.join("fake/claude"), SWITCHING_CLAUDE).unwrap();

    let mut command = world.command(Path::new("/usr/bin/script"), "work");
    command
        .args(["-q", "/dev/null"])
        .arg(home.join("shim/claude"))
        .env("CLAUSET_BIN", env!("CARGO_BIN_EXE_clauset"))
        .env("CLAUSET_ACCOUNT", "team")
        .env("STATUS", "busy")
        .stdin(Stdio::null());
    let out = stdout(command.output().unwrap());

    assert!(out.contains("This session moves to personal"), "{out}");
    assert!(!out.contains("RESUMED:"), "{out}");
    assert!(
        !home
            .join(".claude/clauset/switch")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
}

#[test]
fn a_session_with_background_shells_moves_once_they_stop() {
    let world = World::new();
    let home = world.home();
    fs::write(home.join("fake/claude"), SHELL_CLAUDE).unwrap();

    let mut command = world.command(Path::new("/usr/bin/script"), "work");
    command
        .args(["-q", "/dev/null"])
        .arg(home.join("shim/claude"))
        .env("CLAUSET_BIN", env!("CARGO_BIN_EXE_clauset"))
        .env("CLAUSET_ACCOUNT", "team")
        .stdin(Stdio::null());
    let out = stdout(command.output().unwrap());

    assert!(
        out.contains("This session moves to personal once the current turn ends and its background shell has stopped"),
        "{out}"
    );
    assert!(
        out.contains("`clauset switch team` cancels the move"),
        "{out}"
    );
    assert!(out.contains("RESUMED:--resume s-1"), "{out}");
    assert_eq!(default_account(&world), "personal");
}

#[test]
fn a_switch_without_the_supervisor_is_refused() {
    let world = World::new();
    let output = world
        .clauset("misc")
        .args(["switch", "team"])
        .env("CLAUDE_PID", std::process::id().to_string())
        .env("CLAUDE_CODE_SESSION_ID", "s-1")
        .env("CLAUSET_ACCOUNT", "personal")
        .env("CLAUDE_CONFIG_DIR", world.personal_dir())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("without clauset's supervisor"), "{err}");
    assert!(err.contains("clauset run team --resume s-1"), "{err}");
}

#[test]
fn switching_to_the_current_account_cancels_a_move() {
    let world = World::new();
    let home = world.home();
    fs::write(home.join("fake/claude"), CANCELLING_CLAUDE).unwrap();

    let mut command = world.command(Path::new("/usr/bin/script"), "work");
    command
        .args(["-q", "/dev/null"])
        .arg(home.join("shim/claude"))
        .env("CLAUSET_BIN", env!("CARGO_BIN_EXE_clauset"))
        .env("CLAUSET_ACCOUNT", "team")
        .stdin(Stdio::null());
    let out = stdout(command.output().unwrap());

    assert!(out.contains("the move it asked for is cancelled"), "{out}");
    assert!(!out.contains("RESUMED:"), "{out}");
    assert_eq!(
        default_account(&world),
        "personal",
        "cancelling a move leaves the default alone too"
    );
}

#[test]
fn a_move_waits_while_a_background_shell_runs() {
    let world = World::new();
    let home = world.home();
    fs::write(home.join("fake/claude"), LATE_SHELL_CLAUDE).unwrap();

    let mut command = world.command(Path::new("/usr/bin/script"), "work");
    command
        .args(["-q", "/dev/null"])
        .arg(home.join("shim/claude"))
        .env("CLAUSET_BIN", env!("CARGO_BIN_EXE_clauset"))
        .env("CLAUSET_ACCOUNT", "team")
        .stdin(Stdio::null());
    let out = stdout(command.output().unwrap());

    assert!(out.contains("This session moves to personal"), "{out}");
    assert!(!out.contains("RESUMED:"), "{out}");
}

#[test]
fn switch_outside_a_supervised_session_names_the_way_back() {
    let world = World::new();
    let output = world
        .clauset("misc")
        .args(["switch", "team"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inside a Claude Code session"));
    assert_eq!(default_account(&world), "personal");
}

/// A session that registers under the name `$SESSION`, as Claude Code does,
/// and waits to be ended; resumed, it says where.
const REGISTERED_CLAUDE: &str = r#"#!/bin/sh
case "$*" in *--resume*) echo "RESUMED:$* ON:$CLAUSET_ACCOUNT"; exit 0;; esac
mkdir -p "$HOME/.claude/sessions"
printf '{"pid":%s,"sessionId":"%s","name":"%s","cwd":"%s","status":"%s","procStart":"%s"}' \
  $$ "$SESSION" "$SESSION" "$PWD" "${STATUS:-idle}" "$(TZ=UTC ps -o lstart= -p $$)" \
  > "$HOME/.claude/sessions/$$.json.tmp"
mv "$HOME/.claude/sessions/$$.json.tmp" "$HOME/.claude/sessions/$$.json"
exec sleep 20
"#;

/// Starts a supervised session named `name` on `account` in a terminal, as
/// a `claude` started inside a session on that account.
fn start_session(world: &World, name: &str, account: &str, status: &str) -> std::process::Child {
    let mut command = world.command(Path::new("/usr/bin/script"), "work");
    if account == "personal" {
        command.env("CLAUDE_CONFIG_DIR", world.personal_dir());
    }
    command
        .args(["-q", "/dev/null"])
        .arg(world.home().join("shim/claude"))
        .env("CLAUSET_ACCOUNT", account)
        .env("SESSION", name)
        .env("STATUS", status)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

/// The pid of each registered session, by name, once `count` have registered.
fn registered(world: &World, count: usize) -> std::collections::HashMap<String, u32> {
    let dir = world.home().join(".claude/sessions");
    for _ in 0..100 {
        let sessions: std::collections::HashMap<String, u32> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| {
                let entry: serde_json::Value =
                    serde_json::from_str(&fs::read_to_string(entry.path()).ok()?).ok()?;
                Some((
                    entry["name"].as_str()?.to_owned(),
                    u32::try_from(entry["pid"].as_u64()?).ok()?,
                ))
            })
            .collect();
        if sessions.len() == count {
            return sessions;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("{count} sessions never registered");
}

fn set_idle(world: &World, pid: u32) {
    let path = world.home().join(format!(".claude/sessions/{pid}.json"));
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        text.replace(r#""status":"busy""#, r#""status":"idle""#),
    )
    .unwrap();
}

fn end(pid: u32) {
    Command::new("kill").arg(pid.to_string()).status().unwrap();
}

#[test]
fn a_switch_of_all_sessions_moves_each_supervised_one_and_names_the_rest() {
    let world = World::new();
    let home = world.home();
    fs::write(home.join("fake/claude"), REGISTERED_CLAUDE).unwrap();
    let here = start_session(&world, "here", "team", "busy");
    let idle = start_session(&world, "idle", "team", "idle");
    let stays = start_session(&world, "stays", "personal", "idle");
    let mut loose = world
        .command(&home.join("fake/claude"), "work")
        .env("CLAUSET_ACCOUNT", "team")
        .env("SESSION", "loose")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let pids = registered(&world, 4);

    let out = stdout(
        world
            .clauset("misc")
            .args(["switch", "personal", "--all"])
            .env("CLAUDE_PID", pids["here"].to_string())
            .env("CLAUDE_CODE_SESSION_ID", "here")
            .output()
            .unwrap(),
    );
    assert!(out.contains("These move from team to personal"), "{out}");
    assert!(
        out.contains("- here in ~/work (this session), busy: it moves when its turn ends"),
        "{out}"
    );
    assert!(out.contains("- idle in ~/work\n"), "{out}");
    assert!(
        out.contains("- loose in ~/work: `clauset run personal --resume loose`"),
        "{out}"
    );
    assert!(!out.contains("stays"), "{out}");
    assert!(
        !out.contains("New sessions"),
        "team is not the default, so there is nothing to say about new sessions: {out}"
    );
    assert_eq!(default_account(&world), "personal");

    let idle = idle.wait_with_output().unwrap();
    assert!(
        stdout(idle).contains("RESUMED:--resume idle ON:personal"),
        "the idle session moves without waiting"
    );
    set_idle(&world, pids["here"]);
    let out = stdout(here.wait_with_output().unwrap());
    assert!(out.contains("RESUMED:--resume here ON:personal"), "{out}");

    end(pids["stays"]);
    let stays = stays.wait_with_output().unwrap();
    assert!(!String::from_utf8_lossy(&stays.stdout).contains("RESUMED"));
    loose.kill().unwrap();
    loose.wait().unwrap();
    for dir in ["switch", "supervised"] {
        let left = home.join(".claude/clauset").join(dir).read_dir().unwrap();
        assert_eq!(left.count(), 0, "{dir}");
    }
}

#[test]
fn a_switch_of_all_sessions_off_the_default_account_leaves_the_default() {
    let world = World::new();
    fs::write(world.home().join("fake/claude"), REGISTERED_CLAUDE).unwrap();
    let idle = start_session(&world, "idle", "personal", "idle");
    registered(&world, 1);

    let out = stdout(
        world
            .clauset("misc")
            .args(["switch", "team", "--all", "--from", "personal"])
            .output()
            .unwrap(),
    );
    assert!(out.contains("These move from personal to team"), "{out}");
    assert!(
        out.ends_with("New sessions still start on personal: `clauset use team` changes that.\n"),
        "{out}"
    );
    assert_eq!(default_account(&world), "personal");

    let out = stdout(idle.wait_with_output().unwrap());
    assert!(out.contains("RESUMED:--resume idle ON:team"), "{out}");
}

#[test]
fn a_second_switch_of_all_sessions_leaves_the_waiting_ones_as_they_were() {
    let world = World::new();
    let home = world.home();
    fs::write(home.join("fake/claude"), REGISTERED_CLAUDE).unwrap();
    let busy = start_session(&world, "busy", "team", "busy");
    let pid = registered(&world, 1)["busy"];
    let request = home.join(format!(".claude/clauset/switch/{pid}.json"));

    let switch = || {
        stdout(
            world
                .clauset("misc")
                .args(["switch", "personal", "--all", "--from", "team"])
                .output()
                .unwrap(),
        )
    };
    assert!(switch().contains("- busy in ~/work, busy"));
    let first = fs::read_to_string(&request).unwrap();
    assert!(switch().contains("- busy in ~/work, busy"));
    assert_eq!(fs::read_to_string(&request).unwrap(), first);
    assert_eq!(
        home.join(".claude/clauset/switch")
            .read_dir()
            .unwrap()
            .count(),
        1
    );

    set_idle(&world, pid);
    let out = stdout(busy.wait_with_output().unwrap());
    assert_eq!(out.matches("RESUMED:").count(), 1, "{out}");
    assert!(out.contains("RESUMED:--resume busy ON:personal"), "{out}");
}

#[test]
fn a_switch_of_all_sessions_needs_an_account_to_leave() {
    let world = World::new();
    let output = world
        .clauset("misc")
        .args(["switch", "team", "--all"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--from <account>"));

    let output = world
        .clauset("misc")
        .args(["switch", "team", "--all", "--from", "team"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains("already run there"));

    let out = stdout(
        world
            .clauset("misc")
            .args(["switch", "team", "--all", "--from", "personal"])
            .output()
            .unwrap(),
    );
    assert!(out.contains("No session runs on personal"), "{out}");
    assert_eq!(default_account(&world), "personal");
}

#[test]
fn help_leaves_out_the_plumbing() {
    let world = World::new();
    let out = stdout(world.clauset("misc").arg("--help").output().unwrap());
    for hidden in ["hook", "statusline", "init", "whoami"] {
        assert!(!out.contains(&format!("\n  {hidden} ")), "{hidden}: {out}");
    }
    assert!(out.contains("\n  switch "), "{out}");
}
