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
    Refresh,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactState {
    Observed,
    Inferred,
    Stale,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FactProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collected_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<FactState>,
}

impl UiAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: ActionKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            command: None,
            argv: Vec::new(),
            cwd: None,
            mode: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<FactProvenance>,
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
            provenance: None,
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
    pub tab_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_action_kinds: Vec<ActionKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<UiAction>,
    pub nodes: Vec<UiNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabTitleIntent {
    Set(String),
    Preserve,
}

impl InterfaceDocument {
    pub fn new(id: impl Into<String>, title: impl Into<String>, scope: Scope) -> Self {
        Self {
            schema_version: 1,
            id: id.into(),
            title: title.into(),
            scope,
            tab_title: None,
            theme: None,
            allowed_action_kinds: Vec::new(),
            actions: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub fn tab_title_intent(&self) -> Option<TabTitleIntent> {
        self.tab_title
            .as_deref()
            .and_then(parse_tab_title_intent)
            .or_else(|| {
                self.nodes
                    .first()
                    .and_then(|node| string_property(&node.properties, &["tab_title", "tab-title"]))
                    .and_then(parse_tab_title_intent)
            })
    }

    pub fn resolved_tab_title(&self) -> Option<String> {
        match self.tab_title_intent() {
            Some(TabTitleIntent::Set(title)) => Some(title),
            Some(TabTitleIntent::Preserve) => None,
            None => fallback_tab_title_for_scope(&self.scope),
        }
    }
}

fn parse_tab_title_intent(value: &str) -> Option<TabTitleIntent> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if matches!(
        value.to_ascii_lowercase().as_str(),
        "preserve" | "unchanged" | "keep" | "none"
    ) {
        return Some(TabTitleIntent::Preserve);
    }

    Some(TabTitleIntent::Set(value.to_string()))
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

fn fallback_tab_title_for_scope(scope: &Scope) -> Option<String> {
    if !matches!(
        scope.kind,
        ScopeKind::Project | ScopeKind::Workspace | ScopeKind::Session
    ) {
        return None;
    }

    let scope_id = scope
        .id
        .trim()
        .trim_end_matches(|ch| ch == '/' || ch == '\\');
    if scope_id.is_empty() {
        return None;
    }

    let candidate =
        if scope_id.chars().count() > 32 || scope_id.contains('/') || scope_id.contains('\\') {
            scope_id
                .rsplit(|ch| ch == '/' || ch == '\\')
                .find(|segment| !segment.trim().is_empty())
                .unwrap_or(scope_id)
        } else {
            scope_id
        };

    let candidate = candidate.trim();
    if candidate.is_empty() {
        None
    } else {
        Some(shorten_tab_title(candidate))
    }
}

fn shorten_tab_title(value: &str) -> String {
    const MAX_TAB_TITLE_CHARS: usize = 32;
    let char_count = value.chars().count();
    if char_count <= MAX_TAB_TITLE_CHARS {
        return value.to_string();
    }

    let keep = MAX_TAB_TITLE_CHARS.saturating_sub(3);
    let mut shortened = value.chars().take(keep).collect::<String>();
    shortened.push_str("...");
    shortened
}

#[cfg(test)]
mod tests {
    use super::{
        FactProvenance, FactState, InterfaceDocument, Scope, ScopeKind, TabTitleIntent, UiNode,
        UiNodeKind,
    };

    #[test]
    fn resolved_tab_title_uses_explicit_document_title() {
        let scope = Scope::new(
            ScopeKind::Project,
            "/home/buanzo/git/tools/python/carriertv",
        );
        let mut document = InterfaceDocument::new("carriertv.ops", "CARRIERTV OPS", scope);
        document.tab_title = Some("CTV OPS".to_string());

        assert_eq!(
            document.tab_title_intent(),
            Some(TabTitleIntent::Set("CTV OPS".to_string()))
        );
        assert_eq!(document.resolved_tab_title(), Some("CTV OPS".to_string()));
    }

    #[test]
    fn resolved_tab_title_can_preserve_existing_title() {
        let scope = Scope::new(
            ScopeKind::Project,
            "/home/buanzo/git/tools/python/carriertv",
        );
        let mut document = InterfaceDocument::new("carriertv.ops", "CARRIERTV OPS", scope);
        document.tab_title = Some("preserve".to_string());

        assert_eq!(document.tab_title_intent(), Some(TabTitleIntent::Preserve));
        assert_eq!(document.resolved_tab_title(), None);
    }

    #[test]
    fn resolved_tab_title_uses_root_property_before_scope_fallback() {
        let scope = Scope::new(ScopeKind::Project, "/home/buanzo/git/tools/data/genetica");
        let mut document = InterfaceDocument::new("genetica.local", "GENETICA", scope);
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.properties
            .insert("tab_title".to_string(), "GEN OPS".to_string());
        document.nodes.push(root);

        assert_eq!(document.resolved_tab_title(), Some("GEN OPS".to_string()));
    }

    #[test]
    fn resolved_tab_title_falls_back_to_scope_final_segment() {
        let scope = Scope::new(
            ScopeKind::Project,
            "/home/buanzo/git/tools/python/carriertv",
        );
        let document = InterfaceDocument::new("carriertv.ops", "CARRIERTV OPS", scope);

        assert_eq!(document.resolved_tab_title(), Some("carriertv".to_string()));
    }

    #[test]
    fn ui_node_provenance_round_trips_as_structured_fact_metadata() {
        let mut node = UiNode::new("metric.kernel", UiNodeKind::Metric);
        node.provenance = Some(FactProvenance {
            source: Some("local:/proc".to_string()),
            collected_at: Some("2026-05-18T00:00:00Z".to_string()),
            host: Some("local".to_string()),
            command: Some("read:/proc/uptime".to_string()),
            confidence: Some("high".to_string()),
            error: None,
            state: Some(FactState::Observed),
        });

        let encoded = serde_json::to_string(&node).unwrap();
        assert!(encoded.contains("\"provenance\""));
        assert!(encoded.contains("\"state\":\"observed\""));

        let decoded: UiNode = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.provenance, node.provenance);
    }
}
