use bh_daemon::{
    already_running, cleanup_runtime_files, initialize_runtime_files, log_line, serve, stop_remote,
    DaemonConfig,
};

#[tokio::main]
async fn main() {
    let name = std::env::var("BU_NAME").unwrap_or_else(|_| "default".to_string());
    let mut config = DaemonConfig::new(name);
    config.remote_browser_id = std::env::var("BU_BROWSER_ID")
        .ok()
        .filter(|value| !value.is_empty());
    config.browser_use_api_key = std::env::var("BROWSER_USE_API_KEY")
        .ok()
        .filter(|value| !value.is_empty());
    let paths = config.paths();

    if already_running(&config) {
        eprintln!("daemon already running on {}", paths.sock.display());
        return;
    }

    if let Err(err) = initialize_runtime_files(&config) {
        eprintln!("{err}");
        std::process::exit(1);
    }

    let result = serve(&config).await;
    if let Err(err) = &result {
        log_line(&config, &format!("fatal: {err}"));
    }

    match stop_remote(&config).await {
        Ok(true) => {
            if let Some(remote_browser_id) = config.remote_browser_id.as_deref() {
                log_line(
                    &config,
                    &format!("stopped remote browser {remote_browser_id}"),
                );
            }
        }
        Ok(false) => {}
        Err(err) => {
            if let Some(remote_browser_id) = config.remote_browser_id.as_deref() {
                log_line(
                    &config,
                    &format!("stop_remote failed ({remote_browser_id}): {err}"),
                );
            } else {
                log_line(&config, &format!("stop_remote failed: {err}"));
            }
        }
    }
    cleanup_runtime_files(&config);

    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
