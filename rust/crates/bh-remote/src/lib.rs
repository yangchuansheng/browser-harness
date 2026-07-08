use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use serde_json::Value;

pub const DEFAULT_API_BASE: &str = "https://api.browser-use.com/api/v3";
pub const BROWSER_USE_API_KEY_ENV: &str = "BROWSER_USE_API_KEY";

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

pub fn browser_use_api_key() -> Result<String, String> {
    if let Ok(raw) = std::env::var(BROWSER_USE_API_KEY_ENV) {
        let value = raw.trim();
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    stored_browser_use_api_key()?.ok_or_else(|| {
        "BROWSER_USE_API_KEY missing -- run `browser-harness auth login --api-key-stdin` or export BROWSER_USE_API_KEY".to_string()
    })
}

pub fn auth_status() -> Value {
    let path = auth_path();
    if let Ok(raw) = std::env::var(BROWSER_USE_API_KEY_ENV) {
        if !raw.trim().is_empty() {
            return json!({
                "status": "authenticated",
                "source": "env",
                "path": path.display().to_string(),
            });
        }
    }

    match stored_browser_use_api_key() {
        Ok(Some(_)) => json!({
            "status": "authenticated",
            "source": "stored",
            "path": path.display().to_string(),
        }),
        Ok(None) => json!({
            "status": "missing",
            "source": Value::Null,
            "path": path.display().to_string(),
        }),
        Err(err) => json!({
            "status": "error",
            "source": Value::Null,
            "path": path.display().to_string(),
            "reason": err,
        }),
    }
}

pub fn store_browser_use_api_key(raw: &str) -> Result<Value, String> {
    let api_key = normalize_api_key(raw)?;
    let path = auth_path();
    let mut data = load_auth_file(&path)?;
    let object = data
        .as_object_mut()
        .ok_or_else(|| format!("auth file root must be a JSON object: {}", path.display()))?;
    object.insert(
        "browser_use".to_string(),
        json!({
            "api_key": api_key,
            "source": "manual",
        }),
    );
    write_private_json(&path, &data)?;
    Ok(json!({
        "status": "stored",
        "path": path.display().to_string(),
    }))
}

pub fn clear_browser_use_auth() -> Result<Value, String> {
    let path = auth_path();
    let mut data = load_auth_file(&path)?;
    let Some(object) = data.as_object_mut() else {
        return Err(format!(
            "auth file root must be a JSON object: {}",
            path.display()
        ));
    };
    let removed = object.remove("browser_use").is_some();
    if object.is_empty() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(format!("remove auth file {}: {err}", path.display())),
        }
    } else {
        write_private_json(&path, &data)?;
    }
    Ok(json!({
        "status": if removed { "logged-out" } else { "missing" },
        "path": path.display().to_string(),
    }))
}

fn stored_browser_use_api_key() -> Result<Option<String>, String> {
    let path = auth_path();
    let data = load_auth_file(&path)?;
    Ok(data
        .get("browser_use")
        .and_then(Value::as_object)
        .and_then(|record| record.get("api_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string))
}

fn auth_path() -> PathBuf {
    if let Some(path) = std::env::var_os("BH_AUTH_PATH") {
        return PathBuf::from(path);
    }
    config_dir().join("auth.json")
}

fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("BH_CONFIG_DIR") {
        return PathBuf::from(path);
    }
    if let Some(path) =
        std::env::var_os("BH_HOME").or_else(|| std::env::var_os("BROWSER_HARNESS_HOME"))
    {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("browser-harness");
    }
    home_dir()
        .map(|path| path.join(".config").join("browser-harness"))
        .unwrap_or_else(|| PathBuf::from(".browser-harness"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut combined = std::ffi::OsString::from(drive);
            combined.push(path);
            Some(PathBuf::from(combined))
        })
}

fn load_auth_file(path: &Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let value = serde_json::from_str::<Value>(&raw)
                .map_err(|err| format!("auth file is not valid JSON: {}: {err}", path.display()))?;
            if value.is_object() {
                Ok(value)
            } else {
                Err(format!(
                    "auth file root must be a JSON object: {}",
                    path.display()
                ))
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(err) if err.kind() == std::io::ErrorKind::InvalidData => {
            Err(format!("auth file is not valid JSON: {}", path.display()))
        }
        Err(err) => Err(format!("read auth file {}: {err}", path.display())),
    }
}

fn write_private_json(path: &Path, data: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let existed = parent.exists();
        fs::create_dir_all(parent)
            .map_err(|err| format!("create auth directory {}: {err}", parent.display()))?;
        if !existed {
            chmod_private_dir(parent);
        }
    }

    let tmp = path.with_file_name(format!(
        "{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("auth.json")
    ));
    let mut raw = serde_json::to_vec_pretty(data)
        .map_err(|err| format!("serialize auth file {}: {err}", path.display()))?;
    raw.push(b'\n');
    fs::write(&tmp, raw).map_err(|err| format!("write auth file {}: {err}", tmp.display()))?;
    chmod_private_file(&tmp);
    fs::rename(&tmp, path).map_err(|err| {
        format!(
            "replace auth file {} with {}: {err}",
            path.display(),
            tmp.display()
        )
    })?;
    chmod_private_file(path);
    Ok(())
}

fn normalize_api_key(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("no API key provided".to_string());
    }
    if !value.starts_with("bu_") {
        return Err("Browser Use API key must start with bu_".to_string());
    }
    if value.len() < 20 {
        return Err("Browser Use API key looks too short".to_string());
    }
    Ok(value.to_string())
}

#[cfg(unix)]
fn chmod_private_dir(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn chmod_private_dir(_path: &Path) {}

#[cfg(unix)]
fn chmod_private_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn chmod_private_file(_path: &Path) {}

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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{
        load_auth_file, normalize_api_key, resolve_profile_name_in_profiles, BrowserUseClient,
    };

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

    #[test]
    fn normalize_api_key_accepts_browser_use_keys() {
        assert_eq!(
            normalize_api_key("  bu_12345678901234567890  ").unwrap(),
            "bu_12345678901234567890"
        );
    }

    #[test]
    fn normalize_api_key_rejects_short_or_wrong_prefix_values() {
        assert!(normalize_api_key("bu_short")
            .unwrap_err()
            .contains("too short"));
        assert!(normalize_api_key("sk_12345678901234567890")
            .unwrap_err()
            .contains("must start with bu_"));
    }

    #[test]
    fn load_auth_file_reports_non_utf8_as_invalid_json() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("browser-harness-auth-{stamp}.json"));
        fs::write(&path, [0xff, 0xfe]).unwrap();

        let result = load_auth_file(&path);

        let _ = fs::remove_file(&path);
        assert_eq!(
            result.unwrap_err(),
            format!("auth file is not valid JSON: {}", path.display())
        );
    }
}
