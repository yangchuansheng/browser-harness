use bh_guest_sdk::{cdp_raw, current_session};
use serde_json::{json, Value};

const RAW_TOKEN: &str = "bhrun-raw-cdp-guest";

#[no_mangle]
pub extern "C" fn run() -> i32 {
    match run_inner() {
        Ok(()) => 0,
        Err(code) => code,
    }
}

fn run_inner() -> Result<(), i32> {
    let session_id = current_session()
        .map_err(|_| 1)?
        .session_id
        .ok_or(2)?;

    let response = cdp_raw(
        "Runtime.evaluate",
        Some(json!({
            "expression": format!(
                "window.__bhrunRawCdpGuest = {}; window.__bhrunRawCdpGuest",
                json!(RAW_TOKEN)
            ),
            "returnByValue": true,
            "awaitPromise": true
        })),
        Some(&session_id),
    )
    .map_err(|_| 3)?;

    if response.pointer("/result/value").and_then(Value::as_str) != Some(RAW_TOKEN) {
        return Err(4);
    }

    Ok(())
}
