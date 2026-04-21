use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use bh_daemon::{already_running, log_tail, stop_best_effort, DaemonConfig};
use bh_remote::BrowserUseClient;
use serde_json::{json, Value};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(
            "usage: bhctl <create-browser|stop-browser|list-cloud-profiles|resolve-profile-name|list-local-profiles|sync-local-profile|daemon-alive|ensure-daemon|restart-daemon|stop-daemon>"
                .to_string(),
        );
    };

    let output = match command.as_str() {
        "create-browser" => {
            let client = browser_use_client()?;
            let mut payload = read_json_stdin()?.unwrap_or_else(|| json!({}));
            normalize_create_browser_payload(&client, &mut payload).await?;
            let mut browser = client.create_browser(&payload).await?;
            let cdp_url = browser
                .get("cdpUrl")
                .and_then(Value::as_str)
                .ok_or_else(|| "Browser Use response missing cdpUrl".to_string())?;
            let cdp_ws_url = client.cdp_ws_from_url(cdp_url).await?;
            if let Some(object) = browser.as_object_mut() {
                object.insert("cdpWsUrl".to_string(), Value::String(cdp_ws_url));
            }
            browser
        }
        "stop-browser" => {
            let client = browser_use_client()?;
            let browser_id = args
                .next()
                .ok_or_else(|| "usage: bhctl stop-browser <browser-id>".to_string())?;
            client.stop_browser(&browser_id).await?;
            json!({"ok": true, "browserId": browser_id})
        }
        "list-cloud-profiles" => {
            let client = browser_use_client()?;
            Value::Array(client.list_cloud_profiles().await?)
        }
        "resolve-profile-name" => {
            let client = browser_use_client()?;
            let profile_name = args
                .next()
                .ok_or_else(|| "usage: bhctl resolve-profile-name <profile-name>".to_string())?;
            let profile_id = client.resolve_profile_name(&profile_name).await?;
            json!({"profileId": profile_id})
        }
        "list-local-profiles" => list_local_profiles()?,
        "sync-local-profile" => sync_local_profile()?,
        "daemon-alive" => daemon_alive_output(args.next().as_deref()),
        "ensure-daemon" => ensure_daemon_output()?,
        "restart-daemon" | "stop-daemon" => restart_daemon_output(args.next().as_deref())?,
        other => {
            return Err(format!(
                "unknown bhctl command {:?}; expected create-browser, stop-browser, list-cloud-profiles, resolve-profile-name, list-local-profiles, sync-local-profile, daemon-alive, ensure-daemon, restart-daemon, or stop-daemon",
                other
            ))
        }
    };

    let stdout =
        serde_json::to_string(&output).map_err(|err| format!("serialize bhctl output: {err}"))?;
    println!("{stdout}");
    Ok(())
}

#[derive(Debug, PartialEq)]
struct EnsureDaemonOptions {
    name: Option<String>,
    wait_seconds: f64,
    env: BTreeMap<String, String>,
}

fn read_json_stdin() -> Result<Option<Value>, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|err| format!("read bhctl stdin: {err}"))?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|err| format!("parse bhctl stdin JSON: {err}"))
}

fn browser_use_client() -> Result<BrowserUseClient, String> {
    let api_key = std::env::var("BROWSER_USE_API_KEY")
        .map_err(|_| "BROWSER_USE_API_KEY missing -- see .env.example".to_string())?;
    Ok(BrowserUseClient::new(api_key))
}

fn daemon_alive_output(name: Option<&str>) -> Value {
    let config = daemon_config(name);
    json!({
        "alive": already_running(&config),
        "name": config.name,
    })
}

fn ensure_daemon_output() -> Result<Value, String> {
    let options = parse_ensure_daemon_options(read_json_stdin()?)?;
    let config = daemon_config(options.name.as_deref());
    if already_running(&config) {
        return Ok(json!({
            "ok": true,
            "alreadyRunning": true,
            "name": config.name,
        }));
    }

    let mut command = daemon_launch_command()?;
    command.stdout(Stdio::null()).stderr(Stdio::null());
    if options.name.is_some() {
        command.env("BU_NAME", &config.name);
    }
    command.envs(options.env.iter());

    let mut child = command
        .spawn()
        .map_err(|err| format!("spawn daemon: {err}"))?;
    let deadline = Instant::now() + Duration::from_secs_f64(options.wait_seconds);
    while Instant::now() < deadline {
        if already_running(&config) {
            return Ok(json!({
                "ok": true,
                "alreadyRunning": false,
                "name": config.name,
            }));
        }
        if child
            .try_wait()
            .map_err(|err| format!("wait for daemon startup: {err}"))?
            .is_some()
        {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }

    let message = log_tail(&config).unwrap_or_else(|| {
        format!(
            "daemon {} didn't come up -- check {}",
            config.name,
            config.paths().log.display()
        )
    });
    Err(message)
}

fn restart_daemon_output(name: Option<&str>) -> Result<Value, String> {
    let config = daemon_config(name);
    stop_best_effort(&config)?;
    Ok(json!({
        "ok": true,
        "name": config.name,
    }))
}

async fn normalize_create_browser_payload(
    client: &BrowserUseClient,
    payload: &mut Value,
) -> Result<(), String> {
    let Some(object) = payload.as_object_mut() else {
        return Err("create-browser payload must be a JSON object".to_string());
    };
    let profile_name = object
        .get("profileName")
        .and_then(Value::as_str)
        .map(str::to_string);
    if profile_name.is_none() {
        return Ok(());
    }
    if object.contains_key("profileId") {
        return Err("pass profileName OR profileId, not both".to_string());
    }
    let profile_id = client.resolve_profile_name(&profile_name.unwrap()).await?;
    object.remove("profileName");
    object.insert("profileId".to_string(), Value::String(profile_id));
    Ok(())
}

fn list_local_profiles() -> Result<Value, String> {
    ensure_profile_use_available()?;
    let output = Command::new("profile-use")
        .args(["list", "--json"])
        .output()
        .map_err(|err| format!("run profile-use list: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "profile-use list failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("parse profile-use list output: {err}"))
}

fn sync_local_profile() -> Result<Value, String> {
    ensure_profile_use_available()?;
    if std::env::var("BROWSER_USE_API_KEY").is_err() {
        return Err("BROWSER_USE_API_KEY missing".to_string());
    }
    let payload = read_json_stdin()?
        .ok_or_else(|| "sync-local-profile requires a JSON payload on stdin".to_string())?;
    let profile_name = payload
        .get("profileName")
        .and_then(Value::as_str)
        .ok_or_else(|| "sync-local-profile payload missing profileName".to_string())?;

    let mut cmd = profile_use_sync_command(
        profile_name,
        payload.get("browser").and_then(Value::as_str),
        payload.get("cloudProfileId").and_then(Value::as_str),
        payload
            .get("includeDomains")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default(),
        payload
            .get("excludeDomains")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default(),
    );
    let output = cmd
        .output()
        .map_err(|err| format!("run profile-use sync: {err}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(format!(
            "profile-use sync failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    let cloud_profile_id =
        if let Some(existing_id) = payload.get("cloudProfileId").and_then(Value::as_str) {
            existing_id.to_string()
        } else {
            parse_created_profile_id(&stdout).ok_or_else(|| {
                format!(
                    "profile-use did not report a profile UUID (stdout: {})",
                    stdout.trim()
                )
            })?
        };

    Ok(json!({
        "cloudProfileId": cloud_profile_id,
        "stdout": stdout,
        "stderr": stderr,
    }))
}

fn ensure_profile_use_available() -> Result<(), String> {
    let status = Command::new("profile-use").arg("--help").status();
    match status {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(
            "profile-use not installed -- curl -fsSL https://browser-use.com/profile.sh | sh"
                .to_string(),
        ),
        Err(err) => Err(format!("probe profile-use: {err}")),
    }
}

fn daemon_config(name: Option<&str>) -> DaemonConfig {
    DaemonConfig::new(resolve_daemon_name(name))
}

fn resolve_daemon_name(name: Option<&str>) -> String {
    name.filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("BU_NAME")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "default".to_string())
}

fn parse_ensure_daemon_options(payload: Option<Value>) -> Result<EnsureDaemonOptions, String> {
    let payload = payload.unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return Err("ensure-daemon payload must be a JSON object".to_string());
    };

    let wait_seconds = object
        .get("wait")
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| "ensure-daemon wait must be a number".to_string())
        })
        .transpose()?
        .unwrap_or(60.0);
    if !wait_seconds.is_finite() || wait_seconds <= 0.0 {
        return Err("ensure-daemon wait must be > 0".to_string());
    }

    let env = parse_env_map(object.get("env"))?;
    Ok(EnsureDaemonOptions {
        name: object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        wait_seconds,
        env,
    })
}

fn parse_env_map(value: Option<&Value>) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let Some(object) = value.as_object() else {
        return Err("ensure-daemon env must be a JSON object".to_string());
    };

    let mut env = BTreeMap::new();
    for (key, value) in object {
        let string_value = value
            .as_str()
            .ok_or_else(|| format!("ensure-daemon env {key:?} must be a string"))?;
        env.insert(key.clone(), string_value.to_string());
    }
    Ok(env)
}

fn daemon_launch_command() -> Result<Command, String> {
    if let Ok(custom) = std::env::var("BU_RUST_DAEMON_BIN") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            let mut command = Command::new(trimmed);
            command.current_dir(repo_root());
            return Ok(command);
        }
    }

    let mut command = Command::new("cargo");
    command
        .args(["run", "--quiet", "--bin", "bhd", "--"])
        .current_dir(workspace_root());
    Ok(command)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn repo_root() -> PathBuf {
    workspace_root()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(workspace_root)
}

fn profile_use_sync_command<'a>(
    profile_name: &'a str,
    browser: Option<&'a str>,
    cloud_profile_id: Option<&'a str>,
    include_domains: Vec<&'a str>,
    exclude_domains: Vec<&'a str>,
) -> Command {
    let mut cmd = Command::new("profile-use");
    cmd.arg("sync").arg("--profile").arg(profile_name);
    if let Some(browser) = browser {
        cmd.arg("--browser").arg(browser);
    }
    if let Some(cloud_profile_id) = cloud_profile_id {
        cmd.arg("--cloud-profile-id").arg(cloud_profile_id);
    }
    for domain in include_domains {
        cmd.arg("--domain").arg(domain);
    }
    for domain in exclude_domains {
        cmd.arg("--exclude-domain").arg(domain);
    }
    cmd
}

fn parse_created_profile_id(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("Profile created:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use serde_json::json;

    use super::{
        daemon_launch_command, parse_created_profile_id, parse_ensure_daemon_options,
        profile_use_sync_command, resolve_daemon_name, EnsureDaemonOptions,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn parse_created_profile_id_finds_uuid_line() {
        let stdout = "hello\nProfile created: 123e4567-e89b-12d3-a456-426614174000\nbye\n";
        assert_eq!(
            parse_created_profile_id(stdout),
            Some("123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn profile_use_sync_command_builds_expected_args() {
        let cmd = profile_use_sync_command(
            "Default",
            Some("Google Chrome"),
            Some("abc"),
            vec!["google.com", "stripe.com"],
            vec!["example.com"],
        );
        let args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "sync",
                "--profile",
                "Default",
                "--browser",
                "Google Chrome",
                "--cloud-profile-id",
                "abc",
                "--domain",
                "google.com",
                "--domain",
                "stripe.com",
                "--exclude-domain",
                "example.com",
            ]
        );
    }

    #[test]
    fn parse_ensure_daemon_options_reads_name_wait_and_env() {
        let options = parse_ensure_daemon_options(Some(json!({
            "name": "remote",
            "wait": 12.5,
            "env": {
                "BU_CDP_WS": "wss://example.test/devtools/page/abc",
                "BU_BROWSER_ID": "browser-123"
            }
        })))
        .unwrap();

        assert_eq!(
            options,
            EnsureDaemonOptions {
                name: Some("remote".to_string()),
                wait_seconds: 12.5,
                env: [
                    ("BU_BROWSER_ID".to_string(), "browser-123".to_string()),
                    (
                        "BU_CDP_WS".to_string(),
                        "wss://example.test/devtools/page/abc".to_string()
                    ),
                ]
                .into_iter()
                .collect(),
            }
        );
    }

    #[test]
    fn resolve_daemon_name_prefers_explicit_value_then_env_then_default() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("BU_NAME");
        std::env::set_var("BU_NAME", "from-env");

        assert_eq!(
            resolve_daemon_name(Some("from-arg")),
            "from-arg".to_string()
        );
        assert_eq!(resolve_daemon_name(None), "from-env".to_string());

        std::env::remove_var("BU_NAME");
        assert_eq!(resolve_daemon_name(None), "default".to_string());

        if let Some(previous) = previous {
            std::env::set_var("BU_NAME", previous);
        } else {
            std::env::remove_var("BU_NAME");
        }
    }

    #[test]
    fn daemon_launch_command_defaults_to_cargo_runner() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("BU_RUST_DAEMON_BIN");
        std::env::remove_var("BU_RUST_DAEMON_BIN");

        let command = daemon_launch_command().unwrap();
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(command.get_program(), "cargo");
        assert_eq!(args, vec!["run", "--quiet", "--bin", "bhd", "--"]);

        if let Some(previous) = previous {
            std::env::set_var("BU_RUST_DAEMON_BIN", previous);
        } else {
            std::env::remove_var("BU_RUST_DAEMON_BIN");
        }
    }
}
