use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Pane,
    Tab,
    Window,
    Workspace,
    Project,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Scope {
    pub kind: ScopeKind,
    pub id: String,
}

impl Scope {
    pub fn new(kind: ScopeKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }

    pub fn key(&self) -> String {
        format!("{:?}:{}", self.kind, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Inspect,
    Open,
    Navigate,
    Edit,
    Run,
    Network,
    Destructive,
    CredentialSensitive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAction {
    pub id: String,
    pub label: String,
    pub kind: ActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
}

impl UiAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: ActionKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            command: None,
            target: None,
            requires_confirmation: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeKind {
    Panel,
    Region,
    Group,
    Frame,
    SideRail,
    ContentBay,
    Text,
    Bar,
    BarRun,
    Elbow,
    CommandGrid,
    DataCascade,
    Button,
    Badge,
    List,
    Table,
    Metric,
    Progress,
    Image,
    Spacer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiNode {
    pub id: String,
    pub kind: UiNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UiNode>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

impl UiNode {
    pub fn new(id: impl Into<String>, kind: UiNodeKind) -> Self {
        Self {
            id: id.into(),
            kind,
            label: None,
            text: None,
            role: None,
            action_id: None,
            children: Vec::new(),
            properties: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceDocument {
    pub schema_version: u16,
    pub id: String,
    pub title: String,
    pub scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_action_kinds: Vec<ActionKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<UiAction>,
    pub nodes: Vec<UiNode>,
}

impl InterfaceDocument {
    pub fn new(id: impl Into<String>, title: impl Into<String>, scope: Scope) -> Self {
        Self {
            schema_version: 1,
            id: id.into(),
            title: title.into(),
            scope,
            theme: None,
            allowed_action_kinds: Vec::new(),
            actions: Vec::new(),
            nodes: Vec::new(),
        }
    }
}
