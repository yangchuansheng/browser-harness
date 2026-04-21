use std::fs;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_NAME: &str = "default";
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
    let name = name.unwrap_or(DEFAULT_NAME).to_string();
    RuntimePaths {
        sock: PathBuf::from(format!("/tmp/bu-{name}.sock")),
        pid: PathBuf::from(format!("/tmp/bu-{name}.pid")),
        log: PathBuf::from(format!("/tmp/bu-{name}.log")),
        name,
    }
}

pub fn default_browser_profiles() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    vec![
        home.join("Library/Application Support/Google/Chrome"),
        home.join("Library/Application Support/Microsoft Edge"),
        home.join("Library/Application Support/Microsoft Edge Beta"),
        home.join("Library/Application Support/Microsoft Edge Dev"),
        home.join("Library/Application Support/Microsoft Edge Canary"),
        home.join(".config/google-chrome"),
        home.join(".config/chromium"),
        home.join(".config/chromium-browser"),
        home.join(".config/microsoft-edge"),
        home.join(".config/microsoft-edge-beta"),
        home.join(".config/microsoft-edge-dev"),
        home.join("AppData/Local/Google/Chrome/User Data"),
        home.join("AppData/Local/Chromium/User Data"),
        home.join("AppData/Local/Microsoft/Edge/User Data"),
        home.join("AppData/Local/Microsoft/Edge Beta/User Data"),
        home.join("AppData/Local/Microsoft/Edge Dev/User Data"),
        home.join("AppData/Local/Microsoft/Edge SxS/User Data"),
    ]
}

pub fn is_internal_url(url: &str) -> bool {
    INTERNAL_PREFIXES
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

pub fn get_ws_url() -> Result<String, String> {
    if let Ok(url) = std::env::var("BU_CDP_WS") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let profiles = default_browser_profiles();
    for base in &profiles {
        let devtools_port = base.join("DevToolsActivePort");
        let Ok(contents) = fs::read_to_string(&devtools_port) else {
            continue;
        };
        let mut lines = contents.lines();
        let Some(port_line) = lines.next() else {
            continue;
        };
        let Some(path_line) = lines.next() else {
            continue;
        };
        let Ok(port) = port_line.trim().parse::<u16>() else {
            continue;
        };

        let deadline = Instant::now() + Duration::from_secs(30);
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        loop {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
                Ok(stream) => {
                    drop(stream);
                    return Ok(format!("ws://127.0.0.1:{}{}", port, path_line.trim()));
                }
                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_secs(1));
                }
                Err(_) => {
                    return Err(format!(
                        "Chrome's remote-debugging page is open, but DevTools is not live yet on 127.0.0.1:{} — if Chrome opened a profile picker, choose your normal profile first, then tick the checkbox and click Allow if shown",
                        port
                    ));
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::{get_ws_url, is_internal_url, runtime_paths};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn runtime_paths_use_requested_name() {
        let paths = runtime_paths(Some("work"));
        assert_eq!(paths.name, "work");
        assert_eq!(paths.sock.to_string_lossy(), "/tmp/bu-work.sock");
        assert_eq!(paths.pid.to_string_lossy(), "/tmp/bu-work.pid");
        assert_eq!(paths.log.to_string_lossy(), "/tmp/bu-work.log");
    }

    #[test]
    fn internal_url_detection_matches_known_prefixes() {
        assert!(is_internal_url("chrome://settings"));
        assert!(is_internal_url("about:blank"));
        assert!(!is_internal_url("https://example.com"));
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
}
