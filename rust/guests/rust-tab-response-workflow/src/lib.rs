use bh_guest_sdk::{
    current_session, current_tab, goto, js, list_tabs, new_tab, page_info, switch_tab,
    wait_for_response,
};
use serde_json::Value;

const TARGET_URL: &str = "https://example.com/?via=bhrun-tab-response-guest-smoke";

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let initial_tab = current_tab().map_err(|_| 1)?;
    let initial_tabs = list_tabs(false).map_err(|_| 2)?;

    let new_tab_result = new_tab("about:blank").map_err(|_| 4)?;
    if new_tab_result.target_id == initial_tab.target_id {
        return Err(5);
    }

    let current_after_new = current_tab().map_err(|_| 6)?;
    if current_after_new.target_id != new_tab_result.target_id {
        return Err(7);
    }

    let new_session = current_session().map_err(|_| 8)?;
    let _new_session_id = new_session.session_id.ok_or(9)?;

    let blank_href: String = js("location.href").map_err(|_| 10)?;
    if blank_href != "about:blank" {
        return Err(11);
    }

    let switch_back = switch_tab(&initial_tab.target_id).map_err(|_| 12)?;
    let session_after_switch_back = current_session().map_err(|_| 13)?;
    if session_after_switch_back.session_id.as_deref() != Some(switch_back.session_id.as_str()) {
        return Err(14);
    }

    let current_after_switch_back = current_tab().map_err(|_| 15)?;
    if current_after_switch_back.target_id != initial_tab.target_id {
        return Err(16);
    }

    let switch_forward = switch_tab(&new_tab_result.target_id).map_err(|_| 17)?;
    let session_after_switch_forward = current_session().map_err(|_| 18)?;
    let active_session_id = session_after_switch_forward.session_id.ok_or(19)?;
    if active_session_id != switch_forward.session_id {
        return Err(20);
    }

    let current_after_switch_forward = current_tab().map_err(|_| 21)?;
    if current_after_switch_forward.target_id != new_tab_result.target_id {
        return Err(22);
    }

    goto(TARGET_URL).map_err(|_| 23)?;

    let response = wait_for_response(TARGET_URL, Some(200), Some(&active_session_id), 5_000, 100)
        .map_err(|_| 24)?;
    if !response.matched {
        return Err(25);
    }
    let response_event = response.event.ok_or(26)?;
    if response_event.get("method").and_then(Value::as_str) != Some("Network.responseReceived") {
        return Err(27);
    }
    if response_event.get("session_id").and_then(Value::as_str) != Some(active_session_id.as_str())
    {
        return Err(28);
    }
    if response_event
        .get("params")
        .and_then(|params| params.get("response"))
        .and_then(|response| response.get("url"))
        .and_then(Value::as_str)
        != Some(TARGET_URL)
    {
        return Err(29);
    }
    if response_event
        .get("params")
        .and_then(|params| params.get("response"))
        .and_then(|response| response.get("status"))
        .and_then(Value::as_u64)
        != Some(200)
    {
        return Err(30);
    }

    let page = page_info().map_err(|_| 31)?;
    if page.get("url").and_then(Value::as_str) != Some(TARGET_URL) {
        return Err(32);
    }

    let href: String = js("location.href").map_err(|_| 33)?;
    if href != TARGET_URL {
        return Err(34);
    }

    let tabs_after_navigation = list_tabs(false).map_err(|_| 35)?;
    if !tabs_after_navigation
        .iter()
        .any(|tab| tab.target_id == new_tab_result.target_id)
    {
        return Err(36);
    }
    if tabs_after_navigation.len() < initial_tabs.len() + 1 {
        return Err(37);
    }

    Ok(())
}
