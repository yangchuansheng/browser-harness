use bh_guest_sdk::{goto, js, page_info, wait_for_load_event};
use serde_json::Value;

const TARGET_URL: &str = "https://example.com/?via=bhrun-guest-sample";

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    goto(TARGET_URL).map_err(|_| 1)?;

    let load = wait_for_load_event(5_000, 100).map_err(|_| 2)?;
    if !load.matched {
        return Err(2);
    }

    let page = page_info().map_err(|_| 3)?;
    if page.get("url").and_then(Value::as_str) != Some(TARGET_URL) {
        return Err(4);
    }

    let title: String = js("document.title").map_err(|_| 5)?;
    if !title.contains("Example Domain") {
        return Err(6);
    }

    Ok(())
}
