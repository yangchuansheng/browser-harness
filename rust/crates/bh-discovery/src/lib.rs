use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub const DEFAULT_NAME: &str = "default";
const NO_TOGGLE_GRACE: Duration = Duration::from_secs(3);
const TOGGLE_BOOT_GRACE: Duration = Duration::from_secs(12);
pub const INTERNAL_PREFIXES: &[&str] = &[
    "chrome://",
    "chrome-untrusted://",
    "devtools://",
    "chrome-extension://",
    "about:",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub name: String,
    pub sock: PathBuf,
    pub pid: PathBuf,
    pub log: PathBuf,
}

pub fn runtime_paths(name: Option<&str>) -> RuntimePaths {
    let name = validate_runtime_name(name.unwrap_or(DEFAULT_NAME)).unwrap_or(DEFAULT_NAME);
    let runtime_dir = runtime_dir();
    let tmp_dir = tmp_dir();
    let runtime_stem = if std::env::var_os("BH_RUNTIME_DIR").is_some() {
        "bu".to_string()
    } else {
        format!("bu-{name}")
    };
    let tmp_stem = if std::env::var_os("BH_TMP_DIR").is_some() {
        "bu".to_string()
    } else {
        format!("bu-{name}")
    };
    RuntimePaths {
        sock: runtime_dir.join(format!("{runtime_stem}.sock")),
        pid: runtime_dir.join(format!("{runtime_stem}.pid")),
        log: tmp_dir.join(format!("{tmp_stem}.log")),
        name: name.to_string(),
    }
}

pub fn tmp_dir() -> PathBuf {
    std::env::var_os("BH_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir())
}

pub fn config_dir() -> PathBuf {
    let path = env_path("BH_CONFIG_DIR")
        .or_else(|| env_path("BH_HOME"))
        .or_else(|| env_path("BROWSER_HARNESS_HOME"))
        .or_else(|| env_path("XDG_CONFIG_HOME").map(|base| base.join("browser-harness")))
        .unwrap_or_else(|| home_dir().join(".config/browser-harness"));
    let existed = path.exists();
    if fs::create_dir_all(&path).is_ok() && !existed {
        #[cfg(unix)]
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o700));
    }
    path
}

pub fn inspect_marker() -> PathBuf {
    config_dir().join("inspect-opened")
}

pub fn runtime_dir() -> PathBuf {
    std::env::var_os("BH_RUNTIME_DIR")
        .or_else(|| std::env::var_os("BH_TMP_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn validate_runtime_name(name: &str) -> Result<&str, String> {
    let valid_len = (1..=64).contains(&name.len());
    let valid_chars = name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if valid_len && valid_chars {
        Ok(name)
    } else {
        Err(format!(
            "invalid BU_NAME {name:?}: must match [A-Za-z0-9_-]{{1,64}}"
        ))
    }
}

pub fn default_browser_profiles() -> Vec<PathBuf> {
    let home = home_dir();

    #[cfg(target_os = "macos")]
    let profiles = vec![
        home.join("Library/Application Support/Google/Chrome"),
        home.join("Library/Application Support/Google/Chrome Canary"),
        home.join("Library/Application Support/Comet"),
        home.join("Library/Application Support/Arc/User Data"),
        home.join("Library/Application Support/Dia/User Data"),
        home.join("Library/Application Support/Microsoft Edge"),
        home.join("Library/Application Support/Microsoft Edge Beta"),
        home.join("Library/Application Support/Microsoft Edge Dev"),
        home.join("Library/Application Support/Microsoft Edge Canary"),
        home.join("Library/Application Support/BraveSoftware/Brave-Browser"),
    ];

    #[cfg(target_os = "linux")]
    let profiles = vec![
        home.join(".config/google-chrome"),
        home.join(".config/chromium"),
        home.join(".config/chromium-browser"),
        home.join(".config/microsoft-edge"),
        home.join(".config/microsoft-edge-beta"),
        home.join(".config/microsoft-edge-dev"),
        home.join(".var/app/org.chromium.Chromium/config/chromium"),
        home.join(".var/app/com.google.Chrome/config/google-chrome"),
        home.join(".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser"),
        home.join(".var/app/com.microsoft.Edge/config/microsoft-edge"),
    ];

    #[cfg(target_os = "windows")]
    let profiles = windows_browser_profiles(&home);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let profiles = Vec::new();

    profiles
}

fn home_dir() -> PathBuf {
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
        .unwrap_or_default()
}

fn env_path(key: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(key).filter(|value| !value.is_empty())?;
    let path = PathBuf::from(raw);
    if path == Path::new("~") {
        return Some(home_dir());
    }
    path.strip_prefix("~")
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| home_dir().join(relative))
        .or(Some(path))
}

#[cfg(any(target_os = "windows", test))]
fn windows_browser_profiles(home: &std::path::Path) -> Vec<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData").join("Local"));
    [
        "Google/Chrome/User Data",
        "Google/Chrome SxS/User Data",
        "Google/Chrome Beta/User Data",
        "Google/Chrome Dev/User Data",
        "Chromium/User Data",
        "Microsoft/Edge/User Data",
        "Microsoft/Edge Beta/User Data",
        "Microsoft/Edge Dev/User Data",
        "Microsoft/Edge SxS/User Data",
        "BraveSoftware/Brave-Browser/User Data",
    ]
    .into_iter()
    .map(|relative| local_app_data.join(relative))
    .collect()
}

pub fn is_internal_url(url: &str) -> bool {
    INTERNAL_PREFIXES
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

pub fn devtools_port_live(base: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(base.join("DevToolsActivePort")) else {
        return false;
    };
    let Some(port) = contents
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u16>().ok())
    else {
        return false;
    };
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok()
}

pub fn remote_debugging_user_enabled() -> Option<bool> {
    remote_debugging_user_enabled_in(&default_browser_profiles())
}

pub fn remote_debugging_toggle_profiles() -> Vec<PathBuf> {
    remote_debugging_toggle_profiles_in(&default_browser_profiles())
}

fn remote_debugging_toggle_profiles_in(profiles: &[PathBuf]) -> Vec<PathBuf> {
    profiles
        .iter()
        .filter(|base| remote_debugging_toggle_value(base) == Some(true))
        .cloned()
        .collect()
}

fn remote_debugging_toggle_value(base: &Path) -> Option<bool> {
    let contents = fs::read_to_string(base.join("Local State")).ok()?;
    let state = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    state
        .get("devtools")
        .and_then(serde_json::Value::as_object)
        .and_then(|devtools| devtools.get("remote_debugging"))
        .and_then(serde_json::Value::as_object)
        .and_then(|remote_debugging| remote_debugging.get("user-enabled"))
        .and_then(serde_json::Value::as_bool)
}

fn remote_debugging_user_enabled_in(profiles: &[PathBuf]) -> Option<bool> {
    let mut seen = None;
    for base in profiles {
        match remote_debugging_toggle_value(base) {
            Some(true) if devtools_port_live(base) => return Some(true),
            Some(false) => seen = Some(false),
            _ => {}
        }
    }
    seen
}

#[cfg(unix)]
pub fn browser_running_for_profile(base: &Path) -> bool {
    let Ok(target) = fs::read_link(base.join("SingletonLock")) else {
        return false;
    };
    let Some(pid) = target
        .to_string_lossy()
        .rsplit('-')
        .next()
        .and_then(|value| value.parse::<libc::pid_t>().ok())
        .filter(|pid| *pid > 0)
    else {
        return false;
    };
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
pub fn browser_running_for_profile(_base: &Path) -> bool {
    false
}

pub fn supported_browser_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        let Ok(output) = Command::new("tasklist").output() else {
            return true;
        };
        let processes = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        return [
            "chrome.exe",
            "msedge.exe",
            "chromium.exe",
            "brave.exe",
            "helium.exe",
        ]
        .iter()
        .any(|name| processes.contains(name));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        return default_browser_profiles()
            .iter()
            .any(|base| browser_running_for_profile(base));
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

pub fn get_ws_url() -> Result<String, String> {
    if let Ok(url) = std::env::var("BU_CDP_WS") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if let Ok(url) = std::env::var("BU_CDP_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return ws_from_cdp_url(trimmed, Duration::from_secs(30));
        }
    }

    let profiles = default_browser_profiles();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(30);
    let mut next_liveness_check = started;
    while Instant::now() < deadline {
        for base in &profiles {
            let Some((port, ws_path)) = read_devtools_active_port(base) else {
                continue;
            };
            match ws_from_json_version("127.0.0.1", port, Duration::from_secs(1)) {
                Ok(url) => return Ok(url),
                Err(err) if is_permission_blocked_error(&err) => {
                    return Err(
                        "permission-blocked: Chrome is reachable, but the per-session Allow remote debugging popup has not been accepted"
                            .to_string(),
                    );
                }
                Err(err) if err.contains("HTTP 404") && !ws_path.is_empty() => {
                    return Ok(format!("ws://127.0.0.1:{port}{ws_path}"));
                }
                Err(_) => {}
            }
        }

        let now = Instant::now();
        if now >= next_liveness_check {
            if !supported_browser_running() {
                return Err(
                    "chrome-not-running: no supported Chromium-family browser is running -- start Chrome, then retry"
                        .to_string(),
                );
            }
            next_liveness_check = now + Duration::from_secs(2);
        }

        let grace = if remote_debugging_toggle_profiles_in(&profiles).is_empty() {
            NO_TOGGLE_GRACE
        } else {
            TOGGLE_BOOT_GRACE
        };
        if now.duration_since(started) > grace {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }

    for probe_port in [9222, 9223] {
        match ws_from_json_version("127.0.0.1", probe_port, Duration::from_secs(1)) {
            Ok(url) => return Ok(url),
            Err(err) if is_permission_blocked_error(&err) => {
                return Err(
                    "permission-blocked: Chrome is reachable, but the per-session Allow remote debugging popup has not been accepted"
                        .to_string(),
                );
            }
            Err(_) => {}
        }
    }

    if remote_debugging_user_enabled_in(&profiles) == Some(false) {
        return Err("remote debugging is turned off for this browser instance -- enable chrome://inspect/#remote-debugging (tick \"Allow remote debugging for this browser instance\")".to_string());
    }

    let searched = profiles
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    Err(format!(
        "DevToolsActivePort not found in {:?} — enable chrome://inspect/#remote-debugging, or set BU_CDP_WS for a remote browser",
        searched
    ))
}

fn read_devtools_active_port(base: &PathBuf) -> Option<(u16, String)> {
    let contents = fs::read_to_string(base.join("DevToolsActivePort")).ok()?;
    let mut lines = contents.lines();
    let port = lines.next()?.trim().parse::<u16>().ok()?;
    let ws_path = lines.next().unwrap_or_default().trim().to_string();
    Some((port, ws_path))
}

fn ws_from_cdp_url(url: &str, timeout_duration: Duration) -> Result<String, String> {
    let (host, port) = parse_http_endpoint(url)?;
    let deadline = Instant::now() + timeout_duration;
    loop {
        let last_err = match ws_from_json_version_url(url, Duration::from_secs(5)) {
            Ok(url) => return Ok(url),
            Err(err) if is_permission_blocked_error(&err) => {
                return Err(
                    "permission-blocked: Chrome is reachable, but the per-session Allow remote debugging popup has not been accepted"
                        .to_string(),
                );
            }
            Err(err) if err.contains("HTTP 404") => {
                if let Some(ws_url) = ws_from_devtools_active_port(&host, port) {
                    return Ok(ws_url);
                }
                err
            }
            Err(err) => err,
        };
        if Instant::now() >= deadline {
            return Err(format!(
                "BU_CDP_URL={url} unreachable after {}s: {last_err} -- is the dedicated automation Chrome running? {}",
                timeout_duration.as_secs(),
                cdp_url_launch_hint()
            ));
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn is_permission_blocked_error(error: &str) -> bool {
    error.contains("HTTP 403") || error.contains(" 403 ")
}

fn cdp_url_launch_hint() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Launch it with --remote-debugging-port=<port> --user-data-dir=<dedicated dir>; on Windows also check that a firewall/antivirus is not blocking localhost connections"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "Launch it with --remote-debugging-port=<port> --user-data-dir=<dedicated dir>"
    }
}

fn parse_http_endpoint(url: &str) -> Result<(String, u16), String> {
    let trimmed = url.trim().trim_end_matches('/');
    let (scheme, without_scheme) = if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else {
        return Err(format!(
            "BU_CDP_URL must start with http:// or https://: {url}"
        ));
    };
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or_else(|| format!("invalid IPv6 BU_CDP_URL host: {url}"))?;
        let port = if tail.is_empty() {
            default_port(scheme)
        } else {
            tail.strip_prefix(':')
                .ok_or_else(|| format!("BU_CDP_URL invalid IPv6 port separator: {url}"))?
                .parse::<u16>()
                .map_err(|err| format!("BU_CDP_URL invalid port: {err}"))?
        };
        return Ok((host.to_string(), port));
    }
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port
            .parse::<u16>()
            .map_err(|err| format!("BU_CDP_URL invalid port: {err}"))?;
        (host, port)
    } else {
        (authority, default_port(scheme))
    };
    Ok((host.to_string(), port))
}

fn default_port(scheme: &str) -> u16 {
    if scheme == "https" {
        443
    } else {
        80
    }
}

fn ws_from_json_version_url(url: &str, timeout_duration: Duration) -> Result<String, String> {
    let version_url = format!("{}/json/version", url.trim().trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout_duration)
        .build()
        .map_err(|err| format!("build HTTP client: {err}"))?;
    let response = client
        .get(&version_url)
        .send()
        .map_err(|err| format!("GET {version_url}: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {} from /json/version", status.as_u16()));
    }
    let value: serde_json::Value = response
        .json()
        .map_err(|err| format!("parse /json/version JSON: {err}"))?;
    value
        .get("webSocketDebuggerUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "/json/version missing webSocketDebuggerUrl".to_string())
}

fn ws_from_json_version(
    host: &str,
    port: u16,
    timeout_duration: Duration,
) -> Result<String, String> {
    let host_for_url = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let mut stream = connect_host_port(host, port, timeout_duration)?;
    stream
        .set_read_timeout(Some(timeout_duration))
        .map_err(|err| format!("set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(timeout_duration))
        .map_err(|err| format!("set write timeout: {err}"))?;
    let request = format!(
        "GET /json/version HTTP/1.1\r\nHost: {host_for_url}:{port}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("write /json/version request: {err}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("read /json/version response: {err}"))?;
    let status = response.lines().next().unwrap_or_default().to_string();
    if !status.contains(" 200 ") {
        return Err(if status.contains(" 404 ") {
            "HTTP 404 from /json/version".to_string()
        } else {
            format!("unexpected /json/version status: {status}")
        });
    }
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| "missing /json/version body".to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|err| format!("parse /json/version JSON: {err}"))?;
    value
        .get("webSocketDebuggerUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "/json/version missing webSocketDebuggerUrl".to_string())
}

fn connect_host_port(
    host: &str,
    port: u16,
    timeout_duration: Duration,
) -> Result<TcpStream, String> {
    if host == "127.0.0.1" || host == "localhost" {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        TcpStream::connect_timeout(&addr, timeout_duration)
            .map_err(|err| format!("connect {host}:{port}: {err}"))
    } else {
        TcpStream::connect((host, port)).map_err(|err| format!("connect {host}:{port}: {err}"))
    }
}

fn ws_from_devtools_active_port(host: &str, port: u16) -> Option<String> {
    let host_for_ws = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    default_browser_profiles().into_iter().find_map(|base| {
        let (candidate_port, ws_path) = read_devtools_active_port(&base)?;
        (candidate_port == port && !ws_path.is_empty())
            .then(|| format!("ws://{host_for_ws}:{port}{ws_path}"))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        browser_running_for_profile, cdp_url_launch_hint, config_dir, devtools_port_live,
        get_ws_url, inspect_marker, is_internal_url, is_permission_blocked_error,
        parse_http_endpoint, remote_debugging_toggle_profiles_in, remote_debugging_user_enabled_in,
        runtime_paths, validate_runtime_name, windows_browser_profiles,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_profile(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "browser-harness-discovery-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn runtime_paths_use_requested_name() {
        let paths = runtime_paths(Some("work"));
        assert_eq!(paths.name, "work");
        assert_eq!(paths.sock.to_string_lossy(), "/tmp/bu-work.sock");
        assert_eq!(paths.pid.to_string_lossy(), "/tmp/bu-work.pid");
        assert_eq!(
            paths.log.to_string_lossy(),
            std::env::temp_dir().join("bu-work.log").to_string_lossy()
        );
    }

    #[test]
    fn validates_runtime_names() {
        assert_eq!(validate_runtime_name("work-1_ok"), Ok("work-1_ok"));
        assert!(validate_runtime_name("../bad").is_err());
        assert!(validate_runtime_name("").is_err());
    }

    #[test]
    fn internal_url_detection_matches_known_prefixes() {
        assert!(is_internal_url("chrome://settings"));
        assert!(is_internal_url("about:blank"));
        assert!(!is_internal_url("https://example.com"));
    }

    #[test]
    fn devtools_port_live_requires_a_listening_port() {
        let profile = temp_profile("port-live");
        fs::write(
            profile.join("DevToolsActivePort"),
            "1\n/devtools/browser/test\n",
        )
        .unwrap();
        assert!(!devtools_port_live(&profile));

        let listener = match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                fs::remove_dir_all(profile).unwrap();
                return;
            }
            Err(error) => panic!("bind loopback fixture: {error}"),
        };
        let port = listener.local_addr().unwrap().port();
        fs::write(
            profile.join("DevToolsActivePort"),
            format!("{port}\n/devtools/browser/test\n"),
        )
        .unwrap();
        assert!(devtools_port_live(&profile));

        drop(listener);
        fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn remote_debugging_status_requires_a_live_enabled_profile() {
        let profile = temp_profile("remote-debugging");
        fs::write(
            profile.join("Local State"),
            r#"{"devtools":{"remote_debugging":{"user-enabled":false}}}"#,
        )
        .unwrap();
        assert_eq!(
            remote_debugging_user_enabled_in(std::slice::from_ref(&profile)),
            Some(false)
        );

        fs::write(
            profile.join("Local State"),
            r#"{"devtools":{"remote_debugging":{"user-enabled":true}}}"#,
        )
        .unwrap();
        fs::write(profile.join("DevToolsActivePort"), "1\n").unwrap();
        assert_eq!(
            remote_debugging_user_enabled_in(std::slice::from_ref(&profile)),
            None
        );

        let listener = match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => listener,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                fs::remove_dir_all(profile).unwrap();
                return;
            }
            Err(error) => panic!("bind loopback fixture: {error}"),
        };
        fs::write(
            profile.join("DevToolsActivePort"),
            format!("{}\n", listener.local_addr().unwrap().port()),
        )
        .unwrap();
        assert_eq!(
            remote_debugging_user_enabled_in(std::slice::from_ref(&profile)),
            Some(true)
        );

        drop(listener);
        fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn remote_debugging_toggle_profiles_returns_only_enabled_profiles() {
        let enabled = temp_profile("toggle-enabled");
        let disabled = temp_profile("toggle-disabled");
        fs::write(
            enabled.join("Local State"),
            r#"{"devtools":{"remote_debugging":{"user-enabled":true}}}"#,
        )
        .unwrap();
        fs::write(
            disabled.join("Local State"),
            r#"{"devtools":{"remote_debugging":{"user-enabled":false}}}"#,
        )
        .unwrap();

        let profiles = remote_debugging_toggle_profiles_in(&[enabled.clone(), disabled.clone()]);

        assert_eq!(profiles, vec![enabled.clone()]);
        fs::remove_dir_all(enabled).unwrap();
        fs::remove_dir_all(disabled).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn browser_running_for_profile_reads_singleton_lock_pid() {
        use std::os::unix::fs::symlink;

        let profile = temp_profile("singleton-lock");
        symlink(
            format!("host-{}", std::process::id()),
            profile.join("SingletonLock"),
        )
        .unwrap();

        assert!(browser_running_for_profile(&profile));

        fs::remove_dir_all(profile).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn browser_running_for_profile_rejects_a_stale_pid() {
        use std::os::unix::fs::symlink;

        let profile = temp_profile("stale-singleton-lock");
        symlink("host-2147483647", profile.join("SingletonLock")).unwrap();

        assert!(!browser_running_for_profile(&profile));

        fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn inspect_marker_uses_config_dir_override() {
        let _guard = env_lock().lock().unwrap();
        let config_dir = temp_profile("config-dir");
        let previous = std::env::var_os("BH_CONFIG_DIR");
        std::env::set_var("BH_CONFIG_DIR", &config_dir);

        let marker = inspect_marker();

        if let Some(previous) = previous {
            std::env::set_var("BH_CONFIG_DIR", previous);
        } else {
            std::env::remove_var("BH_CONFIG_DIR");
        }

        assert_eq!(marker, config_dir.join("inspect-opened"));
        fs::remove_dir_all(config_dir).unwrap();
    }

    #[test]
    fn config_dir_honors_environment_priority() {
        let _guard = env_lock().lock().unwrap();
        let root = temp_profile("config-priority");
        let keys = [
            "BH_CONFIG_DIR",
            "BH_HOME",
            "BROWSER_HARNESS_HOME",
            "XDG_CONFIG_HOME",
            "HOME",
        ];
        let previous = keys.map(|key| (key, std::env::var_os(key)));

        std::env::set_var("HOME", root.join("home"));
        std::env::set_var("XDG_CONFIG_HOME", root.join("xdg"));
        std::env::set_var("BROWSER_HARNESS_HOME", root.join("legacy-home"));
        std::env::set_var("BH_HOME", root.join("bh-home"));
        std::env::set_var("BH_CONFIG_DIR", root.join("explicit"));
        assert_eq!(config_dir(), root.join("explicit"));

        std::env::remove_var("BH_CONFIG_DIR");
        assert_eq!(config_dir(), root.join("bh-home"));
        std::env::remove_var("BH_HOME");
        assert_eq!(config_dir(), root.join("legacy-home"));
        std::env::remove_var("BROWSER_HARNESS_HOME");
        assert_eq!(config_dir(), root.join("xdg/browser-harness"));
        std::env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(config_dir(), root.join("home/.config/browser-harness"));

        for (key, value) in previous {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn config_dir_creates_private_directory() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = env_lock().lock().unwrap();
        let root = temp_profile("private-config");
        let path = root.join("config");
        let previous = std::env::var_os("BH_CONFIG_DIR");
        std::env::set_var("BH_CONFIG_DIR", &path);

        assert_eq!(config_dir(), path);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );

        if let Some(previous) = previous {
            std::env::set_var("BH_CONFIG_DIR", previous);
        } else {
            std::env::remove_var("BH_CONFIG_DIR");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn permission_blocked_error_detects_http_403_variants() {
        assert!(is_permission_blocked_error("HTTP 403 from /json/version"));
        assert!(is_permission_blocked_error(
            "unexpected /json/version status: HTTP/1.1 403 Forbidden"
        ));
        assert!(!is_permission_blocked_error("HTTP 404 from /json/version"));
    }

    #[test]
    fn parses_http_and_https_cdp_endpoints() {
        assert_eq!(
            parse_http_endpoint("http://127.0.0.1:9222").unwrap(),
            ("127.0.0.1".to_string(), 9222)
        );
        assert_eq!(
            parse_http_endpoint("https://cloud.example.test/devtools").unwrap(),
            ("cloud.example.test".to_string(), 443)
        );
    }

    #[test]
    fn get_ws_url_prefers_env_override() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("BU_CDP_WS");
        std::env::set_var("BU_CDP_WS", "wss://example.test/devtools/browser/abc");

        let result = get_ws_url();

        if let Some(previous) = previous {
            std::env::set_var("BU_CDP_WS", previous);
        } else {
            std::env::remove_var("BU_CDP_WS");
        }

        assert_eq!(
            result.unwrap(),
            "wss://example.test/devtools/browser/abc".to_string()
        );
    }

    #[test]
    fn windows_profiles_include_chrome_beta_and_dev_channels() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var_os("LOCALAPPDATA");
        std::env::remove_var("LOCALAPPDATA");

        let profiles = windows_browser_profiles(Path::new("C:/Users/test"));

        if let Some(previous) = previous {
            std::env::set_var("LOCALAPPDATA", previous);
        }

        let rendered = profiles
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(rendered
            .iter()
            .any(|path| path.ends_with("AppData/Local/Google/Chrome Beta/User Data")));
        assert!(rendered
            .iter()
            .any(|path| path.ends_with("AppData/Local/Google/Chrome Dev/User Data")));
    }

    #[test]
    fn cdp_url_hint_includes_remote_debugging_launch_flags() {
        assert!(cdp_url_launch_hint().contains("--remote-debugging-port=<port>"));
        assert!(cdp_url_launch_hint().contains("--user-data-dir=<dedicated dir>"));
    }
}
