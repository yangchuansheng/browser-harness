use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use bh_protocol::{
    DaemonRequest, DaemonResponse, META_CLICK, META_CURRENT_TAB, META_DISPATCH_KEY,
    META_DRAIN_EVENTS, META_ENSURE_REAL_TAB, META_GOTO, META_HANDLE_DIALOG, META_IFRAME_TARGET,
    META_JS, META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO, META_PRESS_KEY, META_SCREENSHOT,
    META_SCROLL, META_SESSION, META_SWITCH_TAB, META_TYPE_TEXT, META_UPLOAD_FILE,
    META_WAIT_FOR_LOAD,
};
use bh_wasm_host::{
    console_event_matches, default_manifest, default_runner_config, event_matches_filter,
    operation_names, CdpRawRequest, ClickRequest, CurrentSessionRequest, CurrentSessionResult,
    CurrentTabRequest, DispatchKeyRequest, EnsureRealTabRequest, GotoRequest, GuestCallRecord,
    GuestRunResult, GuestServeRequest, GuestServeResponse, HandleDialogRequest, HttpGetRequest,
    IframeTargetRequest, JsRequest, ListTabsRequest, NewTabRequest, NewTabResult, PageInfoRequest,
    PressKeyRequest, RunnerConfig, ScreenshotRequest, ScrollRequest, SwitchTabRequest,
    SwitchTabResult, TabSummary, TypeTextRequest, UploadFileRequest, WaitForConsoleRequest,
    WaitForDialogRequest, WaitForEventRequest, WaitForEventResult, WaitForLoadEventRequest,
    WaitForLoadRequest, WaitForResponseRequest, WaitRequest, WaitResult, WatchEventsLine,
    WatchEventsRequest,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde_json::{json, Value};
use tokio::runtime::Builder;
use wasmtime::{Caller, Engine, Linker, Module, Store, TypedFunc};

const DEFAULT_DAEMON_READ_TIMEOUT: Duration = Duration::from_secs(30);
const DAEMON_TIMEOUT_SLACK: Duration = Duration::from_secs(5);

fn print_usage() {
    eprintln!(
        "usage: bhrun <manifest|sample-config|capabilities|summary|run-guest [path]|serve-guest [path]|current-tab|list-tabs|new-tab|switch-tab|ensure-real-tab|iframe-target|page-info|goto|wait-for-load|js|click|type-text|press-key|dispatch-key|scroll|screenshot|handle-dialog|upload-file|wait|http-get|current-session|drain-events|cdp-raw|wait-for-event|watch-events|wait-for-load-event|wait-for-response|wait-for-console|wait-for-dialog>\n\
         runner scaffold: persistent guest serving, event waiting, and preview guest execution are live"
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
                "bhrun scaffold: execution_model={:?} guest_transport={:?} protocol_families={} operations={} current_tab=live list_tabs=live new_tab=live switch_tab=live ensure_real_tab=live iframe_target=live page_info=live goto=live wait_for_load=live js=live click=live type_text=live press_key=live dispatch_key=live scroll=live screenshot=live handle_dialog=live upload_file=live wait=live http_get=live current_session=live cdp_raw=experimental wait_for_event=live watch_events=live wait_for_response=live wait_for_console=live wait_for_dialog=live wasm_guests=preview persistent_guest_runner=preview",
                manifest.execution_model,
                manifest.guest_transport,
                manifest.protocol_families.len(),
                manifest.operations.len()
            )
            .map_err(|err| format!("write stdout: {err}"))
        }
        Some("run-guest") => {
            let request = read_optional_json::<RunnerConfig, _>(&mut stdin)?
                .unwrap_or_else(default_runner_config);
            let guest_path = args
                .next()
                .or_else(|| request.guest_module.clone())
                .ok_or_else(|| {
                    "run-guest requires a guest path or config.guest_module".to_string()
                })?;
            let result = run_guest(&guest_path, request)?;
            write_json(&mut stdout, &result)
        }
        Some("serve-guest") => serve_guest(args.next(), stdin, &mut stdout),
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
        Some("ensure-real-tab") => {
            let request =
                read_optional_json::<EnsureRealTabRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = ensure_real_tab(request)?;
            write_json(&mut stdout, &result)
        }
        Some("iframe-target") => {
            let request = read_json::<IframeTargetRequest, _>(&mut stdin)?;
            let result = iframe_target(request)?;
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
        Some("wait-for-load") => {
            let request =
                read_optional_json::<WaitForLoadRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = wait_for_load(request)?;
            write_json(&mut stdout, &result)
        }
        Some("js") => {
            let request = read_json::<JsRequest, _>(&mut stdin)?;
            let result = js(request)?;
            write_json(&mut stdout, &result)
        }
        Some("click") => {
            let request = read_optional_json::<ClickRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = click(request)?;
            write_json(&mut stdout, &result)
        }
        Some("type-text") => {
            let request = read_optional_json::<TypeTextRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = type_text(request)?;
            write_json(&mut stdout, &result)
        }
        Some("press-key") => {
            let request = read_optional_json::<PressKeyRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = press_key(request)?;
            write_json(&mut stdout, &result)
        }
        Some("dispatch-key") => {
            let request = read_json::<DispatchKeyRequest, _>(&mut stdin)?;
            let result = dispatch_key(request)?;
            write_json(&mut stdout, &result)
        }
        Some("scroll") => {
            let request = read_optional_json::<ScrollRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = scroll(request)?;
            write_json(&mut stdout, &result)
        }
        Some("screenshot") => {
            let request =
                read_optional_json::<ScreenshotRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = screenshot(request)?;
            write_json(&mut stdout, &result)
        }
        Some("handle-dialog") => {
            let request =
                read_optional_json::<HandleDialogRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = handle_dialog(request)?;
            write_json(&mut stdout, &result)
        }
        Some("upload-file") => {
            let request = read_json::<UploadFileRequest, _>(&mut stdin)?;
            let result = upload_file(request)?;
            write_json(&mut stdout, &result)
        }
        Some("wait") => {
            let request = read_optional_json::<WaitRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = wait(request);
            write_json(&mut stdout, &result)
        }
        Some("http-get") => {
            let request = read_json::<HttpGetRequest, _>(&mut stdin)?;
            let result = http_get(request)?;
            write_json(&mut stdout, &result)
        }
        Some("current-session") => {
            let request =
                read_optional_json::<CurrentSessionRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = current_session(request)?;
            write_json(&mut stdout, &result)
        }
        Some("drain-events") => {
            let request =
                read_optional_json::<CurrentSessionRequest, _>(&mut stdin)?.unwrap_or_default();
            let result = drain_events_command(request)?;
            write_json(&mut stdout, &result)
        }
        Some("cdp-raw") => {
            let request = read_json::<CdpRawRequest, _>(&mut stdin)?;
            let result = cdp_raw(request)?;
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

fn drain_events_command(request: CurrentSessionRequest) -> Result<Vec<Value>, String> {
    drain_events(&request.daemon_name)
}

fn cdp_raw(request: CdpRawRequest) -> Result<Value, String> {
    cdp_raw_with_sender(request, send_daemon_request)
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

fn ensure_real_tab(request: EnsureRealTabRequest) -> Result<Option<TabSummary>, String> {
    ensure_real_tab_with_sender(request, send_daemon_request)
}

fn iframe_target(request: IframeTargetRequest) -> Result<Option<String>, String> {
    iframe_target_with_sender(request, send_daemon_request)
}

fn page_info(request: PageInfoRequest) -> Result<Value, String> {
    page_info_with_sender(request, send_daemon_request)
}

fn goto(request: GotoRequest) -> Result<Value, String> {
    goto_with_sender(request, send_daemon_request)
}

fn wait_for_load(request: WaitForLoadRequest) -> Result<bool, String> {
    wait_for_load_with_sender(request, send_daemon_request)
}

fn js(request: JsRequest) -> Result<Value, String> {
    js_with_sender(request, send_daemon_request)
}

fn click(request: ClickRequest) -> Result<(), String> {
    click_with_sender(request, send_daemon_request)
}

fn type_text(request: TypeTextRequest) -> Result<(), String> {
    type_text_with_sender(request, send_daemon_request)
}

fn press_key(request: PressKeyRequest) -> Result<(), String> {
    press_key_with_sender(request, send_daemon_request)
}

fn dispatch_key(request: DispatchKeyRequest) -> Result<(), String> {
    dispatch_key_with_sender(request, send_daemon_request)
}

fn scroll(request: ScrollRequest) -> Result<(), String> {
    scroll_with_sender(request, send_daemon_request)
}

fn screenshot(request: ScreenshotRequest) -> Result<String, String> {
    screenshot_with_sender(request, send_daemon_request)
}

fn handle_dialog(request: HandleDialogRequest) -> Result<(), String> {
    handle_dialog_with_sender(request, send_daemon_request)
}

fn upload_file(request: UploadFileRequest) -> Result<(), String> {
    upload_file_with_sender(request, send_daemon_request)
}

fn wait(request: WaitRequest) -> WaitResult {
    let request = request.normalized();
    let start = Instant::now();
    thread::sleep(Duration::from_millis(request.duration_ms));
    WaitResult {
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

fn http_get(request: HttpGetRequest) -> Result<String, String> {
    let request = request.normalized();
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("build tokio runtime for http_get: {err}"))?;
    runtime.block_on(async move {
        let timeout = Duration::from_secs_f64(request.timeout);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| format!("build http client: {err}"))?;
        let headers = merged_http_headers(request.headers.as_ref())?;
        let response = client
            .get(&request.url)
            .headers(headers)
            .send()
            .await
            .map_err(|err| format!("http GET {}: {err}", request.url))?;
        let response = response
            .error_for_status()
            .map_err(|err| format!("http GET {}: {err}", request.url))?;
        response
            .text()
            .await
            .map_err(|err| format!("decode HTTP response {}: {err}", request.url))
    })
}

fn merged_http_headers(
    extra_headers: Option<&std::collections::BTreeMap<String, String>>,
) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0"));
    if let Some(extra_headers) = extra_headers {
        for (name, value) in extra_headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| format!("invalid HTTP header name {name:?}: {err}"))?;
            let value = HeaderValue::from_str(value)
                .map_err(|err| format!("invalid HTTP header value for {name}: {err}"))?;
            headers.insert(name, value);
        }
    }
    Ok(headers)
}

#[derive(Debug)]
struct GuestHostState {
    config: RunnerConfig,
    calls: Vec<GuestCallRecord>,
    error: Option<String>,
}

struct GuestRuntime {
    guest_module: String,
    store: Store<GuestHostState>,
    run: TypedFunc<(), i32>,
    invocation_count: u64,
}

impl GuestRuntime {
    fn new(path: &str, config: RunnerConfig) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, Path::new(path))
            .map_err(|err| format!("load guest module: {err}"))?;
        let mut linker = Linker::new(&engine);
        linker
            .func_wrap(
                "bh",
                "call_json",
                |mut caller: Caller<'_, GuestHostState>,
                 operation_ptr: i32,
                 operation_len: i32,
                 request_ptr: i32,
                 request_len: i32,
                 out_ptr: i32,
                 out_cap: i32|
                 -> i32 {
                    let operation = match read_guest_utf8(&mut caller, operation_ptr, operation_len)
                    {
                        Ok(operation) => operation,
                        Err(err) => return set_guest_error(caller.data_mut(), err),
                    };
                    let request_text = match read_guest_utf8(&mut caller, request_ptr, request_len)
                    {
                        Ok(request_text) => request_text,
                        Err(err) => return set_guest_error(caller.data_mut(), err),
                    };
                    let response = match dispatch_guest_operation(
                        caller.data_mut(),
                        &operation,
                        &request_text,
                    ) {
                        Ok(response) => response,
                        Err(err) => return set_guest_error(caller.data_mut(), err),
                    };
                    match write_guest_bytes(&mut caller, out_ptr, out_cap, &response) {
                        Ok(written) => written,
                        Err(err) => set_guest_error(caller.data_mut(), err),
                    }
                },
            )
            .map_err(|err| format!("define guest host function: {err}"))?;

        let mut store = Store::new(
            &engine,
            GuestHostState {
                config,
                calls: Vec::new(),
                error: None,
            },
        );
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| format!("instantiate guest module: {err}"))?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .map_err(|err| format!("locate guest export run: {err}"))?;

        Ok(Self {
            guest_module: path.to_string(),
            store,
            run,
            invocation_count: 0,
        })
    }

    fn invoke_run(&mut self) -> GuestRunResult {
        let call_start = self.store.data().calls.len();
        self.store.data_mut().error = None;

        let outcome = self.run.call(&mut self.store, ());
        self.invocation_count += 1;

        let state = self.store.data();
        let calls = state.calls[call_start..].to_vec();
        let host_error = state.error.clone();

        match outcome {
            Ok(exit_code) => GuestRunResult {
                exit_code,
                success: exit_code == 0 && host_error.is_none(),
                calls,
                trap: host_error,
            },
            Err(err) => GuestRunResult {
                exit_code: -1,
                success: false,
                calls,
                trap: Some(host_error.unwrap_or_else(|| err.to_string())),
            },
        }
    }

    fn ready_response(&self) -> GuestServeResponse {
        let state = self.store.data();
        GuestServeResponse::Ready {
            guest_module: self.guest_module.clone(),
            persistent_guest_state: state.config.persistent_guest_state,
            granted_operations: state.config.granted_operations.clone(),
            invocation_count: self.invocation_count,
        }
    }

    fn status_response(&self) -> GuestServeResponse {
        let state = self.store.data();
        GuestServeResponse::Status {
            guest_module: self.guest_module.clone(),
            persistent_guest_state: state.config.persistent_guest_state,
            granted_operations: state.config.granted_operations.clone(),
            invocation_count: self.invocation_count,
        }
    }
}

fn run_guest(path: &str, config: RunnerConfig) -> Result<GuestRunResult, String> {
    Ok(GuestRuntime::new(path, config)?.invoke_run())
}

fn serve_guest<R, W>(path_arg: Option<String>, stdin: R, stdout: &mut W) -> Result<(), String>
where
    R: Read,
    W: Write,
{
    let mut reader = io::BufReader::new(stdin);
    let mut runtime: Option<GuestRuntime> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|err| format!("read serve-guest stdin: {err}"))?;
        if read == 0 {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }

        let request: GuestServeRequest = serde_json::from_str(line.trim())
            .map_err(|err| format!("invalid serve-guest request JSON: {err}"))?;
        match request {
            GuestServeRequest::Start {
                guest_module,
                config,
            } => {
                if runtime.is_some() {
                    return Err("serve-guest already started a guest runtime".to_string());
                }
                let config = config.unwrap_or_else(default_runner_config);
                if !config.persistent_guest_state {
                    return Err(
                        "serve-guest requires config.persistent_guest_state=true".to_string()
                    );
                }
                let guest_path = path_arg
                    .clone()
                    .or(guest_module)
                    .or_else(|| config.guest_module.clone())
                    .ok_or_else(|| {
                        "serve-guest start requires a guest path or config.guest_module".to_string()
                    })?;
                let guest_runtime = GuestRuntime::new(&guest_path, config)?;
                write_json_line(stdout, &guest_runtime.ready_response())?;
                runtime = Some(guest_runtime);
            }
            GuestServeRequest::Run => {
                let guest_runtime = runtime
                    .as_mut()
                    .ok_or_else(|| "serve-guest requires a start command first".to_string())?;
                let result = guest_runtime.invoke_run();
                write_json_line(
                    stdout,
                    &GuestServeResponse::RunResult {
                        invocation_count: guest_runtime.invocation_count,
                        result,
                    },
                )?;
            }
            GuestServeRequest::Status => {
                let response = if let Some(guest_runtime) = runtime.as_ref() {
                    guest_runtime.status_response()
                } else {
                    GuestServeResponse::Stopped {
                        invocation_count: 0,
                    }
                };
                write_json_line(stdout, &response)?;
            }
            GuestServeRequest::Stop => {
                let invocation_count = runtime
                    .as_ref()
                    .map(|guest_runtime| guest_runtime.invocation_count)
                    .unwrap_or(0);
                write_json_line(stdout, &GuestServeResponse::Stopped { invocation_count })?;
                return Ok(());
            }
        }
    }
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

fn cdp_raw_with_sender<F>(request: CdpRawRequest, mut sender: F) -> Result<Value, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let response = sender(
        &request.daemon_name,
        &DaemonRequest {
            method: Some(request.method),
            params: request.params,
            session_id: request.session_id,
            ..DaemonRequest::default()
        },
    )?;
    Ok(response
        .result
        .unwrap_or_else(|| Value::Object(serde_json::Map::new())))
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

fn ensure_real_tab_with_sender<F>(
    request: EnsureRealTabRequest,
    mut sender: F,
) -> Result<Option<TabSummary>, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_ENSURE_REAL_TAB,
        None,
        &mut sender,
    )
}

fn iframe_target_with_sender<F>(
    request: IframeTargetRequest,
    mut sender: F,
) -> Result<Option<String>, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_IFRAME_TARGET,
        Some(json!({"url_substr": request.url_substr})),
        &mut sender,
    )
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

fn wait_for_load_with_sender<F>(request: WaitForLoadRequest, mut sender: F) -> Result<bool, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_WAIT_FOR_LOAD,
        Some(json!({"timeout": request.timeout})),
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

fn click_with_sender<F>(request: ClickRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_CLICK,
        Some(json!({
            "x": request.x,
            "y": request.y,
            "button": request.button,
            "clicks": request.clicks,
        })),
        &mut sender,
    )
}

fn type_text_with_sender<F>(request: TypeTextRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_TYPE_TEXT,
        Some(json!({"text": request.text})),
        &mut sender,
    )
}

fn press_key_with_sender<F>(request: PressKeyRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_PRESS_KEY,
        Some(json!({"key": request.key, "modifiers": request.modifiers})),
        &mut sender,
    )
}

fn dispatch_key_with_sender<F>(request: DispatchKeyRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_DISPATCH_KEY,
        Some(json!({
            "selector": request.selector,
            "key": request.key,
            "event": request.event,
        })),
        &mut sender,
    )
}

fn scroll_with_sender<F>(request: ScrollRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_SCROLL,
        Some(json!({"x": request.x, "y": request.y, "dx": request.dx, "dy": request.dy})),
        &mut sender,
    )
}

fn screenshot_with_sender<F>(request: ScreenshotRequest, mut sender: F) -> Result<String, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_SCREENSHOT,
        Some(json!({"full": request.full})),
        &mut sender,
    )
}

fn handle_dialog_with_sender<F>(request: HandleDialogRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let mut params =
        serde_json::Map::from_iter([("action".to_string(), Value::String(request.action))]);
    if let Some(prompt_text) = request.prompt_text {
        params.insert("prompt_text".to_string(), Value::String(prompt_text));
    }
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_HANDLE_DIALOG,
        Some(Value::Object(params)),
        &mut sender,
    )
}

fn upload_file_with_sender<F>(request: UploadFileRequest, mut sender: F) -> Result<(), String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    let request = request.normalized();
    let mut params = serde_json::Map::from_iter([
        ("selector".to_string(), Value::String(request.selector)),
        (
            "files".to_string(),
            Value::Array(request.files.into_iter().map(Value::String).collect()),
        ),
    ]);
    if let Some(target_id) = request.target_id {
        params.insert("target_id".to_string(), Value::String(target_id));
    }
    typed_meta_result_with_sender(
        &request.daemon_name,
        META_UPLOAD_FILE,
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

fn watch_events_collect(request: WatchEventsRequest) -> Result<Vec<WatchEventsLine>, String> {
    watch_events_collect_with_drain(request, drain_events)
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
    drain: F,
) -> Result<(), String>
where
    W: Write,
    F: FnMut(&str) -> Result<Vec<Value>, String>,
{
    run_watch_events_with_drain(request, drain, |line| write_json_line(stdout, &line))
}

fn watch_events_collect_with_drain<F>(
    request: WatchEventsRequest,
    drain: F,
) -> Result<Vec<WatchEventsLine>, String>
where
    F: FnMut(&str) -> Result<Vec<Value>, String>,
{
    let mut lines = Vec::new();
    run_watch_events_with_drain(request, drain, |line| {
        lines.push(line);
        Ok(())
    })?;
    Ok(lines)
}

fn run_watch_events_with_drain<F, E>(
    request: WatchEventsRequest,
    mut drain: F,
    mut emit: E,
) -> Result<(), String>
where
    F: FnMut(&str) -> Result<Vec<Value>, String>,
    E: FnMut(WatchEventsLine) -> Result<(), String>,
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
                emit(WatchEventsLine::Event {
                    event,
                    index: matched_events,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                })?;
                if request.max_events == Some(matched_events) {
                    emit(WatchEventsLine::End {
                        matched_events,
                        polls,
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        timed_out: false,
                        reached_max_events: true,
                    })?;
                    return Ok(());
                }
            }
        }

        if start.elapsed() >= timeout {
            emit(WatchEventsLine::End {
                matched_events,
                polls,
                elapsed_ms: start.elapsed().as_millis() as u64,
                timed_out: true,
                reached_max_events: false,
            })?;
            return Ok(());
        }

        thread::sleep(poll_interval.min(timeout.saturating_sub(start.elapsed())));
    }
}

fn drain_events(daemon_name: &str) -> Result<Vec<Value>, String> {
    drain_events_with_sender(daemon_name, send_daemon_request)
}

fn drain_events_with_sender<F>(daemon_name: &str, mut sender: F) -> Result<Vec<Value>, String>
where
    F: FnMut(&str, &DaemonRequest) -> Result<DaemonResponse, String>,
{
    Ok(sender(
        daemon_name,
        &DaemonRequest {
            meta: Some(META_DRAIN_EVENTS.to_string()),
            ..DaemonRequest::default()
        },
    )?
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

fn read_guest_utf8(
    caller: &mut Caller<'_, GuestHostState>,
    ptr: i32,
    len: i32,
) -> Result<String, String> {
    let memory = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| "guest did not export memory".to_string())?;
    if ptr < 0 || len < 0 {
        return Err("guest memory read used negative ptr/len".to_string());
    }
    let mut buf = vec![0u8; len as usize];
    memory
        .read(caller, ptr as usize, &mut buf)
        .map_err(|err| format!("read guest memory: {err}"))?;
    String::from_utf8(buf).map_err(|err| format!("guest string was not utf-8: {err}"))
}

fn write_guest_bytes(
    caller: &mut Caller<'_, GuestHostState>,
    ptr: i32,
    cap: i32,
    bytes: &[u8],
) -> Result<i32, String> {
    let memory = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| "guest did not export memory".to_string())?;
    if ptr < 0 || cap < 0 {
        return Err("guest memory write used negative ptr/cap".to_string());
    }
    if bytes.len() > cap as usize {
        return Err(format!(
            "guest output buffer too small: need {}, have {}",
            bytes.len(),
            cap
        ));
    }
    memory
        .write(caller, ptr as usize, bytes)
        .map_err(|err| format!("write guest memory: {err}"))?;
    Ok(bytes.len() as i32)
}

fn set_guest_error(state: &mut GuestHostState, err: String) -> i32 {
    if state.error.is_none() {
        state.error = Some(err);
    }
    -1
}

fn dispatch_guest_operation(
    state: &mut GuestHostState,
    operation: &str,
    request_text: &str,
) -> Result<Vec<u8>, String> {
    if !state
        .config
        .granted_operations
        .iter()
        .any(|granted| granted == operation)
    {
        return Err(format!("operation denied by runner config: {operation}"));
    }
    if operation == "http_get" && !state.config.allow_http {
        return Err("operation denied by runner config: http_get disabled".to_string());
    }
    if operation == "cdp_raw" && !state.config.allow_raw_cdp {
        return Err("operation denied by runner config: cdp_raw disabled".to_string());
    }

    let request = inject_daemon_name(request_text, &state.config.daemon_name)?;
    let response = match operation {
        "current_session" => serialize_guest_result(
            current_session(parse_request_value(&request)?),
            "current_session",
        )?,
        "cdp_raw" => cdp_raw(parse_request_value(&request)?)?,
        "current_tab" => {
            serialize_guest_result(current_tab(parse_request_value(&request)?), "current_tab")?
        }
        "list_tabs" => {
            serialize_guest_result(list_tabs(parse_request_value(&request)?), "list_tabs")?
        }
        "new_tab" => serialize_guest_result(new_tab(parse_request_value(&request)?), "new_tab")?,
        "switch_tab" => {
            serialize_guest_result(switch_tab(parse_request_value(&request)?), "switch_tab")?
        }
        "ensure_real_tab" => serialize_guest_result(
            ensure_real_tab(parse_request_value(&request)?),
            "ensure_real_tab",
        )?,
        "iframe_target" => serialize_guest_result(
            iframe_target(parse_request_value(&request)?),
            "iframe_target",
        )?,
        "page_info" => page_info(parse_request_value(&request)?)?,
        "goto" => goto(parse_request_value(&request)?)?,
        "wait_for_load" => serialize_guest_result(
            wait_for_load(parse_request_value(&request)?),
            "wait_for_load",
        )?,
        "js" => js(parse_request_value(&request)?)?,
        "click" => serialize_guest_result(click(parse_request_value(&request)?), "click")?,
        "type_text" => {
            serialize_guest_result(type_text(parse_request_value(&request)?), "type_text")?
        }
        "press_key" => {
            serialize_guest_result(press_key(parse_request_value(&request)?), "press_key")?
        }
        "dispatch_key" => {
            serialize_guest_result(dispatch_key(parse_request_value(&request)?), "dispatch_key")?
        }
        "scroll" => serialize_guest_result(scroll(parse_request_value(&request)?), "scroll")?,
        "screenshot" => {
            serialize_guest_result(screenshot(parse_request_value(&request)?), "screenshot")?
        }
        "handle_dialog" => serialize_guest_result(
            handle_dialog(parse_request_value(&request)?),
            "handle_dialog",
        )?,
        "upload_file" => {
            serialize_guest_result(upload_file(parse_request_value(&request)?), "upload_file")?
        }
        "wait" => serialize_guest_result(Ok(wait(parse_request_value(&request)?)), "wait")?,
        "http_get" => serialize_guest_result(http_get(parse_request_value(&request)?), "http_get")?,
        "wait_for_event" => serialize_guest_result(
            wait_for_event(parse_request_value(&request)?),
            "wait_for_event",
        )?,
        "watch_events" => serialize_guest_result(
            watch_events_collect(parse_request_value(&request)?),
            "watch_events",
        )?,
        "wait_for_load_event" => serialize_guest_result(
            wait_for_load_event(parse_request_value(&request)?),
            "wait_for_load_event",
        )?,
        "wait_for_response" => serialize_guest_result(
            wait_for_response(parse_request_value(&request)?),
            "wait_for_response",
        )?,
        "wait_for_console" => serialize_guest_result(
            wait_for_console(parse_request_value(&request)?),
            "wait_for_console",
        )?,
        "wait_for_dialog" => serialize_guest_result(
            wait_for_dialog(parse_request_value(&request)?),
            "wait_for_dialog",
        )?,
        unsupported => return Err(format!("unsupported guest operation: {unsupported}")),
    };
    state.calls.push(GuestCallRecord {
        operation: operation.to_string(),
        request: serde_json::from_str(&request)
            .map_err(|err| format!("parse normalized request: {err}"))?,
        response: response.clone(),
    });
    serde_json::to_vec(&response).map_err(|err| format!("serialize guest response JSON: {err}"))
}

fn serialize_guest_result<T>(result: Result<T, String>, context: &str) -> Result<Value, String>
where
    T: serde::Serialize,
{
    serde_json::to_value(result?).map_err(|err| format!("serialize {context} result: {err}"))
}

fn inject_daemon_name(request_text: &str, daemon_name: &str) -> Result<String, String> {
    let trimmed = request_text.trim();
    let mut request = if trimmed.is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid guest request JSON: {err}"))?
    };
    let object = request
        .as_object_mut()
        .ok_or_else(|| "guest request JSON must be an object".to_string())?;
    object
        .entry("daemon_name".to_string())
        .or_insert_with(|| Value::String(daemon_name.to_string()));
    serde_json::to_string(&request).map_err(|err| format!("serialize guest request JSON: {err}"))
}

fn parse_request_value<T>(request_text: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(request_text).map_err(|err| format!("parse guest request: {err}"))
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
        .set_read_timeout(Some(daemon_read_timeout(request)))
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

fn daemon_read_timeout(request: &DaemonRequest) -> Duration {
    match request.meta.as_deref() {
        Some(META_WAIT_FOR_LOAD) => request
            .params
            .as_ref()
            .and_then(|params| params.get("timeout"))
            .and_then(Value::as_f64)
            .filter(|timeout| timeout.is_finite() && *timeout >= 0.0)
            .map(|timeout| Duration::from_secs_f64(timeout) + DAEMON_TIMEOUT_SLACK)
            .unwrap_or(DEFAULT_DAEMON_READ_TIMEOUT),
        _ => DEFAULT_DAEMON_READ_TIMEOUT,
    }
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
        cdp_raw_with_sender, click_with_sender, current_session_with_sender,
        current_tab_with_sender, daemon_read_timeout, dispatch_guest_operation,
        dispatch_key_with_sender, drain_events_with_sender, ensure_real_tab_with_sender,
        goto_with_sender, handle_dialog_with_sender, http_get, iframe_target_with_sender,
        inject_daemon_name, js_with_sender, list_tabs_with_sender, new_tab_with_sender,
        page_info_with_sender, press_key_with_sender, run_cli, screenshot_with_sender,
        scroll_with_sender, serialize_guest_result, switch_tab_with_sender, type_text_with_sender,
        upload_file_with_sender, wait, wait_for_console_with_drain, wait_for_event_with_drain,
        wait_for_load_with_sender, watch_events_collect_with_drain, watch_events_with_drain,
        DaemonResponse, GuestHostState, GuestRuntime, META_CLICK, META_CURRENT_TAB,
        META_DISPATCH_KEY, META_DRAIN_EVENTS, META_ENSURE_REAL_TAB, META_GOTO, META_HANDLE_DIALOG,
        META_IFRAME_TARGET, META_JS, META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO, META_PRESS_KEY,
        META_SCREENSHOT, META_SCROLL, META_SESSION, META_SWITCH_TAB, META_TYPE_TEXT,
        META_UPLOAD_FILE, META_WAIT_FOR_LOAD,
    };
    use std::collections::BTreeMap;
    use std::collections::VecDeque;
    use std::io::{self, Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use bh_protocol::DaemonRequest;
    use bh_wasm_host::{
        default_runner_config, CdpRawRequest, ClickRequest, CurrentSessionRequest,
        CurrentSessionResult, CurrentTabRequest, DispatchKeyRequest, EnsureRealTabRequest,
        EventFilter, GotoRequest, GuestServeResponse, HandleDialogRequest, HttpGetRequest,
        IframeTargetRequest, JsRequest, ListTabsRequest, NewTabRequest, PageInfoRequest,
        PressKeyRequest, RunnerConfig, ScreenshotRequest, ScrollRequest, SwitchTabRequest,
        TypeTextRequest, UploadFileRequest, WaitForConsoleRequest, WaitForDialogRequest,
        WaitForEventRequest, WaitForEventResult, WaitForLoadEventRequest, WaitForLoadRequest,
        WaitForResponseRequest, WaitRequest, WatchEventsLine, WatchEventsRequest,
    };
    use serde_json::{json, Value};

    fn persistent_counter_guest_path() -> String {
        format!(
            "{}/../../guests/persistent_counter.wat",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn spawn_http_fixture_server(
        response_body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind HTTP fixture");
        let address = listener.local_addr().expect("local addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            String::from_utf8(request).expect("request utf-8")
        });

        (format!("http://{address}"), handle)
    }

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
    fn inject_daemon_name_adds_runner_daemon_when_missing() {
        let request = inject_daemon_name(r#"{"expression":"location.href"}"#, "runner")
            .expect("inject daemon name");
        let value: Value = serde_json::from_str(&request).expect("parse injected request");

        assert_eq!(
            value.get("daemon_name").and_then(Value::as_str),
            Some("runner")
        );
        assert_eq!(
            value.get("expression").and_then(Value::as_str),
            Some("location.href")
        );
    }

    #[test]
    fn dispatch_guest_operation_rejects_ungranted_operation() {
        let mut state = GuestHostState {
            config: RunnerConfig {
                daemon_name: "runner".to_string(),
                guest_module: None,
                granted_operations: vec!["page_info".to_string()],
                allow_http: false,
                allow_raw_cdp: false,
                persistent_guest_state: true,
            },
            calls: Vec::new(),
            error: None,
        };

        let err = dispatch_guest_operation(&mut state, "goto", r#"{"url":"https://example.com"}"#)
            .expect_err("ungranted operation should fail");
        assert_eq!(err, "operation denied by runner config: goto");
        assert!(state.calls.is_empty());
    }

    #[test]
    fn serialize_guest_result_uses_inner_success_value() {
        let value = serialize_guest_result(
            Ok(json!({"targetId":"target-1","url":"about:blank"})),
            "current_tab",
        )
        .expect("serialize success");

        assert_eq!(
            value.get("targetId").and_then(Value::as_str),
            Some("target-1")
        );
        assert!(value.get("Ok").is_none());
    }

    #[test]
    fn serialize_guest_result_propagates_operation_error() {
        let err = serialize_guest_result::<Value>(Err("boom".to_string()), "current_tab")
            .expect_err("serialization should propagate inner error");

        assert_eq!(err, "boom");
    }

    #[test]
    fn wait_returns_elapsed_time() {
        let result = wait(WaitRequest { duration_ms: 1 });
        assert!(result.elapsed_ms >= 1);
    }

    #[test]
    fn http_get_merges_default_and_custom_headers() {
        let (base_url, handle) = spawn_http_fixture_server("fixture-body");
        let mut headers = BTreeMap::new();
        headers.insert("X-Test".to_string(), "value".to_string());

        let body = http_get(HttpGetRequest {
            url: format!("{base_url}/headers"),
            headers: Some(headers),
            timeout: 5.0,
        })
        .expect("http get body");
        let request_text = handle.join().expect("server request");

        assert_eq!(body, "fixture-body");
        assert!(request_text.starts_with("GET /headers HTTP/1.1\r\n"));
        assert!(request_text.contains("user-agent: Mozilla/5.0\r\n"));
        assert!(request_text.contains("x-test: value\r\n"));
    }

    #[test]
    fn guest_runtime_preserves_state_across_runs() {
        let guest_path = persistent_counter_guest_path();
        let config = RunnerConfig {
            granted_operations: vec!["wait".to_string()],
            ..default_runner_config()
        };
        let mut runtime = GuestRuntime::new(&guest_path, config).expect("create guest runtime");

        let first = runtime.invoke_run();
        let second = runtime.invoke_run();

        assert!(first.success);
        assert!(second.success);
        assert_eq!(runtime.invocation_count, 2);
        assert_eq!(first.calls.len(), 1);
        assert_eq!(second.calls.len(), 1);
        assert_eq!(
            first.calls[0]
                .request
                .get("duration_ms")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            second.calls[0]
                .request
                .get("duration_ms")
                .and_then(Value::as_u64),
            Some(2)
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
    fn ensure_real_tab_uses_meta_request_result() {
        let result = ensure_real_tab_with_sender(
            EnsureRealTabRequest {
                daemon_name: "runner".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_ENSURE_REAL_TAB));
                assert_eq!(request.params, None);
                Ok(DaemonResponse {
                    result: Some(json!({
                        "targetId":"target-3",
                        "title":"Example",
                        "url":"https://example.com"
                    })),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("ensure real tab result");

        assert_eq!(
            result.as_ref().map(|tab| tab.target_id.as_str()),
            Some("target-3")
        );
    }

    #[test]
    fn iframe_target_uses_meta_request_with_url_substring() {
        let result = iframe_target_with_sender(
            IframeTargetRequest {
                daemon_name: "runner".to_string(),
                url_substr: "github.com".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_IFRAME_TARGET));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("url_substr"))
                        .and_then(Value::as_str),
                    Some("github.com")
                );
                Ok(DaemonResponse {
                    result: Some(json!("iframe-3")),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("iframe target result");

        assert_eq!(result.as_deref(), Some("iframe-3"));
    }

    #[test]
    fn wait_for_load_uses_meta_request_timeout() {
        let result = wait_for_load_with_sender(
            WaitForLoadRequest {
                daemon_name: "runner".to_string(),
                timeout: 2.5,
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_WAIT_FOR_LOAD));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("timeout"))
                        .and_then(Value::as_f64),
                    Some(2.5)
                );
                Ok(DaemonResponse {
                    result: Some(json!(true)),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("wait for load result");

        assert!(result);
    }

    #[test]
    fn daemon_read_timeout_extends_wait_for_load_timeout() {
        let timeout = daemon_read_timeout(&DaemonRequest {
            meta: Some(META_WAIT_FOR_LOAD.to_string()),
            params: Some(json!({"timeout": 15.0})),
            ..DaemonRequest::default()
        });

        assert_eq!(timeout, Duration::from_secs(20));
    }

    #[test]
    fn daemon_read_timeout_defaults_for_other_requests() {
        let timeout = daemon_read_timeout(&DaemonRequest {
            meta: Some(META_GOTO.to_string()),
            ..DaemonRequest::default()
        });

        assert_eq!(timeout, Duration::from_secs(30));
    }

    #[test]
    fn click_uses_meta_request_payload() {
        click_with_sender(
            ClickRequest {
                daemon_name: "runner".to_string(),
                x: 12.0,
                y: 34.0,
                button: "middle".to_string(),
                clicks: 2,
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_CLICK));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("x"))
                        .and_then(Value::as_f64),
                    Some(12.0)
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("button"))
                        .and_then(Value::as_str),
                    Some("middle")
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("click result");
    }

    #[test]
    fn type_text_uses_meta_request_payload() {
        type_text_with_sender(
            TypeTextRequest {
                daemon_name: "runner".to_string(),
                text: "token".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_TYPE_TEXT));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("text"))
                        .and_then(Value::as_str),
                    Some("token")
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("type text result");
    }

    #[test]
    fn press_key_uses_meta_request_payload() {
        press_key_with_sender(
            PressKeyRequest {
                daemon_name: "runner".to_string(),
                key: "Enter".to_string(),
                modifiers: 2,
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_PRESS_KEY));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("key"))
                        .and_then(Value::as_str),
                    Some("Enter")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("modifiers"))
                        .and_then(Value::as_i64),
                    Some(2)
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("press key result");
    }

    #[test]
    fn dispatch_key_uses_meta_request_payload() {
        dispatch_key_with_sender(
            DispatchKeyRequest {
                daemon_name: "runner".to_string(),
                selector: "#search".to_string(),
                key: "Tab".to_string(),
                event: "keydown".to_string(),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_DISPATCH_KEY));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("selector"))
                        .and_then(Value::as_str),
                    Some("#search")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("key"))
                        .and_then(Value::as_str),
                    Some("Tab")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("event"))
                        .and_then(Value::as_str),
                    Some("keydown")
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("dispatch key result");
    }

    #[test]
    fn scroll_uses_meta_request_payload() {
        scroll_with_sender(
            ScrollRequest {
                daemon_name: "runner".to_string(),
                x: 1.0,
                y: 2.0,
                dx: 3.0,
                dy: 4.0,
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_SCROLL));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("dy"))
                        .and_then(Value::as_f64),
                    Some(4.0)
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("scroll result");
    }

    #[test]
    fn screenshot_uses_meta_request_payload() {
        let result = screenshot_with_sender(
            ScreenshotRequest {
                daemon_name: "runner".to_string(),
                full: true,
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_SCREENSHOT));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("full"))
                        .and_then(Value::as_bool),
                    Some(true)
                );
                Ok(DaemonResponse {
                    result: Some(json!("cG5nLWJ5dGVz")),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("screenshot result");

        assert_eq!(result, "cG5nLWJ5dGVz");
    }

    #[test]
    fn handle_dialog_uses_meta_request_payload() {
        handle_dialog_with_sender(
            HandleDialogRequest {
                daemon_name: "runner".to_string(),
                action: "dismiss".to_string(),
                prompt_text: Some("typed value".to_string()),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_HANDLE_DIALOG));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("action"))
                        .and_then(Value::as_str),
                    Some("dismiss")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("prompt_text"))
                        .and_then(Value::as_str),
                    Some("typed value")
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("handle dialog result");
    }

    #[test]
    fn cdp_raw_uses_direct_daemon_request_payload() {
        let result = cdp_raw_with_sender(
            CdpRawRequest {
                daemon_name: "runner".to_string(),
                method: "Runtime.evaluate".to_string(),
                params: Some(json!({"expression":"2+3","returnByValue":true})),
                session_id: Some("session-2".to_string()),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta, None);
                assert_eq!(request.method.as_deref(), Some("Runtime.evaluate"));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("expression"))
                        .and_then(Value::as_str),
                    Some("2+3")
                );
                assert_eq!(request.session_id.as_deref(), Some("session-2"));
                Ok(DaemonResponse {
                    result: Some(json!({"result":{"type":"number","value":5}})),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("cdp raw result");

        assert_eq!(
            result.pointer("/result/value").and_then(Value::as_i64),
            Some(5)
        );
    }

    #[test]
    fn upload_file_uses_meta_request_payload() {
        upload_file_with_sender(
            UploadFileRequest {
                daemon_name: "runner".to_string(),
                selector: "#file".to_string(),
                files: vec!["/tmp/one.txt".to_string(), "/tmp/two.txt".to_string()],
                target_id: Some("iframe-1".to_string()),
            },
            |daemon, request| {
                assert_eq!(daemon, "runner");
                assert_eq!(request.meta.as_deref(), Some(META_UPLOAD_FILE));
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("selector"))
                        .and_then(Value::as_str),
                    Some("#file")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.pointer("/files/0"))
                        .and_then(Value::as_str),
                    Some("/tmp/one.txt")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.pointer("/files/1"))
                        .and_then(Value::as_str),
                    Some("/tmp/two.txt")
                );
                assert_eq!(
                    request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("target_id"))
                        .and_then(Value::as_str),
                    Some("iframe-1")
                );
                Ok(DaemonResponse {
                    result: Some(Value::Null),
                    ..DaemonResponse::default()
                })
            },
        )
        .expect("upload file result");
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
        assert!(text.contains("ensure_real_tab=live"));
        assert!(text.contains("iframe_target=live"));
        assert!(text.contains("page_info=live"));
        assert!(text.contains("goto=live"));
        assert!(text.contains("wait_for_load=live"));
        assert!(text.contains("js=live"));
        assert!(text.contains("click=live"));
        assert!(text.contains("type_text=live"));
        assert!(text.contains("press_key=live"));
        assert!(text.contains("dispatch_key=live"));
        assert!(text.contains("scroll=live"));
        assert!(text.contains("screenshot=live"));
        assert!(text.contains("handle_dialog=live"));
        assert!(text.contains("upload_file=live"));
        assert!(text.contains("wait=live"));
        assert!(text.contains("http_get=live"));
        assert!(text.contains("current_session=live"));
        assert!(text.contains("cdp_raw=experimental"));
        assert!(text.contains("wait_for_event=live"));
        assert!(text.contains("watch_events=live"));
        assert!(text.contains("wait_for_response=live"));
        assert!(text.contains("wait_for_console=live"));
        assert!(text.contains("wait_for_dialog=live"));
        assert!(text.contains("persistent_guest_runner=preview"));
    }

    #[test]
    fn cli_serve_guest_reuses_same_runtime_across_run_commands() {
        let guest_path = persistent_counter_guest_path();
        let config = RunnerConfig {
            guest_module: Some(guest_path.clone()),
            granted_operations: vec!["wait".to_string()],
            ..default_runner_config()
        };
        let input = [
            serde_json::to_string(&json!({
                "command":"start",
                "config": config,
            }))
            .expect("serialize start"),
            serde_json::to_string(&json!({"command":"status"})).expect("serialize status"),
            serde_json::to_string(&json!({"command":"run"})).expect("serialize run"),
            serde_json::to_string(&json!({"command":"run"})).expect("serialize run"),
            serde_json::to_string(&json!({"command":"stop"})).expect("serialize stop"),
        ]
        .join("\n")
            + "\n";
        let mut stdout = Vec::new();

        run_cli(
            vec!["serve-guest".to_string(), guest_path.clone()].into_iter(),
            io::Cursor::new(input.into_bytes()),
            &mut stdout,
        )
        .expect("serve-guest");

        let lines = String::from_utf8(stdout).expect("utf-8");
        let responses = lines
            .lines()
            .map(|line| serde_json::from_str::<GuestServeResponse>(line).expect("parse json line"))
            .collect::<Vec<_>>();

        assert_eq!(responses.len(), 5);
        match &responses[0] {
            GuestServeResponse::Ready {
                guest_module,
                invocation_count,
                ..
            } => {
                assert_eq!(guest_module, &guest_path);
                assert_eq!(*invocation_count, 0);
            }
            other => panic!("unexpected ready response: {other:?}"),
        }
        match &responses[1] {
            GuestServeResponse::Status {
                guest_module,
                invocation_count,
                ..
            } => {
                assert_eq!(guest_module, &guest_path);
                assert_eq!(*invocation_count, 0);
            }
            other => panic!("unexpected status response: {other:?}"),
        }
        match &responses[2] {
            GuestServeResponse::RunResult {
                invocation_count,
                result,
            } => {
                assert_eq!(*invocation_count, 1);
                assert!(result.success);
                assert_eq!(
                    result.calls[0]
                        .request
                        .get("duration_ms")
                        .and_then(Value::as_u64),
                    Some(1)
                );
            }
            other => panic!("unexpected first run response: {other:?}"),
        }
        match &responses[3] {
            GuestServeResponse::RunResult {
                invocation_count,
                result,
            } => {
                assert_eq!(*invocation_count, 2);
                assert!(result.success);
                assert_eq!(
                    result.calls[0]
                        .request
                        .get("duration_ms")
                        .and_then(Value::as_u64),
                    Some(2)
                );
            }
            other => panic!("unexpected second run response: {other:?}"),
        }
        match &responses[4] {
            GuestServeResponse::Stopped { invocation_count } => {
                assert_eq!(*invocation_count, 2);
            }
            other => panic!("unexpected stop response: {other:?}"),
        }
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
    fn watch_events_collect_returns_guest_serializable_lines() {
        let mut drains = VecDeque::from(vec![Ok(vec![json!({
            "method":"Runtime.consoleAPICalled",
            "params":{"type":"log","args":[{"type":"string","value":"token-1"}]},
            "session_id":"session-2"
        })])]);

        let lines = watch_events_collect_with_drain(
            WatchEventsRequest {
                daemon_name: "stub".to_string(),
                filter: EventFilter {
                    method: Some("Runtime.consoleAPICalled".to_string()),
                    session_id: Some("session-2".to_string()),
                    ..EventFilter::default()
                },
                timeout_ms: 50,
                poll_interval_ms: 1,
                max_events: Some(1),
            },
            |_| drains.pop_front().unwrap_or_else(|| Ok(vec![])),
        )
        .expect("watch events collect");

        assert_eq!(lines.len(), 2);
        match &lines[0] {
            WatchEventsLine::Event { event, index, .. } => {
                assert_eq!(*index, 1);
                assert_eq!(
                    event.get("method").and_then(Value::as_str),
                    Some("Runtime.consoleAPICalled")
                );
            }
            other => panic!("unexpected first watch line: {other:?}"),
        }
        match &lines[1] {
            WatchEventsLine::End {
                matched_events,
                reached_max_events,
                timed_out,
                ..
            } => {
                assert_eq!(*matched_events, 1);
                assert!(*reached_max_events);
                assert!(!timed_out);
            }
            other => panic!("unexpected second watch line: {other:?}"),
        }
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
    fn drain_events_uses_meta_request_result() {
        let result = drain_events_with_sender("runner", |daemon, request| {
            assert_eq!(daemon, "runner");
            assert_eq!(request.meta.as_deref(), Some(META_DRAIN_EVENTS));
            assert!(request.params.is_none());
            Ok(DaemonResponse {
                events: Some(vec![json!({"method":"Page.loadEventFired"})]),
                ..DaemonResponse::default()
            })
        })
        .expect("drain events result");

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].get("method").and_then(Value::as_str),
            Some("Page.loadEventFired")
        );
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

    #[test]
    fn cli_http_get_prints_json_result() {
        let (base_url, handle) = spawn_http_fixture_server("cli-body");
        let input = serde_json::to_vec(&json!({
            "url": format!("{base_url}/cli"),
            "headers": {"X-Test":"cli"},
            "timeout": 5.0
        }))
        .expect("serialize request");
        let mut stdout = Vec::new();

        run_cli(
            vec!["http-get".to_string()].into_iter(),
            io::Cursor::new(input),
            &mut stdout,
        )
        .expect("http-get cli");

        let request_text = handle.join().expect("server request");
        let body: String = serde_json::from_slice(&stdout).expect("parse stdout");

        assert_eq!(body, "cli-body");
        assert!(request_text.contains("GET /cli HTTP/1.1\r\n"));
        assert!(request_text.contains("x-test: cli\r\n"));
    }

    #[test]
    fn dispatch_guest_operation_rejects_http_get_when_http_disabled() {
        let mut state = GuestHostState {
            config: RunnerConfig {
                daemon_name: "runner".to_string(),
                guest_module: None,
                granted_operations: vec!["http_get".to_string()],
                allow_http: false,
                allow_raw_cdp: false,
                persistent_guest_state: true,
            },
            calls: Vec::new(),
            error: None,
        };

        let err =
            dispatch_guest_operation(&mut state, "http_get", r#"{"url":"https://example.com"}"#)
                .expect_err("http_get should be denied");

        assert_eq!(err, "operation denied by runner config: http_get disabled");
        assert!(state.calls.is_empty());
    }

    #[test]
    fn dispatch_guest_operation_rejects_cdp_raw_when_disabled() {
        let mut state = GuestHostState {
            config: RunnerConfig {
                daemon_name: "runner".to_string(),
                guest_module: None,
                granted_operations: vec!["cdp_raw".to_string()],
                allow_http: false,
                allow_raw_cdp: false,
                persistent_guest_state: true,
            },
            calls: Vec::new(),
            error: None,
        };

        let err = dispatch_guest_operation(
            &mut state,
            "cdp_raw",
            r#"{"method":"Runtime.evaluate","params":{"expression":"2+3","returnByValue":true}}"#,
        )
        .expect_err("cdp_raw should be denied");

        assert_eq!(err, "operation denied by runner config: cdp_raw disabled");
        assert!(state.calls.is_empty());
    }

    #[test]
    fn dispatch_guest_operation_executes_http_get_when_enabled() {
        let (base_url, handle) = spawn_http_fixture_server("guest-body");
        let mut state = GuestHostState {
            config: RunnerConfig {
                daemon_name: "runner".to_string(),
                guest_module: None,
                granted_operations: vec!["http_get".to_string()],
                allow_http: true,
                allow_raw_cdp: false,
                persistent_guest_state: true,
            },
            calls: Vec::new(),
            error: None,
        };

        let response = dispatch_guest_operation(
            &mut state,
            "http_get",
            &format!(
                r#"{{"url":"{base_url}/guest","headers":{{"X-Test":"guest"}},"timeout":5.0}}"#
            ),
        )
        .expect("dispatch http_get");
        let request_text = handle.join().expect("server request");
        let body: String = serde_json::from_slice(&response).expect("parse response");

        assert_eq!(body, "guest-body");
        assert!(request_text.contains("GET /guest HTTP/1.1\r\n"));
        assert!(request_text.contains("x-test: guest\r\n"));
        assert_eq!(state.calls.len(), 1);
        assert_eq!(state.calls[0].operation, "http_get");
        let expected_url = format!("{base_url}/guest");
        assert_eq!(
            state.calls[0].request.get("url").and_then(Value::as_str),
            Some(expected_url.as_str())
        );
        assert_eq!(state.calls[0].response.as_str(), Some("guest-body"));
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
