use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

const ADMIN_COMMANDS: &[&str] = &[
    "create-browser",
    "list-browsers",
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

const INTERNAL_COMMANDS: &[&str] = &["install", "verify-install"];
const INSTALLABLE_BINARIES: &[&str] = &["browser-harness", "bhctl", "bhrun", "bhd"];

const RUNNER_HELP: &str = "manifest|sample-config|capabilities|summary|run-guest|serve-guest|current-tab|list-tabs|new-tab|switch-tab|ensure-real-tab|iframe-target|page-info|goto|wait-for-load|js|click|mouse-move|mouse-down|mouse-up|type-text|press-key|dispatch-key|scroll|set-viewport|print-pdf|screenshot|handle-dialog|upload-file|get-cookies|set-cookies|configure-downloads|wait|http-get|current-session|drain-events|cdp-raw|wait-for-event|watch-events|wait-for-load-event|wait-for-download|wait-for-request|wait-for-response|wait-for-console|wait-for-dialog";
const EXPECTED_INSTALLED_BINARIES: &[&str] = &["bhctl", "bhrun", "bhd"];
const FORBIDDEN_PYTHON_FILES: &[&str] = &[
    "run.py",
    "admin.py",
    "admin_cli.py",
    "helpers.py",
    "runner_cli.py",
    "legacy_warnings.py",
];
const FORBIDDEN_PYTHON_PACKAGES: &[&str] = &[
    "run",
    "admin",
    "admin_cli",
    "helpers",
    "runner_cli",
    "legacy_warnings",
];

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

    if matches!(args[0].to_str(), Some("install")) {
        return run_install(&args[1..]);
    }

    if matches!(args[0].to_str(), Some("verify-install")) {
        return run_verify_install(&args[1..]);
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

#[derive(Debug, Default)]
struct InstallOptions {
    root: Option<PathBuf>,
    workspace_root: Option<PathBuf>,
    debug: bool,
}

fn run_install(args: &[OsString]) -> Result<i32, String> {
    let Some(options) = parse_install_options(args)? else {
        print_install_usage();
        return Ok(0);
    };
    let report = install_report(&options);
    let success = report
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|err| format!("serialize install report: {err}"))?
    );
    Ok(if success { 0 } else { 1 })
}

fn parse_install_options(args: &[OsString]) -> Result<Option<InstallOptions>, String> {
    let mut options = InstallOptions::default();
    let mut index = 0;

    while index < args.len() {
        let value = args[index]
            .to_str()
            .ok_or_else(|| format!("install argument is not valid UTF-8: {:?}", args[index]))?;
        match value {
            "-h" | "--help" => return Ok(None),
            "--root" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| "--root requires a path".to_string())?;
                options.root = Some(PathBuf::from(next));
                index += 2;
            }
            "--workspace-root" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| "--workspace-root requires a path".to_string())?;
                options.workspace_root = Some(PathBuf::from(next));
                index += 2;
            }
            "--debug" => {
                options.debug = true;
                index += 1;
            }
            _ => {
                return Err(format!(
                    "unsupported install argument: {value}\n\n{}",
                    install_usage()
                ));
            }
        }
    }

    Ok(Some(options))
}

fn install_report(options: &InstallOptions) -> serde_json::Value {
    match build_install_report(options) {
        Ok(report) => report,
        Err(err) => json!({
            "success": false,
            "error": err,
        }),
    }
}

fn build_install_report(options: &InstallOptions) -> Result<serde_json::Value, String> {
    let workspace_root = resolve_workspace_root(options.workspace_root.as_deref())?;
    let install_root = match &options.root {
        Some(path) => path.clone(),
        None => default_install_root()?,
    };
    let cargo_binary = cargo_binary();
    build_install_binaries(&cargo_binary, &workspace_root, options.debug)?;

    let profile = if options.debug { "debug" } else { "release" };
    let build_output_dir = workspace_root.join("target").join(profile);
    let binary_dir = install_root.join("bin");
    fs::create_dir_all(&binary_dir).map_err(|err| {
        format!(
            "create install bin directory {}: {err}",
            binary_dir.display()
        )
    })?;

    let mut installed = serde_json::Map::new();
    for name in INSTALLABLE_BINARIES {
        let source = installed_binary_path(&build_output_dir, name);
        if !source.is_file() {
            return Err(format!(
                "built binary is missing after cargo build: {}",
                source.display()
            ));
        }
        let destination = installed_binary_path(&binary_dir, name);
        fs::copy(&source, &destination).map_err(|err| {
            format!(
                "copy built binary {} to {}: {err}",
                source.display(),
                destination.display()
            )
        })?;
        installed.insert(
            (*name).to_string(),
            json!({
                "source": source.display().to_string(),
                "installed": destination.display().to_string(),
            }),
        );
    }

    let verify = build_verify_install_report(&VerifyInstallOptions {
        current_exe: Some(installed_binary_path(&binary_dir, "browser-harness")),
        install_root: Some(install_root.clone()),
    })?;
    let success = verify
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    Ok(json!({
        "success": success,
        "cargo_binary": PathBuf::from(&cargo_binary).display().to_string(),
        "workspace_root": workspace_root.display().to_string(),
        "profile": profile,
        "install_root": install_root.display().to_string(),
        "binary_dir": binary_dir.display().to_string(),
        "path_contains_binary_dir": path_contains_directory(&binary_dir),
        "installed_binaries": installed,
        "verify_install": verify,
    }))
}

fn resolve_workspace_root(override_root: Option<&Path>) -> Result<PathBuf, String> {
    let root = override_root
        .map(Path::to_path_buf)
        .unwrap_or_else(workspace_root);
    let manifest = root.join("Cargo.toml");
    if manifest.is_file() {
        Ok(root)
    } else {
        Err(format!(
            "workspace root does not contain Cargo.toml: {}",
            manifest.display()
        ))
    }
}

fn default_install_root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CARGO_HOME") {
        return Ok(PathBuf::from(path));
    }
    home_dir()
        .map(|path| path.join(".cargo"))
        .ok_or_else(|| "could not determine home directory for install root".to_string())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut combined = OsString::from(drive);
            combined.push(path);
            Some(PathBuf::from(combined))
        })
}

fn cargo_binary() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn build_install_binaries(
    cargo_binary: &OsString,
    workspace_root: &Path,
    debug: bool,
) -> Result<(), String> {
    let mut command = Command::new(cargo_binary);
    command.arg("build");
    if !debug {
        command.arg("--release");
    }
    command
        .arg("--manifest-path")
        .arg(workspace_root.join("Cargo.toml"));
    for name in INSTALLABLE_BINARIES {
        command.arg("--bin").arg(name);
    }

    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| format!("run cargo build for installer: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo build for installer failed with status {status}"
        ))
    }
}

fn path_contains_directory(directory: &Path) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|entry| entry == directory)
}

#[derive(Debug, Default)]
struct VerifyInstallOptions {
    current_exe: Option<PathBuf>,
    install_root: Option<PathBuf>,
}

fn run_verify_install(args: &[OsString]) -> Result<i32, String> {
    let Some(options) = parse_verify_install_options(args)? else {
        print_verify_install_usage();
        return Ok(0);
    };
    let report = verify_install_report(&options);
    let success = report
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|err| format!("serialize verify-install report: {err}"))?
    );
    Ok(if success { 0 } else { 1 })
}

fn parse_verify_install_options(args: &[OsString]) -> Result<Option<VerifyInstallOptions>, String> {
    let mut options = VerifyInstallOptions::default();
    let mut index = 0;

    while index < args.len() {
        let value = args[index].to_str().ok_or_else(|| {
            format!(
                "verify-install argument is not valid UTF-8: {:?}",
                args[index]
            )
        })?;
        match value {
            "-h" | "--help" => return Ok(None),
            "--current-exe" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| "--current-exe requires a path".to_string())?;
                options.current_exe = Some(PathBuf::from(next));
                index += 2;
            }
            "--install-root" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| "--install-root requires a path".to_string())?;
                options.install_root = Some(PathBuf::from(next));
                index += 2;
            }
            _ => {
                return Err(format!(
                    "unsupported verify-install argument: {value}\n\n{}",
                    verify_install_usage()
                ));
            }
        }
    }

    Ok(Some(options))
}

fn verify_install_report(options: &VerifyInstallOptions) -> serde_json::Value {
    match build_verify_install_report(options) {
        Ok(report) => report,
        Err(err) => json!({
            "success": false,
            "error": err,
        }),
    }
}

fn build_verify_install_report(
    options: &VerifyInstallOptions,
) -> Result<serde_json::Value, String> {
    let current_exe = match &options.current_exe {
        Some(path) => path.clone(),
        None => {
            std::env::current_exe().map_err(|err| format!("resolve current executable: {err}"))?
        }
    };
    let binary_dir = current_exe
        .parent()
        .ok_or_else(|| {
            format!(
                "current executable has no parent directory: {}",
                current_exe.display()
            )
        })?
        .to_path_buf();
    let install_root = match &options.install_root {
        Some(path) => path.clone(),
        None => infer_install_root(&current_exe)?,
    };

    let sibling_binaries = EXPECTED_INSTALLED_BINARIES
        .iter()
        .map(|name| {
            let path = installed_binary_path(&binary_dir, name);
            (
                (*name).to_string(),
                json!({
                    "path": path.display().to_string(),
                    "present": path.is_file(),
                }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let missing_binaries = EXPECTED_INSTALLED_BINARIES
        .iter()
        .filter_map(|name| {
            let path = installed_binary_path(&binary_dir, name);
            (!path.is_file()).then(|| path.display().to_string())
        })
        .collect::<Vec<_>>();

    let python_roots = candidate_python_roots(&install_root);
    let search_roots = if python_roots.is_empty() {
        vec![install_root.clone()]
    } else {
        python_roots.clone()
    };
    let unexpected_legacy_artifacts = find_forbidden_python_artifacts(&search_roots)?;
    let browser_harness_py = find_binary_on_path("browser-harness-py");
    let success = missing_binaries.is_empty()
        && unexpected_legacy_artifacts.is_empty()
        && browser_harness_py.is_none();

    Ok(json!({
        "success": success,
        "current_exe": current_exe.display().to_string(),
        "binary_dir": binary_dir.display().to_string(),
        "install_root": install_root.display().to_string(),
        "sibling_binaries": sibling_binaries,
        "missing_binaries": missing_binaries,
        "python_roots": python_roots
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "unexpected_legacy_artifacts": unexpected_legacy_artifacts
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "browser_harness_py": browser_harness_py.map(|path| path.display().to_string()),
    }))
}

fn infer_install_root(current_exe: &Path) -> Result<PathBuf, String> {
    let binary_dir = current_exe.parent().ok_or_else(|| {
        format!(
            "current executable has no parent directory: {}",
            current_exe.display()
        )
    })?;
    let directory_name = binary_dir.file_name().and_then(|value| value.to_str());
    if matches!(directory_name, Some("bin" | "Scripts")) {
        return binary_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
            format!(
                "could not infer install root from executable path: {}",
                current_exe.display()
            )
        });
    }
    Err(format!(
        "could not infer install root from executable path: {} (expected parent directory named bin or Scripts)",
        current_exe.display()
    ))
}

fn installed_binary_path(binary_dir: &Path, name: &str) -> PathBuf {
    if std::env::consts::EXE_EXTENSION.is_empty() {
        binary_dir.join(name)
    } else {
        binary_dir.join(format!("{name}.{}", std::env::consts::EXE_EXTENSION))
    }
}

fn candidate_python_roots(install_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for lib_dir_name in ["lib", "lib64"] {
        let lib_dir = install_root.join(lib_dir_name);
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if path.is_dir() && name.starts_with("python") {
                    let site_packages = path.join("site-packages");
                    if site_packages.is_dir() {
                        roots.push(site_packages);
                    }
                }
            }
        }
    }

    let windows_site_packages = install_root.join("Lib").join("site-packages");
    if windows_site_packages.is_dir() {
        roots.push(windows_site_packages);
    }

    let flat_site_packages = install_root.join("site-packages");
    if flat_site_packages.is_dir() {
        roots.push(flat_site_packages);
    }

    roots.sort();
    roots.dedup();
    roots
}

fn find_forbidden_python_artifacts(search_roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut stack = search_roots.to_vec();
    let mut matches = Vec::new();

    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|err| format!("read directory {}: {err}", directory.display()))?;
        for entry in entries {
            let entry = entry
                .map_err(|err| format!("read directory entry in {}: {err}", directory.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|err| format!("inspect file type for {}: {err}", path.display()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if file_type.is_dir() {
                if should_skip_directory(&name) {
                    continue;
                }
                if FORBIDDEN_PYTHON_PACKAGES.contains(&name.as_ref())
                    && path.join("__init__.py").is_file()
                {
                    matches.push(path.join("__init__.py"));
                }
                stack.push(path);
                continue;
            }

            if file_type.is_file() && FORBIDDEN_PYTHON_FILES.contains(&name.as_ref()) {
                matches.push(path);
            }
        }
    }

    matches.sort();
    matches.dedup();
    Ok(matches)
}

fn should_skip_directory(name: &str) -> bool {
    matches!(
        name,
        "__pycache__" | ".git" | ".hg" | ".svn" | "node_modules" | "target"
    )
}

fn find_binary_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_var) {
        let candidate = installed_binary_path(&directory, name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn verify_install_usage() -> &'static str {
    "usage: browser-harness verify-install [--current-exe <path>] [--install-root <path>]\n\
     notes:\n\
       - validates an installed Rust-only package layout\n\
       - checks sibling binaries, installed Python legacy module absence, and browser-harness-py absence"
}

fn install_usage() -> &'static str {
    "usage: browser-harness install [--root <path>] [--workspace-root <path>] [--debug]\n\
     notes:\n\
       - builds the Rust CLI binaries from the repo workspace and installs them into <root>/bin\n\
       - default install root is $CARGO_HOME or ~/.cargo\n\
       - runs verify-install against the installed binary layout after copying"
}

fn print_install_usage() {
    eprintln!("{}", install_usage());
}

fn print_verify_install_usage() {
    eprintln!("{}", verify_install_usage());
}

fn print_usage() {
    eprintln!(
        "usage: browser-harness <admin-command|runner-command>\n\
         internal commands: {}\n\
         admin commands: {}\n\
         runner commands: {}\n\
         notes:\n\
          - this is the Rust-native top-level CLI\n\
          - internal commands run directly in browser-harness\n\
          - admin commands are forwarded to bhctl\n\
          - runner/helper commands are forwarded to bhrun",
        INTERNAL_COMMANDS.join("|"),
        ADMIN_COMMANDS.join("|"),
        RUNNER_HELP
    );
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        candidate_python_roots, find_forbidden_python_artifacts, infer_install_root,
        parse_install_options, route_command, Route,
    };

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

    #[test]
    fn parses_install_options() {
        let options = parse_install_options(&[
            OsString::from("--root"),
            OsString::from("/tmp/browser-harness"),
            OsString::from("--workspace-root"),
            OsString::from("/tmp/workspace"),
            OsString::from("--debug"),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(options.root, Some(PathBuf::from("/tmp/browser-harness")));
        assert_eq!(
            options.workspace_root,
            Some(PathBuf::from("/tmp/workspace"))
        );
        assert!(options.debug);
    }

    #[test]
    fn infers_install_root_from_bin_directory() {
        let current_exe = Path::new("/tmp/bh/bin/browser-harness");
        assert_eq!(
            infer_install_root(current_exe).unwrap(),
            PathBuf::from("/tmp/bh")
        );
    }

    #[test]
    fn finds_python_site_packages_roots() {
        let temp = TestTempDir::new("browser-harness-cli-python-roots");
        let expected = temp.path().join("lib/python3.11/site-packages");
        fs::create_dir_all(&expected).unwrap();

        assert_eq!(candidate_python_roots(temp.path()), vec![expected]);
    }

    #[test]
    fn detects_legacy_python_files_in_site_packages() {
        let temp = TestTempDir::new("browser-harness-cli-legacy-files");
        let site_packages = temp.path().join("lib/python3.11/site-packages");
        fs::create_dir_all(&site_packages).unwrap();
        let helpers_file = site_packages.join("helpers.py");
        fs::write(&helpers_file, b"").unwrap();

        let artifacts = find_forbidden_python_artifacts(&[site_packages]).unwrap();
        assert_eq!(artifacts, vec![helpers_file]);
    }

    #[test]
    fn detects_legacy_python_package_directories() {
        let temp = TestTempDir::new("browser-harness-cli-legacy-packages");
        let site_packages = temp.path().join("lib/python3.11/site-packages");
        let helpers_pkg = site_packages.join("helpers");
        fs::create_dir_all(&helpers_pkg).unwrap();
        let init_file = helpers_pkg.join("__init__.py");
        fs::write(&init_file, b"").unwrap();

        let artifacts = find_forbidden_python_artifacts(&[site_packages]).unwrap();
        assert_eq!(artifacts, vec![init_file]);
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
