use serde_json::json;
use serde_json::Value;

pub const DEFAULT_API_BASE: &str = "https://api.browser-use.com/api/v3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUseClient {
    api_key: String,
    api_base: String,
}

impl BrowserUseClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_base: DEFAULT_API_BASE.to_string(),
        }
    }

    pub fn with_api_base(api_key: impl Into<String>, api_base: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_base: api_base.into(),
        }
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub fn api_key_present(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn request_json(
        &self,
        method: reqwest::Method,
        path_or_url: &str,
        body: Option<&Value>,
        absolute_url: bool,
    ) -> Result<Value, String> {
        let url = if absolute_url {
            path_or_url.to_string()
        } else {
            format!("{}{}", self.api_base, path_or_url)
        };
        let client = reqwest::Client::new();
        let mut request = client
            .request(method, url)
            .header("X-Browser-Use-API-Key", &self.api_key);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|err| format!("Browser Use request failed: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Browser Use request failed: {err}"))?;
        response
            .json::<Value>()
            .await
            .map_err(|err| format!("decode Browser Use response: {err}"))
    }

    fn stop_browser_url(&self, browser_id: &str) -> String {
        format!("{}/browsers/{}", self.api_base, browser_id)
    }

    fn list_browsers_path(&self, page_size: usize, page_number: usize) -> String {
        format!("/browsers?pageSize={page_size}&pageNumber={page_number}")
    }

    fn stop_browser_payload(&self) -> serde_json::Value {
        json!({ "action": "stop" })
    }

    pub async fn stop_browser(&self, browser_id: &str) -> Result<(), String> {
        let url = self.stop_browser_url(browser_id);
        self.request_json(
            reqwest::Method::PATCH,
            &url,
            Some(&self.stop_browser_payload()),
            true,
        )
        .await?;
        Ok(())
    }

    pub async fn create_browser(&self, body: &Value) -> Result<Value, String> {
        self.request_json(reqwest::Method::POST, "/browsers", Some(body), false)
            .await
    }

    pub async fn list_browsers(
        &self,
        page_size: usize,
        page_number: usize,
    ) -> Result<Value, String> {
        self.request_json(
            reqwest::Method::GET,
            &self.list_browsers_path(page_size, page_number),
            None,
            false,
        )
        .await
    }

    pub async fn cdp_ws_from_url(&self, cdp_url: &str) -> Result<String, String> {
        let value = self
            .request_json(
                reqwest::Method::GET,
                &format!("{cdp_url}/json/version"),
                None,
                true,
            )
            .await?;
        value
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "Browser Use /json/version missing webSocketDebuggerUrl".to_string())
    }

    pub async fn list_cloud_profiles(&self) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        let mut page = 1usize;
        loop {
            let listing = self
                .request_json(
                    reqwest::Method::GET,
                    &format!("/profiles?pageSize=100&pageNumber={page}"),
                    None,
                    false,
                )
                .await?;
            let items = listing
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                break;
            }
            for item in items {
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "profile listing item missing id".to_string())?;
                let detail = self
                    .request_json(
                        reqwest::Method::GET,
                        &format!("/profiles/{id}"),
                        None,
                        false,
                    )
                    .await?;
                out.push(json!({
                    "id": detail.get("id").cloned().unwrap_or(Value::Null),
                    "name": detail.get("name").cloned().unwrap_or(Value::Null),
                    "userId": detail.get("userId").cloned().unwrap_or(Value::Null),
                    "cookieDomains": detail.get("cookieDomains").cloned().unwrap_or_else(|| json!([])),
                    "lastUsedAt": detail.get("lastUsedAt").cloned().unwrap_or(Value::Null),
                }));
            }
            let total_items = listing
                .get("totalItems")
                .and_then(Value::as_u64)
                .unwrap_or(out.len() as u64);
            if out.len() as u64 >= total_items {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    pub async fn resolve_profile_name(&self, profile_name: &str) -> Result<String, String> {
        let profiles = self.list_cloud_profiles().await?;
        resolve_profile_name_in_profiles(&profiles, profile_name)
    }
}

pub fn resolve_profile_name_in_profiles(
    profiles: &[Value],
    profile_name: &str,
) -> Result<String, String> {
    let matches = profiles
        .iter()
        .filter(|profile| profile.get("name").and_then(Value::as_str) == Some(profile_name))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(format!(
            "no cloud profile named {profile_name:?} -- call list_cloud_profiles() or sync_local_profile() first"
        ));
    }
    if matches.len() > 1 {
        return Err(format!(
            "{} cloud profiles named {profile_name:?} -- pass profileId=<uuid> instead",
            matches.len()
        ));
    }
    matches[0]
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "matched cloud profile missing id".to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{resolve_profile_name_in_profiles, BrowserUseClient};

    #[test]
    fn stop_browser_request_uses_expected_url_and_payload() {
        let client = BrowserUseClient::with_api_base("test-key", "https://api.example.test");
        assert_eq!(
            client.stop_browser_url("test-browser"),
            "https://api.example.test/browsers/test-browser"
        );
        assert_eq!(client.stop_browser_payload(), json!({ "action": "stop" }));
    }

    #[test]
    fn create_browser_request_uses_expected_payload_shape() {
        let payload = json!({
            "proxyCountryCode": "us",
            "timeout": 1
        });
        assert_eq!(payload["proxyCountryCode"], "us");
        assert_eq!(payload["timeout"], 1);
    }

    #[test]
    fn list_browsers_request_uses_expected_query_params() {
        let client = BrowserUseClient::with_api_base("test-key", "https://api.example.test");
        assert_eq!(
            client.list_browsers_path(20, 3),
            "/browsers?pageSize=20&pageNumber=3"
        );
    }

    #[test]
    fn resolve_profile_name_requires_exact_single_match() {
        let profiles = vec![
            json!({"id": "a", "name": "work"}),
            json!({"id": "b", "name": "personal"}),
        ];
        assert_eq!(
            resolve_profile_name_in_profiles(&profiles, "work").unwrap(),
            "a".to_string()
        );
        assert!(resolve_profile_name_in_profiles(&profiles, "missing")
            .unwrap_err()
            .contains("no cloud profile named"));
    }

    #[test]
    fn resolve_profile_name_rejects_duplicates() {
        let profiles = vec![
            json!({"id": "a", "name": "dup"}),
            json!({"id": "b", "name": "dup"}),
        ];
        assert!(resolve_profile_name_in_profiles(&profiles, "dup")
            .unwrap_err()
            .contains("cloud profiles named"));
    }
}
