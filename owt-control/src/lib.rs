//! Shared OWT control-plane types.
//!
//! This crate is intentionally small and side-effect free. It is the typed core
//! that native `OWT.exe` can embed before any transport, renderer, or GUI wiring
//! is attached.

pub mod interface;
pub mod protocol;
pub mod runtime;

pub use interface::{
    ActionKind, FactProvenance, FactState, InterfaceDocument, Scope, ScopeKind, TabTitleIntent,
    UiAction, UiNode, UiNodeKind,
};
pub use protocol::{
    ControlStatus, EndpointMode, InterfaceOwner, InterfaceRuntimeStatus, NATIVE_LIFECYCLE_TOOLS,
    PROTOCOL_VERSION, PROTOTYPE_COMPAT_TOOLS,
};
pub use runtime::{
    diff_interface_documents, interface_validation_warnings, reflow_interface_documents_for_scene,
    validate_interface, ActionProvenance, AppliedInterface, ControlError, DispatchedAction,
    EditedInterface, FocusedTableCell, FocusedTableGroup, FocusedTableRow, InterfaceDiff,
    InterfaceEditOperation, InterfaceEditOperationResult, InterfaceLifecyclePatch,
    InterfaceLifecycleState, InterfaceValidationWarning, LayoutFitScore, LayoutOverflowEstimate,
    LayoutPreview, LayoutReservedSpaceEstimate, LayoutSceneActionSlot, LayoutScenePlan,
    LayoutSceneSlot, LayoutSceneSurface, LayoutViewportFit, PatchedInterfaceLayout,
    PatchedInterfaceLifecycle, PermissionDecision, ReplacedInterface, RequestApprovalState,
    RequestedRefresh, Result, RetiredInterface, RuntimeEvent, RuntimeEventKind, RuntimeState,
    TableCellFocusMovement, TableFocusMovement,
};
