use bh_guest_sdk::{goto, js, page_info, wait_for_load_event};
use serde_json::Value;

const TARGET_URL: &str = "https://example.com/?via=bhrun-serve-guest-remote-smoke";
const MARKER: &str = "phase-1";

static mut INVOCATION_COUNT: u32 = 0;

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let invocation = unsafe {
        INVOCATION_COUNT += 1;
        INVOCATION_COUNT
    };

    match invocation {
        1 => run_first(),
        _ => run_later(),
    }
}

fn run_first() -> Result<(), i32> {
    goto(TARGET_URL).map_err(|_| 1)?;

    let load = wait_for_load_event(5_000, 100).map_err(|_| 2)?;
    if !load.matched {
        return Err(2);
    }

    let marker: String =
        js("window.__bhrunPersistentMarker = 'phase-1'; window.__bhrunPersistentMarker")
            .map_err(|_| 3)?;
    if marker != MARKER {
        return Err(4);
    }

    let page = page_info().map_err(|_| 5)?;
    if page.get("url").and_then(Value::as_str) != Some(TARGET_URL) {
        return Err(6);
    }

    Ok(())
}

fn run_later() -> Result<(), i32> {
    let state: String =
        js("JSON.stringify({href: location.href, marker: window.__bhrunPersistentMarker})")
            .map_err(|_| 11)?;
    let state: Value = serde_json::from_str(&state).map_err(|_| 12)?;
    if state.get("href").and_then(Value::as_str) != Some(TARGET_URL) {
        return Err(13);
    }
    if state.get("marker").and_then(Value::as_str) != Some(MARKER) {
        return Err(14);
    }

    let page = page_info().map_err(|_| 15)?;
    if page.get("url").and_then(Value::as_str) != Some(TARGET_URL) {
        return Err(16);
    }

    Ok(())
}
