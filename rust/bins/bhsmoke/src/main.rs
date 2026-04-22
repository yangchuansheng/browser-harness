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
        if let Some(payload) = payload {
            let input = serde_json::to_string(&payload)
                .map_err(|err| format!("serialize stdin JSON: {err}"))?;
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
