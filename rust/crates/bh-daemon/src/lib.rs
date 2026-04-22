use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bh_cdp::{is_browser_level_method, CdpClient, CdpEvent};
use bh_discovery::{get_ws_url, is_internal_url, runtime_paths, RuntimePaths};
use bh_protocol::{
    DaemonRequest, DaemonResponse, META_CLICK, META_CONFIGURE_DOWNLOADS, META_CURRENT_TAB,
    META_DISPATCH_KEY, META_DRAIN_EVENTS, META_ENSURE_REAL_TAB, META_GET_COOKIES, META_GOTO,
    META_HANDLE_DIALOG, META_IFRAME_TARGET, META_JS, META_LIST_TABS, META_MOUSE_DOWN,
    META_MOUSE_MOVE, META_MOUSE_UP, META_NEW_TAB, META_PAGE_INFO, META_PENDING_DIALOG,
    META_PRESS_KEY, META_PRINT_PDF, META_SCREENSHOT, META_SCROLL, META_SESSION, META_SET_COOKIES,
    META_SET_SESSION, META_SET_VIEWPORT, META_SHUTDOWN, META_SWITCH_TAB, META_TYPE_TEXT,
    META_UPLOAD_FILE, META_WAIT_FOR_LOAD,
};
use bh_remote::BrowserUseClient;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream as TokioUnixStream};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::{sleep, Instant};

pub const DEFAULT_EVENT_CAPACITY: usize = 500;
const MARK_JS: &str =
    "const m=String.fromCodePoint(0x1F7E2);if(!document.title.startsWith(m))document.title=m+' '+document.title";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonConfig {
    pub name: String,
    pub event_capacity: usize,
    pub remote_browser_id: Option<String>,
    pub browser_use_api_key: Option<String>,
}

impl DaemonConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            event_capacity: DEFAULT_EVENT_CAPACITY,
            remote_browser_id: None,
            browser_use_api_key: None,
        }
    }

    pub fn paths(&self) -> RuntimePaths {
        runtime_paths(Some(&self.name))
    }
}

pub async fn stop_remote(config: &DaemonConfig) -> Result<bool, String> {
    let Some(remote_browser_id) = config.remote_browser_id.as_deref() else {
        return Ok(false);
    };
    let Some(api_key) = config.browser_use_api_key.as_deref() else {
        return Ok(false);
    };

    BrowserUseClient::new(api_key.to_string())
        .stop_browser(remote_browser_id)
        .await?;
    Ok(true)
}

#[derive(Debug, Default)]
pub struct DaemonState {
    pub session_id: Option<String>,
    pub target_id: Option<String>,
    pub dialog: Option<Value>,
    pub events: VecDeque<Value>,
}

impl DaemonState {
    pub fn set_session(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
        self.target_id = None;
    }

    pub fn set_attachment(&mut self, session_id: impl Into<String>, target_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
        self.target_id = Some(target_id.into());
    }

    pub fn clear_session(&mut self) {
        self.session_id = None;
        self.target_id = None;
    }
}

#[derive(Clone)]
struct Daemon {
    config: DaemonConfig,
    cdp: CdpClient,
    state: Arc<Mutex<DaemonState>>,
    shutdown_tx: watch::Sender<bool>,
}

impl Daemon {
    fn new(config: DaemonConfig, cdp: CdpClient, shutdown_tx: watch::Sender<bool>) -> Self {
        Self {
            config,
            cdp,
            state: Arc::new(Mutex::new(DaemonState::default())),
            shutdown_tx,
        }
    }

    async fn attach_first_page(&self) -> Result<(), String> {
        let page = if let Some(page) = self.first_real_page().await? {
            page
        } else {
            let created = self
                .cdp
                .send_raw("Target.createTarget", json!({ "url": "about:blank" }), None)
                .await?;
            let target_id = created
                .get("targetId")
                .and_then(Value::as_str)
                .ok_or_else(|| "Target.createTarget missing targetId".to_string())?;
            log_line(
                &self.config,
                &format!("no real pages found, created about:blank ({target_id})"),
            );
            json!({"targetId": target_id, "url": "about:blank", "type": "page"})
        };

        let target_id = page
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "target missing targetId".to_string())?;
        let session_id = self.attach_to_target(target_id).await?;

        log_line(
            &self.config,
            &format!(
                "attached {} ({}) session={}",
                target_id,
                page.get("url").and_then(Value::as_str).unwrap_or(""),
                session_id
            ),
        );

        Ok(())
    }

    async fn first_real_page(&self) -> Result<Option<Value>, String> {
        let targets = self
            .cdp
            .send_raw("Target.getTargets", json!({}), None)
            .await?;
        Ok(targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|target| is_real_page(target))
            .cloned())
    }

    async fn attach_to_target(&self, target_id: &str) -> Result<String, String> {
        let attached = self
            .cdp
            .send_raw(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )
            .await?;
        let session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Target.attachToTarget missing sessionId".to_string())?
            .to_string();

        {
            let mut state = self.state.lock().await;
            state.set_attachment(session_id.clone(), target_id.to_string());
        }

        self.enable_session_domains(&session_id).await;
        Ok(session_id)
    }

    async fn mark_session(&self, session_id: &str) {
        let _ = self
            .cdp
            .send_raw(
                "Runtime.evaluate",
                json!({ "expression": MARK_JS }),
                Some(session_id),
            )
            .await;
    }

    async fn unmark_session(&self, session_id: &str) {
        let _ = self
            .cdp
            .send_raw(
                "Runtime.evaluate",
                json!({
                    "expression": "if(document.title.startsWith('\\u{1F7E2} '))document.title=document.title.slice(2)"
                }),
                Some(session_id),
            )
            .await;
    }

    async fn current_session(&self) -> Option<String> {
        self.state.lock().await.session_id.clone()
    }

    async fn current_target(&self) -> Option<String> {
        self.state.lock().await.target_id.clone()
    }

    async fn ensure_session(&self) -> Result<String, String> {
        if let Some(session_id) = self.current_session().await {
            return Ok(session_id);
        }
        self.attach_first_page().await?;
        self.current_session()
            .await
            .ok_or_else(|| "no active session after attach".to_string())
    }

    async fn enable_session_domains(&self, session_id: &str) {
        for domain in ["Page", "DOM", "Runtime", "Network", "Log", "Console"] {
            if let Err(err) = self
                .cdp
                .send_raw(&format!("{domain}.enable"), json!({}), Some(session_id))
                .await
            {
                log_line(&self.config, &format!("enable {domain}: {err}"));
            }
        }
    }

    async fn send_with_retry(
        &self,
        method: &str,
        params: Value,
        session_id: Option<String>,
    ) -> Result<Value, String> {
        let current_session = self.current_session().await;
        match self
            .cdp
            .send_raw(method, params.clone(), session_id.as_deref())
            .await
        {
            Ok(result) => Ok(result),
            Err(err)
                if err.contains("Session with given id not found")
                    && !is_browser_level_method(method)
                    && session_id.is_some()
                    && session_id == current_session =>
            {
                log_line(
                    &self.config,
                    &format!(
                        "stale session {}, re-attaching",
                        session_id.unwrap_or_default()
                    ),
                );
                self.attach_first_page().await?;
                self.cdp
                    .send_raw(method, params, self.current_session().await.as_deref())
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn list_tabs_result(&self, include_internal: bool) -> Result<Value, String> {
        let targets = self
            .cdp
            .send_raw("Target.getTargets", json!({}), None)
            .await?;
        let tabs = targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|target| target.get("type").and_then(Value::as_str) == Some("page"))
            .filter(|target| {
                include_internal
                    || !is_internal_url(target.get("url").and_then(Value::as_str).unwrap_or(""))
            })
            .map(tab_summary)
            .map(Value::Object)
            .collect::<Vec<_>>();
        Ok(Value::Array(tabs))
    }

    async fn current_tab_result(&self) -> Result<Value, String> {
        let info = if let Some(target_id) = self.current_target().await {
            self.cdp
                .send_raw("Target.getTargetInfo", json!({"targetId": target_id}), None)
                .await?
        } else {
            self.cdp
                .send_raw("Target.getTargetInfo", json!({}), None)
                .await?
        };
        let target = info
            .get("targetInfo")
            .ok_or_else(|| "Target.getTargetInfo missing targetInfo".to_string())?;
        Ok(Value::Object(tab_summary(target)))
    }

    async fn page_info_result(&self) -> Result<Value, String> {
        if let Some(dialog) = self.state.lock().await.dialog.clone() {
            return Ok(json!({ "dialog": dialog }));
        }
        let session_id = self.ensure_session().await?;
        let result = self
            .send_with_retry(
                "Runtime.evaluate",
                json!({
                    "expression": "JSON.stringify({url:location.href,title:document.title,w:innerWidth,h:innerHeight,sx:scrollX,sy:scrollY,pw:document.documentElement.scrollWidth,ph:document.documentElement.scrollHeight})",
                    "returnByValue": true
                }),
                Some(session_id),
            )
            .await?;
        let value = result
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .ok_or_else(|| "Runtime.evaluate missing result.value".to_string())?;
        match value {
            Value::String(text) => serde_json::from_str::<Value>(&text)
                .map_err(|err| format!("parse page info JSON: {err}")),
            other => Ok(other),
        }
    }

    async fn switch_tab_result(&self, target_id: &str) -> Result<Value, String> {
        if let Some(session_id) = self.current_session().await {
            self.unmark_session(&session_id).await;
        }
        self.cdp
            .send_raw(
                "Target.activateTarget",
                json!({"targetId": target_id}),
                None,
            )
            .await?;
        let session_id = self.attach_to_target(target_id).await?;
        self.mark_session(&session_id).await;
        Ok(Value::String(session_id))
    }

    async fn new_tab_result(&self, url: &str) -> Result<Value, String> {
        let created = self
            .cdp
            .send_raw("Target.createTarget", json!({"url": "about:blank"}), None)
            .await?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Target.createTarget missing targetId".to_string())?
            .to_string();
        self.switch_tab_result(&target_id).await?;
        if url != "about:blank" {
            let session_id = self.ensure_session().await?;
            self.send_with_retry("Page.navigate", json!({"url": url}), Some(session_id))
                .await?;
            self.wait_for_navigation_start_result("about:blank", 5.0)
                .await?;
        }
        Ok(Value::String(target_id))
    }

    async fn ensure_real_tab_result(&self) -> Result<Value, String> {
        let tabs = self.list_tabs_result(false).await?;
        let tabs = tabs
            .as_array()
            .cloned()
            .ok_or_else(|| "list_tabs result was not an array".to_string())?;
        if tabs.is_empty() {
            return Ok(Value::Null);
        }

        if let Ok(current) = self.current_tab_result().await {
            let is_real = current
                .get("url")
                .and_then(Value::as_str)
                .map(|url| !is_internal_url(url))
                .unwrap_or(false);
            if is_real {
                return Ok(current);
            }
        }

        let target_id = tabs
            .first()
            .and_then(|tab| tab.get("targetId"))
            .and_then(Value::as_str)
            .ok_or_else(|| "tab summary missing targetId".to_string())?;
        self.switch_tab_result(target_id).await?;
        Ok(tabs.first().cloned().unwrap_or(Value::Null))
    }

    async fn iframe_target_result(&self, url_substr: &str) -> Result<Value, String> {
        let targets = self
            .cdp
            .send_raw("Target.getTargets", json!({}), None)
            .await?;
        let target_id = targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|target| {
                target.get("type").and_then(Value::as_str) == Some("iframe")
                    && target
                        .get("url")
                        .and_then(Value::as_str)
                        .map(|url| url.contains(url_substr))
                        .unwrap_or(false)
            })
            .and_then(|target| target.get("targetId"))
            .and_then(Value::as_str);
        Ok(match target_id {
            Some(target_id) => Value::String(target_id.to_string()),
            None => Value::Null,
        })
    }

    async fn wait_for_load_result(&self, timeout_seconds: f64) -> Result<Value, String> {
        let timeout_seconds = if timeout_seconds.is_finite() && timeout_seconds > 0.0 {
            timeout_seconds
        } else {
            15.0
        };
        let deadline = Instant::now() + Duration::from_secs_f64(timeout_seconds);
        loop {
            let session_id = self.ensure_session().await?;
            let result = self
                .send_with_retry(
                    "Runtime.evaluate",
                    json!({
                        "expression": "document.readyState",
                        "returnByValue": true
                    }),
                    Some(session_id),
                )
                .await?;
            if result
                .get("result")
                .and_then(|result| result.get("value"))
                .and_then(Value::as_str)
                == Some("complete")
            {
                return Ok(Value::Bool(true));
            }
            if Instant::now() >= deadline {
                return Ok(Value::Bool(false));
            }
            sleep(Duration::from_millis(300)).await;
        }
    }

    async fn wait_for_navigation_start_result(
        &self,
        previous_url: &str,
        timeout_seconds: f64,
    ) -> Result<(), String> {
        let timeout_seconds = if timeout_seconds.is_finite() && timeout_seconds > 0.0 {
            timeout_seconds
        } else {
            5.0
        };
        let deadline = Instant::now() + Duration::from_secs_f64(timeout_seconds);
        loop {
            let session_id = self.ensure_session().await?;
            let result = self
                .send_with_retry(
                    "Runtime.evaluate",
                    json!({
                        "expression": "location.href",
                        "returnByValue": true
                    }),
                    Some(session_id),
                )
                .await?;
            let current_url = result
                .get("result")
                .and_then(|result| result.get("value"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if current_url != previous_url {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "navigation did not start before timeout from {previous_url}"
                ));
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    async fn goto_result(&self, url: &str) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry("Page.navigate", json!({"url": url}), Some(session_id))
            .await
    }

    async fn attach_transient_target(&self, target_id: &str) -> Result<String, String> {
        let attached = self
            .cdp
            .send_raw(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )
            .await?;
        let session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "Target.attachToTarget missing sessionId".to_string())?;
        self.enable_session_domains(&session_id).await;
        Ok(session_id)
    }

    async fn detach_transient_target(&self, session_id: &str) {
        let _ = self
            .cdp
            .send_raw(
                "Target.detachFromTarget",
                json!({"sessionId": session_id}),
                None,
            )
            .await;
    }

    async fn js_result(&self, expression: &str, target_id: Option<&str>) -> Result<Value, String> {
        let params = json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true
        });
        let result = if let Some(target_id) = target_id {
            let session_id = self.attach_transient_target(target_id).await?;
            let result = self
                .cdp
                .send_raw("Runtime.evaluate", params, Some(&session_id))
                .await;
            self.detach_transient_target(&session_id).await;
            result?
        } else {
            let session_id = self.ensure_session().await?;
            self.send_with_retry("Runtime.evaluate", params, Some(session_id))
                .await?
        };

        Ok(result
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    async fn click_result(
        &self,
        x: f64,
        y: f64,
        button: &str,
        clicks: i64,
    ) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": button,
                "clickCount": clicks
            }),
            Some(session_id.clone()),
        )
        .await?;
        self.send_with_retry(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": button,
                "clickCount": clicks
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn mouse_move_result(&self, x: f64, y: f64, buttons: i64) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseMoved",
                "x": x,
                "y": y,
                "button": "none",
                "buttons": buttons
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn mouse_down_result(
        &self,
        x: f64,
        y: f64,
        button: &str,
        buttons: i64,
        click_count: i64,
    ) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": button,
                "buttons": buttons,
                "clickCount": click_count
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn mouse_up_result(
        &self,
        x: f64,
        y: f64,
        button: &str,
        buttons: i64,
        click_count: i64,
    ) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": button,
                "buttons": buttons,
                "clickCount": click_count
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn screenshot_result(&self, full: bool) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        let result = self
            .send_with_retry(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": full
                }),
                Some(session_id),
            )
            .await?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| "Page.captureScreenshot missing data".to_string())?;
        Ok(Value::String(data.to_string()))
    }

    async fn set_viewport_result(
        &self,
        width: u32,
        height: u32,
        device_scale_factor: f64,
        mobile: bool,
    ) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": width,
                "height": height,
                "deviceScaleFactor": device_scale_factor,
                "mobile": mobile
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn print_pdf_result(&self, landscape: bool) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        let result = self
            .send_with_retry(
                "Page.printToPDF",
                json!({
                    "landscape": landscape,
                    "printBackground": true,
                    "preferCSSPageSize": true
                }),
                Some(session_id),
            )
            .await?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| "Page.printToPDF missing data".to_string())?;
        Ok(Value::String(data.to_string()))
    }

    async fn configure_downloads_result(&self, download_path: &str) -> Result<Value, String> {
        self.cdp
            .send_raw(
                "Browser.setDownloadBehavior",
                json!({
                    "behavior": "allow",
                    "downloadPath": download_path,
                    "eventsEnabled": true
                }),
                None,
            )
            .await?;
        Ok(Value::Null)
    }

    async fn handle_dialog_result(
        &self,
        accept: bool,
        prompt_text: Option<&str>,
    ) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        let mut params = serde_json::Map::new();
        params.insert("accept".to_string(), Value::Bool(accept));
        if let Some(prompt_text) = prompt_text {
            params.insert(
                "promptText".to_string(),
                Value::String(prompt_text.to_string()),
            );
        }
        let result = self
            .send_with_retry(
                "Page.handleJavaScriptDialog",
                Value::Object(params),
                Some(session_id),
            )
            .await?;
        self.state.lock().await.dialog = None;
        let _ = result;
        Ok(Value::Null)
    }

    async fn type_text_result(&self, text: &str) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Input.insertText",
            json!({
                "text": text
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn press_key_result(&self, key: &str, modifiers: i64) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        let (vk, code, text) = key_fields(key);
        let key_down = {
            let mut payload = json!({
                "type": "keyDown",
                "key": key,
                "code": code,
                "modifiers": modifiers,
                "windowsVirtualKeyCode": vk,
                "nativeVirtualKeyCode": vk
            });
            if let Some(text) = text.as_deref() {
                payload["text"] = Value::String(text.to_string());
            }
            payload
        };
        self.send_with_retry("Input.dispatchKeyEvent", key_down, Some(session_id.clone()))
            .await?;

        if text
            .as_deref()
            .map(|text| text.chars().count() == 1)
            .unwrap_or(false)
        {
            self.send_with_retry(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "char",
                    "text": text,
                    "key": key,
                    "code": code,
                    "modifiers": modifiers,
                    "windowsVirtualKeyCode": vk,
                    "nativeVirtualKeyCode": vk
                }),
                Some(session_id.clone()),
            )
            .await?;
        }

        self.send_with_retry(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": key,
                "code": code,
                "modifiers": modifiers,
                "windowsVirtualKeyCode": vk,
                "nativeVirtualKeyCode": vk
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn dispatch_key_result(
        &self,
        selector: &str,
        key: &str,
        event: &str,
    ) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        let selector_json =
            serde_json::to_string(selector).map_err(|err| format!("serialize selector: {err}"))?;
        let key_json = serde_json::to_string(key).map_err(|err| format!("serialize key: {err}"))?;
        let event_json =
            serde_json::to_string(event).map_err(|err| format!("serialize event: {err}"))?;
        let key_code = key_fields(key).0;
        let expression = format!(
            "(()=>{{const e=document.querySelector({selector});if(e){{e.focus();e.dispatchEvent(new KeyboardEvent({event},{{key:{key},code:{key},keyCode:{key_code},which:{key_code},bubbles:true}}));}}}})()",
            selector = selector_json,
            event = event_json,
            key = key_json,
            key_code = key_code,
        );
        self.send_with_retry(
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn scroll_result(&self, x: f64, y: f64, dx: f64, dy: f64) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseWheel",
                "x": x,
                "y": y,
                "deltaX": dx,
                "deltaY": dy
            }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn upload_file_result(
        &self,
        selector: &str,
        files: &[String],
        target_id: Option<&str>,
    ) -> Result<Value, String> {
        let session_id = if let Some(target_id) = target_id {
            self.attach_transient_target(target_id).await?
        } else {
            self.ensure_session().await?
        };
        let transient = target_id.is_some();

        let result = async {
            let document = if transient {
                self.cdp
                    .send_raw("DOM.getDocument", json!({"depth": -1}), Some(&session_id))
                    .await?
            } else {
                self.send_with_retry(
                    "DOM.getDocument",
                    json!({"depth": -1}),
                    Some(session_id.clone()),
                )
                .await?
            };
            let root_id = document
                .get("root")
                .and_then(|root| root.get("nodeId"))
                .and_then(Value::as_i64)
                .ok_or_else(|| "DOM.getDocument missing root.nodeId".to_string())?;
            let query = json!({
                "nodeId": root_id,
                "selector": selector
            });
            let node = if transient {
                self.cdp
                    .send_raw("DOM.querySelector", query, Some(&session_id))
                    .await?
            } else {
                self.send_with_retry("DOM.querySelector", query, Some(session_id.clone()))
                    .await?
            };
            let node_id = node
                .get("nodeId")
                .and_then(Value::as_i64)
                .filter(|node_id| *node_id != 0)
                .ok_or_else(|| format!("no element for {selector}"))?;
            let params = json!({
                "files": files,
                "nodeId": node_id
            });
            if transient {
                self.cdp
                    .send_raw("DOM.setFileInputFiles", params, Some(&session_id))
                    .await?;
            } else {
                self.send_with_retry("DOM.setFileInputFiles", params, Some(session_id.clone()))
                    .await?;
            }
            Ok(Value::Null)
        }
        .await;

        if transient {
            self.detach_transient_target(&session_id).await;
        }
        result
    }

    async fn get_cookies_result(&self, urls: Option<&[String]>) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        let params = urls
            .filter(|urls| !urls.is_empty())
            .map(|urls| json!({ "urls": urls }))
            .unwrap_or_else(|| json!({}));
        let result = self
            .send_with_retry("Network.getCookies", params, Some(session_id))
            .await?;
        Ok(result
            .get("cookies")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())))
    }

    async fn set_cookies_result(&self, cookies: &[Value]) -> Result<Value, String> {
        let session_id = self.ensure_session().await?;
        self.send_with_retry(
            "Network.setCookies",
            json!({ "cookies": cookies }),
            Some(session_id),
        )
        .await?;
        Ok(Value::Null)
    }

    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        match request.meta.as_deref() {
            Some(META_DRAIN_EVENTS) => {
                let mut state = self.state.lock().await;
                let events = state.events.drain(..).collect::<Vec<_>>();
                DaemonResponse {
                    events: Some(events),
                    ..DaemonResponse::default()
                }
            }
            Some(META_SESSION) => DaemonResponse {
                session_id: Some(self.current_session().await),
                ..DaemonResponse::default()
            },
            Some(META_SET_SESSION) => {
                {
                    let mut state = self.state.lock().await;
                    if let Some(session_id) = request.session_id.clone() {
                        state.set_session(session_id);
                    } else {
                        state.clear_session();
                    }
                }
                if let Some(session_id) = self.current_session().await {
                    self.enable_session_domains(&session_id).await;
                    self.mark_session(&session_id).await;
                }
                DaemonResponse {
                    session_id: Some(self.current_session().await),
                    ..DaemonResponse::default()
                }
            }
            Some(META_PENDING_DIALOG) => {
                let dialog = self
                    .state
                    .lock()
                    .await
                    .dialog
                    .clone()
                    .unwrap_or(Value::Null);
                DaemonResponse {
                    dialog: Some(dialog),
                    ..DaemonResponse::default()
                }
            }
            Some(META_PAGE_INFO) => match self.page_info_result().await {
                Ok(result) => DaemonResponse {
                    result: Some(result),
                    ..DaemonResponse::default()
                },
                Err(err) => DaemonResponse {
                    error: Some(err),
                    ..DaemonResponse::default()
                },
            },
            Some(META_LIST_TABS) => {
                let include_internal = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("include_internal"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                match self.list_tabs_result(include_internal).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_CURRENT_TAB) => match self.current_tab_result().await {
                Ok(result) => DaemonResponse {
                    result: Some(result),
                    ..DaemonResponse::default()
                },
                Err(err) => DaemonResponse {
                    error: Some(err),
                    ..DaemonResponse::default()
                },
            },
            Some(META_SWITCH_TAB) => {
                let target_id = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("target_id"))
                    .and_then(Value::as_str);
                match target_id {
                    Some(target_id) => match self.switch_tab_result(target_id).await {
                        Ok(result) => DaemonResponse {
                            result: Some(result),
                            ..DaemonResponse::default()
                        },
                        Err(err) => DaemonResponse {
                            error: Some(err),
                            ..DaemonResponse::default()
                        },
                    },
                    None => DaemonResponse {
                        error: Some("switch_tab requires params.target_id".to_string()),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_NEW_TAB) => {
                let url = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("url"))
                    .and_then(Value::as_str)
                    .unwrap_or("about:blank");
                match self.new_tab_result(url).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_ENSURE_REAL_TAB) => match self.ensure_real_tab_result().await {
                Ok(result) => DaemonResponse {
                    result: Some(result),
                    ..DaemonResponse::default()
                },
                Err(err) => DaemonResponse {
                    error: Some(err),
                    ..DaemonResponse::default()
                },
            },
            Some(META_IFRAME_TARGET) => {
                let url_substr = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("url_substr"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                match self.iframe_target_result(url_substr).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_WAIT_FOR_LOAD) => {
                let timeout_seconds = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("timeout"))
                    .and_then(Value::as_f64)
                    .unwrap_or(15.0);
                match self.wait_for_load_result(timeout_seconds).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_GOTO) => {
                let url = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("url"))
                    .and_then(Value::as_str)
                    .unwrap_or("about:blank");
                match self.goto_result(url).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_JS) => {
                let expression = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("expression"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let target_id = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("target_id"))
                    .and_then(Value::as_str);
                match self.js_result(expression, target_id).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_SCREENSHOT) => {
                let full = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("full"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match self.screenshot_result(full).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_SET_VIEWPORT) => {
                let params = request.params.as_ref();
                let width = params
                    .and_then(|params| params.get("width"))
                    .and_then(Value::as_u64)
                    .and_then(|width| u32::try_from(width).ok())
                    .unwrap_or(1280);
                let height = params
                    .and_then(|params| params.get("height"))
                    .and_then(Value::as_u64)
                    .and_then(|height| u32::try_from(height).ok())
                    .unwrap_or(800);
                let device_scale_factor = params
                    .and_then(|params| params.get("device_scale_factor"))
                    .and_then(Value::as_f64)
                    .filter(|scale| scale.is_finite() && *scale > 0.0)
                    .unwrap_or(1.0);
                let mobile = params
                    .and_then(|params| params.get("mobile"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match self
                    .set_viewport_result(width, height, device_scale_factor, mobile)
                    .await
                {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_PRINT_PDF) => {
                let landscape = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("landscape"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match self.print_pdf_result(landscape).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_CONFIGURE_DOWNLOADS) => {
                let download_path = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("download_path"))
                    .and_then(Value::as_str);
                match download_path {
                    Some(download_path) if !download_path.is_empty() => {
                        match self.configure_downloads_result(download_path).await {
                            Ok(result) => DaemonResponse {
                                result: Some(result),
                                ..DaemonResponse::default()
                            },
                            Err(err) => DaemonResponse {
                                error: Some(err),
                                ..DaemonResponse::default()
                            },
                        }
                    }
                    _ => DaemonResponse {
                        error: Some(
                            "configure_downloads requires params.download_path".to_string(),
                        ),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_HANDLE_DIALOG) => {
                let params = request.params.as_ref();
                let action = params
                    .and_then(|params| params.get("action"))
                    .and_then(Value::as_str)
                    .unwrap_or("accept");
                let prompt_text = params
                    .and_then(|params| params.get("prompt_text"))
                    .and_then(Value::as_str);
                let accept = match action {
                    "accept" => true,
                    "dismiss" => false,
                    other => {
                        return DaemonResponse {
                            error: Some(format!(
                                "handle_dialog action must be 'accept' or 'dismiss', got {other:?}"
                            )),
                            ..DaemonResponse::default()
                        };
                    }
                };
                match self.handle_dialog_result(accept, prompt_text).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_CLICK) => {
                let params = request.params.as_ref();
                let x = params
                    .and_then(|params| params.get("x"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let y = params
                    .and_then(|params| params.get("y"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let button = params
                    .and_then(|params| params.get("button"))
                    .and_then(Value::as_str)
                    .unwrap_or("left");
                let clicks = params
                    .and_then(|params| params.get("clicks"))
                    .and_then(Value::as_i64)
                    .unwrap_or(1);
                match self.click_result(x, y, button, clicks).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_MOUSE_MOVE) => {
                let params = request.params.as_ref();
                let x = params
                    .and_then(|params| params.get("x"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let y = params
                    .and_then(|params| params.get("y"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let buttons = params
                    .and_then(|params| params.get("buttons"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                match self.mouse_move_result(x, y, buttons).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_MOUSE_DOWN) => {
                let params = request.params.as_ref();
                let x = params
                    .and_then(|params| params.get("x"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let y = params
                    .and_then(|params| params.get("y"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let button = params
                    .and_then(|params| params.get("button"))
                    .and_then(Value::as_str)
                    .unwrap_or("left");
                let buttons = params
                    .and_then(|params| params.get("buttons"))
                    .and_then(Value::as_i64)
                    .unwrap_or(1);
                let click_count = params
                    .and_then(|params| params.get("click_count"))
                    .and_then(Value::as_i64)
                    .unwrap_or(1);
                match self
                    .mouse_down_result(x, y, button, buttons, click_count)
                    .await
                {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_MOUSE_UP) => {
                let params = request.params.as_ref();
                let x = params
                    .and_then(|params| params.get("x"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let y = params
                    .and_then(|params| params.get("y"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let button = params
                    .and_then(|params| params.get("button"))
                    .and_then(Value::as_str)
                    .unwrap_or("left");
                let buttons = params
                    .and_then(|params| params.get("buttons"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let click_count = params
                    .and_then(|params| params.get("click_count"))
                    .and_then(Value::as_i64)
                    .unwrap_or(1);
                match self
                    .mouse_up_result(x, y, button, buttons, click_count)
                    .await
                {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_TYPE_TEXT) => {
                let text = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                match self.type_text_result(text).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_PRESS_KEY) => {
                let params = request.params.as_ref();
                let key = params
                    .and_then(|params| params.get("key"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let modifiers = params
                    .and_then(|params| params.get("modifiers"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                match self.press_key_result(key, modifiers).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_DISPATCH_KEY) => {
                let params = request.params.as_ref();
                let selector = params
                    .and_then(|params| params.get("selector"))
                    .and_then(Value::as_str);
                let key = params
                    .and_then(|params| params.get("key"))
                    .and_then(Value::as_str)
                    .unwrap_or("Enter");
                let event = params
                    .and_then(|params| params.get("event"))
                    .and_then(Value::as_str)
                    .unwrap_or("keypress");
                match selector {
                    Some(selector) => match self.dispatch_key_result(selector, key, event).await {
                        Ok(result) => DaemonResponse {
                            result: Some(result),
                            ..DaemonResponse::default()
                        },
                        Err(err) => DaemonResponse {
                            error: Some(err),
                            ..DaemonResponse::default()
                        },
                    },
                    None => DaemonResponse {
                        error: Some("dispatch_key requires params.selector".to_string()),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_SCROLL) => {
                let params = request.params.as_ref();
                let x = params
                    .and_then(|params| params.get("x"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let y = params
                    .and_then(|params| params.get("y"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let dx = params
                    .and_then(|params| params.get("dx"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let dy = params
                    .and_then(|params| params.get("dy"))
                    .and_then(Value::as_f64)
                    .unwrap_or(-300.0);
                match self.scroll_result(x, y, dx, dy).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_UPLOAD_FILE) => {
                let params = request.params.as_ref();
                let selector = params
                    .and_then(|params| params.get("selector"))
                    .and_then(Value::as_str);
                let files = params
                    .and_then(|params| params.get("files"))
                    .and_then(Value::as_array)
                    .map(|files| {
                        files
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let target_id = params
                    .and_then(|params| params.get("target_id"))
                    .and_then(Value::as_str);
                match selector {
                    Some(selector) if !files.is_empty() => {
                        match self.upload_file_result(selector, &files, target_id).await {
                            Ok(result) => DaemonResponse {
                                result: Some(result),
                                ..DaemonResponse::default()
                            },
                            Err(err) => DaemonResponse {
                                error: Some(err),
                                ..DaemonResponse::default()
                            },
                        }
                    }
                    Some(_) => DaemonResponse {
                        error: Some("upload_file requires params.files".to_string()),
                        ..DaemonResponse::default()
                    },
                    None => DaemonResponse {
                        error: Some("upload_file requires params.selector".to_string()),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_GET_COOKIES) => {
                let urls = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("urls"))
                    .and_then(Value::as_array)
                    .map(|urls| {
                        urls.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    });
                match self.get_cookies_result(urls.as_deref()).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_SET_COOKIES) => {
                let cookies = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("cookies"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if cookies.is_empty() {
                    return DaemonResponse {
                        error: Some("set_cookies requires params.cookies".to_string()),
                        ..DaemonResponse::default()
                    };
                }
                match self.set_cookies_result(&cookies).await {
                    Ok(result) => DaemonResponse {
                        result: Some(result),
                        ..DaemonResponse::default()
                    },
                    Err(err) => DaemonResponse {
                        error: Some(err),
                        ..DaemonResponse::default()
                    },
                }
            }
            Some(META_SHUTDOWN) => {
                let _ = self.shutdown_tx.send(true);
                DaemonResponse {
                    ok: Some(true),
                    ..DaemonResponse::default()
                }
            }
            Some(other) => DaemonResponse {
                error: Some(format!("unsupported meta command: {other}")),
                ..DaemonResponse::default()
            },
            None => self.handle_cdp_request(request).await,
        }
    }

    async fn handle_cdp_request(&self, request: DaemonRequest) -> DaemonResponse {
        let Some(method) = request.method.clone() else {
            return DaemonResponse {
                error: Some("request must include 'meta' or 'method'".to_string()),
                ..DaemonResponse::default()
            };
        };
        let params = request.params.clone().unwrap_or_else(|| json!({}));
        let current_session = self.current_session().await;
        let session_id = if is_browser_level_method(&method) {
            request.session_id.clone()
        } else {
            request.session_id.clone().or(current_session.clone())
        };

        match self
            .send_with_retry(&method, params.clone(), session_id.clone())
            .await
        {
            Ok(result) => DaemonResponse {
                result: Some(result),
                ..DaemonResponse::default()
            },
            Err(err) => DaemonResponse {
                error: Some(err),
                ..DaemonResponse::default()
            },
        }
    }

    async fn handle_event(&self, event: CdpEvent) {
        let method = event.method.clone();
        let params = event.params.clone();
        let session_id = event.session_id.clone();
        let current_session = {
            let mut state = self.state.lock().await;
            push_event(
                &mut state.events,
                json!({
                    "method": method,
                    "params": params,
                    "session_id": session_id,
                }),
                self.config.event_capacity,
            );
            match method.as_str() {
                "Page.javascriptDialogOpening" => state.dialog = Some(params),
                "Page.javascriptDialogClosed" => state.dialog = None,
                _ => {}
            }
            state.session_id.clone()
        };

        if matches!(
            method.as_str(),
            "Page.loadEventFired" | "Page.domContentEventFired"
        ) {
            if let Some(session_id) = current_session {
                self.mark_session(&session_id).await;
            }
        }
    }
}

pub fn already_running(config: &DaemonConfig) -> bool {
    UnixStream::connect(config.paths().sock).is_ok()
}

pub fn initialize_runtime_files(config: &DaemonConfig) -> Result<(), String> {
    let paths = config.paths();
    if let Some(parent) = paths.log.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create log dir: {err}"))?;
    }
    File::create(&paths.log).map_err(|err| format!("create log file: {err}"))?;
    fs::write(&paths.pid, format!("{}", std::process::id()))
        .map_err(|err| format!("write pid file: {err}"))?;
    Ok(())
}

pub fn cleanup_runtime_files(config: &DaemonConfig) {
    let paths = config.paths();
    let _ = fs::remove_file(paths.pid);
    let _ = fs::remove_file(paths.sock);
}

pub fn log_tail(config: &DaemonConfig) -> Option<String> {
    let text = fs::read_to_string(config.paths().log).ok()?;
    text.lines()
        .last()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

pub fn log_line(config: &DaemonConfig, message: &str) {
    let paths = config.paths();
    if let Ok(mut file) = File::options().create(true).append(true).open(paths.log) {
        let _ = writeln!(file, "{message}");
    }
}

pub fn stop_best_effort(config: &DaemonConfig) -> Result<(), String> {
    let _ = request_shutdown(config);
    if let Some(pid) = read_pid(config)? {
        if !wait_for_exit(pid, 75, Duration::from_millis(200))? {
            terminate_process(pid)?;
        }
    }
    cleanup_runtime_files(config);
    Ok(())
}

pub async fn serve(config: &DaemonConfig) -> Result<(), String> {
    let paths = config.paths();
    let _ = fs::remove_file(&paths.sock);

    let listener = UnixListener::bind(&paths.sock).map_err(|err| format!("bind socket: {err}"))?;
    fs::set_permissions(&paths.sock, fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("chmod socket: {err}"))?;

    let url = get_ws_url()?;
    log_line(config, &format!("connecting to {url}"));
    let (cdp, events_rx) = CdpClient::connect(url).await?;
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let daemon = Daemon::new(config.clone(), cdp.clone(), shutdown_tx);
    daemon.attach_first_page().await?;
    tokio::spawn(run_event_loop(daemon.clone(), events_rx));

    log_line(
        config,
        &format!(
            "listening on {} (name={})",
            paths.sock.display(),
            paths.name
        ),
    );

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            incoming = listener.accept() => {
                let (stream, _) = incoming.map_err(|err| format!("accept socket: {err}"))?;
                let daemon = daemon.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_stream(daemon.clone(), stream).await {
                        log_line(&daemon.config, &format!("conn: {err}"));
                    }
                });
            }
        }
    }

    Ok(())
}

async fn run_event_loop(daemon: Daemon, mut events_rx: mpsc::UnboundedReceiver<CdpEvent>) {
    while let Some(event) = events_rx.recv().await {
        daemon.handle_event(event).await;
    }
}

async fn handle_stream(daemon: Daemon, stream: TokioUnixStream) -> Result<(), String> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .await
        .map_err(|err| format!("read request: {err}"))?;
    if bytes == 0 || line.trim().is_empty() {
        return Ok(());
    }

    let request = DaemonRequest::from_json_line(&line)?;
    let response = daemon.handle_request(request).await;
    let payload = response.to_json_line()?;
    write_half
        .write_all(payload.as_bytes())
        .await
        .map_err(|err| format!("write response: {err}"))?;
    Ok(())
}

fn is_real_page(target: &Value) -> bool {
    target.get("type").and_then(Value::as_str) == Some("page")
        && !is_internal_url(target.get("url").and_then(Value::as_str).unwrap_or(""))
}

fn tab_summary(target: &Value) -> serde_json::Map<String, Value> {
    let mut summary = serde_json::Map::new();
    summary.insert(
        "targetId".to_string(),
        Value::String(
            target
                .get("targetId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
    );
    summary.insert(
        "title".to_string(),
        Value::String(
            target
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
    );
    summary.insert(
        "url".to_string(),
        Value::String(
            target
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
    );
    summary
}

fn key_fields(key: &str) -> (i64, String, Option<String>) {
    match key {
        "Enter" => (13, "Enter".to_string(), Some("\r".to_string())),
        "Tab" => (9, "Tab".to_string(), Some("\t".to_string())),
        "Backspace" => (8, "Backspace".to_string(), None),
        "Escape" => (27, "Escape".to_string(), None),
        "Delete" => (46, "Delete".to_string(), None),
        " " => (32, "Space".to_string(), Some(" ".to_string())),
        "ArrowLeft" => (37, "ArrowLeft".to_string(), None),
        "ArrowUp" => (38, "ArrowUp".to_string(), None),
        "ArrowRight" => (39, "ArrowRight".to_string(), None),
        "ArrowDown" => (40, "ArrowDown".to_string(), None),
        "Home" => (36, "Home".to_string(), None),
        "End" => (35, "End".to_string(), None),
        "PageUp" => (33, "PageUp".to_string(), None),
        "PageDown" => (34, "PageDown".to_string(), None),
        _ => {
            if key.chars().count() == 1 {
                let ch = key.chars().next().unwrap_or_default();
                (
                    i64::from(u32::from(ch)),
                    key.to_string(),
                    Some(key.to_string()),
                )
            } else {
                (0, key.to_string(), None)
            }
        }
    }
}

fn push_event(events: &mut VecDeque<Value>, event: Value, capacity: usize) {
    if events.len() >= capacity {
        events.pop_front();
    }
    events.push_back(event);
}

fn request_shutdown(config: &DaemonConfig) -> Result<(), String> {
    let mut stream =
        UnixStream::connect(config.paths().sock).map_err(|err| format!("connect socket: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("set socket read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("set socket write timeout: {err}"))?;
    stream
        .write_all(b"{\"meta\":\"shutdown\"}\n")
        .map_err(|err| format!("write shutdown request: {err}"))?;
    let mut buf = [0_u8; 1024];
    let _ = stream.read(&mut buf);
    Ok(())
}

fn read_pid(config: &DaemonConfig) -> Result<Option<i32>, String> {
    match fs::read_to_string(config.paths().pid) {
        Ok(contents) => Ok(contents.trim().parse::<i32>().ok()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("read pid file: {err}")),
    }
}

fn wait_for_exit(pid: i32, polls: usize, interval: Duration) -> Result<bool, String> {
    for _ in 0..polls {
        if !process_exists(pid)? {
            return Ok(true);
        }
        thread::sleep(interval);
    }
    Ok(!process_exists(pid)?)
}

fn terminate_process(pid: i32) -> Result<(), String> {
    let result = unsafe { libc::kill(pid, libc::SIGTERM) };
    if result == 0 {
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::ESRCH => Ok(()),
        _ => Err(format!("send SIGTERM to pid {pid}: {err}")),
    }
}

fn process_exists(pid: i32) -> Result<bool, String> {
    let result = unsafe { libc::kill(pid, 0) };
    if result == 0 {
        return Ok(true);
    }

    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::ESRCH => Ok(false),
        Some(code) if code == libc::EPERM => Ok(true),
        _ => Err(format!("probe pid {pid}: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::{json, Value};

    use super::{
        is_real_page, key_fields, log_tail, push_event, stop_best_effort, stop_remote, tab_summary,
        DaemonConfig,
    };

    fn test_config(label: &str) -> DaemonConfig {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        DaemonConfig::new(format!("test-{label}-{}-{now}", std::process::id()))
    }

    #[test]
    fn push_event_keeps_only_latest_items() {
        let mut events = std::collections::VecDeque::new();
        push_event(&mut events, json!({"n": 1}), 2);
        push_event(&mut events, json!({"n": 2}), 2);
        push_event(&mut events, json!({"n": 3}), 2);

        let drained = events.into_iter().collect::<Vec<_>>();
        assert_eq!(drained, vec![json!({"n": 2}), json!({"n": 3})]);
    }

    #[test]
    fn is_real_page_filters_internal_targets() {
        assert!(is_real_page(&json!({
            "type": "page",
            "url": "https://example.com"
        })));
        assert!(!is_real_page(&json!({
            "type": "page",
            "url": "chrome://settings"
        })));
        assert!(!is_real_page(&json!({
            "type": "iframe",
            "url": "https://example.com"
        })));
    }

    #[tokio::test]
    async fn stop_remote_is_noop_without_remote_config() {
        let config = DaemonConfig::new("default");
        assert!(!stop_remote(&config).await.unwrap());
    }

    #[test]
    fn log_tail_returns_last_non_empty_line() {
        let config = test_config("log-tail");
        let paths = config.paths();
        fs::write(&paths.log, "one\n\ntwo\n").unwrap();

        let result = log_tail(&config);

        assert_eq!(result, Some("two".to_string()));
        let _ = fs::remove_file(paths.log);
    }

    #[test]
    fn stop_best_effort_cleans_up_stale_runtime_files() {
        let config = test_config("cleanup");
        let paths = config.paths();
        fs::write(&paths.pid, "not-a-pid").unwrap();
        fs::write(&paths.sock, "").unwrap();

        stop_best_effort(&config).unwrap();

        assert!(!paths.pid.exists());
        assert!(!paths.sock.exists());
    }

    #[test]
    fn tab_summary_keeps_expected_fields() {
        let summary = tab_summary(&json!({
            "targetId": "target-1",
            "title": "Example",
            "url": "https://example.com/",
            "type": "page"
        }));

        assert_eq!(
            Value::Object(summary),
            json!({
                "targetId": "target-1",
                "title": "Example",
                "url": "https://example.com/"
            })
        );
    }

    #[test]
    fn key_fields_match_python_helper_behavior() {
        assert_eq!(
            key_fields("Enter"),
            (13, "Enter".to_string(), Some("\r".to_string()))
        );
        assert_eq!(
            key_fields("a"),
            (97, "a".to_string(), Some("a".to_string()))
        );
        assert_eq!(key_fields("Escape"), (27, "Escape".to_string(), None));
        assert_eq!(
            key_fields("UnknownKey"),
            (0, "UnknownKey".to_string(), None)
        );
    }
}
