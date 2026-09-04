use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bh_cdp::{is_browser_level_method, CdpClient, CdpEvent};
use bh_discovery::{get_ws_url, inspect_marker, is_internal_url, runtime_paths, RuntimePaths};
use bh_protocol::{
    DaemonRequest, DaemonResponse, META_CLICK, META_CLOSE_TAB, META_CONFIGURE_DOWNLOADS,
    META_CONNECTION_STATUS, META_CURRENT_TAB, META_DISPATCH_KEY, META_DRAIN_EVENTS,
    META_ENSURE_REAL_TAB, META_GET_COOKIES, META_GOTO, META_HANDLE_DIALOG, META_IFRAME_TARGET,
    META_JS, META_LIST_TABS, META_MOUSE_DOWN, META_MOUSE_MOVE, META_MOUSE_UP, META_NEW_TAB,
    META_PAGE_INFO, META_PENDING_DIALOG, META_PING, META_PRESS_KEY, META_PRINT_PDF,
    META_SCREENSHOT, META_SCROLL, META_SESSION, META_SET_COOKIES, META_SET_SESSION,
    META_SET_VIEWPORT, META_SHUTDOWN, META_SWITCH_TAB, META_TYPE_TEXT, META_UPLOAD_FILE,
    META_WAIT_FOR_LOAD,
};
use bh_remote::BrowserUseClient;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream as TokioUnixStream};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::{sleep, Instant};

pub const DEFAULT_EVENT_CAPACITY: usize = 500;
const MAX_SESSION_REPLACEMENTS: usize = 32;
const MARK_JS: &str =
    "const m=String.fromCodePoint(0x1F434);if(!document.title.startsWith(m))document.title=m+' '+document.title";

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

    /// Returns "cloud", "cdp", or "local" matching the upstream Python
    /// `BROWSER_KIND` classification: cloud when BU_BROWSER_ID is set,
    /// cdp when BU_CDP_WS or BU_CDP_URL is set, otherwise local.
    pub fn browser_kind(&self) -> &'static str {
        if self.remote_browser_id.is_some() {
            "cloud"
        } else if std::env::var("BU_CDP_WS")
            .ok()
            .filter(|v| !v.is_empty())
            .is_some()
            || std::env::var("BU_CDP_URL")
                .ok()
                .filter(|v| !v.is_empty())
                .is_some()
        {
            "cdp"
        } else {
            "local"
        }
    }

    fn uses_dedicated_tab(&self) -> bool {
        self.name != "default" && self.remote_browser_id.is_none()
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
    dedicated_target_id: Option<String>,
    session_replacements: VecDeque<(String, String)>,
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

    fn replacement_for(&self, stale_session: &str) -> Option<String> {
        self.session_replacements
            .iter()
            .rev()
            .find(|(source, _)| source == stale_session)
            .map(|(_, replacement)| replacement.clone())
    }

    fn record_session_replacement(
        &mut self,
        stale_session: Option<&str>,
        replacement_session: &str,
    ) {
        let Some(stale_session) = stale_session.filter(|stale| *stale != replacement_session)
        else {
            return;
        };

        for (_, replacement) in &mut self.session_replacements {
            if replacement == stale_session {
                *replacement = replacement_session.to_string();
            }
        }
        self.session_replacements
            .retain(|(source, _)| source != stale_session);
        self.session_replacements
            .push_back((stale_session.to_string(), replacement_session.to_string()));
        while self.session_replacements.len() > MAX_SESSION_REPLACEMENTS {
            self.session_replacements.pop_front();
        }
    }
}

#[derive(Clone)]
struct Daemon {
    config: DaemonConfig,
    cdp: CdpClient,
    state: Arc<Mutex<DaemonState>>,
    session_state_lock: Arc<Mutex<()>>,
    shutdown_tx: watch::Sender<bool>,
}

impl Daemon {
    fn new(config: DaemonConfig, cdp: CdpClient, shutdown_tx: watch::Sender<bool>) -> Self {
        Self {
            config,
            cdp,
            state: Arc::new(Mutex::new(DaemonState::default())),
            session_state_lock: Arc::new(Mutex::new(())),
            shutdown_tx,
        }
    }

    async fn attach_first_page(&self) -> Result<(), String> {
        let _session_guard = self.session_state_lock.lock().await;
        self.attach_first_page_locked(None, true).await.map(|_| ())
    }

    async fn attach_first_page_locked(
        &self,
        replaces_session: Option<&str>,
        enable_domains: bool,
    ) -> Result<String, String> {
        let target_result = self
            .cdp
            .send_raw("Target.getTargets", json!({}), None)
            .await?;
        let targets = target_result
            .get("targetInfos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "Target.getTargets missing targetInfos".to_string())?;

        if self.config.uses_dedicated_tab() {
            if self.config.browser_kind() == "local" {
                self.close_inspect_tabs(&targets).await;
            }

            let (current_target, dedicated_target) = {
                let state = self.state.lock().await;
                (state.target_id.clone(), state.dedicated_target_id.clone())
            };
            let mut page = find_named_page(
                &targets,
                current_target.as_deref(),
                dedicated_target.as_deref(),
            );

            if page.is_none() {
                let refreshed = self
                    .cdp
                    .send_raw("Target.getTargets", json!({}), None)
                    .await?;
                let refreshed = refreshed
                    .get("targetInfos")
                    .and_then(Value::as_array)
                    .cloned()
                    .ok_or_else(|| "Target.getTargets missing targetInfos".to_string())?;
                let (current_target, dedicated_target) = {
                    let state = self.state.lock().await;
                    (state.target_id.clone(), state.dedicated_target_id.clone())
                };
                page = find_named_page(
                    &refreshed,
                    current_target.as_deref(),
                    dedicated_target.as_deref(),
                );
            }

            let page = match page {
                Some(page) => page,
                None => {
                    let page = self.create_blank_page().await?;
                    let target_id = page
                        .get("targetId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "target missing targetId".to_string())?;
                    self.state.lock().await.dedicated_target_id = Some(target_id.to_string());
                    log_line(
                        &self.config,
                        &format!(
                            "named daemon {}: created dedicated tab ({target_id})",
                            self.config.name
                        ),
                    );
                    page
                }
            };
            return self
                .attach_page_locked(&page, replaces_session, enable_domains)
                .await;
        }

        let mut take_over_inspect = false;
        let reusable = targets
            .iter()
            .find(|target| is_real_page(target))
            .or_else(|| targets.iter().find(|target| is_reusable_blank_page(target)))
            .or_else(|| {
                targets
                    .iter()
                    .find(|target| is_reusable_new_tab_page(target))
            })
            .cloned();
        let page = if let Some(page) = reusable {
            page
        } else if harness_opened_inspect() {
            if let Some(page) = targets
                .iter()
                .find(|target| is_inspect_tab(target))
                .cloned()
            {
                take_over_inspect = true;
                page
            } else {
                self.create_blank_page().await?
            }
        } else {
            self.create_blank_page().await?
        };

        let session_id = self
            .attach_page_locked(&page, replaces_session, enable_domains)
            .await?;

        let target_id = page
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "target missing targetId".to_string())?;

        if take_over_inspect {
            match self
                .cdp
                .send_raw(
                    "Page.navigate",
                    json!({"url": "about:blank"}),
                    Some(&session_id),
                )
                .await
            {
                Ok(_) => log_line(
                    &self.config,
                    &format!("took over inspect tab {target_id} -> about:blank"),
                ),
                Err(err) => log_line(
                    &self.config,
                    &format!("take over inspect tab {target_id}: {err}"),
                ),
            }
        }
        if self.config.browser_kind() == "local" {
            self.close_inspect_tabs(&targets).await;
        }

        Ok(session_id)
    }

    async fn attach_page_locked(
        &self,
        page: &Value,
        replaces_session: Option<&str>,
        enable_domains: bool,
    ) -> Result<String, String> {
        let target_id = page
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "target missing targetId".to_string())?;
        let session_id = self
            .attach_to_target_locked(target_id, replaces_session, enable_domains)
            .await?;
        log_line(
            &self.config,
            &format!(
                "attached {} ({}) session={}",
                target_id,
                page.get("url").and_then(Value::as_str).unwrap_or(""),
                session_id
            ),
        );
        Ok(session_id)
    }

    async fn create_blank_page(&self) -> Result<Value, String> {
        let created = self
            .cdp
            .send_raw(
                "Target.createTarget",
                create_target_params("about:blank"),
                None,
            )
            .await?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Target.createTarget missing targetId".to_string())?;
        log_line(
            &self.config,
            &format!("no real pages found, created about:blank ({target_id})"),
        );
        Ok(json!({"targetId": target_id, "url": "about:blank", "type": "page"}))
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

    async fn attach_to_target_locked(
        &self,
        target_id: &str,
        replaces_session: Option<&str>,
        enable_domains: bool,
    ) -> Result<String, String> {
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
            state.record_session_replacement(replaces_session, &session_id);
            state.set_attachment(session_id.clone(), target_id.to_string());
        }

        if enable_domains {
            self.enable_session_domains(&session_id).await;
        }
        Ok(session_id)
    }

    async fn close_inspect_tabs(&self, targets: &[Value]) {
        if !harness_opened_inspect() {
            return;
        }
        let current_target = self.current_target().await;
        for target in targets.iter().filter(|target| is_inspect_tab(target)) {
            let Some(target_id) = target.get("targetId").and_then(Value::as_str) else {
                continue;
            };
            if current_target.as_deref() == Some(target_id) {
                continue;
            }
            match self
                .cdp
                .send_raw("Target.closeTarget", json!({"targetId": target_id}), None)
                .await
            {
                Ok(_) => log_line(
                    &self.config,
                    &format!("closed leftover chrome://inspect tab {target_id}"),
                ),
                Err(err) => log_line(
                    &self.config,
                    &format!("close inspect tab {target_id}: {err}"),
                ),
            }
        }
        let _ = fs::remove_file(inspect_marker());
    }

    async fn mark_session(&self, session_id: &str) {
        if !tab_marker_enabled() {
            return;
        }
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
                    "expression": "const m=String.fromCodePoint(0x1F434)+' ';if(document.title.startsWith(m))document.title=document.title.slice(m.length)"
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
        let _session_guard = self.session_state_lock.lock().await;
        if let Some(session_id) = self.current_session().await {
            return Ok(session_id);
        }
        self.attach_first_page_locked(None, true).await?;
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
                    && session_id.is_some() =>
            {
                let stale_session = session_id.as_deref().unwrap_or_default();
                let (replacement_session, recovered_here) = {
                    let _session_guard = self.session_state_lock.lock().await;
                    let replacement = self.state.lock().await.replacement_for(stale_session);
                    if replacement.is_some() {
                        (replacement, false)
                    } else if current_session.as_deref() == Some(stale_session)
                        && self.current_session().await.as_deref() == Some(stale_session)
                    {
                        log_line(
                            &self.config,
                            &format!("stale session {stale_session}, re-attaching"),
                        );
                        let replacement = self
                            .attach_first_page_locked(Some(stale_session), false)
                            .await?;
                        (Some(replacement), true)
                    } else {
                        (None, false)
                    }
                };

                if let Some(replacement_session) = replacement_session {
                    if recovered_here {
                        self.enable_session_domains(&replacement_session).await;
                    }
                    self.cdp
                        .send_raw(method, params, Some(&replacement_session))
                        .await
                } else {
                    Err(err)
                }
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
        let target_id = self
            .current_target()
            .await
            .ok_or_else(|| "not_attached".to_string())?;
        let info = self
            .cdp
            .send_raw("Target.getTargetInfo", json!({"targetId": target_id}), None)
            .await?;
        let target = info
            .get("targetInfo")
            .ok_or_else(|| "Target.getTargetInfo missing targetInfo".to_string())?;
        Ok(Value::Object(tab_summary(target)))
    }

    async fn connection_status_result(&self) -> Result<Value, String> {
        let target_id = self
            .current_target()
            .await
            .ok_or_else(|| "not_attached".to_string())?;
        let info = self
            .cdp
            .send_raw("Target.getTargetInfo", json!({"targetId": target_id}), None)
            .await
            .map_err(|_| "cdp_disconnected".to_string())?;
        let target = info
            .get("targetInfo")
            .ok_or_else(|| "Target.getTargetInfo missing targetInfo".to_string())?;
        let page = if is_real_page(target) {
            let mut summary = tab_summary(target);
            if summary
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
            {
                summary.insert("title".to_string(), Value::String("(untitled)".to_string()));
            }
            Value::Object(summary)
        } else {
            Value::Null
        };
        Ok(json!({
            "target_id": target_id,
            "session_id": self.current_session().await,
            "page": page,
        }))
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

    async fn switch_tab_result(&self, target_id: &str, activate: bool) -> Result<Value, String> {
        let session_id = {
            let _session_guard = self.session_state_lock.lock().await;
            if let Some(session_id) = self.current_session().await {
                self.unmark_session(&session_id).await;
            }
            if activate {
                self.cdp
                    .send_raw(
                        "Target.activateTarget",
                        json!({"targetId": target_id}),
                        None,
                    )
                    .await?;
            }
            self.attach_to_target_locked(target_id, None, false).await?
        };
        self.enable_session_domains(&session_id).await;
        self.mark_session(&session_id).await;
        Ok(Value::String(session_id))
    }

    async fn new_tab_result(&self, url: &str) -> Result<Value, String> {
        if url != "about:blank" {
            if let Some(target_id) = self.current_target().await {
                if let Ok(info) = self
                    .cdp
                    .send_raw("Target.getTargetInfo", json!({"targetId": target_id}), None)
                    .await
                {
                    let target = info.get("targetInfo").cloned().unwrap_or(Value::Null);
                    if can_reuse_for_new_tab(&target, url) {
                        let current_url = target.get("url").and_then(Value::as_str).unwrap_or("");
                        let navigate_result = if url == current_url {
                            Ok(())
                        } else {
                            let session_id = self.ensure_session().await?;
                            self.send_with_retry(
                                "Page.navigate",
                                json!({"url": url}),
                                Some(session_id),
                            )
                            .await
                            .map(|_| ())
                        };
                        if navigate_result.is_ok() {
                            return Ok(Value::String(target_id));
                        }
                    }
                }
            }
        }

        let created = self
            .cdp
            .send_raw(
                "Target.createTarget",
                create_target_params("about:blank"),
                None,
            )
            .await?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Target.createTarget missing targetId".to_string())?
            .to_string();
        self.switch_tab_result(&target_id, false).await?;
        if url != "about:blank" {
            let session_id = self.ensure_session().await?;
            self.send_with_retry("Page.navigate", json!({"url": url}), Some(session_id))
                .await?;
            self.wait_for_navigation_start_result("about:blank", 5.0)
                .await?;
        }
        Ok(Value::String(target_id))
    }

    async fn close_tab_result(&self, target_id: Option<&str>) -> Result<Value, String> {
        let target_id = match target_id {
            Some(target_id) if !target_id.trim().is_empty() => target_id.to_string(),
            _ => self
                .current_target()
                .await
                .ok_or_else(|| "not_attached".to_string())?,
        };
        let closing_current = self.current_target().await.as_deref() == Some(target_id.as_str());
        let previous_session = if closing_current {
            self.current_session().await
        } else {
            None
        };

        if let Some(session_id) = previous_session.as_deref() {
            self.unmark_session(session_id).await;
        }

        self.cdp
            .send_raw("Target.closeTarget", json!({"targetId": target_id}), None)
            .await?;

        if closing_current {
            {
                let mut state = self.state.lock().await;
                state.clear_session();
                state.dialog = None;
            }
            if !self.config.uses_dedicated_tab() {
                if let Ok(Some(tab)) = self.first_real_page().await {
                    if let Some(next_target_id) = tab.get("targetId").and_then(Value::as_str) {
                        if self.switch_tab_result(next_target_id, false).await.is_err() {
                            let mut state = self.state.lock().await;
                            state.clear_session();
                        }
                    }
                }
            }
        }

        Ok(Value::Bool(true))
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
        self.switch_tab_result(target_id, false).await?;
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

    async fn detach_transient_target(&self, session_id: &str) -> Result<(), String> {
        self.cdp
            .send_raw(
                "Target.detachFromTarget",
                json!({"sessionId": session_id}),
                None,
            )
            .await
            .map(|_| ())
    }

    async fn js_result(&self, expression: &str, target_id: Option<&str>) -> Result<Value, String> {
        let params = json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true
        });
        let result = if let Some(target_id) = target_id {
            let session_id = self.attach_transient_target(target_id).await?;
            let evaluation = self
                .cdp
                .send_raw("Runtime.evaluate", params, Some(&session_id))
                .await;
            let detach = self.detach_transient_target(&session_id).await;
            match evaluation {
                Err(err) => return Err(err),
                Ok(result) => match detach {
                    Ok(()) => result,
                    Err(err) if detach_reports_missing_session(&err) => result,
                    Err(err) => return Err(err),
                },
            }
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

    async fn screenshot_result(&self, full: bool, max_dim: Option<u32>) -> Result<Value, String> {
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
        if let Some(max_dim) = max_dim.filter(|value| *value > 0) {
            return shrink_png_data_url(data, max_dim).map(Value::String);
        }
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
        let modifiers = press_key_effective_modifiers(key, modifiers);
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
            let _ = self.detach_transient_target(&session_id).await;
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
            Some(META_PING) => DaemonResponse {
                result: Some(
                    json!({"pong": true, "pid": std::process::id(), "browser_kind": self.config.browser_kind()}),
                ),
                ..DaemonResponse::default()
            },
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
                let (old_session, new_session) = {
                    let _session_guard = self.session_state_lock.lock().await;
                    let old_session = self.current_session().await;
                    let mut state = self.state.lock().await;
                    if let Some(session_id) = request.session_id.clone() {
                        if let Some(target_id) = request.target_id.clone().or_else(|| {
                            request
                                .params
                                .as_ref()?
                                .get("target_id")?
                                .as_str()
                                .map(str::to_string)
                        }) {
                            state.set_attachment(session_id, target_id);
                        } else {
                            state.set_session(session_id);
                        }
                    } else {
                        state.clear_session();
                    }
                    (old_session, state.session_id.clone())
                };
                if let Some(session_id) = new_session {
                    if old_session.as_deref().is_some_and(|old| old != session_id) {
                        if let Some(old_session) = old_session {
                            let _ = self
                                .cdp
                                .send_raw("Network.disable", json!({}), Some(&old_session))
                                .await;
                        }
                    }
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
            Some(META_CONNECTION_STATUS) => match self.connection_status_result().await {
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
                let activate = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("activate"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match target_id {
                    Some(target_id) => match self.switch_tab_result(target_id, activate).await {
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
            Some(META_CLOSE_TAB) => {
                let target_id = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("target_id"))
                    .and_then(Value::as_str);
                match self.close_tab_result(target_id).await {
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
                let max_dim = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("max_dim"))
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok());
                match self.screenshot_result(full, max_dim).await {
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
        let browser_level = is_browser_level_method(&method);
        let explicit_session = request.session_id.is_some();
        let current_session = self.current_session().await;
        let session_id = if browser_level {
            None
        } else {
            request.session_id.clone().or(current_session.clone())
        };

        let result = if browser_level || explicit_session {
            self.cdp
                .send_raw(&method, params, session_id.as_deref())
                .await
        } else {
            self.send_with_retry(&method, params, session_id).await
        };

        match result {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingGeneration {
    pub pid: i32,
    pub started: String,
}

pub struct SpawnLock(File);

impl Drop for SpawnLock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub fn spawn_lock(config: &DaemonConfig) -> Result<SpawnLock, String> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(config.paths().pid.with_extension("spawnlock"))
        .map_err(|err| format!("open daemon spawn lock: {err}"))?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(format!(
            "lock daemon spawn lock: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(SpawnLock(file))
}

pub fn process_start_fingerprint(pid: i32) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        let raw = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        return raw
            .rsplit_once(") ")?
            .1
            .split_whitespace()
            .nth(19)
            .map(str::to_string);
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("ps")
            .args(["-o", "lstart=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        return output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|value| !value.is_empty());
    }
    #[allow(unreachable_code)]
    None
}

pub fn publish_pending_generation(
    config: &DaemonConfig,
    pid: i32,
) -> Result<PendingGeneration, String> {
    let started = process_start_fingerprint(pid)
        .ok_or_else(|| format!("cannot fingerprint daemon pid {pid}"))?;
    let generation = PendingGeneration { pid, started };
    let paths = config.paths();
    let tmp = paths
        .pid
        .with_extension(format!("pid.{}.tmp", std::process::id()));
    let content =
        serde_json::to_vec(&json!({"pid": generation.pid, "started": generation.started}))
            .map_err(|err| format!("serialize pending pid record: {err}"))?;
    fs::write(&tmp, content).map_err(|err| format!("write pending pid record: {err}"))?;
    fs::rename(&tmp, &paths.pid).map_err(|err| format!("publish pending pid record: {err}"))?;
    Ok(generation)
}

pub fn pending_generation(config: &DaemonConfig) -> Result<Option<PendingGeneration>, String> {
    let raw = match fs::read_to_string(config.paths().pid) {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("read pid file: {err}")),
    };
    let record: Value = match serde_json::from_str(&raw) {
        Ok(record) => record,
        Err(_) => return Ok(None),
    };
    let Some(pid) = record
        .get("pid")
        .and_then(Value::as_i64)
        .and_then(|pid| i32::try_from(pid).ok())
    else {
        return Ok(None);
    };
    let Some(started) = record
        .get("started")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok(None);
    };
    Ok(
        (process_start_fingerprint(pid).as_deref() == Some(started.as_str()))
            .then_some(PendingGeneration { pid, started }),
    )
}

pub fn initialize_runtime_files(config: &DaemonConfig) -> Result<(), String> {
    let paths = config.paths();
    if let Some(parent) = paths.log.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create log dir: {err}"))?;
    }
    File::create(&paths.log).map_err(|err| format!("create log file: {err}"))?;
    publish_pending_generation(config, std::process::id() as i32)?;
    Ok(())
}

pub fn cleanup_runtime_files(config: &DaemonConfig) {
    let paths = config.paths();
    let own = process_start_fingerprint(std::process::id() as i32);
    let record = fs::read_to_string(&paths.pid)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|record| {
            record
                .get("started")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    if own.is_some() && own == record {
        let _ = fs::remove_file(paths.pid);
        let _ = fs::remove_file(paths.sock);
    }
}

fn cleanup_generation_files(config: &DaemonConfig, expected: &PendingGeneration) {
    if generation_record_matches(config, expected) {
        let paths = config.paths();
        let _ = fs::remove_file(paths.pid);
        let _ = fs::remove_file(paths.sock);
    }
}

fn generation_record_matches(config: &DaemonConfig, expected: &PendingGeneration) -> bool {
    fs::read_to_string(config.paths().pid)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .is_some_and(|record| {
            record.get("pid").and_then(Value::as_i64) == Some(i64::from(expected.pid))
                && record.get("started").and_then(Value::as_str) == Some(expected.started.as_str())
        })
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
    let ready = already_running(config);
    let _lock = spawn_lock(config)?;
    let expected = pending_generation(config)?;
    let _ = request_shutdown(config);
    if ready {
        if let Some(generation) = expected.as_ref() {
            if !wait_for_exit(generation.pid, 75, Duration::from_millis(200))? {
                if pending_generation(config)? == Some(generation.clone()) {
                    terminate_process(generation.pid)?;
                }
            }
        }
        if let Some(generation) = expected.as_ref() {
            cleanup_generation_files(config, generation);
        }
    } else if let Some(generation) = expected {
        if !already_running(config) && pending_generation(config)? == Some(generation.clone()) {
            terminate_process(generation.pid)?;
            if pending_generation(config)? == Some(generation.clone()) {
                cleanup_generation_files(config, &generation);
            }
        }
    }
    Ok(())
}

pub fn tab_marker_enabled() -> bool {
    !std::env::var("BH_TAB_MARKER").ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

fn detach_reports_missing_session(error: &str) -> bool {
    matches!(
        error,
        "No session with given id" | "Session with given id not found"
    )
}

pub async fn serve(config: &DaemonConfig) -> Result<(), String> {
    let paths = config.paths();
    let _ = fs::remove_file(&paths.sock);

    let url = get_ws_url()?;
    log_line(
        config,
        &format!("connecting to {}", redact_cdp_endpoint(&url)),
    );
    let is_local = config.browser_kind() == "local";
    if is_local {
        log_line(
            config,
            "handshake-wait: if Chrome shows an Allow remote debugging popup, click Allow",
        );
    }
    let (cdp, events_rx) = if is_local {
        CdpClient::connect(url).await?
    } else {
        CdpClient::connect_with_timeout(url, 45).await?
    };
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let daemon = Daemon::new(config.clone(), cdp.clone(), shutdown_tx);
    daemon.attach_first_page().await?;

    let listener = UnixListener::bind(&paths.sock).map_err(|err| format!("bind socket: {err}"))?;
    fs::set_permissions(&paths.sock, fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("chmod socket: {err}"))?;
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

fn create_target_params(url: &str) -> Value {
    json!({"url": url, "background": true})
}

fn find_named_page(
    targets: &[Value],
    current_target: Option<&str>,
    dedicated_target: Option<&str>,
) -> Option<Value> {
    [current_target, dedicated_target]
        .into_iter()
        .flatten()
        .find_map(|target_id| {
            targets
                .iter()
                .find(|target| {
                    target.get("type").and_then(Value::as_str) == Some("page")
                        && target.get("targetId").and_then(Value::as_str) == Some(target_id)
                })
                .cloned()
        })
}

fn is_real_page(target: &Value) -> bool {
    target.get("type").and_then(Value::as_str) == Some("page")
        && !is_internal_url(target.get("url").and_then(Value::as_str).unwrap_or(""))
}

fn is_reusable_blank_page(target: &Value) -> bool {
    let url = target.get("url").and_then(Value::as_str).unwrap_or("");
    target.get("type").and_then(Value::as_str) == Some("page")
        && (url == "about:blank" || url == "data:text/html," || url.starts_with("about:blank#"))
        && !target
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .starts_with("Starting agent ")
}

fn is_reusable_new_tab_page(target: &Value) -> bool {
    let url = target.get("url").and_then(Value::as_str).unwrap_or("");
    target.get("type").and_then(Value::as_str) == Some("page")
        && [
            "chrome://newtab",
            "chrome://new-tab-page",
            "edge://newtab",
            "about:newtab",
        ]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

fn is_inspect_tab(target: &Value) -> bool {
    target.get("type").and_then(Value::as_str) == Some("page")
        && target
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .starts_with("chrome://inspect")
}

fn can_reuse_for_new_tab(target: &Value, requested_url: &str) -> bool {
    if requested_url == "about:blank" {
        return false;
    }
    if target.get("type").and_then(Value::as_str) != Some("page") {
        return false;
    }
    target
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
        || is_reusable_blank_page(target)
        || is_reusable_new_tab_page(target)
}

fn harness_opened_inspect() -> bool {
    inspect_marker().exists()
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
                printable_key_fields(ch)
                    .map(|(vk, code)| (vk, code, Some(key.to_string())))
                    .unwrap_or((0, String::new(), Some(key.to_string())))
            } else {
                (0, key.to_string(), None)
            }
        }
    }
}

fn press_key_effective_modifiers(key: &str, modifiers: i64) -> i64 {
    let needs_shift = key.chars().count() == 1
        && key.chars().next().is_some_and(|ch| {
            ch.is_ascii_uppercase()
                || matches!(
                    ch,
                    '~' | '!'
                        | '@'
                        | '#'
                        | '$'
                        | '%'
                        | '^'
                        | '&'
                        | '*'
                        | '('
                        | ')'
                        | '_'
                        | '+'
                        | '{'
                        | '}'
                        | '|'
                        | ':'
                        | '"'
                        | '<'
                        | '>'
                        | '?'
                )
        });
    if needs_shift && modifiers & 7 == 0 {
        modifiers | 8
    } else {
        modifiers
    }
}

fn printable_key_fields(ch: char) -> Option<(i64, String)> {
    let unshifted = match ch {
        '~' => '`',
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '_' => '-',
        '+' => '=',
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        other => other,
    };
    if unshifted.is_ascii_alphabetic() {
        let upper = unshifted.to_ascii_uppercase();
        return Some((i64::from(upper as u8), format!("Key{upper}")));
    }
    if unshifted.is_ascii_digit() {
        return Some((i64::from(unshifted as u8), format!("Digit{unshifted}")));
    }
    let (code, vk) = match unshifted {
        '`' => ("Backquote", 192),
        '-' => ("Minus", 189),
        '=' => ("Equal", 187),
        '[' => ("BracketLeft", 219),
        ']' => ("BracketRight", 221),
        '\\' => ("Backslash", 220),
        ';' => ("Semicolon", 186),
        '\'' => ("Quote", 222),
        ',' => ("Comma", 188),
        '.' => ("Period", 190),
        '/' => ("Slash", 191),
        _ => return None,
    };
    Some((vk, code.to_string()))
}

fn redact_cdp_endpoint(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return "<redacted-cdp-endpoint>".to_string();
    };
    let scheme = &url[..scheme_end];
    let authority = &url[scheme_end + 3..];
    let authority = authority.split('/').next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    if host.is_empty() {
        "<redacted-cdp-endpoint>".to_string()
    } else {
        format!("{scheme}://{host}/<redacted>")
    }
}

fn push_event(events: &mut VecDeque<Value>, event: Value, capacity: usize) {
    if events.len() >= capacity {
        events.pop_front();
    }
    events.push_back(event);
}

fn shrink_png_data_url(encoded: &str, max_dim: u32) -> Result<String, String> {
    let bytes = decode_base64_standard(encoded)?;
    let (width, height) = png_dimensions_from_bytes(&bytes)?;
    if width <= max_dim && height <= max_dim {
        return Ok(encoded.to_string());
    }

    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|err| format!("decode screenshot PNG: {err}"))?;
    let resized = image.thumbnail(max_dim, max_dim);
    let mut output = Cursor::new(Vec::new());
    resized
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|err| format!("encode resized screenshot PNG: {err}"))?;
    Ok(encode_base64_standard(&output.into_inner()))
}

#[cfg(test)]
fn png_dimensions_from_base64(encoded: &str) -> Result<(u32, u32), String> {
    let bytes = decode_base64_standard(encoded)?;
    png_dimensions_from_bytes(&bytes)
}

fn png_dimensions_from_bytes(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("screenshot result was not a PNG".to_string());
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Ok((width, height))
}

fn decode_base64_standard(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut chunk = [0u8; 4];
    let mut len = 0usize;
    for byte in input.bytes().filter(|b| !b.is_ascii_whitespace()) {
        let value = if byte == b'=' {
            64
        } else {
            TABLE
                .iter()
                .position(|candidate| *candidate == byte)
                .ok_or_else(|| "invalid base64 character".to_string())? as u8
        };
        chunk[len] = value;
        len += 1;
        if len == 4 {
            if chunk[0] == 64 || chunk[1] == 64 {
                return Err("invalid base64 padding".to_string());
            }
            out.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                out.push((chunk[1] << 4) | (chunk[2] >> 2));
            }
            if chunk[3] != 64 {
                out.push((chunk[2] << 6) | chunk[3]);
            }
            len = 0;
        }
    }
    if len != 0 {
        return Err("invalid base64 length".to_string());
    }
    Ok(out)
}

fn encode_base64_standard(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
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
        can_reuse_for_new_tab, create_target_params, detach_reports_missing_session,
        encode_base64_standard, find_named_page, is_inspect_tab, is_real_page,
        is_reusable_blank_page, is_reusable_new_tab_page, key_fields, log_tail, pending_generation,
        png_dimensions_from_base64, press_key_effective_modifiers, process_start_fingerprint,
        publish_pending_generation, push_event, redact_cdp_endpoint, shrink_png_data_url,
        stop_best_effort, stop_remote, tab_marker_enabled, tab_summary, DaemonConfig, DaemonState,
        MAX_SESSION_REPLACEMENTS,
    };

    fn test_config(label: &str) -> DaemonConfig {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        DaemonConfig::new(format!("test-{label}-{}-{now}", std::process::id()))
    }

    #[test]
    fn tab_marker_default_and_opt_out_values_match_upstream() {
        let previous = std::env::var_os("BH_TAB_MARKER");
        std::env::remove_var("BH_TAB_MARKER");
        assert!(tab_marker_enabled());
        std::env::set_var("BH_TAB_MARKER", "enabled");
        assert!(tab_marker_enabled());
        for value in ["0", "false", "no", "off", "FALSE", "No", "OFF"] {
            std::env::set_var("BH_TAB_MARKER", value);
            assert!(!tab_marker_enabled(), "{value}");
        }
        if let Some(previous) = previous {
            std::env::set_var("BH_TAB_MARKER", previous);
        } else {
            std::env::remove_var("BH_TAB_MARKER");
        }
    }

    #[test]
    fn transient_detach_ignores_only_chrome_missing_session_messages() {
        assert!(detach_reports_missing_session("No session with given id"));
        assert!(detach_reports_missing_session(
            "Session with given id not found"
        ));
        assert!(!detach_reports_missing_session("Target closed"));
    }

    #[test]
    fn pending_generation_is_fingerprinted_and_atomic() {
        let config = test_config("generation");
        let generation = publish_pending_generation(&config, std::process::id() as i32).unwrap();
        assert_eq!(pending_generation(&config).unwrap(), Some(generation));
        assert!(process_start_fingerprint(std::process::id() as i32).is_some());
        let _ = fs::remove_file(config.paths().pid);
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

    #[test]
    fn reusable_blank_page_excludes_agent_starting_tabs() {
        assert!(is_reusable_blank_page(&json!({
            "type": "page",
            "url": "about:blank#ready",
            "title": ""
        })));
        assert!(is_reusable_blank_page(&json!({
            "type": "page", "url": "data:text/html,", "title": ""
        })));
        assert!(!is_reusable_blank_page(&json!({
            "type": "page",
            "url": "about:blank",
            "title": "Starting agent task"
        })));
        assert!(!is_reusable_blank_page(&json!({
            "type": "iframe",
            "url": "about:blank"
        })));
    }

    #[test]
    fn reusable_new_tab_page_matches_chromium_family_urls() {
        for url in [
            "chrome://newtab/",
            "chrome://new-tab-page/",
            "edge://newtab/",
            "about:newtab",
        ] {
            assert!(is_reusable_new_tab_page(&json!({
                "type": "page",
                "url": url
            })));
        }
        assert!(!is_reusable_new_tab_page(&json!({
            "type": "page",
            "url": "https://example.com"
        })));
    }

    #[test]
    fn new_tab_reuses_blank_pages_only_for_navigation() {
        let blank = json!({
            "type": "page",
            "url": "about:blank",
            "title": ""
        });
        let new_tab = json!({
            "type": "page",
            "url": "chrome://newtab/"
        });

        assert!(can_reuse_for_new_tab(&blank, "https://example.com"));
        assert!(can_reuse_for_new_tab(&new_tab, "https://example.com"));
        assert!(!can_reuse_for_new_tab(&blank, "about:blank"));
        assert!(!can_reuse_for_new_tab(
            &json!({"type": "iframe", "url": ""}),
            "https://example.com"
        ));
    }

    #[test]
    fn target_creation_stays_in_the_background() {
        assert_eq!(
            create_target_params("about:blank"),
            json!({"url": "about:blank", "background": true})
        );
    }

    #[test]
    fn named_page_prefers_current_target_then_dedicated_target() {
        let targets = vec![
            json!({"targetId": "dedicated", "type": "page", "url": "about:blank"}),
            json!({"targetId": "current", "type": "page", "url": "https://example.com"}),
        ];

        assert_eq!(
            find_named_page(&targets, Some("current"), Some("dedicated"))
                .and_then(|target| target.get("targetId").cloned()),
            Some(json!("current"))
        );
        assert_eq!(
            find_named_page(&targets, Some("missing"), Some("dedicated"))
                .and_then(|target| target.get("targetId").cloned()),
            Some(json!("dedicated"))
        );
    }

    #[test]
    fn named_daemons_use_dedicated_tabs_except_for_cloud_browsers() {
        assert!(DaemonConfig::new("worker-a").uses_dedicated_tab());
        assert!(!DaemonConfig::new("default").uses_dedicated_tab());

        let mut cloud = DaemonConfig::new("worker-a");
        cloud.remote_browser_id = Some("browser-id".to_string());
        assert!(!cloud.uses_dedicated_tab());
    }

    #[test]
    fn session_replacements_follow_recovery_chains_and_stay_bounded() {
        let mut state = DaemonState::default();
        state.record_session_replacement(Some("stale-a"), "replacement-b");
        state.record_session_replacement(Some("replacement-b"), "replacement-c");

        assert_eq!(
            state.replacement_for("stale-a").as_deref(),
            Some("replacement-c")
        );
        assert_eq!(
            state.replacement_for("replacement-b").as_deref(),
            Some("replacement-c")
        );

        for index in 0..=MAX_SESSION_REPLACEMENTS {
            state.record_session_replacement(
                Some(&format!("stale-{index}")),
                &format!("replacement-{index}"),
            );
        }
        assert_eq!(state.session_replacements.len(), MAX_SESSION_REPLACEMENTS);
        assert_eq!(state.replacement_for("stale-0"), None);
    }

    #[test]
    fn inspect_tab_requires_page_target() {
        assert!(is_inspect_tab(&json!({
            "type": "page",
            "url": "chrome://inspect/#remote-debugging"
        })));
        assert!(!is_inspect_tab(&json!({
            "type": "iframe",
            "url": "chrome://inspect/#remote-debugging"
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
    fn stop_best_effort_preserves_unverifiable_legacy_ownership() {
        let config = test_config("cleanup");
        let paths = config.paths();
        fs::write(&paths.pid, "not-a-pid").unwrap();
        fs::write(&paths.sock, "").unwrap();

        stop_best_effort(&config).unwrap();

        assert!(paths.pid.exists());
        assert!(paths.sock.exists());
        let _ = fs::remove_file(paths.pid);
        let _ = fs::remove_file(paths.sock);
    }

    #[test]
    fn png_dimensions_decode_base64_header() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAADCAQAAADY5+WAAAAAA0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
        assert_eq!(png_dimensions_from_base64(png).unwrap(), (2, 3));
    }

    #[test]
    fn screenshot_shrink_resizes_png_within_max_dim() {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 2));
        let mut output = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        let encoded = encode_base64_standard(&output.into_inner());

        let resized = shrink_png_data_url(&encoded, 2).unwrap();

        assert_eq!(png_dimensions_from_base64(&resized).unwrap(), (2, 1));
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
            (65, "KeyA".to_string(), Some("a".to_string()))
        );
        assert_eq!(
            key_fields("?"),
            (191, "Slash".to_string(), Some("?".to_string()))
        );
        assert_eq!(
            key_fields("界"),
            (0, "".to_string(), Some("界".to_string()))
        );
        assert_eq!(key_fields("Escape"), (27, "Escape".to_string(), None));
        assert_eq!(
            key_fields("UnknownKey"),
            (0, "UnknownKey".to_string(), None)
        );
    }

    #[test]
    fn press_key_adds_shift_for_shifted_printable_characters() {
        assert_eq!(press_key_effective_modifiers("A", 0), 8);
        assert_eq!(press_key_effective_modifiers("?", 0), 8);
        assert_eq!(press_key_effective_modifiers("a", 0) & 8, 0);
        assert_eq!(press_key_effective_modifiers("A", 2) & 8, 0);
        assert_eq!(press_key_effective_modifiers("1", 0) & 8, 0);
    }

    #[test]
    fn cdp_endpoint_logging_removes_credentials_and_paths() {
        assert_eq!(
            redact_cdp_endpoint("wss://user:secret@example.com/session/token?x=1"),
            "wss://example.com/<redacted>"
        );
    }
}
