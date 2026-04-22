use bh_protocol::{
    META_CLICK, META_CURRENT_TAB, META_DISPATCH_KEY, META_ENSURE_REAL_TAB, META_GOTO,
    META_HANDLE_DIALOG, META_IFRAME_TARGET, META_JS, META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO,
    META_PRESS_KEY, META_SCREENSHOT, META_SCROLL, META_SWITCH_TAB, META_TYPE_TEXT,
    META_UPLOAD_FILE, META_WAIT_FOR_LOAD, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionModel {
    PersistentRunner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestTransport {
    HostCallsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFamilyKind {
    GeneratedCdp,
    HostUtility,
    CompatibilityHelper,
    EscapeHatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    Experimental,
    Preview,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtocolFamily {
    pub name: &'static str,
    pub kind: ProtocolFamilyKind,
    pub stability: Stability,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostOperation {
    pub name: &'static str,
    pub kind: ProtocolFamilyKind,
    pub stability: Stability,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostManifest {
    pub daemon_protocol_version: u32,
    pub execution_model: ExecutionModel,
    pub guest_transport: GuestTransport,
    pub protocol_families: Vec<ProtocolFamily>,
    pub operations: Vec<HostOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub daemon_name: String,
    pub guest_module: Option<String>,
    pub granted_operations: Vec<String>,
    pub allow_http: bool,
    pub allow_raw_cdp: bool,
    pub persistent_guest_state: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuestCallRecord {
    pub operation: String,
    pub request: Value,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuestRunResult {
    pub exit_code: i32,
    pub success: bool,
    #[serde(default)]
    pub calls: Vec<GuestCallRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum GuestServeRequest {
    Start {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guest_module: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        config: Option<RunnerConfig>,
    },
    Run,
    Status,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuestServeResponse {
    Ready {
        guest_module: String,
        persistent_guest_state: bool,
        granted_operations: Vec<String>,
        invocation_count: u64,
    },
    RunResult {
        invocation_count: u64,
        result: GuestRunResult,
    },
    Status {
        guest_module: String,
        persistent_guest_state: bool,
        granted_operations: Vec<String>,
        invocation_count: u64,
    },
    Stopped {
        invocation_count: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitRequest {
    #[serde(default = "default_wait_duration_ms")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpGetRequest {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(default = "default_http_timeout_seconds")]
    pub timeout: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CdpRawRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitResult {
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentSessionRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentSessionResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentTabRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTabsRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default = "default_include_internal")]
    pub include_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTabRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default = "default_new_tab_url")]
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchTabRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInfoRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsureRealTabRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IframeTargetRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub url_substr: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaitForLoadRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default = "default_wait_timeout_seconds")]
    pub timeout: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabSummary {
    #[serde(rename = "targetId")]
    pub target_id: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTabResult {
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchTabResult {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GotoRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_click_button")]
    pub button: String,
    #[serde(default = "default_clicks")]
    pub clicks: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeTextRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PressKeyRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub modifiers: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchKeyRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    pub selector: String,
    #[serde(default = "default_dispatch_key")]
    pub key: String,
    #[serde(default = "default_dispatch_event")]
    pub event: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_scroll_dx")]
    pub dx: f64,
    #[serde(default = "default_scroll_dy")]
    pub dy: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub full: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleDialogRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default = "default_handle_dialog_action")]
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadFileRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    pub selector: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    pub expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_subset: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForEventRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub filter: EventFilter,
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForEventResult {
    pub matched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<Value>,
    pub polls: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchEventsRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default)]
    pub filter: EventFilter,
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_events: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatchEventsLine {
    Event {
        event: Value,
        index: u64,
        elapsed_ms: u64,
    },
    End {
        matched_events: u64,
        polls: u64,
        elapsed_ms: u64,
        timed_out: bool,
        reached_max_events: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForLoadEventRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForResponseRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForConsoleRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub console_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForDialogRequest {
    #[serde(default = "default_daemon_name")]
    pub daemon_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub dialog_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default = "default_wait_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

impl Default for WaitForEventRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            filter: EventFilter::default(),
            timeout_ms: default_wait_timeout_ms(),
            poll_interval_ms: default_poll_interval_ms(),
        }
    }
}

impl Default for WaitForDialogRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            session_id: None,
            dialog_type: None,
            message: None,
            timeout_ms: default_wait_timeout_ms(),
            poll_interval_ms: default_poll_interval_ms(),
        }
    }
}

impl Default for WatchEventsRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            filter: EventFilter::default(),
            timeout_ms: default_wait_timeout_ms(),
            poll_interval_ms: default_poll_interval_ms(),
            max_events: None,
        }
    }
}

impl Default for CurrentSessionRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
        }
    }
}

impl Default for WaitRequest {
    fn default() -> Self {
        Self {
            duration_ms: default_wait_duration_ms(),
        }
    }
}

impl Default for HttpGetRequest {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: None,
            timeout: default_http_timeout_seconds(),
        }
    }
}

impl Default for CdpRawRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            method: String::new(),
            params: None,
            session_id: None,
        }
    }
}

impl Default for CurrentTabRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
        }
    }
}

impl Default for ListTabsRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            include_internal: default_include_internal(),
        }
    }
}

impl Default for NewTabRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            url: default_new_tab_url(),
        }
    }
}

impl Default for PageInfoRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
        }
    }
}

impl Default for EnsureRealTabRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
        }
    }
}

impl Default for IframeTargetRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            url_substr: String::new(),
        }
    }
}

impl Default for WaitForLoadRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            timeout: default_wait_timeout_seconds(),
        }
    }
}

impl Default for ClickRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            x: 0.0,
            y: 0.0,
            button: default_click_button(),
            clicks: default_clicks(),
        }
    }
}

impl Default for TypeTextRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            text: String::new(),
        }
    }
}

impl Default for PressKeyRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            key: String::new(),
            modifiers: 0,
        }
    }
}

impl Default for DispatchKeyRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            selector: String::new(),
            key: default_dispatch_key(),
            event: default_dispatch_event(),
        }
    }
}

impl Default for ScrollRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            x: 0.0,
            y: 0.0,
            dx: default_scroll_dx(),
            dy: default_scroll_dy(),
        }
    }
}

impl Default for ScreenshotRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            full: false,
        }
    }
}

impl Default for HandleDialogRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            action: default_handle_dialog_action(),
            prompt_text: None,
        }
    }
}

impl Default for UploadFileRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            selector: String::new(),
            files: Vec::new(),
            target_id: None,
        }
    }
}

impl CurrentSessionRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
        }
    }
}

impl CurrentTabRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
        }
    }
}

impl ListTabsRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            include_internal: self.include_internal,
        }
    }
}

impl NewTabRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            url: if self.url.trim().is_empty() {
                default_new_tab_url()
            } else {
                self.url.clone()
            },
        }
    }
}

impl SwitchTabRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            target_id: self.target_id.clone(),
        }
    }
}

impl PageInfoRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
        }
    }
}

impl EnsureRealTabRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
        }
    }
}

impl IframeTargetRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            url_substr: self.url_substr.clone(),
        }
    }
}

impl WaitForLoadRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            timeout: if self.timeout.is_finite() && self.timeout > 0.0 {
                self.timeout
            } else {
                default_wait_timeout_seconds()
            },
        }
    }
}

impl GotoRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            url: self.url.clone(),
        }
    }
}

impl ClickRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            x: self.x,
            y: self.y,
            button: if self.button.trim().is_empty() {
                default_click_button()
            } else {
                self.button.clone()
            },
            clicks: self.clicks,
        }
    }
}

impl TypeTextRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            text: self.text.clone(),
        }
    }
}

impl PressKeyRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            key: self.key.clone(),
            modifiers: self.modifiers,
        }
    }
}

impl DispatchKeyRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            selector: self.selector.clone(),
            key: if self.key.trim().is_empty() {
                default_dispatch_key()
            } else {
                self.key.clone()
            },
            event: if self.event.trim().is_empty() {
                default_dispatch_event()
            } else {
                self.event.clone()
            },
        }
    }
}

impl ScrollRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            x: self.x,
            y: self.y,
            dx: self.dx,
            dy: self.dy,
        }
    }
}

impl ScreenshotRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            full: self.full,
        }
    }
}

impl HandleDialogRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            action: if self.action.trim().is_empty() {
                default_handle_dialog_action()
            } else {
                self.action.clone()
            },
            prompt_text: self.prompt_text.clone(),
        }
    }
}

impl UploadFileRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            selector: self.selector.clone(),
            files: self.files.clone(),
            target_id: self.target_id.clone(),
        }
    }
}

impl JsRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            expression: self.expression.clone(),
            target_id: self.target_id.clone(),
        }
    }
}

impl WaitRequest {
    pub fn normalized(&self) -> Self {
        Self {
            duration_ms: self.duration_ms,
        }
    }
}

impl HttpGetRequest {
    pub fn normalized(&self) -> Self {
        Self {
            url: self.url.clone(),
            headers: self.headers.clone(),
            timeout: if self.timeout.is_finite() && self.timeout > 0.0 {
                self.timeout
            } else {
                default_http_timeout_seconds()
            },
        }
    }
}

impl CdpRawRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            method: self.method.clone(),
            params: self.params.clone(),
            session_id: self.session_id.clone(),
        }
    }
}

impl WaitForEventRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            filter: self.filter.clone(),
            timeout_ms: self.timeout_ms,
            poll_interval_ms: if self.poll_interval_ms == 0 {
                default_poll_interval_ms()
            } else {
                self.poll_interval_ms
            },
        }
    }
}

impl WaitForDialogRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            session_id: self.session_id.clone(),
            dialog_type: self.dialog_type.clone(),
            message: self.message.clone(),
            timeout_ms: self.timeout_ms,
            poll_interval_ms: if self.poll_interval_ms == 0 {
                default_poll_interval_ms()
            } else {
                self.poll_interval_ms
            },
        }
    }

    pub fn into_wait_for_event_request(self) -> WaitForEventRequest {
        let request = self.normalized();
        WaitForEventRequest {
            daemon_name: request.daemon_name,
            filter: dialog_event_filter(
                request.session_id.as_deref(),
                request.dialog_type.as_deref(),
                request.message.as_deref(),
            ),
            timeout_ms: request.timeout_ms,
            poll_interval_ms: request.poll_interval_ms,
        }
    }
}

impl WatchEventsRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            filter: self.filter.clone(),
            timeout_ms: self.timeout_ms,
            poll_interval_ms: if self.poll_interval_ms == 0 {
                default_poll_interval_ms()
            } else {
                self.poll_interval_ms
            },
            max_events: self.max_events.filter(|max| *max > 0),
        }
    }
}

impl Default for WaitForLoadEventRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            session_id: None,
            timeout_ms: default_wait_timeout_ms(),
            poll_interval_ms: default_poll_interval_ms(),
        }
    }
}

impl WaitForLoadEventRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            session_id: self.session_id.clone(),
            timeout_ms: self.timeout_ms,
            poll_interval_ms: if self.poll_interval_ms == 0 {
                default_poll_interval_ms()
            } else {
                self.poll_interval_ms
            },
        }
    }

    pub fn into_wait_for_event_request(self) -> WaitForEventRequest {
        let request = self.normalized();
        WaitForEventRequest {
            daemon_name: request.daemon_name,
            filter: load_event_filter(request.session_id.as_deref()),
            timeout_ms: request.timeout_ms,
            poll_interval_ms: request.poll_interval_ms,
        }
    }
}

impl WaitForResponseRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            session_id: self.session_id.clone(),
            url: self.url.clone(),
            status: self.status,
            timeout_ms: self.timeout_ms,
            poll_interval_ms: if self.poll_interval_ms == 0 {
                default_poll_interval_ms()
            } else {
                self.poll_interval_ms
            },
        }
    }

    pub fn into_wait_for_event_request(self) -> WaitForEventRequest {
        let request = self.normalized();
        WaitForEventRequest {
            daemon_name: request.daemon_name,
            filter: response_received_filter(
                request.session_id.as_deref(),
                &request.url,
                request.status,
            ),
            timeout_ms: request.timeout_ms,
            poll_interval_ms: request.poll_interval_ms,
        }
    }
}

impl Default for WaitForConsoleRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
            session_id: None,
            console_type: None,
            text: None,
            timeout_ms: default_wait_timeout_ms(),
            poll_interval_ms: default_poll_interval_ms(),
        }
    }
}

impl WaitForConsoleRequest {
    pub fn normalized(&self) -> Self {
        Self {
            daemon_name: if self.daemon_name.trim().is_empty() {
                default_daemon_name()
            } else {
                self.daemon_name.clone()
            },
            session_id: self.session_id.clone(),
            console_type: self.console_type.clone(),
            text: self.text.clone(),
            timeout_ms: self.timeout_ms,
            poll_interval_ms: if self.poll_interval_ms == 0 {
                default_poll_interval_ms()
            } else {
                self.poll_interval_ms
            },
        }
    }

    pub fn into_wait_for_event_request(self) -> WaitForEventRequest {
        let request = self.normalized();
        WaitForEventRequest {
            daemon_name: request.daemon_name,
            filter: console_event_filter(
                request.session_id.as_deref(),
                request.console_type.as_deref(),
                request.text.as_deref(),
            ),
            timeout_ms: request.timeout_ms,
            poll_interval_ms: request.poll_interval_ms,
        }
    }
}

pub fn default_manifest() -> HostManifest {
    HostManifest {
        daemon_protocol_version: PROTOCOL_VERSION,
        execution_model: ExecutionModel::PersistentRunner,
        guest_transport: GuestTransport::HostCallsOnly,
        protocol_families: vec![
            ProtocolFamily {
                name: "cdp.browser_protocol",
                kind: ProtocolFamilyKind::GeneratedCdp,
                stability: Stability::Preview,
                description: "Generated bindings for the Chrome browser protocol schema.",
            },
            ProtocolFamily {
                name: "cdp.js_protocol",
                kind: ProtocolFamilyKind::GeneratedCdp,
                stability: Stability::Preview,
                description: "Generated bindings for the Chrome JS protocol schema.",
            },
            ProtocolFamily {
                name: "host.events",
                kind: ProtocolFamilyKind::HostUtility,
                stability: Stability::Preview,
                description: "Runner-owned event waiting and filtering utilities.",
            },
            ProtocolFamily {
                name: "compat.helpers",
                kind: ProtocolFamilyKind::CompatibilityHelper,
                stability: Stability::Preview,
                description: "Stable convenience helpers carried forward from the Python shell.",
            },
            ProtocolFamily {
                name: "escape.raw_cdp",
                kind: ProtocolFamilyKind::EscapeHatch,
                stability: Stability::Experimental,
                description: "Deliberate raw CDP escape hatch for gaps in generated bindings or helper coverage.",
            },
        ],
        operations: default_operations(),
    }
}

pub fn default_operations() -> Vec<HostOperation> {
    vec![
        compatibility_helper(
            META_PAGE_INFO,
            "Viewport, scroll, and page metadata snapshot.",
        ),
        compatibility_helper(META_LIST_TABS, "List visible page targets."),
        compatibility_helper(
            META_CURRENT_TAB,
            "Return the currently attached page target.",
        ),
        compatibility_helper(META_NEW_TAB, "Create and attach a new page target."),
        compatibility_helper(
            META_SWITCH_TAB,
            "Activate and attach a specific page target.",
        ),
        compatibility_helper(
            META_ENSURE_REAL_TAB,
            "Recover from internal or stale tabs by selecting a real page tab.",
        ),
        compatibility_helper(
            META_IFRAME_TARGET,
            "Find an iframe target by URL substring for scoped guest operations.",
        ),
        compatibility_helper(META_GOTO, "Navigate the current page target."),
        compatibility_helper(
            META_WAIT_FOR_LOAD,
            "Wait for document readiness in the current page.",
        ),
        compatibility_helper(
            META_JS,
            "Evaluate JavaScript in the current page or iframe target.",
        ),
        compatibility_helper(META_CLICK, "Dispatch a browser-level pointer click."),
        compatibility_helper(
            META_TYPE_TEXT,
            "Insert text using browser input primitives.",
        ),
        compatibility_helper(
            META_PRESS_KEY,
            "Dispatch browser-level keydown/char/keyup sequences.",
        ),
        compatibility_helper(
            META_DISPATCH_KEY,
            "Dispatch a DOM KeyboardEvent on a matched element.",
        ),
        compatibility_helper(META_SCROLL, "Dispatch browser-level mouse wheel scrolling."),
        compatibility_helper(META_SCREENSHOT, "Capture the current page as an image."),
        compatibility_helper(
            META_HANDLE_DIALOG,
            "Accept or dismiss a pending browser dialog, with optional prompt text.",
        ),
        compatibility_helper(
            META_UPLOAD_FILE,
            "Assign files to an input element in the current page or iframe target.",
        ),
        HostOperation {
            name: "current_session",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Return the daemon's currently attached CDP session id for session-scoped waits.",
        },
        HostOperation {
            name: "wait",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Stable,
            description: "Sleep without involving the browser connection.",
        },
        HostOperation {
            name: "wait_for_event",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Wait for a filtered browser event stream match owned by the runner/daemon.",
        },
        HostOperation {
            name: "watch_events",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description: "Stream matching browser events as NDJSON until timeout or max_events.",
        },
        HostOperation {
            name: "wait_for_load_event",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Wait for Page.loadEventFired, optionally scoped to a specific attached session.",
        },
        HostOperation {
            name: "wait_for_response",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Wait for Network.responseReceived matching a URL and optional HTTP status.",
        },
        HostOperation {
            name: "wait_for_console",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Wait for browser console output matching an optional type and message text.",
        },
        HostOperation {
            name: "wait_for_dialog",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Wait for Page.javascriptDialogOpening matching an optional dialog type and message.",
        },
        HostOperation {
            name: "http_get",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description: "Issue pure HTTP reads outside the browser session.",
        },
        HostOperation {
            name: "cdp_raw",
            kind: ProtocolFamilyKind::EscapeHatch,
            stability: Stability::Experimental,
            description: "Send an explicit raw CDP request through the daemon.",
        },
    ]
}

pub fn default_runner_config() -> RunnerConfig {
    let manifest = default_manifest();
    RunnerConfig {
        daemon_name: "default".to_string(),
        guest_module: None,
        granted_operations: manifest
            .operations
            .iter()
            .filter(|operation| operation.kind != ProtocolFamilyKind::EscapeHatch)
            .map(|operation| operation.name.to_string())
            .collect(),
        allow_http: true,
        allow_raw_cdp: false,
        persistent_guest_state: true,
    }
}

pub fn operation_names() -> Vec<&'static str> {
    default_manifest()
        .operations
        .into_iter()
        .map(|operation| operation.name)
        .collect()
}

pub fn event_matches_filter(event: &Value, filter: &EventFilter) -> bool {
    if let Some(method) = filter.method.as_deref() {
        if event.get("method").and_then(Value::as_str) != Some(method) {
            return false;
        }
    }
    if let Some(session_id) = filter.session_id.as_deref() {
        if event.get("session_id").and_then(Value::as_str) != Some(session_id) {
            return false;
        }
    }
    if let Some(expected) = filter.params_subset.as_ref() {
        let Some(actual) = event.get("params") else {
            return false;
        };
        if !json_contains_subset(actual, expected) {
            return false;
        }
    }
    true
}

pub fn load_event_filter(session_id: Option<&str>) -> EventFilter {
    EventFilter {
        method: Some("Page.loadEventFired".to_string()),
        session_id: session_id.map(str::to_string),
        params_subset: None,
    }
}

pub fn response_received_filter(
    session_id: Option<&str>,
    url: &str,
    status: Option<u16>,
) -> EventFilter {
    let mut response = serde_json::Map::new();
    response.insert("url".to_string(), Value::String(url.to_string()));
    if let Some(status) = status {
        response.insert("status".to_string(), Value::from(status));
    }

    let mut params = serde_json::Map::new();
    params.insert("response".to_string(), Value::Object(response));

    EventFilter {
        method: Some("Network.responseReceived".to_string()),
        session_id: session_id.map(str::to_string),
        params_subset: Some(Value::Object(params)),
    }
}

pub fn console_event_filter(
    session_id: Option<&str>,
    console_type: Option<&str>,
    text: Option<&str>,
) -> EventFilter {
    let mut params = serde_json::Map::new();
    if let Some(console_type) = console_type {
        params.insert(
            "message".to_string(),
            Value::Object(serde_json::Map::from_iter([(
                "level".to_string(),
                Value::String(console_type.to_string()),
            )])),
        );
    }
    if let Some(text) = text {
        let message = params
            .entry("message".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(message) = message.as_object_mut() {
            message.insert("text".to_string(), Value::String(text.to_string()));
        }
    }

    EventFilter {
        method: Some("Console.messageAdded".to_string()),
        session_id: session_id.map(str::to_string),
        params_subset: (!params.is_empty()).then_some(Value::Object(params)),
    }
}

pub fn console_event_matches(event: &Value, request: &WaitForConsoleRequest) -> bool {
    let request = request.normalized();
    if let Some(session_id) = request.session_id.as_deref() {
        if event.get("session_id").and_then(Value::as_str) != Some(session_id) {
            return false;
        }
    }

    match event.get("method").and_then(Value::as_str) {
        Some("Console.messageAdded") => {
            if let Some(console_type) = request.console_type.as_deref() {
                if event
                    .pointer("/params/message/level")
                    .and_then(Value::as_str)
                    != Some(console_type)
                {
                    return false;
                }
            }
            if let Some(text) = request.text.as_deref() {
                if event
                    .pointer("/params/message/text")
                    .and_then(Value::as_str)
                    != Some(text)
                {
                    return false;
                }
            }
            true
        }
        Some("Runtime.consoleAPICalled") => {
            if let Some(console_type) = request.console_type.as_deref() {
                if event.pointer("/params/type").and_then(Value::as_str) != Some(console_type) {
                    return false;
                }
            }
            if let Some(text) = request.text.as_deref() {
                let arg = event.pointer("/params/args/0");
                let value = arg.and_then(|arg| arg.get("value")).and_then(Value::as_str);
                let description = arg
                    .and_then(|arg| arg.get("description"))
                    .and_then(Value::as_str);
                if value != Some(text) && description != Some(text) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

pub fn dialog_event_filter(
    session_id: Option<&str>,
    dialog_type: Option<&str>,
    message: Option<&str>,
) -> EventFilter {
    let mut params = serde_json::Map::new();
    if let Some(dialog_type) = dialog_type {
        params.insert("type".to_string(), Value::String(dialog_type.to_string()));
    }
    if let Some(message) = message {
        params.insert("message".to_string(), Value::String(message.to_string()));
    }

    EventFilter {
        method: Some("Page.javascriptDialogOpening".to_string()),
        session_id: session_id.map(str::to_string),
        params_subset: (!params.is_empty()).then_some(Value::Object(params)),
    }
}

fn compatibility_helper(name: &'static str, description: &'static str) -> HostOperation {
    HostOperation {
        name,
        kind: ProtocolFamilyKind::CompatibilityHelper,
        stability: Stability::Stable,
        description,
    }
}

fn default_wait_duration_ms() -> u64 {
    0
}

fn default_wait_timeout_seconds() -> f64 {
    15.0
}

fn default_http_timeout_seconds() -> f64 {
    20.0
}

fn default_daemon_name() -> String {
    "default".to_string()
}

fn default_include_internal() -> bool {
    true
}

fn default_new_tab_url() -> String {
    "about:blank".to_string()
}

fn default_click_button() -> String {
    "left".to_string()
}

fn default_clicks() -> i64 {
    1
}

fn default_dispatch_key() -> String {
    "Enter".to_string()
}

fn default_dispatch_event() -> String {
    "keypress".to_string()
}

fn default_handle_dialog_action() -> String {
    "accept".to_string()
}

fn default_wait_timeout_ms() -> u64 {
    15_000
}

fn default_poll_interval_ms() -> u64 {
    200
}

fn default_scroll_dx() -> f64 {
    0.0
}

fn default_scroll_dy() -> f64 {
    -300.0
}

fn json_contains_subset(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .map(|candidate| json_contains_subset(candidate, value))
                .unwrap_or(false)
        }),
        (Value::Array(actual), Value::Array(expected)) => {
            actual.len() == expected.len()
                && actual
                    .iter()
                    .zip(expected.iter())
                    .all(|(candidate, value)| json_contains_subset(candidate, value))
        }
        _ => actual == expected,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        console_event_filter, console_event_matches, default_manifest, default_runner_config,
        dialog_event_filter, event_matches_filter, load_event_filter, operation_names,
        response_received_filter, CdpRawRequest, ClickRequest, CurrentSessionRequest,
        CurrentTabRequest, DispatchKeyRequest, EnsureRealTabRequest, EventFilter, ExecutionModel,
        GotoRequest, GuestTransport, HandleDialogRequest, HttpGetRequest, IframeTargetRequest,
        JsRequest, ListTabsRequest, NewTabRequest, PageInfoRequest, PressKeyRequest,
        ProtocolFamilyKind, ScreenshotRequest, ScrollRequest, Stability, SwitchTabRequest,
        TypeTextRequest, UploadFileRequest, WaitForConsoleRequest, WaitForDialogRequest,
        WaitForEventRequest, WaitForLoadEventRequest, WaitForLoadRequest, WaitForResponseRequest,
        WatchEventsLine, WatchEventsRequest,
    };
    use std::collections::BTreeMap;

    #[test]
    fn manifest_uses_persistent_runner_boundary() {
        let manifest = default_manifest();

        assert_eq!(manifest.execution_model, ExecutionModel::PersistentRunner);
        assert_eq!(manifest.guest_transport, GuestTransport::HostCallsOnly);
        assert!(manifest
            .protocol_families
            .iter()
            .any(|family| family.name == "cdp.browser_protocol"
                && family.kind == ProtocolFamilyKind::GeneratedCdp));
        assert!(manifest
            .operations
            .iter()
            .any(|operation| operation.name == "wait_for_event"
                && operation.kind == ProtocolFamilyKind::HostUtility
                && operation.stability == Stability::Preview));
    }

    #[test]
    fn runner_config_disables_raw_cdp_by_default() {
        let config = default_runner_config();

        assert!(config.persistent_guest_state);
        assert!(!config.allow_raw_cdp);
        assert!(config
            .granted_operations
            .iter()
            .all(|name| name != "cdp_raw"));
    }

    #[test]
    fn operation_names_include_helper_and_escape_hatch_layers() {
        let names = operation_names();

        assert!(names.contains(&"page_info"));
        assert!(names.contains(&"current_session"));
        assert!(names.contains(&"wait_for_event"));
        assert!(names.contains(&"watch_events"));
        assert!(names.contains(&"wait_for_load_event"));
        assert!(names.contains(&"wait_for_response"));
        assert!(names.contains(&"wait_for_console"));
        assert!(names.contains(&"wait_for_dialog"));
        assert!(names.contains(&"screenshot"));
        assert!(names.contains(&"handle_dialog"));
        assert!(names.contains(&"dispatch_key"));
        assert!(names.contains(&"upload_file"));
        assert!(names.contains(&"http_get"));
        assert!(names.contains(&"cdp_raw"));
    }

    #[test]
    fn event_filter_matches_method_session_and_nested_params_subset() {
        let event = json!({
            "method": "Network.responseReceived",
            "session_id": "session-1",
            "params": {
                "requestId": "abc",
                "response": {
                    "url": "https://example.com/api",
                    "status": 200
                }
            }
        });
        let filter = EventFilter {
            method: Some("Network.responseReceived".to_string()),
            session_id: Some("session-1".to_string()),
            params_subset: Some(json!({
                "response": {
                    "status": 200
                }
            })),
        };

        assert!(event_matches_filter(&event, &filter));
    }

    #[test]
    fn event_filter_rejects_non_matching_subset() {
        let event = json!({
            "method": "Page.loadEventFired",
            "params": {
                "timestamp": 1.25
            }
        });
        let filter = EventFilter {
            params_subset: Some(json!({"timestamp": 2.0})),
            ..EventFilter::default()
        };

        assert!(!event_matches_filter(&event, &filter));
    }

    #[test]
    fn wait_for_event_request_normalizes_blank_name_and_zero_poll_interval() {
        let request = WaitForEventRequest {
            daemon_name: "   ".to_string(),
            poll_interval_ms: 0,
            ..WaitForEventRequest::default()
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.poll_interval_ms, 200);
    }

    #[test]
    fn watch_events_request_normalizes_blank_name_zero_poll_and_zero_max() {
        let request = WatchEventsRequest {
            daemon_name: "   ".to_string(),
            poll_interval_ms: 0,
            max_events: Some(0),
            ..WatchEventsRequest::default()
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.poll_interval_ms, 200);
        assert_eq!(normalized.max_events, None);
    }

    #[test]
    fn load_event_filter_scopes_to_requested_session() {
        let event = json!({
            "method": "Page.loadEventFired",
            "session_id": "session-2",
            "params": {
                "timestamp": 1.25
            }
        });

        assert!(event_matches_filter(
            &event,
            &load_event_filter(Some("session-2"))
        ));
        assert!(!event_matches_filter(
            &event,
            &load_event_filter(Some("session-1"))
        ));
    }

    #[test]
    fn wait_for_load_event_request_builds_scoped_wait_for_event_request() {
        let request = WaitForLoadEventRequest {
            daemon_name: "runner".to_string(),
            session_id: Some("session-9".to_string()),
            timeout_ms: 3210,
            poll_interval_ms: 25,
        };
        let built = request.into_wait_for_event_request();

        assert_eq!(built.daemon_name, "runner");
        assert_eq!(built.timeout_ms, 3210);
        assert_eq!(built.poll_interval_ms, 25);
        assert_eq!(built.filter.method.as_deref(), Some("Page.loadEventFired"));
        assert_eq!(built.filter.session_id.as_deref(), Some("session-9"));
        assert_eq!(built.filter.params_subset, None);
    }

    #[test]
    fn current_session_request_normalizes_blank_name() {
        let request = CurrentSessionRequest {
            daemon_name: "   ".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
    }

    #[test]
    fn current_tab_request_normalizes_blank_name() {
        let request = CurrentTabRequest {
            daemon_name: "   ".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
    }

    #[test]
    fn list_tabs_request_normalizes_blank_name() {
        let request = ListTabsRequest {
            daemon_name: "   ".to_string(),
            include_internal: false,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert!(!normalized.include_internal);
    }

    #[test]
    fn new_tab_request_normalizes_blank_name_and_url() {
        let request = NewTabRequest {
            daemon_name: "   ".to_string(),
            url: "   ".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.url, "about:blank");
    }

    #[test]
    fn switch_tab_request_normalizes_blank_name() {
        let request = SwitchTabRequest {
            daemon_name: "   ".to_string(),
            target_id: "target-7".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.target_id, "target-7");
    }

    #[test]
    fn page_info_request_normalizes_blank_name() {
        let request = PageInfoRequest {
            daemon_name: "   ".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
    }

    #[test]
    fn ensure_real_tab_request_normalizes_blank_name() {
        let request = EnsureRealTabRequest {
            daemon_name: "   ".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
    }

    #[test]
    fn iframe_target_request_normalizes_blank_name() {
        let request = IframeTargetRequest {
            daemon_name: "   ".to_string(),
            url_substr: "github.com".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.url_substr, "github.com");
    }

    #[test]
    fn wait_for_load_request_normalizes_blank_name_and_timeout() {
        let request = WaitForLoadRequest {
            daemon_name: "   ".to_string(),
            timeout: 0.0,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.timeout, 15.0);
    }

    #[test]
    fn goto_request_normalizes_blank_name() {
        let request = GotoRequest {
            daemon_name: "   ".to_string(),
            url: "https://example.com".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.url, "https://example.com");
    }

    #[test]
    fn click_request_normalizes_blank_name_and_button() {
        let request = ClickRequest {
            daemon_name: "   ".to_string(),
            x: 10.0,
            y: 20.0,
            button: "   ".to_string(),
            clicks: 2,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.button, "left");
        assert_eq!(normalized.clicks, 2);
    }

    #[test]
    fn type_text_request_normalizes_blank_name() {
        let request = TypeTextRequest {
            daemon_name: "   ".to_string(),
            text: "hello".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.text, "hello");
    }

    #[test]
    fn press_key_request_normalizes_blank_name() {
        let request = PressKeyRequest {
            daemon_name: "   ".to_string(),
            key: "Enter".to_string(),
            modifiers: 2,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.key, "Enter");
        assert_eq!(normalized.modifiers, 2);
    }

    #[test]
    fn dispatch_key_request_normalizes_blank_name_key_and_event() {
        let request = DispatchKeyRequest {
            daemon_name: "   ".to_string(),
            selector: "#search".to_string(),
            key: "   ".to_string(),
            event: "   ".to_string(),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.selector, "#search");
        assert_eq!(normalized.key, "Enter");
        assert_eq!(normalized.event, "keypress");
    }

    #[test]
    fn scroll_request_normalizes_blank_name() {
        let request = ScrollRequest {
            daemon_name: "   ".to_string(),
            x: 1.0,
            y: 2.0,
            dx: 3.0,
            dy: 4.0,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.x, 1.0);
        assert_eq!(normalized.dy, 4.0);
    }

    #[test]
    fn screenshot_request_normalizes_blank_name() {
        let request = ScreenshotRequest {
            daemon_name: "   ".to_string(),
            full: true,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert!(normalized.full);
    }

    #[test]
    fn handle_dialog_request_normalizes_blank_name_and_action() {
        let request = HandleDialogRequest {
            daemon_name: "   ".to_string(),
            action: "   ".to_string(),
            prompt_text: Some("typed value".to_string()),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.action, "accept");
        assert_eq!(normalized.prompt_text.as_deref(), Some("typed value"));
    }

    #[test]
    fn upload_file_request_normalizes_blank_name() {
        let request = UploadFileRequest {
            daemon_name: "   ".to_string(),
            selector: "#file".to_string(),
            files: vec!["/tmp/example.txt".to_string()],
            target_id: Some("iframe-1".to_string()),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.selector, "#file");
        assert_eq!(normalized.files, vec!["/tmp/example.txt".to_string()]);
        assert_eq!(normalized.target_id.as_deref(), Some("iframe-1"));
    }

    #[test]
    fn js_request_normalizes_blank_name() {
        let request = JsRequest {
            daemon_name: "   ".to_string(),
            expression: "location.href".to_string(),
            target_id: Some("iframe-1".to_string()),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.expression, "location.href");
        assert_eq!(normalized.target_id.as_deref(), Some("iframe-1"));
    }

    #[test]
    fn http_get_request_normalizes_timeout_and_keeps_headers() {
        let mut headers = BTreeMap::new();
        headers.insert("X-Test".to_string(), "value".to_string());
        let request = HttpGetRequest {
            url: "https://example.com".to_string(),
            headers: Some(headers.clone()),
            timeout: 0.0,
        };
        let normalized = request.normalized();

        assert_eq!(normalized.url, "https://example.com");
        assert_eq!(normalized.timeout, 20.0);
        assert_eq!(normalized.headers, Some(headers));
    }

    #[test]
    fn cdp_raw_request_normalizes_blank_name_and_keeps_payload() {
        let request = CdpRawRequest {
            daemon_name: "   ".to_string(),
            method: "Runtime.evaluate".to_string(),
            params: Some(json!({"expression":"2+3","returnByValue":true})),
            session_id: Some("session-1".to_string()),
        };
        let normalized = request.normalized();

        assert_eq!(normalized.daemon_name, "default");
        assert_eq!(normalized.method, "Runtime.evaluate");
        assert_eq!(
            normalized.params,
            Some(json!({"expression":"2+3","returnByValue":true}))
        );
        assert_eq!(normalized.session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn response_received_filter_scopes_url_status_and_session() {
        let event = json!({
            "method": "Network.responseReceived",
            "session_id": "session-2",
            "params": {
                "response": {
                    "url": "https://example.com/api",
                    "status": 200
                }
            }
        });

        assert!(event_matches_filter(
            &event,
            &response_received_filter(Some("session-2"), "https://example.com/api", Some(200))
        ));
        assert!(!event_matches_filter(
            &event,
            &response_received_filter(Some("session-1"), "https://example.com/api", Some(200))
        ));
        assert!(!event_matches_filter(
            &event,
            &response_received_filter(Some("session-2"), "https://example.com/api", Some(404))
        ));
    }

    #[test]
    fn wait_for_response_request_builds_scoped_wait_for_event_request() {
        let request = WaitForResponseRequest {
            daemon_name: "runner".to_string(),
            session_id: Some("session-9".to_string()),
            url: "https://example.com/api".to_string(),
            status: Some(204),
            timeout_ms: 3210,
            poll_interval_ms: 25,
        };
        let built = request.into_wait_for_event_request();

        assert_eq!(built.daemon_name, "runner");
        assert_eq!(built.timeout_ms, 3210);
        assert_eq!(built.poll_interval_ms, 25);
        assert_eq!(
            built.filter.method.as_deref(),
            Some("Network.responseReceived")
        );
        assert_eq!(built.filter.session_id.as_deref(), Some("session-9"));
        assert_eq!(
            built.filter.params_subset,
            Some(json!({
                "response": {
                    "url": "https://example.com/api",
                    "status": 204
                }
            }))
        );
    }

    #[test]
    fn console_event_filter_scopes_type_text_and_session() {
        let event = json!({
            "method": "Console.messageAdded",
            "session_id": "session-2",
            "params": {
                "message": {
                    "level": "log",
                    "text": "token-1"
                }
            }
        });

        assert!(event_matches_filter(
            &event,
            &console_event_filter(Some("session-2"), Some("log"), Some("token-1"))
        ));
        assert!(!event_matches_filter(
            &event,
            &console_event_filter(Some("session-1"), Some("log"), Some("token-1"))
        ));
        assert!(!event_matches_filter(
            &event,
            &console_event_filter(Some("session-2"), Some("error"), Some("token-1"))
        ));
        assert!(!event_matches_filter(
            &event,
            &console_event_filter(Some("session-2"), Some("log"), Some("token-2"))
        ));
    }

    #[test]
    fn wait_for_console_request_builds_scoped_wait_for_event_request() {
        let request = WaitForConsoleRequest {
            daemon_name: "runner".to_string(),
            session_id: Some("session-8".to_string()),
            console_type: Some("log".to_string()),
            text: Some("token-3".to_string()),
            timeout_ms: 2500,
            poll_interval_ms: 50,
        };
        let built = request.into_wait_for_event_request();

        assert_eq!(built.daemon_name, "runner");
        assert_eq!(built.timeout_ms, 2500);
        assert_eq!(built.poll_interval_ms, 50);
        assert_eq!(
            built.filter,
            console_event_filter(Some("session-8"), Some("log"), Some("token-3"))
        );
    }

    #[test]
    fn dialog_event_filter_scopes_type_message_and_session() {
        let event = json!({
            "method":"Page.javascriptDialogOpening",
            "session_id":"session-2",
            "params":{"type":"alert","message":"token-1"}
        });

        assert!(event_matches_filter(
            &event,
            &dialog_event_filter(Some("session-2"), Some("alert"), Some("token-1"))
        ));
        assert!(!event_matches_filter(
            &event,
            &dialog_event_filter(Some("session-1"), Some("alert"), Some("token-1"))
        ));
        assert!(!event_matches_filter(
            &event,
            &dialog_event_filter(Some("session-2"), Some("confirm"), Some("token-1"))
        ));
        assert!(!event_matches_filter(
            &event,
            &dialog_event_filter(Some("session-2"), Some("alert"), Some("token-2"))
        ));
    }

    #[test]
    fn wait_for_dialog_request_builds_scoped_wait_for_event_request() {
        let request = WaitForDialogRequest {
            daemon_name: "runner".to_string(),
            session_id: Some("session-7".to_string()),
            dialog_type: Some("alert".to_string()),
            message: Some("token-8".to_string()),
            timeout_ms: 2500,
            poll_interval_ms: 50,
        };
        let built = request.into_wait_for_event_request();

        assert_eq!(built.daemon_name, "runner");
        assert_eq!(built.timeout_ms, 2500);
        assert_eq!(built.poll_interval_ms, 50);
        assert_eq!(
            built.filter,
            dialog_event_filter(Some("session-7"), Some("alert"), Some("token-8"))
        );
    }

    #[test]
    fn console_event_matches_runtime_and_console_domain_shapes() {
        let request = WaitForConsoleRequest {
            daemon_name: "runner".to_string(),
            session_id: Some("session-5".to_string()),
            console_type: Some("log".to_string()),
            text: Some("token-9".to_string()),
            timeout_ms: 1000,
            poll_interval_ms: 50,
        };
        let runtime_event = json!({
            "method": "Runtime.consoleAPICalled",
            "session_id": "session-5",
            "params": {
                "type": "log",
                "args": [{"type": "string", "value": "token-9"}]
            }
        });
        let console_event = json!({
            "method": "Console.messageAdded",
            "session_id": "session-5",
            "params": {
                "message": {
                    "level": "log",
                    "text": "token-9"
                }
            }
        });

        assert!(console_event_matches(&runtime_event, &request));
        assert!(console_event_matches(&console_event, &request));
    }

    #[test]
    fn watch_events_line_serializes_as_tagged_ndjson_payloads() {
        let event_line = WatchEventsLine::Event {
            event: json!({"method":"Page.loadEventFired"}),
            index: 2,
            elapsed_ms: 99,
        };
        let end_line = WatchEventsLine::End {
            matched_events: 2,
            polls: 4,
            elapsed_ms: 150,
            timed_out: false,
            reached_max_events: true,
        };

        assert_eq!(
            serde_json::to_value(event_line).expect("serialize event line"),
            json!({
                "kind":"event",
                "event":{"method":"Page.loadEventFired"},
                "index":2,
                "elapsed_ms":99
            })
        );
        assert_eq!(
            serde_json::to_value(end_line).expect("serialize end line"),
            json!({
                "kind":"end",
                "matched_events":2,
                "polls":4,
                "elapsed_ms":150,
                "timed_out":false,
                "reached_max_events":true
            })
        );
    }
}
