use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::{Duration, Instant};

use bh_protocol::{
    DaemonRequest, DaemonResponse, META_CURRENT_TAB, META_DRAIN_EVENTS, META_GOTO, META_JS,
    META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO, META_SESSION, META_SWITCH_TAB,
};
use bh_wasm_host::{
    console_event_matches, default_manifest, default_runner_config, event_matches_filter,
    operation_names, CurrentSessionRequest, CurrentSessionResult, CurrentTabRequest, GotoRequest,
    JsRequest, ListTabsRequest, NewTabRequest, NewTabResult, PageInfoRequest, SwitchTabRequest,
    SwitchTabResult, TabSummary, WaitForConsoleRequest, WaitForDialogRequest, WaitForEventRequest,
    WaitForEventResult, WaitForLoadEventRequest, WaitForResponseRequest, WatchEventsLine,
    WatchEventsRequest,
};
use serde_json::{json, Value};

fn print_usage() {
    eprintln!(
        "usage: bhrun <manifest|sample-config|capabilities|summary|current-tab|list-tabs|new-tab|switch-tab|page-info|goto|js|current-session|wait-for-event|watch-events|wait-for-load-event|wait-for-response|wait-for-console|wait-for-dialog>\n\
         runner scaffold: event waiting is live; WASM guest execution is not implemented yet"
    );
}

fn main() {
    let exit_code = match run_cli(std::env::args().skip(1), io::stdin(), io::stdout()) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run_cli<I, R, W>(mut args: I, mut stdin: R, mut stdout: W) -> Result<(), String>
where
    I: Iterator<Item = String>,
    R: Read,
    W: Write,
{
    match args.next().as_deref() {
        Some("manifest") => write_json(&mut stdout, &default_manifest()),
        Some("sample-config") => write_json(&mut stdout, &default_runner_config()),
        Some("capabilities") => {
            for name in operation_names() {
                writeln!(stdout, "{name}").map_err(|err| format!("write stdout: {err}"))?;
            }
            Ok(())
        }
        Some("summary") => {
            let manifest = default_manifest();
            writeln!(
                stdout,
                "bhrun scaffold: execution_model={:?} guest_transport={:?} protocol_families={} operations={} current_tab=live list_tabs=live new_tab=live switch_tab=live page_info=live goto=live js=live current_session=live wait_for_event=live watch_events=live wait_for_response=live wait_for_console=live wait_for_dialog=live wasm_guests=not_implemented",
                manifest.execution_model,
                manifest.guest_transport,
                manifest.protocol_families.len(),
                manifest.operations.len()
            )
            .map_err(|err| format!("write stdout: {err}"))
        }
        Some("current-tab") => {
            let request =
                read_optional_json::<CurrentTabRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = current_tab(request)?;
            write_json(&mut stdout, &result)
        }
        Some("list-tabs") => {
            let request = read_optional_json::<ListTabsRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = list_tabs(request)?;
            write_json(&mut stdout, &result)
        }
        Some("new-tab") => {
            let request = read_optional_json::<NewTabRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = new_tab(request)?;
            write_json(&mut stdout, &result)
        }
        Some("switch-tab") => {
            let request = read_json::<SwitchTabRequest, _>(&mut stdin)?;
            let result = switch_tab(request)?;
            write_json(&mut stdout, &result)
        }
        Some("page-info") => {
            let request = read_optional_json::<PageInfoRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = page_info(request)?;
            write_json(&mut stdout, &result)
        }
        Some("goto") => {
            let request = read_json::<GotoRequest, _>(&mut stdin)?;
            let result = goto(request)?;
            write_json(&mut stdout, &result)
        }
        Some("js") => {
            let request = read_json::<JsRequest, _>(&mut stdin)?;
            let result = js(request)?;
            write_json(&mut stdout, &result)
        }
        Some("current-session") => {
            let request =
                read_optional_json::<CurrentSessionRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = current_session(request)?;
            write_json(&mut stdout, &result)
        }
        Some("wait-for-event") => {
            let request = read_json::<WaitForEventRequest, _>(&mut stdin)?;
            let result = wait_for_event(request)?;
            write_json(&mut stdout, &result)
        }
        Some("watch-events") => {
            let request = read_json::<WatchEventsRequest, _>(&mut stdin)?;
            watch_events(request, &mut stdout)
        }
        Some("wait-for-load-event") => {
            let request = read_json::<WaitForLoadEventRequest, _>(&mut stdin)?;
            let result = wait_for_load_event(request)?;
            write_json(&mut stdout, &result)
        }
        Some("wait-for-response") => {
            let request = read_json::<WaitForResponseRequest, _>(&mut stdin)?;
            let result = wait_for_response(request)?;
            write_json(&mut stdout, &result)
        }
        Some("wait-for-console") => {
            let request = read_json::<WaitForConsoleRequest, _>(&mut stdin)?;
            let result = wait_for_console(request)?;
            write_json(&mut stdout, &result)
        }
        Some("wait-for-dialog") => {
            let request = read_json::<WaitForDialogRequest, _>(&mut stdin)?;
            let result = wait_for_dialog(request)?;
            write_json(&mut stdout, &result)
        }
        _ => {
            print_usage();
            Err("unsupported bhrun command".to_string())
        }
    }
}

fn current_session(request: CurrentSessionRequest) -> Result<CurrentSessionResult, String> {
    current_session_with_sender(request, send_daemon_meta_request)
}

fn current_tab(request: CurrentTabRequest) -> Result<TabSummary, String> {
    current_tab_with_sender(request, send_daemon_request)
}

fn list_tabs(request: ListTabsRequest) -> Result<Vec<TabSummary>, String> {
    list_tabs_with_sender(request, send_daemon_request)
}

fn new_tab(request: NewTabRequest) -> Result<NewTabResult, String> {
    new_tab_with_sender(request, send_daemon_request)
}

fn switch_tab(request: SwitchTabRequest) -> Result<SwitchTabResult, String> {
    switch_tab_with_sender(request, send_daemon_request)
}

fn page_info(request: PageInfoRequest) -> Result<Value, String> {
    page_info_with_sender(request, send_daemon_request)
}

fn goto(request: GotoRequest) -> Result<Value, String> {
    goto_with_sender(request, send_daemon_request)
}

fn js(request: JsRequest) -> Result<Value, String> {
    js_with_sender(request, send_daemon_request)
}

fn current_session_with_sender<F>(
    request: CurrentSessionRequest,
    mut sender: F,
) -> Result<CurrentSessionResult, String>
where
    F: FnMut(&str, &str) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let response = sender(&request.daemon_name, META_SESSION)?;
    Ok(CurrentSessionResult {
        session_id: response.session_id.unwrap_or(None),
    })
}

fn current_tab_with_sender<F>(
    request: CurrentTabRequest,
    mut sender: F,
) -> Result<TabSummary, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(&request.daemon_name, META_CURRENT_TAB, None, &mut sender)
}

fn list_tabs_with_sender<F>(
    request: ListTabsRequest,
    mut sender: F,
) -> Result<Vec<TabSummary>, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_LIST_TABS,
        Some(json!({"include_internal": request.include_internal})),
        &mut sender,
    )
}

fn new_tab_with_sender<F>(request: NewTabRequest, mut sender: F) -> Result<NewTabResult, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let target_id: String = typed_meta_result_with_sender(
        &request.daemon_name,
        META_NEW_TAB,
        Some(json!({"url": request.url})),
        &mut sender,
    )?;
    Ok(NewTabResult { target_id })
}

fn switch_tab_with_sender<F>(
    request: SwitchTabRequest,
    mut sender: F,
) -> Result<SwitchTabResult, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let session_id: String = typed_meta_result_with_sender(
        &request.daemon_name,
        META_SWITCH_TAB,
        Some(json!({"target_id": request.target_id})),
        &mut sender,
    )?;
    Ok(SwitchTabResult { session_id })
}

fn page_info_with_sender<F>(request: PageInfoRequest, mut sender: F) -> Result<Value, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    meta_result_with_sender(&request.daemon_name, META_PAGE_INFO, None, &mut sender)
}

fn goto_with_sender<F>(request: GotoRequest, mut sender: F) -> Result<Value, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    meta_result_with_sender(
        &request.daemon_name,
        META_GOTO,
        Some(json!({"url": request.url})),
        &mut sender,
    )
}

fn js_with_sender<F>(request: JsRequest, mut sender: F) -> Result<Value, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let mut params =
        serde_json::Map::from_iter([("expression".to_string(), Value::String(request.expression))]);
    if let Some(target_id) = request.target_id {
        params.insert("target_id".to_string(), Value::String(target_id));
    }
    meta_result_with_sender(
        &request.daemon_name,
        META_JS,
        Some(Value::Object(params)),
        &mut sender,
    )
}

fn wait_for_event(request: WaitForEventRequest) -> Result<WaitForEventResult, String> {
    wait_for_event_with_drain(request, drain_events)
}

fn wait_for_load_event(request: WaitForLoadEventRequest) -> Result<WaitForEventResult, String> {
    wait_for_event(request.into_wait_for_event_request())
}

fn wait_for_response(request: WaitForResponseRequest) -> Result<WaitForEventResult, String> {
    wait_for_event(request.into_wait_for_event_request())
}

fn wait_for_console(request: WaitForConsoleRequest) -> Result<WaitForEventResult, String> {
    wait_for_console_with_drain(request, drain_events)
}

fn wait_for_dialog(request: WaitForDialogRequest) -> Result<WaitForEventResult, String> {
    wait_for_event(request.into_wait_for_event_request())
}

fn watch_events<W>(request: WatchEventsRequest, stdout: &mut W) -> Result<(), String>
where
    W: Write,
{
    watch_events_with_drain(request, stdout, drain_events)
}

fn wait_for_event_with_drain<F>(
    request: WaitForEventRequest,
    mut drain: F,
) -> Result<WaitForEventResult, String>
where
    F: FnMut(&str) -> Result<Vec<Value>, String>,
{
    let request = request.normalized();
    let start = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms);
    let poll_interval = Duration::from_millis(request.poll_interval_ms);
    let mut polls = 0;

    loop {
        polls += 1;
        let events = drain(&request.daemon_name)?;
        for event in events {
            if event_matches_filter(&event, &request.filter) {
                return Ok(WaitForEventResult {
                    matched: true,
                    event: Some(event),
                    polls,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                });
            }
        }

        if start.elapsed() >= timeout {
            return Ok(WaitForEventResult {
                matched: false,
                event: None,
                polls,
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        }

        thread::sleep(poll_interval.min(timeout.saturating_sub(start.elapsed())));
    }
}

fn wait_for_console_with_drain<F>(
    request: WaitForConsoleRequest,
    mut drain: F,
) -> Result<WaitForEventResult, String>
where
    F: FnMut(&str) -> Result<Vec<Value>, String>,
{
    let request = request.normalized();
    let start = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms);
    let poll_interval = Duration::from_millis(request.poll_interval_ms);
    let mut polls = 0;

    loop {
        polls += 1;
        let events = drain(&request.daemon_name)?;
        for event in events {
            if console_event_matches(&event, &request) {
                return Ok(WaitForEventResult {
                    matched: true,
                    event: Some(event),
                    polls,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                });
            }
        }

        if start.elapsed() >= timeout {
            return Ok(WaitForEventResult {
                matched: false,
                event: None,
                polls,
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        }

        thread::sleep(poll_interval.min(timeout.saturating_sub(start.elapsed())));
    }
}

fn watch_events_with_drain<W, F>(
    request: WatchEventsRequest,
    stdout: &mut W,
    mut drain: F,
) -> Result<(), String>
where
    W: Write,
    F: FnMut(&str) -> Result<Vec<Value>, String>,
{
    let request = request.normalized();
    let start = Instant::now();
    let timeout = Duration::from_millis(request.timeout_ms);
    let poll_interval = Duration::from_millis(request.poll_interval_ms);
    let mut polls = 0;
    let mut matched_events = 0;

    loop {
        polls += 1;
        let events = drain(&request.daemon_name)?;
        for event in events {
            if event_matches_filter(&event, &request.filter) {
                matched_events += 1;
                write_json_line(
                    stdout,
                    &WatchEventsLine::Event {
                        event,
                        index: matched_events,
                        elapsed_ms: start.elapsed().as_millis() as u64,
                    },
                )?;
                if request.max_events == Some(matched_events) {
                    return write_json_line(
                        stdout,
                        &WatchEventsLine::End {
                            matched_events,
                            polls,
                            elapsed_ms: start.elapsed().as_millis() as u64,
                            timed_out: false,
                            reached_max_events: true,
                        },
                    );
                }
            }
        }

        if start.elapsed() >= timeout {
            return write_json_line(
                stdout,
                &WatchEventsLine::End {
                    matched_events,
                    polls,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    timed_out: true,
                    reached_max_events: false,
                },
            );
        }

        thread::sleep(poll_interval.min(timeout.saturating_sub(start.elapsed())));
    }
}

fn drain_events(daemon_name: &str) -> Result<Vec<Value>, String> {
    Ok(send_daemon_meta_request(daemon_name, META_DRAIN_EVENTS)?
        .events
        .unwrap_or_default())
}

fn meta_result_with_sender<F>(
    daemon_name: &str,
    meta: &str,
    params: Option<Value>,
    mut sender: F,
) -> Result<Value, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let response = sender(
        daemon_name,
        &DaemonRequest {
            meta: Some(meta.to_string()),
            params,
            ..DaemonRequest::default()
        },
    )?;
    Ok(response.result.unwrap_or(Value::Null))
}

fn typed_meta_result_with_sender<T, F>(
    daemon_name: &str,
    meta: &str,
    params: Option<Value>,
    sender: F,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let result = meta_result_with_sender(daemon_name, meta, params, sender)?;
    serde_json::from_value(result).map_err(|err| format!("parse {meta} result: {err}"))
}

fn read_json<T, R>(stdin: &mut R) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
    R: Read,
{
    let mut text = String::new();
    stdin
        .read_to_string(&mut text)
        .map_err(|err| format!("read stdin: {err}"))?;
    if text.trim().is_empty() {
        return Err("expected JSON on stdin".to_string());
    }
    serde_json::from_str(text.trim()).map_err(|err| format!("invalid JSON on stdin: {err}"))
}

fn read_optional_json<T, R>(stdin: &mut R) -> Result<Option<T>, String>
where
    T: serde::de::DeserializeOwned,
    R: Read,
{
    let mut text = String::new();
    stdin
        .read_to_string(&mut text)
        .map_err(|err| format!("read stdin: {err}"))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(text.trim())
        .map(Some)
        .map_err(|err| format!("invalid JSON on stdin: {err}"))
}

fn send_daemon_meta_request(daemon_name: &str, meta: &str) -> Result<DaemonResponse, String> {
    send_daemon_request(
        daemon_name,
        &DaemonRequest {
            meta: Some(meta.to_string()),
            ..DaemonRequest::default()
        },
    )
}

fn send_daemon_request(
    daemon_name: &str,
    request: &DaemonRequest,
) -> Result<DaemonResponse, String> {
    let mut stream = UnixStream::connect(format!("/tmp/bu-{daemon_name}.sock"))
        .map_err(|err| format!("connect daemon socket: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("set write timeout: {err}"))?;

    let payload =
        serde_json::to_vec(request).map_err(|err| format!("serialize daemon request: {err}"))?;
    stream
        .write_all(&payload)
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|err| format!("write daemon request: {err}"))?;

    let mut response = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        let read = stream
            .read(&mut chunk)
            .map_err(|err| format!("read daemon response: {err}"))?;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&chunk[..read]);
        if response.ends_with(b"\n") {
            break;
        }
    }

    let response_text = String::from_utf8(response)
        .map_err(|err| format!("daemon response was not utf-8: {err}"))?;
    let parsed: DaemonResponse = serde_json::from_str(response_text.trim())
        .map_err(|err| format!("invalid daemon response JSON: {err}"))?;
    if let Some(error) = parsed.error.clone() {
        return Err(error);
    }
    Ok(parsed)
}

fn write_json<T, W>(stdout: &mut W, value: &T) -> Result<(), String>
where
    T: serde::Serialize,
    W: Write,
{
    let text =
        serde_json::to_string_pretty(value).map_err(|err| format!("serialize JSON: {err}"))?;
    writeln!(stdout, "{text}").map_err(|err| format!("write stdout: {err}"))
}

fn write_json_line<T, W>(stdout: &mut W, value: &T) -> Result<(), String>
where
    T: serde::Serialize,
    W: Write,
{
    let text = serde_json::to_string(value).map_err(|err| format!("serialize JSON: {err}"))?;
    stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush())
        .map_err(|err| format!("write stdout: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{
        current_session_with_sender, current_tab_with_sender, goto_with_sender, js_with_sender,
        list_tabs_with_sender, new_tab_with_sender, page_info_with_sender, run_cli,
        switch_tab_with_sender, wait_for_console_with_drain, wait_for_event_with_drain,
        watch_events_with_drain, DaemonResponse, META_CURRENT_TAB, META_GOTO, META_JS,
        META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO, META_SESSION, META_SWITCH_TAB,
    };
    use std::collections::VecDeque;
    use std::io;

    use bh_wasm_host::{
        CurrentSessionRequest, CurrentSessionResult, CurrentTabRequest, EventFilter, GotoRequest,
        JsRequest, ListTabsRequest, NewTabRequest, PageInfoRequest, SwitchTabRequest,
        WaitForConsoleRequest, WaitForDialogRequest, WaitForEventRequest, WaitForEventResult,
        WaitForLoadEventRequest, WaitForResponseRequest, WatchEventsRequest,
    };
    use serde_json::{json, Value};

    #[test]
    fn wait_for_event_matches_after_multiple_polls() {
        let mut drains = VecDeque::from(vec![
            Ok(vec![]),
            Ok(vec![json!({
                "method":"Page.loadEventFired",
                "params":{"frameId":"f-1"},
                "session_id":"session-1"
            })]),
        ]);
        let result = wait_for_event_with_drain(
            WaitForEventRequest {
                daemon_name: "stub".to_string(),
                filter: EventFilter {
                    method: Some("Page.loadEventFired".to_string()),
                    session_id: Some("session-1".to_string()),
                    ..EventFilter::default()
                },
                timeout_ms: 500,
                poll_interval_ms: 10,
            },
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
        assert_eq!(
            result.event,
            Some(json!({
                "method":"Page.loadEventFired",
                "params":{"frameId":"f-1"},
                "session_id":"session-1"
            }))
        );
    }

    #[test]
    fn wait_for_event_returns_timeout_result_without_match() {
        let mut drains = VecDeque::from(vec![Ok(vec![]), Ok(vec![])]);
        let result = wait_for_event_with_drain(
            WaitForEventRequest {
                daemon_name: "stub".to_string(),
                filter: EventFilter {
                    method: Some("Page.loadEventFired".to_string()),
                    ..EventFilter::default()
                },
                timeout_ms: 15,
                poll_interval_ms: 10,
            },
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("wait result");

        assert!(!result.matched);
        assert!(result.polls >= 2);
        assert!(result.elapsed_ms >= 10);
    }

    #[test]
    fn cli_wait_for_event_prints_json_result() {
        let input = r#"{"daemon_name":"stub","filter":{"method":"Runtime.consoleAPICalled","params_subset":{"type":"log"}},"timeout_ms":100,"poll_interval_ms":10}"#;
        let output = run_wait_for_event_cli(input, |_| {
            Ok(vec![json!({
                "method":"Runtime.consoleAPICalled",
                "params":{"type":"log"},
                "session_id":"session-2"
            })])
        })
        .expect("cli result");

        assert_eq!(output.matched, true);
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.get("method"))
                .and_then(Value::as_str),
            Some("Runtime.consoleAPICalled")
        );
    }

    #[test]
    fn page_info_uses_meta_request_result() {
        let result = page_info_with_sender(PageInfoRequest::default(), |daemon, request| {
            assert_eq!(daemon, "default");
            assert_eq!(request.meta.as_deref(), Some(META_PAGE_INFO));
            assert_eq!(request.params, None);
            Ok(DaemonResponse {
                result: Some(json!({"url":"about:blank","title":"","w":1280})),
                ..DaemonResponse::default()
            })
        })
        .expect("page info result");

        assert_eq!(
            result.pointer("/url").and_then(Value::as_str),
            Some("about:blank")
        );
    }

    #[test]
    fn goto_uses_meta_request_with_url() {
        let result = goto_with_sender(
            GotoRequest {
                daemon_name: "runner".to_string(),
                url: "https://example.com".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_GOTO));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("url"))
                        .and_then(Value::as_str),
                    Some("https://example.com")
                );
                Ok(DaemonResponse {
                    result: Some(json!({"frameId":"frame-1"})),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("goto result");

        assert_eq!(
            result.pointer("/frameId").and_then(Value::as_str),
            Some("frame-1")
        );
    }

    #[test]
    fn js_uses_meta_request_with_expression_and_target_id() {
        let result = js_with_sender(
            JsRequest {
                daemon_name: "runner".to_string(),
                expression: "location.href".to_string(),
                target_id: Some("iframe-7".to_string()),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_JS));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("expression"))
                        .and_then(Value::as_str),
                    Some("location.href")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("target_id"))
                        .and_then(Value::as_str),
                    Some("iframe-7")
                );
                Ok(DaemonResponse {
                    result: Some(json!("https://example.com/frame")),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("js result");

        assert_eq!(result.as_str(), Some("https://example.com/frame"));
    }

    #[test]
    fn current_tab_uses_meta_request_result() {
        let result = current_tab_with_sender(CurrentTabRequest::default(), |daemon, request| {
            assert_eq!(daemon, "default");
            assert_eq!(request.meta.as_deref(), Some(META_CURRENT_TAB));
            assert_eq!(request.params, None);
            Ok(DaemonResponse {
                result: Some(json!({
                    "targetId":"target-1",
                    "title":"Example",
                    "url":"https://example.com"
                })),
                ..DaemonResponse::default()
            })
        })
        .expect("current tab result");

        assert_eq!(result.target_id, "target-1");
        assert_eq!(result.url, "https://example.com");
    }

    #[test]
    fn list_tabs_uses_meta_request_flag() {
        let result = list_tabs_with_sender(
            ListTabsRequest {
                daemon_name: "runner".to_string(),
                include_internal: false,
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_LIST_TABS));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("include_internal"))
                        .and_then(Value::as_bool),
                    Some(false)
                );
                Ok(DaemonResponse {
                    result: Some(json!([
                        {"targetId":"target-1","title":"One","url":"about:blank"},
                        {"targetId":"target-2","title":"Two","url":"https://example.com"}
                    ])),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("list tabs result");

        assert_eq!(result.len(), 2);
        assert_eq!(result[1].target_id, "target-2");
    }

    #[test]
    fn new_tab_uses_meta_request_with_url() {
        let result = new_tab_with_sender(
            NewTabRequest {
                daemon_name: "runner".to_string(),
                url: "https://example.com/new".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_NEW_TAB));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("url"))
                        .and_then(Value::as_str),
                    Some("https://example.com/new")
                );
                Ok(DaemonResponse {
                    result: Some(json!("target-new")),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("new tab result");

        assert_eq!(result.target_id, "target-new");
    }

    #[test]
    fn switch_tab_uses_meta_request_with_target_id() {
        let result = switch_tab_with_sender(
            SwitchTabRequest {
                daemon_name: "runner".to_string(),
                target_id: "target-9".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_SWITCH_TAB));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("target_id"))
                        .and_then(Value::as_str),
                    Some("target-9")
                );
                Ok(DaemonResponse {
                    result: Some(json!("session-9")),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("switch tab result");

        assert_eq!(result.session_id, "session-9");
    }

    #[test]
    fn cli_summary_mentions_live_event_waiting() {
        let mut stdout = Vec::new();

        run_cli(
            vec!["summary".to_string()].into_iter(),
            io::empty(),
            &mut stdout,
        )
        .expect("summary");

        let text = String::from_utf8(stdout).expect("utf-8");
        assert!(text.contains("current_tab=live"));
        assert!(text.contains("list_tabs=live"));
        assert!(text.contains("new_tab=live"));
        assert!(text.contains("switch_tab=live"));
        assert!(text.contains("page_info=live"));
        assert!(text.contains("goto=live"));
        assert!(text.contains("js=live"));
        assert!(text.contains("current_session=live"));
        assert!(text.contains("wait_for_event=live"));
        assert!(text.contains("watch_events=live"));
        assert!(text.contains("wait_for_response=live"));
        assert!(text.contains("wait_for_console=live"));
        assert!(text.contains("wait_for_dialog=live"));
    }

    #[test]
    fn watch_events_streams_ndjson_events_and_end_summary() {
        let mut drains = VecDeque::from(vec![
            Ok(vec![json!({
                "method":"Network.requestWillBeSent",
                "session_id":"session-1"
            })]),
            Ok(vec![
                json!({
                    "method":"Page.loadEventFired",
                    "params":{"timestamp":1.0},
                    "session_id":"session-1"
                }),
                json!({
                    "method":"Page.loadEventFired",
                    "params":{"timestamp":2.0},
                    "session_id":"session-1"
                }),
            ]),
        ]);
        let mut stdout = Vec::new();

        watch_events_with_drain(
            WatchEventsRequest {
                daemon_name: "stub".to_string(),
                filter: EventFilter {
                    method: Some("Page.loadEventFired".to_string()),
                    session_id: Some("session-1".to_string()),
                    ..EventFilter::default()
                },
                timeout_ms: 500,
                poll_interval_ms: 10,
                max_events: Some(2),
            },
            &mut stdout,
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("watch events result");

        let lines = String::from_utf8(stdout).expect("utf-8");
        let parsed = lines
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("parse json line"))
            .collect::<Vec<_>>();

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].get("kind").and_then(Value::as_str), Some("event"));
        assert_eq!(parsed[0].get("index").and_then(Value::as_u64), Some(1));
        assert_eq!(
            parsed[1].pointer("/event/method").and_then(Value::as_str),
            Some("Page.loadEventFired")
        );
        assert_eq!(parsed[2].get("kind").and_then(Value::as_str), Some("end"));
        assert_eq!(
            parsed[2].get("reached_max_events").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            parsed[2].get("matched_events").and_then(Value::as_u64),
            Some(2)
        );
    }

    #[test]
    fn wait_for_load_event_ignores_other_sessions() {
        let mut drains = VecDeque::from(vec![
            Ok(vec![json!({
                "method":"Page.loadEventFired",
                "params":{"timestamp": 1.0},
                "session_id":"session-other"
            })]),
            Ok(vec![json!({
                "method":"Page.loadEventFired",
                "params":{"timestamp": 2.0},
                "session_id":"session-target"
            })]),
        ]);
        let result = wait_for_event_with_drain(
            WaitForLoadEventRequest {
                daemon_name: "stub".to_string(),
                session_id: Some("session-target".to_string()),
                timeout_ms: 500,
                poll_interval_ms: 10,
            }
            .into_wait_for_event_request(),
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("load event wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
        assert_eq!(
            result
                .event
                .as_ref()
                .and_then(|event| event.get("session_id"))
                .and_then(Value::as_str),
            Some("session-target")
        );
    }

    #[test]
    fn cli_wait_for_load_event_prints_json_result() {
        let output = wait_for_load_event_with_stub(
            r#"{"daemon_name":"stub","session_id":"session-2","timeout_ms":100,"poll_interval_ms":10}"#,
            |_| {
                Ok(vec![json!({
                    "method":"Page.loadEventFired",
                    "params":{"timestamp": 5.0},
                    "session_id":"session-2"
                })])
            },
        )
        .expect("cli result");

        assert_eq!(output.matched, true);
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.get("method"))
                .and_then(Value::as_str),
            Some("Page.loadEventFired")
        );
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.get("session_id"))
                .and_then(Value::as_str),
            Some("session-2")
        );
    }

    #[test]
    fn wait_for_response_ignores_other_urls_and_statuses() {
        let mut drains = VecDeque::from(vec![
            Ok(vec![json!({
                "method":"Network.responseReceived",
                "params":{"response":{"url":"https://example.com/other","status":200}},
                "session_id":"session-target"
            })]),
            Ok(vec![json!({
                "method":"Network.responseReceived",
                "params":{"response":{"url":"https://example.com/api","status":404}},
                "session_id":"session-target"
            })]),
            Ok(vec![json!({
                "method":"Network.responseReceived",
                "params":{"response":{"url":"https://example.com/api","status":200}},
                "session_id":"session-target"
            })]),
        ]);
        let result = wait_for_event_with_drain(
            WaitForResponseRequest {
                daemon_name: "stub".to_string(),
                session_id: Some("session-target".to_string()),
                url: "https://example.com/api".to_string(),
                status: Some(200),
                timeout_ms: 500,
                poll_interval_ms: 10,
            }
            .into_wait_for_event_request(),
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("response wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 3);
        assert_eq!(
            result
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/response/url"))
                .and_then(Value::as_str),
            Some("https://example.com/api")
        );
    }

    #[test]
    fn cli_wait_for_response_prints_json_result() {
        let output = wait_for_response_with_stub(
            r#"{"daemon_name":"stub","session_id":"session-2","url":"https://example.com/api","status":200,"timeout_ms":100,"poll_interval_ms":10}"#,
            |_| {
                Ok(vec![json!({
                    "method":"Network.responseReceived",
                    "params":{"response":{"url":"https://example.com/api","status":200}},
                    "session_id":"session-2"
                })])
            },
        )
        .expect("cli result");

        assert_eq!(output.matched, true);
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.get("method"))
                .and_then(Value::as_str),
            Some("Network.responseReceived")
        );
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/response/status"))
                .and_then(Value::as_u64),
            Some(200)
        );
    }

    #[test]
    fn wait_for_console_ignores_other_types_text_and_sessions() {
        let mut drains = VecDeque::from(vec![
            Ok(vec![json!({
                "method":"Console.messageAdded",
                "params":{"message":{"level":"error","text":"token-1"}},
                "session_id":"session-target"
            })]),
            Ok(vec![json!({
                "method":"Console.messageAdded",
                "params":{"message":{"level":"log","text":"token-2"}},
                "session_id":"session-target"
            })]),
            Ok(vec![json!({
                "method":"Runtime.consoleAPICalled",
                "params":{"type":"log","args":[{"type":"string","value":"token-1"}]},
                "session_id":"session-other"
            })]),
            Ok(vec![json!({
                "method":"Console.messageAdded",
                "params":{"message":{"level":"log","text":"token-1"}},
                "session_id":"session-target"
            })]),
        ]);
        let result = wait_for_console_with_drain(
            WaitForConsoleRequest {
                daemon_name: "stub".to_string(),
                session_id: Some("session-target".to_string()),
                console_type: Some("log".to_string()),
                text: Some("token-1".to_string()),
                timeout_ms: 500,
                poll_interval_ms: 10,
            },
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("console wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 4);
        assert_eq!(
            result
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/message/text"))
                .and_then(Value::as_str),
            Some("token-1")
        );
    }

    #[test]
    fn cli_wait_for_console_prints_json_result() {
        let output = wait_for_console_with_stub(
            r#"{"daemon_name":"stub","session_id":"session-2","type":"log","text":"token-7","timeout_ms":100,"poll_interval_ms":10}"#,
            |_| {
                Ok(vec![json!({
                    "method":"Console.messageAdded",
                    "params":{"message":{"level":"log","text":"token-7"}},
                    "session_id":"session-2"
                })])
            },
        )
        .expect("cli result");

        assert_eq!(output.matched, true);
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.get("method"))
                .and_then(Value::as_str),
            Some("Console.messageAdded")
        );
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/message/text"))
                .and_then(Value::as_str),
            Some("token-7")
        );
    }

    #[test]
    fn wait_for_dialog_ignores_other_types_messages_and_sessions() {
        let mut drains = VecDeque::from(vec![
            Ok(vec![json!({
                "method":"Page.javascriptDialogOpening",
                "params":{"type":"confirm","message":"token-1"},
                "session_id":"session-target"
            })]),
            Ok(vec![json!({
                "method":"Page.javascriptDialogOpening",
                "params":{"type":"alert","message":"token-2"},
                "session_id":"session-target"
            })]),
            Ok(vec![json!({
                "method":"Page.javascriptDialogOpening",
                "params":{"type":"alert","message":"token-1"},
                "session_id":"session-other"
            })]),
            Ok(vec![json!({
                "method":"Page.javascriptDialogOpening",
                "params":{"type":"alert","message":"token-1"},
                "session_id":"session-target"
            })]),
        ]);
        let result = wait_for_event_with_drain(
            WaitForDialogRequest {
                daemon_name: "stub".to_string(),
                session_id: Some("session-target".to_string()),
                dialog_type: Some("alert".to_string()),
                message: Some("token-1".to_string()),
                timeout_ms: 500,
                poll_interval_ms: 10,
            }
            .into_wait_for_event_request(),
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("dialog wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 4);
        assert_eq!(
            result
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/message"))
                .and_then(Value::as_str),
            Some("token-1")
        );
    }

    #[test]
    fn cli_wait_for_dialog_prints_json_result() {
        let output = wait_for_dialog_with_stub(
            r#"{"daemon_name":"stub","session_id":"session-2","type":"alert","message":"token-9","timeout_ms":100,"poll_interval_ms":10}"#,
            |_| {
                Ok(vec![json!({
                    "method":"Page.javascriptDialogOpening",
                    "params":{"type":"alert","message":"token-9"},
                    "session_id":"session-2"
                })])
            },
        )
        .expect("cli result");

        assert_eq!(output.matched, true);
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.get("method"))
                .and_then(Value::as_str),
            Some("Page.javascriptDialogOpening")
        );
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/type"))
                .and_then(Value::as_str),
            Some("alert")
        );
        assert_eq!(
            output
                .event
                .as_ref()
                .and_then(|event| event.pointer("/params/message"))
                .and_then(Value::as_str),
            Some("token-9")
        );
    }

    #[test]
    fn watch_events_with_stub_prints_ndjson_lines() {
        let output = watch_events_with_stub(
            r#"{"daemon_name":"stub","filter":{"method":"Page.loadEventFired","session_id":"session-2"},"timeout_ms":100,"poll_interval_ms":10,"max_events":1}"#,
            |_| {
                Ok(vec![json!({
                    "method":"Page.loadEventFired",
                    "params":{"timestamp":5.0},
                    "session_id":"session-2"
                })])
            },
        )
        .expect("cli result");

        let parsed = output
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("parse json line"))
            .collect::<Vec<_>>();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].get("kind").and_then(Value::as_str), Some("event"));
        assert_eq!(parsed[1].get("kind").and_then(Value::as_str), Some("end"));
        assert_eq!(
            parsed[1].get("reached_max_events").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn current_session_uses_sender_response() {
        let result =
            current_session_with_sender(CurrentSessionRequest::default(), |daemon, meta| {
                assert_eq!(daemon, "default");
                assert_eq!(meta, META_SESSION);
                Ok(DaemonResponse {
                    session_id: Some(Some("session-7".to_string())),
                    ..DaemonResponse::default()
                })
            })
            .expect("current session result");

        assert_eq!(
            result,
            CurrentSessionResult {
                session_id: Some("session-7".to_string())
            }
        )
    }

    #[test]
    fn cli_current_session_prints_json_result() {
        let request: CurrentSessionRequest =
            serde_json::from_str(r#"{"daemon_name":"runner"}"#).expect("parse request");
        let result = current_session_with_sender(request, |daemon, meta| {
            assert_eq!(daemon, "runner");
            assert_eq!(meta, META_SESSION);
            Ok(DaemonResponse {
                session_id: Some(Some("session-9".to_string())),
                ..DaemonResponse::default()
            })
        })
        .expect("current session");

        let text = serde_json::to_string(&result).expect("serialize result");
        assert_eq!(text, r#"{"session_id":"session-9"}"#);
    }

    fn run_wait_for_event_cli<F>(input: &str, drain: F) -> Result<WaitForEventResult, String>
    where
        F: FnMut(&str) -> Result<Vec<Value>, String>,
    {
        let request: WaitForEventRequest =
            serde_json::from_str(input).map_err(|err| format!("parse request: {err}"))?;
        wait_for_event_with_drain(request, drain)
    }

    fn wait_for_load_event_with_stub<F>(input: &str, drain: F) -> Result<WaitForEventResult, String>
    where
        F: FnMut(&str) -> Result<Vec<Value>, String>,
    {
        let request: WaitForLoadEventRequest =
            serde_json::from_str(input).map_err(|err| format!("parse request: {err}"))?;
        wait_for_event_with_drain(request.into_wait_for_event_request(), drain)
    }

    fn wait_for_response_with_stub<F>(input: &str, drain: F) -> Result<WaitForEventResult, String>
    where
        F: FnMut(&str) -> Result<Vec<Value>, String>,
    {
        let request: WaitForResponseRequest =
            serde_json::from_str(input).map_err(|err| format!("parse request: {err}"))?;
        wait_for_event_with_drain(request.into_wait_for_event_request(), drain)
    }

    fn wait_for_console_with_stub<F>(input: &str, drain: F) -> Result<WaitForEventResult, String>
    where
        F: FnMut(&str) -> Result<Vec<Value>, String>,
    {
        let request: WaitForConsoleRequest =
            serde_json::from_str(input).map_err(|err| format!("parse request: {err}"))?;
        wait_for_console_with_drain(request, drain)
    }

    fn wait_for_dialog_with_stub<F>(input: &str, drain: F) -> Result<WaitForEventResult, String>
    where
        F: FnMut(&str) -> Result<Vec<Value>, String>,
    {
        let request: WaitForDialogRequest =
            serde_json::from_str(input).map_err(|err| format!("parse request: {err}"))?;
        wait_for_event_with_drain(request.into_wait_for_event_request(), drain)
    }

    fn watch_events_with_stub<F>(input: &str, drain: F) -> Result<String, String>
    where
        F: FnMut(&str) -> Result<Vec<Value>, String>,
    {
        let request: WatchEventsRequest =
            serde_json::from_str(input).map_err(|err| format!("parse request: {err}"))?;
        let mut stdout = Vec::new();
        watch_events_with_drain(request, &mut stdout, drain)?;
        String::from_utf8(stdout).map_err(|err| format!("utf-8: {err}"))
    }
}
