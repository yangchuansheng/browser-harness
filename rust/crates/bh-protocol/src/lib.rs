use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

pub const META_DRAIN_EVENTS: &str = "drain_events";
pub const META_SESSION: &str = "session";
pub const META_SET_SESSION: &str = "set_session";
pub const META_PENDING_DIALOG: &str = "pending_dialog";
pub const META_SHUTDOWN: &str = "shutdown";
pub const META_PAGE_INFO: &str = "page_info";
pub const META_LIST_TABS: &str = "list_tabs";
pub const META_CURRENT_TAB: &str = "current_tab";
pub const META_SWITCH_TAB: &str = "switch_tab";
pub const META_NEW_TAB: &str = "new_tab";
pub const META_ENSURE_REAL_TAB: &str = "ensure_real_tab";
pub const META_IFRAME_TARGET: &str = "iframe_target";
pub const META_WAIT_FOR_LOAD: &str = "wait_for_load";
pub const META_JS: &str = "js";
pub const META_GOTO: &str = "goto";
pub const META_SET_VIEWPORT: &str = "set_viewport";
pub const META_PRINT_PDF: &str = "print_pdf";
pub const META_CONFIGURE_DOWNLOADS: &str = "configure_downloads";
pub const META_GET_COOKIES: &str = "get_cookies";
pub const META_SET_COOKIES: &str = "set_cookies";
pub const META_SCREENSHOT: &str = "screenshot";
pub const META_HANDLE_DIALOG: &str = "handle_dialog";
pub const META_CLICK: &str = "click";
pub const META_MOUSE_MOVE: &str = "mouse_move";
pub const META_MOUSE_DOWN: &str = "mouse_down";
pub const META_MOUSE_UP: &str = "mouse_up";
pub const META_TYPE_TEXT: &str = "type_text";
pub const META_PRESS_KEY: &str = "press_key";
pub const META_DISPATCH_KEY: &str = "dispatch_key";
pub const META_SCROLL: &str = "scroll";
pub const META_UPLOAD_FILE: &str = "upload_file";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DaemonRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,
}

impl DaemonRequest {
    pub fn from_json_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line.trim()).map_err(|err| format!("invalid request JSON: {err}"))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DaemonResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialog: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
}

impl DaemonResponse {
    pub fn to_json_line(&self) -> Result<String, String> {
        serde_json::to_string(self)
            .map(|json| format!("{json}\n"))
            .map_err(|err| format!("serialize daemon response: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DaemonRequest, DaemonResponse};

    #[test]
    fn parses_cdp_request() {
        let request = DaemonRequest::from_json_line(
            r#"{"method":"Page.navigate","params":{"url":"https://example.com"},"session_id":"abc"}"#,
        )
        .unwrap();
        assert_eq!(request.method.as_deref(), Some("Page.navigate"));
        assert_eq!(request.session_id.as_deref(), Some("abc"));
        assert_eq!(request.params, Some(json!({"url": "https://example.com"})));
    }

    #[test]
    fn serializes_meta_response() {
        let response = DaemonResponse {
            events: Some(Vec::new()),
            ..DaemonResponse::default()
        };
        assert_eq!(response.to_json_line().unwrap(), "{\"events\":[]}\n");
    }
}
