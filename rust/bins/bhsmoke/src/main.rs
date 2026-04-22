use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde_json::{json, Map, Value};

const SCENARIOS: &[&str] = &[
    "remote",
    "actions",
    "tabs",
    "guest-run",
    "guest-serve",
    "persistent-guest",
    "persistent-guest-browser",
    "tab-response-guest",
    "event-waits-guest",
    "raw-cdp-guest",
    "wait-for-load-event",
    "watch-events",
    "wait-for-request",
    "wait-for-response",
    "wait-for-console",
    "wait-for-dialog",
    "set-viewport",
    "screenshot",
    "print-pdf",
    "cookies",
    "wait-for-download",
    "drag",
    "upload-file",
];

const SAMPLE_GUEST_TARGET_URL: &str = "https://example.com/?via=bhrun-guest-sample";
const PERSISTENT_BROWSER_GUEST_TARGET_URL: &str =
    "https://example.com/?via=bhrun-serve-guest-remote-smoke";
const PERSISTENT_BROWSER_GUEST_MARKER: &str = "phase-1";
const TAB_RESPONSE_GUEST_TARGET_URL: &str =
    "https://example.com/?via=bhrun-tab-response-guest-smoke";
const EVENT_WAIT_TOKEN: &str = "bhrun-event-wait";
const EVENT_WATCH_TOKEN_ONE: &str = "bhrun-event-watch-1";
const EVENT_WATCH_TOKEN_TWO: &str = "bhrun-event-watch-2";
const EVENT_CONSOLE_TOKEN: &str = "bhrun-event-console";
const EVENT_DIALOG_TOKEN: &str = "bhrun-event-dialog";
const RAW_CDP_CLI_TOKEN: &str = "bhrun-raw-cdp-cli";
const RAW_CDP_GUEST_TOKEN: &str = "bhrun-raw-cdp-guest";

fn main() {
    match run() {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("serialize smoke report")
            );
        }
        Err(err) if err.is_empty() => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<Value, String> {
    let mut args = env::args_os().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(json!({"ok": true}));
    };
    if is_help_flag(&command) {
        print_usage();
        return Ok(json!({"ok": true}));
    }
    if args.next().is_some() {
        return Err(format!("usage: bhsmoke <{}>", SCENARIOS.join("|")));
    }

    match command.to_string_lossy().as_ref() {
        "remote" => smoke_remote(),
        "actions" => smoke_actions(),
        "tabs" => smoke_tabs(),
        "guest-run" => smoke_guest_run(),
        "guest-serve" => smoke_guest_serve(),
        "persistent-guest" => smoke_persistent_guest(),
        "persistent-guest-browser" => smoke_persistent_guest_browser(),
        "tab-response-guest" => smoke_tab_response_guest(),
        "event-waits-guest" => smoke_event_waits_guest(),
        "raw-cdp-guest" => smoke_raw_cdp_guest(),
        "wait-for-load-event" => smoke_wait_for_load_event(),
        "watch-events" => smoke_watch_events(),
        "wait-for-request" => smoke_wait_for_request(),
        "wait-for-response" => smoke_wait_for_response(),
        "wait-for-console" => smoke_wait_for_console(),
        "wait-for-dialog" => smoke_wait_for_dialog(),
        "set-viewport" => smoke_set_viewport(),
        "screenshot" => smoke_screenshot(),
        "print-pdf" => smoke_print_pdf(),
        "cookies" => smoke_cookies(),
        "wait-for-download" => smoke_wait_for_download(),
        "drag" => smoke_drag(),
        "upload-file" => smoke_upload_file(),
        other => Err(format!(
            "unknown smoke scenario {:?}; expected one of {}",
            other,
            SCENARIOS.join(", ")
        )),
    }
}

fn is_help_flag(value: &OsString) -> bool {
    matches!(value.to_str(), Some("-h" | "--help" | "help"))
}

fn print_usage() {
    eprintln!(
        "usage: bhsmoke <{}>\n\
         notes:\n\
         - repo-local Rust smoke runner for browser-harness\n\
         - remote scenarios require BROWSER_USE_API_KEY\n\
         - local scenarios attach through the Rust daemon via DevToolsActivePort",
        SCENARIOS.join("|")
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrowserMode {
    Local,
    Remote,
}

impl BrowserMode {
    fn as_str(self) -> &'static str {
        match self {
            BrowserMode::Local => "local",
            BrowserMode::Remote => "remote",
        }
    }
}

#[derive(Debug)]
struct SmokeOptions {
    name: String,
    daemon_impl: String,
    browser_mode: BrowserMode,
    remote_timeout_minutes: u64,
    local_wait_seconds: f64,
}

#[derive(Debug)]
struct RemoteBrowser {
    id: String,
}

#[derive(Clone, Copy)]
enum ToolKind {
    Admin,
    Runner,
}

struct CommandOutput {
    stdout: String,
}

fn smoke_remote() -> Result<Value, String> {
    require_remote_api_key()?;
    let options = load_options("remote-smoke", BrowserMode::Remote)?;
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, false, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(name, "https://example.com")?),
        );
        result.insert("after_new_tab".into(), page_info(name)?);
        if result
            .get("after_new_tab")
            .and_then(|value| value.get("url"))
            .and_then(Value::as_str)
            == Some("about:blank")
        {
            return Err("new-tab left the active page at about:blank".to_string());
        }
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("url_via_js".into(), js(name, "location.href")?);
        result.insert(
            "goto_result".into(),
            goto(name, "https://example.com/?via=typed-goto")?,
        );
        result.insert(
            "loaded_after_goto".into(),
            Value::Bool(wait_for_load(name)?),
        );
        result.insert("after_nav".into(), page_info(name)?);
        js(
            name,
            "(()=>{let e=document.querySelector('#codex-dispatch');\
             if(!e){e=document.createElement('input');e.id='codex-dispatch';document.body.appendChild(e)}\
             window.__dispatchKey=null;\
             e.addEventListener('keypress',ev=>window.__dispatchKey={key:ev.key,which:ev.which,type:ev.type},{once:true});\
             return true})()",
        )?;
        dispatch_key(name, "#codex-dispatch", "Enter", "keypress")?;
        result.insert("dispatch_key".into(), js(name, "window.__dispatchKey")?);
        let screenshot_b64 = screenshot_b64(name, true)?;
        let (png, _, _) = decode_png_dimensions(&screenshot_b64)?;
        result.insert("screenshot_size".into(), Value::from(png.len() as u64));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_actions() -> Result<Value, String> {
    let options = load_options("bhrun-actions-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let initial_page = page_info(name)?;
        result.insert("initial_page".into(), initial_page);

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));

        let token = unique_token("bhrun-actions-smoke");
        let target_url = format!("https://example.com/?via=bhrun-actions-smoke&token={token}");
        let wait_payload = json!({
            "daemon_name": name,
            "session_id": session_id,
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child = start_command(
            ToolKind::Runner,
            "wait-for-load-event",
            Some(wait_payload),
            &[],
        )?;
        sleep_ms(500);
        result.insert("goto_result".into(), goto(name, &target_url)?);
        let wait_result = finish_json(wait_child, Duration::from_secs(10))?;
        result.insert("wait_result".into(), wait_result.clone());
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-load-event returned matched=false".to_string());
        }
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-load-event response missing event".to_string())?;
        if event.get("method").and_then(Value::as_str) != Some("Page.loadEventFired") {
            return Err(format!(
                "unexpected load event method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("load event session_id did not match the active session".to_string());
        }

        let page_after_goto = page_info(name)?;
        result.insert("page_after_goto".into(), page_after_goto.clone());
        if page_after_goto.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err("page-info URL did not match the navigation target".to_string());
        }

        let js_href = js(name, "location.href")?;
        result.insert("js_href".into(), js_href.clone());
        if js_href.as_str() != Some(target_url.as_str()) {
            return Err("js(location.href) did not match the navigation target".to_string());
        }

        let js_title = js(name, "document.title")?;
        result.insert("js_title".into(), js_title.clone());
        if !js_title
            .as_str()
            .unwrap_or_default()
            .contains("Example Domain")
        {
            return Err(format!("unexpected document.title from js(): {js_title:?}"));
        }

        result.insert("target_url".into(), Value::String(target_url));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_tabs() -> Result<Value, String> {
    let options = load_options("bhrun-tabs-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let initial_current_tab = current_tab(name)?;
        result.insert("initial_current_tab".into(), initial_current_tab.clone());
        let initial_tabs = list_tabs(name)?;
        result.insert("initial_tabs".into(), Value::Array(initial_tabs.clone()));
        let initial_target_id = required_string_field(&initial_current_tab, "targetId")?;

        let token = unique_token("bhrun-tabs-smoke");
        let target_url = format!("https://example.com/?via=bhrun-tabs-smoke&token={token}");
        let new_target_id = new_tab(name, &target_url)?;
        result.insert(
            "new_tab".into(),
            json!({"target_id": new_target_id.clone()}),
        );

        let current_after_new = current_tab(name)?;
        result.insert("current_after_new".into(), current_after_new.clone());
        if current_after_new.get("targetId").and_then(Value::as_str) != Some(new_target_id.as_str())
        {
            return Err("current-tab did not move to the new tab target".to_string());
        }

        let page_after_new = page_info(name)?;
        result.insert("page_after_new".into(), page_after_new.clone());
        if page_after_new.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err("page-info did not report the new tab URL".to_string());
        }

        let tabs_after_new = list_tabs(name)?;
        result.insert(
            "tabs_after_new".into(),
            Value::Array(tabs_after_new.clone()),
        );
        let target_ids = tabs_after_new
            .iter()
            .filter_map(|tab| tab.get("targetId").and_then(Value::as_str))
            .collect::<std::collections::BTreeSet<_>>();
        if !target_ids.contains(initial_target_id.as_str())
            || !target_ids.contains(new_target_id.as_str())
        {
            return Err("list-tabs did not include both the initial and new targets".to_string());
        }

        let switch_back = switch_tab(name, &initial_target_id)?;
        result.insert("switch_back".into(), switch_back.clone());
        let session_after_switch_back = current_session(name)?;
        result.insert(
            "session_after_switch_back".into(),
            session_after_switch_back.clone(),
        );
        if session_after_switch_back
            .get("session_id")
            .and_then(Value::as_str)
            != switch_back.get("session_id").and_then(Value::as_str)
        {
            return Err(
                "current-session did not match switch-tab result after switching back".to_string(),
            );
        }

        let current_after_switch_back = current_tab(name)?;
        result.insert(
            "current_after_switch_back".into(),
            current_after_switch_back.clone(),
        );
        if current_after_switch_back
            .get("targetId")
            .and_then(Value::as_str)
            != Some(initial_target_id.as_str())
        {
            return Err("current-tab did not move back to the initial target".to_string());
        }

        let page_after_switch_back = page_info(name)?;
        result.insert(
            "page_after_switch_back".into(),
            page_after_switch_back.clone(),
        );
        if let Some(expected_url) = current_after_switch_back.get("url").and_then(Value::as_str) {
            if page_after_switch_back.get("url").and_then(Value::as_str) != Some(expected_url) {
                return Err(
                    "page-info after switch-back did not match the active current-tab URL"
                        .to_string(),
                );
            }
        }

        let switch_forward = switch_tab(name, &new_target_id)?;
        result.insert("switch_forward".into(), switch_forward.clone());
        let session_after_switch_forward = current_session(name)?;
        result.insert(
            "session_after_switch_forward".into(),
            session_after_switch_forward.clone(),
        );
        if session_after_switch_forward
            .get("session_id")
            .and_then(Value::as_str)
            != switch_forward.get("session_id").and_then(Value::as_str)
        {
            return Err(
                "current-session did not match switch-tab result after switching forward"
                    .to_string(),
            );
        }

        let current_after_switch_forward = current_tab(name)?;
        result.insert(
            "current_after_switch_forward".into(),
            current_after_switch_forward.clone(),
        );
        if current_after_switch_forward
            .get("targetId")
            .and_then(Value::as_str)
            != Some(new_target_id.as_str())
        {
            return Err("current-tab did not move back to the new target".to_string());
        }

        let page_after_switch_forward = page_info(name)?;
        result.insert(
            "page_after_switch_forward".into(),
            page_after_switch_forward.clone(),
        );
        if page_after_switch_forward.get("url").and_then(Value::as_str) != Some(target_url.as_str())
        {
            return Err("page-info after switch-forward did not match the new tab URL".to_string());
        }

        result.insert("target_url".into(), Value::String(target_url));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_guest_run() -> Result<Value, String> {
    let options = load_options("bhrun-guest-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let guest_path = env_guest_path(default_navigate_guest_path());
    result.insert("guest_mode".into(), Value::String("run-guest".to_string()));
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let config = build_guest_config(
            options.name.as_str(),
            &guest_path,
            &["goto", "wait_for_load_event", "page_info", "js"],
            true,
        )?;
        result.insert("guest_config".into(), config.clone());
        let guest_run = runner_json_with_args(
            "run-guest",
            Some(config),
            &[guest_path.display().to_string()],
            Duration::from_secs(20),
        )?;
        result.insert("guest_run".into(), guest_run.clone());
        validate_sample_guest_run(&guest_run, &mut result)?;

        let page_after_guest = page_info(options.name.as_str())?;
        result.insert("page_after_guest".into(), page_after_guest.clone());
        if page_after_guest.get("url").and_then(Value::as_str) != Some(SAMPLE_GUEST_TARGET_URL) {
            return Err("runner page-info after guest did not match the expected URL".to_string());
        }

        let js_after_guest = js(options.name.as_str(), "location.href")?;
        result.insert("js_after_guest".into(), js_after_guest.clone());
        if js_after_guest.as_str() != Some(SAMPLE_GUEST_TARGET_URL) {
            return Err("runner js after guest did not match the expected URL".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_guest_serve() -> Result<Value, String> {
    let options = load_options("bhrun-guest-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let guest_path = env_guest_path(default_navigate_guest_path());
    result.insert(
        "guest_mode".into(),
        Value::String("serve-guest".to_string()),
    );
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let config = build_guest_config(
            options.name.as_str(),
            &guest_path,
            &["goto", "wait_for_load_event", "page_info", "js"],
            true,
        )?;
        result.insert("guest_config".into(), config.clone());
        let responses = runner_ndjson_with_args(
            "serve-guest",
            &serve_guest_input(&[
                json!({"command":"start","config":config}),
                json!({"command":"run"}),
                json!({"command":"status"}),
                json!({"command":"stop"}),
            ])?,
            &[guest_path.display().to_string()],
            Duration::from_secs(20),
        )?;
        if responses.len() != 4 {
            return Err(format!(
                "serve-guest returned {} lines, expected 4",
                responses.len()
            ));
        }
        result.insert("guest_ready".into(), responses[0].clone());
        result.insert("guest_run_response".into(), responses[1].clone());
        result.insert("guest_status".into(), responses[2].clone());
        result.insert("guest_stopped".into(), responses[3].clone());

        expect_kind(&responses[0], "ready", "serve-guest ready response")?;
        expect_kind(&responses[2], "status", "serve-guest status response")?;
        expect_kind(&responses[3], "stopped", "serve-guest stop response")?;
        if responses[0]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 0
        {
            return Err(format!(
                "unexpected serve-guest ready invocation count: {:?}",
                responses[0].get("invocation_count")
            ));
        }
        if responses[2]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 1
        {
            return Err(format!(
                "unexpected serve-guest status invocation count: {:?}",
                responses[2].get("invocation_count")
            ));
        }
        if responses[3]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 1
        {
            return Err(format!(
                "unexpected serve-guest stop invocation count: {:?}",
                responses[3].get("invocation_count")
            ));
        }

        let guest_run = responses[1]
            .get("result")
            .cloned()
            .ok_or_else(|| "serve-guest run response missing result".to_string())?;
        result.insert("guest_run".into(), guest_run.clone());
        validate_sample_guest_run(&guest_run, &mut result)?;

        let page_after_guest = page_info(options.name.as_str())?;
        result.insert("page_after_guest".into(), page_after_guest.clone());
        if page_after_guest.get("url").and_then(Value::as_str) != Some(SAMPLE_GUEST_TARGET_URL) {
            return Err("runner page-info after guest did not match the expected URL".to_string());
        }

        let js_after_guest = js(options.name.as_str(), "location.href")?;
        result.insert("js_after_guest".into(), js_after_guest.clone());
        if js_after_guest.as_str() != Some(SAMPLE_GUEST_TARGET_URL) {
            return Err("runner js after guest did not match the expected URL".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_persistent_guest() -> Result<Value, String> {
    let guest_path = env_guest_path(default_persistent_counter_guest_path());
    let mut result = Map::new();
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    let config = build_guest_config("default", &guest_path, &["wait"], true)?;
    let responses = runner_ndjson_with_args(
        "serve-guest",
        &serve_guest_input(&[
            json!({"command":"start","config":config}),
            json!({"command":"status"}),
            json!({"command":"run"}),
            json!({"command":"run"}),
            json!({"command":"stop"}),
        ])?,
        &[guest_path.display().to_string()],
        Duration::from_secs(20),
    )?;
    if responses.len() != 5 {
        return Err(format!(
            "serve-guest returned {} lines, expected 5",
            responses.len()
        ));
    }

    result.insert("ready".into(), responses[0].clone());
    result.insert("status".into(), responses[1].clone());
    result.insert("first_run".into(), responses[2].clone());
    result.insert("second_run".into(), responses[3].clone());
    result.insert("stopped".into(), responses[4].clone());

    expect_kind(&responses[0], "ready", "persistent guest ready response")?;
    expect_kind(&responses[1], "status", "persistent guest status response")?;
    expect_kind(
        &responses[2],
        "run_result",
        "persistent guest first run response",
    )?;
    expect_kind(
        &responses[3],
        "run_result",
        "persistent guest second run response",
    )?;
    expect_kind(&responses[4], "stopped", "persistent guest stop response")?;

    if responses[2]
        .get("invocation_count")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
        != 1
    {
        return Err(format!(
            "unexpected first invocation count: {:?}",
            responses[2].get("invocation_count")
        ));
    }
    if responses[3]
        .get("invocation_count")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
        != 2
    {
        return Err(format!(
            "unexpected second invocation count: {:?}",
            responses[3].get("invocation_count")
        ));
    }
    if responses[4]
        .get("invocation_count")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
        != 2
    {
        return Err(format!(
            "unexpected stop invocation count: {:?}",
            responses[4].get("invocation_count")
        ));
    }

    let first_duration = responses[2]
        .pointer("/result/calls/0/request/duration_ms")
        .and_then(Value::as_u64);
    let second_duration = responses[3]
        .pointer("/result/calls/0/request/duration_ms")
        .and_then(Value::as_u64);
    if first_duration != Some(1) {
        return Err(format!(
            "unexpected first guest duration: {first_duration:?}"
        ));
    }
    if second_duration != Some(2) {
        return Err(format!(
            "unexpected second guest duration: {second_duration:?}"
        ));
    }

    Ok(Value::Object(result))
}

fn smoke_persistent_guest_browser() -> Result<Value, String> {
    let options = load_options("bhrun-persistent-guest-remote-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let guest_manifest = rust_persistent_guest_manifest_path();
    let default_guest_path = rust_persistent_guest_default_path();
    let guest_path = env_guest_path(default_guest_path.clone());
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    result.insert(
        "target_url".into(),
        Value::String(PERSISTENT_BROWSER_GUEST_TARGET_URL.to_string()),
    );

    maybe_build_default_guest(
        &guest_path,
        &default_guest_path,
        &guest_manifest,
        "persistent browser-state",
        &mut result,
    )?;

    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let config = build_guest_config(
            options.name.as_str(),
            &guest_path,
            &["goto", "wait_for_load_event", "js", "page_info"],
            true,
        )?;
        let responses = runner_ndjson_with_args(
            "serve-guest",
            &serve_guest_input(&[
                json!({"command":"start","config":config}),
                json!({"command":"status"}),
                json!({"command":"run"}),
                json!({"command":"run"}),
                json!({"command":"status"}),
                json!({"command":"stop"}),
            ])?,
            &[guest_path.display().to_string()],
            Duration::from_secs(25),
        )?;
        if responses.len() != 6 {
            return Err(format!(
                "serve-guest returned {} lines, expected 6",
                responses.len()
            ));
        }

        result.insert("ready".into(), responses[0].clone());
        result.insert("status_after_start".into(), responses[1].clone());
        result.insert("first_run".into(), responses[2].clone());
        result.insert("second_run".into(), responses[3].clone());
        result.insert("status_after_runs".into(), responses[4].clone());
        result.insert("stopped".into(), responses[5].clone());

        expect_kind(
            &responses[0],
            "ready",
            "persistent browser guest ready response",
        )?;
        expect_kind(
            &responses[1],
            "status",
            "persistent browser guest status-after-start response",
        )?;
        expect_kind(
            &responses[2],
            "run_result",
            "persistent browser guest first run response",
        )?;
        expect_kind(
            &responses[3],
            "run_result",
            "persistent browser guest second run response",
        )?;
        expect_kind(
            &responses[4],
            "status",
            "persistent browser guest status-after-runs response",
        )?;
        expect_kind(
            &responses[5],
            "stopped",
            "persistent browser guest stop response",
        )?;

        if responses[0]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 0
        {
            return Err(format!(
                "unexpected ready invocation count: {:?}",
                responses[0].get("invocation_count")
            ));
        }
        if responses[1]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 0
        {
            return Err(format!(
                "unexpected initial status invocation count: {:?}",
                responses[1].get("invocation_count")
            ));
        }
        if responses[2]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 1
        {
            return Err(format!(
                "unexpected first invocation count: {:?}",
                responses[2].get("invocation_count")
            ));
        }
        if responses[3]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 2
        {
            return Err(format!(
                "unexpected second invocation count: {:?}",
                responses[3].get("invocation_count")
            ));
        }
        if responses[4]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 2
        {
            return Err(format!(
                "unexpected post-run status invocation count: {:?}",
                responses[4].get("invocation_count")
            ));
        }
        if responses[5]
            .get("invocation_count")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            != 2
        {
            return Err(format!(
                "unexpected stop invocation count: {:?}",
                responses[5].get("invocation_count")
            ));
        }

        let first_result = responses[2]
            .get("result")
            .cloned()
            .ok_or_else(|| "first guest run missing result".to_string())?;
        let second_result = responses[3]
            .get("result")
            .cloned()
            .ok_or_else(|| "second guest run missing result".to_string())?;
        validate_guest_result_success(&first_result, "first guest run")?;
        validate_guest_result_success(&second_result, "second guest run")?;

        let first_ops = collect_guest_operations(&first_result)?;
        let second_ops = collect_guest_operations(&second_result)?;
        result.insert(
            "first_run_operations".into(),
            Value::Array(first_ops.iter().cloned().map(Value::String).collect()),
        );
        result.insert(
            "second_run_operations".into(),
            Value::Array(second_ops.iter().cloned().map(Value::String).collect()),
        );
        if first_ops != ["goto", "wait_for_load_event", "js", "page_info"] {
            return Err(format!(
                "unexpected first guest operation sequence: {first_ops:?}"
            ));
        }
        if second_ops != ["js", "page_info"] {
            return Err(format!(
                "unexpected second guest operation sequence: {second_ops:?}"
            ));
        }

        let first_js_response = first_result
            .pointer("/calls/2/response")
            .and_then(Value::as_str);
        if first_js_response != Some(PERSISTENT_BROWSER_GUEST_MARKER) {
            return Err(format!(
                "unexpected first guest js response: {first_js_response:?}"
            ));
        }

        let first_page_info = first_result
            .pointer("/calls/3/response")
            .ok_or_else(|| "first guest page_info response missing".to_string())?;
        if first_page_info.get("url").and_then(Value::as_str)
            != Some(PERSISTENT_BROWSER_GUEST_TARGET_URL)
        {
            return Err("first guest page_info did not report the target URL".to_string());
        }

        let second_js_response = second_result
            .pointer("/calls/0/response")
            .and_then(Value::as_str)
            .ok_or_else(|| "second guest js response missing".to_string())?;
        let second_js_state: Value = serde_json::from_str(second_js_response)
            .map_err(|err| format!("second guest js response was not JSON: {err}"))?;
        result.insert("second_js_state".into(), second_js_state.clone());
        if second_js_state.get("href").and_then(Value::as_str)
            != Some(PERSISTENT_BROWSER_GUEST_TARGET_URL)
        {
            return Err("second guest js href did not match the target URL".to_string());
        }
        if second_js_state.get("marker").and_then(Value::as_str)
            != Some(PERSISTENT_BROWSER_GUEST_MARKER)
        {
            return Err("second guest js marker did not preserve browser state".to_string());
        }

        let second_page_info = second_result
            .pointer("/calls/1/response")
            .ok_or_else(|| "second guest page_info response missing".to_string())?;
        if second_page_info.get("url").and_then(Value::as_str)
            != Some(PERSISTENT_BROWSER_GUEST_TARGET_URL)
        {
            return Err("second guest page_info did not report the target URL".to_string());
        }

        let page_after_guest = page_info(options.name.as_str())?;
        result.insert("page_after_guest".into(), page_after_guest.clone());
        if page_after_guest.get("url").and_then(Value::as_str)
            != Some(PERSISTENT_BROWSER_GUEST_TARGET_URL)
        {
            return Err("runner page-info after guest did not match the target URL".to_string());
        }

        let marker_after_guest = js(options.name.as_str(), "window.__bhrunPersistentMarker")?;
        result.insert("marker_after_guest".into(), marker_after_guest.clone());
        if marker_after_guest.as_str() != Some(PERSISTENT_BROWSER_GUEST_MARKER) {
            return Err("runner js after guest did not preserve the marker".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_tab_response_guest() -> Result<Value, String> {
    let options = load_options("bhrun-tab-response-guest-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let guest_manifest = rust_tab_response_guest_manifest_path();
    let default_guest_path = rust_tab_response_guest_default_path();
    let guest_path = env_guest_path(default_guest_path.clone());
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    result.insert(
        "target_url".into(),
        Value::String(TAB_RESPONSE_GUEST_TARGET_URL.to_string()),
    );
    maybe_build_default_guest(
        &guest_path,
        &default_guest_path,
        &guest_manifest,
        "tab-response",
        &mut result,
    )?;
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let config = build_guest_config(
            options.name.as_str(),
            &guest_path,
            &[
                "current_tab",
                "list_tabs",
                "new_tab",
                "switch_tab",
                "current_session",
                "goto",
                "wait_for_response",
                "page_info",
                "js",
            ],
            true,
        )?;
        result.insert("guest_config".into(), config.clone());
        let guest_run = runner_json_with_args(
            "run-guest",
            Some(config),
            &[guest_path.display().to_string()],
            Duration::from_secs(25),
        )?;
        result.insert("guest_run".into(), guest_run.clone());
        validate_guest_result_success(&guest_run, "guest run")?;

        let operations = collect_guest_operations(&guest_run)?;
        result.insert(
            "guest_operations".into(),
            Value::Array(operations.iter().cloned().map(Value::String).collect()),
        );
        let expected_operations = [
            "current_tab",
            "list_tabs",
            "new_tab",
            "current_tab",
            "current_session",
            "js",
            "switch_tab",
            "current_session",
            "current_tab",
            "switch_tab",
            "current_session",
            "current_tab",
            "goto",
            "wait_for_response",
            "page_info",
            "js",
            "list_tabs",
        ];
        if operations != expected_operations {
            return Err(format!(
                "unexpected guest operation sequence: {operations:?}"
            ));
        }

        let calls = guest_run
            .get("calls")
            .and_then(Value::as_array)
            .ok_or_else(|| "guest run missing calls array".to_string())?;
        let initial_tab = calls
            .get(0)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest initial current_tab response missing".to_string())?;
        let initial_tabs = calls
            .get(1)
            .and_then(|call| call.get("response"))
            .and_then(Value::as_array)
            .ok_or_else(|| "guest initial list_tabs response missing".to_string())?;
        let current_after_new = calls
            .get(3)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest current_tab after new_tab missing".to_string())?;
        let new_session = calls
            .get(4)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest current_session after new_tab missing".to_string())?;
        let switch_back = calls
            .get(6)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest switch-tab back response missing".to_string())?;
        let session_after_switch_back = calls
            .get(7)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest current_session after switch-back missing".to_string())?;
        let current_after_switch_back = calls
            .get(8)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest current_tab after switch-back missing".to_string())?;
        let switch_forward = calls
            .get(9)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest switch-tab forward response missing".to_string())?;
        let session_after_switch_forward = calls
            .get(10)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest current_session after switch-forward missing".to_string())?;
        let current_after_switch_forward = calls
            .get(11)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest current_tab after switch-forward missing".to_string())?;
        let wait_result = calls
            .get(13)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest wait_for_response response missing".to_string())?;
        let final_page = calls
            .get(14)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest final page_info response missing".to_string())?;
        let final_tabs = calls
            .get(16)
            .and_then(|call| call.get("response"))
            .and_then(Value::as_array)
            .ok_or_else(|| "guest final list_tabs response missing".to_string())?;

        let initial_target_id = required_string_field(initial_tab, "targetId")?;
        let new_target_id = calls
            .get(2)
            .and_then(|call| call.get("response"))
            .and_then(|response| response.get("target_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| "guest new_tab response did not include target_id".to_string())?
            .to_string();
        let active_session_id = required_string_field(session_after_switch_forward, "session_id")?;
        let blank_href = calls
            .get(5)
            .and_then(|call| call.get("response"))
            .and_then(Value::as_str);
        let final_href = calls
            .get(15)
            .and_then(|call| call.get("response"))
            .and_then(Value::as_str);

        if new_target_id == initial_target_id {
            return Err("guest new_tab target_id matched the initial target".to_string());
        }
        if current_after_new.get("targetId").and_then(Value::as_str) != Some(new_target_id.as_str())
        {
            return Err(
                "guest current_tab after new_tab did not move to the new target".to_string(),
            );
        }
        if new_session
            .get("session_id")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err("guest current_session after new_tab was empty".to_string());
        }
        if blank_href != Some("about:blank") {
            return Err(format!(
                "guest js after new_tab did not report about:blank: {blank_href:?}"
            ));
        }
        if session_after_switch_back
            .get("session_id")
            .and_then(Value::as_str)
            != switch_back.get("session_id").and_then(Value::as_str)
        {
            return Err(
                "guest current_session did not match switch_tab after switching back".to_string(),
            );
        }
        if current_after_switch_back
            .get("targetId")
            .and_then(Value::as_str)
            != Some(initial_target_id.as_str())
        {
            return Err("guest current_tab did not return to the initial target".to_string());
        }
        if session_after_switch_forward
            .get("session_id")
            .and_then(Value::as_str)
            != switch_forward.get("session_id").and_then(Value::as_str)
        {
            return Err(
                "guest current_session did not match switch_tab after switching forward"
                    .to_string(),
            );
        }
        if current_after_switch_forward
            .get("targetId")
            .and_then(Value::as_str)
            != Some(new_target_id.as_str())
        {
            return Err("guest current_tab did not move back to the new target".to_string());
        }

        let event = wait_result
            .get("event")
            .ok_or_else(|| "guest wait_for_response event missing".to_string())?;
        let response = event
            .get("params")
            .and_then(|params| params.get("response"))
            .ok_or_else(|| "guest wait_for_response response params missing".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("guest wait_for_response returned matched=false".to_string());
        }
        if event.get("method").and_then(Value::as_str) != Some("Network.responseReceived") {
            return Err(format!(
                "unexpected wait_for_response method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if event.get("session_id").and_then(Value::as_str) != Some(active_session_id.as_str()) {
            return Err(
                "guest wait_for_response event did not match the active session".to_string(),
            );
        }
        if response.get("url").and_then(Value::as_str) != Some(TAB_RESPONSE_GUEST_TARGET_URL) {
            return Err("guest wait_for_response URL did not match the target URL".to_string());
        }
        if response
            .get("status")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 200
        {
            return Err(format!(
                "unexpected wait_for_response status: {:?}",
                response.get("status")
            ));
        }
        if final_page.get("url").and_then(Value::as_str) != Some(TAB_RESPONSE_GUEST_TARGET_URL) {
            return Err("guest final page_info did not match the target URL".to_string());
        }
        if final_href != Some(TAB_RESPONSE_GUEST_TARGET_URL) {
            return Err("guest final js href did not match the target URL".to_string());
        }
        if !final_tabs
            .iter()
            .any(|tab| tab.get("targetId").and_then(Value::as_str) == Some(new_target_id.as_str()))
        {
            return Err("guest final list_tabs result lost the new target".to_string());
        }
        if final_tabs.len() < initial_tabs.len() + 1 {
            return Err(
                "guest final list_tabs result did not grow after creating a new tab".to_string(),
            );
        }

        let page_after_guest = page_info(options.name.as_str())?;
        result.insert("page_after_guest".into(), page_after_guest.clone());
        if page_after_guest.get("url").and_then(Value::as_str)
            != Some(TAB_RESPONSE_GUEST_TARGET_URL)
        {
            return Err("runner page-info after guest did not match the target URL".to_string());
        }

        let current_tab_after_guest = current_tab(options.name.as_str())?;
        result.insert(
            "current_tab_after_guest".into(),
            current_tab_after_guest.clone(),
        );
        if current_tab_after_guest
            .get("targetId")
            .and_then(Value::as_str)
            != Some(new_target_id.as_str())
        {
            return Err(
                "runner current-tab after guest did not stay on the new target".to_string(),
            );
        }

        let current_session_after_guest = current_session(options.name.as_str())?;
        result.insert(
            "current_session_after_guest".into(),
            current_session_after_guest.clone(),
        );
        if current_session_after_guest
            .get("session_id")
            .and_then(Value::as_str)
            != Some(active_session_id.as_str())
        {
            return Err(
                "runner current-session after guest did not match the guest session".to_string(),
            );
        }

        let tabs_after_guest = list_tabs(options.name.as_str())?;
        result.insert(
            "tabs_after_guest".into(),
            Value::Array(tabs_after_guest.clone()),
        );
        if !tabs_after_guest
            .iter()
            .any(|tab| tab.get("targetId").and_then(Value::as_str) == Some(new_target_id.as_str()))
        {
            return Err(
                "runner list-tabs after guest did not include the new tab target".to_string(),
            );
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_event_waits_guest() -> Result<Value, String> {
    let options = load_options("bhrun-event-waits-guest-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let guest_manifest = rust_event_waits_guest_manifest_path();
    let default_guest_path = rust_event_waits_guest_default_path();
    let guest_path = env_guest_path(default_guest_path.clone());
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    maybe_build_default_guest(
        &guest_path,
        &default_guest_path,
        &guest_manifest,
        "event-waits",
        &mut result,
    )?;
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let prewarm_request = named_payload(options.name.as_str(), json!({"url": "about:blank"}))?;
        result.insert("prewarm_tab_request".into(), prewarm_request.clone());
        let prewarm_tab = runner_json("new-tab", Some(prewarm_request), Duration::from_secs(10))?;
        result.insert("prewarm_tab".into(), prewarm_tab);

        let config = build_guest_config(
            options.name.as_str(),
            &guest_path,
            &[
                "current_session",
                "wait_for_event",
                "watch_events",
                "wait_for_console",
                "wait_for_dialog",
                "handle_dialog",
                "js",
            ],
            true,
        )?;
        result.insert("guest_config".into(), config.clone());
        let guest_run = runner_json_with_args(
            "run-guest",
            Some(config),
            &[guest_path.display().to_string()],
            Duration::from_secs(30),
        )?;
        result.insert("guest_run".into(), guest_run.clone());
        validate_guest_result_success(&guest_run, "guest run")?;

        let operations = collect_guest_operations(&guest_run)?;
        result.insert(
            "guest_operations".into(),
            Value::Array(operations.iter().cloned().map(Value::String).collect()),
        );
        let expected_operations = [
            "current_session",
            "js",
            "wait_for_event",
            "js",
            "watch_events",
            "js",
            "wait_for_console",
            "js",
            "wait_for_dialog",
            "handle_dialog",
        ];
        if operations != expected_operations {
            return Err(format!(
                "unexpected guest operation sequence: {operations:?}"
            ));
        }

        let calls = guest_run
            .get("calls")
            .and_then(Value::as_array)
            .ok_or_else(|| "guest run missing calls array".to_string())?;
        let session_id = calls
            .get(0)
            .and_then(|call| call.get("response"))
            .and_then(|response| response.get("session_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| "guest current_session response did not include session_id".to_string())?
            .to_string();

        let wait_event = calls
            .get(2)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest wait_for_event response missing".to_string())?;
        if !wait_event
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("guest wait_for_event returned matched=false".to_string());
        }
        if wait_event.pointer("/event/method").and_then(Value::as_str)
            != Some("Runtime.consoleAPICalled")
        {
            return Err("guest wait_for_event did not return Runtime.consoleAPICalled".to_string());
        }
        if wait_event
            .pointer("/event/session_id")
            .and_then(Value::as_str)
            != Some(session_id.as_str())
        {
            return Err("guest wait_for_event session mismatch".to_string());
        }
        let wait_token = wait_event
            .pointer("/event/params/args/0/value")
            .and_then(Value::as_str);
        if wait_token != Some(EVENT_WAIT_TOKEN) {
            return Err(format!(
                "guest wait_for_event token mismatch: {wait_token:?}"
            ));
        }

        let watched = calls
            .get(4)
            .and_then(|call| call.get("response"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "guest watch_events response missing".to_string())?;
        result.insert("watched_lines".into(), Value::Array(watched.clone()));
        if watched.len() != 3 {
            return Err(format!(
                "guest watch_events returned unexpected line count: {}",
                watched.len()
            ));
        }
        if watched[0].get("kind").and_then(Value::as_str) != Some("event")
            || watched[1].get("kind").and_then(Value::as_str) != Some("event")
        {
            return Err("guest watch_events did not return event lines first".to_string());
        }
        let watch_one = watched[0]
            .pointer("/event/params/args/0/value")
            .and_then(Value::as_str);
        let watch_two = watched[1]
            .pointer("/event/params/args/0/value")
            .and_then(Value::as_str);
        if [watch_one, watch_two] != [Some(EVENT_WATCH_TOKEN_ONE), Some(EVENT_WATCH_TOKEN_TWO)] {
            return Err(format!(
                "guest watch_events tokens did not match: {:?}",
                [watch_one, watch_two]
            ));
        }
        let end_line = &watched[2];
        if end_line.get("kind").and_then(Value::as_str) != Some("end") {
            return Err("guest watch_events did not end with an end line".to_string());
        }
        if end_line.get("matched_events").and_then(Value::as_u64) != Some(2)
            || !end_line
                .get("reached_max_events")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return Err(format!(
                "guest watch_events end summary was unexpected: {end_line}"
            ));
        }

        let console_wait = calls
            .get(6)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest wait_for_console response missing".to_string())?;
        let console_event = console_wait
            .get("event")
            .ok_or_else(|| "guest wait_for_console event missing".to_string())?;
        if !console_wait
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("guest wait_for_console returned matched=false".to_string());
        }
        if console_event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("guest wait_for_console session mismatch".to_string());
        }
        let console_text = if console_event.get("method").and_then(Value::as_str)
            == Some("Console.messageAdded")
        {
            console_event
                .pointer("/params/message/text")
                .and_then(Value::as_str)
        } else {
            console_event
                .pointer("/params/args/0/value")
                .and_then(Value::as_str)
                .or_else(|| {
                    console_event
                        .pointer("/params/args/0/description")
                        .and_then(Value::as_str)
                })
        };
        if console_text != Some(EVENT_CONSOLE_TOKEN) {
            return Err(format!(
                "guest wait_for_console token mismatch: {console_text:?}"
            ));
        }

        let dialog_wait = calls
            .get(8)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest wait_for_dialog response missing".to_string())?;
        let dialog_event = dialog_wait
            .get("event")
            .ok_or_else(|| "guest wait_for_dialog event missing".to_string())?;
        if !dialog_wait
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("guest wait_for_dialog returned matched=false".to_string());
        }
        if dialog_event.get("method").and_then(Value::as_str)
            != Some("Page.javascriptDialogOpening")
        {
            return Err("guest wait_for_dialog did not return dialog opening".to_string());
        }
        if dialog_event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("guest wait_for_dialog session mismatch".to_string());
        }
        if dialog_event
            .pointer("/params/message")
            .and_then(Value::as_str)
            != Some(EVENT_DIALOG_TOKEN)
        {
            return Err("guest wait_for_dialog message mismatch".to_string());
        }

        let handle_dialog_response = calls
            .get(9)
            .and_then(|call| call.get("response"))
            .cloned()
            .ok_or_else(|| "guest handle_dialog response missing".to_string())?;
        result.insert("handle_dialog_response".into(), handle_dialog_response);

        let post_run_page_info = page_info(options.name.as_str())?;
        result.insert("post_run_page_info".into(), post_run_page_info.clone());
        if post_run_page_info.get("dialog").is_some() {
            return Err("dialog was still pending after guest handle_dialog".to_string());
        }
        Ok(())
    })();
    if !result.contains_key("post_run_page_info") {
        let cleanup = cleanup_dialog_best_effort(options.name.as_str());
        if cleanup
            .as_object()
            .map(|object| !object.is_empty())
            .unwrap_or(false)
        {
            result.insert("page_after_cleanup_attempt".into(), cleanup);
        }
    }
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_raw_cdp_guest() -> Result<Value, String> {
    let options = load_options("bhrun-raw-cdp-guest-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let guest_manifest = rust_raw_cdp_guest_manifest_path();
    let default_guest_path = rust_raw_cdp_guest_default_path();
    let guest_path = env_guest_path(default_guest_path.clone());
    result.insert(
        "guest_path".into(),
        Value::String(guest_path.display().to_string()),
    );
    maybe_build_default_guest(
        &guest_path,
        &default_guest_path,
        &guest_manifest,
        "raw CDP",
        &mut result,
    )?;
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let current_session_value = current_session(options.name.as_str())?;
        let session_id = required_string_field(&current_session_value, "session_id")?;
        result.insert("current_session".into(), current_session_value);

        let cli_raw_request = named_payload(
            options.name.as_str(),
            json!({
                "method": "Runtime.evaluate",
                "session_id": session_id,
                "params": {
                    "expression": format!("window.__bhrunRawCdpCli = {token:?}; window.__bhrunRawCdpCli", token = RAW_CDP_CLI_TOKEN),
                    "returnByValue": true,
                    "awaitPromise": true,
                }
            }),
        )?;
        result.insert("cli_raw_request".into(), cli_raw_request.clone());
        let cli_raw_result =
            runner_json("cdp-raw", Some(cli_raw_request), Duration::from_secs(10))?;
        result.insert("cli_raw_result".into(), cli_raw_result.clone());
        if cli_raw_result
            .pointer("/result/value")
            .and_then(Value::as_str)
            != Some(RAW_CDP_CLI_TOKEN)
        {
            return Err(format!("unexpected cli cdp-raw result: {cli_raw_result}"));
        }

        let guest_config = build_guest_config(
            options.name.as_str(),
            &guest_path,
            &["current_session", "cdp_raw"],
            true,
        )?;
        result.insert("guest_config".into(), guest_config.clone());
        let disabled_run = runner_json_with_args(
            "run-guest",
            Some(guest_config.clone()),
            &[guest_path.display().to_string()],
            Duration::from_secs(20),
        )?;
        result.insert("guest_run_disabled".into(), disabled_run.clone());
        if disabled_run
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("guest unexpectedly succeeded with allow_raw_cdp=false".to_string());
        }
        if !disabled_run
            .get("trap")
            .map(Value::to_string)
            .unwrap_or_default()
            .contains("cdp_raw disabled")
        {
            return Err(format!(
                "guest did not report the raw CDP gate: {disabled_run}"
            ));
        }

        let mut enabled_config = guest_config;
        enabled_config
            .as_object_mut()
            .ok_or_else(|| "guest config was not a JSON object".to_string())?
            .insert("allow_raw_cdp".into(), Value::Bool(true));
        result.insert("guest_config_enabled".into(), enabled_config.clone());
        let enabled_run = runner_json_with_args(
            "run-guest",
            Some(enabled_config),
            &[guest_path.display().to_string()],
            Duration::from_secs(20),
        )?;
        result.insert("guest_run_enabled".into(), enabled_run.clone());
        validate_guest_result_success(&enabled_run, "guest run with allow_raw_cdp=true")?;

        let operations = collect_guest_operations(&enabled_run)?;
        result.insert(
            "guest_operations".into(),
            Value::Array(operations.iter().cloned().map(Value::String).collect()),
        );
        if operations != ["current_session", "cdp_raw"] {
            return Err(format!(
                "unexpected guest operation sequence: {operations:?}"
            ));
        }
        let calls = enabled_run
            .get("calls")
            .and_then(Value::as_array)
            .ok_or_else(|| "guest enabled run missing calls array".to_string())?;
        let guest_response = calls
            .get(1)
            .and_then(|call| call.get("response"))
            .ok_or_else(|| "guest raw CDP response missing".to_string())?;
        if guest_response
            .pointer("/result/value")
            .and_then(Value::as_str)
            != Some(RAW_CDP_GUEST_TOKEN)
        {
            return Err(format!(
                "unexpected guest raw CDP response: {guest_response}"
            ));
        }
        if calls
            .get(1)
            .and_then(|call| call.get("request"))
            .and_then(|request| request.get("session_id"))
            .and_then(Value::as_str)
            != calls
                .get(0)
                .and_then(|call| call.get("response"))
                .and_then(|response| response.get("session_id"))
                .and_then(Value::as_str)
        {
            return Err("guest cdp_raw did not use the active session_id".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn env_guest_path(default_path: PathBuf) -> PathBuf {
    match env::var("BU_GUEST_PATH") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => default_path,
    }
}

fn default_navigate_guest_path() -> PathBuf {
    repo_root().join("rust/guests/navigate_and_read.wat")
}

fn default_persistent_counter_guest_path() -> PathBuf {
    repo_root().join("rust/guests/persistent_counter.wat")
}

fn rust_persistent_guest_manifest_path() -> PathBuf {
    repo_root().join("rust/guests/rust-persistent-browser-state/Cargo.toml")
}

fn rust_persistent_guest_default_path() -> PathBuf {
    repo_root().join(
        "rust/guests/rust-persistent-browser-state/target/wasm32-unknown-unknown/release/rust_persistent_browser_state_guest.wasm",
    )
}

fn rust_tab_response_guest_manifest_path() -> PathBuf {
    repo_root().join("rust/guests/rust-tab-response-workflow/Cargo.toml")
}

fn rust_tab_response_guest_default_path() -> PathBuf {
    repo_root().join(
        "rust/guests/rust-tab-response-workflow/target/wasm32-unknown-unknown/release/rust_tab_response_workflow_guest.wasm",
    )
}

fn rust_event_waits_guest_manifest_path() -> PathBuf {
    repo_root().join("rust/guests/rust-event-waits-sdk/Cargo.toml")
}

fn rust_event_waits_guest_default_path() -> PathBuf {
    repo_root().join(
        "rust/guests/rust-event-waits-sdk/target/wasm32-unknown-unknown/release/rust_event_waits_sdk_guest.wasm",
    )
}

fn rust_raw_cdp_guest_manifest_path() -> PathBuf {
    repo_root().join("rust/guests/rust-raw-cdp-smoke/Cargo.toml")
}

fn rust_raw_cdp_guest_default_path() -> PathBuf {
    repo_root().join(
        "rust/guests/rust-raw-cdp-smoke/target/wasm32-unknown-unknown/release/rust_raw_cdp_smoke_guest.wasm",
    )
}

fn maybe_build_default_guest(
    guest_path: &Path,
    default_guest_path: &Path,
    guest_manifest: &Path,
    guest_label: &str,
    result: &mut Map<String, Value>,
) -> Result<(), String> {
    if env::var("BU_SKIP_GUEST_BUILD").ok().as_deref() != Some("1")
        && guest_path == default_guest_path
    {
        build_guest_module(guest_manifest, guest_label)?;
        result.insert(
            "guest_manifest".into(),
            Value::String(guest_manifest.display().to_string()),
        );
        result.insert(
            "guest_build_mode".into(),
            Value::String("cargo+stable".to_string()),
        );
    }
    Ok(())
}

fn build_guest_module(guest_manifest: &Path, guest_label: &str) -> Result<(), String> {
    let output = Command::new("cargo")
        .args([
            "+stable",
            "build",
            "--offline",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
        ])
        .arg(guest_manifest)
        .current_dir(repo_root())
        .output()
        .map_err(|err| format!("spawn guest build: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = stderr
        .clone()
        .if_empty_then(stdout)
        .if_empty_then("guest build failed".to_string());
    Err(format!(
        "failed to build the Rust {guest_label} guest; ensure the stable wasm target is installed via `rustup target add --toolchain stable-x86_64-unknown-linux-gnu wasm32-unknown-unknown`\n{detail}"
    ))
}

fn build_guest_config(
    daemon_name: &str,
    guest_path: &Path,
    granted_operations: &[&str],
    persistent_guest_state: bool,
) -> Result<Value, String> {
    let mut config = runner_json_with_args("sample-config", None, &[], Duration::from_secs(10))?;
    let object = config
        .as_object_mut()
        .ok_or_else(|| "sample-config did not return a JSON object".to_string())?;
    object.insert("daemon_name".into(), Value::String(daemon_name.to_string()));
    object.insert(
        "guest_module".into(),
        Value::String(guest_path.display().to_string()),
    );
    object.insert(
        "granted_operations".into(),
        Value::Array(
            granted_operations
                .iter()
                .map(|operation| Value::String((*operation).to_string()))
                .collect(),
        ),
    );
    object.insert("allow_http".into(), Value::Bool(false));
    object.insert("allow_raw_cdp".into(), Value::Bool(false));
    object.insert(
        "persistent_guest_state".into(),
        Value::Bool(persistent_guest_state),
    );
    Ok(config)
}

fn runner_json_with_args(
    subcommand: &str,
    payload: Option<Value>,
    extra_args: &[String],
    timeout: Duration,
) -> Result<Value, String> {
    finish_json(
        start_command(ToolKind::Runner, subcommand, payload, extra_args)?,
        timeout,
    )
}

fn runner_ndjson_with_args(
    subcommand: &str,
    stdin_text: &str,
    extra_args: &[String],
    timeout: Duration,
) -> Result<Vec<Value>, String> {
    finish_ndjson(
        start_command_with_stdin_text(
            ToolKind::Runner,
            subcommand,
            if stdin_text.is_empty() {
                None
            } else {
                Some(stdin_text)
            },
            extra_args,
        )?,
        timeout,
    )
}

fn serve_guest_input(commands: &[Value]) -> Result<String, String> {
    let mut lines = Vec::with_capacity(commands.len());
    for command in commands {
        lines.push(
            serde_json::to_string(command)
                .map_err(|err| format!("serialize serve-guest command: {err}"))?,
        );
    }
    Ok(format!("{}\n", lines.join("\n")))
}

fn validate_sample_guest_run(
    guest_run: &Value,
    result: &mut Map<String, Value>,
) -> Result<(), String> {
    validate_guest_result_success(guest_run, "guest run")?;
    let operations = collect_guest_operations(guest_run)?;
    result.insert(
        "guest_operations".into(),
        Value::Array(operations.iter().cloned().map(Value::String).collect()),
    );
    if operations != ["goto", "wait_for_load_event", "page_info", "js"] {
        return Err(format!(
            "unexpected guest operation sequence: {operations:?}"
        ));
    }
    let page_info_response = guest_run
        .pointer("/calls/2/response")
        .ok_or_else(|| "guest page_info response missing".to_string())?;
    if page_info_response.get("url").and_then(Value::as_str) != Some(SAMPLE_GUEST_TARGET_URL) {
        return Err("guest page_info response did not match the expected URL".to_string());
    }

    let js_response = guest_run
        .pointer("/calls/3/response")
        .ok_or_else(|| "guest js response missing".to_string())?;
    let js_text = js_response
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| js_response.to_string());
    if !js_text.contains("Example Domain") {
        return Err(format!(
            "guest js response did not match the page title: {js_response}"
        ));
    }
    Ok(())
}

fn validate_guest_result_success(result: &Value, label: &str) -> Result<(), String> {
    if !result
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(format!("{label} failed: {result}"));
    }
    if result.get("exit_code").and_then(Value::as_i64) != Some(0) {
        return Err(format!(
            "{label} returned unexpected exit code: {:?}",
            result.get("exit_code")
        ));
    }
    Ok(())
}

fn collect_guest_operations(result: &Value) -> Result<Vec<String>, String> {
    let calls = result
        .get("calls")
        .and_then(Value::as_array)
        .ok_or_else(|| "guest result missing calls array".to_string())?;
    calls
        .iter()
        .map(|call| required_string_field(call, "operation"))
        .collect()
}

fn expect_kind(value: &Value, expected: &str, label: &str) -> Result<(), String> {
    if value.get("kind").and_then(Value::as_str) != Some(expected) {
        return Err(format!("unexpected {label}: {value}"));
    }
    Ok(())
}

fn smoke_wait_for_load_event() -> Result<Value, String> {
    require_remote_api_key()?;
    let options = load_options("bhrun-event-smoke", BrowserMode::Remote)?;
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, false, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(name, "https://example.com/?via=bhrun-event-smoke")?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("after_nav".into(), page_info(name)?);

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));
        let drained_before_wait = drain_events(name)?;
        result.insert(
            "drained_before_wait".into(),
            Value::from(drained_before_wait.len() as u64),
        );

        let wait_payload = json!({
            "daemon_name": name,
            "session_id": session_id,
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child = start_command(
            ToolKind::Runner,
            "wait-for-load-event",
            Some(wait_payload),
            &[],
        )?;
        sleep_ms(500);
        let token = unique_token("bhrun-event-smoke");
        let target_url = format!("https://example.com/?via=bhrun-event-smoke&token={token}");
        result.insert("goto_result".into(), goto(name, &target_url)?);
        let wait_result = finish_json(wait_child, Duration::from_secs(10))?;
        result.insert("wait_result".into(), wait_result.clone());
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-load-event response missing event".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-load-event returned matched=false".to_string());
        }
        if event.get("method").and_then(Value::as_str) != Some("Page.loadEventFired") {
            return Err(format!(
                "unexpected event method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("load event session_id did not match the active session".to_string());
        }
        let after_wait_page = page_info(name)?;
        result.insert("after_wait_page".into(), after_wait_page.clone());
        if after_wait_page.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err(
                "page URL did not match the navigation triggered for wait-for-load-event"
                    .to_string(),
            );
        }
        result.insert("target_url".into(), Value::String(target_url));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_watch_events() -> Result<Value, String> {
    let options = load_options("bhrun-watch-events-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(
                name,
                "https://example.com/?via=bhrun-watch-events-smoke",
            )?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("after_nav".into(), page_info(name)?);
        let drained_before_watch = drain_events(name)?;
        result.insert(
            "drained_before_watch".into(),
            Value::from(drained_before_watch.len() as u64),
        );

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));

        let watch_payload = json!({
            "daemon_name": name,
            "filter": {"session_id": session_id},
            "timeout_ms": 4000,
            "poll_interval_ms": 100,
            "max_events": 20,
        });
        result.insert("watch_request".into(), watch_payload.clone());
        let watch_child =
            start_command(ToolKind::Runner, "watch-events", Some(watch_payload), &[])?;
        sleep_ms(500);
        let token = unique_token("bhrun-watch-events-smoke");
        let target_url = format!("https://example.com/?via=bhrun-watch-events-smoke&token={token}");
        result.insert("goto_result".into(), goto(name, &target_url)?);
        let lines = finish_ndjson(watch_child, Duration::from_secs(10))?;
        result.insert("watch_lines".into(), Value::Array(lines.clone()));
        let event_lines = lines
            .iter()
            .filter(|line| line.get("kind").and_then(Value::as_str) == Some("event"))
            .cloned()
            .collect::<Vec<_>>();
        let end_lines = lines
            .iter()
            .filter(|line| line.get("kind").and_then(Value::as_str) == Some("end"))
            .cloned()
            .collect::<Vec<_>>();
        if event_lines.is_empty() {
            return Err("watch-events returned no matching event lines".to_string());
        }
        if end_lines.len() != 1 {
            return Err("watch-events did not return exactly one end line".to_string());
        }
        let methods = event_lines
            .iter()
            .filter_map(|line| {
                line.get("event")
                    .and_then(|event| event.get("method"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        result.insert(
            "methods".into(),
            Value::Array(methods.iter().cloned().map(Value::String).collect()),
        );
        if !methods
            .iter()
            .any(|method| method == "Page.frameStartedNavigating")
        {
            return Err("watch-events did not capture frameStartedNavigating".to_string());
        }
        if !methods.iter().any(|method| method == "Page.loadEventFired") {
            return Err("watch-events did not capture loadEventFired".to_string());
        }
        let end_line = &end_lines[0];
        if end_line
            .get("matched_events")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            < event_lines.len() as u64
        {
            return Err("watch-events end line under-reported matched events".to_string());
        }
        let after_watch_page = page_info(name)?;
        result.insert("after_watch_page".into(), after_watch_page.clone());
        if after_watch_page.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err(
                "page URL did not match the navigation triggered for watch-events".to_string(),
            );
        }
        result.insert("target_url".into(), Value::String(target_url));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_wait_for_request() -> Result<Value, String> {
    let options = load_options("bhrun-request-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let base_url = "https://example.com/?via=bhrun-request-smoke";
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(name, base_url)?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("after_nav".into(), page_info(name)?);

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));

        let token = unique_token("bhrun-request-smoke");
        let target_url = format!("https://example.com/?via=bhrun-request-smoke&token={token}");
        let wait_payload = json!({
            "daemon_name": name,
            "session_id": session_id,
            "url": target_url,
            "method": "GET",
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child = start_command(
            ToolKind::Runner,
            "wait-for-request",
            Some(wait_payload),
            &[],
        )?;
        sleep_ms(500);
        let fetch_result = js(
            name,
            &format!(
                "fetch({}, {{cache: 'no-store'}}).then(() => 'ok').catch(err => String(err))",
                serde_json::to_string(&target_url).map_err(|err| err.to_string())?
            ),
        )?;
        result.insert("fetch_result".into(), fetch_result);
        let wait_result = finish_json(wait_child, Duration::from_secs(15))?;
        result.insert("wait_result".into(), wait_result.clone());
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-request response missing event".to_string())?;
        let request = event
            .get("params")
            .and_then(|value| value.get("request"))
            .ok_or_else(|| "wait-for-request response missing params.request".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-request returned matched=false".to_string());
        }
        if event.get("method").and_then(Value::as_str) != Some("Network.requestWillBeSent") {
            return Err(format!(
                "unexpected event method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("request event session_id did not match the active session".to_string());
        }
        if request.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err("request event URL did not match the triggered fetch URL".to_string());
        }
        if request.get("method").and_then(Value::as_str) != Some("GET") {
            return Err(format!(
                "unexpected request method: {:?}",
                request.get("method").and_then(Value::as_str)
            ));
        }
        let after_wait_page = page_info(name)?;
        result.insert("after_wait_page".into(), after_wait_page.clone());
        if after_wait_page.get("url").and_then(Value::as_str) != Some(base_url) {
            return Err("page URL changed during request-side wait smoke".to_string());
        }
        result.insert("target_url".into(), Value::String(target_url));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_wait_for_response() -> Result<Value, String> {
    let options = load_options("bhrun-response-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(
                name,
                "https://example.com/?via=bhrun-response-smoke",
            )?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("after_nav".into(), page_info(name)?);

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));

        let token = unique_token("bhrun-response-smoke");
        let target_url = format!("https://example.com/?via=bhrun-response-smoke&token={token}");
        let wait_payload = json!({
            "daemon_name": name,
            "session_id": session_id,
            "url": target_url,
            "status": 200,
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child = start_command(
            ToolKind::Runner,
            "wait-for-response",
            Some(wait_payload),
            &[],
        )?;
        sleep_ms(500);
        result.insert("goto_result".into(), goto(name, &target_url)?);
        let wait_result = finish_json(wait_child, Duration::from_secs(15))?;
        result.insert("wait_result".into(), wait_result.clone());
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-response response missing event".to_string())?;
        let response = event
            .get("params")
            .and_then(|value| value.get("response"))
            .ok_or_else(|| "wait-for-response response missing params.response".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-response returned matched=false".to_string());
        }
        if event.get("method").and_then(Value::as_str) != Some("Network.responseReceived") {
            return Err(format!(
                "unexpected event method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("response event session_id did not match the active session".to_string());
        }
        if response.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err("response event URL did not match the requested target URL".to_string());
        }
        if response.get("status").and_then(Value::as_i64) != Some(200) {
            return Err(format!(
                "unexpected response status: {:?}",
                response.get("status")
            ));
        }
        let after_wait_page = page_info(name)?;
        result.insert("after_wait_page".into(), after_wait_page.clone());
        if after_wait_page.get("url").and_then(Value::as_str) != Some(target_url.as_str()) {
            return Err(
                "page URL did not match the navigation triggered for wait-for-response".to_string(),
            );
        }
        result.insert("target_url".into(), Value::String(target_url));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_wait_for_console() -> Result<Value, String> {
    require_remote_api_key()?;
    let options = load_options("bhrun-console-smoke", BrowserMode::Remote)?;
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, false, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(
                name,
                "https://example.com/?via=bhrun-console-smoke",
            )?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("after_nav".into(), page_info(name)?);

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));

        let token = unique_token("bhrun-console-smoke");
        let wait_payload = json!({
            "daemon_name": name,
            "session_id": session_id,
            "type": "log",
            "text": token,
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child = start_command(
            ToolKind::Runner,
            "wait-for-console",
            Some(wait_payload),
            &[],
        )?;
        sleep_ms(500);
        result.insert(
            "js_result".into(),
            js(
                name,
                &format!(
                    "setTimeout(() => console.log({}), 50); null",
                    serde_json::to_string(&token).map_err(|err| err.to_string())?
                ),
            )?,
        );
        let wait_result = finish_json(wait_child, Duration::from_secs(10))?;
        result.insert("wait_result".into(), wait_result.clone());
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-console response missing event".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-console returned matched=false".to_string());
        }
        if event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("console event session_id did not match the active session".to_string());
        }
        match event.get("method").and_then(Value::as_str) {
            Some("Console.messageAdded") => {
                let message = event
                    .get("params")
                    .and_then(|value| value.get("message"))
                    .ok_or_else(|| "console event missing params.message".to_string())?;
                if message.get("level").and_then(Value::as_str) != Some("log") {
                    return Err(format!(
                        "unexpected console level: {:?}",
                        message.get("level").and_then(Value::as_str)
                    ));
                }
                if message.get("text").and_then(Value::as_str) != Some(token.as_str()) {
                    return Err("console message text did not match the logged token".to_string());
                }
            }
            Some("Runtime.consoleAPICalled") => {
                let params = event
                    .get("params")
                    .ok_or_else(|| "runtime console event missing params".to_string())?;
                if params.get("type").and_then(Value::as_str) != Some("log") {
                    return Err(format!(
                        "unexpected console type: {:?}",
                        params.get("type").and_then(Value::as_str)
                    ));
                }
                let first_arg = params
                    .get("args")
                    .and_then(Value::as_array)
                    .and_then(|args| args.first())
                    .ok_or_else(|| "runtime console event missing args[0]".to_string())?;
                let value = first_arg
                    .get("value")
                    .and_then(Value::as_str)
                    .or_else(|| first_arg.get("description").and_then(Value::as_str));
                if value != Some(token.as_str()) {
                    return Err("runtime console event did not match the logged token".to_string());
                }
            }
            other => {
                return Err(format!("unexpected event method: {other:?}"));
            }
        }
        result.insert("token".into(), Value::String(token));
        result.insert("after_wait_page".into(), page_info(name)?);
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_wait_for_dialog() -> Result<Value, String> {
    require_remote_api_key()?;
    let options = load_options("bhrun-dialog-smoke", BrowserMode::Remote)?;
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, false, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("initial_page".into(), page_info(name)?);
        result.insert(
            "new_tab_target".into(),
            Value::String(new_tab(
                name,
                "https://example.com/?via=bhrun-dialog-smoke",
            )?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("after_nav".into(), page_info(name)?);

        let current_session = current_session(name)?;
        let session_id = required_string_field(&current_session, "session_id")?;
        result.insert("current_session".into(), current_session);
        result.insert("session_id".into(), Value::String(session_id.clone()));
        let drained_before_wait = drain_events(name)?;
        result.insert(
            "drained_before_wait".into(),
            Value::from(drained_before_wait.len() as u64),
        );

        let token = unique_token("bhrun-dialog-smoke");
        let wait_payload = json!({
            "daemon_name": name,
            "session_id": session_id,
            "type": "alert",
            "message": token,
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child =
            start_command(ToolKind::Runner, "wait-for-dialog", Some(wait_payload), &[])?;
        sleep_ms(500);
        result.insert(
            "js_result".into(),
            js(
                name,
                &format!(
                    "setTimeout(() => alert({}), 50); null",
                    serde_json::to_string(&token).map_err(|err| err.to_string())?
                ),
            )?,
        );
        let wait_result = finish_json(wait_child, Duration::from_secs(10))?;
        result.insert("wait_result".into(), wait_result.clone());
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-dialog response missing event".to_string())?;
        let params = event
            .get("params")
            .ok_or_else(|| "wait-for-dialog response missing params".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-dialog returned matched=false".to_string());
        }
        if event.get("method").and_then(Value::as_str) != Some("Page.javascriptDialogOpening") {
            return Err(format!(
                "unexpected event method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
            return Err("dialog event session_id did not match the active session".to_string());
        }
        if params.get("type").and_then(Value::as_str) != Some("alert") {
            return Err(format!(
                "unexpected dialog type: {:?}",
                params.get("type").and_then(Value::as_str)
            ));
        }
        if params.get("message").and_then(Value::as_str) != Some(token.as_str()) {
            return Err("dialog message did not match the triggered token".to_string());
        }

        let page_info_with_dialog = page_info(name)?;
        let dialog = page_info_with_dialog
            .get("dialog")
            .ok_or_else(|| "page-info did not surface the pending dialog".to_string())?;
        if dialog.get("type").and_then(Value::as_str) != Some("alert") {
            return Err(format!(
                "unexpected page-info dialog type: {:?}",
                dialog.get("type").and_then(Value::as_str)
            ));
        }
        if dialog.get("message").and_then(Value::as_str) != Some(token.as_str()) {
            return Err("page-info dialog message did not match the triggered token".to_string());
        }
        result.insert("page_info_with_dialog".into(), page_info_with_dialog);
        result.insert("dismiss_result".into(), handle_dialog(name, "accept")?);
        sleep_ms(300);
        let page_info_after_dismiss = page_info(name)?;
        if page_info_after_dismiss.get("dialog").is_some() {
            return Err("dialog was still pending after Page.handleJavaScriptDialog".to_string());
        }
        result.insert("page_info_after_dismiss".into(), page_info_after_dismiss);
        result.insert("token".into(), Value::String(token));
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_set_viewport() -> Result<Value, String> {
    let options = load_options("bhrun-viewport-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let target_url = "https://example.com/?via=bhrun-viewport-smoke";
        result.insert("target_url".into(), Value::String(target_url.to_string()));
        result.insert("goto_result".into(), goto(name, target_url)?);
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        let initial_page = page_info(name)?;
        result.insert("initial_page".into(), initial_page.clone());
        let initial_width = required_i64_field(&initial_page, "w")?;
        let initial_height = required_i64_field(&initial_page, "h")?;

        let desktop_request = json!({
            "width": 900,
            "height": 700,
            "device_scale_factor": 1.0,
            "mobile": false,
        });
        result.insert("desktop_request".into(), desktop_request.clone());
        set_viewport(name, 900, 700, 1.0, false)?;
        sleep_ms(300);
        let desktop_page = page_info(name)?;
        let desktop_metrics = js(
            name,
            "({width: innerWidth, height: innerHeight, dpr: window.devicePixelRatio})",
        )?;
        result.insert("desktop_page".into(), desktop_page.clone());
        result.insert("desktop_metrics".into(), desktop_metrics.clone());
        assert_page_size(&desktop_page, 900, 700, "desktop")?;
        assert_dpr(&desktop_metrics, 1.0, "desktop")?;

        let mobile_request = json!({
            "width": 480,
            "height": 720,
            "device_scale_factor": 2.0,
            "mobile": true,
        });
        result.insert("mobile_request".into(), mobile_request.clone());
        set_viewport(name, 480, 720, 2.0, true)?;
        sleep_ms(300);
        let mobile_page = page_info(name)?;
        let mobile_metrics = js(
            name,
            "(() => ({width: innerWidth, height: innerHeight, dpr: window.devicePixelRatio, coarse: matchMedia('(pointer: coarse)').matches, reducedHover: matchMedia('(hover: none)').matches}))()",
        )?;
        result.insert("mobile_page".into(), mobile_page.clone());
        result.insert("mobile_metrics".into(), mobile_metrics.clone());
        assert_page_size(&mobile_page, 480, 720, "mobile")?;
        assert_dpr(&mobile_metrics, 2.0, "mobile")?;

        set_viewport(name, initial_width, initial_height, 1.0, false)?;
        sleep_ms(300);
        let restored_page = page_info(name)?;
        result.insert("restored_page".into(), restored_page.clone());
        if restored_page.get("w").and_then(Value::as_i64) != Some(initial_width)
            || restored_page.get("h").and_then(Value::as_i64) != Some(initial_height)
        {
            return Err(format!(
                "viewport did not restore close to the initial size: {}x{} vs {}x{}",
                restored_page
                    .get("w")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                restored_page
                    .get("h")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                initial_width,
                initial_height
            ));
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_screenshot() -> Result<Value, String> {
    let options = load_options("bhrun-screenshot-smoke", BrowserMode::Remote)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let target_url = "https://example.com/?via=bhrun-screenshot-smoke";
        result.insert("target_url".into(), Value::String(target_url.to_string()));
        result.insert(
            "target_id".into(),
            Value::String(new_tab(name, target_url)?),
        );
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("page_before_setup".into(), page_info(name)?);
        let tall_layout = js(
            name,
            "(() => { const marker = document.createElement('div'); marker.id = 'bhrun-screenshot-smoke-marker'; marker.textContent = 'full-shot-marker'; marker.style.cssText = ['display:block', 'height:3200px', 'background:linear-gradient(#ffffff,#d6e4ff)', 'border-top:8px solid #345'].join(';'); document.body.style.margin = '0'; document.body.appendChild(marker); window.scrollTo(0, 0); return { marker: marker.textContent, scrollHeight: document.documentElement.scrollHeight }; })()",
        )?;
        result.insert("layout_setup".into(), tall_layout);
        let page_after_setup = page_info(name)?;
        result.insert("page_after_setup".into(), page_after_setup.clone());
        if page_after_setup
            .get("ph")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            <= page_after_setup
                .get("h")
                .and_then(Value::as_i64)
                .unwrap_or_default()
        {
            return Err(
                "page did not become taller than the viewport before full screenshot".to_string(),
            );
        }

        let viewport_png_b64 = screenshot_b64(name, false)?;
        let full_png_b64 = screenshot_b64(name, true)?;
        let (viewport_png, viewport_width, viewport_height) =
            decode_png_dimensions(&viewport_png_b64)?;
        let (full_png, full_width, full_height) = decode_png_dimensions(&full_png_b64)?;
        result.insert(
            "viewport_png_bytes".into(),
            Value::from(viewport_png.len() as u64),
        );
        result.insert("full_png_bytes".into(), Value::from(full_png.len() as u64));
        result.insert(
            "viewport_png_dimensions".into(),
            json!({"width": viewport_width, "height": viewport_height}),
        );
        result.insert(
            "full_png_dimensions".into(),
            json!({"width": full_width, "height": full_height}),
        );
        if viewport_width == 0 || viewport_height == 0 {
            return Err("viewport screenshot dimensions were invalid".to_string());
        }
        if full_width == 0 || full_height == 0 {
            return Err("full screenshot dimensions were invalid".to_string());
        }
        if full_height <= viewport_height {
            return Err(format!(
                "full screenshot height did not exceed viewport height: {full_height} <= {viewport_height}"
            ));
        }
        if full_width + 128 < viewport_width {
            return Err(format!(
                "full screenshot width shrank more than a scrollbar-sized tolerance: {full_width} << {viewport_width}"
            ));
        }
        let page_after_screenshots = page_info(name)?;
        result.insert(
            "page_after_screenshots".into(),
            page_after_screenshots.clone(),
        );
        if page_after_screenshots.get("url").and_then(Value::as_str) != Some(target_url) {
            return Err("page URL changed during screenshot capture".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_print_pdf() -> Result<Value, String> {
    let options = load_options("bhrun-print-pdf-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let target_url = "https://example.com/?via=bhrun-print-pdf-smoke";
        result.insert("target_url".into(), Value::String(target_url.to_string()));
        result.insert("goto_result".into(), goto(name, target_url)?);
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("page_before_print".into(), page_info(name)?);

        let portrait_pdf = decode_pdf(&print_pdf_b64(name, false)?)?;
        let landscape_pdf = decode_pdf(&print_pdf_b64(name, true)?)?;
        result.insert(
            "portrait_pdf_bytes".into(),
            Value::from(portrait_pdf.len() as u64),
        );
        result.insert(
            "landscape_pdf_bytes".into(),
            Value::from(landscape_pdf.len() as u64),
        );
        result.insert(
            "portrait_prefix".into(),
            Value::String(String::from_utf8_lossy(&portrait_pdf[..8]).to_string()),
        );
        result.insert(
            "landscape_prefix".into(),
            Value::String(String::from_utf8_lossy(&landscape_pdf[..8]).to_string()),
        );
        if portrait_pdf.len() < 1000 {
            return Err("portrait PDF was unexpectedly small".to_string());
        }
        if landscape_pdf.len() < 1000 {
            return Err("landscape PDF was unexpectedly small".to_string());
        }
        if portrait_pdf == landscape_pdf {
            return Err("portrait and landscape PDFs were identical".to_string());
        }
        let page_after_print = page_info(name)?;
        result.insert("page_after_print".into(), page_after_print.clone());
        if page_after_print.get("url").and_then(Value::as_str) != Some(target_url) {
            return Err("page URL changed during print-pdf smoke".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_cookies() -> Result<Value, String> {
    let options = load_options("bhrun-cookies-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let target_url = "https://example.com/?via=bhrun-cookies-smoke";
        result.insert("target_url".into(), Value::String(target_url.to_string()));
        result.insert("goto_result".into(), goto(name, target_url)?);
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("page_before_cookie".into(), page_info(name)?);

        let cookie_name = unique_token("bhrun_cookie");
        let cookie_value = unique_token("cookie_value");
        let cookie = json!({
            "name": cookie_name,
            "value": cookie_value,
            "url": target_url,
            "secure": true,
            "sameSite": "Lax",
        });
        set_cookies(name, vec![cookie.clone()])?;
        result.insert("cookie_set".into(), cookie.clone());
        let visible_cookie = js(
            name,
            &format!(
                "document.cookie.split('; ').find(c => c.startsWith({})) || null",
                serde_json::to_string(&format!("{cookie_name}=")).map_err(|err| err.to_string())?
            ),
        )?;
        result.insert("document_cookie_entry".into(), visible_cookie.clone());
        if visible_cookie.as_str() != Some(format!("{cookie_name}={cookie_value}").as_str()) {
            return Err(format!(
                "document.cookie did not expose the new cookie: {visible_cookie:?}"
            ));
        }
        let cookies = get_cookies(name, vec![target_url.to_string()])?;
        let cookies_array = cookies
            .as_array()
            .ok_or_else(|| "get-cookies did not return an array".to_string())?;
        result.insert(
            "cookie_count".into(),
            Value::from(cookies_array.len() as u64),
        );
        let matched = cookies_array
            .iter()
            .filter(|cookie| {
                cookie.get("name").and_then(Value::as_str) == Some(cookie_name.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        result.insert("matched_cookies".into(), Value::Array(matched.clone()));
        if matched.len() != 1 {
            return Err(format!(
                "expected exactly one matched cookie, got {}",
                matched.len()
            ));
        }
        if matched[0].get("value").and_then(Value::as_str) != Some(cookie_value.as_str()) {
            return Err("get-cookies returned the wrong cookie value".to_string());
        }
        if !matched[0]
            .get("domain")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("example.com")
        {
            return Err(format!(
                "cookie domain did not contain example.com: {:?}",
                matched[0].get("domain").and_then(Value::as_str)
            ));
        }
        let page_after_cookie = page_info(name)?;
        result.insert("page_after_cookie".into(), page_after_cookie.clone());
        if page_after_cookie.get("url").and_then(Value::as_str) != Some(target_url) {
            return Err("page URL changed during cookie smoke".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_wait_for_download() -> Result<Value, String> {
    let options = load_options("bhrun-download-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        let target_url = "https://example.com/?via=bhrun-download-smoke";
        result.insert("target_url".into(), Value::String(target_url.to_string()));
        result.insert("goto_result".into(), goto(name, target_url)?);
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        result.insert("page_before_download".into(), page_info(name)?);
        drain_events(name)?;

        let temp_dir = TempDir::new("bhrun-download-smoke")?;
        let filename = format!("bhrun-download-{}.txt", unique_token("file"));
        let file_path = temp_dir.path().join(&filename);
        let file_text = format!("bhrun download smoke {}", unique_token("payload"));

        configure_downloads(name, temp_dir.path())?;
        result.insert(
            "download_dir".into(),
            Value::String(temp_dir.path().display().to_string()),
        );
        result.insert("filename".into(), Value::String(filename.clone()));

        let wait_payload = json!({
            "daemon_name": name,
            "filename": filename,
            "timeout_ms": 5000,
            "poll_interval_ms": 100,
        });
        result.insert("wait_request".into(), wait_payload.clone());
        let wait_child = start_command(
            ToolKind::Runner,
            "wait-for-download",
            Some(wait_payload),
            &[],
        )?;
        sleep_ms(400);
        result.insert(
            "trigger_result".into(),
            js(
                name,
                &format!(
                    "(() => {{ const text = {text}; const blob = new Blob([text], {{type: 'text/plain'}}); const href = URL.createObjectURL(blob); const link = document.createElement('a'); link.href = href; link.download = {filename}; document.body.appendChild(link); link.click(); setTimeout(() => {{ URL.revokeObjectURL(href); link.remove(); }}, 250); return {{href, filename: link.download, textLength: text.length}}; }})()",
                    text = serde_json::to_string(&file_text).map_err(|err| err.to_string())?,
                    filename = serde_json::to_string(&filename).map_err(|err| err.to_string())?,
                ),
            )?,
        );
        let wait_result = finish_json(wait_child, Duration::from_secs(15))?;
        result.insert("wait_result".into(), wait_result.clone());
        let event = wait_result
            .get("event")
            .ok_or_else(|| "wait-for-download response missing event".to_string())?;
        let params = event
            .get("params")
            .ok_or_else(|| "wait-for-download response missing params".to_string())?;
        if !wait_result
            .get("matched")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("wait-for-download returned matched=false".to_string());
        }
        if event.get("method").and_then(Value::as_str) != Some("Browser.downloadWillBegin") {
            return Err(format!(
                "unexpected download event method: {:?}",
                event.get("method").and_then(Value::as_str)
            ));
        }
        if params.get("suggestedFilename").and_then(Value::as_str) != Some(filename.as_str()) {
            return Err(format!(
                "download event filename mismatch: {:?} vs {:?}",
                params.get("suggestedFilename").and_then(Value::as_str),
                filename
            ));
        }

        if options.browser_mode == BrowserMode::Local {
            let downloaded = wait_for_downloaded_file(&file_path, Duration::from_secs(10))?;
            let downloaded_text = fs::read_to_string(&downloaded)
                .map_err(|err| format!("read downloaded file {}: {err}", downloaded.display()))?;
            result.insert(
                "downloaded_file".into(),
                Value::String(downloaded.display().to_string()),
            );
            result.insert(
                "downloaded_bytes".into(),
                Value::from(
                    downloaded
                        .metadata()
                        .map_err(|err| {
                            format!("stat downloaded file {}: {err}", downloaded.display())
                        })?
                        .len(),
                ),
            );
            result.insert(
                "downloaded_text".into(),
                Value::String(downloaded_text.clone()),
            );
            if downloaded_text != file_text {
                return Err("downloaded file content did not match the blob payload".to_string());
            }
        } else {
            result.insert(
                "download_verification".into(),
                Value::String("event_only".to_string()),
            );
        }

        let page_after_download = page_info(name)?;
        result.insert("page_after_download".into(), page_after_download.clone());
        if page_after_download.get("url").and_then(Value::as_str) != Some(target_url) {
            return Err("page URL changed during download smoke".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_drag() -> Result<Value, String> {
    let options = load_options("bhrun-drag-smoke", BrowserMode::Local)?;
    if options.browser_mode == BrowserMode::Remote {
        require_remote_api_key()?;
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, true, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("goto_result".into(), goto(name, "about:blank")?);
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        set_viewport(name, 900, 700, 1.0, false)?;
        sleep_ms(200);
        result.insert("page_before_drag".into(), page_info(name)?);
        let geometry = js(
            name,
            &format!(
                "(() => {{ document.title = {title}; document.body.innerHTML = `<style>body {{ margin: 0; font-family: monospace; background: #f6f2e8; }} #track {{ position: absolute; left: 80px; top: 160px; width: 520px; height: 24px; background: #d0c8b6; border-radius: 999px; }} #fill {{ position: absolute; left: 0; top: 0; height: 24px; width: 0; background: #2f7a6b; border-radius: 999px; }} #handle {{ position: absolute; left: 0; top: -8px; width: 40px; height: 40px; background: #0a5f7a; border-radius: 999px; box-shadow: 0 4px 12px rgba(0,0,0,.18); }} #status {{ position: absolute; left: 80px; top: 230px; }}</style><div id=\"track\"><div id=\"fill\"></div><div id=\"handle\"></div></div><pre id=\"status\"></pre>`; const track = document.getElementById('track'); const fill = document.getElementById('fill'); const handle = document.getElementById('handle'); const status = document.getElementById('status'); const state = {{ events: [], finalLeft: 0, dragging: false }}; window.__dragState = state; let offsetX = 0; const maxLeft = () => track.clientWidth - handle.offsetWidth; const clamp = value => Math.max(0, Math.min(maxLeft(), value)); const sync = left => {{ handle.style.left = `${{left}}px`; fill.style.width = `${{left + handle.offsetWidth / 2}}px`; state.finalLeft = left; status.textContent = JSON.stringify(state); }}; handle.addEventListener('mousedown', event => {{ state.dragging = true; offsetX = event.clientX - handle.getBoundingClientRect().left; state.events.push({{ type: 'down', x: event.clientX, y: event.clientY, buttons: event.buttons }}); sync(state.finalLeft); event.preventDefault(); }}); document.addEventListener('mousemove', event => {{ if (!state.dragging) return; const nextLeft = clamp(event.clientX - track.getBoundingClientRect().left - offsetX); state.events.push({{ type: 'move', x: event.clientX, y: event.clientY, buttons: event.buttons, left: nextLeft }}); sync(nextLeft); }}); document.addEventListener('mouseup', event => {{ if (!state.dragging) return; state.dragging = false; state.events.push({{ type: 'up', x: event.clientX, y: event.clientY, buttons: event.buttons }}); sync(state.finalLeft); }}); sync(0); const handleRect = handle.getBoundingClientRect(); const trackRect = track.getBoundingClientRect(); return {{ startX: handleRect.left + handleRect.width / 2, startY: handleRect.top + handleRect.height / 2, midX: trackRect.left + trackRect.width * 0.55, endX: trackRect.left + trackRect.width - handleRect.width / 2 - 8, endY: handleRect.top + handleRect.height / 2, maxLeft: maxLeft() }}; }})()",
                title = serde_json::to_string(name).map_err(|err| err.to_string())?
            ),
        )?;
        result.insert("fixture_geometry".into(), geometry.clone());
        let start_x = required_number_field(&geometry, "startX")?;
        let start_y = required_number_field(&geometry, "startY")?;
        let mid_x = required_number_field(&geometry, "midX")?;
        let end_x = required_number_field(&geometry, "endX")?;
        let end_y = required_number_field(&geometry, "endY")?;
        let max_left = required_number_field(&geometry, "maxLeft")?;
        mouse_move(name, start_x, start_y, 0)?;
        sleep_ms(50);
        mouse_down(name, start_x, start_y, "left", 1, 1)?;
        sleep_ms(50);
        mouse_move(name, mid_x, end_y, 1)?;
        sleep_ms(50);
        mouse_move(name, end_x, end_y, 1)?;
        sleep_ms(50);
        mouse_up(name, end_x, end_y, "left", 0, 1)?;
        sleep_ms(100);

        let drag_state = js(name, "window.__dragState")?;
        result.insert("drag_state".into(), drag_state.clone());
        result.insert("page_after_drag".into(), page_info(name)?);
        let events = drag_state
            .get("events")
            .and_then(Value::as_array)
            .ok_or_else(|| "drag state missing events".to_string())?;
        let event_types = events
            .iter()
            .filter_map(|event| {
                event
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        result.insert(
            "event_types".into(),
            Value::Array(event_types.iter().cloned().map(Value::String).collect()),
        );
        if event_types.first().map(String::as_str) != Some("down")
            || !event_types.iter().any(|item| item == "move")
            || event_types.last().map(String::as_str) != Some("up")
        {
            return Err(format!("unexpected drag event sequence: {event_types:?}"));
        }
        if drag_state
            .get("finalLeft")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            < max_left * 0.65
        {
            return Err(format!(
                "drag did not move far enough: {} vs {}",
                drag_state
                    .get("finalLeft")
                    .and_then(Value::as_f64)
                    .unwrap_or_default(),
                max_left
            ));
        }
        if drag_state
            .get("dragging")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("drag state stayed active after mouse-up".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn smoke_upload_file() -> Result<Value, String> {
    let options = load_options("bhrun-upload-smoke", BrowserMode::Local)?;
    if options.browser_mode != BrowserMode::Local {
        return Err("upload-file smoke currently supports only BU_BROWSER_MODE=local".to_string());
    }
    let mut result = result_map(&options);
    let remote_browser = setup_browser(&options, true, false, &mut result)?;
    let run_result = (|| {
        let name = options.name.as_str();
        result.insert("goto_result".into(), goto(name, "about:blank")?);
        result.insert("loaded".into(), Value::Bool(wait_for_load(name)?));
        set_viewport(name, 900, 700, 1.0, false)?;
        sleep_ms(200);
        result.insert("page_before_upload".into(), page_info(name)?);
        js(
            name,
            "(() => { document.body.innerHTML = `<style>body{font-family:monospace;padding:32px;background:#f5f0e8}</style><label for=\"upload\">Upload fixture</label><input id=\"upload\" type=\"file\" multiple /><pre id=\"state\"></pre>`; window.__uploadState = {ready: false, names: [], texts: []}; const input = document.getElementById('upload'); const state = document.getElementById('state'); input.addEventListener('change', async () => { const files = Array.from(input.files || []); window.__uploadState = { ready: true, names: files.map(file => file.name), sizes: files.map(file => file.size), texts: await Promise.all(files.map(file => file.text())) }; state.textContent = JSON.stringify(window.__uploadState); }); return true; })()",
        )?;
        let temp_dir = TempDir::new("bhrun-upload-smoke")?;
        let file_path = temp_dir.path().join("upload-fixture.txt");
        let file_text = "bhrun upload smoke payload";
        fs::write(&file_path, file_text)
            .map_err(|err| format!("write upload fixture {}: {err}", file_path.display()))?;
        result.insert(
            "upload_file".into(),
            Value::String(file_path.display().to_string()),
        );

        upload_file(name, "#upload", &[file_path.clone()])?;
        sleep_ms(300);
        let upload_state = js(name, "window.__uploadState")?;
        result.insert("upload_state".into(), upload_state.clone());
        if !upload_state
            .get("ready")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err("file input change handler did not run".to_string());
        }
        let names = upload_state
            .get("names")
            .and_then(Value::as_array)
            .ok_or_else(|| "upload state missing names".to_string())?;
        if names.len() != 1 || names[0].as_str() != Some("upload-fixture.txt") {
            return Err(format!("unexpected uploaded file names: {names:?}"));
        }
        let texts = upload_state
            .get("texts")
            .and_then(Value::as_array)
            .ok_or_else(|| "upload state missing texts".to_string())?;
        if texts.len() != 1 || texts[0].as_str() != Some(file_text) {
            return Err(format!("unexpected uploaded file text: {texts:?}"));
        }

        let page_after_upload = page_info(name)?;
        result.insert("page_after_upload".into(), page_after_upload.clone());
        if page_after_upload.get("url").and_then(Value::as_str) != Some("about:blank") {
            return Err("page URL changed during upload smoke".to_string());
        }
        Ok(())
    })();
    finalize_smoke(&options, remote_browser, &mut result, run_result)
}

fn load_options(default_name: &str, default_mode: BrowserMode) -> Result<SmokeOptions, String> {
    let browser_mode = match env::var("BU_BROWSER_MODE") {
        Ok(value) if !value.trim().is_empty() => parse_browser_mode(&value)?,
        _ => default_mode,
    };
    let remote_timeout_minutes = parse_env_u64("BU_REMOTE_TIMEOUT_MINUTES", 1)?;
    let local_wait_seconds = parse_env_f64("BU_LOCAL_DAEMON_WAIT_SECONDS", 15.0)?;
    Ok(SmokeOptions {
        name: env::var("BU_NAME").unwrap_or_else(|_| default_name.to_string()),
        daemon_impl: env::var("BU_DAEMON_IMPL").unwrap_or_else(|_| "rust".to_string()),
        browser_mode,
        remote_timeout_minutes,
        local_wait_seconds,
    })
}

fn parse_browser_mode(raw: &str) -> Result<BrowserMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "local" => Ok(BrowserMode::Local),
        "remote" => Ok(BrowserMode::Remote),
        _ => Err("BU_BROWSER_MODE must be 'remote' or 'local'".to_string()),
    }
}

fn parse_env_u64(key: &str, default: u64) -> Result<u64, String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse::<u64>()
            .map_err(|err| format!("parse {key}: {err}")),
        _ => Ok(default),
    }
}

fn parse_env_f64(key: &str, default: f64) -> Result<f64, String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse::<f64>()
            .map_err(|err| format!("parse {key}: {err}")),
        _ => Ok(default),
    }
}

fn require_remote_api_key() -> Result<(), String> {
    if env::var("BROWSER_USE_API_KEY").is_err() {
        return Err("BROWSER_USE_API_KEY is required".to_string());
    }
    Ok(())
}

fn result_map(options: &SmokeOptions) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("name".into(), Value::String(options.name.clone()));
    result.insert(
        "daemon_impl".into(),
        Value::String(options.daemon_impl.clone()),
    );
    result.insert(
        "browser_mode".into(),
        Value::String(options.browser_mode.as_str().to_string()),
    );
    result
}

fn setup_browser(
    options: &SmokeOptions,
    allow_local: bool,
    allow_remote: bool,
    result: &mut Map<String, Value>,
) -> Result<Option<RemoteBrowser>, String> {
    match options.browser_mode {
        BrowserMode::Local if !allow_local => {
            Err("this smoke scenario does not support BU_BROWSER_MODE=local".to_string())
        }
        BrowserMode::Remote if !allow_remote => {
            Err("this smoke scenario does not support BU_BROWSER_MODE=remote".to_string())
        }
        BrowserMode::Local => {
            ensure_daemon(options.name.as_str(), options.local_wait_seconds)?;
            result.insert(
                "local_attach".into(),
                Value::String("DevToolsActivePort".to_string()),
            );
            Ok(None)
        }
        BrowserMode::Remote => {
            let browser =
                start_remote_daemon(options.name.as_str(), options.remote_timeout_minutes)?;
            let browser_id = required_string_field(&browser, "id")?;
            result.insert("browser_id".into(), Value::String(browser_id.clone()));
            Ok(Some(RemoteBrowser { id: browser_id }))
        }
    }
}

fn finalize_smoke(
    options: &SmokeOptions,
    remote_browser: Option<RemoteBrowser>,
    result: &mut Map<String, Value>,
    run_result: Result<(), String>,
) -> Result<Value, String> {
    let cleanup_result = cleanup_smoke(options, remote_browser, result);
    match (run_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(Value::Object(std::mem::take(result))),
        (Err(run_err), Ok(())) => Err(run_err),
        (Ok(()), Err(cleanup_err)) => Err(cleanup_err),
        (Err(run_err), Err(cleanup_err)) => Err(format!("{run_err}\ncleanup: {cleanup_err}")),
    }
}

fn cleanup_smoke(
    options: &SmokeOptions,
    remote_browser: Option<RemoteBrowser>,
    result: &mut Map<String, Value>,
) -> Result<(), String> {
    let mut cleanup_error = None;
    if let Err(err) = restart_daemon(options.name.as_str()) {
        cleanup_error = Some(err);
    }
    sleep_ms(1000);
    if let Some(remote_browser) = remote_browser {
        match poll_browser_status(&remote_browser.id, 10, Duration::from_secs(1)) {
            Ok(status) => {
                result.insert("post_shutdown_status".into(), Value::String(status));
            }
            Err(err) if cleanup_error.is_none() => cleanup_error = Some(err),
            Err(_) => {}
        }
    }
    if let Some(log_tail) = read_log_tail(options.name.as_str())? {
        result.insert(
            "log_tail".into(),
            Value::Array(log_tail.into_iter().map(Value::String).collect()),
        );
    }
    if let Some(err) = cleanup_error {
        return Err(err);
    }
    Ok(())
}

fn start_remote_daemon(name: &str, timeout_minutes: u64) -> Result<Value, String> {
    let alive = admin_json(
        "daemon-alive",
        None,
        &[name.to_string()],
        Duration::from_secs(10),
    )?;
    if alive.get("alive").and_then(Value::as_bool).unwrap_or(false) {
        return Err(format!(
            "daemon {:?} already alive; stop it before starting remote smoke",
            name
        ));
    }

    let browser = admin_json(
        "create-browser",
        Some(json!({"timeout": timeout_minutes})),
        &[],
        Duration::from_secs(60),
    )?;
    let browser_id = required_string_field(&browser, "id")?;
    let cdp_ws = required_string_field(&browser, "cdpWsUrl")?;
    let ensure_result = admin_json(
        "ensure-daemon",
        Some(json!({
            "wait": 60.0,
            "name": name,
            "env": {
                "BU_CDP_WS": cdp_ws,
                "BU_BROWSER_ID": browser_id,
            }
        })),
        &[],
        Duration::from_secs(70),
    );
    if let Err(err) = ensure_result {
        let _ = admin_json("stop-browser", None, &[browser_id], Duration::from_secs(30));
        return Err(err);
    }
    Ok(browser)
}

fn ensure_daemon(name: &str, wait_seconds: f64) -> Result<(), String> {
    admin_json(
        "ensure-daemon",
        Some(json!({
            "wait": wait_seconds,
            "name": name,
            "env": {},
        })),
        &[],
        Duration::from_secs((wait_seconds.ceil() as u64).max(20) + 10),
    )?;
    Ok(())
}

fn restart_daemon(name: &str) -> Result<(), String> {
    admin_json(
        "restart-daemon",
        None,
        &[name.to_string()],
        Duration::from_secs(20),
    )?;
    Ok(())
}

fn poll_browser_status(
    browser_id: &str,
    attempts: usize,
    delay: Duration,
) -> Result<String, String> {
    let mut status = "missing".to_string();
    for _ in 0..attempts {
        let listing = admin_json(
            "list-browsers",
            Some(json!({"pageSize": 20, "pageNumber": 1})),
            &[],
            Duration::from_secs(30),
        )?;
        let item = listing
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("id").and_then(Value::as_str) == Some(browser_id))
            });
        status = item
            .and_then(|item| item.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("missing")
            .to_string();
        if status != "active" {
            return Ok(status);
        }
        thread::sleep(delay);
    }
    Ok(status)
}

fn read_log_tail(name: &str) -> Result<Option<Vec<String>>, String> {
    let path = PathBuf::from(format!("/tmp/bu-{name}.log"));
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read daemon log {}: {err}", path.display()))?;
    let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    let start = lines.len().saturating_sub(8);
    Ok(Some(lines[start..].to_vec()))
}

fn admin_json(
    subcommand: &str,
    payload: Option<Value>,
    extra_args: &[String],
    timeout: Duration,
) -> Result<Value, String> {
    finish_json(
        start_command(ToolKind::Admin, subcommand, payload, extra_args)?,
        timeout,
    )
}

fn runner_json(
    subcommand: &str,
    payload: Option<Value>,
    timeout: Duration,
) -> Result<Value, String> {
    finish_json(
        start_command(ToolKind::Runner, subcommand, payload, &[])?,
        timeout,
    )
}

fn start_command(
    kind: ToolKind,
    subcommand: &str,
    payload: Option<Value>,
    extra_args: &[String],
) -> Result<Child, String> {
    let stdin_text = payload
        .map(|payload| {
            serde_json::to_string(&payload).map_err(|err| format!("serialize stdin JSON: {err}"))
        })
        .transpose()?;
    start_command_with_stdin_text(kind, subcommand, stdin_text.as_deref(), extra_args)
}

fn start_command_with_stdin_text(
    kind: ToolKind,
    subcommand: &str,
    stdin_text: Option<&str>,
    extra_args: &[String],
) -> Result<Child, String> {
    let mut command = child_command(kind)?;
    command
        .arg(subcommand)
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|err| format!("spawn {subcommand}: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(input) = stdin_text {
            stdin
                .write_all(input.as_bytes())
                .map_err(|err| format!("write stdin for {subcommand}: {err}"))?;
        }
    }
    Ok(child)
}

fn finish_json(child: Child, timeout: Duration) -> Result<Value, String> {
    let output = wait_for_output(child, timeout)?;
    if output.stdout.trim().is_empty() {
        return Err("command returned empty stdout".to_string());
    }
    serde_json::from_str(output.stdout.trim()).map_err(|err| {
        format!(
            "parse command JSON output: {err}\nstdout: {}",
            output.stdout
        )
    })
}

fn finish_ndjson(child: Child, timeout: Duration) -> Result<Vec<Value>, String> {
    let output = wait_for_output(child, timeout)?;
    if output.stdout.trim().is_empty() {
        return Err("command returned empty stdout".to_string());
    }
    output
        .stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|err| format!("parse command NDJSON output: {err}\nline: {line}"))
        })
        .collect()
}

fn wait_for_output(mut child: Child, timeout: Duration) -> Result<CommandOutput, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_string(&mut stdout)
                        .map_err(|err| format!("read command stdout: {err}"))?;
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_string(&mut stderr)
                        .map_err(|err| format!("read command stderr: {err}"))?;
                }
                if !status.success() {
                    return Err(stderr
                        .trim()
                        .strip_prefix("")
                        .unwrap_or_default()
                        .to_string()
                        .trim()
                        .to_string()
                        .if_empty_then(stdout.trim().to_string())
                        .if_empty_then(format!(
                            "command exited with status {}",
                            status.code().unwrap_or(-1)
                        )));
                }
                return Ok(CommandOutput { stdout });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("command timed out after {}ms", timeout.as_millis()));
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(format!("wait for child process: {err}")),
        }
    }
}

fn child_command(kind: ToolKind) -> Result<Command, String> {
    let (binary_name, env_override) = match kind {
        ToolKind::Admin => ("bhctl", env::var_os("BU_RUST_ADMIN_BIN")),
        ToolKind::Runner => ("bhrun", env::var_os("BU_RUST_RUNNER_BIN")),
    };

    if let Some(program) = env_override
        .map(PathBuf::from)
        .or_else(|| sibling_binary_path(binary_name))
    {
        let mut command = Command::new(program);
        command.current_dir(repo_root());
        return Ok(command);
    }

    let mut command = Command::new("cargo");
    command
        .args(["run", "--quiet", "--bin", binary_name, "--"])
        .current_dir(workspace_root());
    Ok(command)
}

fn sibling_binary_path(name: &str) -> Option<PathBuf> {
    let current_exe = env::current_exe().ok()?;
    let parent = current_exe.parent()?;
    let sibling = installed_binary_path(parent, name);
    sibling.is_file().then_some(sibling)
}

fn installed_binary_path(directory: &Path, name: &str) -> PathBuf {
    if env::consts::EXE_EXTENSION.is_empty() {
        directory.join(name)
    } else {
        directory.join(format!("{name}.{}", env::consts::EXE_EXTENSION))
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn repo_root() -> PathBuf {
    workspace_root()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(workspace_root)
}

fn named_payload(name: &str, payload: Value) -> Result<Value, String> {
    let mut object = match payload {
        Value::Null => Map::new(),
        Value::Object(object) => object,
        _ => return Err("named payload must be a JSON object".to_string()),
    };
    object.insert("daemon_name".into(), Value::String(name.to_string()));
    Ok(Value::Object(object))
}

fn page_info(name: &str) -> Result<Value, String> {
    runner_json(
        "page-info",
        Some(named_payload(name, Value::Null)?),
        Duration::from_secs(10),
    )
}

fn new_tab(name: &str, url: &str) -> Result<String, String> {
    let value = runner_json(
        "new-tab",
        Some(named_payload(name, json!({"url": url}))?),
        Duration::from_secs(10),
    )?;
    required_string_field(&value, "target_id")
}

fn wait_for_load(name: &str) -> Result<bool, String> {
    let value = runner_json(
        "wait-for-load",
        Some(named_payload(name, json!({"timeout": 15.0}))?),
        Duration::from_secs(20),
    )?;
    Ok(value.as_bool().unwrap_or(false))
}

fn goto(name: &str, url: &str) -> Result<Value, String> {
    runner_json(
        "goto",
        Some(named_payload(name, json!({"url": url}))?),
        Duration::from_secs(15),
    )
}

fn js(name: &str, expression: &str) -> Result<Value, String> {
    runner_json(
        "js",
        Some(named_payload(name, json!({"expression": expression}))?),
        Duration::from_secs(20),
    )
}

fn current_session(name: &str) -> Result<Value, String> {
    runner_json(
        "current-session",
        Some(named_payload(name, Value::Null)?),
        Duration::from_secs(10),
    )
}

fn current_tab(name: &str) -> Result<Value, String> {
    runner_json(
        "current-tab",
        Some(named_payload(name, Value::Null)?),
        Duration::from_secs(10),
    )
}

fn list_tabs(name: &str) -> Result<Vec<Value>, String> {
    let value = runner_json(
        "list-tabs",
        Some(named_payload(name, Value::Null)?),
        Duration::from_secs(10),
    )?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "list-tabs did not return an array".to_string())
}

fn switch_tab(name: &str, target_id: &str) -> Result<Value, String> {
    runner_json(
        "switch-tab",
        Some(named_payload(name, json!({"target_id": target_id}))?),
        Duration::from_secs(10),
    )
}

fn drain_events(name: &str) -> Result<Vec<Value>, String> {
    let value = runner_json(
        "drain-events",
        Some(named_payload(name, Value::Null)?),
        Duration::from_secs(10),
    )?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "drain-events did not return an array".to_string())
}

fn dispatch_key(name: &str, selector: &str, key: &str, event: &str) -> Result<Value, String> {
    runner_json(
        "dispatch-key",
        Some(named_payload(
            name,
            json!({"selector": selector, "key": key, "event": event}),
        )?),
        Duration::from_secs(10),
    )
}

fn screenshot_b64(name: &str, full: bool) -> Result<String, String> {
    let value = runner_json(
        "screenshot",
        Some(named_payload(name, json!({"full": full}))?),
        Duration::from_secs(20),
    )?;
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "screenshot did not return a base64 string".to_string())
}

fn print_pdf_b64(name: &str, landscape: bool) -> Result<String, String> {
    let value = runner_json(
        "print-pdf",
        Some(named_payload(name, json!({"landscape": landscape}))?),
        Duration::from_secs(20),
    )?;
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "print-pdf did not return a base64 string".to_string())
}

fn set_viewport(
    name: &str,
    width: i64,
    height: i64,
    device_scale_factor: f64,
    mobile: bool,
) -> Result<Value, String> {
    runner_json(
        "set-viewport",
        Some(named_payload(
            name,
            json!({
                "width": width,
                "height": height,
                "device_scale_factor": device_scale_factor,
                "mobile": mobile,
            }),
        )?),
        Duration::from_secs(10),
    )
}

fn get_cookies(name: &str, urls: Vec<String>) -> Result<Value, String> {
    runner_json(
        "get-cookies",
        Some(named_payload(name, json!({"urls": urls}))?),
        Duration::from_secs(10),
    )
}

fn set_cookies(name: &str, cookies: Vec<Value>) -> Result<Value, String> {
    runner_json(
        "set-cookies",
        Some(named_payload(name, json!({"cookies": cookies}))?),
        Duration::from_secs(10),
    )
}

fn configure_downloads(name: &str, download_path: &Path) -> Result<Value, String> {
    runner_json(
        "configure-downloads",
        Some(named_payload(
            name,
            json!({"download_path": download_path.display().to_string()}),
        )?),
        Duration::from_secs(10),
    )
}

fn handle_dialog(name: &str, action: &str) -> Result<Value, String> {
    runner_json(
        "handle-dialog",
        Some(named_payload(name, json!({"action": action}))?),
        Duration::from_secs(10),
    )
}

fn cleanup_dialog_best_effort(name: &str) -> Value {
    let mut result = Map::new();
    let handle_dialog_request = match named_payload(name, json!({"action": "accept"})) {
        Ok(request) => request,
        Err(err) => {
            result.insert("handle_dialog_error".into(), Value::String(err));
            return Value::Object(result);
        }
    };
    result.insert(
        "handle_dialog_request".into(),
        handle_dialog_request.clone(),
    );
    match runner_json(
        "handle-dialog",
        Some(handle_dialog_request),
        Duration::from_secs(5),
    ) {
        Ok(handle_dialog_result) => {
            result.insert("handle_dialog_result".into(), handle_dialog_result);
        }
        Err(err) => {
            result.insert("handle_dialog_error".into(), Value::String(err));
            return Value::Object(result);
        }
    }

    sleep_ms(200);
    let page_info_request = match named_payload(name, Value::Null) {
        Ok(request) => request,
        Err(err) => {
            result.insert("page_info_error".into(), Value::String(err));
            return Value::Object(result);
        }
    };
    result.insert("page_info_request".into(), page_info_request.clone());
    match runner_json("page-info", Some(page_info_request), Duration::from_secs(5)) {
        Ok(page_info) => {
            result.insert("page_info".into(), page_info);
        }
        Err(err) => {
            result.insert("page_info_error".into(), Value::String(err));
        }
    }
    Value::Object(result)
}

fn mouse_move(name: &str, x: f64, y: f64, buttons: i64) -> Result<Value, String> {
    runner_json(
        "mouse-move",
        Some(named_payload(
            name,
            json!({"x": x, "y": y, "buttons": buttons}),
        )?),
        Duration::from_secs(10),
    )
}

fn mouse_down(
    name: &str,
    x: f64,
    y: f64,
    button: &str,
    buttons: i64,
    click_count: i64,
) -> Result<Value, String> {
    runner_json(
        "mouse-down",
        Some(named_payload(
            name,
            json!({
                "x": x,
                "y": y,
                "button": button,
                "buttons": buttons,
                "click_count": click_count,
            }),
        )?),
        Duration::from_secs(10),
    )
}

fn mouse_up(
    name: &str,
    x: f64,
    y: f64,
    button: &str,
    buttons: i64,
    click_count: i64,
) -> Result<Value, String> {
    runner_json(
        "mouse-up",
        Some(named_payload(
            name,
            json!({
                "x": x,
                "y": y,
                "button": button,
                "buttons": buttons,
                "click_count": click_count,
            }),
        )?),
        Duration::from_secs(10),
    )
}

fn upload_file(name: &str, selector: &str, files: &[PathBuf]) -> Result<Value, String> {
    let files = files
        .iter()
        .map(|path| Value::String(path.display().to_string()))
        .collect::<Vec<_>>();
    runner_json(
        "upload-file",
        Some(named_payload(
            name,
            json!({"selector": selector, "files": files}),
        )?),
        Duration::from_secs(10),
    )
}

fn decode_png_dimensions(encoded_png: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let png = BASE64_STANDARD
        .decode(encoded_png)
        .map_err(|err| format!("decode PNG base64: {err}"))?;
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("runner screenshot did not return a PNG".to_string());
    }
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    Ok((png, width, height))
}

fn decode_pdf(encoded: &str) -> Result<Vec<u8>, String> {
    let data = BASE64_STANDARD
        .decode(encoded)
        .map_err(|err| format!("decode PDF base64: {err}"))?;
    if !data.starts_with(b"%PDF-") {
        return Err("runner print-pdf did not return a PDF".to_string());
    }
    Ok(data)
}

fn assert_page_size(page: &Value, width: i64, height: i64, label: &str) -> Result<(), String> {
    if page.get("w").and_then(Value::as_i64) != Some(width)
        || page.get("h").and_then(Value::as_i64) != Some(height)
    {
        return Err(format!(
            "{label} viewport mismatch: expected {width}x{height}, got {}x{}",
            page.get("w").and_then(Value::as_i64).unwrap_or_default(),
            page.get("h").and_then(Value::as_i64).unwrap_or_default()
        ));
    }
    Ok(())
}

fn assert_dpr(metrics: &Value, expected: f64, label: &str) -> Result<(), String> {
    let actual = metrics
        .get("dpr")
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("{label} viewport metrics missing devicePixelRatio"))?;
    if (actual - expected).abs() > 0.05 {
        return Err(format!(
            "{label} viewport expected devicePixelRatio {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn required_string_field(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing string field {key:?} in {value}"))
}

fn required_i64_field(value: &Value, key: &str) -> Result<i64, String> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing integer field {key:?} in {value}"))
}

fn required_number_field(value: &Value, key: &str) -> Result<f64, String> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing numeric field {key:?} in {value}"))
}

fn unique_token(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}-{nanos:x}-{}", std::process::id())
}

fn wait_for_downloaded_file(path: &Path, timeout: Duration) -> Result<PathBuf, String> {
    let deadline = Instant::now() + timeout;
    let partial = PathBuf::from(format!("{}.crdownload", path.display()));
    while Instant::now() < deadline {
        if path.exists() && !partial.exists() {
            return Ok(path.to_path_buf());
        }
        sleep_ms(200);
    }
    Err(format!(
        "downloaded file did not appear at {}",
        path.display()
    ))
}

fn sleep_ms(milliseconds: u64) {
    thread::sleep(Duration::from_millis(milliseconds));
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Result<Self, String> {
        let path = env::temp_dir().join(format!("{prefix}-{}", unique_token("tmp")));
        fs::create_dir_all(&path)
            .map_err(|err| format!("create temp directory {}: {err}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

trait StringFallback {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringFallback for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}
