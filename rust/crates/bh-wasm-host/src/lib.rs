use bh_protocol::{
    META_CLICK, META_CURRENT_TAB, META_DISPATCH_KEY, META_ENSURE_REAL_TAB, META_GOTO,
    META_IFRAME_TARGET, META_JS, META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO, META_PRESS_KEY,
    META_SCROLL, META_SWITCH_TAB, META_TYPE_TEXT, META_UPLOAD_FILE, META_WAIT_FOR_LOAD,
    PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerConfig {
    pub daemon_name: String,
    pub guest_module: Option<String>,
    pub granted_operations: Vec<String>,
    pub allow_http: bool,
    pub allow_raw_cdp: bool,
    pub persistent_guest_state: bool,
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

impl Default for CurrentSessionRequest {
    fn default() -> Self {
        Self {
            daemon_name: default_daemon_name(),
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
        compatibility_helper("screenshot", "Capture the current page as an image."),
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

fn compatibility_helper(name: &'static str, description: &'static str) -> HostOperation {
    HostOperation {
        name,
        kind: ProtocolFamilyKind::CompatibilityHelper,
        stability: Stability::Stable,
        description,
    }
}

fn default_daemon_name() -> String {
    "default".to_string()
}

fn default_wait_timeout_ms() -> u64 {
    15_000
}

fn default_poll_interval_ms() -> u64 {
    200
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
        default_manifest, default_runner_config, event_matches_filter, load_event_filter,
        operation_names, response_received_filter, CurrentSessionRequest, EventFilter,
        ExecutionModel, GuestTransport, ProtocolFamilyKind, Stability, WaitForEventRequest,
        WaitForLoadEventRequest, WaitForResponseRequest,
    };

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
        assert!(names.contains(&"wait_for_load_event"));
        assert!(names.contains(&"wait_for_response"));
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
}
