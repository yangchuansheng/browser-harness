use std::collections::BTreeMap;
use std::fs;
#[cfg(target_os = "macos")]
use std::io::Write;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use bh_daemon::{
    already_running, log_tail, pending_generation, publish_pending_generation, spawn_lock,
    stop_best_effort, DaemonConfig, PendingGeneration,
};
use bh_discovery::{
    default_browser_profiles, inspect_marker, remote_debugging_toggle_profiles,
    remote_debugging_user_enabled, supported_browser_running,
};
use bh_remote::{
    auth_status, browser_use_api_key, clear_browser_use_auth, store_browser_use_api_key,
    BrowserUseClient,
};
use serde_json::{json, Value};

const INSPECT_REOPEN_TTL: Duration = Duration::from_secs(180);
const CHROME_INSPECT_URL: &str = "chrome://inspect/#remote-debugging";
const MAC_APPROVE_ACCESSIBILITY_DETAIL: &str =
    "allow the app launching browser-harness (for example Terminal, iTerm, or Codex) in System Settings > Privacy & Security > Accessibility";

#[cfg(target_os = "macos")]
const MAC_APPROVE_CHROME_ROOT_SUFFIX: &str = "Library/Application Support/Google/Chrome";

#[cfg(target_os = "macos")]
const MAC_APPROVE_APPLESCRIPT: &str = r#"using terms from application "System Events"
    on clickAllow(nodeRef)
        try
            if (role of nodeRef as text) is "AXButton" and ¬
                (description of nodeRef as text) is "Allow" then
                perform action "AXPress" of nodeRef
                return true
            end if
        end try
        try
            repeat with childRef in UI elements of nodeRef
                if my clickAllow(childRef) then return true
            end repeat
        end try
        return false
    end clickAllow
end using terms from

set resultText to "not-found"
tell application "System Events"
    if exists process "Google Chrome" then
        tell process "Google Chrome"
            repeat with w in windows
                try
                    repeat with s in sheets of w
                        if (name of s as text) is "Allow remote debugging?" then
                            if my clickAllow(s) then
                                set resultText to "ready"
                                exit repeat
                            end if
                        end if
                    end repeat
                end try
                if resultText is "ready" then exit repeat
            end repeat
        end tell
    end if
end tell
return resultText
"#;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<i32, String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(
            "usage: bhctl <auth|mac-approve|create-browser|list-browsers|stop-browser|list-cloud-profiles|resolve-profile-name|list-local-profiles|sync-local-profile|daemon-alive|ensure-daemon|restart-daemon|stop-daemon>"
                .to_string(),
        );
    };

    let output = match command.as_str() {
        "auth" => auth_output(args.collect::<Vec<_>>())?,
        "mac-approve" => {
            if args.next().is_some() {
                println!("usage: browser-harness mac-approve");
                return Ok(2);
            }
            let (status, detail) = mac_approve_output();
            if let Some(detail) = detail {
                println!("{status}: {detail}");
            } else {
                println!("{status}");
            }
            return Ok(if status == "ready" { 0 } else { 1 });
        }
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
                if !should_show_remote_live_view()? {
                    object.remove("liveUrl");
                }
            }
            browser
        }
        "list-browsers" => {
            let client = browser_use_client()?;
            let options = parse_list_browsers_options(read_json_stdin()?)?;
            client
                .list_browsers(options.page_size, options.page_number)
                .await?
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
        "doctor" => doctor_output(args.next().as_deref()),
        "ensure-daemon" => ensure_daemon_output()?,
        "restart-daemon" | "stop-daemon" => restart_daemon_output(args.next().as_deref())?,
        other => {
            return Err(format!(
                "unknown bhctl command {:?}; expected auth, mac-approve, create-browser, list-browsers, stop-browser, list-cloud-profiles, resolve-profile-name, list-local-profiles, sync-local-profile, daemon-alive, ensure-daemon, restart-daemon, or stop-daemon",
                other
            ))
        }
    };

    let stdout =
        serde_json::to_string(&output).map_err(|err| format!("serialize bhctl output: {err}"))?;
    println!("{stdout}");
    Ok(0)
}

#[derive(Debug, PartialEq)]
struct EnsureDaemonOptions {
    name: Option<String>,
    wait_seconds: Option<f64>,
    env: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq)]
struct ListBrowsersOptions {
    page_size: usize,
    page_number: usize,
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
    let api_key = browser_use_api_key()?;
    Ok(BrowserUseClient::new(api_key))
}

fn should_show_remote_live_view() -> Result<bool, String> {
    let Ok(raw) = std::env::var("BH_OPEN_LIVE_URL") else {
        return Ok(true);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => {
            Err("BH_OPEN_LIVE_URL must be one of: 1, true, yes, on, 0, false, no, off".to_string())
        }
    }
}

fn auth_output(args: Vec<String>) -> Result<Value, String> {
    match args.first().map(String::as_str).unwrap_or("status") {
        "status" => Ok(auth_status()),
        "login" => {
            let api_key = if args.get(1).map(String::as_str) == Some("--api-key-stdin") {
                let mut stdin = String::new();
                io::stdin()
                    .read_to_string(&mut stdin)
                    .map_err(|err| format!("read API key from stdin: {err}"))?;
                stdin
            } else {
                let payload = read_json_stdin()?.unwrap_or_else(|| json!({}));
                payload
                    .get("apiKey")
                    .or_else(|| payload.get("api_key"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| {
                        "usage: bhctl auth login --api-key-stdin OR JSON {\"apiKey\":\"bu_...\"} on stdin"
                            .to_string()
                    })?
            };
            store_browser_use_api_key(&api_key)
        }
        "logout" => clear_browser_use_auth(),
        other => Err(format!(
            "unknown bhctl auth command {:?}; expected status, login, or logout",
            other
        )),
    }
}

fn daemon_alive_output(name: Option<&str>) -> Value {
    let config = daemon_config(name);
    json!({
        "alive": already_running(&config),
        "name": config.name,
    })
}

fn doctor_output(name: Option<&str>) -> Value {
    let config = daemon_config(name);
    let alive = already_running(&config);
    json!({
        "schemaVersion": 1,
        "healthy": alive,
        "daemon": {
            "name": config.name,
            "alive": alive,
            "browserKind": config.browser_kind(),
        }
    })
}

fn mac_approve_output() -> (&'static str, Option<String>) {
    #[cfg(not(target_os = "macos"))]
    {
        (
            "unsupported",
            Some("mac-approve is only available on macOS".to_string()),
        )
    }

    #[cfg(target_os = "macos")]
    {
        let config = daemon_config(None);
        if already_running(&config) {
            return ("ready", None);
        }
        let chrome_root = default_browser_profiles()
            .into_iter()
            .find(|profile| profile.ends_with(MAC_APPROVE_CHROME_ROOT_SUFFIX));
        let enabled_profiles = remote_debugging_toggle_profiles();
        if !mac_approve_toggle_enabled(chrome_root.as_deref(), &enabled_profiles) {
            return (
                "setup-required",
                Some(
                    "first enable \"Allow remote debugging for this browser instance\" at chrome://inspect/#remote-debugging, then run `browser-harness mac-approve` again"
                        .to_string(),
                ),
            );
        }

        let mut child = match Command::new("osascript")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => return ("error", Some(err.to_string())),
        };
        if let Err(err) = child
            .stdin
            .take()
            .ok_or_else(|| "osascript stdin unavailable".to_string())
            .and_then(|mut stdin| {
                stdin
                    .write_all(MAC_APPROVE_APPLESCRIPT.as_bytes())
                    .map_err(|err| err.to_string())
            })
        {
            let _ = child.kill();
            let _ = child.wait();
            return ("error", Some(err));
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return (
                        "accessibility-required",
                        Some(MAC_APPROVE_ACCESSIBILITY_DETAIL.to_string()),
                    );
                }
                Err(err) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return ("error", Some(err.to_string()));
                }
            }
        }

        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(err) => return ("error", Some(err.to_string())),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let ready_after = stdout.trim() == "not-found" && already_running(&config);
        classify_mac_approve_output(output.status.success(), &stdout, &stderr, ready_after)
    }
}

fn mac_approve_toggle_enabled(chrome_root: Option<&Path>, enabled_profiles: &[PathBuf]) -> bool {
    chrome_root.is_some_and(|root| enabled_profiles.iter().any(|profile| profile == root))
}

fn classify_mac_approve_output(
    success: bool,
    stdout: &str,
    stderr: &str,
    daemon_ready: bool,
) -> (&'static str, Option<String>) {
    if !success {
        let detail = stderr.trim();
        let lower = detail.to_ascii_lowercase();
        if lower.contains("not authorized") || lower.contains("assistive") {
            return (
                "accessibility-required",
                Some(MAC_APPROVE_ACCESSIBILITY_DETAIL.to_string()),
            );
        }
        return (
            "error",
            Some(if detail.is_empty() {
                "osascript failed".to_string()
            } else {
                detail.to_string()
            }),
        );
    }

    match stdout.trim() {
        "ready" => ("ready", None),
        "not-found" if daemon_ready => ("ready", None),
        "not-found" => (
            "not-found",
            Some(
                "retry the browser command and run `browser-harness mac-approve` when the prompt appears"
                    .to_string(),
            ),
        ),
        status => (
            "error",
            Some(format!(
                "unexpected osascript result: {}",
                if status.is_empty() { "<empty>" } else { status }
            )),
        ),
    }
}

fn parse_list_browsers_options(payload: Option<Value>) -> Result<ListBrowsersOptions, String> {
    let payload = payload.unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return Err("list-browsers payload must be a JSON object".to_string());
    };

    let page_size =
        parse_positive_usize_field(object.get("pageSize"), "list-browsers pageSize")?.unwrap_or(20);
    let page_number =
        parse_positive_usize_field(object.get("pageNumber"), "list-browsers pageNumber")?
            .unwrap_or(1);

    Ok(ListBrowsersOptions {
        page_size,
        page_number,
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

    let is_local = ensure_daemon_uses_local_browser(&options);
    let (mut child, pending) = {
        let _lock = spawn_lock(&config)?;
        if already_running(&config) {
            return Ok(json!({"ok": true, "alreadyRunning": true, "name": config.name}));
        }
        if let Some(generation) = pending_generation(&config)? {
            (None, generation)
        } else {
            let mut command = daemon_launch_command()?;
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if options.name.is_some() {
                command.env("BU_NAME", &config.name);
            }
            command.envs(options.env.iter());

            let child = command
                .spawn()
                .map_err(|err| format!("spawn daemon: {err}"))?;
            let generation = publish_pending_generation(&config, child.id() as i32)?;
            (Some(child), generation)
        }
    };
    wait_for_daemon(
        &config,
        is_local,
        options.wait_seconds,
        &mut child,
        &pending,
    )
}

fn wait_for_daemon(
    config: &DaemonConfig,
    local: bool,
    explicit_wait: Option<f64>,
    child: &mut Option<std::process::Child>,
    pending: &PendingGeneration,
) -> Result<Value, String> {
    let started = Instant::now();
    let mut deadline = Some(started + Duration::from_secs_f64(explicit_wait.unwrap_or(60.0)));
    let mut hinted = !local;
    loop {
        if already_running(config) {
            return Ok(json!({"ok": true, "alreadyRunning": false, "name": config.name}));
        }
        let died = child
            .as_mut()
            .map(|child| child.try_wait().map(|status| status.is_some()))
            .transpose()
            .map_err(|err| format!("wait for daemon startup: {err}"))?
            .unwrap_or_else(|| pending_generation(config).ok().flatten() != Some(pending.clone()));
        let message = log_tail(config).unwrap_or_default();
        if local && needs_chrome_permission_popup(&message) {
            if explicit_wait.is_none() {
                deadline = None;
            }
            if !hinted && started.elapsed() > Duration::from_secs(2) {
                let command = if config.name == "default" {
                    "browser-harness mac-approve".to_string()
                } else {
                    format!("BU_NAME={} browser-harness mac-approve", config.name)
                };
                let action = if cfg!(target_os = "macos") {
                    format!("run `{command}` in another shell or click Allow")
                } else {
                    "click Allow".to_string()
                };
                eprintln!("browser-harness: Chrome is asking \"Allow remote debugging?\" -- {action} to continue.");
                hinted = true;
            }
        }
        if died {
            if local && chrome_not_running(&message) && launch_browser() {
                let boot_deadline = Instant::now() + Duration::from_secs(15);
                while Instant::now() < boot_deadline && !supported_browser_running() {
                    thread::sleep(Duration::from_millis(300));
                }
            }
            if local && needs_chrome_remote_debugging_prompt(&message) {
                let _ = open_chrome_inspect_once();
            }
            return Err(if message.starts_with("handshake-wait") {
                "permission-blocked: the pending Chrome connection ended before approval; browser-harness did not retry or create another connection.".to_string()
            } else {
                daemon_startup_error(
                    message,
                    local,
                    local.then(remote_debugging_user_enabled).flatten(),
                )
            });
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            if local && needs_chrome_permission_popup(&message) {
                return Err("permission-blocked: Chrome's Allow popup is still open and the pending daemon was left running. Approve that exact popup; browser-harness did not retry or create another connection.".to_string());
            }
            return Err(if message.is_empty() {
                format!(
                    "daemon {} didn't come up -- check {}",
                    config.name,
                    config.paths().log.display()
                )
            } else {
                message
            });
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn ensure_daemon_uses_local_browser(options: &EnsureDaemonOptions) -> bool {
    ["BU_BROWSER_ID", "BU_CDP_WS", "BU_CDP_URL"]
        .iter()
        .all(|key| match options.env.get(*key) {
            Some(value) => value.trim().is_empty(),
            None => std::env::var(key)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true),
        })
}

fn chrome_not_running(message: &str) -> bool {
    message.to_ascii_lowercase().contains("chrome-not-running")
}

fn daemon_startup_error(
    message: String,
    is_local: bool,
    remote_debugging_enabled: Option<bool>,
) -> String {
    let is_remote_debugging_failure = message.starts_with("handshake-wait")
        || message.contains("CDP WS handshake")
        || message.contains("DevToolsActivePort")
        || message.contains("remote-debugging");
    if !is_local || !is_remote_debugging_failure {
        return message;
    }

    match remote_debugging_enabled {
        Some(true) => "permission-blocked: Chrome remote debugging is enabled and its DevTools port is live, but the Allow popup was not accepted -- click Allow in Chrome, then retry".to_string(),
        Some(false) => "remote debugging is turned off for this browser instance -- enable chrome://inspect/#remote-debugging (tick \"Allow remote debugging for this browser instance\")".to_string(),
        None => message,
    }
}

fn needs_chrome_permission_popup(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("permission-blocked") || lower.starts_with("handshake-wait")
}

fn needs_chrome_remote_debugging_prompt(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("devtoolsactiveport")
        || lower.contains("enable chrome://inspect")
        || lower.contains("not live yet")
        || lower.contains("remote debugging is turned off")
        || (lower.contains("cdp ws")
            && (lower.contains("403")
                || lower.contains("opening handshake")
                || lower.contains("timed out")
                || lower.contains("timeout")))
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
    let api_key = browser_use_api_key()?;
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
    cmd.env("BROWSER_USE_API_KEY", api_key);
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
        .transpose()?;
    if wait_seconds.is_some_and(|wait| !wait.is_finite() || wait <= 0.0) {
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

fn parse_positive_usize_field(value: Option<&Value>, label: &str) -> Result<Option<usize>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let raw = value
        .as_u64()
        .ok_or_else(|| format!("{label} must be a positive integer"))?;
    let parsed =
        usize::try_from(raw).map_err(|_| format!("{label} is too large for this platform"))?;
    if parsed == 0 {
        return Err(format!("{label} must be >= 1"));
    }
    Ok(Some(parsed))
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

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct BrowserLaunchSpec {
    profile_fragment: &'static str,
    macos_app: &'static str,
    posix_commands: &'static [&'static str],
    windows_target: Option<&'static str>,
}

const BROWSER_LAUNCH_SPECS: &[BrowserLaunchSpec] = &[
    BrowserLaunchSpec {
        profile_fragment: "chrome canary",
        macos_app: "Google Chrome Canary",
        posix_commands: &["google-chrome-canary"],
        windows_target: Some("chrome"),
    },
    BrowserLaunchSpec {
        profile_fragment: "chromium",
        macos_app: "Chromium",
        posix_commands: &["chromium", "chromium-browser"],
        windows_target: Some("chromium"),
    },
    BrowserLaunchSpec {
        profile_fragment: "chrome",
        macos_app: "Google Chrome",
        posix_commands: &["google-chrome-stable", "google-chrome"],
        windows_target: Some("chrome"),
    },
    BrowserLaunchSpec {
        profile_fragment: "edge",
        macos_app: "Microsoft Edge",
        posix_commands: &["microsoft-edge", "microsoft-edge-stable"],
        windows_target: Some("msedge"),
    },
    BrowserLaunchSpec {
        profile_fragment: "brave-origin",
        macos_app: "Brave Origin",
        posix_commands: &["brave-browser", "brave"],
        windows_target: Some("brave"),
    },
    BrowserLaunchSpec {
        profile_fragment: "brave",
        macos_app: "Brave Browser",
        posix_commands: &["brave-browser", "brave"],
        windows_target: Some("brave"),
    },
    BrowserLaunchSpec {
        profile_fragment: "arc",
        macos_app: "Arc",
        posix_commands: &[],
        windows_target: None,
    },
    BrowserLaunchSpec {
        profile_fragment: "dia",
        macos_app: "Dia",
        posix_commands: &[],
        windows_target: None,
    },
    BrowserLaunchSpec {
        profile_fragment: "comet",
        macos_app: "Comet",
        posix_commands: &[],
        windows_target: None,
    },
];

const DEFAULT_BROWSER_LAUNCH: BrowserLaunchSpec = BrowserLaunchSpec {
    profile_fragment: "",
    macos_app: "Google Chrome",
    posix_commands: &[
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
    ],
    windows_target: Some("chrome"),
};

fn browser_launch_spec(base: Option<&Path>) -> &'static BrowserLaunchSpec {
    let Some(base) = base else {
        return &DEFAULT_BROWSER_LAUNCH;
    };
    let mut tail = base
        .components()
        .rev()
        .take(2)
        .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    tail.reverse();
    let tail = tail.join("/");
    BROWSER_LAUNCH_SPECS
        .iter()
        .find(|spec| tail.contains(spec.profile_fragment))
        .unwrap_or(&DEFAULT_BROWSER_LAUNCH)
}

fn profile_directory_args(base: &Path) -> Vec<String> {
    let last_used = fs::read_to_string(base.join("Local State"))
        .ok()
        .and_then(|contents| serde_json::from_str::<Value>(&contents).ok())
        .and_then(|state| {
            state
                .get("profile")
                .and_then(Value::as_object)
                .and_then(|profile| profile.get("last_used"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Default".to_string());
    if base.join(&last_used).is_dir() {
        vec![format!("--profile-directory={last_used}")]
    } else {
        Vec::new()
    }
}

fn expand_home_path(raw: &str) -> PathBuf {
    if raw == "~" {
        return user_home_dir();
    }
    if let Some(relative) = raw.strip_prefix("~/") {
        return user_home_dir().join(relative);
    }
    PathBuf::from(raw)
}

fn user_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn launch_browser() -> bool {
    for key in ["BH_CHROME_PATH", "CHROME_PATH"] {
        let Some(raw) = std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let path = expand_home_path(&raw);
        if path.is_file()
            && Command::new(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        {
            return true;
        }
    }

    let enabled_profiles = remote_debugging_toggle_profiles();
    let base = enabled_profiles.into_iter().next().or_else(|| {
        default_browser_profiles()
            .into_iter()
            .find(|base| base.join("Local State").is_file())
    });
    let spec = browser_launch_spec(base.as_deref());
    let profile_args = base
        .as_deref()
        .map(profile_directory_args)
        .unwrap_or_default();

    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.args(["-a", spec.macos_app]);
        if !profile_args.is_empty() {
            command.arg("--args").args(&profile_args);
        }
        let launched = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if launched {
            return true;
        }
        if spec.macos_app != DEFAULT_BROWSER_LAUNCH.macos_app {
            return Command::new("open")
                .args(["-a", DEFAULT_BROWSER_LAUNCH.macos_app])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
        }
        false
    }

    #[cfg(target_os = "windows")]
    {
        let target = spec
            .windows_target
            .unwrap_or(DEFAULT_BROWSER_LAUNCH.windows_target.unwrap_or("chrome"));
        Command::new("cmd")
            .args(["/c", "start", "", target])
            .args(&profile_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let commands = if spec.posix_commands.is_empty() {
            DEFAULT_BROWSER_LAUNCH.posix_commands
        } else {
            spec.posix_commands
        };
        for candidate in commands {
            let Ok(output) = Command::new("which").arg(candidate).output() else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            let executable = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(str::to_string);
            if let Some(executable) = executable {
                if Command::new(executable)
                    .args(&profile_args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .is_ok()
                {
                    return true;
                }
            }
        }
        false
    }
}

fn inspect_marker_is_fresh(marker: &Path, now: SystemTime) -> bool {
    let Ok(modified) = fs::metadata(marker).and_then(|metadata| metadata.modified()) else {
        return false;
    };
    now.duration_since(modified)
        .map(|age| age < INSPECT_REOPEN_TTL)
        .unwrap_or(true)
}

fn open_chrome_inspect() -> bool {
    #[cfg(target_os = "macos")]
    {
        let result = Command::new("osascript")
            .args([
                "-e",
                "tell application \"Google Chrome\" to activate",
                "-e",
                &format!(
                    "tell application \"Google Chrome\" to open location \"{CHROME_INSPECT_URL}\""
                ),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if result.is_ok_and(|status| status.success()) {
            return true;
        }
        return Command::new("open")
            .arg(CHROME_INSPECT_URL)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/c", "start", "", CHROME_INSPECT_URL])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        [
            ("xdg-open", Vec::new()),
            ("gio", vec!["open"]),
            ("sensible-browser", Vec::new()),
        ]
        .into_iter()
        .any(|(program, prefix_args)| {
            Command::new(program)
                .args(prefix_args)
                .arg(CHROME_INSPECT_URL)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        })
    }
}

fn open_chrome_inspect_once() -> bool {
    let marker = inspect_marker();
    if inspect_marker_is_fresh(&marker, SystemTime::now()) {
        return true;
    }
    if !open_chrome_inspect() {
        return false;
    }
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(marker, b"");
    true
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

    if let Ok(current_exe) = std::env::current_exe() {
        let sibling = current_exe.with_file_name("bhd");
        if sibling.is_file() {
            return Ok(Command::new(sibling));
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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{
        browser_launch_spec, chrome_not_running, classify_mac_approve_output,
        daemon_launch_command, daemon_startup_error, doctor_output,
        ensure_daemon_uses_local_browser, inspect_marker_is_fresh, mac_approve_toggle_enabled,
        needs_chrome_permission_popup, needs_chrome_remote_debugging_prompt,
        parse_created_profile_id, parse_ensure_daemon_options, parse_list_browsers_options,
        profile_directory_args, profile_use_sync_command, resolve_daemon_name,
        should_show_remote_live_view, EnsureDaemonOptions, ListBrowsersOptions,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "browser-harness-bhctl-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
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
    fn doctor_output_has_versioned_health_shape() {
        let report = doctor_output(Some("missing-test-daemon"));
        assert_eq!(report["schemaVersion"], 1);
        assert_eq!(report["daemon"]["name"], "missing-test-daemon");
        assert_eq!(report["healthy"], false);
    }

    #[test]
    fn remote_live_view_setting_is_strict() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("BH_OPEN_LIVE_URL");
        std::env::set_var("BH_OPEN_LIVE_URL", "off");
        assert!(!should_show_remote_live_view().unwrap());
        std::env::set_var("BH_OPEN_LIVE_URL", "invalid");
        assert!(should_show_remote_live_view().is_err());
        if let Some(previous) = previous {
            std::env::set_var("BH_OPEN_LIVE_URL", previous);
        } else {
            std::env::remove_var("BH_OPEN_LIVE_URL");
        }
    }

    #[test]
    fn mac_approve_classification_matches_cli_contract() {
        assert_eq!(
            classify_mac_approve_output(true, "ready\n", "", false),
            ("ready", None)
        );
        assert_eq!(
            classify_mac_approve_output(true, "not-found\n", "", true),
            ("ready", None)
        );
        assert_eq!(
            classify_mac_approve_output(false, "", "not authorized to send Apple events", false).0,
            "accessibility-required"
        );
        assert_eq!(
            classify_mac_approve_output(true, "unexpected\n", "", false),
            (
                "error",
                Some("unexpected osascript result: unexpected".to_string())
            )
        );
    }

    #[test]
    fn mac_approve_setup_accepts_only_the_google_chrome_toggle() {
        let chrome_root = PathBuf::from("/tmp/Google/Chrome");
        let edge_root = PathBuf::from("/tmp/Microsoft Edge");
        let enabled_profiles = [edge_root, chrome_root.clone()];

        assert!(mac_approve_toggle_enabled(
            Some(&chrome_root),
            &enabled_profiles
        ));
        assert!(!mac_approve_toggle_enabled(
            Some(&chrome_root),
            &enabled_profiles[..1]
        ));
        assert!(!mac_approve_toggle_enabled(None, &enabled_profiles));
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
                wait_seconds: Some(12.5),
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
    fn ensure_daemon_omitted_wait_stays_distinct_from_explicit_wait() {
        assert_eq!(
            parse_ensure_daemon_options(None).unwrap().wait_seconds,
            None
        );
        assert_eq!(
            parse_ensure_daemon_options(Some(json!({"wait": 60})))
                .unwrap()
                .wait_seconds,
            Some(60.0)
        );
    }

    #[test]
    fn ensure_daemon_browser_kind_honors_env_overrides() {
        let local = EnsureDaemonOptions {
            name: None,
            wait_seconds: None,
            env: ["BU_BROWSER_ID", "BU_CDP_WS", "BU_CDP_URL"]
                .into_iter()
                .map(|key| (key.to_string(), String::new()))
                .collect(),
        };
        assert!(ensure_daemon_uses_local_browser(&local));

        let mut remote = local;
        remote.env.insert(
            "BU_CDP_WS".to_string(),
            "wss://example.test/devtools/browser/abc".to_string(),
        );
        assert!(!ensure_daemon_uses_local_browser(&remote));
    }

    #[test]
    fn daemon_startup_classifiers_cover_recovery_paths() {
        assert!(chrome_not_running(
            "fatal: chrome-not-running: no supported browser"
        ));
        assert!(needs_chrome_permission_popup(
            "fatal: permission-blocked: click Allow in Chrome"
        ));
        assert!(needs_chrome_permission_popup(
            "handshake-wait: click Allow in Chrome"
        ));
        assert!(needs_chrome_remote_debugging_prompt(
            "fatal: CDP WS opening handshake timed out after 45s"
        ));
        assert!(needs_chrome_remote_debugging_prompt(
            "fatal: DevToolsActivePort not found"
        ));
        assert!(needs_chrome_remote_debugging_prompt(
            "remote debugging is turned off for this browser instance"
        ));
    }

    #[test]
    fn daemon_startup_error_preserves_remote_debugging_state_messages() {
        let message = "fatal: CDP WS handshake failed".to_string();
        assert!(daemon_startup_error(message.clone(), true, Some(true))
            .starts_with("permission-blocked:"));
        assert!(daemon_startup_error(message.clone(), true, Some(false))
            .starts_with("remote debugging is turned off"));
        assert_eq!(daemon_startup_error(message.clone(), true, None), message);
        assert_eq!(
            daemon_startup_error(message.clone(), false, Some(true)),
            message
        );
    }

    #[test]
    fn profile_directory_args_uses_last_profile_and_default_fallback() {
        let base = temp_dir("profile-args");
        fs::create_dir_all(base.join("Profile 2")).unwrap();
        fs::write(
            base.join("Local State"),
            r#"{"profile":{"last_used":"Profile 2"}}"#,
        )
        .unwrap();

        assert_eq!(
            profile_directory_args(&base),
            vec!["--profile-directory=Profile 2".to_string()]
        );

        fs::write(base.join("Local State"), "{}").unwrap();
        fs::create_dir_all(base.join("Default")).unwrap();
        assert_eq!(
            profile_directory_args(&base),
            vec!["--profile-directory=Default".to_string()]
        );
        fs::remove_dir_all(base.join("Default")).unwrap();
        assert!(profile_directory_args(&base).is_empty());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn browser_launch_spec_matches_profile_family() {
        assert_eq!(
            browser_launch_spec(Some(Path::new(
                "profiles/Library/Application Support/Microsoft Edge"
            )))
            .macos_app,
            "Microsoft Edge"
        );
        assert_eq!(
            browser_launch_spec(Some(Path::new(
                "profiles/Library/Application Support/Google/Chrome Canary"
            )))
            .macos_app,
            "Google Chrome Canary"
        );
        assert_eq!(
            browser_launch_spec(Some(Path::new(
                "profiles/Library/Application Support/BraveSoftware/Brave-Origin"
            )))
            .macos_app,
            "Brave Origin"
        );
    }

    #[test]
    fn inspect_marker_freshness_honors_recent_marker() {
        let dir = temp_dir("inspect-marker");
        let marker = dir.join("inspect-opened");
        fs::write(&marker, "").unwrap();

        assert!(inspect_marker_is_fresh(&marker, SystemTime::now()));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parse_list_browsers_options_uses_defaults() {
        assert_eq!(
            parse_list_browsers_options(None).unwrap(),
            ListBrowsersOptions {
                page_size: 20,
                page_number: 1,
            }
        );
    }

    #[test]
    fn parse_list_browsers_options_reads_payload_values() {
        assert_eq!(
            parse_list_browsers_options(Some(json!({
                "pageSize": 50,
                "pageNumber": 3,
            })))
            .unwrap(),
            ListBrowsersOptions {
                page_size: 50,
                page_number: 3,
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
