use serde::{Deserialize, Serialize};

use crate::{interface::Scope, runtime::DispatchedAction};

pub const PROTOCOL_VERSION: u16 = 1;

pub const PROTOTYPE_COMPAT_TOOLS: &[&str] = &[
    "owt_status",
    "owt_expected_contract",
    "get_active_context",
    "list_panes",
    "focus_pane",
    "resize_pane",
    "split_pane",
    "send_input_to_pane",
    "spawn_tab",
    "set_tab_title",
    "get_ui_state",
    "set_status_bar",
    "clear_status_bar",
    "notify_ui",
    "register_ui_action",
    "clear_ui_action",
    "attach_resource_view",
    "toggle_resource_view",
    "clear_resource_view",
    "attach_folder_view",
    "toggle_folder_view",
    "clear_folder_view",
    "attach_lcars_panel",
    "clear_lcars_panel",
];

pub const NATIVE_LIFECYCLE_TOOLS: &[&str] = &[
    "build_interface",
    "validate_interface",
    "apply_interface",
    "save_interface",
    "load_interface",
    "update_node",
    "bind_action",
    "dispatch_action",
    "export_interface",
    "import_interface",
    "diff_interface",
    "retire_interface",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointMode {
    Native,
    NativeWindowOnly,
    PrototypeSidecar,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStatus {
    pub protocol_version: u16,
    pub product: String,
    pub endpoint_mode: EndpointMode,
    pub native_runtime_ready: bool,
    #[serde(default)]
    pub native_rendering_ready: bool,
    #[serde(default)]
    pub native_render_passes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatched_action: Option<DispatchedAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scope: Option<Scope>,
    pub message: String,
}

impl ControlStatus {
    pub fn native_window_only(message: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            product: "OWT".to_string(),
            endpoint_mode: EndpointMode::NativeWindowOnly,
            native_runtime_ready: false,
            native_rendering_ready: false,
            native_render_passes: 0,
            last_dispatched_action: None,
            active_scope: None,
            message: message.into(),
        }
    }

    pub fn native_ready(active_scope: Option<Scope>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            product: "OWT".to_string(),
            endpoint_mode: EndpointMode::Native,
            native_runtime_ready: true,
            native_rendering_ready: false,
            native_render_passes: 0,
            last_dispatched_action: None,
            active_scope,
            message: "native OWT control endpoint is ready".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NATIVE_LIFECYCLE_TOOLS, PROTOTYPE_COMPAT_TOOLS};

    #[test]
    fn tool_lists_include_compat_and_lifecycle_surface() {
        assert!(PROTOTYPE_COMPAT_TOOLS.contains(&"attach_lcars_panel"));
        assert!(PROTOTYPE_COMPAT_TOOLS.contains(&"attach_resource_view"));
        assert!(PROTOTYPE_COMPAT_TOOLS.contains(&"attach_folder_view"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"apply_interface"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"retire_interface"));
    }
}
