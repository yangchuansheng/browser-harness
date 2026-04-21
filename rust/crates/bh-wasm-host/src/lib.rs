#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCapability {
    pub name: &'static str,
}

pub fn default_capabilities() -> Vec<HostCapability> {
    vec![
        HostCapability { name: "page_info" },
        HostCapability { name: "new_tab" },
        HostCapability { name: "goto" },
        HostCapability { name: "click" },
        HostCapability { name: "type_text" },
        HostCapability { name: "press_key" },
        HostCapability { name: "scroll" },
        HostCapability { name: "screenshot" },
        HostCapability { name: "wait" },
        HostCapability {
            name: "wait_for_load",
        },
        HostCapability { name: "list_tabs" },
        HostCapability { name: "switch_tab" },
        HostCapability { name: "js" },
        HostCapability { name: "http_get" },
        HostCapability { name: "cdp_raw" },
    ]
}
