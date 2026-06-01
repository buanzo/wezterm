use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::interface::{
    ActionKind, FactProvenance, InterfaceDocument, Scope, UiAction, UiNode, UiNodeKind,
};

pub type Result<T> = std::result::Result<T, ControlError>;
const NATIVE_KEYBOARD_ACTION_LIMIT: usize = 9;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlError {
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("duplicate {kind} id: {id}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("node {node_id} references unknown action id: {action_id}")]
    UnknownAction { node_id: String, action_id: String },
    #[error("action {action_id} kind is not allowed by this interface: {kind}")]
    ActionKindNotAllowed { action_id: String, kind: String },
    #[error("action {action_id} kind requires explicit confirmation: {kind}")]
    ActionRequiresConfirmation { action_id: String, kind: String },
    #[error("{field} contains credential-sensitive content: {indicator}")]
    SensitiveContent { field: String, indicator: String },
    #[error("action {action_id} is not registered")]
    UnregisteredAction { action_id: String },
    #[error("action {action_id} is registered by multiple interfaces: {interface_ids}")]
    AmbiguousAction {
        action_id: String,
        interface_ids: String,
    },
    #[error("action {action_id} requires confirmation before dispatch: {kind}")]
    DispatchRequiresConfirmation { action_id: String, kind: String },
    #[error("action {action_id} requires explicit permission before external execution: {kind}")]
    ActionExecutionRequiresPermission { action_id: String, kind: String },
    #[error("action {action_id} requires explicit confirmation before external execution: {kind}")]
    ActionExecutionRequiresConfirmation { action_id: String, kind: String },
    #[error("refresh action {action_id} requires explicit permission before external execution")]
    RefreshRequiresPermission { action_id: String },
    #[error("permission request {request_id} was not found")]
    UnknownPermissionRequest { request_id: String },
    #[error("permission request {request_id} is already {approval_state}")]
    PermissionRequestAlreadyResolved {
        request_id: String,
        approval_state: String,
    },
    #[error("unknown interface id: {interface_id}")]
    UnknownInterface { interface_id: String },
    #[error("no active interface is available")]
    NoActiveInterface,
    #[error("parent node {parent_id} was not found in interface {interface_id}")]
    UnknownParent {
        interface_id: String,
        parent_id: String,
    },
    #[error("node {node_id} was not found in interface {interface_id}")]
    UnknownNode {
        interface_id: String,
        node_id: String,
    },
    #[error("interface {interface_id} has no focusable table rows")]
    NoFocusableTableRows { interface_id: String },
    #[error("interface {interface_id} has no focusable table columns")]
    NoFocusableTableColumns { interface_id: String },
    #[error("interface {interface_id} has no focused table group")]
    NoFocusedTableGroup { interface_id: String },
    #[error("interface {interface_id} has no focused table row action")]
    NoFocusedTableRowAction { interface_id: String },
    #[error("interface must contain at least one root node")]
    EmptyInterface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedInterface {
    pub scope: Scope,
    pub interface_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceValidationWarning {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchedInterfaceLayout {
    pub applied: AppliedInterface,
    pub node_id: String,
    pub prior_properties: BTreeMap<String, String>,
    pub applied_properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutPreview {
    pub target: AppliedInterface,
    pub node_id: String,
    pub prior_properties: BTreeMap<String, String>,
    pub requested_properties: BTreeMap<String, String>,
    pub preview_properties: BTreeMap<String, String>,
    pub scene_plan: LayoutScenePlan,
    pub estimated_reserved_space: LayoutReservedSpaceEstimate,
    pub viewport_fit: LayoutViewportFit,
    pub overflow_estimate: LayoutOverflowEstimate,
    pub fit_score: LayoutFitScore,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflict_hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutScenePlan {
    pub active_interface_count: usize,
    pub surface_count: usize,
    pub reserved_surface_count: usize,
    pub estimated_left_columns: u16,
    pub estimated_right_columns: u16,
    pub estimated_top_rows: u16,
    pub estimated_bottom_rows: u16,
    pub basis: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<LayoutSceneSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<LayoutSceneSurface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflict_hints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutSceneSlot {
    pub slot: String,
    pub interface_count: usize,
    pub reserved_surface_count: usize,
    pub estimated_columns: u16,
    pub estimated_rows: u16,
    pub conflict: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interface_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutSceneSurface {
    pub interface_id: String,
    pub scope: Scope,
    pub node_id: String,
    pub title: String,
    pub visible: bool,
    pub slot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solved_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflow_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapse_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapse_policy: Option<String>,
    pub priority: i32,
    pub reserves_terminal_space: bool,
    pub estimated_columns: u16,
    pub estimated_rows: u16,
    pub action_count: usize,
    pub action_page_count: usize,
    pub automatic_action_slots_per_page: usize,
    pub overflow_action_count: usize,
    pub table_count: usize,
    pub table_row_count: usize,
    pub estimated_hidden_table_rows: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_slots: Vec<LayoutSceneActionSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overflow_action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutSceneActionSlot {
    pub slot_index: usize,
    pub action_id: String,
    pub label: String,
    pub kind: ActionKind,
    pub automatic_shortcut: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutReservedSpaceEstimate {
    pub slot: String,
    pub reserves_terminal_space: bool,
    pub columns: u16,
    pub rows: u16,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutViewportFit {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_columns: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_rows: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_columns_after_reservation: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_rows_after_reservation: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_columns: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_rows: Option<u16>,
    pub reserved_columns: u16,
    pub reserved_rows: u16,
    pub overflow_columns: u16,
    pub overflow_rows: u16,
    pub basis: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutOverflowEstimate {
    pub action_count: usize,
    pub automatic_action_slots: usize,
    pub overflow_action_count: usize,
    pub table_count: usize,
    pub table_row_count: usize,
    pub estimated_visible_table_rows: usize,
    pub estimated_hidden_table_rows: usize,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutFitScore {
    pub score: u8,
    pub status: String,
    pub reservation_slot: String,
    pub reserves_terminal_space: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredInterface {
    pub interface_id: String,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired_scope_keys: Vec<String>,
    pub remaining_interfaces: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceLifecycleState {
    pub interface_id: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub expired: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl InterfaceLifecycleState {
    fn new(interface_id: String) -> Self {
        Self {
            interface_id,
            pinned: false,
            hidden: false,
            expired: false,
            expires_at_unix: None,
            ttl_seconds: None,
            previous_hidden: None,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceLifecyclePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(default)]
    pub restore_previous: bool,
    #[serde(default)]
    pub expire_now: bool,
    #[serde(default)]
    pub clear_expiration: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now_unix: Option<u64>,
}

impl InterfaceLifecyclePatch {
    pub fn is_empty(&self) -> bool {
        self.pinned.is_none()
            && self.hidden.is_none()
            && !self.restore_previous
            && !self.expire_now
            && !self.clear_expiration
            && self.expires_at_unix.is_none()
            && self.ttl_seconds.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchedInterfaceLifecycle {
    pub interface_id: String,
    pub scope: Scope,
    pub prior: InterfaceLifecycleState,
    pub state: InterfaceLifecycleState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacedInterface {
    pub previous_interface_id: String,
    pub applied: AppliedInterface,
    #[serde(default)]
    pub lifecycle_preserved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum InterfaceEditOperation {
    AddNode {
        #[serde(default, alias = "parent")]
        parent_id: Option<String>,
        node: UiNode,
        #[serde(default)]
        action: Option<UiAction>,
        #[serde(default)]
        replace: bool,
    },
    ReplaceNode {
        node: UiNode,
        #[serde(default)]
        action: Option<UiAction>,
    },
    PatchNode {
        #[serde(default, alias = "node", alias = "target_node_id")]
        node_id: Option<String>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        action_id: Option<String>,
        #[serde(default)]
        provenance: Option<FactProvenance>,
        #[serde(default)]
        properties: BTreeMap<String, String>,
        #[serde(default)]
        remove_properties: Vec<String>,
        #[serde(default)]
        clear_fields: Vec<String>,
    },
    RemoveNode {
        #[serde(default, alias = "node", alias = "target_node_id")]
        node_id: Option<String>,
        #[serde(default)]
        prune_orphan_actions: Option<bool>,
    },
    MoveNode {
        #[serde(default, alias = "node", alias = "target_node_id")]
        node_id: Option<String>,
        #[serde(default, alias = "parent", alias = "new_parent")]
        parent_id: Option<String>,
        #[serde(default)]
        position: Option<usize>,
    },
    ClearChildren {
        #[serde(default, alias = "node", alias = "target_node_id")]
        node_id: Option<String>,
    },
    BindAction {
        #[serde(default, alias = "node", alias = "target_node_id")]
        node_id: Option<String>,
        action: UiAction,
    },
    HighlightNode {
        #[serde(default, alias = "node", alias = "target_node_id")]
        node_id: Option<String>,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceEditOperationResult {
    pub index: usize,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditedInterface {
    pub applied: AppliedInterface,
    #[serde(default)]
    pub dry_run: bool,
    pub operations: Vec<InterfaceEditOperationResult>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestApprovalState {
    #[default]
    NotRequired,
    Pending,
    Granted,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub request_id: String,
    pub request_kind: String,
    pub approval_state: RequestApprovalState,
    pub interface_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestedRefresh {
    pub interface_id: String,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ActionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_at_unix: Option<u64>,
    #[serde(default)]
    pub approval_state: RequestApprovalState,
    #[serde(default)]
    pub approval_required: bool,
    #[serde(default)]
    pub permission_granted: bool,
    #[serde(default)]
    pub external_execution_requested: bool,
    #[serde(default)]
    pub executed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableFocusMovement {
    Previous,
    Next,
    PreviousGroup,
    NextGroup,
    First,
    Last,
}

impl TableFocusMovement {
    fn as_str(self) -> &'static str {
        match self {
            TableFocusMovement::Previous => "previous",
            TableFocusMovement::Next => "next",
            TableFocusMovement::PreviousGroup => "previous_group",
            TableFocusMovement::NextGroup => "next_group",
            TableFocusMovement::First => "first",
            TableFocusMovement::Last => "last",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableCellFocusMovement {
    Previous,
    Next,
    First,
    Last,
}

impl TableCellFocusMovement {
    fn as_str(self) -> &'static str {
        match self {
            TableCellFocusMovement::Previous => "previous",
            TableCellFocusMovement::Next => "next",
            TableCellFocusMovement::First => "first",
            TableCellFocusMovement::Last => "last",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusedTableRow {
    pub interface_id: String,
    pub scope: Scope,
    pub table_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_node_id: Option<String>,
    pub focused_row: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_group: Option<String>,
    pub movement: TableFocusMovement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusedTableCell {
    pub interface_id: String,
    pub scope: Scope,
    pub table_node_id: String,
    pub focused_column: String,
    pub column_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<TableCellFocusMovement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusedTableGroup {
    pub interface_id: String,
    pub scope: Scope,
    pub table_node_id: String,
    pub focused_group: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_focus_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_expanded_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceDiff {
    pub left_interface_id: String,
    pub right_interface_id: String,
    #[serde(default)]
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summary: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_root_properties: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_facts: Vec<FactDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactDiff {
    pub node_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventKind {
    InterfaceApplied,
    InterfaceRetired,
    NodeUpdated,
    NodeAdded,
    NodeReplaced,
    NodePatched,
    NodeRemoved,
    NodeMoved,
    NodeHighlighted,
    LayoutPatched,
    LifecyclePatched,
    InterfaceReplaced,
    ActionDispatched,
    ActionExecutionRequested,
    RefreshRequested,
    PermissionGranted,
    PermissionDenied,
    RefreshCompleted,
    RefreshFailed,
    Rendered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub sequence: u64,
    pub kind: RuntimeEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchedAction {
    pub action_id: String,
    pub label: String,
    pub kind: ActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_at_unix: Option<u64>,
    #[serde(default)]
    pub approval_state: RequestApprovalState,
    #[serde(default)]
    pub approval_required: bool,
    #[serde(default)]
    pub permission_granted: bool,
    #[serde(default)]
    pub confirmation_granted: bool,
    #[serde(default)]
    pub external_execution_requested: bool,
    #[serde(default)]
    pub executed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_at_unix: Option<u64>,
}

fn approval_state_name(state: RequestApprovalState) -> &'static str {
    match state {
        RequestApprovalState::NotRequired => "not_required",
        RequestApprovalState::Pending => "pending",
        RequestApprovalState::Granted => "granted",
        RequestApprovalState::Denied => "denied",
    }
}

fn request_id_component(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if output.len() >= 48 {
            break;
        }
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '.' | '_' | '-') {
            output.push(ch);
        } else if !output.ends_with('.') {
            output.push('.');
        }
    }
    let trimmed = output.trim_matches('.').to_string();
    if trimmed.is_empty() {
        "request".to_string()
    } else {
        trimmed
    }
}

fn generated_request_id(
    prefix: &str,
    interface_id: &str,
    action_id: &str,
    sequence: u64,
) -> String {
    format!(
        "{}.{}.{}.{}",
        prefix,
        request_id_component(interface_id),
        request_id_component(action_id),
        sequence
    )
}

const MAX_RUNTIME_EVENTS: usize = 128;
const MAX_ACTION_REQUESTS: usize = 128;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub interfaces: BTreeMap<String, InterfaceDocument>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub active_interface_by_scope: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, UiAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions_by_interface: BTreeMap<String, BTreeMap<String, UiAction>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_by_scope: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatched_action: Option<DispatchedAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub last_dispatched_action_by_interface: BTreeMap<String, DispatchedAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub last_render_pass_by_interface: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lifecycle_by_interface: BTreeMap<String, InterfaceLifecycleState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<RuntimeEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_requests: Vec<DispatchedAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refresh_requests: Vec<RequestedRefresh>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub pending_render_event_interfaces: BTreeSet<String>,
    #[serde(default)]
    pub next_event_sequence: u64,
}

impl RuntimeState {
    pub fn apply_interface(&mut self, document: InterfaceDocument) -> Result<AppliedInterface> {
        validate_interface(&document)?;

        let scope = document.scope.clone();
        let interface_id = document.id.clone();

        for action in &document.actions {
            self.actions.insert(action.id.clone(), action.clone());
        }
        self.actions_by_interface.insert(
            interface_id.clone(),
            document
                .actions
                .iter()
                .map(|action| (action.id.clone(), action.clone()))
                .collect(),
        );
        self.pending_render_event_interfaces
            .insert(interface_id.clone());
        self.lifecycle_by_interface
            .entry(interface_id.clone())
            .or_insert_with(|| InterfaceLifecycleState::new(interface_id.clone()));

        self.interfaces.insert(interface_id.clone(), document);
        self.active_interface_by_scope
            .insert(scope.key(), interface_id.clone());
        self.push_event(
            RuntimeEventKind::InterfaceApplied,
            Some(interface_id.clone()),
            Some(scope.clone()),
            None,
            None,
            Some("interface accepted into native runtime".to_string()),
        );

        Ok(AppliedInterface {
            scope,
            interface_id,
        })
    }

    pub fn active_interface_for_scope(&self, scope: &Scope) -> Option<&InterfaceDocument> {
        let interface_id = self.active_interface_by_scope.get(&scope.key())?;
        self.interfaces.get(interface_id)
    }

    pub fn active_interface_ids_for_scene(&self) -> Vec<String> {
        let active_ids = self
            .active_interface_by_scope
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();

        for event in self.events.iter().rev() {
            if !matches!(
                event.kind,
                RuntimeEventKind::InterfaceApplied | RuntimeEventKind::InterfaceReplaced
            ) {
                continue;
            }
            let Some(interface_id) = event.interface_id.as_ref() else {
                continue;
            };
            if active_ids.contains(interface_id) && seen.insert(interface_id.clone()) {
                ordered.push(interface_id.clone());
            }
        }

        for interface_id in self.active_interface_by_scope.values() {
            if seen.insert(interface_id.clone()) {
                ordered.push(interface_id.clone());
            }
        }

        ordered
    }

    pub fn dispatch_action(&mut self, action_id: &str) -> Result<DispatchedAction> {
        self.dispatch_action_for_interface(None, action_id)
    }

    pub fn dispatch_action_for_interface(
        &mut self,
        interface_id: Option<&str>,
        action_id: &str,
    ) -> Result<DispatchedAction> {
        self.dispatch_action_for_interface_with_intent(interface_id, action_id, false, false, false)
    }

    pub fn dispatch_action_for_interface_with_intent(
        &mut self,
        interface_id: Option<&str>,
        action_id: &str,
        permission_granted: bool,
        confirmation_granted: bool,
        external_execution_requested: bool,
    ) -> Result<DispatchedAction> {
        self.dispatch_action_for_interface_with_intent_and_provenance(
            interface_id,
            action_id,
            permission_granted,
            confirmation_granted,
            external_execution_requested,
            ActionProvenance::default(),
        )
    }

    pub fn dispatch_action_for_interface_with_intent_and_provenance(
        &mut self,
        interface_id: Option<&str>,
        action_id: &str,
        permission_granted: bool,
        confirmation_granted: bool,
        external_execution_requested: bool,
        provenance: ActionProvenance,
    ) -> Result<DispatchedAction> {
        require_non_empty("action_id", action_id)?;
        let (interface_id, scope, action) =
            self.resolve_dispatch_action(interface_id, action_id)?;

        let needs_confirmation = action.requires_confirmation
            || matches!(
                &action.kind,
                ActionKind::Destructive | ActionKind::CredentialSensitive
            );
        if external_execution_requested {
            if permission_granted && needs_confirmation && !confirmation_granted {
                return Err(ControlError::ActionExecutionRequiresConfirmation {
                    action_id: action.id.clone(),
                    kind: action_kind_name(&action),
                });
            }
        } else if needs_confirmation {
            return Err(ControlError::DispatchRequiresConfirmation {
                action_id: action.id.clone(),
                kind: action_kind_name(&action),
            });
        }

        let approval_state = if external_execution_requested {
            if permission_granted {
                RequestApprovalState::Granted
            } else {
                RequestApprovalState::Pending
            }
        } else {
            RequestApprovalState::NotRequired
        };
        let request_id = if external_execution_requested {
            Some(provenance.request_id.clone().unwrap_or_else(|| {
                generated_request_id(
                    "action",
                    &interface_id,
                    &action.id,
                    self.next_event_sequence + 1,
                )
            }))
        } else {
            provenance.request_id.clone()
        };
        let execution_message = if external_execution_requested {
            Some(if approval_state == RequestApprovalState::Granted {
                "external execution approved and audited; native OWT did not run the command"
                    .to_string()
            } else {
                "external execution requested; pending terminal-owned approval".to_string()
            })
        } else {
            None
        };
        let dispatched = DispatchedAction {
            action_id: action.id.clone(),
            label: action.label.clone(),
            kind: action.kind.clone(),
            interface_id: Some(interface_id.clone()),
            scope: Some(scope.clone()),
            command: action.command.clone(),
            argv: action.argv.clone(),
            cwd: action.cwd.clone(),
            mode: action.mode.clone(),
            target: action.target.clone(),
            source: provenance.source.clone(),
            request_id,
            requested_at_unix: provenance.requested_at_unix,
            approval_state,
            approval_required: approval_state == RequestApprovalState::Pending,
            permission_granted: approval_state == RequestApprovalState::Granted,
            confirmation_granted: approval_state == RequestApprovalState::Granted
                && confirmation_granted,
            external_execution_requested,
            executed: false,
            execution_status: external_execution_requested.then(|| {
                if approval_state == RequestApprovalState::Granted {
                    "recorded_not_executed".to_string()
                } else {
                    "pending_approval".to_string()
                }
            }),
            execution_message: execution_message.clone(),
        };
        let focused_node_id = self.focus_table_row_for_action(&interface_id, &action.id);
        self.last_dispatched_action = Some(dispatched.clone());
        self.last_dispatched_action_by_interface
            .insert(interface_id.clone(), dispatched.clone());
        if external_execution_requested {
            self.push_action_request(dispatched.clone());
        }
        self.push_event(
            RuntimeEventKind::ActionDispatched,
            Some(interface_id.clone()),
            Some(scope.clone()),
            Some(dispatched.action_id.clone()),
            focused_node_id.clone(),
            Some(if focused_node_id.is_some() {
                "safe action dispatch recorded; table row focus updated".to_string()
            } else {
                "safe action dispatch recorded".to_string()
            }),
        );
        if external_execution_requested {
            self.push_event(
                RuntimeEventKind::ActionExecutionRequested,
                Some(interface_id),
                Some(scope),
                Some(dispatched.action_id.clone()),
                focused_node_id,
                execution_message,
            );
        }
        Ok(dispatched)
    }

    pub fn declared_action_for_interface(
        &self,
        interface_id: Option<&str>,
        action_id: &str,
    ) -> Result<(String, Scope, UiAction)> {
        self.resolve_dispatch_action(interface_id, action_id)
    }

    pub fn action_is_root_allowlisted(&self, interface_id: &str, action: &UiAction) -> bool {
        self.interfaces
            .get(interface_id)
            .map(|document| action_is_allowlisted(action, &action_allowlist_entries(document)))
            .unwrap_or(false)
    }

    pub fn record_action_execution_result(
        &mut self,
        interface_id: &str,
        action_id: &str,
        executed: bool,
        status: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<DispatchedAction> {
        require_non_empty("interface_id", interface_id)?;
        require_non_empty("action_id", action_id)?;
        let status = status.into();
        let message = message.into();
        let Some(mut dispatched) = self
            .last_dispatched_action_by_interface
            .get(interface_id)
            .filter(|action| action.action_id == action_id)
            .cloned()
            .or_else(|| {
                self.last_dispatched_action
                    .as_ref()
                    .filter(|action| {
                        action.interface_id.as_deref() == Some(interface_id)
                            && action.action_id == action_id
                    })
                    .cloned()
            })
        else {
            return Err(ControlError::UnregisteredAction {
                action_id: action_id.to_string(),
            });
        };

        dispatched.executed = executed;
        dispatched.execution_status = Some(status.clone());
        dispatched.execution_message = Some(message.clone());
        self.last_dispatched_action = Some(dispatched.clone());
        self.last_dispatched_action_by_interface
            .insert(interface_id.to_string(), dispatched.clone());

        let scope = dispatched.scope.clone();
        self.push_event(
            RuntimeEventKind::ActionDispatched,
            Some(interface_id.to_string()),
            scope,
            Some(action_id.to_string()),
            None,
            Some(message),
        );

        Ok(dispatched)
    }

    pub fn record_action_request_execution_result(
        &mut self,
        request_id: &str,
        executed: bool,
        status: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<DispatchedAction> {
        require_non_empty("request_id", request_id)?;
        let status = status.into();
        let message = message.into();
        let Some(index) = self
            .action_requests
            .iter()
            .position(|request| request.request_id.as_deref() == Some(request_id))
        else {
            return Err(ControlError::UnknownPermissionRequest {
                request_id: request_id.to_string(),
            });
        };

        let (interface_id, action_id, scope, dispatched) = {
            let request = &mut self.action_requests[index];
            request.executed = executed;
            request.approval_required = false;
            request.execution_status = Some(status);
            request.execution_message = Some(message.clone());
            (
                request.interface_id.clone().unwrap_or_default(),
                request.action_id.clone(),
                request.scope.clone(),
                request.clone(),
            )
        };

        self.last_dispatched_action = Some(dispatched.clone());
        if !interface_id.is_empty() {
            self.last_dispatched_action_by_interface
                .insert(interface_id.clone(), dispatched.clone());
        }
        self.push_event(
            RuntimeEventKind::ActionDispatched,
            Some(interface_id),
            scope,
            Some(action_id),
            None,
            Some(message),
        );

        Ok(dispatched)
    }

    pub fn dispatch_focused_table_row_action(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
    ) -> Result<DispatchedAction> {
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let action_id = self
            .interfaces
            .get(&target_interface_id)
            .and_then(focused_table_row_action_for_document)
            .ok_or_else(|| ControlError::NoFocusedTableRowAction {
                interface_id: target_interface_id.clone(),
            })?;
        self.dispatch_action_for_interface(Some(&target_interface_id), &action_id)
    }

    pub fn update_node(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        parent_id: Option<&str>,
        node: UiNode,
        action: Option<UiAction>,
        replace: bool,
    ) -> Result<AppliedInterface> {
        require_non_empty("node.id", &node.id)?;
        let node_id_for_event = node.id.clone();
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let mut document = self
            .interfaces
            .get(&target_interface_id)
            .cloned()
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;

        if let Some(action) = action {
            document.actions.retain(|existing| existing.id != action.id);
            document.actions.push(action);
        }

        if let Some(existing) = find_node_mut(&mut document.nodes, &node.id) {
            if !replace {
                return Err(ControlError::DuplicateId {
                    kind: "node",
                    id: node.id,
                });
            }
            *existing = node;
        } else if let Some(parent_id) = parent_id.filter(|value| !value.trim().is_empty()) {
            let parent = find_node_mut(&mut document.nodes, parent_id).ok_or_else(|| {
                ControlError::UnknownParent {
                    interface_id: target_interface_id.clone(),
                    parent_id: parent_id.to_string(),
                }
            })?;
            parent.children.push(node);
        } else {
            document.nodes.push(node);
        }

        let applied = self.apply_interface(document)?;
        self.push_event(
            RuntimeEventKind::NodeUpdated,
            Some(applied.interface_id.clone()),
            Some(applied.scope.clone()),
            None,
            Some(node_id_for_event),
            Some("semantic node update applied".to_string()),
        );
        Ok(applied)
    }

    pub fn bind_action(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        node_id: &str,
        action: UiAction,
    ) -> Result<AppliedInterface> {
        require_non_empty("node.id", node_id)?;
        require_non_empty("action.id", &action.id)?;
        require_non_empty("action.label", &action.label)?;
        let action_id = action.id.clone();
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let mut document = self
            .interfaces
            .get(&target_interface_id)
            .cloned()
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;

        let node = find_node_mut(&mut document.nodes, node_id).ok_or_else(|| {
            ControlError::UnknownNode {
                interface_id: target_interface_id.clone(),
                node_id: node_id.to_string(),
            }
        })?;
        node.action_id = Some(action_id.clone());
        document.actions.retain(|existing| existing.id != action.id);
        document.actions.push(action);

        let applied = self.apply_interface(document)?;
        self.push_event(
            RuntimeEventKind::NodeUpdated,
            Some(applied.interface_id.clone()),
            Some(applied.scope.clone()),
            Some(action_id),
            Some(node_id.to_string()),
            Some("semantic action binding applied".to_string()),
        );
        Ok(applied)
    }

    pub fn edit_interface(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        operations: Vec<InterfaceEditOperation>,
        dry_run: bool,
    ) -> Result<EditedInterface> {
        if operations.is_empty() {
            return Err(ControlError::EmptyField {
                field: "operations",
            });
        }

        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let mut document = self
            .interfaces
            .get(&target_interface_id)
            .cloned()
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;
        let mut results = Vec::new();

        for (index, operation) in operations.into_iter().enumerate() {
            let result = apply_interface_edit_operation(
                &target_interface_id,
                &mut document,
                index,
                operation,
            )?;
            results.push(result);
        }

        validate_interface(&document)?;
        let applied = AppliedInterface {
            scope: document.scope.clone(),
            interface_id: document.id.clone(),
        };

        if dry_run {
            return Ok(EditedInterface {
                applied,
                dry_run: true,
                operations: results,
            });
        }

        let applied = self.apply_interface(document)?;
        for result in &results {
            self.push_event(
                runtime_event_kind_for_edit_operation(&result.operation),
                Some(applied.interface_id.clone()),
                Some(applied.scope.clone()),
                result.action_id.clone(),
                result.node_id.clone(),
                Some(result.message.clone()),
            );
        }

        Ok(EditedInterface {
            applied,
            dry_run: false,
            operations: results,
        })
    }

    pub fn patch_interface_layout(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        node_id: Option<&str>,
        properties: BTreeMap<String, String>,
    ) -> Result<PatchedInterfaceLayout> {
        let properties = normalized_layout_properties(properties);
        if properties.is_empty() {
            return Err(ControlError::EmptyField {
                field: "layout_properties",
            });
        }

        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let mut document = self
            .interfaces
            .get(&target_interface_id)
            .cloned()
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;

        let target_node_id = match node_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
        {
            Some(node_id) => node_id,
            None => document
                .nodes
                .first()
                .map(|node| node.id.clone())
                .ok_or(ControlError::EmptyInterface)?,
        };

        let node = find_node_mut(&mut document.nodes, &target_node_id).ok_or_else(|| {
            ControlError::UnknownNode {
                interface_id: target_interface_id.clone(),
                node_id: target_node_id.clone(),
            }
        })?;

        let prior_properties = node.properties.clone();
        for (key, value) in properties {
            node.properties.insert(key, value);
        }
        let applied_properties = node.properties.clone();

        let applied = self.apply_interface(document)?;
        self.push_event(
            RuntimeEventKind::LayoutPatched,
            Some(applied.interface_id.clone()),
            Some(applied.scope.clone()),
            None,
            Some(target_node_id.clone()),
            Some("layout intent patched".to_string()),
        );
        Ok(PatchedInterfaceLayout {
            applied,
            node_id: target_node_id,
            prior_properties,
            applied_properties,
        })
    }

    pub fn preview_interface_layout(
        &self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        node_id: Option<&str>,
        properties: BTreeMap<String, String>,
    ) -> Result<LayoutPreview> {
        let requested_properties = normalized_layout_properties(properties);
        if requested_properties.is_empty() {
            return Err(ControlError::EmptyField {
                field: "layout_properties",
            });
        }

        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self.interfaces.get(&target_interface_id).ok_or_else(|| {
            ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            }
        })?;

        let target_node_id = match node_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
        {
            Some(node_id) => node_id,
            None => document
                .nodes
                .first()
                .map(|node| node.id.clone())
                .ok_or(ControlError::EmptyInterface)?,
        };

        let node = find_node(&document.nodes, &target_node_id).ok_or_else(|| {
            ControlError::UnknownNode {
                interface_id: target_interface_id.clone(),
                node_id: target_node_id.clone(),
            }
        })?;

        let prior_properties = node.properties.clone();
        let mut preview_properties = prior_properties.clone();
        for (key, value) in &requested_properties {
            preview_properties.insert(key.clone(), value.clone());
        }
        let unsupported_hints = layout_preview_unsupported_hints(&preview_properties);
        let conflict_hints =
            self.layout_preview_conflict_hints(&target_interface_id, &preview_properties);
        let estimated_reserved_space = layout_preview_reserved_space(&preview_properties);
        let viewport_fit =
            layout_preview_viewport_fit(&preview_properties, &estimated_reserved_space);
        let overflow_estimate =
            layout_preview_overflow_estimate(document, &estimated_reserved_space);
        let scene_plan = self.layout_scene_plan_with_preview(Some((
            target_interface_id.as_str(),
            target_node_id.as_str(),
            &preview_properties,
        )));
        let fit_score = layout_preview_fit_score(
            &preview_properties,
            &unsupported_hints,
            &conflict_hints,
            &viewport_fit,
        );

        Ok(LayoutPreview {
            target: AppliedInterface {
                scope: document.scope.clone(),
                interface_id: target_interface_id.clone(),
            },
            node_id: target_node_id,
            prior_properties,
            scene_plan,
            estimated_reserved_space,
            viewport_fit,
            overflow_estimate,
            fit_score,
            unsupported_hints,
            conflict_hints,
            requested_properties,
            preview_properties,
        })
    }

    pub fn layout_scene_plan(&self) -> LayoutScenePlan {
        self.layout_scene_plan_with_preview(None)
    }

    pub fn patch_interface_lifecycle(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        patch: InterfaceLifecyclePatch,
    ) -> Result<PatchedInterfaceLifecycle> {
        if patch.is_empty() {
            return Err(ControlError::EmptyField {
                field: "lifecycle_patch",
            });
        }

        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let scope = self
            .interfaces
            .get(&target_interface_id)
            .map(|document| document.scope.clone())
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;
        let prior = self
            .lifecycle_by_interface
            .get(&target_interface_id)
            .cloned()
            .unwrap_or_else(|| InterfaceLifecycleState::new(target_interface_id.clone()));
        let mut state = prior.clone();
        state.interface_id = target_interface_id.clone();

        if let Some(pinned) = patch.pinned {
            state.pinned = pinned;
        }

        if patch.clear_expiration {
            state.expired = false;
            state.expires_at_unix = None;
            state.ttl_seconds = None;
        }

        if let Some(ttl_seconds) = patch.ttl_seconds {
            state.ttl_seconds = Some(ttl_seconds);
            if let Some(now_unix) = patch.now_unix {
                let expires_at = now_unix.saturating_add(ttl_seconds);
                state.expires_at_unix = Some(expires_at);
                state.expired = expires_at <= now_unix;
            }
        }

        if let Some(expires_at_unix) = patch.expires_at_unix {
            state.expires_at_unix = Some(expires_at_unix);
            if let Some(now_unix) = patch.now_unix {
                state.expired = expires_at_unix <= now_unix;
            }
        }

        if let Some(hidden) = patch.hidden {
            if state.hidden != hidden {
                state.previous_hidden = Some(state.hidden);
            }
            state.hidden = hidden;
        }

        if patch.expire_now {
            if !state.hidden {
                state.previous_hidden = Some(false);
            }
            state.expired = true;
            state.hidden = true;
        }

        if patch.restore_previous {
            let restored_hidden = state.previous_hidden.unwrap_or(false);
            if state.hidden != restored_hidden {
                state.previous_hidden = Some(state.hidden);
            }
            state.hidden = restored_hidden;
            state.expired = false;
            state.expires_at_unix = None;
            state.ttl_seconds = None;
        }

        state.message = Some(lifecycle_message(&state));
        self.lifecycle_by_interface
            .insert(target_interface_id.clone(), state.clone());

        self.push_event(
            RuntimeEventKind::LifecyclePatched,
            Some(target_interface_id.clone()),
            Some(scope.clone()),
            None,
            None,
            state.message.clone(),
        );

        Ok(PatchedInterfaceLifecycle {
            interface_id: target_interface_id,
            scope,
            prior,
            state,
        })
    }

    pub fn replace_interface(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        document: InterfaceDocument,
        preserve_lifecycle: bool,
    ) -> Result<ReplacedInterface> {
        validate_interface(&document)?;
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let prior_lifecycle = self
            .lifecycle_by_interface
            .get(&target_interface_id)
            .cloned();
        let replacement_id = document.id.clone();

        if target_interface_id != replacement_id {
            self.interfaces.remove(&target_interface_id);
            self.actions_by_interface.remove(&target_interface_id);
            self.last_dispatched_action_by_interface
                .remove(&target_interface_id);
            if self
                .last_dispatched_action
                .as_ref()
                .and_then(|action| action.interface_id.as_deref())
                == Some(target_interface_id.as_str())
            {
                self.last_dispatched_action = None;
            }
            self.last_render_pass_by_interface
                .remove(&target_interface_id);
            self.pending_render_event_interfaces
                .remove(&target_interface_id);
            self.active_interface_by_scope
                .retain(|_, active_id| active_id != &target_interface_id);
            self.lifecycle_by_interface.remove(&target_interface_id);
            self.rebuild_action_index();
        }

        let applied = self.apply_interface(document)?;
        let lifecycle_preserved = if preserve_lifecycle {
            if let Some(mut lifecycle) = prior_lifecycle {
                lifecycle.interface_id = applied.interface_id.clone();
                self.lifecycle_by_interface
                    .insert(applied.interface_id.clone(), lifecycle);
                true
            } else {
                false
            }
        } else {
            false
        };

        self.push_event(
            RuntimeEventKind::InterfaceReplaced,
            Some(applied.interface_id.clone()),
            Some(applied.scope.clone()),
            None,
            None,
            Some(format!(
                "interface {} replaced by {}",
                target_interface_id, applied.interface_id
            )),
        );

        Ok(ReplacedInterface {
            previous_interface_id: target_interface_id,
            applied,
            lifecycle_preserved,
        })
    }

    pub fn retire_interface(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
    ) -> Result<RetiredInterface> {
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self
            .interfaces
            .remove(&target_interface_id)
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;

        let mut retired_scope_keys = Vec::new();
        self.active_interface_by_scope
            .retain(|scope_key, active_id| {
                if active_id == &target_interface_id {
                    retired_scope_keys.push(scope_key.clone());
                    false
                } else {
                    true
                }
            });
        retired_scope_keys.sort();

        self.actions_by_interface.remove(&target_interface_id);
        self.last_dispatched_action_by_interface
            .remove(&target_interface_id);
        if self
            .last_dispatched_action
            .as_ref()
            .and_then(|action| action.interface_id.as_deref())
            == Some(target_interface_id.as_str())
        {
            self.last_dispatched_action = None;
        }
        self.last_render_pass_by_interface
            .remove(&target_interface_id);
        self.lifecycle_by_interface.remove(&target_interface_id);
        self.pending_render_event_interfaces
            .remove(&target_interface_id);
        self.rebuild_action_index();

        self.push_event(
            RuntimeEventKind::InterfaceRetired,
            Some(target_interface_id.clone()),
            Some(document.scope.clone()),
            None,
            None,
            Some("interface retired from native runtime".to_string()),
        );

        Ok(RetiredInterface {
            interface_id: target_interface_id,
            scope: document.scope,
            retired_scope_keys,
            remaining_interfaces: self.interfaces.len(),
        })
    }

    pub fn request_refresh(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        action_id: Option<&str>,
        permission_granted: bool,
        external_execution_requested: bool,
    ) -> Result<RequestedRefresh> {
        self.request_refresh_with_provenance(
            interface_id,
            scope,
            action_id,
            permission_granted,
            external_execution_requested,
            ActionProvenance::default(),
        )
    }

    pub fn request_refresh_with_provenance(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        action_id: Option<&str>,
        permission_granted: bool,
        external_execution_requested: bool,
        provenance: ActionProvenance,
    ) -> Result<RequestedRefresh> {
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self.interfaces.get(&target_interface_id).ok_or_else(|| {
            ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            }
        })?;

        let selected_action = match action_id.map(str::trim).filter(|value| !value.is_empty()) {
            Some(action_id) => {
                let action = document
                    .actions
                    .iter()
                    .find(|action| action.id == action_id)
                    .ok_or_else(|| ControlError::UnregisteredAction {
                        action_id: action_id.to_string(),
                    })?;
                Some(action.clone())
            }
            None => document
                .actions
                .iter()
                .find(|action| action.kind == ActionKind::Refresh)
                .cloned(),
        };

        let approval_state = if external_execution_requested {
            if permission_granted {
                RequestApprovalState::Granted
            } else {
                RequestApprovalState::Pending
            }
        } else {
            RequestApprovalState::NotRequired
        };
        let selected_action_id = selected_action
            .as_ref()
            .map(|action| action.id.clone())
            .unwrap_or_else(|| "refresh".to_string());
        let request_id = if external_execution_requested {
            Some(provenance.request_id.clone().unwrap_or_else(|| {
                generated_request_id(
                    "refresh",
                    &target_interface_id,
                    &selected_action_id,
                    self.next_event_sequence + 1,
                )
            }))
        } else {
            provenance.request_id.clone()
        };

        let message = match (external_execution_requested, approval_state) {
            (true, RequestApprovalState::Granted) => {
                "refresh intent approved and recorded; native OWT does not execute external commands directly"
            }
            (true, RequestApprovalState::Pending) => {
                "refresh execution requested; pending terminal-owned approval"
            }
            _ => "refresh intent recorded without external execution",
        }
        .to_string();

        let requested = RequestedRefresh {
            interface_id: target_interface_id,
            scope: document.scope.clone(),
            action_id: selected_action.as_ref().map(|action| action.id.clone()),
            label: selected_action.as_ref().map(|action| action.label.clone()),
            kind: selected_action.as_ref().map(|action| action.kind.clone()),
            target: selected_action
                .as_ref()
                .and_then(|action| action.target.clone()),
            command: selected_action
                .as_ref()
                .and_then(|action| action.command.clone()),
            source: provenance.source.clone(),
            request_id,
            requested_at_unix: provenance.requested_at_unix,
            approval_state,
            approval_required: approval_state == RequestApprovalState::Pending,
            permission_granted: approval_state == RequestApprovalState::Granted,
            external_execution_requested,
            executed: false,
            message,
        };

        self.push_refresh_request(requested.clone());
        self.push_event(
            RuntimeEventKind::RefreshRequested,
            Some(requested.interface_id.clone()),
            Some(requested.scope.clone()),
            requested.action_id.clone(),
            None,
            Some(requested.message.clone()),
        );

        Ok(requested)
    }

    pub fn approve_permission_request(&mut self, request_id: &str) -> Result<PermissionDecision> {
        self.set_permission_request_state(request_id, RequestApprovalState::Granted)
    }

    pub fn deny_permission_request(&mut self, request_id: &str) -> Result<PermissionDecision> {
        self.set_permission_request_state(request_id, RequestApprovalState::Denied)
    }

    fn set_permission_request_state(
        &mut self,
        request_id: &str,
        approval_state: RequestApprovalState,
    ) -> Result<PermissionDecision> {
        require_non_empty("request_id", request_id)?;
        if let Some(index) = self
            .action_requests
            .iter()
            .position(|request| request.request_id.as_deref() == Some(request_id))
        {
            let needs_confirmation = {
                let request = &self.action_requests[index];
                request
                    .interface_id
                    .as_deref()
                    .and_then(|interface_id| {
                        self.actions_by_interface
                            .get(interface_id)
                            .and_then(|actions| actions.get(&request.action_id))
                    })
                    .map(|action| {
                        action.requires_confirmation
                            || matches!(
                                &action.kind,
                                ActionKind::Run
                                    | ActionKind::Network
                                    | ActionKind::Destructive
                                    | ActionKind::CredentialSensitive
                            )
                    })
                    .unwrap_or_else(|| {
                        matches!(
                            &request.kind,
                            ActionKind::Run
                                | ActionKind::Network
                                | ActionKind::Destructive
                                | ActionKind::CredentialSensitive
                        )
                    })
            };
            let (interface_id, scope, action_id, message) = {
                let request = &mut self.action_requests[index];
                if request.approval_state != RequestApprovalState::Pending {
                    return Err(ControlError::PermissionRequestAlreadyResolved {
                        request_id: request_id.to_string(),
                        approval_state: approval_state_name(request.approval_state).to_string(),
                    });
                }
                request.approval_state = approval_state;
                request.approval_required = false;
                request.permission_granted = approval_state == RequestApprovalState::Granted;
                request.confirmation_granted =
                    approval_state == RequestApprovalState::Granted && needs_confirmation;
                request.execution_status = Some(match approval_state {
                    RequestApprovalState::Granted => "recorded_not_executed".to_string(),
                    RequestApprovalState::Denied => "denied".to_string(),
                    _ => approval_state_name(approval_state).to_string(),
                });
                request.execution_message = Some(match approval_state {
                    RequestApprovalState::Granted => {
                        "external execution approved by native OWT; native OWT did not run the command"
                    }
                    RequestApprovalState::Denied => {
                        "external execution denied by native OWT"
                    }
                    _ => "permission request state updated by native OWT",
                }
                .to_string());
                (
                    request.interface_id.clone().unwrap_or_default(),
                    request.scope.clone(),
                    Some(request.action_id.clone()),
                    request.execution_message.clone().unwrap_or_default(),
                )
            };
            self.push_event(
                if approval_state == RequestApprovalState::Granted {
                    RuntimeEventKind::PermissionGranted
                } else {
                    RuntimeEventKind::PermissionDenied
                },
                Some(interface_id.clone()),
                scope,
                action_id.clone(),
                None,
                Some(message.clone()),
            );
            return Ok(PermissionDecision {
                request_id: request_id.to_string(),
                request_kind: "action".to_string(),
                approval_state,
                interface_id,
                action_id,
                message,
            });
        }

        if let Some(index) = self
            .refresh_requests
            .iter()
            .position(|request| request.request_id.as_deref() == Some(request_id))
        {
            let (interface_id, scope, action_id, message) = {
                let request = &mut self.refresh_requests[index];
                if request.approval_state != RequestApprovalState::Pending {
                    return Err(ControlError::PermissionRequestAlreadyResolved {
                        request_id: request_id.to_string(),
                        approval_state: approval_state_name(request.approval_state).to_string(),
                    });
                }
                request.approval_state = approval_state;
                request.approval_required = false;
                request.permission_granted = approval_state == RequestApprovalState::Granted;
                request.message = match approval_state {
                    RequestApprovalState::Granted => {
                        "refresh execution approved by native OWT; native OWT did not run external commands directly"
                    }
                    RequestApprovalState::Denied => "refresh execution denied by native OWT",
                    _ => "refresh permission request state updated by native OWT",
                }
                .to_string();
                (
                    request.interface_id.clone(),
                    Some(request.scope.clone()),
                    request.action_id.clone(),
                    request.message.clone(),
                )
            };
            self.push_event(
                if approval_state == RequestApprovalState::Granted {
                    RuntimeEventKind::PermissionGranted
                } else {
                    RuntimeEventKind::PermissionDenied
                },
                Some(interface_id.clone()),
                scope,
                action_id.clone(),
                None,
                Some(message.clone()),
            );
            return Ok(PermissionDecision {
                request_id: request_id.to_string(),
                request_kind: "refresh".to_string(),
                approval_state,
                interface_id,
                action_id,
                message,
            });
        }

        Err(ControlError::UnknownPermissionRequest {
            request_id: request_id.to_string(),
        })
    }

    pub fn focus_table_row_relative(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        movement: TableFocusMovement,
    ) -> Result<FocusedTableRow> {
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self
            .interfaces
            .get_mut(&target_interface_id)
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;
        let scope = document.scope.clone();

        for node in &mut document.nodes {
            if let Some(selection) = focus_table_row_relative_in_node(node, movement) {
                let focused = FocusedTableRow {
                    interface_id: target_interface_id.clone(),
                    scope: scope.clone(),
                    table_node_id: selection.table_node_id,
                    row_node_id: selection.row_node_id,
                    focused_row: selection.focused_row,
                    focused_group: selection.focused_group,
                    movement,
                };
                self.push_event(
                    RuntimeEventKind::NodeUpdated,
                    Some(focused.interface_id.clone()),
                    Some(scope),
                    None,
                    Some(
                        focused
                            .row_node_id
                            .clone()
                            .unwrap_or_else(|| focused.table_node_id.clone()),
                    ),
                    Some(format!(
                        "table row focus moved {}",
                        focused.movement.as_str()
                    )),
                );
                return Ok(focused);
            }
        }

        Err(ControlError::NoFocusableTableRows {
            interface_id: target_interface_id,
        })
    }

    pub fn focus_table_cell_relative(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        movement: TableCellFocusMovement,
    ) -> Result<FocusedTableCell> {
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self
            .interfaces
            .get_mut(&target_interface_id)
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;
        let scope = document.scope.clone();

        for node in &mut document.nodes {
            if let Some(selection) = focus_table_cell_relative_in_node(node, movement) {
                let focused = FocusedTableCell {
                    interface_id: target_interface_id.clone(),
                    scope: scope.clone(),
                    table_node_id: selection.table_node_id,
                    focused_column: selection.focused_column,
                    column_index: selection.column_index,
                    movement: Some(movement),
                };
                self.push_event(
                    RuntimeEventKind::NodeUpdated,
                    Some(focused.interface_id.clone()),
                    Some(scope),
                    None,
                    Some(focused.table_node_id.clone()),
                    Some(format!(
                        "table cell focus moved {} to {}",
                        movement.as_str(),
                        focused.focused_column
                    )),
                );
                return Ok(focused);
            }
        }

        Err(ControlError::NoFocusableTableColumns {
            interface_id: target_interface_id,
        })
    }

    pub fn focus_table_cell(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
        column: &str,
    ) -> Result<FocusedTableCell> {
        require_non_empty("column", column)?;
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self
            .interfaces
            .get_mut(&target_interface_id)
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;
        let scope = document.scope.clone();

        for node in &mut document.nodes {
            if let Some(selection) = focus_table_cell_in_node(node, column) {
                let focused = FocusedTableCell {
                    interface_id: target_interface_id.clone(),
                    scope: scope.clone(),
                    table_node_id: selection.table_node_id,
                    focused_column: selection.focused_column,
                    column_index: selection.column_index,
                    movement: None,
                };
                self.push_event(
                    RuntimeEventKind::NodeUpdated,
                    Some(focused.interface_id.clone()),
                    Some(scope),
                    None,
                    Some(focused.table_node_id.clone()),
                    Some(format!(
                        "table cell focus set to {}",
                        focused.focused_column
                    )),
                );
                return Ok(focused);
            }
        }

        Err(ControlError::NoFocusableTableColumns {
            interface_id: target_interface_id,
        })
    }

    pub fn toggle_focused_table_group_mode(
        &mut self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
    ) -> Result<FocusedTableGroup> {
        let target_interface_id = self.resolve_update_interface_id(interface_id, scope)?;
        let document = self
            .interfaces
            .get_mut(&target_interface_id)
            .ok_or_else(|| ControlError::UnknownInterface {
                interface_id: target_interface_id.clone(),
            })?;
        let scope = document.scope.clone();

        for node in &mut document.nodes {
            if let Some(selection) = toggle_focused_table_group_mode_in_node(node) {
                let focused = FocusedTableGroup {
                    interface_id: target_interface_id.clone(),
                    scope: scope.clone(),
                    table_node_id: selection.table_node_id,
                    focused_group: selection.focused_group,
                    active: selection.active,
                    previous_focus_mode: selection.previous_focus_mode,
                    previous_expanded_group: selection.previous_expanded_group,
                };
                self.push_event(
                    RuntimeEventKind::NodeUpdated,
                    Some(focused.interface_id.clone()),
                    Some(scope),
                    None,
                    Some(focused.table_node_id.clone()),
                    Some(format!(
                        "table group focus mode {} {}",
                        if focused.active {
                            "entered"
                        } else {
                            "restored"
                        },
                        focused.focused_group
                    )),
                );
                return Ok(focused);
            }
        }

        Err(ControlError::NoFocusedTableGroup {
            interface_id: target_interface_id,
        })
    }

    fn resolve_update_interface_id(
        &self,
        interface_id: Option<&str>,
        scope: Option<&Scope>,
    ) -> Result<String> {
        if let Some(interface_id) = interface_id.filter(|value| !value.trim().is_empty()) {
            if self.interfaces.contains_key(interface_id) {
                return Ok(interface_id.to_string());
            }
            return Err(ControlError::UnknownInterface {
                interface_id: interface_id.to_string(),
            });
        }

        if let Some(scope) = scope {
            return self
                .active_interface_by_scope
                .get(&scope.key())
                .cloned()
                .ok_or(ControlError::NoActiveInterface);
        }

        self.active_interface_by_scope
            .values()
            .next()
            .cloned()
            .ok_or(ControlError::NoActiveInterface)
    }

    fn layout_preview_conflict_hints(
        &self,
        target_interface_id: &str,
        preview_properties: &BTreeMap<String, String>,
    ) -> Vec<String> {
        let preview_slot = layout_preview_reservation_slot(preview_properties);
        if !preview_slot.reserves_terminal_space {
            return Vec::new();
        }

        let mut hints = Vec::new();
        let mut seen = BTreeSet::new();
        for interface_id in self.active_interface_by_scope.values() {
            if interface_id == target_interface_id || !seen.insert(interface_id) {
                continue;
            }
            let Some(document) = self.interfaces.get(interface_id) else {
                continue;
            };
            let Some(root) = document.nodes.first() else {
                continue;
            };
            let existing_slot = layout_preview_reservation_slot(&root.properties);
            if existing_slot.reserves_terminal_space
                && existing_slot.slot == preview_slot.slot
                && preview_slot.slot != "overlay"
            {
                hints.push(format!(
                    "reserved {} placement already has active interface {}; current reservation merge uses max edge space, not stacked layout",
                    preview_slot.slot, interface_id
                ));
            }
        }

        hints
    }

    fn layout_scene_plan_with_preview(
        &self,
        preview: Option<(&str, &str, &BTreeMap<String, String>)>,
    ) -> LayoutScenePlan {
        let mut documents = Vec::new();
        for interface_id in self.active_interface_ids_for_scene() {
            let Some(document) = self.interfaces.get(&interface_id) else {
                continue;
            };
            let mut document = document.clone();
            if let Some((preview_interface_id, preview_node_id, preview_properties)) = preview {
                if preview_interface_id == interface_id {
                    if let Some(root) = document.nodes.first_mut() {
                        root.id = preview_node_id.to_string();
                        root.properties = preview_properties.clone();
                    }
                }
            }
            documents.push(document);
        }

        reflow_interface_documents_for_scene(&mut documents);
        let surfaces = documents
            .iter()
            .filter_map(|document| {
                let root = document.nodes.first()?;
                Some(layout_scene_surface(document, &root.id, &root.properties))
            })
            .collect();

        layout_scene_plan_from_surfaces(surfaces)
    }

    pub fn last_dispatched_action_for_interface(
        &self,
        interface_id: &str,
    ) -> Option<DispatchedAction> {
        self.last_dispatched_action_by_interface
            .get(interface_id)
            .cloned()
    }

    fn focus_table_row_for_action(
        &mut self,
        interface_id: &str,
        action_id: &str,
    ) -> Option<String> {
        let document = self.interfaces.get_mut(interface_id)?;
        for node in &mut document.nodes {
            if let Some(node_id) = focus_table_row_for_action_in_node(node, action_id) {
                return Some(node_id);
            }
        }
        None
    }

    pub fn recent_events(&self, limit: usize) -> Vec<RuntimeEvent> {
        let len = self.events.len();
        let start = len.saturating_sub(limit);
        self.events[start..].to_vec()
    }

    pub fn recent_action_requests(
        &self,
        interface_id: Option<&str>,
        limit: usize,
    ) -> Vec<DispatchedAction> {
        recent_matching_items(&self.action_requests, limit, |request| {
            interface_id
                .map(|interface_id| request.interface_id.as_deref() == Some(interface_id))
                .unwrap_or(true)
        })
    }

    pub fn recent_refresh_requests(
        &self,
        interface_id: Option<&str>,
        limit: usize,
    ) -> Vec<RequestedRefresh> {
        recent_matching_items(&self.refresh_requests, limit, |request| {
            interface_id
                .map(|interface_id| request.interface_id == interface_id)
                .unwrap_or(true)
        })
    }

    fn push_action_request(&mut self, request: DispatchedAction) {
        self.action_requests.push(request);
        trim_front(&mut self.action_requests, MAX_ACTION_REQUESTS);
    }

    fn push_refresh_request(&mut self, request: RequestedRefresh) {
        self.refresh_requests.push(request);
        trim_front(&mut self.refresh_requests, MAX_ACTION_REQUESTS);
    }

    pub fn record_rendered_interfaces(&mut self, interface_ids: &[String], render_pass: u64) {
        for interface_id in interface_ids {
            self.last_render_pass_by_interface
                .insert(interface_id.clone(), render_pass);
            if self.pending_render_event_interfaces.remove(interface_id) {
                let scope = self
                    .interfaces
                    .get(interface_id)
                    .map(|document| document.scope.clone());
                self.push_event(
                    RuntimeEventKind::Rendered,
                    Some(interface_id.clone()),
                    scope,
                    None,
                    None,
                    Some(format!("native renderer reached pass {render_pass}")),
                );
            }
        }
    }

    fn resolve_dispatch_action(
        &self,
        interface_id: Option<&str>,
        action_id: &str,
    ) -> Result<(String, Scope, UiAction)> {
        if let Some(interface_id) = interface_id.filter(|value| !value.trim().is_empty()) {
            let document = self.interfaces.get(interface_id).ok_or_else(|| {
                ControlError::UnknownInterface {
                    interface_id: interface_id.to_string(),
                }
            })?;
            let action = document
                .actions
                .iter()
                .find(|action| action.id == action_id)
                .cloned()
                .ok_or_else(|| ControlError::UnregisteredAction {
                    action_id: action_id.to_string(),
                })?;
            return Ok((interface_id.to_string(), document.scope.clone(), action));
        }

        let mut matches = Vec::new();
        for (candidate_interface_id, document) in &self.interfaces {
            if let Some(action) = document
                .actions
                .iter()
                .find(|action| action.id == action_id)
                .cloned()
            {
                matches.push((
                    candidate_interface_id.clone(),
                    document.scope.clone(),
                    action,
                ));
            }
        }

        match matches.len() {
            0 => Err(ControlError::UnregisteredAction {
                action_id: action_id.to_string(),
            }),
            1 => Ok(matches.remove(0)),
            _ => Err(ControlError::AmbiguousAction {
                action_id: action_id.to_string(),
                interface_ids: matches
                    .into_iter()
                    .map(|(interface_id, _, _)| interface_id)
                    .collect::<Vec<_>>()
                    .join(","),
            }),
        }
    }

    fn push_event(
        &mut self,
        kind: RuntimeEventKind,
        interface_id: Option<String>,
        scope: Option<Scope>,
        action_id: Option<String>,
        node_id: Option<String>,
        message: Option<String>,
    ) {
        self.next_event_sequence = self.next_event_sequence.saturating_add(1);
        self.events.push(RuntimeEvent {
            sequence: self.next_event_sequence,
            kind,
            interface_id,
            scope,
            action_id,
            node_id,
            message,
        });
        if self.events.len() > MAX_RUNTIME_EVENTS {
            let drop_count = self.events.len() - MAX_RUNTIME_EVENTS;
            self.events.drain(0..drop_count);
        }
    }

    fn rebuild_action_index(&mut self) {
        self.actions.clear();
        for document in self.interfaces.values() {
            for action in &document.actions {
                self.actions.insert(action.id.clone(), action.clone());
            }
        }
    }
}

fn normalized_layout_properties(properties: BTreeMap<String, String>) -> BTreeMap<String, String> {
    properties
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
}

fn recent_matching_items<T, F>(items: &[T], limit: usize, mut matches: F) -> Vec<T>
where
    T: Clone,
    F: FnMut(&T) -> bool,
{
    let limit = normalized_request_limit(limit);
    let mut recent = items
        .iter()
        .rev()
        .filter(|item| matches(item))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    recent.reverse();
    recent
}

fn normalized_request_limit(limit: usize) -> usize {
    if limit == 0 {
        24
    } else {
        limit.min(MAX_ACTION_REQUESTS)
    }
}

fn trim_front<T>(items: &mut Vec<T>, max_len: usize) {
    if items.len() > max_len {
        let drop_count = items.len() - max_len;
        items.drain(0..drop_count);
    }
}

fn find_node_mut<'a>(nodes: &'a mut [UiNode], node_id: &str) -> Option<&'a mut UiNode> {
    for node in nodes {
        if node.id == node_id {
            return Some(node);
        }
        if let Some(child) = find_node_mut(&mut node.children, node_id) {
            return Some(child);
        }
    }
    None
}

fn find_node<'a>(nodes: &'a [UiNode], node_id: &str) -> Option<&'a UiNode> {
    for node in nodes {
        if node.id == node_id {
            return Some(node);
        }
        if let Some(child) = find_node(&node.children, node_id) {
            return Some(child);
        }
    }
    None
}

fn apply_interface_edit_operation(
    interface_id: &str,
    document: &mut InterfaceDocument,
    index: usize,
    operation: InterfaceEditOperation,
) -> Result<InterfaceEditOperationResult> {
    match operation {
        InterfaceEditOperation::AddNode {
            parent_id,
            node,
            action,
            replace,
        } => {
            require_non_empty("node.id", &node.id)?;
            let node_id = node.id.clone();
            let parent_id = normalized_optional_id(parent_id);
            if let Some(action) = action {
                upsert_document_action(document, action);
            }
            if let Some(existing) = find_node_mut(&mut document.nodes, &node_id) {
                if !replace {
                    return Err(ControlError::DuplicateId {
                        kind: "node",
                        id: node_id,
                    });
                }
                *existing = node;
                return Ok(edit_result(
                    index,
                    "replace_node",
                    Some(existing.id.clone()),
                    parent_id,
                    None,
                    "semantic node replaced during live build",
                ));
            }
            insert_node_at(document, interface_id, parent_id.as_deref(), node, None)?;
            Ok(edit_result(
                index,
                "add_node",
                Some(node_id),
                parent_id,
                None,
                "semantic node added during live build",
            ))
        }
        InterfaceEditOperation::ReplaceNode { node, action } => {
            require_non_empty("node.id", &node.id)?;
            let node_id = node.id.clone();
            if let Some(action) = action {
                upsert_document_action(document, action);
            }
            let existing = find_node_mut(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            *existing = node;
            Ok(edit_result(
                index,
                "replace_node",
                Some(node_id),
                None,
                None,
                "semantic node replaced during live build",
            ))
        }
        InterfaceEditOperation::PatchNode {
            node_id,
            label,
            text,
            role,
            action_id,
            provenance,
            properties,
            remove_properties,
            clear_fields,
        } => {
            let node_id = required_node_id(node_id)?;
            let node = find_node_mut(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            for field in clear_fields {
                clear_node_field(node, &field);
            }
            if let Some(label) = label {
                node.label = Some(label);
            }
            if let Some(text) = text {
                node.text = Some(text);
            }
            if let Some(role) = role {
                node.role = Some(role);
            }
            if let Some(action_id) = action_id {
                node.action_id = normalized_optional_id(Some(action_id));
            }
            if let Some(provenance) = provenance {
                node.provenance = Some(provenance);
            }
            for key in remove_properties {
                node.properties.remove(key.trim());
            }
            for (key, value) in properties {
                let key = key.trim();
                if !key.is_empty() {
                    node.properties.insert(key.to_string(), value);
                }
            }
            Ok(edit_result(
                index,
                "patch_node",
                Some(node_id),
                None,
                None,
                "semantic node fields patched during live build",
            ))
        }
        InterfaceEditOperation::RemoveNode {
            node_id,
            prune_orphan_actions,
        } => {
            let node_id = required_node_id(node_id)?;
            let removed = remove_node_by_id(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            if prune_orphan_actions.unwrap_or(true) {
                prune_unreferenced_actions(document);
            }
            Ok(edit_result(
                index,
                "remove_node",
                Some(removed.id),
                None,
                None,
                "semantic node removed during live build",
            ))
        }
        InterfaceEditOperation::MoveNode {
            node_id,
            parent_id,
            position,
        } => {
            let node_id = required_node_id(node_id)?;
            let parent_id = normalized_optional_id(parent_id);
            let moving = remove_node_by_id(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            insert_node_at(
                document,
                interface_id,
                parent_id.as_deref(),
                moving,
                position,
            )?;
            Ok(edit_result(
                index,
                "move_node",
                Some(node_id),
                parent_id,
                None,
                "semantic node moved during live build",
            ))
        }
        InterfaceEditOperation::ClearChildren { node_id } => {
            let node_id = node_id
                .and_then(|value| normalized_optional_id(Some(value)))
                .or_else(|| document.nodes.first().map(|node| node.id.clone()))
                .ok_or(ControlError::EmptyInterface)?;
            let node = find_node_mut(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            node.children.clear();
            Ok(edit_result(
                index,
                "clear_children",
                Some(node_id),
                None,
                None,
                "semantic node children cleared during live build",
            ))
        }
        InterfaceEditOperation::BindAction { node_id, action } => {
            let node_id = required_node_id(node_id)?;
            require_non_empty("action.id", &action.id)?;
            require_non_empty("action.label", &action.label)?;
            let action_id = action.id.clone();
            let node = find_node_mut(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            node.action_id = Some(action_id.clone());
            upsert_document_action(document, action);
            Ok(edit_result(
                index,
                "bind_action",
                Some(node_id),
                None,
                Some(action_id),
                "semantic action binding patched during live build",
            ))
        }
        InterfaceEditOperation::HighlightNode {
            node_id,
            mode,
            label,
            duration_ms,
        } => {
            let node_id = required_node_id(node_id)?;
            let node = find_node_mut(&mut document.nodes, &node_id).ok_or_else(|| {
                ControlError::UnknownNode {
                    interface_id: interface_id.to_string(),
                    node_id: node_id.clone(),
                }
            })?;
            node.properties.insert(
                "owt_builder_highlight".to_string(),
                mode.unwrap_or_else(|| "pulse".to_string()),
            );
            if let Some(label) = label {
                node.properties
                    .insert("owt_builder_highlight_label".to_string(), label);
            }
            if let Some(duration_ms) = duration_ms {
                node.properties.insert(
                    "owt_builder_highlight_duration_ms".to_string(),
                    duration_ms.to_string(),
                );
            }
            Ok(edit_result(
                index,
                "highlight_node",
                Some(node_id),
                None,
                None,
                "semantic node highlighted during live build",
            ))
        }
    }
}

fn insert_node_at(
    document: &mut InterfaceDocument,
    interface_id: &str,
    parent_id: Option<&str>,
    node: UiNode,
    position: Option<usize>,
) -> Result<()> {
    if let Some(parent_id) = parent_id {
        let parent = find_node_mut(&mut document.nodes, parent_id).ok_or_else(|| {
            ControlError::UnknownParent {
                interface_id: interface_id.to_string(),
                parent_id: parent_id.to_string(),
            }
        })?;
        insert_child_at(&mut parent.children, node, position);
    } else {
        insert_child_at(&mut document.nodes, node, position);
    }
    Ok(())
}

fn insert_child_at(children: &mut Vec<UiNode>, node: UiNode, position: Option<usize>) {
    let index = position.unwrap_or(children.len()).min(children.len());
    children.insert(index, node);
}

fn remove_node_by_id(nodes: &mut Vec<UiNode>, node_id: &str) -> Option<UiNode> {
    if let Some(index) = nodes.iter().position(|node| node.id == node_id) {
        return Some(nodes.remove(index));
    }
    for node in nodes {
        if let Some(removed) = remove_node_by_id(&mut node.children, node_id) {
            return Some(removed);
        }
    }
    None
}

fn upsert_document_action(document: &mut InterfaceDocument, action: UiAction) {
    document.actions.retain(|existing| existing.id != action.id);
    document.actions.push(action);
}

fn prune_unreferenced_actions(document: &mut InterfaceDocument) {
    let mut referenced = BTreeSet::new();
    collect_referenced_actions(&document.nodes, &mut referenced);
    document
        .actions
        .retain(|action| referenced.contains(&action.id));
}

fn collect_referenced_actions(nodes: &[UiNode], referenced: &mut BTreeSet<String>) {
    for node in nodes {
        if let Some(action_id) = &node.action_id {
            referenced.insert(action_id.clone());
        }
        collect_referenced_actions(&node.children, referenced);
    }
}

fn clear_node_field(node: &mut UiNode, field: &str) {
    match field.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "label" => node.label = None,
        "text" => node.text = None,
        "role" => node.role = None,
        "action" | "action_id" => node.action_id = None,
        "provenance" => node.provenance = None,
        _ => {}
    }
}

fn required_node_id(node_id: Option<String>) -> Result<String> {
    normalized_optional_id(node_id).ok_or(ControlError::EmptyField { field: "node_id" })
}

fn normalized_optional_id(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn edit_result(
    index: usize,
    operation: &str,
    node_id: Option<String>,
    parent_id: Option<String>,
    action_id: Option<String>,
    message: &str,
) -> InterfaceEditOperationResult {
    InterfaceEditOperationResult {
        index,
        operation: operation.to_string(),
        node_id,
        parent_id,
        action_id,
        message: message.to_string(),
    }
}

fn runtime_event_kind_for_edit_operation(operation: &str) -> RuntimeEventKind {
    match operation {
        "add_node" => RuntimeEventKind::NodeAdded,
        "replace_node" => RuntimeEventKind::NodeReplaced,
        "patch_node" => RuntimeEventKind::NodePatched,
        "remove_node" => RuntimeEventKind::NodeRemoved,
        "move_node" => RuntimeEventKind::NodeMoved,
        "highlight_node" => RuntimeEventKind::NodeHighlighted,
        _ => RuntimeEventKind::NodeUpdated,
    }
}

struct LayoutReservationSlot {
    slot: &'static str,
    reserves_terminal_space: bool,
}

const ESTIMATED_TOP_RESERVED_ROWS: u16 = 8;
const ESTIMATED_COMPACT_BUTTON_RESERVED_ROWS: u16 = 3;
const ESTIMATED_ACTION_STRIP_RESERVED_ROWS: u16 = 4;
const ESTIMATED_STRUCTURAL_TOP_RESERVED_ROWS: u16 = 18;
const ESTIMATED_SIDE_RESERVED_COLUMNS: u16 = 28;
const ESTIMATED_STRUCTURAL_SIDE_RESERVED_COLUMNS: u16 = 24;
const ESTIMATED_BOTTOM_RESERVED_ROWS: u16 = 8;
const ESTIMATED_BLOCK_BOTTOM_RESERVED_ROWS: u16 = 12;

fn layout_preview_reservation_slot(properties: &BTreeMap<String, String>) -> LayoutReservationSlot {
    let slot = string_property(
        properties,
        &[
            "placement",
            "dock",
            "docking",
            "origin",
            "anchor",
            "panel_origin",
            "surface_origin",
            "layout",
        ],
    )
    .map(normalized_layout_slot)
    .unwrap_or("top");
    let reservation = string_property(properties, &["reservation", "reserve", "terminal_space"])
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let reserves_terminal_space = slot != "overlay"
        && !matches!(
            reservation.as_str(),
            "overlay"
                | "floating"
                | "free"
                | "undocked"
                | "widget"
                | "false"
                | "0"
                | "no"
                | "none"
                | "passthrough"
        );

    LayoutReservationSlot {
        slot,
        reserves_terminal_space,
    }
}

fn normalized_layout_slot(value: &str) -> &'static str {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "left" | "left_rail" | "rail_left" | "docked_left" | "west" => "left",
        "right" | "right_rail" | "rail_right" | "docked_right" | "east" => "right",
        "bottom" | "bottom_strip" | "docked_bottom" | "bottom_right" | "right_bottom" | "south"
        | "lower" => "bottom",
        "overlay" | "hud" | "floating" | "heads_up" | "heads_up_display" | "center" | "centre"
        | "focus" | "free" | "undocked" | "widget" => "overlay",
        _ => "top",
    }
}

fn layout_preview_reserved_space(
    properties: &BTreeMap<String, String>,
) -> LayoutReservedSpaceEstimate {
    let reservation_slot = layout_preview_reservation_slot(properties);
    let profile = string_property(
        properties,
        &[
            "profile",
            "layout_profile",
            "composition_profile",
            "information_shape",
        ],
    )
    .unwrap_or_default()
    .trim()
    .to_ascii_lowercase()
    .replace(['-', ' '], "_");

    let structural_profile = matches!(
        profile.as_str(),
        "structural_console" | "fleet_matrix" | "incident_summary" | "queue_triage"
    );
    let block_profile = matches!(
        profile.as_str(),
        "block_composition" | "semantic_blocks" | "thelcars_control_panel"
    );
    let compact_button_profile = matches!(
        profile.as_str(),
        "corner_button"
            | "cycle_render_shortcut"
            | "floating_button"
            | "single_button"
            | "shortcut_button"
            | "action_button"
            | "folder_opener"
    );
    let action_strip_profile = matches!(
        profile.as_str(),
        "action_strip" | "button_strip" | "simple_buttons" | "folder_buttons" | "folder_open_strip"
    );

    let (columns, rows) = if !reservation_slot.reserves_terminal_space {
        (0, 0)
    } else {
        match reservation_slot.slot {
            "left" | "right" if structural_profile => {
                (ESTIMATED_STRUCTURAL_SIDE_RESERVED_COLUMNS, 0)
            }
            "left" | "right" => (ESTIMATED_SIDE_RESERVED_COLUMNS, 0),
            "bottom" if compact_button_profile => (0, ESTIMATED_COMPACT_BUTTON_RESERVED_ROWS),
            "bottom" if action_strip_profile => (0, ESTIMATED_ACTION_STRIP_RESERVED_ROWS),
            "bottom" if block_profile => (0, ESTIMATED_BLOCK_BOTTOM_RESERVED_ROWS),
            "bottom" => (0, ESTIMATED_BOTTOM_RESERVED_ROWS),
            _ if structural_profile => (
                ESTIMATED_STRUCTURAL_SIDE_RESERVED_COLUMNS,
                ESTIMATED_STRUCTURAL_TOP_RESERVED_ROWS,
            ),
            _ if compact_button_profile => (0, ESTIMATED_COMPACT_BUTTON_RESERVED_ROWS),
            _ if action_strip_profile => (0, ESTIMATED_ACTION_STRIP_RESERVED_ROWS),
            _ => (0, ESTIMATED_TOP_RESERVED_ROWS),
        }
    };

    LayoutReservedSpaceEstimate {
        slot: reservation_slot.slot.to_string(),
        reserves_terminal_space: reservation_slot.reserves_terminal_space,
        columns,
        rows,
        basis: "first_pass_static_cells_without_live_viewport".to_string(),
    }
}

fn layout_preview_viewport_fit(
    properties: &BTreeMap<String, String>,
    reserved_space: &LayoutReservedSpaceEstimate,
) -> LayoutViewportFit {
    let viewport_columns = u16_property(
        properties,
        &[
            "viewport_columns",
            "viewport_cols",
            "terminal_columns",
            "terminal_cols",
            "view_columns",
            "view_cols",
        ],
    );
    let viewport_rows = u16_property(
        properties,
        &[
            "viewport_rows",
            "terminal_rows",
            "view_rows",
            "viewport_lines",
            "terminal_lines",
        ],
    );
    let (minimum_columns, minimum_rows) = minimum_terminal_cells(properties);
    let mut factors = Vec::new();

    let terminal_columns_after_reservation = viewport_columns.map(|columns| {
        if reserved_space.reserves_terminal_space {
            columns.saturating_sub(reserved_space.columns)
        } else {
            columns
        }
    });
    let terminal_rows_after_reservation = viewport_rows.map(|rows| {
        if reserved_space.reserves_terminal_space {
            rows.saturating_sub(reserved_space.rows)
        } else {
            rows
        }
    });

    if viewport_columns.is_none() && viewport_rows.is_none() {
        factors.push("viewport cells were not provided".to_string());
        return LayoutViewportFit {
            status: "not_provided".to_string(),
            viewport_columns,
            viewport_rows,
            terminal_columns_after_reservation,
            terminal_rows_after_reservation,
            minimum_columns,
            minimum_rows,
            reserved_columns: reserved_space.columns,
            reserved_rows: reserved_space.rows,
            overflow_columns: 0,
            overflow_rows: 0,
            basis: "viewport_cells_not_provided".to_string(),
            factors,
        };
    }

    if !reserved_space.reserves_terminal_space {
        factors.push("overlay/floating placement leaves terminal cells unchanged".to_string());
    } else {
        factors.push(format!(
            "reserved estimate consumes {} column(s) and {} row(s)",
            reserved_space.columns, reserved_space.rows
        ));
    }

    if let (Some(columns), Some(rows)) = (
        terminal_columns_after_reservation,
        terminal_rows_after_reservation,
    ) {
        factors.push(format!(
            "terminal cells after reservation are {columns}x{rows}"
        ));
    }

    let overflow_columns = match (minimum_columns, terminal_columns_after_reservation) {
        (Some(minimum), Some(columns)) => minimum.saturating_sub(columns),
        _ => 0,
    };
    let overflow_rows = match (minimum_rows, terminal_rows_after_reservation) {
        (Some(minimum), Some(rows)) => minimum.saturating_sub(rows),
        _ => 0,
    };
    if minimum_columns.is_some() || minimum_rows.is_some() {
        factors.push(format!(
            "minimum terminal cells are {}x{}",
            minimum_columns
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            minimum_rows
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ));
    }

    let no_terminal_space = matches!(terminal_columns_after_reservation, Some(0))
        || matches!(terminal_rows_after_reservation, Some(0));
    let status = if no_terminal_space {
        "no_terminal_space"
    } else if overflow_columns > 0 || overflow_rows > 0 {
        "too_small"
    } else if !reserved_space.reserves_terminal_space {
        "overlay_unreserved"
    } else {
        "fits"
    };

    LayoutViewportFit {
        status: status.to_string(),
        viewport_columns,
        viewport_rows,
        terminal_columns_after_reservation,
        terminal_rows_after_reservation,
        minimum_columns,
        minimum_rows,
        reserved_columns: reserved_space.columns,
        reserved_rows: reserved_space.rows,
        overflow_columns,
        overflow_rows,
        basis: "first_pass_viewport_cells_from_request_properties".to_string(),
        factors,
    }
}

fn layout_scene_surface(
    document: &InterfaceDocument,
    node_id: &str,
    properties: &BTreeMap<String, String>,
) -> LayoutSceneSurface {
    let visible = layout_properties_visible(properties);
    let mut reserved_space = layout_preview_reserved_space(properties);
    if !visible {
        reserved_space.slot = "hidden".to_string();
        reserved_space.reserves_terminal_space = false;
        reserved_space.columns = 0;
        reserved_space.rows = 0;
    }
    let overflow = layout_preview_overflow_estimate(document, &reserved_space);
    let action_slots = document
        .actions
        .iter()
        .take(NATIVE_KEYBOARD_ACTION_LIMIT)
        .enumerate()
        .map(|(index, action)| LayoutSceneActionSlot {
            slot_index: index + 1,
            action_id: action.id.clone(),
            label: action.label.clone(),
            kind: action.kind.clone(),
            automatic_shortcut: format!("Ctrl+Alt+Shift+{}", index + 1),
        })
        .collect();
    let overflow_action_ids = document
        .actions
        .iter()
        .skip(NATIVE_KEYBOARD_ACTION_LIMIT)
        .map(|action| action.id.clone())
        .collect();
    let action_page_count = action_page_count_for_action_count(document.actions.len());
    let requested_slot = string_property(
        properties,
        &[
            "owt_requested_slot",
            "requested_slot",
            "requested_dock",
            "requested_placement",
        ],
    )
    .map(ToString::to_string);
    let solved_slot = string_property(
        properties,
        &[
            "owt_solved_slot",
            "solved_slot",
            "actual_slot",
            "actual_dock",
        ],
    )
    .map(ToString::to_string);
    let reflow_reason = string_property(properties, &["owt_layout_reflow", "layout_reflow_reason"])
        .map(ToString::to_string);
    let collapse_state = string_property(properties, &["owt_layout_collapse", "collapse_state"])
        .map(ToString::to_string);
    let collapse_policy = layout_collapse_policy(properties);

    LayoutSceneSurface {
        interface_id: document.id.clone(),
        scope: document.scope.clone(),
        node_id: node_id.to_string(),
        title: document.title.clone(),
        visible,
        slot: reserved_space.slot,
        requested_slot,
        solved_slot,
        reflow_reason,
        collapse_state,
        collapse_policy,
        priority: layout_surface_priority(document, 0),
        reserves_terminal_space: reserved_space.reserves_terminal_space,
        estimated_columns: reserved_space.columns,
        estimated_rows: reserved_space.rows,
        action_count: overflow.action_count,
        action_page_count,
        automatic_action_slots_per_page: NATIVE_KEYBOARD_ACTION_LIMIT,
        overflow_action_count: overflow.overflow_action_count,
        table_count: overflow.table_count,
        table_row_count: overflow.table_row_count,
        estimated_hidden_table_rows: overflow.estimated_hidden_table_rows,
        action_slots,
        overflow_action_ids,
    }
}

pub fn reflow_interface_documents_for_scene(documents: &mut [InterfaceDocument]) {
    let mut occupied_slots: BTreeMap<String, usize> = BTreeMap::new();

    for index in 0..documents.len() {
        let Some(root) = documents[index].nodes.first() else {
            continue;
        };
        if !layout_properties_visible(&root.properties) {
            continue;
        }
        let reserved_space = layout_preview_reserved_space(&root.properties);
        if !reserved_space.reserves_terminal_space {
            continue;
        }
        let current_slot = reserved_space.slot.clone();
        if let Some(occupied_index) = occupied_slots.get(&current_slot).copied() {
            resolve_slot_conflict(documents, &mut occupied_slots, occupied_index, index);
        } else {
            occupied_slots.insert(current_slot, index);
        }
    }
}

fn resolve_slot_conflict(
    documents: &mut [InterfaceDocument],
    occupied_slots: &mut BTreeMap<String, usize>,
    occupied_index: usize,
    current_index: usize,
) {
    let Some(current_root) = documents[current_index].nodes.first() else {
        return;
    };
    let current_slot = layout_preview_reserved_space(&current_root.properties).slot;
    let occupied_priority = layout_surface_priority(&documents[occupied_index], occupied_index);
    let current_priority = layout_surface_priority(&documents[current_index], current_index);
    let current_should_win = current_priority > occupied_priority;

    if current_should_win && layout_surface_can_move_or_collapse(&documents[occupied_index]) {
        if move_or_collapse_surface(documents, occupied_slots, occupied_index, &current_slot) {
            occupied_slots.insert(current_slot, current_index);
            return;
        }
    }

    if layout_surface_can_move_or_collapse(&documents[current_index]) {
        let _ = move_or_collapse_surface(documents, occupied_slots, current_index, &current_slot);
    }
}

fn move_or_collapse_surface(
    documents: &mut [InterfaceDocument],
    occupied_slots: &mut BTreeMap<String, usize>,
    index: usize,
    previous_slot: &str,
) -> bool {
    if let Some(target_slot) = first_available_reflow_slot(occupied_slots, previous_slot) {
        reflow_document_root_to_slot(&mut documents[index], previous_slot, target_slot);
        occupied_slots.insert(target_slot.to_string(), index);
        return true;
    }

    collapse_document_root_for_scene(&mut documents[index], previous_slot)
}

fn layout_surface_can_move_or_collapse(document: &InterfaceDocument) -> bool {
    !layout_reflow_locked(document)
        && (compact_auxiliary_surface(document)
            || !matches!(
                document
                    .nodes
                    .first()
                    .and_then(|root| layout_collapse_policy(&root.properties))
                    .as_deref(),
                Some("preserve")
            ))
}

fn layout_surface_priority(document: &InterfaceDocument, order_index: usize) -> i32 {
    let Some(root) = document.nodes.first() else {
        return 0;
    };
    if let Some(priority) = i32_property(
        &root.properties,
        &["priority", "layout_priority", "surface_priority"],
    ) {
        return priority;
    }
    if property_bool(&root.properties, &["lifecycle_pinned", "pinned"]) == Some(true) {
        return 90;
    }
    let profile = string_property(
        &root.properties,
        &[
            "profile",
            "layout_profile",
            "composition_profile",
            "information_shape",
            "surface_kind",
            "widget_kind",
            "shape",
        ],
    )
    .unwrap_or_default()
    .trim()
    .to_ascii_lowercase()
    .replace(['-', ' '], "_");
    let base = match profile.as_str() {
        "structural_console"
        | "fleet_matrix"
        | "incident_summary"
        | "queue_triage"
        | "project_status"
        | "artifact_browser"
        | "block_composition"
        | "semantic_blocks"
        | "thelcars_control_panel" => 70,
        "action_strip" | "button_strip" | "simple_buttons" | "folder_buttons"
        | "folder_open_strip" => 40,
        "corner_button"
        | "cycle_render_shortcut"
        | "floating_button"
        | "single_button"
        | "shortcut_button"
        | "action_button"
        | "folder_opener" => 20,
        _ => 50,
    };
    base + (100usize.saturating_sub(order_index).min(20) as i32)
}

fn layout_collapse_policy(properties: &BTreeMap<String, String>) -> Option<String> {
    string_property(
        properties,
        &["collapse_policy", "collapse", "overflow_policy"],
    )
    .map(|value| value.trim().to_ascii_lowercase().replace(['-', ' '], "_"))
    .filter(|value| !value.is_empty())
}

fn action_page_count_for_action_count(action_count: usize) -> usize {
    action_count.max(1).div_ceil(NATIVE_KEYBOARD_ACTION_LIMIT)
}

fn document_action_paging_enabled(document: &InterfaceDocument) -> bool {
    if document.actions.len() <= NATIVE_KEYBOARD_ACTION_LIMIT {
        return false;
    }
    let Some(root) = document.nodes.first() else {
        return true;
    };
    property_bool(
        &root.properties,
        &[
            "action_paging",
            "actions_paged",
            "native_action_paging",
            "keyboard_action_paging",
        ],
    )
    .unwrap_or(true)
}

fn collapse_document_root_for_scene(document: &mut InterfaceDocument, previous_slot: &str) -> bool {
    let compact = compact_auxiliary_surface(document);
    let Some(root) = document.nodes.first_mut() else {
        return false;
    };
    let policy = layout_collapse_policy(&root.properties).unwrap_or_else(|| {
        if compact {
            "menu".to_string()
        } else {
            "summary".to_string()
        }
    });
    if policy == "preserve" {
        return false;
    }
    root.properties
        .insert("owt_requested_slot".to_string(), previous_slot.to_string());
    root.properties
        .insert("owt_layout_collapse".to_string(), policy.clone());
    root.properties
        .insert("display".to_string(), "hidden".to_string());
    root.properties
        .insert("owt_solved_slot".to_string(), "hidden".to_string());
    true
}

fn compact_auxiliary_surface(document: &InterfaceDocument) -> bool {
    document
        .nodes
        .first()
        .and_then(|root| {
            string_property(
                &root.properties,
                &[
                    "profile",
                    "layout_profile",
                    "composition_profile",
                    "information_shape",
                    "surface_kind",
                    "widget_kind",
                    "shape",
                ],
            )
        })
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
            matches!(
                normalized.as_str(),
                "corner_button"
                    | "cycle_render_shortcut"
                    | "floating_button"
                    | "single_button"
                    | "shortcut_button"
                    | "action_button"
                    | "folder_opener"
                    | "action_strip"
                    | "button_strip"
                    | "simple_buttons"
                    | "folder_buttons"
                    | "folder_open_strip"
            )
        })
        .unwrap_or(false)
}

fn layout_reflow_locked(document: &InterfaceDocument) -> bool {
    document
        .nodes
        .first()
        .map(|root| {
            property_bool(
                &root.properties,
                &[
                    "layout_reflow_locked",
                    "lock_layout",
                    "pinned_layout",
                    "disable_auto_reflow",
                    "no_auto_reflow",
                ],
            ) == Some(true)
        })
        .unwrap_or(false)
}

fn first_available_reflow_slot(
    occupied_slots: &BTreeMap<String, usize>,
    previous_slot: &str,
) -> Option<&'static str> {
    available_reflow_slots(previous_slot)
        .iter()
        .copied()
        .find(|slot| !occupied_slots.contains_key(*slot))
}

fn available_reflow_slots(previous_slot: &str) -> &'static [&'static str] {
    match previous_slot {
        "top" => &["bottom", "right", "left"],
        "bottom" => &["top", "right", "left"],
        "left" => &["right", "bottom", "top"],
        "right" => &["left", "bottom", "top"],
        _ => &["bottom", "right", "left", "top"],
    }
}

fn reflow_document_root_to_slot(
    document: &mut InterfaceDocument,
    previous_slot: &str,
    target_slot: &str,
) {
    let Some(root) = document.nodes.first_mut() else {
        return;
    };
    for key in [
        "placement",
        "dock",
        "docking",
        "origin",
        "anchor",
        "panel_origin",
        "surface_origin",
    ] {
        root.properties.remove(key);
    }
    root.properties
        .insert("owt_requested_slot".to_string(), previous_slot.to_string());
    root.properties
        .insert("dock".to_string(), target_slot.to_string());
    root.properties
        .insert("orientation".to_string(), "horizontal".to_string());
    root.properties
        .insert("owt_solved_slot".to_string(), target_slot.to_string());
    root.properties.insert(
        "owt_layout_reflow".to_string(),
        format!("{previous_slot}->{target_slot}"),
    );
}

fn layout_scene_plan_from_surfaces(surfaces: Vec<LayoutSceneSurface>) -> LayoutScenePlan {
    let mut slot_entries: BTreeMap<String, Vec<&LayoutSceneSurface>> = BTreeMap::new();
    let mut conflict_hints = Vec::new();
    let mut estimated_left_columns = 0;
    let mut estimated_right_columns = 0;
    let mut estimated_top_rows = 0;
    let mut estimated_bottom_rows = 0;
    let mut reserved_surface_count = 0;

    for surface in &surfaces {
        slot_entries
            .entry(surface.slot.clone())
            .or_default()
            .push(surface);
        if !surface.visible || !surface.reserves_terminal_space {
            continue;
        }
        reserved_surface_count += 1;
        match surface.slot.as_str() {
            "left" => {
                estimated_left_columns = estimated_left_columns.max(surface.estimated_columns)
            }
            "right" => {
                estimated_right_columns = estimated_right_columns.max(surface.estimated_columns)
            }
            "bottom" => estimated_bottom_rows = estimated_bottom_rows.max(surface.estimated_rows),
            "top" => {
                estimated_top_rows = estimated_top_rows.max(surface.estimated_rows);
                if surface.estimated_columns > 0 {
                    estimated_left_columns = estimated_left_columns.max(surface.estimated_columns);
                }
            }
            _ => {}
        }
    }

    let mut slots = Vec::new();
    for (slot, entries) in slot_entries {
        let reserved_entries = entries
            .iter()
            .filter(|surface| surface.visible && surface.reserves_terminal_space)
            .count();
        let conflict = !matches!(slot.as_str(), "overlay" | "hidden") && reserved_entries > 1;
        let interface_ids = entries
            .iter()
            .map(|surface| surface.interface_id.clone())
            .collect::<Vec<_>>();
        if conflict {
            conflict_hints.push(format!(
                "reserved {} placement has {} active interfaces: {}",
                slot,
                reserved_entries,
                interface_ids.join(",")
            ));
        }
        let estimated_columns = entries
            .iter()
            .map(|surface| surface.estimated_columns)
            .max()
            .unwrap_or(0);
        let estimated_rows = entries
            .iter()
            .map(|surface| surface.estimated_rows)
            .max()
            .unwrap_or(0);
        slots.push(LayoutSceneSlot {
            slot,
            interface_count: entries.len(),
            reserved_surface_count: reserved_entries,
            estimated_columns,
            estimated_rows,
            conflict,
            interface_ids,
        });
    }

    LayoutScenePlan {
        active_interface_count: surfaces.len(),
        surface_count: surfaces.len(),
        reserved_surface_count,
        estimated_left_columns,
        estimated_right_columns,
        estimated_top_rows,
        estimated_bottom_rows,
        basis: "active_runtime_interfaces_first_pass_static_scene".to_string(),
        slots,
        surfaces,
        conflict_hints,
    }
}

fn layout_preview_overflow_estimate(
    document: &InterfaceDocument,
    reserved_space: &LayoutReservedSpaceEstimate,
) -> LayoutOverflowEstimate {
    let action_count = document.actions.len();
    let table_count = count_table_nodes(&document.nodes);
    let table_row_count = count_table_rows(&document.nodes);
    let per_table_visible_rows = match reserved_space.slot.as_str() {
        "left" | "right" => 8,
        "bottom" => 4,
        _ if reserved_space.columns > 0 && reserved_space.rows > 0 => 12,
        _ => 10,
    };
    let estimated_visible_table_rows = if table_count == 0 {
        0
    } else {
        table_count * per_table_visible_rows
    };

    LayoutOverflowEstimate {
        action_count,
        automatic_action_slots: NATIVE_KEYBOARD_ACTION_LIMIT,
        overflow_action_count: action_count.saturating_sub(NATIVE_KEYBOARD_ACTION_LIMIT),
        table_count,
        table_row_count,
        estimated_visible_table_rows,
        estimated_hidden_table_rows: table_row_count.saturating_sub(estimated_visible_table_rows),
        basis: "first_pass_static_counts_without_live_viewport".to_string(),
    }
}

fn count_table_nodes(nodes: &[UiNode]) -> usize {
    nodes
        .iter()
        .map(|node| usize::from(node.kind == UiNodeKind::Table) + count_table_nodes(&node.children))
        .sum()
}

fn count_table_rows(nodes: &[UiNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            let current = if node.kind == UiNodeKind::Table {
                let property_rows = node
                    .properties
                    .get("rows")
                    .map(|rows| rows.lines().filter(|line| !line.trim().is_empty()).count())
                    .unwrap_or(0);
                let child_rows = node
                    .children
                    .iter()
                    .filter(|child| {
                        child.kind == UiNodeKind::DataCascade
                            || child.properties.contains_key("cells")
                            || child.properties.contains_key("focus_key")
                    })
                    .count();
                property_rows + child_rows
            } else {
                0
            };
            current + count_table_rows(&node.children)
        })
        .sum()
}

fn layout_properties_visible(properties: &BTreeMap<String, String>) -> bool {
    let display = string_property(properties, &["display"])
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if matches!(display.as_str(), "hide" | "hidden" | "none" | "off") {
        return false;
    }
    if property_bool(properties, &["hidden"]) == Some(true) {
        return false;
    }
    if property_bool(properties, &["visible", "active", "enabled"]) == Some(false) {
        return false;
    }
    true
}

fn layout_preview_unsupported_hints(properties: &BTreeMap<String, String>) -> Vec<String> {
    let mut hints = Vec::new();

    let placement = string_property(
        properties,
        &[
            "placement",
            "dock",
            "docking",
            "origin",
            "anchor",
            "panel_origin",
            "surface_origin",
            "layout",
        ],
    )
    .unwrap_or_default()
    .to_ascii_lowercase();
    if matches!(placement.as_str(), "focus" | "curved") {
        hints.push(
            "current native renderer records this placement intent, but focus/curved placement is not yet a first-class paint path".to_string(),
        );
    }

    let reservation = string_property(properties, &["reservation", "reserve", "terminal_space"])
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        reservation.as_str(),
        "overlay" | "floating" | "free" | "undocked" | "widget" | "false" | "0" | "no"
    ) {
        hints.push(
            "overlay/floating placement does not reserve terminal grid; final overlap depends on the renderer viewport".to_string(),
        );
    }

    for key in ["z_order", "z", "layer"] {
        if properties.contains_key(key) {
            hints.push(format!(
                "{key} is recorded for spatial planning, but is not yet used for live collision resolution"
            ));
        }
    }

    hints
}

fn property_bool(properties: &BTreeMap<String, String>, names: &[&str]) -> Option<bool> {
    string_property(properties, names).and_then(|value| {
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "on" | "show" | "visible" | "active" | "enabled" => {
                Some(true)
            }
            "false" | "0" | "no" | "n" | "off" | "hide" | "hidden" | "none" | "inactive"
            | "disabled" => Some(false),
            _ => None,
        }
    })
}

fn layout_preview_fit_score(
    properties: &BTreeMap<String, String>,
    unsupported_hints: &[String],
    conflict_hints: &[String],
    viewport_fit: &LayoutViewportFit,
) -> LayoutFitScore {
    let reservation_slot = layout_preview_reservation_slot(properties);
    let mut score = 100i32;
    let mut factors = Vec::new();

    if !reservation_slot.reserves_terminal_space {
        score -= 10;
        factors.push("overlay/floating placement does not reserve terminal grid".to_string());
    }

    if !conflict_hints.is_empty() {
        let penalty = (conflict_hints.len() as i32 * 30).min(60);
        score -= penalty;
        factors.push(format!(
            "{} reserved-placement conflict(s) detected",
            conflict_hints.len()
        ));
    }

    if !unsupported_hints.is_empty() {
        let penalty = (unsupported_hints.len() as i32 * 10).min(40);
        score -= penalty;
        factors.push(format!(
            "{} unsupported or future layout hint(s) recorded",
            unsupported_hints.len()
        ));
    }

    match viewport_fit.status.as_str() {
        "no_terminal_space" => {
            score -= 40;
            factors.push(
                "requested reservation leaves no terminal cells in one dimension".to_string(),
            );
        }
        "too_small" => {
            score -= 20;
            factors.push("viewport estimate is smaller than minimum terminal cells".to_string());
        }
        _ => {}
    }

    if factors.is_empty() {
        factors.push(
            "no current reservation conflict or unsupported layout hint detected".to_string(),
        );
    }

    let score = score.clamp(0, 100) as u8;
    let status = if !conflict_hints.is_empty() {
        "conflicted"
    } else if !unsupported_hints.is_empty() {
        "constrained"
    } else if !reservation_slot.reserves_terminal_space {
        "overlay"
    } else if score >= 90 {
        "good"
    } else {
        "constrained"
    };

    LayoutFitScore {
        score,
        status: status.to_string(),
        reservation_slot: reservation_slot.slot.to_string(),
        reserves_terminal_space: reservation_slot.reserves_terminal_space,
        factors,
    }
}

fn string_property<'a>(
    properties: &'a BTreeMap<String, String>,
    names: &[&str],
) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| properties.get(*name))
        .map(String::as_str)
}

fn u16_property(properties: &BTreeMap<String, String>, names: &[&str]) -> Option<u16> {
    string_property(properties, names)
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|value| *value > 0)
}

fn i32_property(properties: &BTreeMap<String, String>, names: &[&str]) -> Option<i32> {
    string_property(properties, names).and_then(|value| value.trim().parse::<i32>().ok())
}

fn minimum_terminal_cells(properties: &BTreeMap<String, String>) -> (Option<u16>, Option<u16>) {
    if let Some(value) = string_property(
        properties,
        &[
            "min_terminal_cells",
            "minimum_terminal_cells",
            "min_grid_cells",
        ],
    ) {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['x', ',', ';'], " ");
        let mut parts = normalized
            .split_whitespace()
            .filter_map(|part| part.parse::<u16>().ok())
            .filter(|value| *value > 0);
        let columns = parts.next();
        let rows = parts.next();
        if columns.is_some() || rows.is_some() {
            return (columns, rows);
        }
    }
    (
        u16_property(
            properties,
            &[
                "min_terminal_columns",
                "minimum_terminal_columns",
                "min_terminal_cols",
            ],
        ),
        u16_property(
            properties,
            &[
                "min_terminal_rows",
                "minimum_terminal_rows",
                "min_terminal_lines",
            ],
        ),
    )
}

pub fn validate_interface(document: &InterfaceDocument) -> Result<()> {
    require_non_empty("interface.id", &document.id)?;
    require_non_empty("interface.title", &document.title)?;
    require_non_empty("scope.id", &document.scope.id)?;
    validate_public_text("interface.title", &document.title)?;
    if let Some(tab_title) = &document.tab_title {
        validate_public_text("interface.tab_title", tab_title)?;
    }
    if let Some(theme) = &document.theme {
        validate_public_text("interface.theme", theme)?;
    }

    if document.nodes.is_empty() {
        return Err(ControlError::EmptyInterface);
    }

    let allowed_action_kinds = document
        .allowed_action_kinds
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut action_ids = BTreeSet::new();
    for action in &document.actions {
        require_non_empty("action.id", &action.id)?;
        require_non_empty("action.label", &action.label)?;
        validate_public_text("action.label", &action.label)?;
        if let Some(command) = &action.command {
            validate_public_text("action.command", command)?;
        }
        for (index, arg) in action.argv.iter().enumerate() {
            validate_public_text(&format!("action.argv[{index}]"), arg)?;
        }
        if let Some(cwd) = &action.cwd {
            validate_public_text("action.cwd", cwd)?;
        }
        if let Some(mode) = &action.mode {
            validate_public_text("action.mode", mode)?;
        }
        if let Some(target) = &action.target {
            validate_public_text("action.target", target)?;
        }
        if !allowed_action_kinds.is_empty() && !allowed_action_kinds.contains(&action.kind) {
            return Err(ControlError::ActionKindNotAllowed {
                action_id: action.id.clone(),
                kind: action_kind_name(action),
            });
        }
        if matches!(
            action.kind,
            crate::interface::ActionKind::Network
                | crate::interface::ActionKind::Destructive
                | crate::interface::ActionKind::CredentialSensitive
        ) && !action.requires_confirmation
        {
            return Err(ControlError::ActionRequiresConfirmation {
                action_id: action.id.clone(),
                kind: action_kind_name(action),
            });
        }
        if !action_ids.insert(action.id.clone()) {
            return Err(ControlError::DuplicateId {
                kind: "action",
                id: action.id.clone(),
            });
        }
    }

    let mut node_ids = BTreeSet::new();
    for node in &document.nodes {
        validate_node(node, &action_ids, &mut node_ids)?;
    }

    Ok(())
}

pub fn interface_validation_warnings(
    document: &InterfaceDocument,
) -> Vec<InterfaceValidationWarning> {
    let automatic_action_slots = automatic_keyboard_action_slots(document);
    let action_paging_enabled = document_action_paging_enabled(document);
    let mut warnings = Vec::new();
    for node in &document.nodes {
        collect_keyboard_fallback_warnings(
            node,
            &automatic_action_slots,
            action_paging_enabled,
            &mut warnings,
        );
        collect_data_provenance_warnings(node, DataProvenancePresence::default(), &mut warnings);
    }
    collect_action_allowlist_warnings(document, &mut warnings);
    warnings
}

pub fn diff_interface_documents(
    left: &InterfaceDocument,
    right: &InterfaceDocument,
) -> InterfaceDiff {
    let mut diff = InterfaceDiff {
        left_interface_id: left.id.clone(),
        right_interface_id: right.id.clone(),
        changed: false,
        summary: Vec::new(),
        added_actions: Vec::new(),
        removed_actions: Vec::new(),
        changed_actions: Vec::new(),
        added_nodes: Vec::new(),
        removed_nodes: Vec::new(),
        changed_nodes: Vec::new(),
        changed_root_properties: Vec::new(),
        changed_facts: Vec::new(),
    };

    if left.title != right.title {
        diff.summary.push("title changed".to_string());
    }
    if left.scope != right.scope {
        diff.summary.push("scope changed".to_string());
    }
    if left.theme != right.theme {
        diff.summary.push("theme changed".to_string());
    }

    let left_actions = left
        .actions
        .iter()
        .map(|action| (action.id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let right_actions = right
        .actions
        .iter()
        .map(|action| (action.id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    for action_id in right_actions.keys() {
        if !left_actions.contains_key(action_id) {
            diff.added_actions.push(action_id.clone());
        }
    }
    for action_id in left_actions.keys() {
        if !right_actions.contains_key(action_id) {
            diff.removed_actions.push(action_id.clone());
        }
    }
    for (action_id, left_action) in &left_actions {
        if let Some(right_action) = right_actions.get(action_id) {
            if *left_action != *right_action {
                diff.changed_actions.push(action_id.clone());
            }
        }
    }

    let left_nodes = flatten_nodes(&left.nodes);
    let right_nodes = flatten_nodes(&right.nodes);
    for node_id in right_nodes.keys() {
        if !left_nodes.contains_key(node_id) {
            diff.added_nodes.push(node_id.clone());
        }
    }
    for node_id in left_nodes.keys() {
        if !right_nodes.contains_key(node_id) {
            diff.removed_nodes.push(node_id.clone());
        }
    }
    for (node_id, left_node) in &left_nodes {
        if let Some(right_node) = right_nodes.get(node_id) {
            if *left_node != *right_node {
                diff.changed_nodes.push(node_id.clone());
            }
            if let (Some(left_fact), Some(right_fact)) =
                (fact_snapshot(left_node), fact_snapshot(right_node))
            {
                if left_fact != right_fact {
                    diff.changed_facts.push(FactDiff {
                        node_id: node_id.clone(),
                        kind: ui_node_kind_name(&left_node.kind).to_string(),
                        label: left_node.label.clone().or_else(|| right_node.label.clone()),
                        left: left_fact,
                        right: right_fact,
                    });
                }
            }
        }
    }

    let left_root_properties = left
        .nodes
        .first()
        .map(|node| &node.properties)
        .cloned()
        .unwrap_or_default();
    let right_root_properties = right
        .nodes
        .first()
        .map(|node| &node.properties)
        .cloned()
        .unwrap_or_default();
    let property_keys = left_root_properties
        .keys()
        .chain(right_root_properties.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in property_keys {
        if left_root_properties.get(&key) != right_root_properties.get(&key) {
            diff.changed_root_properties.push(key);
        }
    }

    if !diff.added_actions.is_empty() {
        diff.summary
            .push(format!("{} action(s) added", diff.added_actions.len()));
    }
    if !diff.removed_actions.is_empty() {
        diff.summary
            .push(format!("{} action(s) removed", diff.removed_actions.len()));
    }
    if !diff.changed_actions.is_empty() {
        diff.summary
            .push(format!("{} action(s) changed", diff.changed_actions.len()));
    }
    if !diff.added_nodes.is_empty() {
        diff.summary
            .push(format!("{} node(s) added", diff.added_nodes.len()));
    }
    if !diff.removed_nodes.is_empty() {
        diff.summary
            .push(format!("{} node(s) removed", diff.removed_nodes.len()));
    }
    if !diff.changed_nodes.is_empty() {
        diff.summary
            .push(format!("{} node(s) changed", diff.changed_nodes.len()));
    }
    if !diff.changed_root_properties.is_empty() {
        diff.summary.push(format!(
            "{} root layout/provenance property value(s) changed",
            diff.changed_root_properties.len()
        ));
    }
    if !diff.changed_facts.is_empty() {
        diff.summary.push(format!(
            "{} operational fact(s) changed",
            diff.changed_facts.len()
        ));
    }
    diff.changed = !diff.summary.is_empty();
    diff
}

fn flatten_nodes(nodes: &[UiNode]) -> BTreeMap<String, UiNode> {
    let mut output = BTreeMap::new();
    flatten_nodes_into(nodes, &mut output);
    output
}

fn flatten_nodes_into(nodes: &[UiNode], output: &mut BTreeMap<String, UiNode>) {
    for node in nodes {
        output.insert(node.id.clone(), node.clone());
        flatten_nodes_into(&node.children, output);
    }
}

fn fact_snapshot(node: &UiNode) -> Option<String> {
    if !node_is_fact_bearing(node) {
        return None;
    }

    let mut fields = Vec::new();
    push_named_value(&mut fields, "label", node.label.as_deref());
    push_named_value(&mut fields, "text", node.text.as_deref());
    for key in [
        "value",
        "status",
        "state",
        "severity",
        "columns",
        "rows",
        "cells",
        "group",
        "cell_severity",
        "cell_provenance",
        "provenance",
        "source",
        "host",
        "confidence",
        "error",
    ] {
        push_named_value(
            &mut fields,
            key,
            node.properties.get(key).map(String::as_str),
        );
    }
    if let Some(provenance) = &node.provenance {
        push_named_value(&mut fields, "source", provenance.source.as_deref());
        push_named_value(&mut fields, "host", provenance.host.as_deref());
        push_named_value(&mut fields, "command", provenance.command.as_deref());
        push_named_value(&mut fields, "confidence", provenance.confidence.as_deref());
        push_named_value(&mut fields, "error", provenance.error.as_deref());
        if let Some(state) = &provenance.state {
            fields.push((
                "fact_state".to_string(),
                format!("{state:?}").to_ascii_lowercase(),
            ));
        }
    }

    if fields.is_empty() {
        Some(ui_node_kind_name(&node.kind).to_string())
    } else {
        Some(
            fields
                .into_iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }
}

fn push_named_value(fields: &mut Vec<(String, String)>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        fields.push((key.to_string(), value.to_string()));
    }
}

fn node_is_fact_bearing(node: &UiNode) -> bool {
    matches!(
        node.kind,
        UiNodeKind::DataCascade | UiNodeKind::Table | UiNodeKind::Metric | UiNodeKind::Progress
    )
}

fn ui_node_kind_name(kind: &UiNodeKind) -> &'static str {
    match kind {
        UiNodeKind::Panel => "panel",
        UiNodeKind::Region => "region",
        UiNodeKind::Group => "group",
        UiNodeKind::Frame => "frame",
        UiNodeKind::SideRail => "side_rail",
        UiNodeKind::ContentBay => "content_bay",
        UiNodeKind::Text => "text",
        UiNodeKind::Bar => "bar",
        UiNodeKind::BarRun => "bar_run",
        UiNodeKind::Elbow => "elbow",
        UiNodeKind::CommandGrid => "command_grid",
        UiNodeKind::DataCascade => "data_cascade",
        UiNodeKind::Button => "button",
        UiNodeKind::Badge => "badge",
        UiNodeKind::List => "list",
        UiNodeKind::Table => "table",
        UiNodeKind::Metric => "metric",
        UiNodeKind::Progress => "progress",
        UiNodeKind::Image => "image",
        UiNodeKind::Spacer => "spacer",
    }
}

fn automatic_keyboard_action_slots(document: &InterfaceDocument) -> BTreeSet<String> {
    let mut slots = Vec::new();
    let mut referenced_action_ids = BTreeSet::new();
    for node in &document.nodes {
        collect_button_action_slots(node, &mut slots, &mut referenced_action_ids);
    }
    for action in &document.actions {
        if !referenced_action_ids.contains(&action.id) {
            slots.push(action.id.clone());
        }
    }
    slots
        .into_iter()
        .filter(|action_id| !action_id.trim().is_empty())
        .take(NATIVE_KEYBOARD_ACTION_LIMIT)
        .collect()
}

fn collect_button_action_slots(
    node: &UiNode,
    slots: &mut Vec<String>,
    referenced_action_ids: &mut BTreeSet<String>,
) {
    if node.kind == UiNodeKind::Button {
        if let Some(action_id) = node
            .action_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            slots.push(action_id.clone());
            referenced_action_ids.insert(action_id.clone());
        }
    }
    for child in &node.children {
        collect_button_action_slots(child, slots, referenced_action_ids);
    }
}

fn collect_keyboard_fallback_warnings(
    node: &UiNode,
    automatic_action_slots: &BTreeSet<String>,
    action_paging_enabled: bool,
    warnings: &mut Vec<InterfaceValidationWarning>,
) {
    if node.kind == UiNodeKind::Button {
        if let Some(action_id) = node
            .action_id
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            if !automatic_action_slots.contains(action_id)
                && !action_paging_enabled
                && !node_declares_keyboard_fallback(node)
            {
                warnings.push(InterfaceValidationWarning {
                    code: "keyboard_fallback_missing".to_string(),
                    node_id: Some(node.id.clone()),
                    action_id: Some(action_id.clone()),
                    message: format!(
                        "button {} action {} is outside native Ctrl+Alt+Shift+1..9 automatic slots and has no explicit keyboard fallback property",
                        node.id, action_id
                    ),
                });
            }
        }
    }
    for child in &node.children {
        collect_keyboard_fallback_warnings(
            child,
            automatic_action_slots,
            action_paging_enabled,
            warnings,
        );
    }
}

fn node_declares_keyboard_fallback(node: &UiNode) -> bool {
    [
        "keyboard_fallback",
        "keyboard_shortcut",
        "keyboard",
        "shortcut",
        "hotkey",
        "keybinding",
        "key_binding",
        "access_key",
        "accesskey",
        "on_key",
    ]
    .iter()
    .any(|key| {
        node.properties
            .get(*key)
            .map(|value| keyboard_fallback_value_is_active(value))
            .unwrap_or(false)
    })
}

fn keyboard_fallback_value_is_active(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    !normalized.is_empty()
        && !matches!(
            normalized.as_str(),
            "0" | "false" | "no" | "none" | "off" | "disabled" | "not_available"
        )
}

#[derive(Debug, Clone, Copy, Default)]
struct DataProvenancePresence {
    source: bool,
    collected_at: bool,
}

impl DataProvenancePresence {
    fn merge_node(self, node: &UiNode) -> Self {
        Self {
            source: self.source || node_has_provenance_source(node),
            collected_at: self.collected_at || node_has_provenance_collected_at(node),
        }
    }

    fn is_sufficient(self) -> bool {
        self.source && self.collected_at
    }
}

fn collect_data_provenance_warnings(
    node: &UiNode,
    inherited: DataProvenancePresence,
    warnings: &mut Vec<InterfaceValidationWarning>,
) {
    let provenance = inherited.merge_node(node);
    if node_requires_data_provenance(node) && !provenance.is_sufficient() {
        warnings.push(InterfaceValidationWarning {
            code: "data_provenance_missing".to_string(),
            node_id: Some(node.id.clone()),
            action_id: None,
            message: format!(
                "fact-bearing node {} should declare data provenance with at least source and collected_at metadata",
                node.id
            ),
        });
    }
    for child in &node.children {
        collect_data_provenance_warnings(child, provenance, warnings);
    }
}

fn node_requires_data_provenance(node: &UiNode) -> bool {
    if !node
        .properties
        .get("provenance_required")
        .map(|value| !explicit_false_value(value))
        .unwrap_or(true)
    {
        return false;
    }
    matches!(
        node.kind,
        UiNodeKind::DataCascade | UiNodeKind::Table | UiNodeKind::Metric | UiNodeKind::Progress
    )
}

fn explicit_false_value(value: &str) -> bool {
    matches!(
        value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str(),
        "0" | "false" | "no" | "none" | "off" | "disabled" | "not_required"
    )
}

fn node_has_provenance_source(node: &UiNode) -> bool {
    node.provenance
        .as_ref()
        .and_then(|provenance| provenance.source.as_deref())
        .map(has_text)
        .unwrap_or(false)
        || node_property_has_text(
            node,
            &[
                "source",
                "data_source",
                "fact_source",
                "provenance",
                "provenance_source",
            ],
        )
}

fn node_has_provenance_collected_at(node: &UiNode) -> bool {
    node.provenance
        .as_ref()
        .and_then(|provenance| provenance.collected_at.as_deref())
        .map(has_text)
        .unwrap_or(false)
        || node_property_has_text(
            node,
            &[
                "collected_at",
                "collected_at_unix",
                "collected_at_epoch",
                "timestamp",
                "sampled_at",
                "updated_at",
            ],
        )
}

fn node_property_has_text(node: &UiNode, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        node.properties
            .get(*key)
            .map(|value| has_text(value))
            .unwrap_or(false)
    })
}

fn has_text(value: &str) -> bool {
    !value.trim().is_empty()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionAllowlistEntry {
    action_id: String,
    target_or_command: Option<String>,
}

fn collect_action_allowlist_warnings(
    document: &InterfaceDocument,
    warnings: &mut Vec<InterfaceValidationWarning>,
) {
    let allowlist = action_allowlist_entries(document);
    for action in &document.actions {
        if action_needs_explicit_allowlist(action) && !action_is_allowlisted(action, &allowlist) {
            warnings.push(InterfaceValidationWarning {
                code: "action_allowlist_missing".to_string(),
                node_id: None,
                action_id: Some(action.id.clone()),
                message: format!(
                    "action {} declares external execution intent; add a root action allowlist entry for this action target or command",
                    action.id
                ),
            });
        }
    }
}

fn action_needs_explicit_allowlist(action: &UiAction) -> bool {
    if !action.argv.is_empty() {
        return true;
    }

    if action
        .command
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }

    action
        .target
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
        && matches!(
            action.kind,
            ActionKind::Run
                | ActionKind::Network
                | ActionKind::Refresh
                | ActionKind::Destructive
                | ActionKind::CredentialSensitive
        )
}

fn action_is_allowlisted(action: &UiAction, allowlist: &[ActionAllowlistEntry]) -> bool {
    allowlist.iter().any(|entry| {
        if entry.action_id != action.id {
            return false;
        }
        match entry.target_or_command.as_deref() {
            None | Some("*") => true,
            Some(allowed) => {
                action
                    .target
                    .as_deref()
                    .map(|target| target.trim() == allowed)
                    .unwrap_or(false)
                    || action
                        .command
                        .as_deref()
                        .map(|command| command.trim() == allowed)
                        .unwrap_or(false)
                    || (!action.argv.is_empty() && action.argv.join(" ") == allowed)
            }
        }
    })
}

fn action_allowlist_entries(document: &InterfaceDocument) -> Vec<ActionAllowlistEntry> {
    document
        .nodes
        .iter()
        .flat_map(|node| {
            node.properties.iter().filter_map(|(key, value)| {
                if action_allowlist_property_key(key) {
                    Some(parse_action_allowlist_entries(value))
                } else {
                    None
                }
            })
        })
        .flatten()
        .collect()
}

fn action_allowlist_property_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "action_allowlist"
            | "allowed_actions"
            | "allowed_action_targets"
            | "external_action_allowlist"
            | "shell_action_allowlist"
            | "execution_allowlist"
    )
}

fn parse_action_allowlist_entries(value: &str) -> Vec<ActionAllowlistEntry> {
    value
        .split([',', ';', '\n'])
        .filter_map(parse_action_allowlist_entry)
        .collect()
}

fn parse_action_allowlist_entry(raw: &str) -> Option<ActionAllowlistEntry> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (action_id, target_or_command) = raw
        .split_once('=')
        .map(|(action_id, target)| {
            let target = target.trim();
            (
                action_id.trim(),
                if target.is_empty() {
                    None
                } else {
                    Some(target.to_string())
                },
            )
        })
        .unwrap_or((raw, None));
    if action_id.trim().is_empty() {
        return None;
    }
    Some(ActionAllowlistEntry {
        action_id: action_id.trim().to_string(),
        target_or_command,
    })
}

fn lifecycle_message(state: &InterfaceLifecycleState) -> String {
    let mut parts = Vec::new();
    if state.pinned {
        parts.push("pinned");
    }
    if state.hidden {
        parts.push("hidden");
    }
    if state.expired {
        parts.push("expired");
    }
    if let Some(expires_at_unix) = state.expires_at_unix {
        parts.push(if state.ttl_seconds.is_some() {
            "ttl"
        } else {
            "expires_at"
        });
        if expires_at_unix == 0 {
            parts.push("now");
        }
    }
    if parts.is_empty() {
        "lifecycle visible".to_string()
    } else {
        format!("lifecycle {}", parts.join("/"))
    }
}

fn validate_node(
    node: &UiNode,
    action_ids: &BTreeSet<String>,
    node_ids: &mut BTreeSet<String>,
) -> Result<()> {
    require_non_empty("node.id", &node.id)?;
    if let Some(label) = &node.label {
        validate_public_text(format!("node.{}.label", node.id), label)?;
    }
    if let Some(text) = &node.text {
        validate_public_text(format!("node.{}.text", node.id), text)?;
    }
    if let Some(role) = &node.role {
        validate_public_text(format!("node.{}.role", node.id), role)?;
    }
    if let Some(provenance) = &node.provenance {
        validate_provenance(&node.id, provenance)?;
    }
    for (key, value) in &node.properties {
        validate_public_property(&node.id, key, value)?;
    }
    if !node_ids.insert(node.id.clone()) {
        return Err(ControlError::DuplicateId {
            kind: "node",
            id: node.id.clone(),
        });
    }

    if let Some(action_id) = &node.action_id {
        if !action_ids.contains(action_id) {
            return Err(ControlError::UnknownAction {
                node_id: node.id.clone(),
                action_id: action_id.clone(),
            });
        }
    }

    for child in &node.children {
        validate_node(child, action_ids, node_ids)?;
    }

    Ok(())
}

fn validate_provenance(node_id: &str, provenance: &crate::interface::FactProvenance) -> Result<()> {
    if let Some(source) = &provenance.source {
        validate_public_text(format!("node.{node_id}.provenance.source"), source)?;
    }
    if let Some(collected_at) = &provenance.collected_at {
        validate_public_text(
            format!("node.{node_id}.provenance.collected_at"),
            collected_at,
        )?;
    }
    if let Some(host) = &provenance.host {
        validate_public_text(format!("node.{node_id}.provenance.host"), host)?;
    }
    if let Some(command) = &provenance.command {
        validate_public_text(format!("node.{node_id}.provenance.command"), command)?;
    }
    if let Some(confidence) = &provenance.confidence {
        validate_public_text(format!("node.{node_id}.provenance.confidence"), confidence)?;
    }
    if let Some(error) = &provenance.error {
        validate_public_text(format!("node.{node_id}.provenance.error"), error)?;
    }
    Ok(())
}

fn validate_public_property(node_id: &str, key: &str, value: &str) -> Result<()> {
    if let Some(indicator) = credential_sensitive_key_indicator(key) {
        return Err(ControlError::SensitiveContent {
            field: format!("node.{node_id}.properties.{key}"),
            indicator,
        });
    }
    validate_public_text(format!("node.{node_id}.properties.{key}"), value)
}

fn validate_public_text(field: impl Into<String>, value: &str) -> Result<()> {
    if let Some(indicator) = credential_sensitive_value_indicator(value) {
        return Err(ControlError::SensitiveContent {
            field: field.into(),
            indicator,
        });
    }
    Ok(())
}

fn credential_sensitive_key_indicator(key: &str) -> Option<String> {
    let normalized = key.to_ascii_lowercase();
    let compact = normalized
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    let terms = [
        "token",
        "accesstoken",
        "refreshtoken",
        "password",
        "passwd",
        "secret",
        "apikey",
        "apiaccesskey",
        "credential",
        "credentials",
        "privatekey",
        "authorization",
    ];
    terms
        .iter()
        .copied()
        .find(|term| compact.contains(term))
        .map(str::to_string)
}

fn credential_sensitive_value_indicator(value: &str) -> Option<String> {
    let lowered = value.to_ascii_lowercase();
    let patterns = [
        "authorization:",
        "bearer ",
        "password=",
        "passwd=",
        "secret=",
        "token=",
        "access_token=",
        "refresh_token=",
        "api_key=",
        "apikey=",
        "credential=",
        "credentials=",
        "private_key=",
        "sk-",
    ];
    patterns
        .iter()
        .copied()
        .find(|pattern| lowered.contains(pattern))
        .map(|pattern| pattern.trim().trim_end_matches('=').to_string())
}

fn focus_table_row_for_action_in_node(node: &mut UiNode, action_id: &str) -> Option<String> {
    if node.kind == UiNodeKind::Table {
        if let Some(row) = table_focusable_rows(node)
            .into_iter()
            .find(|row| row.action_id.as_deref() == Some(action_id))
        {
            set_table_focus_metadata(node, row.focus_key.clone(), row.group.clone());
            return Some(row.row_node_id.unwrap_or_else(|| node.id.clone()));
        }
    }

    for child in &mut node.children {
        if let Some(node_id) = focus_table_row_for_action_in_node(child, action_id) {
            return Some(node_id);
        }
    }
    None
}

struct TableFocusSelection {
    table_node_id: String,
    row_node_id: Option<String>,
    focused_row: String,
    focused_group: Option<String>,
}

struct TableCellFocusSelection {
    table_node_id: String,
    focused_column: String,
    column_index: usize,
}

struct TableGroupFocusSelection {
    table_node_id: String,
    focused_group: String,
    active: bool,
    previous_focus_mode: Option<String>,
    previous_expanded_group: Option<String>,
}

#[derive(Clone)]
struct TableFocusableRow {
    row_node_id: Option<String>,
    focus_key: String,
    group: Option<String>,
    action_id: Option<String>,
    match_keys: Vec<String>,
}

fn focus_table_row_relative_in_node(
    node: &mut UiNode,
    movement: TableFocusMovement,
) -> Option<TableFocusSelection> {
    if node.kind == UiNodeKind::Table {
        if let Some(selection) = focus_table_row_relative_in_table(node, movement) {
            return Some(selection);
        }
    }

    for child in &mut node.children {
        if let Some(selection) = focus_table_row_relative_in_node(child, movement) {
            return Some(selection);
        }
    }
    None
}

fn focus_table_cell_relative_in_node(
    node: &mut UiNode,
    movement: TableCellFocusMovement,
) -> Option<TableCellFocusSelection> {
    if node.kind == UiNodeKind::Table {
        if let Some(selection) = focus_table_cell_relative_in_table(node, movement) {
            return Some(selection);
        }
    }

    for child in &mut node.children {
        if let Some(selection) = focus_table_cell_relative_in_node(child, movement) {
            return Some(selection);
        }
    }
    None
}

fn focus_table_cell_in_node(node: &mut UiNode, column: &str) -> Option<TableCellFocusSelection> {
    if node.kind == UiNodeKind::Table {
        if let Some(selection) = focus_table_cell_in_table(node, column) {
            return Some(selection);
        }
    }

    for child in &mut node.children {
        if let Some(selection) = focus_table_cell_in_node(child, column) {
            return Some(selection);
        }
    }
    None
}

fn toggle_focused_table_group_mode_in_node(node: &mut UiNode) -> Option<TableGroupFocusSelection> {
    if node.kind == UiNodeKind::Table {
        if let Some(selection) = toggle_focused_table_group_mode_in_table(node) {
            return Some(selection);
        }
    }

    for child in &mut node.children {
        if let Some(selection) = toggle_focused_table_group_mode_in_node(child) {
            return Some(selection);
        }
    }
    None
}

fn focused_table_row_action_for_document(document: &InterfaceDocument) -> Option<String> {
    document
        .nodes
        .iter()
        .find_map(focused_table_row_action_in_node)
}

fn focused_table_row_action_in_node(node: &UiNode) -> Option<String> {
    if node.kind == UiNodeKind::Table {
        let focus_target = node
            .properties
            .get("focused_row")
            .or_else(|| node.properties.get("focus_row"))
            .or_else(|| node.properties.get("selected_row"))
            .map(|value| normalize_table_focus_token(value))?;
        return table_focusable_rows(node)
            .into_iter()
            .find(|row| {
                row.match_keys
                    .iter()
                    .any(|candidate| normalize_table_focus_token(candidate) == focus_target)
            })
            .and_then(|row| row.action_id);
    }

    node.children
        .iter()
        .find_map(focused_table_row_action_in_node)
}

fn toggle_focused_table_group_mode_in_table(
    table: &mut UiNode,
) -> Option<TableGroupFocusSelection> {
    let group = table_focused_group(table)?;
    let group_token = normalize_table_focus_token(&group);
    let active_group = table
        .properties
        .get("focus_mode")
        .filter(|value| {
            matches!(
                normalize_table_focus_token(value).as_str(),
                "group" | "cohort"
            )
        })
        .and_then(|_| {
            table
                .properties
                .get("expanded_group")
                .or_else(|| table.properties.get("focus_group"))
                .or_else(|| table.properties.get("selected_group"))
        })
        .map(|value| normalize_table_focus_token(value));
    let mut previous_focus_mode = table.properties.get("previous_focus_mode").cloned();
    let mut previous_expanded_group = table.properties.get("previous_expanded_group").cloned();
    let active = active_group.as_deref() != Some(group_token.as_str());

    if active {
        if !table.properties.contains_key("previous_focus_mode") {
            if let Some(value) = table.properties.get("focus_mode").cloned() {
                table
                    .properties
                    .insert("previous_focus_mode".to_string(), value);
                previous_focus_mode = table.properties.get("previous_focus_mode").cloned();
            }
        }
        if !table.properties.contains_key("previous_expanded_group") {
            if let Some(value) = table.properties.get("expanded_group").cloned() {
                table
                    .properties
                    .insert("previous_expanded_group".to_string(), value);
                previous_expanded_group = table.properties.get("previous_expanded_group").cloned();
            }
        }
        table
            .properties
            .insert("focus_mode".to_string(), "group".to_string());
        table
            .properties
            .insert("expanded_group".to_string(), group.clone());
        table
            .properties
            .insert("focused_group".to_string(), group.clone());
    } else {
        restore_optional_property(table, "focus_mode", previous_focus_mode.as_deref());
        restore_optional_property(table, "expanded_group", previous_expanded_group.as_deref());
        table.properties.remove("previous_focus_mode");
        table.properties.remove("previous_expanded_group");
    }

    Some(TableGroupFocusSelection {
        table_node_id: table.id.clone(),
        focused_group: group,
        active,
        previous_focus_mode,
        previous_expanded_group,
    })
}

fn restore_optional_property(table: &mut UiNode, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        table.properties.insert(key.to_string(), value.to_string());
    } else {
        table.properties.remove(key);
    }
}

fn table_focused_group(table: &UiNode) -> Option<String> {
    table
        .properties
        .get("focused_group")
        .or_else(|| table.properties.get("focus_group"))
        .or_else(|| table.properties.get("selected_group"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| {
            let focus_target = table
                .properties
                .get("focused_row")
                .or_else(|| table.properties.get("focus_row"))
                .or_else(|| table.properties.get("selected_row"))
                .map(|value| normalize_table_focus_token(value))?;
            table_focusable_rows(table).into_iter().find_map(|row| {
                row.match_keys
                    .iter()
                    .any(|candidate| normalize_table_focus_token(candidate) == focus_target)
                    .then_some(row.group)
                    .flatten()
            })
        })
}

fn focus_table_row_relative_in_table(
    table: &mut UiNode,
    movement: TableFocusMovement,
) -> Option<TableFocusSelection> {
    let rows = table_focusable_rows(table);
    if rows.is_empty() {
        return None;
    }

    let current = table
        .properties
        .get("focused_row")
        .or_else(|| table.properties.get("focus_row"))
        .or_else(|| table.properties.get("selected_row"))
        .map(|value| normalize_table_focus_token(value));
    let current_index = current.as_deref().and_then(|target| {
        rows.iter().position(|row| {
            row.match_keys
                .iter()
                .any(|candidate| normalize_table_focus_token(candidate) == target)
        })
    });
    let next_index = match (movement, current_index) {
        (TableFocusMovement::Previous, Some(0)) => rows.len() - 1,
        (TableFocusMovement::Previous, Some(index)) => index - 1,
        (TableFocusMovement::Previous, None) => rows.len() - 1,
        (TableFocusMovement::Next, Some(index)) => (index + 1) % rows.len(),
        (TableFocusMovement::Next, None) => 0,
        (TableFocusMovement::PreviousGroup, Some(index)) => {
            previous_table_group_index(&rows, index).unwrap_or(index)
        }
        (TableFocusMovement::PreviousGroup, None) => rows.len() - 1,
        (TableFocusMovement::NextGroup, Some(index)) => {
            next_table_group_index(&rows, index).unwrap_or(index)
        }
        (TableFocusMovement::NextGroup, None) => 0,
        (TableFocusMovement::First, _) => 0,
        (TableFocusMovement::Last, _) => rows.len() - 1,
    };

    let row = rows.get(next_index)?.clone();
    set_table_focus_metadata(table, row.focus_key.clone(), row.group.clone());
    Some(TableFocusSelection {
        table_node_id: table.id.clone(),
        row_node_id: row.row_node_id,
        focused_row: row.focus_key,
        focused_group: row.group,
    })
}

fn focus_table_cell_relative_in_table(
    table: &mut UiNode,
    movement: TableCellFocusMovement,
) -> Option<TableCellFocusSelection> {
    let columns = table_focus_columns(table);
    if columns.is_empty() {
        return None;
    }

    let current = table
        .properties
        .get("focused_column")
        .or_else(|| table.properties.get("focus_column"))
        .or_else(|| table.properties.get("selected_column"))
        .or_else(|| table.properties.get("focused_cell"))
        .or_else(|| table.properties.get("focus_cell"))
        .or_else(|| table.properties.get("selected_cell"))
        .map(|value| normalize_table_focus_token(value));
    let current_index = current.as_deref().and_then(|target| {
        columns
            .iter()
            .position(|column| normalize_table_focus_token(column) == target)
    });
    let next_index = match (movement, current_index) {
        (TableCellFocusMovement::Previous, Some(0)) => columns.len() - 1,
        (TableCellFocusMovement::Previous, Some(index)) => index - 1,
        (TableCellFocusMovement::Previous, None) => columns.len() - 1,
        (TableCellFocusMovement::Next, Some(index)) => (index + 1) % columns.len(),
        (TableCellFocusMovement::Next, None) => 0,
        (TableCellFocusMovement::First, _) => 0,
        (TableCellFocusMovement::Last, _) => columns.len() - 1,
    };
    let focused_column = columns.get(next_index)?.clone();
    set_table_cell_focus_metadata(table, focused_column.clone());
    Some(TableCellFocusSelection {
        table_node_id: table.id.clone(),
        focused_column,
        column_index: next_index,
    })
}

fn focus_table_cell_in_table(table: &mut UiNode, column: &str) -> Option<TableCellFocusSelection> {
    let columns = table_focus_columns(table);
    let target = normalize_table_focus_token(column);
    let column_index = columns
        .iter()
        .position(|candidate| normalize_table_focus_token(candidate) == target)?;
    let focused_column = columns.get(column_index)?.clone();
    set_table_cell_focus_metadata(table, focused_column.clone());
    Some(TableCellFocusSelection {
        table_node_id: table.id.clone(),
        focused_column,
        column_index,
    })
}

fn set_table_focus_metadata(table: &mut UiNode, focus_key: String, group: Option<String>) {
    table
        .properties
        .insert("focused_row".to_string(), focus_key);
    if let Some(group) = group.filter(|value| !value.trim().is_empty()) {
        table.properties.insert("focused_group".to_string(), group);
    } else {
        table.properties.remove("focused_group");
    }
}

fn set_table_cell_focus_metadata(table: &mut UiNode, focused_column: String) {
    table
        .properties
        .insert("focused_column".to_string(), focused_column);
    for alias in [
        "focus_column",
        "selected_column",
        "focused_cell",
        "focus_cell",
        "selected_cell",
    ] {
        table.properties.remove(alias);
    }
}

fn table_focusable_rows(table: &UiNode) -> Vec<TableFocusableRow> {
    let mut rows = Vec::new();
    let columns = table
        .properties
        .get("columns")
        .or_else(|| table.properties.get("headers"))
        .map(|value| split_table_focus_cells(value))
        .unwrap_or_default();
    let group_index = table_focus_group_index(table, &columns);

    if let Some(source) = table
        .properties
        .get("rows")
        .or(table.text.as_ref())
        .filter(|value| !value.trim().is_empty())
    {
        for line in split_table_focus_rows(source) {
            let cells = split_table_focus_cells(&line);
            if let Some(focus_key) = cells
                .iter()
                .map(|cell| cell.trim())
                .find(|cell| !cell.is_empty())
                .map(str::to_string)
            {
                let group = group_index
                    .and_then(|index| cells.get(index))
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                rows.push(TableFocusableRow {
                    row_node_id: None,
                    group,
                    action_id: None,
                    match_keys: cells,
                    focus_key,
                });
            }
        }
    }

    for child in &table.children {
        if let Some(row) = table_focusable_child_row(child, group_index) {
            rows.push(row);
        }
    }

    rows
}

fn table_focus_columns(table: &UiNode) -> Vec<String> {
    let columns = table
        .properties
        .get("columns")
        .or_else(|| table.properties.get("headers"))
        .map(|value| split_table_focus_cells(value))
        .unwrap_or_default();
    if !columns.is_empty() {
        return columns;
    }

    table
        .children
        .first()
        .and_then(|child| child.properties.get("cells"))
        .map(|value| split_table_focus_cells(value))
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, _)| format!("cell_{}", index + 1))
        .collect()
}

fn table_focusable_child_row(
    node: &UiNode,
    group_index: Option<usize>,
) -> Option<TableFocusableRow> {
    let cells = node
        .properties
        .get("cells")
        .map(|value| split_table_focus_cells(value))
        .unwrap_or_else(|| {
            vec![
                node.label.clone().unwrap_or_else(|| node.id.clone()),
                node.properties
                    .get("value")
                    .or_else(|| node.properties.get("state"))
                    .or_else(|| node.properties.get("status"))
                    .cloned()
                    .unwrap_or_default(),
            ]
        });
    if !cells.iter().any(|cell| !cell.trim().is_empty()) {
        return None;
    }

    let focus_key = node
        .properties
        .get("focus_key")
        .or_else(|| node.properties.get("row_key"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| {
            cells
                .iter()
                .map(|cell| cell.trim())
                .find(|cell| !cell.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| node.id.clone());
    let mut match_keys = vec![focus_key.clone(), node.id.clone()];
    let action_id = node
        .action_id
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| {
            ["action_id", "drilldown_action_id", "focus_action_id"]
                .iter()
                .find_map(|key| node.properties.get(*key))
                .filter(|value| !value.trim().is_empty())
                .cloned()
        });
    if let Some(action_id) = action_id.as_ref() {
        match_keys.push(action_id.clone());
    }
    for key in [
        "action_id",
        "drilldown_action_id",
        "focus_action_id",
        "focus_key",
        "row_key",
    ] {
        if let Some(value) = node
            .properties
            .get(key)
            .filter(|value| !value.trim().is_empty())
        {
            match_keys.push(value.clone());
        }
    }
    let group_from_cells = group_index
        .and_then(|index| cells.get(index))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    match_keys.extend(cells);
    let group = node
        .properties
        .get("group")
        .or_else(|| node.properties.get("row_group"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or(group_from_cells);

    Some(TableFocusableRow {
        row_node_id: Some(node.id.clone()),
        focus_key,
        group,
        action_id,
        match_keys,
    })
}

fn next_table_group_index(rows: &[TableFocusableRow], current_index: usize) -> Option<usize> {
    let current_group = rows.get(current_index).and_then(|row| row.group.as_deref());
    for offset in 1..rows.len() {
        let candidate_index = (current_index + offset) % rows.len();
        let candidate_group = rows
            .get(candidate_index)
            .and_then(|row| row.group.as_deref());
        if candidate_group != current_group {
            return Some(candidate_index);
        }
    }
    None
}

fn previous_table_group_index(rows: &[TableFocusableRow], current_index: usize) -> Option<usize> {
    let current_group = rows.get(current_index).and_then(|row| row.group.as_deref());
    for offset in 1..rows.len() {
        let candidate_index = (current_index + rows.len() - offset) % rows.len();
        let candidate_group = rows
            .get(candidate_index)
            .and_then(|row| row.group.as_deref());
        if candidate_group != current_group {
            return Some(candidate_index);
        }
    }
    None
}

fn table_focus_group_index(table: &UiNode, columns: &[String]) -> Option<usize> {
    let value = table
        .properties
        .get("group_by")
        .or_else(|| table.properties.get("group_column"))
        .or_else(|| table.properties.get("group"))?
        .trim();
    if value.is_empty() || matches!(normalize_table_focus_token(value).as_str(), "none" | "off") {
        return None;
    }
    if let Ok(index) = value.parse::<usize>() {
        return (index > 0).then_some(index - 1);
    }
    let normalized = normalize_table_focus_token(value);
    columns
        .iter()
        .position(|column| normalize_table_focus_token(column) == normalized)
}

fn split_table_focus_rows(source: &str) -> Vec<String> {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.contains('\n') {
        normalized
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        normalized
            .split(';')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn split_table_focus_cells(source: &str) -> Vec<String> {
    let delimiter = if source.contains('|') {
        '|'
    } else if source.contains('\t') {
        '\t'
    } else {
        ','
    };
    source
        .split(delimiter)
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_table_focus_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn require_non_empty(field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(ControlError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn action_kind_name(action: &UiAction) -> String {
    format!("{:?}", action.kind)
}

#[cfg(test)]
mod tests {
    use crate::interface::{
        ActionKind, FactProvenance, InterfaceDocument, Scope, ScopeKind, UiAction, UiNode,
        UiNodeKind,
    };

    use super::{
        interface_validation_warnings, validate_interface, ActionProvenance, ControlError,
        InterfaceEditOperation, InterfaceLifecyclePatch, RequestApprovalState, RuntimeEventKind,
        RuntimeState, TableCellFocusMovement, TableFocusMovement,
    };

    fn sample_interface() -> InterfaceDocument {
        let scope = Scope::new(ScopeKind::Project, "/home/buanzo/git/tools/data/genetica");
        let mut document = InterfaceDocument::new("genetica.local", "GENETICA", scope);
        document.theme = Some("lcars".to_string());
        document.allowed_action_kinds = vec![ActionKind::Inspect, ActionKind::Open];
        document.actions.push(UiAction::new(
            "open.workspace",
            "Open workspace",
            ActionKind::Open,
        ));

        let mut panel = UiNode::new("panel.root", UiNodeKind::Panel);
        panel.label = Some("GENETICA LOCAL ONLY".to_string());

        let mut button = UiNode::new("button.open", UiNodeKind::Button);
        button.label = Some("Open workspace".to_string());
        button.action_id = Some("open.workspace".to_string());

        panel.children.push(button);
        document.nodes.push(panel);
        document
    }

    #[test]
    fn interface_round_trips_as_json() {
        let document = sample_interface();
        let encoded = serde_json::to_string_pretty(&document).unwrap();
        let decoded: InterfaceDocument = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, document);
    }

    #[test]
    fn structural_lcars_primitives_round_trip_as_json() {
        let scope = Scope::new(ScopeKind::Project, "/tmp/owt-lcars-public-demo");
        let mut document = InterfaceDocument::new("lcars.structural", "LCARS STRUCTURAL", scope);
        let mut frame = UiNode::new("frame.main", UiNodeKind::Frame);
        frame
            .children
            .push(UiNode::new("rail.primary", UiNodeKind::SideRail));
        frame
            .children
            .push(UiNode::new("bars.header", UiNodeKind::BarRun));
        frame
            .children
            .push(UiNode::new("bay.terminal", UiNodeKind::ContentBay));
        frame
            .children
            .push(UiNode::new("cascade.status", UiNodeKind::DataCascade));
        frame
            .children
            .push(UiNode::new("commands.primary", UiNodeKind::CommandGrid));
        document.nodes.push(frame);

        validate_interface(&document).unwrap();
        let encoded = serde_json::to_string_pretty(&document).unwrap();
        assert!(encoded.contains("\"kind\": \"frame\""));
        assert!(encoded.contains("\"kind\": \"bar_run\""));
        let decoded: InterfaceDocument = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, document);
    }

    #[test]
    fn apply_interface_registers_active_state() {
        let document = sample_interface();
        let scope = document.scope.clone();

        let mut state = RuntimeState::default();
        let applied = state.apply_interface(document).unwrap();

        assert_eq!(applied.interface_id, "genetica.local");
        assert!(state.actions.contains_key("open.workspace"));
        assert_eq!(
            state.active_interface_for_scope(&scope).unwrap().title,
            "GENETICA"
        );
    }

    #[test]
    fn apply_interface_keeps_distinct_scopes_active() {
        let mut left = sample_interface();
        left.id = "genetica.local.left".to_string();
        left.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-left");
        let left_scope = left.scope.clone();

        let mut right = sample_interface();
        right.id = "genetica.local.right".to_string();
        right.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-right");
        let right_scope = right.scope.clone();

        let mut state = RuntimeState::default();
        state.apply_interface(left).unwrap();
        state.apply_interface(right).unwrap();

        assert_eq!(state.active_interface_by_scope.len(), 2);
        assert_eq!(
            state.active_interface_for_scope(&left_scope).unwrap().id,
            "genetica.local.left"
        );
        assert_eq!(
            state.active_interface_for_scope(&right_scope).unwrap().id,
            "genetica.local.right"
        );
    }

    #[test]
    fn dispatch_action_records_last_action() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let dispatched = state.dispatch_action("open.workspace").unwrap();

        assert_eq!(dispatched.action_id, "open.workspace");
        assert_eq!(dispatched.interface_id.as_deref(), Some("genetica.local"));
        assert_eq!(
            state.last_dispatched_action.as_ref().unwrap().label,
            "Open workspace"
        );
        assert_eq!(
            state
                .last_dispatched_action_for_interface("genetica.local")
                .as_ref()
                .unwrap()
                .action_id,
            "open.workspace"
        );
        assert!(state
            .recent_events(8)
            .iter()
            .any(|event| event.kind == RuntimeEventKind::ActionDispatched
                && event.interface_id.as_deref() == Some("genetica.local")));
    }

    #[test]
    fn runtime_records_native_action_execution_result() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        state.dispatch_action("open.workspace").unwrap();

        let recorded = state
            .record_action_execution_result(
                "genetica.local",
                "open.workspace",
                true,
                "native_open_requested",
                "native OWT requested platform open",
            )
            .unwrap();

        assert!(recorded.executed);
        assert_eq!(
            state
                .last_dispatched_action_for_interface("genetica.local")
                .as_ref()
                .unwrap()
                .execution_status
                .as_deref(),
            Some("native_open_requested")
        );
    }

    #[test]
    fn dispatch_action_records_permissioned_execution_intent_without_running_command() {
        let mut document = sample_interface();
        document.actions[0].command = Some("xdg-open .".to_string());
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let dispatched = state
            .dispatch_action_for_interface_with_intent_and_provenance(
                Some("genetica.local"),
                "open.workspace",
                true,
                false,
                true,
                ActionProvenance {
                    source: Some("mcp:owt_current".to_string()),
                    request_id: Some("dispatch-001".to_string()),
                    requested_at_unix: Some(1779160000),
                },
            )
            .unwrap();

        assert_eq!(dispatched.action_id, "open.workspace");
        assert_eq!(dispatched.source.as_deref(), Some("mcp:owt_current"));
        assert_eq!(dispatched.request_id.as_deref(), Some("dispatch-001"));
        assert_eq!(dispatched.requested_at_unix, Some(1779160000));
        assert!(dispatched.permission_granted);
        assert_eq!(dispatched.approval_state, RequestApprovalState::Granted);
        assert!(!dispatched.approval_required);
        assert!(dispatched.external_execution_requested);
        assert!(!dispatched.executed);
        assert_eq!(
            dispatched.execution_status.as_deref(),
            Some("recorded_not_executed")
        );
        assert_eq!(dispatched.command.as_deref(), Some("xdg-open ."));
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::ActionExecutionRequested
                && event.action_id.as_deref() == Some("open.workspace")
        }));
    }

    #[test]
    fn dispatch_action_records_pending_external_execution_without_permission() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let dispatched = state
            .dispatch_action_for_interface_with_intent(
                Some("genetica.local"),
                "open.workspace",
                false,
                false,
                true,
            )
            .unwrap();

        assert_eq!(dispatched.approval_state, RequestApprovalState::Pending);
        assert!(dispatched.approval_required);
        assert!(!dispatched.permission_granted);
        assert_eq!(
            dispatched.execution_status.as_deref(),
            Some("pending_approval")
        );
        assert_eq!(
            state.recent_action_requests(Some("genetica.local"), 8)[0].approval_state,
            RequestApprovalState::Pending
        );
    }

    #[test]
    fn dispatch_action_requires_confirmation_for_dangerous_execution_intent() {
        let mut document = sample_interface();
        document.allowed_action_kinds.push(ActionKind::Destructive);
        document.actions.push({
            let mut action =
                UiAction::new("delete.output", "Delete output", ActionKind::Destructive);
            action.requires_confirmation = true;
            action
        });
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let pending = state
            .dispatch_action_for_interface_with_intent(
                Some("genetica.local"),
                "delete.output",
                false,
                false,
                true,
            )
            .unwrap();
        assert_eq!(pending.approval_state, RequestApprovalState::Pending);
        assert!(!pending.confirmation_granted);

        assert_eq!(
            state.dispatch_action_for_interface_with_intent(
                Some("genetica.local"),
                "delete.output",
                true,
                false,
                true,
            ),
            Err(ControlError::ActionExecutionRequiresConfirmation {
                action_id: "delete.output".to_string(),
                kind: "Destructive".to_string(),
            })
        );

        let dispatched = state
            .dispatch_action_for_interface_with_intent(
                Some("genetica.local"),
                "delete.output",
                true,
                true,
                true,
            )
            .unwrap();
        assert!(dispatched.confirmation_granted);
        assert_eq!(dispatched.approval_state, RequestApprovalState::Granted);
        assert!(!dispatched.executed);
    }

    #[test]
    fn native_permission_decision_approves_pending_action_request() {
        let mut document = sample_interface();
        document.allowed_action_kinds.push(ActionKind::Run);
        document.actions[0].kind = ActionKind::Run;
        document.actions[0].requires_confirmation = true;
        document.actions[0].command = Some("xdg-open .".to_string());
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let pending = state
            .dispatch_action_for_interface_with_intent_and_provenance(
                Some("genetica.local"),
                "open.workspace",
                false,
                false,
                true,
                ActionProvenance {
                    source: Some("mcp:owt_current".to_string()),
                    request_id: Some("dispatch-approval-001".to_string()),
                    requested_at_unix: Some(1779160400),
                },
            )
            .unwrap();

        let decision = state
            .approve_permission_request(pending.request_id.as_deref().unwrap())
            .unwrap();

        assert_eq!(decision.request_kind, "action");
        assert_eq!(decision.approval_state, RequestApprovalState::Granted);
        let action_requests = state.recent_action_requests(Some("genetica.local"), 8);
        assert_eq!(
            action_requests[0].approval_state,
            RequestApprovalState::Granted
        );
        assert!(action_requests[0].permission_granted);
        assert!(action_requests[0].confirmation_granted);
        assert_eq!(
            action_requests[0].execution_status.as_deref(),
            Some("recorded_not_executed")
        );
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::PermissionGranted
                && event.action_id.as_deref() == Some("open.workspace")
        }));
    }

    #[test]
    fn dispatch_action_can_target_duplicate_action_ids_by_interface() {
        let mut left = sample_interface();
        left.id = "genetica.local.left".to_string();
        left.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-left");

        let mut right = sample_interface();
        right.id = "genetica.local.right".to_string();
        right.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-right");

        let mut state = RuntimeState::default();
        state.apply_interface(left).unwrap();
        state.apply_interface(right).unwrap();

        assert_eq!(
            state.dispatch_action("open.workspace"),
            Err(ControlError::AmbiguousAction {
                action_id: "open.workspace".to_string(),
                interface_ids: "genetica.local.left,genetica.local.right".to_string(),
            })
        );

        let dispatched = state
            .dispatch_action_for_interface(Some("genetica.local.right"), "open.workspace")
            .unwrap();
        assert_eq!(
            dispatched.interface_id.as_deref(),
            Some("genetica.local.right")
        );
        assert!(state
            .last_dispatched_action_for_interface("genetica.local.left")
            .is_none());
        assert!(state
            .last_dispatched_action_for_interface("genetica.local.right")
            .is_some());
    }

    #[test]
    fn dispatch_action_focuses_matching_table_row() {
        let mut document = sample_interface();
        document.actions.push(UiAction::new(
            "kernel.local.drilldown",
            "Inspect local kernel",
            ActionKind::Inspect,
        ));
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        let mut row = UiNode::new("row.local", UiNodeKind::DataCascade);
        row.properties
            .insert("cells".to_string(), "local|6.8|ok".to_string());
        row.properties
            .insert("focus_key".to_string(), "local".to_string());
        row.properties
            .insert("group".to_string(), "current".to_string());
        row.action_id = Some("kernel.local.drilldown".to_string());
        table.children.push(row);
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        state
            .dispatch_action_for_interface(Some("genetica.local"), "kernel.local.drilldown")
            .unwrap();

        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focused_row").map(String::as_str),
            Some("local")
        );
        assert_eq!(
            table.properties.get("focused_group").map(String::as_str),
            Some("current")
        );
        assert!(state.recent_events(4).iter().any(|event| {
            event.kind == RuntimeEventKind::ActionDispatched
                && event.node_id.as_deref() == Some("row.local")
        }));
    }

    #[test]
    fn table_focus_navigation_moves_through_text_rows() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table.properties.insert(
            "rows".to_string(),
            "alpha|4.4|legacy\nbeta|6.1|current\ngamma|6.8|test".to_string(),
        );
        table
            .properties
            .insert("focused_row".to_string(), "beta".to_string());
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let moved = state
            .focus_table_row_relative(Some("genetica.local"), None, TableFocusMovement::Next)
            .unwrap();

        assert_eq!(moved.table_node_id, "table.kernel");
        assert_eq!(moved.focused_row, "gamma");
        assert_eq!(moved.focused_group, None);
        assert_eq!(moved.row_node_id, None);
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focused_row").map(String::as_str),
            Some("gamma")
        );

        let moved = state
            .focus_table_row_relative(Some("genetica.local"), None, TableFocusMovement::Previous)
            .unwrap();
        assert_eq!(moved.focused_row, "beta");
    }

    #[test]
    fn table_focus_navigation_uses_child_focus_keys() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        for (id, focus_key, cells) in [
            ("row.local", "local", "local|6.8|ok"),
            ("row.remote", "remote", "remote|6.1|stale"),
        ] {
            let mut row = UiNode::new(id, UiNodeKind::DataCascade);
            row.properties
                .insert("focus_key".to_string(), focus_key.to_string());
            row.properties
                .insert("cells".to_string(), cells.to_string());
            table.children.push(row);
        }
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let moved = state
            .focus_table_row_relative(Some("genetica.local"), None, TableFocusMovement::Next)
            .unwrap();

        assert_eq!(moved.row_node_id.as_deref(), Some("row.local"));
        assert_eq!(moved.focused_row, "local");
        let moved = state
            .focus_table_row_relative(Some("genetica.local"), None, TableFocusMovement::Next)
            .unwrap();
        assert_eq!(moved.row_node_id.as_deref(), Some("row.remote"));
        assert_eq!(moved.focused_row, "remote");
    }

    #[test]
    fn table_focus_navigation_can_jump_between_groups() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|cohort".to_string());
        table.properties.insert(
            "rows".to_string(),
            "alpha|4.4|legacy\nbeta|4.4|legacy\ngamma|6.8|current\ndelta|6.9|current".to_string(),
        );
        table
            .properties
            .insert("group_by".to_string(), "cohort".to_string());
        table
            .properties
            .insert("focused_row".to_string(), "beta".to_string());
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let moved = state
            .focus_table_row_relative(Some("genetica.local"), None, TableFocusMovement::NextGroup)
            .unwrap();

        assert_eq!(moved.focused_row, "gamma");
        assert_eq!(moved.focused_group.as_deref(), Some("current"));
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focused_group").map(String::as_str),
            Some("current")
        );
        let moved = state
            .focus_table_row_relative(
                Some("genetica.local"),
                None,
                TableFocusMovement::PreviousGroup,
            )
            .unwrap();
        assert_eq!(moved.focused_row, "beta");
        assert_eq!(moved.focused_group.as_deref(), Some("legacy"));
    }

    #[test]
    fn table_cell_focus_navigation_moves_between_columns() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table
            .properties
            .insert("focused_column".to_string(), "kernel".to_string());
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let moved = state
            .focus_table_cell_relative(Some("genetica.local"), None, TableCellFocusMovement::Next)
            .unwrap();

        assert_eq!(moved.table_node_id, "table.kernel");
        assert_eq!(moved.focused_column, "state");
        assert_eq!(moved.column_index, 2);
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focused_column").map(String::as_str),
            Some("state")
        );

        let moved = state
            .focus_table_cell_relative(
                Some("genetica.local"),
                None,
                TableCellFocusMovement::Previous,
            )
            .unwrap();
        assert_eq!(moved.focused_column, "kernel");

        let moved = state
            .focus_table_cell_relative(Some("genetica.local"), None, TableCellFocusMovement::First)
            .unwrap();
        assert_eq!(moved.focused_column, "host");

        let moved = state
            .focus_table_cell_relative(Some("genetica.local"), None, TableCellFocusMovement::Last)
            .unwrap();
        assert_eq!(moved.focused_column, "state");
    }

    #[test]
    fn table_cell_focus_navigation_canonicalizes_aliases() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table
            .properties
            .insert("selected_cell".to_string(), "host".to_string());
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let moved = state
            .focus_table_cell_relative(Some("genetica.local"), None, TableCellFocusMovement::Next)
            .unwrap();

        assert_eq!(moved.focused_column, "kernel");
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focused_column").map(String::as_str),
            Some("kernel")
        );
        assert!(!table.properties.contains_key("selected_cell"));
    }

    #[test]
    fn table_cell_focus_can_target_named_column() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table
            .properties
            .insert("selected_column".to_string(), "host".to_string());
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let focused = state
            .focus_table_cell(Some("genetica.local"), None, "state")
            .unwrap();

        assert_eq!(focused.table_node_id, "table.kernel");
        assert_eq!(focused.focused_column, "state");
        assert_eq!(focused.column_index, 2);
        assert_eq!(focused.movement, None);
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focused_column").map(String::as_str),
            Some("state")
        );
        assert!(!table.properties.contains_key("selected_column"));
    }

    #[test]
    fn table_focus_group_mode_toggles_and_preserves_previous_state() {
        let mut document = sample_interface();
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|cohort".to_string());
        table.properties.insert(
            "rows".to_string(),
            "alpha|4.4|legacy\nbeta|4.4|legacy\ngamma|6.8|current".to_string(),
        );
        table
            .properties
            .insert("group_by".to_string(), "cohort".to_string());
        table
            .properties
            .insert("focused_row".to_string(), "gamma".to_string());
        table
            .properties
            .insert("focused_group".to_string(), "current".to_string());
        table
            .properties
            .insert("focus_mode".to_string(), "rows".to_string());
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let focused = state
            .toggle_focused_table_group_mode(Some("genetica.local"), None)
            .unwrap();

        assert!(focused.active);
        assert_eq!(focused.focused_group, "current");
        assert_eq!(focused.previous_focus_mode.as_deref(), Some("rows"));
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focus_mode").map(String::as_str),
            Some("group")
        );
        assert_eq!(
            table.properties.get("expanded_group").map(String::as_str),
            Some("current")
        );
        assert_eq!(
            table
                .properties
                .get("previous_focus_mode")
                .map(String::as_str),
            Some("rows")
        );

        let restored = state
            .toggle_focused_table_group_mode(Some("genetica.local"), None)
            .unwrap();

        assert!(!restored.active);
        let table = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "table.kernel")
            .expect("kernel table");
        assert_eq!(
            table.properties.get("focus_mode").map(String::as_str),
            Some("rows")
        );
        assert!(!table.properties.contains_key("expanded_group"));
        assert!(!table.properties.contains_key("previous_focus_mode"));
    }

    #[test]
    fn dispatch_focused_table_row_action_uses_focused_row() {
        let mut document = sample_interface();
        document.actions.push(UiAction::new(
            "kernel.remote.drilldown",
            "Inspect remote kernel",
            ActionKind::Inspect,
        ));
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("focused_row".to_string(), "remote".to_string());
        let mut row = UiNode::new("row.remote", UiNodeKind::DataCascade);
        row.properties
            .insert("focus_key".to_string(), "remote".to_string());
        row.properties
            .insert("cells".to_string(), "remote|6.1|stale".to_string());
        row.action_id = Some("kernel.remote.drilldown".to_string());
        table.children.push(row);
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();
        let dispatched = state
            .dispatch_focused_table_row_action(Some("genetica.local"), None)
            .unwrap();

        assert_eq!(dispatched.action_id, "kernel.remote.drilldown");
        assert_eq!(dispatched.interface_id.as_deref(), Some("genetica.local"));
        assert!(state.recent_events(4).iter().any(|event| {
            event.kind == RuntimeEventKind::ActionDispatched
                && event.node_id.as_deref() == Some("row.remote")
        }));
    }

    #[test]
    fn dispatch_action_rejects_unknown_action() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        assert_eq!(
            state.dispatch_action("missing.action"),
            Err(ControlError::UnregisteredAction {
                action_id: "missing.action".to_string(),
            })
        );
    }

    #[test]
    fn request_refresh_records_permissioned_intent_without_execution() {
        let mut document = sample_interface();
        document.allowed_action_kinds.push(ActionKind::Refresh);
        document.actions.push({
            let mut action = UiAction::new("kernel.refresh", "Refresh Kernel", ActionKind::Refresh);
            action.target = Some("local:/proc".to_string());
            action
        });
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let requested = state
            .request_refresh_with_provenance(
                Some("genetica.local"),
                None,
                Some("kernel.refresh"),
                true,
                true,
                ActionProvenance {
                    source: Some("mcp:owt_current".to_string()),
                    request_id: Some("refresh-001".to_string()),
                    requested_at_unix: Some(1779160100),
                },
            )
            .unwrap();

        assert_eq!(requested.interface_id, "genetica.local");
        assert_eq!(requested.action_id.as_deref(), Some("kernel.refresh"));
        assert_eq!(requested.label.as_deref(), Some("Refresh Kernel"));
        assert_eq!(requested.kind, Some(ActionKind::Refresh));
        assert_eq!(requested.target.as_deref(), Some("local:/proc"));
        assert_eq!(requested.source.as_deref(), Some("mcp:owt_current"));
        assert_eq!(requested.request_id.as_deref(), Some("refresh-001"));
        assert_eq!(requested.requested_at_unix, Some(1779160100));
        assert!(requested.permission_granted);
        assert_eq!(requested.approval_state, RequestApprovalState::Granted);
        assert!(!requested.approval_required);
        assert!(requested.external_execution_requested);
        assert!(!requested.executed);
        assert!(state
            .recent_events(8)
            .iter()
            .any(|event| event.kind == RuntimeEventKind::RefreshRequested
                && event.action_id.as_deref() == Some("kernel.refresh")));
    }

    #[test]
    fn runtime_tracks_recent_permissioned_action_and_refresh_requests() {
        let mut document = sample_interface();
        document.actions[0].command = Some("xdg-open .".to_string());
        document.allowed_action_kinds.push(ActionKind::Refresh);
        document.actions.push({
            let mut action = UiAction::new("kernel.refresh", "Refresh Kernel", ActionKind::Refresh);
            action.target = Some("local:/proc".to_string());
            action
        });
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        state
            .dispatch_action_for_interface_with_intent_and_provenance(
                Some("genetica.local"),
                "open.workspace",
                true,
                false,
                true,
                ActionProvenance {
                    source: Some("mcp:owt_current".to_string()),
                    request_id: Some("dispatch-queue-001".to_string()),
                    requested_at_unix: Some(1779160200),
                },
            )
            .unwrap();
        state
            .request_refresh_with_provenance(
                Some("genetica.local"),
                None,
                Some("kernel.refresh"),
                true,
                true,
                ActionProvenance {
                    source: Some("mcp:owt_current".to_string()),
                    request_id: Some("refresh-queue-001".to_string()),
                    requested_at_unix: Some(1779160300),
                },
            )
            .unwrap();

        let action_requests = state.recent_action_requests(Some("genetica.local"), 24);
        assert_eq!(action_requests.len(), 1);
        assert_eq!(
            action_requests[0].request_id.as_deref(),
            Some("dispatch-queue-001")
        );
        assert!(action_requests[0].external_execution_requested);
        assert_eq!(
            action_requests[0].approval_state,
            RequestApprovalState::Granted
        );
        assert!(!action_requests[0].executed);

        let refresh_requests = state.recent_refresh_requests(Some("genetica.local"), 24);
        assert_eq!(refresh_requests.len(), 1);
        assert_eq!(
            refresh_requests[0].request_id.as_deref(),
            Some("refresh-queue-001")
        );
        assert_eq!(refresh_requests[0].kind, Some(ActionKind::Refresh));
        assert_eq!(refresh_requests[0].target.as_deref(), Some("local:/proc"));
        assert!(refresh_requests[0].external_execution_requested);
        assert_eq!(
            refresh_requests[0].approval_state,
            RequestApprovalState::Granted
        );
        assert!(!refresh_requests[0].executed);
    }

    #[test]
    fn request_refresh_records_pending_external_execution_without_permission() {
        let mut document = sample_interface();
        document.allowed_action_kinds.push(ActionKind::Refresh);
        document.actions.push(UiAction::new(
            "kernel.refresh",
            "Refresh Kernel",
            ActionKind::Refresh,
        ));
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let requested = state
            .request_refresh(
                Some("genetica.local"),
                None,
                Some("kernel.refresh"),
                false,
                true,
            )
            .unwrap();

        assert_eq!(requested.approval_state, RequestApprovalState::Pending);
        assert!(requested.approval_required);
        assert!(!requested.permission_granted);
        assert!(requested.external_execution_requested);
        assert_eq!(
            state.recent_refresh_requests(Some("genetica.local"), 8)[0].approval_state,
            RequestApprovalState::Pending
        );
    }

    #[test]
    fn native_permission_decision_denies_pending_refresh_request() {
        let mut document = sample_interface();
        document.allowed_action_kinds.push(ActionKind::Refresh);
        document.actions.push(UiAction::new(
            "kernel.refresh",
            "Refresh Kernel",
            ActionKind::Refresh,
        ));
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let pending = state
            .request_refresh_with_provenance(
                Some("genetica.local"),
                None,
                Some("kernel.refresh"),
                false,
                true,
                ActionProvenance {
                    source: Some("mcp:owt_current".to_string()),
                    request_id: Some("refresh-denial-001".to_string()),
                    requested_at_unix: Some(1779160500),
                },
            )
            .unwrap();

        let decision = state
            .deny_permission_request(pending.request_id.as_deref().unwrap())
            .unwrap();

        assert_eq!(decision.request_kind, "refresh");
        assert_eq!(decision.approval_state, RequestApprovalState::Denied);
        let refresh_requests = state.recent_refresh_requests(Some("genetica.local"), 8);
        assert_eq!(
            refresh_requests[0].approval_state,
            RequestApprovalState::Denied
        );
        assert!(!refresh_requests[0].permission_granted);
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::PermissionDenied
                && event.action_id.as_deref() == Some("kernel.refresh")
        }));
    }

    #[test]
    fn diff_interface_documents_reports_semantic_changes() {
        let left = sample_interface();
        let mut right = sample_interface();
        right.title = "GENETICA UPDATED".to_string();
        right.nodes[0]
            .properties
            .insert("dock".to_string(), "left".to_string());
        right.nodes[0]
            .children
            .push(UiNode::new("metric.kernel", UiNodeKind::Metric));
        right.actions.push(UiAction::new(
            "kernel.refresh",
            "Refresh Kernel",
            ActionKind::Refresh,
        ));
        right.allowed_action_kinds.push(ActionKind::Refresh);

        let diff = super::diff_interface_documents(&left, &right);

        assert!(diff.changed);
        assert!(diff.summary.iter().any(|item| item == "title changed"));
        assert_eq!(diff.added_actions, vec!["kernel.refresh"]);
        assert_eq!(diff.added_nodes, vec!["metric.kernel"]);
        assert_eq!(diff.changed_root_properties, vec!["dock"]);
    }

    #[test]
    fn diff_interface_documents_reports_operational_fact_changes() {
        let mut left = sample_interface();
        let mut left_metric = UiNode::new("metric.kernel", UiNodeKind::Metric);
        left_metric.label = Some("KERNEL".to_string());
        left_metric
            .properties
            .insert("value".to_string(), "6.8.0".to_string());
        left_metric
            .properties
            .insert("status".to_string(), "live".to_string());
        left.nodes[0].children.push(left_metric);

        let mut right = left.clone();
        right.nodes[0].children[1]
            .properties
            .insert("value".to_string(), "6.9.1".to_string());
        right.nodes[0].children[1]
            .properties
            .insert("status".to_string(), "stale".to_string());

        let diff = super::diff_interface_documents(&left, &right);

        assert!(diff.changed);
        assert_eq!(diff.changed_facts.len(), 1);
        assert_eq!(diff.changed_facts[0].node_id, "metric.kernel");
        assert_eq!(diff.changed_facts[0].kind, "metric");
        assert!(diff.changed_facts[0].left.contains("value=6.8.0"));
        assert!(diff.changed_facts[0].right.contains("value=6.9.1"));
        assert!(diff
            .summary
            .iter()
            .any(|item| item == "1 operational fact(s) changed"));
    }

    #[test]
    fn validation_rejects_unknown_action_reference() {
        let mut document = sample_interface();
        document.nodes[0].children[0].action_id = Some("missing.action".to_string());

        assert_eq!(
            validate_interface(&document),
            Err(ControlError::UnknownAction {
                node_id: "button.open".to_string(),
                action_id: "missing.action".to_string(),
            })
        );
    }

    #[test]
    fn validation_rejects_duplicate_node_id() {
        let mut document = sample_interface();
        document.nodes[0]
            .children
            .push(UiNode::new("button.open", UiNodeKind::Badge));

        assert_eq!(
            validate_interface(&document),
            Err(ControlError::DuplicateId {
                kind: "node",
                id: "button.open".to_string(),
            })
        );
    }

    #[test]
    fn validation_rejects_action_kind_outside_allowlist() {
        let mut document = sample_interface();
        document.allowed_action_kinds = vec![ActionKind::Inspect];

        assert_eq!(
            validate_interface(&document),
            Err(ControlError::ActionKindNotAllowed {
                action_id: "open.workspace".to_string(),
                kind: "Open".to_string(),
            })
        );
    }

    #[test]
    fn validation_requires_confirmation_for_destructive_actions() {
        let scope = Scope::new(ScopeKind::Project, "test");
        let mut document = InterfaceDocument::new("danger.test", "Danger Test", scope);
        document.allowed_action_kinds = vec![ActionKind::Destructive];
        document.actions.push(UiAction::new(
            "delete.output",
            "Delete output",
            ActionKind::Destructive,
        ));
        let mut button = UiNode::new("button.delete", UiNodeKind::Button);
        button.action_id = Some("delete.output".to_string());
        document.nodes.push(button);

        assert_eq!(
            validate_interface(&document),
            Err(ControlError::ActionRequiresConfirmation {
                action_id: "delete.output".to_string(),
                kind: "Destructive".to_string(),
            })
        );
    }

    #[test]
    fn validation_requires_confirmation_for_network_actions() {
        for (kind, action_id, label, expected_kind) in [(
            ActionKind::Network,
            "network.fetch",
            "Fetch updates",
            "Network",
        )] {
            let scope = Scope::new(ScopeKind::Project, "test");
            let mut document =
                InterfaceDocument::new(format!("{action_id}.test"), "External Test", scope);
            document.allowed_action_kinds = vec![kind.clone()];
            document
                .actions
                .push(UiAction::new(action_id, label, kind.clone()));
            let mut button = UiNode::new(format!("button.{action_id}"), UiNodeKind::Button);
            button.action_id = Some(action_id.to_string());
            document.nodes.push(button);

            assert_eq!(
                validate_interface(&document),
                Err(ControlError::ActionRequiresConfirmation {
                    action_id: action_id.to_string(),
                    kind: expected_kind.to_string(),
                })
            );

            document.actions[0].requires_confirmation = true;
            validate_interface(&document).unwrap();
        }
    }

    #[test]
    fn validation_allows_structured_run_action_without_confirmation() {
        let scope = Scope::new(ScopeKind::Project, "test");
        let mut document = InterfaceDocument::new("runner.exec.test", "Run Test", scope);
        document.allowed_action_kinds = vec![ActionKind::Run];
        let mut action = UiAction::new("runner.exec", "Run check", ActionKind::Run);
        action.argv = vec!["owt-check".to_string(), "--dry-run".to_string()];
        document.actions.push(action);
        let mut button = UiNode::new("button.runner.exec", UiNodeKind::Button);
        button.action_id = Some("runner.exec".to_string());
        document.nodes.push(button);

        validate_interface(&document).unwrap();
        assert_eq!(interface_validation_warnings(&document).len(), 1);
    }

    #[test]
    fn validation_rejects_credential_sensitive_visible_text() {
        let mut document = sample_interface();
        document.nodes[0].children[0].label =
            Some("Authorization: Bearer abc123456789".to_string());

        assert_eq!(
            validate_interface(&document),
            Err(ControlError::SensitiveContent {
                field: "node.button.open.label".to_string(),
                indicator: "authorization:".to_string(),
            })
        );
    }

    #[test]
    fn validation_rejects_credential_sensitive_property_keys() {
        let mut document = sample_interface();
        document.nodes[0].children[0]
            .properties
            .insert("api_token".to_string(), "redacted".to_string());

        assert_eq!(
            validate_interface(&document),
            Err(ControlError::SensitiveContent {
                field: "node.button.open.properties.api_token".to_string(),
                indicator: "token".to_string(),
            })
        );
    }

    #[test]
    fn validation_warns_when_actionable_button_lacks_keyboard_fallback() {
        let mut document = InterfaceDocument::new(
            "keyboard.validation",
            "Keyboard Validation",
            Scope::new(ScopeKind::Session, "keyboard.validation"),
        );
        document.allowed_action_kinds = vec![ActionKind::Inspect];

        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        for index in 1..=10 {
            let action_id = format!("inspect.{index}");
            document.actions.push(UiAction::new(
                action_id.clone(),
                format!("Inspect {index}"),
                ActionKind::Inspect,
            ));
            let mut button = UiNode::new(format!("button.{index}"), UiNodeKind::Button);
            button.label = Some(format!("Inspect {index}"));
            button.action_id = Some(action_id);
            root.children.push(button);
        }
        root.properties
            .insert("action_paging".to_string(), "false".to_string());
        document.nodes.push(root);

        validate_interface(&document).unwrap();
        let warnings = interface_validation_warnings(&document);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "keyboard_fallback_missing");
        assert_eq!(warnings[0].node_id.as_deref(), Some("button.10"));
        assert_eq!(warnings[0].action_id.as_deref(), Some("inspect.10"));

        document.nodes[0].children[9].properties.insert(
            "keyboard_shortcut".to_string(),
            "Ctrl+Alt+Shift+0".to_string(),
        );
        assert!(interface_validation_warnings(&document).is_empty());
    }

    #[test]
    fn validation_warns_when_external_action_lacks_allowlist() {
        let mut document = InterfaceDocument::new(
            "allowlist.validation",
            "Allowlist Validation",
            Scope::new(ScopeKind::Session, "allowlist.validation"),
        );
        document.allowed_action_kinds = vec![ActionKind::Refresh];
        let mut action = UiAction::new("kernel.refresh", "Refresh Kernel", ActionKind::Refresh);
        action.target = Some("local:/proc".to_string());
        document.actions.push(action);

        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        let mut button = UiNode::new("button.refresh", UiNodeKind::Button);
        button.label = Some("Refresh".to_string());
        button.action_id = Some("kernel.refresh".to_string());
        root.children.push(button);
        document.nodes.push(root);

        validate_interface(&document).unwrap();
        let warnings = interface_validation_warnings(&document);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "action_allowlist_missing");
        assert_eq!(warnings[0].node_id, None);
        assert_eq!(warnings[0].action_id.as_deref(), Some("kernel.refresh"));

        document.nodes[0].properties.insert(
            "allowed_action_targets".to_string(),
            "kernel.refresh=local:/proc".to_string(),
        );
        assert!(interface_validation_warnings(&document).is_empty());
    }

    #[test]
    fn validation_warns_when_command_action_lacks_allowlist() {
        let mut document = InterfaceDocument::new(
            "command.allowlist.validation",
            "Command Allowlist Validation",
            Scope::new(ScopeKind::Session, "command.allowlist.validation"),
        );
        document.allowed_action_kinds = vec![ActionKind::Run];
        let mut action = UiAction::new("runner.exec", "Run Check", ActionKind::Run);
        action.command = Some("owt-check --dry-run".to_string());
        action.requires_confirmation = true;
        document.actions.push(action);

        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.properties.insert(
            "execution_allowlist".to_string(),
            "runner.exec=owt-check --dry-run".to_string(),
        );
        document.nodes.push(root);

        validate_interface(&document).unwrap();
        assert!(interface_validation_warnings(&document).is_empty());

        document.nodes[0].properties.clear();
        let warnings = interface_validation_warnings(&document);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "action_allowlist_missing");
        assert_eq!(warnings[0].action_id.as_deref(), Some("runner.exec"));
    }

    #[test]
    fn validation_warns_when_fact_node_lacks_data_provenance() {
        let mut document = InterfaceDocument::new(
            "provenance.validation",
            "Provenance Validation",
            Scope::new(ScopeKind::Session, "provenance.validation"),
        );
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.children
            .push(UiNode::new("metric.kernel", UiNodeKind::Metric));
        document.nodes.push(root);

        validate_interface(&document).unwrap();
        let warnings = interface_validation_warnings(&document);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "data_provenance_missing");
        assert_eq!(warnings[0].node_id.as_deref(), Some("metric.kernel"));
    }

    #[test]
    fn validation_accepts_inherited_data_provenance() {
        let mut document = InterfaceDocument::new(
            "provenance.inherited",
            "Provenance Inherited",
            Scope::new(ScopeKind::Session, "provenance.inherited"),
        );
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.provenance = Some(FactProvenance {
            source: Some("fixture:/proc".to_string()),
            collected_at: Some("2026-05-19T00:00:00Z".to_string()),
            ..FactProvenance::default()
        });
        root.children
            .push(UiNode::new("table.kernel", UiNodeKind::Table));
        document.nodes.push(root);

        validate_interface(&document).unwrap();

        assert!(interface_validation_warnings(&document).is_empty());
    }

    #[test]
    fn update_node_appends_atomic_child_to_active_interface() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut badge = UiNode::new("badge.ready", UiNodeKind::Badge);
        badge.label = Some("READY".to_string());
        badge.text = Some("ATOMIC".to_string());

        let applied = state
            .update_node(None, None, Some("button.open"), badge, None, false)
            .unwrap();

        assert_eq!(applied.interface_id, "genetica.local");
        let root = &state.interfaces["genetica.local"].nodes[0];
        assert_eq!(root.children[0].children[0].id, "badge.ready");
    }

    #[test]
    fn update_node_can_bind_action_and_button_atomically() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let action = UiAction::new("inspect.status", "Inspect status", ActionKind::Inspect);
        let mut button = UiNode::new("button.status", UiNodeKind::Button);
        button.label = Some("STATUS".to_string());
        button.action_id = Some("inspect.status".to_string());

        state
            .update_node(None, None, Some("panel.root"), button, Some(action), false)
            .unwrap();

        assert!(state.interfaces["genetica.local"]
            .nodes
            .iter()
            .flat_map(|node| &node.children)
            .any(|node| node.id == "button.status"));
        assert!(state.actions.contains_key("inspect.status"));
    }

    #[test]
    fn bind_action_attaches_action_to_existing_node() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let action = UiAction::new("inspect.root", "Inspect Root", ActionKind::Inspect);
        let applied = state.bind_action(None, None, "panel.root", action).unwrap();

        assert_eq!(applied.interface_id, "genetica.local");
        assert_eq!(
            state.interfaces["genetica.local"].nodes[0]
                .action_id
                .as_deref(),
            Some("inspect.root")
        );
        assert!(state.actions.contains_key("inspect.root"));
        assert!(state
            .recent_events(4)
            .iter()
            .any(|event| event.action_id.as_deref() == Some("inspect.root")
                && event.node_id.as_deref() == Some("panel.root")));
    }

    #[test]
    fn update_node_rejects_unknown_parent() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let badge = UiNode::new("badge.ready", UiNodeKind::Badge);

        assert_eq!(
            state.update_node(None, None, Some("missing.parent"), badge, None, false),
            Err(ControlError::UnknownParent {
                interface_id: "genetica.local".to_string(),
                parent_id: "missing.parent".to_string(),
            })
        );
    }

    #[test]
    fn edit_interface_applies_ordered_live_builder_operations() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut frame = UiNode::new("frame.main", UiNodeKind::Frame);
        frame.label = Some("FRAME".to_string());
        let mut bay = UiNode::new("bay.main", UiNodeKind::ContentBay);
        bay.label = Some("CONTENT".to_string());
        let mut metric = UiNode::new("metric.ready", UiNodeKind::Metric);
        metric.label = Some("READY".to_string());

        let edited = state
            .edit_interface(
                Some("genetica.local"),
                None,
                vec![
                    InterfaceEditOperation::AddNode {
                        parent_id: Some("panel.root".to_string()),
                        node: frame,
                        action: None,
                        replace: false,
                    },
                    InterfaceEditOperation::AddNode {
                        parent_id: Some("frame.main".to_string()),
                        node: bay,
                        action: None,
                        replace: false,
                    },
                    InterfaceEditOperation::AddNode {
                        parent_id: Some("bay.main".to_string()),
                        node: metric,
                        action: None,
                        replace: false,
                    },
                    InterfaceEditOperation::PatchNode {
                        node_id: Some("metric.ready".to_string()),
                        label: None,
                        text: Some("ONLINE".to_string()),
                        role: None,
                        action_id: None,
                        provenance: None,
                        properties: std::collections::BTreeMap::new(),
                        remove_properties: Vec::new(),
                        clear_fields: Vec::new(),
                    },
                ],
                false,
            )
            .unwrap();

        assert_eq!(edited.applied.interface_id, "genetica.local");
        assert_eq!(edited.operations.len(), 4);
        let frame = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "frame.main")
            .expect("frame added");
        let metric = &frame.children[0].children[0];
        assert_eq!(metric.id, "metric.ready");
        assert_eq!(metric.text.as_deref(), Some("ONLINE"));
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::NodePatched
                && event.node_id.as_deref() == Some("metric.ready")
        }));
    }

    #[test]
    fn edit_interface_removes_moves_and_highlights_nodes() {
        let mut document = sample_interface();
        document.actions.push(UiAction::new(
            "inspect.extra",
            "Inspect Extra",
            ActionKind::Inspect,
        ));
        let mut extra = UiNode::new("button.extra", UiNodeKind::Button);
        extra.action_id = Some("inspect.extra".to_string());
        document.nodes[0].children.push(extra);
        let mut bay = UiNode::new("bay.main", UiNodeKind::ContentBay);
        bay.label = Some("BAY".to_string());
        document.nodes[0].children.push(bay);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let edited = state
            .edit_interface(
                Some("genetica.local"),
                None,
                vec![
                    InterfaceEditOperation::MoveNode {
                        node_id: Some("button.open".to_string()),
                        parent_id: Some("bay.main".to_string()),
                        position: Some(0),
                    },
                    InterfaceEditOperation::RemoveNode {
                        node_id: Some("button.extra".to_string()),
                        prune_orphan_actions: Some(true),
                    },
                    InterfaceEditOperation::HighlightNode {
                        node_id: Some("bay.main".to_string()),
                        mode: Some("focus".to_string()),
                        label: Some("BUILD".to_string()),
                        duration_ms: Some(1500),
                    },
                ],
                false,
            )
            .unwrap();

        assert_eq!(edited.operations[0].operation, "move_node");
        let bay = state.interfaces["genetica.local"].nodes[0]
            .children
            .iter()
            .find(|node| node.id == "bay.main")
            .expect("bay");
        assert_eq!(bay.children[0].id, "button.open");
        assert_eq!(
            bay.properties
                .get("owt_builder_highlight")
                .map(String::as_str),
            Some("focus")
        );
        assert!(!state.interfaces["genetica.local"]
            .actions
            .iter()
            .any(|action| action.id == "inspect.extra"));
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::NodeHighlighted
                && event.node_id.as_deref() == Some("bay.main")
        }));
    }

    #[test]
    fn edit_interface_dry_run_does_not_mutate_runtime() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let edited = state
            .edit_interface(
                Some("genetica.local"),
                None,
                vec![InterfaceEditOperation::ClearChildren {
                    node_id: Some("panel.root".to_string()),
                }],
                true,
            )
            .unwrap();

        assert!(edited.dry_run);
        assert_eq!(
            state.interfaces["genetica.local"].nodes[0].children.len(),
            1
        );
    }

    #[test]
    fn patch_interface_layout_updates_root_properties_without_replacing_nodes() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "right".to_string());
        properties.insert("reservation".to_string(), "reserved".to_string());

        let patched = state
            .patch_interface_layout(None, None, None, properties)
            .unwrap();

        assert_eq!(patched.applied.interface_id, "genetica.local");
        assert_eq!(patched.node_id, "panel.root");
        assert!(patched.prior_properties.is_empty());
        let root = &state.interfaces["genetica.local"].nodes[0];
        assert_eq!(root.properties["dock"], "right");
        assert_eq!(root.properties["reservation"], "reserved");
        assert_eq!(root.children[0].id, "button.open");
    }

    #[test]
    fn patch_interface_layout_can_target_one_active_surface_by_interface_id() {
        let mut left = sample_interface();
        left.id = "genetica.local.left".to_string();
        left.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-left");

        let mut right = sample_interface();
        right.id = "genetica.local.right".to_string();
        right.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-right");

        let mut state = RuntimeState::default();
        state.apply_interface(left).unwrap();
        state.apply_interface(right).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "right".to_string());
        properties.insert("visible".to_string(), "false".to_string());

        let patched = state
            .patch_interface_layout(Some("genetica.local.right"), None, None, properties)
            .unwrap();

        assert_eq!(patched.applied.interface_id, "genetica.local.right");
        assert!(!state.interfaces["genetica.local.left"].nodes[0]
            .properties
            .contains_key("dock"));
        assert_eq!(
            state.interfaces["genetica.local.right"].nodes[0].properties["dock"],
            "right"
        );
        assert_eq!(
            state.interfaces["genetica.local.right"].nodes[0].properties["visible"],
            "false"
        );
    }

    #[test]
    fn preview_interface_layout_does_not_mutate_document() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "left".to_string());
        properties.insert("reservation".to_string(), "overlay".to_string());

        let preview = state
            .preview_interface_layout(None, None, None, properties)
            .unwrap();

        assert_eq!(preview.target.interface_id, "genetica.local");
        assert_eq!(preview.node_id, "panel.root");
        assert_eq!(preview.preview_properties["dock"], "left");
        assert_eq!(preview.preview_properties["reservation"], "overlay");
        assert_eq!(preview.fit_score.score, 80);
        assert_eq!(preview.fit_score.status, "constrained");
        assert_eq!(preview.fit_score.reservation_slot, "left");
        assert!(!preview.fit_score.reserves_terminal_space);
        assert_eq!(preview.estimated_reserved_space.slot, "left");
        assert_eq!(preview.estimated_reserved_space.columns, 0);
        assert_eq!(preview.estimated_reserved_space.rows, 0);
        assert!(!preview.estimated_reserved_space.reserves_terminal_space);
        assert_eq!(preview.viewport_fit.status, "not_provided");
        assert_eq!(preview.viewport_fit.basis, "viewport_cells_not_provided");
        assert!(!preview.unsupported_hints.is_empty());
        assert!(!state.interfaces["genetica.local"].nodes[0]
            .properties
            .contains_key("dock"));
        assert!(!state
            .recent_events(8)
            .iter()
            .any(|event| event.kind == RuntimeEventKind::LayoutPatched));
    }

    #[test]
    fn preview_interface_layout_reports_reserved_edge_conflicts() {
        let mut existing = sample_interface();
        existing.id = "existing.left".to_string();
        existing.scope = Scope::new(ScopeKind::Session, "existing.left");
        existing.nodes[0]
            .properties
            .insert("dock".to_string(), "left".to_string());

        let mut target = sample_interface();
        target.id = "target.panel".to_string();
        target.scope = Scope::new(ScopeKind::Session, "target.panel");

        let mut state = RuntimeState::default();
        state.apply_interface(existing).unwrap();
        state.apply_interface(target).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "left_rail".to_string());
        properties.insert("z_order".to_string(), "4".to_string());
        properties.insert("collapse_policy".to_string(), "minimize".to_string());

        let preview = state
            .preview_interface_layout(Some("target.panel"), None, None, properties)
            .unwrap();

        assert!(preview
            .conflict_hints
            .iter()
            .any(|hint| hint.contains("existing.left")));
        assert!(preview
            .unsupported_hints
            .iter()
            .any(|hint| hint.contains("z_order")));
        assert_eq!(preview.fit_score.score, 60);
        assert_eq!(preview.fit_score.status, "conflicted");
        assert_eq!(preview.fit_score.reservation_slot, "left");
        assert!(preview.fit_score.reserves_terminal_space);
        assert_eq!(preview.estimated_reserved_space.slot, "left");
        assert_eq!(preview.estimated_reserved_space.columns, 28);
        assert_eq!(preview.estimated_reserved_space.rows, 0);
        assert!(preview.estimated_reserved_space.reserves_terminal_space);
        assert!(preview
            .fit_score
            .factors
            .iter()
            .any(|factor| factor.contains("reserved-placement conflict")));
        assert!(preview.scene_plan.conflict_hints.is_empty());
        assert!(preview.scene_plan.surfaces.iter().any(|surface| {
            surface.requested_slot.as_deref() == Some("left") && surface.slot != "left"
        }));
        assert_eq!(preview.scene_plan.estimated_left_columns, 28);
    }

    #[test]
    fn layout_scene_plan_reports_active_reservation_map() {
        let mut left = sample_interface();
        left.id = "scene.left".to_string();
        left.scope = Scope::new(ScopeKind::Session, "scene.left");
        for index in 0..10 {
            left.actions.push(UiAction::new(
                format!("inspect.scene.{index}"),
                format!("Inspect scene {index}"),
                ActionKind::Inspect,
            ));
        }
        left.nodes[0]
            .properties
            .insert("dock".to_string(), "left".to_string());

        let mut right = sample_interface();
        right.id = "scene.right".to_string();
        right.scope = Scope::new(ScopeKind::Session, "scene.right");
        right.nodes[0]
            .properties
            .insert("dock".to_string(), "right".to_string());

        let mut hidden = sample_interface();
        hidden.id = "scene.hidden".to_string();
        hidden.scope = Scope::new(ScopeKind::Session, "scene.hidden");
        hidden.nodes[0]
            .properties
            .insert("display".to_string(), "hidden".to_string());

        let mut state = RuntimeState::default();
        state.apply_interface(left).unwrap();
        state.apply_interface(right).unwrap();
        state.apply_interface(hidden).unwrap();

        let scene = state.layout_scene_plan();

        assert_eq!(scene.active_interface_count, 3);
        assert_eq!(scene.reserved_surface_count, 2);
        assert_eq!(scene.estimated_left_columns, 28);
        assert_eq!(scene.estimated_right_columns, 28);
        assert!(scene.conflict_hints.is_empty());
        assert!(scene
            .slots
            .iter()
            .any(|slot| slot.slot == "hidden" && slot.interface_count == 1));
        assert!(scene
            .surfaces
            .iter()
            .any(|surface| surface.interface_id == "scene.left"
                && surface.slot == "left"
                && surface.reserves_terminal_space));
        let left_surface = scene
            .surfaces
            .iter()
            .find(|surface| surface.interface_id == "scene.left")
            .unwrap();
        assert_eq!(
            left_surface.action_slots.len(),
            super::NATIVE_KEYBOARD_ACTION_LIMIT
        );
        assert_eq!(left_surface.action_slots[0].slot_index, 1);
        assert_eq!(
            left_surface.action_slots[0].automatic_shortcut,
            "Ctrl+Alt+Shift+1"
        );
        assert_eq!(left_surface.overflow_action_count, 2);
        assert_eq!(left_surface.overflow_action_ids.len(), 2);
    }

    #[test]
    fn layout_scene_plan_estimates_corner_button_as_compact_top_reservation() {
        let mut document = sample_interface();
        document.id = "scene.corner".to_string();
        document.scope = Scope::new(ScopeKind::Project, "/tmp/owt-corner");
        document.nodes[0]
            .properties
            .insert("profile".to_string(), "single_button".to_string());

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let scene = state.layout_scene_plan();
        let surface = scene
            .surfaces
            .iter()
            .find(|surface| surface.interface_id == "scene.corner")
            .unwrap();

        assert_eq!(scene.estimated_top_rows, 3);
        assert_eq!(surface.slot, "top");
        assert_eq!(surface.estimated_rows, 3);
        assert!(surface.reserves_terminal_space);
    }

    #[test]
    fn layout_scene_plan_estimates_action_strip_as_compact_top_reservation() {
        let mut document = sample_interface();
        document.id = "scene.action_strip".to_string();
        document.scope = Scope::new(ScopeKind::Project, "/tmp/owt-actions");
        document.nodes[0]
            .properties
            .insert("profile".to_string(), "action_strip".to_string());

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let scene = state.layout_scene_plan();
        let surface = scene
            .surfaces
            .iter()
            .find(|surface| surface.interface_id == "scene.action_strip")
            .unwrap();

        assert_eq!(scene.estimated_top_rows, 4);
        assert_eq!(surface.slot, "top");
        assert_eq!(surface.estimated_rows, 4);
        assert!(surface.reserves_terminal_space);
    }

    #[test]
    fn layout_scene_plan_reflows_compact_button_off_occupied_top() {
        let mut button = sample_interface();
        button.id = "carriersingles.renders.button".to_string();
        button.scope = Scope::new(
            ScopeKind::Project,
            "/home/buanzo/git/tools/python/carriersingles",
        );
        button.nodes[0]
            .properties
            .insert("profile".to_string(), "single_button".to_string());

        let mut primary = sample_interface();
        primary.id = "genetica.native.scope".to_string();
        primary.scope = Scope::new(ScopeKind::Project, "/home/buanzo/git/tools/data/genetica");

        let mut state = RuntimeState::default();
        state.apply_interface(button).unwrap();
        state.apply_interface(primary).unwrap();

        let scene = state.layout_scene_plan();
        let button_surface = scene
            .surfaces
            .iter()
            .find(|surface| surface.interface_id == "carriersingles.renders.button")
            .unwrap();
        let primary_surface = scene
            .surfaces
            .iter()
            .find(|surface| surface.interface_id == "genetica.native.scope")
            .unwrap();

        assert_eq!(
            state.active_interface_ids_for_scene()[0],
            "genetica.native.scope"
        );
        assert!(scene.conflict_hints.is_empty());
        assert_eq!(primary_surface.slot, "top");
        assert_eq!(button_surface.slot, "bottom");
        assert_eq!(button_surface.estimated_rows, 3);
        assert_eq!(scene.estimated_top_rows, 8);
        assert_eq!(scene.estimated_bottom_rows, 3);
    }

    #[test]
    fn preview_interface_layout_estimates_structural_reserved_space() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("profile".to_string(), "structural_console".to_string());

        let preview = state
            .preview_interface_layout(None, None, None, properties)
            .unwrap();

        assert_eq!(preview.estimated_reserved_space.slot, "top");
        assert_eq!(preview.estimated_reserved_space.columns, 24);
        assert_eq!(preview.estimated_reserved_space.rows, 18);
        assert!(preview.estimated_reserved_space.reserves_terminal_space);
        assert_eq!(
            preview.estimated_reserved_space.basis,
            "first_pass_static_cells_without_live_viewport"
        );
    }

    #[test]
    fn preview_interface_layout_reports_first_pass_overflow_counts() {
        let mut document = sample_interface();
        for index in 0..12 {
            document.actions.push(UiAction::new(
                format!("inspect.{index}"),
                format!("Inspect {index}"),
                ActionKind::Inspect,
            ));
        }
        let mut table = UiNode::new("table.preview", UiNodeKind::Table);
        table.properties.insert(
            "rows".to_string(),
            (0..11)
                .map(|index| format!("row-{index}|ok"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        document.nodes[0].children.push(table);

        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "left".to_string());

        let preview = state
            .preview_interface_layout(None, None, None, properties)
            .unwrap();

        assert_eq!(preview.overflow_estimate.action_count, 13);
        assert_eq!(
            preview.overflow_estimate.automatic_action_slots,
            super::NATIVE_KEYBOARD_ACTION_LIMIT
        );
        assert_eq!(preview.overflow_estimate.overflow_action_count, 4);
        assert_eq!(preview.overflow_estimate.table_count, 1);
        assert_eq!(preview.overflow_estimate.table_row_count, 11);
        assert_eq!(preview.overflow_estimate.estimated_visible_table_rows, 8);
        assert_eq!(preview.overflow_estimate.estimated_hidden_table_rows, 3);
    }

    #[test]
    fn preview_interface_layout_reports_viewport_fit_from_request() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "left".to_string());
        properties.insert("viewport_columns".to_string(), "100".to_string());
        properties.insert("viewport_rows".to_string(), "30".to_string());
        properties.insert("min_terminal_cells".to_string(), "80x24".to_string());

        let preview = state
            .preview_interface_layout(None, None, None, properties)
            .unwrap();

        assert_eq!(preview.estimated_reserved_space.slot, "left");
        assert_eq!(preview.estimated_reserved_space.columns, 28);
        assert_eq!(preview.viewport_fit.status, "too_small");
        assert_eq!(preview.viewport_fit.viewport_columns, Some(100));
        assert_eq!(preview.viewport_fit.viewport_rows, Some(30));
        assert_eq!(
            preview.viewport_fit.terminal_columns_after_reservation,
            Some(72)
        );
        assert_eq!(
            preview.viewport_fit.terminal_rows_after_reservation,
            Some(30)
        );
        assert_eq!(preview.viewport_fit.minimum_columns, Some(80));
        assert_eq!(preview.viewport_fit.minimum_rows, Some(24));
        assert_eq!(preview.viewport_fit.overflow_columns, 8);
        assert_eq!(preview.viewport_fit.overflow_rows, 0);
        assert!(preview
            .fit_score
            .factors
            .iter()
            .any(|factor| factor.contains("viewport estimate")));
    }

    #[test]
    fn patch_interface_layout_rejects_unknown_node() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("dock".to_string(), "bottom".to_string());

        assert_eq!(
            state.patch_interface_layout(None, None, Some("missing.node"), properties),
            Err(ControlError::UnknownNode {
                interface_id: "genetica.local".to_string(),
                node_id: "missing.node".to_string(),
            })
        );
    }

    #[test]
    fn patch_interface_lifecycle_pins_hides_expires_and_restores() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        let applied = state.apply_interface(document).unwrap();

        let patched = state
            .patch_interface_lifecycle(
                Some(&applied.interface_id),
                None,
                InterfaceLifecyclePatch {
                    pinned: Some(true),
                    hidden: Some(true),
                    ttl_seconds: Some(60),
                    now_unix: Some(1000),
                    ..InterfaceLifecyclePatch::default()
                },
            )
            .unwrap();

        assert!(patched.state.pinned);
        assert!(patched.state.hidden);
        assert_eq!(patched.state.previous_hidden, Some(false));
        assert_eq!(patched.state.expires_at_unix, Some(1060));
        assert_eq!(patched.state.ttl_seconds, Some(60));

        let restored = state
            .patch_interface_lifecycle(
                Some(&applied.interface_id),
                None,
                InterfaceLifecyclePatch {
                    restore_previous: true,
                    ..InterfaceLifecyclePatch::default()
                },
            )
            .unwrap();

        assert!(restored.state.pinned);
        assert!(!restored.state.hidden);
        assert!(!restored.state.expired);
        assert_eq!(restored.state.expires_at_unix, None);
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::LifecyclePatched
                && event.interface_id.as_deref() == Some(applied.interface_id.as_str())
        }));
    }

    #[test]
    fn patch_interface_lifecycle_can_expire_without_retiring() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        let applied = state.apply_interface(document).unwrap();

        let patched = state
            .patch_interface_lifecycle(
                Some(&applied.interface_id),
                None,
                InterfaceLifecyclePatch {
                    expire_now: true,
                    ..InterfaceLifecyclePatch::default()
                },
            )
            .unwrap();

        assert!(patched.state.expired);
        assert!(patched.state.hidden);
        assert!(state.interfaces.contains_key(&applied.interface_id));
    }

    #[test]
    fn replace_interface_swaps_document_and_preserves_lifecycle() {
        let mut state = RuntimeState::default();
        state.apply_interface(sample_interface()).unwrap();
        state
            .patch_interface_lifecycle(
                Some("genetica.local"),
                None,
                InterfaceLifecyclePatch {
                    pinned: Some(true),
                    hidden: Some(true),
                    ..InterfaceLifecyclePatch::default()
                },
            )
            .unwrap();

        let mut replacement = sample_interface();
        replacement.id = "kernel.panel".to_string();
        replacement.title = "KERNEL PANEL".to_string();
        replacement.scope = Scope::new(ScopeKind::Project, "/tmp/owt/kernel");

        let replaced = state
            .replace_interface(Some("genetica.local"), None, replacement, true)
            .unwrap();

        assert_eq!(replaced.previous_interface_id, "genetica.local");
        assert_eq!(replaced.applied.interface_id, "kernel.panel");
        assert!(replaced.lifecycle_preserved);
        assert!(!state.interfaces.contains_key("genetica.local"));
        assert!(state.interfaces.contains_key("kernel.panel"));
        assert!(state.lifecycle_by_interface["kernel.panel"].pinned);
        assert!(state.lifecycle_by_interface["kernel.panel"].hidden);
        assert!(state.recent_events(8).iter().any(|event| {
            event.kind == RuntimeEventKind::InterfaceReplaced
                && event.interface_id.as_deref() == Some("kernel.panel")
        }));
    }

    #[test]
    fn retire_interface_removes_runtime_surface_and_dispatch_target() {
        let mut left = sample_interface();
        left.id = "genetica.local.left".to_string();
        left.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-left");

        let mut right = sample_interface();
        right.id = "genetica.local.right".to_string();
        right.scope = Scope::new(ScopeKind::Project, "/tmp/owt/genetica-right");

        let mut state = RuntimeState::default();
        state.apply_interface(left).unwrap();
        state.apply_interface(right).unwrap();
        state
            .dispatch_action_for_interface(Some("genetica.local.right"), "open.workspace")
            .unwrap();

        let retired = state
            .retire_interface(Some("genetica.local.right"), None)
            .unwrap();

        assert_eq!(retired.interface_id, "genetica.local.right");
        assert_eq!(retired.remaining_interfaces, 1);
        assert!(!state.interfaces.contains_key("genetica.local.right"));
        assert!(!state
            .active_interface_by_scope
            .values()
            .any(|interface_id| interface_id == "genetica.local.right"));
        assert!(state
            .last_dispatched_action_for_interface("genetica.local.right")
            .is_none());
        assert!(state.dispatch_action("open.workspace").is_ok());
        assert!(state
            .recent_events(8)
            .iter()
            .any(|event| event.kind == RuntimeEventKind::InterfaceRetired
                && event.interface_id.as_deref() == Some("genetica.local.right")));
    }

    #[test]
    fn record_rendered_interfaces_updates_per_interface_status_once() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        state.record_rendered_interfaces(&["genetica.local".to_string()], 7);
        state.record_rendered_interfaces(&["genetica.local".to_string()], 8);

        assert_eq!(state.last_render_pass_by_interface["genetica.local"], 8);
        assert_eq!(
            state
                .recent_events(16)
                .iter()
                .filter(|event| event.kind == RuntimeEventKind::Rendered)
                .count(),
            1
        );
    }
}
