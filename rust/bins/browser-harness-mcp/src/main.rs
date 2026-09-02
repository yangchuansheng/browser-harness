use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2025-06-18";
const TOOLS: &[(&str, &str, &str, &str)] = &[
    (
        "browser_current_tab",
        "current-tab",
        "Return the attached tab.",
        "{}",
    ),
    ("browser_list_tabs", "list-tabs", "List browser tabs.", "{}"),
    (
        "browser_new_tab",
        "new-tab",
        "Open or reuse a tab.",
        r#"{"type":"object","properties":{"url":{"type":"string"}}}"#,
    ),
    (
        "browser_close_tab",
        "close-tab",
        "Close a tab.",
        r#"{"type":"object","properties":{"target_id":{"type":"string"}}}"#,
    ),
    (
        "browser_switch_tab",
        "switch-tab",
        "Attach to a tab.",
        r#"{"type":"object","properties":{"target_id":{"type":"string"},"activate":{"type":"boolean"}},"required":["target_id"]}"#,
    ),
    (
        "browser_page_info",
        "page-info",
        "Return page metadata.",
        "{}",
    ),
    (
        "browser_goto",
        "goto",
        "Navigate the attached tab.",
        r#"{"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}"#,
    ),
    (
        "browser_js",
        "js",
        "Evaluate JavaScript.",
        r#"{"type":"object","properties":{"expression":{"type":"string"}},"required":["expression"]}"#,
    ),
    (
        "browser_click",
        "click",
        "Click browser coordinates.",
        r#"{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"},"button":{"type":"string"},"clicks":{"type":"integer"}},"required":["x","y"]}"#,
    ),
    (
        "browser_type",
        "type-text",
        "Insert text into the focused element.",
        r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
    ),
    (
        "browser_fill",
        "fill-input",
        "Fill a framework-managed input.",
        r#"{"type":"object","properties":{"selector":{"type":"string"},"text":{"type":"string"},"clear_first":{"type":"boolean"},"timeout":{"type":"number"}},"required":["selector","text"]}"#,
    ),
    (
        "browser_press",
        "press-key",
        "Press a keyboard key.",
        r#"{"type":"object","properties":{"key":{"type":"string"},"modifiers":{"type":"integer"}},"required":["key"]}"#,
    ),
    (
        "browser_scroll",
        "scroll",
        "Scroll at browser coordinates.",
        r#"{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"},"dy":{"type":"number"},"dx":{"type":"number"}}}"#,
    ),
    (
        "browser_screenshot",
        "screenshot",
        "Capture a PNG as base64.",
        r#"{"type":"object","properties":{"full":{"type":"boolean"},"max_dim":{"type":"integer"}}}"#,
    ),
    (
        "browser_wait_for_load",
        "wait-for-load",
        "Wait for page load.",
        r#"{"type":"object","properties":{"timeout":{"type":"number"}}}"#,
    ),
    (
        "browser_wait_for_element",
        "wait-for-element",
        "Wait for a DOM element.",
        r#"{"type":"object","properties":{"selector":{"type":"string"},"timeout":{"type":"number"},"visible":{"type":"boolean"}},"required":["selector"]}"#,
    ),
    (
        "browser_upload_file",
        "upload-file",
        "Set file paths on an input.",
        r#"{"type":"object","properties":{"selector":{"type":"string"},"files":{"type":"array","items":{"type":"string"}}},"required":["selector","files"]}"#,
    ),
    (
        "browser_http_get",
        "http-get",
        "Fetch an HTTP URL.",
        r#"{"type":"object","properties":{"url":{"type":"string"},"timeout":{"type":"number"},"headers":{"type":"object"}},"required":["url"]}"#,
    ),
];

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let response = line
            .map_err(|error| error.to_string())
            .and_then(|line| {
                serde_json::from_str::<Value>(&line).map_err(|error| error.to_string())
            })
            .and_then(handle);
        let response = match response {
            Ok(response) => response,
            Err(error) => Some(error_response(Value::Null, -32603, &error)),
        };
        if let Some(response) = response {
            let _ = writeln!(stdout, "{}", response);
            let _ = stdout.flush();
        }
    }
}

fn handle(request: Value) -> Result<Option<Value>, String> {
    let id = request.get("id").cloned();
    let Some(id) = id else {
        return Ok(None);
    };
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "browser-harness", "version": env!("CARGO_PKG_VERSION")}
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": TOOLS.iter().map(|(name, _, description, schema)| json!({
            "name": name,
            "description": description,
            "inputSchema": serde_json::from_str::<Value>(schema).unwrap()
        })).collect::<Vec<_>>() }),
        "tools/call" => {
            let params = request
                .get("params")
                .and_then(Value::as_object)
                .ok_or("tools/call requires params")?;
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("tools/call requires name")?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            call_tool(name, arguments)
        }
        _ => return Ok(Some(error_response(id, -32601, "method not found"))),
    };
    Ok(Some(json!({"jsonrpc":"2.0","id":id,"result":result})))
}

fn call_tool(name: &str, arguments: Value) -> Value {
    let Some((_, command, _, _)) = TOOLS.iter().find(|tool| tool.0 == name) else {
        return json!({"content":[{"type":"text","text":"unknown tool"}],"isError":true});
    };
    match run_harness(command, &arguments) {
        Ok(output) => json!({"content":[{"type":"text","text":output}]}),
        Err(error) => json!({"content":[{"type":"text","text":error}],"isError":true}),
    }
}

fn run_harness(command: &str, arguments: &Value) -> Result<String, String> {
    let program =
        std::env::var("BROWSER_HARNESS_BIN").unwrap_or_else(|_| "browser-harness".to_string());
    let mut child = Command::new(program)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start browser-harness: {error}"))?;
    serde_json::to_writer(child.stdin.as_mut().unwrap(), arguments)
        .map_err(|error| format!("write tool arguments: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for browser-harness: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

#[cfg(test)]
mod tests {
    use super::{handle, TOOLS};
    use serde_json::json;

    #[test]
    fn initialize_and_tool_listing_are_valid_mcp() {
        let initialized = handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
            .unwrap()
            .unwrap();
        assert_eq!(
            initialized["result"]["serverInfo"]["name"],
            "browser-harness"
        );
        let listed = handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .unwrap()
            .unwrap();
        assert_eq!(
            listed["result"]["tools"].as_array().unwrap().len(),
            TOOLS.len()
        );
    }
}
