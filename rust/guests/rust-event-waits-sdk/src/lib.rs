use bh_guest_sdk::{
    current_session, handle_dialog, js, wait_for_console, wait_for_dialog, wait_for_event,
    watch_events, EventFilter, WatchEventsLine,
};
use serde_json::{json, Value};

const WAIT_EVENT_TOKEN: &str = "bhrun-event-wait";
const WATCH_TOKEN_ONE: &str = "bhrun-event-watch-1";
const WATCH_TOKEN_TWO: &str = "bhrun-event-watch-2";
const CONSOLE_TOKEN: &str = "bhrun-event-console";
const DIALOG_TOKEN: &str = "bhrun-event-dialog";

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

    js::<Value>(&format!(
        "setTimeout(() => console.log({}), 50); null",
        json!(WAIT_EVENT_TOKEN)
    ))
    .map_err(|_| 3)?;
    let event_wait = wait_for_event(
        EventFilter {
            method: Some("Runtime.consoleAPICalled".to_string()),
            session_id: Some(session_id.clone()),
            params_subset: Some(json!({
                "type":"log",
                "args":[{"type":"string","value":WAIT_EVENT_TOKEN}]
            })),
        },
        5_000,
        100,
    )
    .map_err(|_| 4)?;
    if !event_wait.matched {
        return Err(5);
    }
    let wait_event = event_wait.event.ok_or(6)?;
    if wait_event.get("method").and_then(Value::as_str) != Some("Runtime.consoleAPICalled") {
        return Err(7);
    }
    if wait_event.get("session_id").and_then(Value::as_str) != Some(session_id.as_str()) {
        return Err(8);
    }
    let wait_arg = wait_event
        .pointer("/params/args/0/value")
        .or_else(|| wait_event.pointer("/params/args/0/description"))
        .and_then(Value::as_str);
    if wait_arg != Some(WAIT_EVENT_TOKEN) {
        return Err(9);
    }

    js::<Value>(&format!(
        "console.log({}); console.log({}); null",
        json!(WATCH_TOKEN_ONE),
        json!(WATCH_TOKEN_TWO)
    ))
    .map_err(|_| 10)?;

    let lines = watch_events(
        EventFilter {
            method: Some("Runtime.consoleAPICalled".to_string()),
            session_id: Some(session_id.clone()),
            params_subset: Some(json!({"type":"log"})),
        },
        2_000,
        100,
        Some(2),
    )
    .map_err(|_| 11)?;
    if lines.len() != 3 {
        return Err(12);
    }
    match &lines[0] {
        WatchEventsLine::Event { event, index, .. } => {
            if *index != 1 {
                return Err(13);
            }
            if event.get("method").and_then(Value::as_str) != Some("Runtime.consoleAPICalled") {
                return Err(14);
            }
        }
        _ => return Err(15),
    }
    match &lines[1] {
        WatchEventsLine::Event { event, index, .. } => {
            if *index != 2 {
                return Err(16);
            }
            if event.get("method").and_then(Value::as_str) != Some("Runtime.consoleAPICalled") {
                return Err(17);
            }
        }
        _ => return Err(18),
    }
    match &lines[2] {
        WatchEventsLine::End {
            matched_events,
            reached_max_events,
            timed_out,
            ..
        } => {
            if *matched_events != 2 || !*reached_max_events || *timed_out {
                return Err(19);
            }
        }
        _ => return Err(20),
    }

    js::<Value>(&format!(
        "setTimeout(() => console.log({}), 50); null",
        json!(CONSOLE_TOKEN)
    ))
    .map_err(|_| 21)?;
    let console_wait =
        wait_for_console(Some("log"), Some(CONSOLE_TOKEN), Some(&session_id), 5_000, 100)
            .map_err(|_| 22)?;
    if !console_wait.matched {
        return Err(23);
    }
    let console_event = console_wait.event.ok_or(24)?;
    let console_method = console_event.get("method").and_then(Value::as_str);
    if console_method != Some("Runtime.consoleAPICalled")
        && console_method != Some("Console.messageAdded")
    {
        return Err(25);
    }

    js::<Value>(&format!(
        "setTimeout(() => alert({}), 50); null",
        json!(DIALOG_TOKEN)
    ))
    .map_err(|_| 26)?;
    let dialog_wait =
        wait_for_dialog(Some("alert"), Some(DIALOG_TOKEN), Some(&session_id), 5_000, 100)
            .map_err(|_| 27)?;
    if !dialog_wait.matched {
        return Err(28);
    }
    let dialog_event = dialog_wait.event.ok_or(29)?;
    if dialog_event.get("method").and_then(Value::as_str) != Some("Page.javascriptDialogOpening")
    {
        return Err(30);
    }
    if dialog_event
        .pointer("/params/message")
        .and_then(Value::as_str)
        != Some(DIALOG_TOKEN)
    {
        return Err(31);
    }
    handle_dialog("accept", None).map_err(|_| 32)?;

    Ok(())
}
