use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::interface::{ActionKind, InterfaceDocument, Scope, UiAction, UiNode};

pub type Result<T> = std::result::Result<T, ControlError>;

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
    #[error("action {action_id} is not registered")]
    UnregisteredAction { action_id: String },
    #[error("action {action_id} requires confirmation before dispatch: {kind}")]
    DispatchRequiresConfirmation { action_id: String, kind: String },
    #[error("unknown interface id: {interface_id}")]
    UnknownInterface { interface_id: String },
    #[error("no active interface is available for update_node")]
    NoActiveInterface,
    #[error("parent node {parent_id} was not found in interface {interface_id}")]
    UnknownParent {
        interface_id: String,
        parent_id: String,
    },
    #[error("interface must contain at least one root node")]
    EmptyInterface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedInterface {
    pub scope: Scope,
    pub interface_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchedAction {
    pub action_id: String,
    pub label: String,
    pub kind: ActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub interfaces: BTreeMap<String, InterfaceDocument>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub active_interface_by_scope: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, UiAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub status_by_scope: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dispatched_action: Option<DispatchedAction>,
}

impl RuntimeState {
    pub fn apply_interface(&mut self, document: InterfaceDocument) -> Result<AppliedInterface> {
        validate_interface(&document)?;

        let scope = document.scope.clone();
        let interface_id = document.id.clone();

        for action in &document.actions {
            self.actions.insert(action.id.clone(), action.clone());
        }

        self.interfaces.insert(interface_id.clone(), document);
        self.active_interface_by_scope
            .insert(scope.key(), interface_id.clone());

        Ok(AppliedInterface {
            scope,
            interface_id,
        })
    }

    pub fn active_interface_for_scope(&self, scope: &Scope) -> Option<&InterfaceDocument> {
        let interface_id = self.active_interface_by_scope.get(&scope.key())?;
        self.interfaces.get(interface_id)
    }

    pub fn dispatch_action(&mut self, action_id: &str) -> Result<DispatchedAction> {
        require_non_empty("action_id", action_id)?;
        let action =
            self.actions
                .get(action_id)
                .ok_or_else(|| ControlError::UnregisteredAction {
                    action_id: action_id.to_string(),
                })?;

        if action.requires_confirmation
            || matches!(
                action.kind,
                ActionKind::Destructive | ActionKind::CredentialSensitive
            )
        {
            return Err(ControlError::DispatchRequiresConfirmation {
                action_id: action.id.clone(),
                kind: action_kind_name(action),
            });
        }

        let dispatched = DispatchedAction {
            action_id: action.id.clone(),
            label: action.label.clone(),
            kind: action.kind.clone(),
            command: action.command.clone(),
            target: action.target.clone(),
        };
        self.last_dispatched_action = Some(dispatched.clone());
        Ok(dispatched)
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

        self.apply_interface(document)
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

pub fn validate_interface(document: &InterfaceDocument) -> Result<()> {
    require_non_empty("interface.id", &document.id)?;
    require_non_empty("interface.title", &document.title)?;
    require_non_empty("scope.id", &document.scope.id)?;

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
        if !allowed_action_kinds.is_empty() && !allowed_action_kinds.contains(&action.kind) {
            return Err(ControlError::ActionKindNotAllowed {
                action_id: action.id.clone(),
                kind: action_kind_name(action),
            });
        }
        if matches!(
            action.kind,
            crate::interface::ActionKind::Destructive
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

fn validate_node(
    node: &UiNode,
    action_ids: &BTreeSet<String>,
    node_ids: &mut BTreeSet<String>,
) -> Result<()> {
    require_non_empty("node.id", &node.id)?;
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
        ActionKind, InterfaceDocument, Scope, ScopeKind, UiAction, UiNode, UiNodeKind,
    };

    use super::{validate_interface, ControlError, RuntimeState};

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
    fn dispatch_action_records_last_action() {
        let document = sample_interface();
        let mut state = RuntimeState::default();
        state.apply_interface(document).unwrap();

        let dispatched = state.dispatch_action("open.workspace").unwrap();

        assert_eq!(dispatched.action_id, "open.workspace");
        assert_eq!(
            state.last_dispatched_action.as_ref().unwrap().label,
            "Open workspace"
        );
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
}
