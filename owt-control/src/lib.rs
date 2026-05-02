//! Shared OWT control-plane types.
//!
//! This crate is intentionally small and side-effect free. It is the typed core
//! that native `OWT.exe` can embed before any transport, renderer, or GUI wiring
//! is attached.

pub mod interface;
pub mod protocol;
pub mod runtime;

pub use interface::{
    ActionKind, InterfaceDocument, Scope, ScopeKind, UiAction, UiNode, UiNodeKind,
};
pub use protocol::{
    ControlStatus, EndpointMode, NATIVE_LIFECYCLE_TOOLS, PROTOCOL_VERSION, PROTOTYPE_COMPAT_TOOLS,
};
pub use runtime::{
    validate_interface, AppliedInterface, ControlError, DispatchedAction, Result, RuntimeState,
};
