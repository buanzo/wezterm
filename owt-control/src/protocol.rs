use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    interface::{Scope, ScopeKind},
    runtime::{
        DispatchedAction, FocusedTableCell, InterfaceLifecycleState, LayoutScenePlan,
        RequestedRefresh, RuntimeEvent,
    },
};

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
    "list_interfaces",
    "load_interface",
    "replace_interface",
    "edit_interface",
    "start_interface_build",
    "update_node",
    "add_interface_node",
    "replace_interface_node",
    "patch_interface_node",
    "remove_interface_node",
    "move_interface_node",
    "clear_interface_children",
    "highlight_interface_node",
    "set_interface_feedback_mode",
    "request_refresh",
    "list_action_requests",
    "preview_reflow",
    "validate_layout",
    "patch_interface_layout",
    "patch_interface_lifecycle",
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub last_dispatched_action_by_interface: BTreeMap<String, DispatchedAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interface_statuses: Vec<InterfaceRuntimeStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_scene_plan: Option<LayoutScenePlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_events: Vec<RuntimeEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_action_requests: Vec<DispatchedAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_refresh_requests: Vec<RequestedRefresh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scope: Option<Scope>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceRuntimeStatus {
    pub interface_id: String,
    pub title: String,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<InterfaceOwner>,
    #[serde(default)]
    pub accepted: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub native_render_passes: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub render_projection: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatched_action: Option<DispatchedAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<InterfaceLifecycleState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_table_cell: Option<FocusedTableCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceOwner {
    pub kind: ScopeKind,
    pub id: String,
    pub scope_key: String,
}

impl InterfaceOwner {
    pub fn from_scope(scope: &Scope) -> Self {
        Self {
            kind: scope.kind.clone(),
            id: scope.id.clone(),
            scope_key: scope.key(),
        }
    }
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
            last_dispatched_action_by_interface: BTreeMap::new(),
            interface_statuses: Vec::new(),
            layout_scene_plan: None,
            recent_events: Vec::new(),
            recent_action_requests: Vec::new(),
            recent_refresh_requests: Vec::new(),
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
            last_dispatched_action_by_interface: BTreeMap::new(),
            interface_statuses: Vec::new(),
            layout_scene_plan: None,
            recent_events: Vec::new(),
            recent_action_requests: Vec::new(),
            recent_refresh_requests: Vec::new(),
            active_scope,
            message: "native OWT control endpoint is ready".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InterfaceOwner, NATIVE_LIFECYCLE_TOOLS, PROTOTYPE_COMPAT_TOOLS};
    use crate::interface::{Scope, ScopeKind};

    #[test]
    fn tool_lists_include_compat_and_lifecycle_surface() {
        assert!(PROTOTYPE_COMPAT_TOOLS.contains(&"attach_lcars_panel"));
        assert!(PROTOTYPE_COMPAT_TOOLS.contains(&"attach_resource_view"));
        assert!(PROTOTYPE_COMPAT_TOOLS.contains(&"attach_folder_view"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"apply_interface"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"replace_interface"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"request_refresh"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"list_action_requests"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"preview_reflow"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"validate_layout"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"patch_interface_layout"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"patch_interface_lifecycle"));
        assert!(NATIVE_LIFECYCLE_TOOLS.contains(&"retire_interface"));
    }

    #[test]
    fn interface_owner_projects_scope_without_model_context() {
        let scope = Scope::new(ScopeKind::Project, "/home/buanzo/git/tools");
        let owner = InterfaceOwner::from_scope(&scope);

        assert_eq!(owner.kind, ScopeKind::Project);
        assert_eq!(owner.id, "/home/buanzo/git/tools");
        assert_eq!(owner.scope_key, "Project:/home/buanzo/git/tools");
    }
}
