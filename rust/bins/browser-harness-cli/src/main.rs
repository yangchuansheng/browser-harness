use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ADMIN_COMMANDS: &[&str] = &[
    "create-browser",
    "stop-browser",
    "list-cloud-profiles",
    "resolve-profile-name",
    "list-local-profiles",
    "sync-local-profile",
    "daemon-alive",
    "ensure-daemon",
    "restart-daemon",
    "stop-daemon",
];

const RUNNER_HELP: &str = "manifest|sample-config|capabilities|summary|run-guest|serve-guest|current-tab|list-tabs|new-tab|switch-tab|ensure-real-tab|iframe-target|page-info|goto|wait-for-load|js|click|type-text|press-key|scroll|screenshot|wait|http-get|current-session|wait-for-event|watch-events|wait-for-load-event|wait-for-response|wait-for-console|wait-for-dialog";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Admin,
    Runner,
}

fn main() {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run(args: Vec<OsString>) -> Result<i32, String> {
    if args.is_empty() || is_help_flag(&args[0]) {
        print_usage();
        return Ok(0);
    }

    let command = args[0].to_string_lossy().to_string();
    let route = route_command(&command);
    let mut child = spawn_child(route, &args)?;
    let status = child
        .wait()
        .map_err(|err| format!("wait for child process: {err}"))?;
    Ok(status.code().unwrap_or(1))
}

fn is_help_flag(value: &OsString) -> bool {
    matches!(value.to_str(), Some("-h" | "--help" | "help"))
}

fn route_command(command: &str) -> Route {
    if ADMIN_COMMANDS.contains(&command) {
        Route::Admin
    } else {
        Route::Runner
    }
}

fn spawn_child(route: Route, args: &[OsString]) -> Result<std::process::Child, String> {
    let child_binary = match route {
        Route::Admin => "bhctl",
        Route::Runner => "bhrun",
    };
    let env_override = match route {
        Route::Admin => std::env::var_os("BU_RUST_ADMIN_BIN"),
        Route::Runner => std::env::var_os("BU_RUST_RUNNER_BIN"),
    };

    if let Some(program) = env_override
        .map(PathBuf::from)
        .or_else(|| sibling_binary_path(child_binary))
    {
        return Command::new(program)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| format!("spawn {child_binary}: {err}"));
    }

    let workspace_root = workspace_root();
    Command::new("cargo")
        .args(["run", "--quiet", "--bin", child_binary, "--"])
        .args(args)
        .current_dir(workspace_root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| format!("spawn cargo fallback for {child_binary}: {err}"))
}

fn sibling_binary_path(name: &str) -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let sibling = current_exe.with_file_name(name);
    sibling.is_file().then_some(sibling)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist")
        .to_path_buf()
}

fn print_usage() {
    eprintln!(
        "usage: browser-harness <admin-command|runner-command>\n\
         admin commands: {}\n\
         runner commands: {}\n\
         notes:\n\
           - this is the Rust-native top-level CLI\n\
           - admin commands are forwarded to bhctl\n\
           - runner/helper commands are forwarded to bhrun",
        ADMIN_COMMANDS.join("|"),
        RUNNER_HELP
    );
}

#[cfg(test)]
mod tests {
    use super::{route_command, Route};

    #[test]
    fn routes_admin_commands_to_bhctl() {
        assert_eq!(route_command("ensure-daemon"), Route::Admin);
        assert_eq!(route_command("create-browser"), Route::Admin);
    }

    #[test]
    fn routes_other_commands_to_bhrun() {
        assert_eq!(route_command("page-info"), Route::Runner);
        assert_eq!(route_command("run-guest"), Route::Runner);
        assert_eq!(route_command("unknown-command"), Route::Runner);
    }
}
