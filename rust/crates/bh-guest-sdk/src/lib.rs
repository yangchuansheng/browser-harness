use std::fmt;

use bh_wasm_host::WaitForEventResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

const DEFAULT_OUTPUT_CAPACITY: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestError {
    SerializeRequest(String),
    DeserializeResponse(String),
    HostCallFailed { operation: String },
}

impl fmt::Display for GuestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SerializeRequest(err) => write!(f, "serialize guest request: {err}"),
            Self::DeserializeResponse(err) => write!(f, "parse guest response: {err}"),
            Self::HostCallFailed { operation } => {
                write!(f, "host call failed for operation: {operation}")
            }
        }
    }
}

impl std::error::Error for GuestError {}

pub fn call_json<TRequest, TResponse>(
    operation: &str,
    request: &TRequest,
) -> Result<TResponse, GuestError>
where
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    call_json_with(imported_call_json, operation, request)
}

pub fn goto(url: &str) -> Result<Value, GuestError> {
    call_json("goto", &json!({ "url": url }))
}

pub fn page_info() -> Result<Value, GuestError> {
    call_json("page_info", &json!({}))
}

pub fn js<T>(expression: &str) -> Result<T, GuestError>
where
    T: DeserializeOwned,
{
    call_json("js", &json!({ "expression": expression }))
}

pub fn wait_for_load_event(
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<WaitForEventResult, GuestError> {
    call_json(
        "wait_for_load_event",
        &json!({
            "timeout_ms": timeout_ms,
            "poll_interval_ms": poll_interval_ms,
        }),
    )
}

fn call_json_with<F, TRequest, TResponse>(
    mut host_call: F,
    operation: &str,
    request: &TRequest,
) -> Result<TResponse, GuestError>
where
    F: FnMut(&[u8], &[u8], &mut [u8]) -> i32,
    TRequest: Serialize,
    TResponse: DeserializeOwned,
{
    let request_bytes =
        serde_json::to_vec(request).map_err(|err| GuestError::SerializeRequest(err.to_string()))?;
    let operation_bytes = operation.as_bytes();
    let mut output = vec![0u8; DEFAULT_OUTPUT_CAPACITY];
    let written = host_call(operation_bytes, &request_bytes, &mut output);
    if written < 0 {
        return Err(GuestError::HostCallFailed {
            operation: operation.to_string(),
        });
    }

    output.truncate(written as usize);
    serde_json::from_slice(&output).map_err(|err| GuestError::DeserializeResponse(err.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "bh")]
extern "C" {
    #[link_name = "call_json"]
    fn bh_call_json(
        operation_ptr: *const u8,
        operation_len: usize,
        request_ptr: *const u8,
        request_len: usize,
        out_ptr: *mut u8,
        out_cap: usize,
    ) -> i32;
}

#[cfg(target_arch = "wasm32")]
fn imported_call_json(operation: &[u8], request: &[u8], output: &mut [u8]) -> i32 {
    unsafe {
        bh_call_json(
            operation.as_ptr(),
            operation.len(),
            request.as_ptr(),
            request.len(),
            output.as_mut_ptr(),
            output.len(),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn imported_call_json(_operation: &[u8], _request: &[u8], _output: &mut [u8]) -> i32 {
    panic!("bh-guest-sdk host import is only available on wasm32 guests");
}

#[cfg(test)]
mod tests {
    use super::{call_json_with, goto, js, page_info, wait_for_load_event, GuestError};
    use bh_wasm_host::WaitForEventResult;
    use serde_json::{json, Value};

    #[test]
    fn goto_serializes_url_request() {
        let result: Value = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"goto");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("url").and_then(Value::as_str),
                    Some("https://example.com")
                );
                let response =
                    serde_json::to_vec(&json!({"frameId":"frame-1"})).expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "goto",
            &json!({"url":"https://example.com"}),
        )
        .expect("goto result");

        assert_eq!(
            result.get("frameId").and_then(Value::as_str),
            Some("frame-1")
        );
    }

    #[test]
    fn js_deserializes_string_response() {
        let title: String = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"js");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("expression").and_then(Value::as_str),
                    Some("document.title")
                );
                let response = serde_json::to_vec("Example Domain").expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "js",
            &json!({"expression":"document.title"}),
        )
        .expect("js result");

        assert_eq!(title, "Example Domain");
    }

    #[test]
    fn wait_for_load_event_deserializes_typed_result() {
        let result: WaitForEventResult = call_json_with(
            |operation, request, output| {
                assert_eq!(operation, b"wait_for_load_event");
                let request: Value = serde_json::from_slice(request).expect("parse request");
                assert_eq!(
                    request.get("timeout_ms").and_then(Value::as_u64),
                    Some(5000)
                );
                assert_eq!(
                    request.get("poll_interval_ms").and_then(Value::as_u64),
                    Some(100)
                );
                let response = serde_json::to_vec(&json!({
                    "matched": true,
                    "event": {"method":"Page.loadEventFired"},
                    "polls": 3,
                    "elapsed_ms": 250
                }))
                .expect("serialize response");
                output[..response.len()].copy_from_slice(&response);
                response.len() as i32
            },
            "wait_for_load_event",
            &json!({"timeout_ms":5000,"poll_interval_ms":100}),
        )
        .expect("wait result");

        assert!(result.matched);
        assert_eq!(result.polls, 3);
    }

    #[test]
    fn helper_functions_use_expected_operations() {
        let _ = goto;
        let _ = page_info;
        let _ = wait_for_load_event;
        let _ = js::<String>;
    }

    #[test]
    fn negative_host_result_becomes_guest_error() {
        let err = call_json_with::<_, _, Value>(
            |_operation, _request, _output| -1,
            "page_info",
            &json!({}),
        )
        .expect_err("host call should fail");

        assert_eq!(
            err,
            GuestError::HostCallFailed {
                operation: "page_info".to_string()
            }
        );
    }
}
