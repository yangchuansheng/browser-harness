use bh_protocol::{
    META_CLICK, META_CURRENT_TAB, META_DISPATCH_KEY, META_ENSURE_REAL_TAB, META_GOTO,
    META_IFRAME_TARGET, META_JS, META_LIST_TABS, META_NEW_TAB, META_PAGE_INFO, META_PRESS_KEY,
    META_SCROLL, META_SWITCH_TAB, META_TYPE_TEXT, META_UPLOAD_FILE, META_WAIT_FOR_LOAD,
    PROTOCOL_VERSION,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionModel {
    PersistentRunner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestTransport {
    HostCallsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFamilyKind {
    GeneratedCdp,
    HostUtility,
    CompatibilityHelper,
    EscapeHatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    Experimental,
    Preview,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtocolFamily {
    pub name: &'static str,
    pub kind: ProtocolFamilyKind,
    pub stability: Stability,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostOperation {
    pub name: &'static str,
    pub kind: ProtocolFamilyKind,
    pub stability: Stability,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostManifest {
    pub daemon_protocol_version: u32,
    pub execution_model: ExecutionModel,
    pub guest_transport: GuestTransport,
    pub protocol_families: Vec<ProtocolFamily>,
    pub operations: Vec<HostOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerConfig {
    pub daemon_name: String,
    pub guest_module: Option<String>,
    pub granted_operations: Vec<String>,
    pub allow_http: bool,
    pub allow_raw_cdp: bool,
    pub persistent_guest_state: bool,
}

pub fn default_manifest() -> HostManifest {
    HostManifest {
        daemon_protocol_version: PROTOCOL_VERSION,
        execution_model: ExecutionModel::PersistentRunner,
        guest_transport: GuestTransport::HostCallsOnly,
        protocol_families: vec![
            ProtocolFamily {
                name: "cdp.browser_protocol",
                kind: ProtocolFamilyKind::GeneratedCdp,
                stability: Stability::Preview,
                description: "Generated bindings for the Chrome browser protocol schema.",
            },
            ProtocolFamily {
                name: "cdp.js_protocol",
                kind: ProtocolFamilyKind::GeneratedCdp,
                stability: Stability::Preview,
                description: "Generated bindings for the Chrome JS protocol schema.",
            },
            ProtocolFamily {
                name: "host.events",
                kind: ProtocolFamilyKind::HostUtility,
                stability: Stability::Preview,
                description: "Runner-owned event waiting and filtering utilities.",
            },
            ProtocolFamily {
                name: "compat.helpers",
                kind: ProtocolFamilyKind::CompatibilityHelper,
                stability: Stability::Preview,
                description: "Stable convenience helpers carried forward from the Python shell.",
            },
            ProtocolFamily {
                name: "escape.raw_cdp",
                kind: ProtocolFamilyKind::EscapeHatch,
                stability: Stability::Experimental,
                description: "Deliberate raw CDP escape hatch for gaps in generated bindings or helper coverage.",
            },
        ],
        operations: default_operations(),
    }
}

pub fn default_operations() -> Vec<HostOperation> {
    vec![
        compatibility_helper(
            META_PAGE_INFO,
            "Viewport, scroll, and page metadata snapshot.",
        ),
        compatibility_helper(META_LIST_TABS, "List visible page targets."),
        compatibility_helper(
            META_CURRENT_TAB,
            "Return the currently attached page target.",
        ),
        compatibility_helper(META_NEW_TAB, "Create and attach a new page target."),
        compatibility_helper(
            META_SWITCH_TAB,
            "Activate and attach a specific page target.",
        ),
        compatibility_helper(
            META_ENSURE_REAL_TAB,
            "Recover from internal or stale tabs by selecting a real page tab.",
        ),
        compatibility_helper(
            META_IFRAME_TARGET,
            "Find an iframe target by URL substring for scoped guest operations.",
        ),
        compatibility_helper(META_GOTO, "Navigate the current page target."),
        compatibility_helper(
            META_WAIT_FOR_LOAD,
            "Wait for document readiness in the current page.",
        ),
        compatibility_helper(
            META_JS,
            "Evaluate JavaScript in the current page or iframe target.",
        ),
        compatibility_helper(META_CLICK, "Dispatch a browser-level pointer click."),
        compatibility_helper(
            META_TYPE_TEXT,
            "Insert text using browser input primitives.",
        ),
        compatibility_helper(
            META_PRESS_KEY,
            "Dispatch browser-level keydown/char/keyup sequences.",
        ),
        compatibility_helper(
            META_DISPATCH_KEY,
            "Dispatch a DOM KeyboardEvent on a matched element.",
        ),
        compatibility_helper(META_SCROLL, "Dispatch browser-level mouse wheel scrolling."),
        compatibility_helper("screenshot", "Capture the current page as an image."),
        compatibility_helper(
            META_UPLOAD_FILE,
            "Assign files to an input element in the current page or iframe target.",
        ),
        HostOperation {
            name: "wait",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Stable,
            description: "Sleep without involving the browser connection.",
        },
        HostOperation {
            name: "wait_for_event",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description:
                "Wait for a filtered browser event stream match owned by the runner/daemon.",
        },
        HostOperation {
            name: "http_get",
            kind: ProtocolFamilyKind::HostUtility,
            stability: Stability::Preview,
            description: "Issue pure HTTP reads outside the browser session.",
        },
        HostOperation {
            name: "cdp_raw",
            kind: ProtocolFamilyKind::EscapeHatch,
            stability: Stability::Experimental,
            description: "Send an explicit raw CDP request through the daemon.",
        },
    ]
}

pub fn default_runner_config() -> RunnerConfig {
    let manifest = default_manifest();
    RunnerConfig {
        daemon_name: "default".to_string(),
        guest_module: None,
        granted_operations: manifest
            .operations
            .iter()
            .filter(|operation| operation.kind != ProtocolFamilyKind::EscapeHatch)
            .map(|operation| operation.name.to_string())
            .collect(),
        allow_http: true,
        allow_raw_cdp: false,
        persistent_guest_state: true,
    }
}

pub fn operation_names() -> Vec<&'static str> {
    default_manifest()
        .operations
        .into_iter()
        .map(|operation| operation.name)
        .collect()
}

fn compatibility_helper(name: &'static str, description: &'static str) -> HostOperation {
    HostOperation {
        name,
        kind: ProtocolFamilyKind::CompatibilityHelper,
        stability: Stability::Stable,
        description,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_manifest, default_runner_config, operation_names, ExecutionModel, GuestTransport,
        ProtocolFamilyKind, Stability,
    };

    #[test]
    fn manifest_uses_persistent_runner_boundary() {
        let manifest = default_manifest();

        assert_eq!(manifest.execution_model, ExecutionModel::PersistentRunner);
        assert_eq!(manifest.guest_transport, GuestTransport::HostCallsOnly);
        assert!(manifest
            .protocol_families
            .iter()
            .any(|family| family.name == "cdp.browser_protocol"
                && family.kind == ProtocolFamilyKind::GeneratedCdp));
        assert!(manifest
            .operations
            .iter()
            .any(|operation| operation.name == "wait_for_event"
                && operation.kind == ProtocolFamilyKind::HostUtility
                && operation.stability == Stability::Preview));
    }

    #[test]
    fn runner_config_disables_raw_cdp_by_default() {
        let config = default_runner_config();

        assert!(config.persistent_guest_state);
        assert!(!config.allow_raw_cdp);
        assert!(config
            .granted_operations
            .iter()
            .all(|name| name != "cdp_raw"));
    }

    #[test]
    fn operation_names_include_helper_and_escape_hatch_layers() {
        let names = operation_names();

        assert!(names.contains(&"page_info"));
        assert!(names.contains(&"wait_for_event"));
        assert!(names.contains(&"cdp_raw"));
    }
}
