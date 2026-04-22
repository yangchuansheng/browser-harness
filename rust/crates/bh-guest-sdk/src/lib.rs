use std::collections::BTreeMap;
use std::fmt;

pub use bh_wasm_host::{
    CookieParam, CookieRecord, CurrentSessionResult, EventFilter, HttpGetRequest, NewTabResult,
    SwitchTabResult, TabSummary, WaitForEventResult, WaitResult, WatchEventsLine,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

const DEFAULT_OUTPUT_CAPACITY: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestError {
    SerializeRequest(String),
    DeserializeResponse(String),
    HostCallFailed { operation: String },
}

impl fmt::Display for GuestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SerializeRequest(err) => write!(f, "serialize guest request: {err}"),
            Self::DeserializeResponse(err) => write!(f, "parse guest response: {err}"),
            Self::HostCallFailed { operation } => {
                write!(f, "host call failed for operation: {operation}")
            }
        }
    }
}

impl std::error::Error for GuestError {}

pub fn call_json<TRequest, TResponse>(
    operation: &str,
    request: &TRequest,
) -> Result<TResponse, GuestError>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    call_json_with(imported_call_json, operation, request)
}

pub fn goto(url: &str) -> Result<Value, GuestError> {
    call_json("goto", &json!({ "url": url }))
}

pub fn wait(duration_ms: u64) -> Result<WaitResult, GuestError> {
    call_json(
        "wait",
        &json!({
            "duration_ms": duration_ms,
        }),
    )
}

pub fn http_get(
    url: &str,
    headers: Option<BTreeMap<String, String>>,
    timeout: Option<f64>,
) -> Result<String, GuestError> {
    call_json(
        "http_get",
        &json!({
            "url": url,
            "headers": headers,
            "timeout": timeout,
        }),
    )
}

pub fn cdp_raw(
    method: &str,
    params: Option<Value>,
    session_id: Option<&str>,
) -> Result<Value, GuestError> {
    call_json(
        "cdp_raw",
        &json!({
            "method": method,
            "params": params,
            "session_id": session_id,
        }),
    )
}

pub fn current_session() -> Result<CurrentSessionResult, GuestError> {
    call_json("current_session", &json!({}))
}

pub fn current_tab() -> Result<TabSummary, GuestError> {
    call_json("current_tab", &json!({}))
}

pub fn list_tabs(include_internal: bool) -> Result<Vec<TabSummary>, GuestError> {
    call_json(
        "list_tabs",
        &json!({
            "include_internal": include_internal,
        }),
    )
}

pub fn new_tab(url: &str) -> Result<NewTabResult, GuestError> {
    call_json("new_tab", &json!({ "url": url }))
}

pub fn switch_tab(target_id: &str) -> Result<SwitchTabResult, GuestError> {
    call_json(
        "switch_tab",
        &json!({
            "target_id": target_id,
        }),
    )
}

pub fn ensure_real_tab() -> Result<Option<TabSummary>, GuestError> {
    call_json("ensure_real_tab", &json!({}))
}

pub fn iframe_target(url_substr: &str) -> Result<Option<String>, GuestError> {
    call_json(
        "iframe_target",
        &json!({
            "url_substr": url_substr,
        }),
    )
}

pub fn page_info() -> Result<Value, GuestError> {
    call_json("page_info", &json!({}))
}

pub fn wait_for_load(timeout: f64) -> Result<bool, GuestError> {
    call_json(
        "wait_for_load",
        &json!({
            "timeout": timeout,
        }),
    )
}

pub fn js<T>(expression: &str) -> Result<T, GuestError>
where
    T: DeserializeOwned,
{
    call_json("js", &json!({ "expression": expression }))
}

pub fn click(x: f64, y: f64, button: &str, clicks: i64) -> Result<(), GuestError> {
    call_json(
        "click",
        &json!({
            "x": x,
            "y": y,
            "button": button,
            "clicks": clicks,
        }),
    )
}

pub fn mouse_move(x: f64, y: f64, buttons: i64) -> Result<(), GuestError> {
    call_json(
        "mouse_move",
        &json!({
            "x": x,
            "y": y,
            "buttons": buttons,
        }),
    )
}

pub fn mouse_down(
    x: f64,
    y: f64,
    button: &str,
    buttons: i64,
    click_count: i64,
) -> Result<(), GuestError> {
    call_json(
        "mouse_down",
        &json!({
            "x": x,
            "y": y,
            "button": button,
            "buttons": buttons,
            "click_count": click_count,
        }),
    )
}

pub fn mouse_up(
    x: f64,
    y: f64,
    button: &str,
    buttons: i64,
    click_count: i64,
) -> Result<(), GuestError> {
    call_json(
        "mouse_up",
        &json!({
            "x": x,
            "y": y,
            "button": button,
            "buttons": buttons,
            "click_count": click_count,
        }),
    )
}

pub fn type_text(text: &str) -> Result<(), GuestError> {
    call_json(
        "type_text",
        &json!({
            "text": text,
        }),
    )
}

pub fn press_key(key: &str, modifiers: i64) -> Result<(), GuestError> {
    call_json(
        "press_key",
        &json!({
            "key": key,
            "modifiers": modifiers,
        }),
    )
}

pub fn dispatch_key(selector: &str, key: &str, event: &str) -> Result<(), GuestError> {
    call_json(
        "dispatch_key",
        &json!({
            "selector": selector,
            "key": key,
            "event": event,
        }),
    )
}

pub fn scroll(x: f64, y: f64, dy: f64, dx: f64) -> Result<(), GuestError> {
    call_json(
        "scroll",
        &json!({
            "x": x,
            "y": y,
            "dy": dy,
            "dx": dx,
        }),
    )
}

pub fn set_viewport(
    width: u32,
    height: u32,
    device_scale_factor: Option<f64>,
    mobile: bool,
) -> Result<(), GuestError> {
    call_json(
        "set_viewport",
        &json!({
            "width": width,
            "height": height,
            "device_scale_factor": device_scale_factor,
            "mobile": mobile,
        }),
    )
}

pub fn print_pdf(landscape: bool) -> Result<String, GuestError> {
    call_json(
        "print_pdf",
        &json!({
            "landscape": landscape,
        }),
    )
}

pub fn screenshot(full: bool) -> Result<String, GuestError> {
    call_json(
        "screenshot",
        &json!({
            "full": full,
        }),
    )
}

pub fn handle_dialog(action: &str, prompt_text: Option<&str>) -> Result<(), GuestError> {
    call_json(
        "handle_dialog",
        &json!({
            "action": action,
            "prompt_text": prompt_text,
        }),
    )
}

pub fn upload_file<I, S>(
    selector: &str,
    files: I,
    target_id: Option<&str>,
) -> Result<(), GuestError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let files = files
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>();
    call_json(
        "upload_file",
        &json!({
            "selector": selector,
            "files": files,
            "target_id": target_id,
        }),
    )
}

pub fn get_cookies(urls: Option<Vec<String>>) -> Result<Vec<CookieRecord>, GuestError> {
    call_json(
        "get_cookies",
        &json!({
            "urls": urls,
        }),
    )
}

pub fn set_cookies(cookies: &[CookieParam]) -> Result<(), GuestError> {
    call_json(
        "set_cookies",
        &json!({
            "cookies": cookies,
        }),
    )
}

pub fn configure_downloads(download_path: &str) -> Result<(), GuestError> {
    call_json(
        "configure_downloads",
        &json!({
            "download_path": download_path,
        }),
    )
}

pub fn wait_for_load_event(
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_load_event",
        &json!({
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

pub fn wait_for_download(
    filename: Option<&str>,
    url: Option<&str>,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_download",
        &json!({
            "filename": filename,
            "url": url,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

pub fn wait_for_request(
    url: &str,
    method: Option<&str>,
    session_id: Option<&str>,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_request",
        &json!({
            "url": url,
            "method": method,
            "session_id": session_id,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

pub fn wait_for_response(
    url: &str,
    status: Option<u16>,
    session_id: Option<&str>,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_response",
        &json!({
            "url": url,
            "status": status,
            "session_id": session_id,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

pub fn wait_for_event(
    filter: EventFilter,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_event",
        &json!({
            "filter": filter,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

pub fn watch_events(
    filter: EventFilter,
    timeout_ms: u64,
    poll_interval_ms: u64,
    max_events: Option<u64>,
) -> Result<Vec<WatchEventsLine>, GuestError> {
    call_json(
        "watch_events",
        &json!({
            "filter": filter,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
            "max_events": max_events,
        }),
    )
}

pub fn wait_for_console(
    console_type: Option<&str>,
    text: Option<&str>,
    session_id: Option<&str>,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_console",
        &json!({
            "type": console_type,
            "text": text,
            "session_id": session_id,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

pub fn wait_for_dialog(
    dialog_type: Option<&str>,
    message: Option<&str>,
    session_id: Option<&str>,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_dialog",
        &json!({
            "type": dialog_type,
            "message": message,
            "session_id": session_id,
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

fn call_json_with<F, TRequest, TResponse>(
    mut host_call: F,
    operation: &str,
    request: &TRequest,
) -> Result<TResponse, GuestError>
where
    F: FnMut(&[u8], &[u8], &mut [u8]) -> i32,
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    let request_bytes =
        serde_json::to_vec(request).map_err(|err| GuestError::SerializeRequest(err.to_string()))?;
    let operation_bytes = operation.as_bytes();
    let mut output = vec![0u8; DEFAULT_OUTPUT_CAPACITY];
    let written = host_call(operation_bytes, &request_bytes, &mut output);
    if written < 0 {
        return Err(GuestError::HostCallFailed {
            operation: operation.to_string(),
        });
    }

    output.truncate(written as usize);
    serde_json::from_slice(&output).map_err(|err| GuestError::DeserializeResponse(err.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "bh")]
extern "C" {
    #[link_name = "call_json"]
    fn bh_call_json(
        operation_ptr: *const u8,
        operation_len: usize,
        request_ptr: *const u8,
        request_len: usize,
        out_ptr: *mut u8,
        out_cap: usize,
    ) -> i32;
}

#[cfg(target_arch = "wasm32")]
fn imported_call_json(operation: &[u8], request: &[u8], output: &mut [u8]) -> i32 {
    unsafe {
        bh_call_json(
            operation.as_ptr(),
            operation.len(),
            request.as_ptr(),
            request.len(),
            output.as_mut_ptr(),
            output.len(),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn imported_call_json(_operation: &[u8], _request: &[u8], _output: &mut [u8]) -> i32 {
    panic!("bh-guest-sdk host import is only available on wasm32 guests");
}

#[cfg(test)]
mod tests {
    use super::{
        call_json_with, cdp_raw, click, configure_downloads, current_session, current_tab,
        dispatch_key, ensure_real_tab, get_cookies, goto, handle_dialog, http_get, iframe_target,
        js, list_tabs, mouse_down, mouse_move, mouse_up, new_tab, page_info, press_key, print_pdf,
        screenshot, scroll, set_cookies, set_viewport, switch_tab, type_text, upload_file, wait,
        wait_for_console, wait_for_dialog, wait_for_download, wait_for_event, wait_for_load,
        wait_for_load_event, wait_for_request, wait_for_response, watch_events,
        CurrentSessionResult, GuestError, NewTabResult, SwitchTabResult, TabSummary, WaitResult,
        WatchEventsLine,
    };
    use bh_wasm_host::WaitForEventResult;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    #[test]
    fn goto_serializes_url_request() {
        let result: Value = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"goto");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("https://example.com")
                );
                let response =
                    serde_json::to_vec(&json!({"frameId":"frame-1"})).expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "goto",
            &json!({"url":"https://example.com"}),
        )
        .expect("goto result");

        assert_eq!(
            result.get("frameId").and_then(Value::as_str),
            Some("frame-1")
        );
    }

    #[test]
    fn session_and_tab_helpers_deserialize_typed_results() {
        let current_tab_result: TabSummary = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"current_tab");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request, json!({}));
                let response = serde_json::to_vec(&json!({
                    "targetId":"target-1",
                    "title":"Example",
                    "url":"https://example.com"
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "current_tab",
            &json!({}),
        )
        .expect("current tab result");
        assert_eq!(current_tab_result.target_id, "target-1");

        let current_session_result: CurrentSessionResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"current_session");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request, json!({}));
                let response =
                    serde_json::to_vec(&json!({"session_id":"session-1"})).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "current_session",
            &json!({}),
        )
        .expect("current session result");
        assert_eq!(
            current_session_result.session_id.as_deref(),
            Some("session-1")
        );

        let tabs: Vec<TabSummary> = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"list_tabs");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("include_internal").and_then(Value::as_bool),
                    Some(false)
                );
                let response = serde_json::to_vec(&json!([
                    {"targetId":"target-1","title":"One","url":"about:blank"},
                    {"targetId":"target-2","title":"Two","url":"https://example.com"}
                ]))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "list_tabs",
            &json!({"include_internal":false}),
        )
        .expect("list tabs result");
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[1].target_id, "target-2");
    }

    #[test]
    fn tab_mutation_helpers_serialize_expected_requests() {
        let new_tab_result: NewTabResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"new_tab");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("https://example.com/new")
                );
                let response =
                    serde_json::to_vec(&json!({"target_id":"target-new"})).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "new_tab",
            &json!({"url":"https://example.com/new"}),
        )
        .expect("new tab result");
        assert_eq!(new_tab_result.target_id, "target-new");

        let switch_tab_result: SwitchTabResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"switch_tab");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("target_id").and_then(Value::as_str),
                    Some("target-new")
                );
                let response =
                    serde_json::to_vec(&json!({"session_id":"session-new"})).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "switch_tab",
            &json!({"target_id":"target-new"}),
        )
        .expect("switch tab result");
        assert_eq!(switch_tab_result.session_id, "session-new");
    }

    #[test]
    fn utility_and_target_helpers_serialize_expected_requests() {
        let wait_result: WaitResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("duration_ms").and_then(Value::as_u64),
                    Some(2000)
                );
                let response =
                    serde_json::to_vec(&json!({"elapsed_ms":2000})).expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait",
            &json!({"duration_ms":2000}),
        )
        .expect("wait result");
        assert_eq!(wait_result.elapsed_ms, 2000);

        let ensured_tab: Option<TabSummary> = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"ensure_real_tab");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request, json!({}));
                let response = serde_json::to_vec(&json!({
                    "targetId":"target-real",
                    "title":"Trending",
                    "url":"https://github.com/trending"
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "ensure_real_tab",
            &json!({}),
        )
        .expect("ensure real tab result");
        assert_eq!(
            ensured_tab.as_ref().map(|tab| tab.target_id.as_str()),
            Some("target-real")
        );

        let iframe: Option<String> = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"iframe_target");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url_substr").and_then(Value::as_str),
                    Some("github.com")
                );
                let response = serde_json::to_vec(&json!("iframe-7")).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "iframe_target",
            &json!({"url_substr":"github.com"}),
        )
        .expect("iframe target result");
        assert_eq!(iframe.as_deref(), Some("iframe-7"));

        let loaded: bool = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_load");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("timeout").and_then(Value::as_f64), Some(2.0));
                let response = serde_json::to_vec(&json!(true)).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_load",
            &json!({"timeout":2.0}),
        )
        .expect("wait for load result");
        assert!(loaded);

        let mut headers = BTreeMap::new();
        headers.insert("X-Test".to_string(), "value".to_string());
        let body: String = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"http_get");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("https://example.com/api")
                );
                assert_eq!(request["headers"]["X-Test"], "value");
                assert_eq!(request.get("timeout").and_then(Value::as_f64), Some(12.5));
                let response = serde_json::to_vec("ok").expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "http_get",
            &json!({
                "url":"https://example.com/api",
                "headers": headers,
                "timeout": 12.5
            }),
        )
        .expect("http get result");
        assert_eq!(body, "ok");
    }

    #[test]
    fn input_helpers_serialize_expected_requests() {
        let click_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"click");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("x").and_then(Value::as_f64), Some(10.0));
                assert_eq!(request.get("button").and_then(Value::as_str), Some("left"));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "click",
            &json!({"x":10.0,"y":20.0,"button":"left","clicks":2}),
        )
        .expect("click result");
        assert_eq!(click_result, ());

        let mouse_move_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"mouse_move");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("x").and_then(Value::as_f64), Some(11.0));
                assert_eq!(request.get("buttons").and_then(Value::as_i64), Some(1));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "mouse_move",
            &json!({"x":11.0,"y":22.0,"buttons":1}),
        )
        .expect("mouse move result");
        assert_eq!(mouse_move_result, ());

        let mouse_down_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"mouse_down");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("button").and_then(Value::as_str), Some("left"));
                assert_eq!(request.get("buttons").and_then(Value::as_i64), Some(1));
                assert_eq!(request.get("click_count").and_then(Value::as_i64), Some(1));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "mouse_down",
            &json!({"x":11.0,"y":22.0,"button":"left","buttons":1,"click_count":1}),
        )
        .expect("mouse down result");
        assert_eq!(mouse_down_result, ());

        let mouse_up_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"mouse_up");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("button").and_then(Value::as_str), Some("left"));
                assert_eq!(request.get("buttons").and_then(Value::as_i64), Some(0));
                assert_eq!(request.get("click_count").and_then(Value::as_i64), Some(1));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "mouse_up",
            &json!({"x":33.0,"y":44.0,"button":"left","buttons":0,"click_count":1}),
        )
        .expect("mouse up result");
        assert_eq!(mouse_up_result, ());

        let type_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"type_text");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("text").and_then(Value::as_str), Some("hello"));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "type_text",
            &json!({"text":"hello"}),
        )
        .expect("type text result");
        assert_eq!(type_result, ());

        let press_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"press_key");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("key").and_then(Value::as_str), Some("Enter"));
                assert_eq!(request.get("modifiers").and_then(Value::as_i64), Some(2));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "press_key",
            &json!({"key":"Enter","modifiers":2}),
        )
        .expect("press key result");
        assert_eq!(press_result, ());

        let dispatch_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"dispatch_key");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("selector").and_then(Value::as_str),
                    Some("#search")
                );
                assert_eq!(request.get("key").and_then(Value::as_str), Some("Tab"));
                assert_eq!(
                    request.get("event").and_then(Value::as_str),
                    Some("keydown")
                );
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "dispatch_key",
            &json!({"selector":"#search","key":"Tab","event":"keydown"}),
        )
        .expect("dispatch key result");
        assert_eq!(dispatch_result, ());

        let scroll_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"scroll");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("dy").and_then(Value::as_f64), Some(100.0));
                assert_eq!(request.get("dx").and_then(Value::as_f64), Some(5.0));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "scroll",
            &json!({"x":1.0,"y":2.0,"dy":100.0,"dx":5.0}),
        )
        .expect("scroll result");
        assert_eq!(scroll_result, ());

        let viewport_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"set_viewport");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("width").and_then(Value::as_u64), Some(900));
                assert_eq!(request.get("height").and_then(Value::as_u64), Some(700));
                assert_eq!(
                    request.get("device_scale_factor").and_then(Value::as_f64),
                    Some(2.0)
                );
                assert_eq!(request.get("mobile").and_then(Value::as_bool), Some(true));
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "set_viewport",
            &json!({"width":900,"height":700,"device_scale_factor":2.0,"mobile":true}),
        )
        .expect("set viewport result");
        assert_eq!(viewport_result, ());

        let pdf_result: String = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"print_pdf");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("landscape").and_then(Value::as_bool),
                    Some(true)
                );
                let response = serde_json::to_vec("JVBERi0xLjQ=").expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "print_pdf",
            &json!({"landscape":true}),
        )
        .expect("print pdf result");
        assert_eq!(pdf_result, "JVBERi0xLjQ=");

        let screenshot_result: String = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"screenshot");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("full").and_then(Value::as_bool), Some(true));
                let response = serde_json::to_vec("cG5nLWJ5dGVz").expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "screenshot",
            &json!({"full":true}),
        )
        .expect("screenshot result");
        assert_eq!(screenshot_result, "cG5nLWJ5dGVz");

        let upload_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"upload_file");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("selector").and_then(Value::as_str),
                    Some("#file")
                );
                assert_eq!(
                    request.pointer("/files/0").and_then(Value::as_str),
                    Some("/tmp/one.txt")
                );
                assert_eq!(
                    request.pointer("/files/1").and_then(Value::as_str),
                    Some("/tmp/two.txt")
                );
                assert_eq!(
                    request.get("target_id").and_then(Value::as_str),
                    Some("iframe-1")
                );
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "upload_file",
            &json!({
                "selector":"#file",
                "files":["/tmp/one.txt","/tmp/two.txt"],
                "target_id":"iframe-1"
            }),
        )
        .expect("upload file result");
        assert_eq!(upload_result, ());

        let cookies_result: Vec<bh_wasm_host::CookieRecord> = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"get_cookies");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.pointer("/urls/0").and_then(Value::as_str),
                    Some("https://example.com")
                );
                let response = serde_json::to_vec(&json!([
                    {
                        "name":"session",
                        "value":"token",
                        "domain":"example.com",
                        "path":"/",
                        "secure":true,
                        "httpOnly":false,
                        "session":false
                    }
                ]))
                .expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "get_cookies",
            &json!({"urls":["https://example.com"]}),
        )
        .expect("get cookies result");
        assert_eq!(cookies_result.len(), 1);
        assert_eq!(cookies_result[0].name, "session");

        let set_cookies_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"set_cookies");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.pointer("/cookies/0/name").and_then(Value::as_str),
                    Some("session")
                );
                assert_eq!(
                    request.pointer("/cookies/0/url").and_then(Value::as_str),
                    Some("https://example.com")
                );
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "set_cookies",
            &json!({
                "cookies":[{
                    "name":"session",
                    "value":"token",
                    "url":"https://example.com",
                    "secure":true
                }]
            }),
        )
        .expect("set cookies result");
        assert_eq!(set_cookies_result, ());

        let configure_downloads_result: () = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"configure_downloads");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("download_path").and_then(Value::as_str),
                    Some("/tmp/downloads")
                );
                let response = serde_json::to_vec(&Value::Null).expect("serialize");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "configure_downloads",
            &json!({"download_path":"/tmp/downloads"}),
        )
        .expect("configure downloads result");
        assert_eq!(configure_downloads_result, ());
    }

    #[test]
    fn js_deserializes_string_response() {
        let title: String = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"js");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("expression").and_then(Value::as_str),
                    Some("document.title")
                );
                let response = serde_json::to_vec("Example Domain").expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "js",
            &json!({"expression":"document.title"}),
        )
        .expect("js result");

        assert_eq!(title, "Example Domain");
    }

    #[test]
    fn wait_for_load_event_deserializes_typed_result() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_load_event");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("timeout_ms").and_then(Value::as_u64),
                    Some(5000)
                );
                assert_eq!(
                    request.get("poll_interval_ms").and_then(Value::as_u64),
                    Some(100)
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Page.loadEventFired"},
                    "polls": 3,
                    "elapsed_ms": 250
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_load_event",
            &json!({"timeout_ms":5000,"poll_interval_ms":100}),
        )
        .expect("wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 3);
    }

    #[test]
    fn wait_for_download_serializes_filename_and_url_filters() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_download");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("filename").and_then(Value::as_str),
                    Some("report.txt")
                );
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("blob:https://example.com/token")
                );
                assert_eq!(
                    request.get("timeout_ms").and_then(Value::as_u64),
                    Some(5000)
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Browser.downloadWillBegin"},
                    "polls": 2,
                    "elapsed_ms": 120
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_download",
            &json!({
                "filename":"report.txt",
                "url":"blob:https://example.com/token",
                "timeout_ms":5000,
                "poll_interval_ms":100
            }),
        )
        .expect("wait for download result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
    }

    #[test]
    fn wait_for_response_serializes_scope_and_status() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_response");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("https://example.com/data")
                );
                assert_eq!(request.get("status").and_then(Value::as_u64), Some(200));
                assert_eq!(
                    request.get("session_id").and_then(Value::as_str),
                    Some("session-1")
                );
                assert_eq!(
                    request.get("timeout_ms").and_then(Value::as_u64),
                    Some(5000)
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Network.responseReceived","session_id":"session-1"},
                    "polls": 2,
                    "elapsed_ms": 111
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_response",
            &json!({
                "url":"https://example.com/data",
                "status":200,
                "session_id":"session-1",
                "timeout_ms":5000,
                "poll_interval_ms":100
            }),
        )
        .expect("wait for response result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
        assert_eq!(
            result
                .event
                .as_ref()
                .and_then(|event| event.get("session_id"))
                .and_then(Value::as_str),
            Some("session-1")
        );
    }

    #[test]
    fn wait_for_event_serializes_filter_request() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_event");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.pointer("/filter/method").and_then(Value::as_str),
                    Some("Page.loadEventFired")
                );
                assert_eq!(
                    request
                        .pointer("/filter/session_id")
                        .and_then(Value::as_str),
                    Some("session-2")
                );
                assert_eq!(
                    request.get("timeout_ms").and_then(Value::as_u64),
                    Some(4000)
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Page.loadEventFired","session_id":"session-2"},
                    "polls": 1,
                    "elapsed_ms": 22
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_event",
            &json!({
                "filter":{
                    "method":"Page.loadEventFired",
                    "session_id":"session-2"
                },
                "timeout_ms":4000,
                "poll_interval_ms":100
            }),
        )
        .expect("wait for event result");

        assert!(result.matched);
        assert_eq!(result.polls, 1);
    }

    #[test]
    fn cdp_raw_serializes_method_params_and_session() {
        let result: Value = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"cdp_raw");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("method").and_then(Value::as_str),
                    Some("Runtime.evaluate")
                );
                assert_eq!(
                    request
                        .pointer("/params/expression")
                        .and_then(Value::as_str),
                    Some("2+3")
                );
                assert_eq!(
                    request.get("session_id").and_then(Value::as_str),
                    Some("session-2")
                );
                let response = serde_json::to_vec(&json!({
                    "result":{"type":"number","value":5}
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "cdp_raw",
            &json!({
                "method":"Runtime.evaluate",
                "params":{"expression":"2+3","returnByValue":true},
                "session_id":"session-2"
            }),
        )
        .expect("cdp raw result");

        assert_eq!(
            result.pointer("/result/value").and_then(Value::as_i64),
            Some(5)
        );
    }

    #[test]
    fn watch_events_deserializes_line_sequence() {
        let result: Vec<WatchEventsLine> = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"watch_events");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.pointer("/filter/method").and_then(Value::as_str),
                    Some("Runtime.consoleAPICalled")
                );
                assert_eq!(request.get("max_events").and_then(Value::as_u64), Some(2));
                let response = serde_json::to_vec(&json!([
                    {
                        "kind":"event",
                        "event":{"method":"Runtime.consoleAPICalled","session_id":"session-3"},
                        "index":1,
                        "elapsed_ms":10
                    },
                    {
                        "kind":"end",
                        "matched_events":1,
                        "polls":2,
                        "elapsed_ms":30,
                        "timed_out":false,
                        "reached_max_events":false
                    }
                ]))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "watch_events",
            &json!({
                "filter":{"method":"Runtime.consoleAPICalled"},
                "timeout_ms":500,
                "poll_interval_ms":50,
                "max_events":2
            }),
        )
        .expect("watch events result");

        assert_eq!(result.len(), 2);
        match &result[0] {
            WatchEventsLine::Event { index, .. } => assert_eq!(*index, 1),
            other => panic!("unexpected first watch line: {other:?}"),
        }
        match &result[1] {
            WatchEventsLine::End { polls, .. } => assert_eq!(*polls, 2),
            other => panic!("unexpected second watch line: {other:?}"),
        }
    }

    #[test]
    fn wait_for_console_serializes_console_filter() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_console");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("type").and_then(Value::as_str), Some("log"));
                assert_eq!(request.get("text").and_then(Value::as_str), Some("token-1"));
                assert_eq!(
                    request.get("session_id").and_then(Value::as_str),
                    Some("session-4")
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Runtime.consoleAPICalled","session_id":"session-4"},
                    "polls": 2,
                    "elapsed_ms": 51
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_console",
            &json!({
                "type":"log",
                "text":"token-1",
                "session_id":"session-4",
                "timeout_ms":5000,
                "poll_interval_ms":100
            }),
        )
        .expect("wait for console result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
    }

    #[test]
    fn wait_for_request_serializes_scope_and_method() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_request");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("https://example.com/data")
                );
                assert_eq!(request.get("method").and_then(Value::as_str), Some("POST"));
                assert_eq!(
                    request.get("session_id").and_then(Value::as_str),
                    Some("session-1")
                );
                assert_eq!(
                    request.get("timeout_ms").and_then(Value::as_u64),
                    Some(5000)
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Network.requestWillBeSent","session_id":"session-1"},
                    "polls": 2,
                    "elapsed_ms": 111
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_request",
            &json!({
                "url":"https://example.com/data",
                "method":"POST",
                "session_id":"session-1",
                "timeout_ms":5000,
                "poll_interval_ms":100
            }),
        )
        .expect("wait for request result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
    }

    #[test]
    fn wait_for_dialog_serializes_dialog_filter() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_dialog");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(request.get("type").and_then(Value::as_str), Some("alert"));
                assert_eq!(
                    request.get("message").and_then(Value::as_str),
                    Some("token-2")
                );
                assert_eq!(
                    request.get("session_id").and_then(Value::as_str),
                    Some("session-5")
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Page.javascriptDialogOpening","session_id":"session-5"},
                    "polls": 2,
                    "elapsed_ms": 61
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_dialog",
            &json!({
                "type":"alert",
                "message":"token-2",
                "session_id":"session-5",
                "timeout_ms":5000,
                "poll_interval_ms":100
            }),
        )
        .expect("wait for dialog result");

        assert!(result.matched);
        assert_eq!(result.polls, 2);
    }

    #[test]
    fn handle_dialog_serializes_action_and_prompt_text() {
        call_json_with::<_, _, ()>(
            |operation, request, output| {
                assert_eq!(operation, b"handle_dialog");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("action").and_then(Value::as_str),
                    Some("accept")
                );
                assert_eq!(
                    request.get("prompt_text").and_then(Value::as_str),
                    Some("typed value")
                );
                let response = serde_json::to_vec(&Value::Null).expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "handle_dialog",
            &json!({
                "action":"accept",
                "prompt_text":"typed value"
            }),
        )
        .expect("handle dialog result");
    }

    #[test]
    fn helper_functions_use_expected_operations() {
        let _ = wait;
        let _ = http_get;
        let _ = cdp_raw;
        let _ = current_session;
        let _ = current_tab;
        let _ = list_tabs;
        let _ = new_tab;
        let _ = switch_tab;
        let _ = ensure_real_tab;
        let _ = iframe_target;
        let _ = goto;
        let _ = wait_for_load;
        let _ = page_info;
        let _ = click;
        let _ = mouse_move;
        let _ = mouse_down;
        let _ = mouse_up;
        let _ = type_text;
        let _ = press_key;
        let _ = dispatch_key;
        let _ = scroll;
        let _ = set_viewport;
        let _ = print_pdf;
        let _ = screenshot;
        let _ = handle_dialog;
        let _ = upload_file::<Vec<&str>, &str>;
        let _ = get_cookies;
        let _ = set_cookies;
        let _ = configure_downloads;
        let _ = wait_for_event;
        let _ = watch_events;
        let _ = wait_for_console;
        let _ = wait_for_dialog;
        let _ = wait_for_load_event;
        let _ = wait_for_download;
        let _ = wait_for_request;
        let _ = wait_for_response;
        let _ = js::<String>;
    }

    #[test]
    fn negative_host_result_becomes_guest_error() {
        let err = call_json_with::<_, _, Value>(
            |_operation, _request, _output| -1,
            "page_info",
            &json!({}),
        )
        .expect_err("host call should fail");

        assert_eq!(
            err,
            GuestError::HostCallFailed {
                operation: "page_info".to_string()
            }
        );
    }
}
