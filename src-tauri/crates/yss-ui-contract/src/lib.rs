//! Closed presentation protocol shared by desktop clients and automation.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_ELEMENTS: usize = 128;
pub const MAX_DEPTH: usize = 12;
pub const MAX_SPEC_BYTES: usize = 65_536;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSource {
    pub execution_session_id: String,
    pub result_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReportSection {
    Equation,
    ModelSummary,
    Anova,
    CoefficientTable,
    HypothesisTest,
    Diagnostics,
    ResidualPlot,
    Observations,
    AcfPacf,
    SerialTests,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UiPanel {
    Project,
    Nodes,
    Commands,
    Details,
    Assistant,
    Problems,
    Output,
    Logs,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UiIntent {
    OpenGraph {
        graph_path: String,
        node_id: Option<String>,
    },
    OpenResult {
        source: UiSource,
    },
    ShowPanel {
        panel: UiPanel,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "props",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum UiComponent {
    Column { gap: u8 },
    Row { gap: u8 },
    Text { text: String },
    ReportSection { section: ReportSection },
    Button { label: String, intent: UiIntent },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiElement {
    pub component: UiComponent,
    pub visible: bool,
    pub children: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSpec {
    pub root: String,
    pub elements: BTreeMap<String, UiElement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid UI specification")]
pub struct InvalidUiSpec;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        && !matches!(id, "__proto__" | "prototype" | "constructor")
}

impl UiSpec {
    pub fn validate(&self) -> Result<(), InvalidUiSpec> {
        if self.elements.is_empty()
            || self.elements.len() > MAX_ELEMENTS
            || serde_json::to_vec(self).map_err(|_| InvalidUiSpec)?.len() > MAX_SPEC_BYTES
        {
            return Err(InvalidUiSpec);
        }
        let mut visited = BTreeSet::new();
        self.visit(&self.root, 0, &mut visited)?;
        if visited.len() != self.elements.len() {
            return Err(InvalidUiSpec);
        }
        Ok(())
    }

    fn visit<'a>(
        &'a self,
        id: &'a str,
        depth: usize,
        visited: &mut BTreeSet<&'a str>,
    ) -> Result<(), InvalidUiSpec> {
        if depth > MAX_DEPTH || !valid_id(id) || !visited.insert(id) {
            return Err(InvalidUiSpec);
        }
        let element = self.elements.get(id).ok_or(InvalidUiSpec)?;
        match &element.component {
            UiComponent::Column { gap } | UiComponent::Row { gap } if *gap <= 8 => {}
            UiComponent::Text { text } if text.len() <= 8_192 && element.children.is_empty() => {}
            UiComponent::ReportSection { .. } if element.children.is_empty() => {}
            UiComponent::Button { label, intent }
                if !label
                    .trim_matches(|character: char| {
                        character.is_whitespace() || character == '\u{feff}'
                    })
                    .is_empty()
                    && label.len() <= 256
                    && element.children.is_empty() =>
            {
                intent.validate()?;
            }
            _ => return Err(InvalidUiSpec),
        }
        for child in &element.children {
            self.visit(child, depth + 1, visited)?;
        }
        Ok(())
    }

    pub fn regression_report() -> Self {
        use ReportSection::*;
        let sections = [
            Equation,
            ModelSummary,
            Anova,
            CoefficientTable,
            HypothesisTest,
            Diagnostics,
            ResidualPlot,
            Observations,
            AcfPacf,
            SerialTests,
        ];
        let mut elements = BTreeMap::new();
        let mut children = Vec::new();
        for section in sections {
            let id = serde_json::to_value(section)
                .expect("section serializes")
                .as_str()
                .unwrap()
                .to_owned();
            children.push(id.clone());
            elements.insert(
                id,
                UiElement {
                    component: UiComponent::ReportSection { section },
                    visible: true,
                    children: vec![],
                },
            );
        }
        elements.insert(
            "report".into(),
            UiElement {
                component: UiComponent::Column { gap: 4 },
                visible: true,
                children,
            },
        );
        Self {
            root: "report".into(),
            elements,
        }
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

impl UiSource {
    pub fn validate(&self) -> Result<(), InvalidUiSpec> {
        if !valid_uuid(&self.execution_session_id)
            || self.result_id.is_empty()
            || self.result_id.len() > 20
            || self.result_id.starts_with('0')
            || !self.result_id.bytes().all(|b| b.is_ascii_digit())
            || self.result_id.parse::<u64>().is_err()
        {
            return Err(InvalidUiSpec);
        }
        Ok(())
    }
}

impl UiIntent {
    pub fn validate(&self) -> Result<(), InvalidUiSpec> {
        match self {
            Self::OpenGraph {
                graph_path,
                node_id,
            } => {
                if graph_path.is_empty()
                    || graph_path.len() > 4096
                    || node_id.as_ref().is_some_and(|id| !valid_uuid(id))
                {
                    return Err(InvalidUiSpec);
                }
            }
            Self::OpenResult { source } => source.validate()?,
            Self::ShowPanel { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiPage {
    pub source: UiSource,
    pub revision: u64,
    pub spec: UiSpec,
}

/// Stable-element operations; this intentionally is not an unrestricted JSON Pointer protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum UiPatch {
    Set { id: String, element: UiElement },
    Remove { id: String },
    Root { id: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UiUpdate {
    Snapshot {
        page: UiPage,
    },
    Patch {
        source: UiSource,
        base_revision: u64,
        revision: u64,
        operations: Vec<UiPatch>,
    },
}

pub fn diff(previous: &UiPage, next: &UiPage) -> UiUpdate {
    let mut operations = Vec::new();
    for (id, element) in &next.spec.elements {
        if previous.spec.elements.get(id) != Some(element) {
            operations.push(UiPatch::Set {
                id: id.clone(),
                element: element.clone(),
            });
        }
    }
    for id in previous.spec.elements.keys() {
        if !next.spec.elements.contains_key(id) {
            operations.push(UiPatch::Remove { id: id.clone() });
        }
    }
    if previous.spec.root != next.spec.root {
        operations.push(UiPatch::Root {
            id: next.spec.root.clone(),
        });
    }
    UiUpdate::Patch {
        source: next.source.clone(),
        base_revision: previous.revision,
        revision: next.revision,
        operations,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum UiAction {
    Replace { spec: UiSpec },
    Patch { operations: Vec<UiPatch> },
    Visibility { id: String, visible: bool },
    Move { id: String, offset: i8 },
    Reset,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateUiRequest {
    pub source: UiSource,
    pub base_revision: u64,
    pub action: UiAction,
}

impl UpdateUiRequest {
    pub fn validate(&self) -> Result<(), InvalidUiSpec> {
        self.source.validate()?;
        if self.base_revision == 0
            || self.base_revision > 9_007_199_254_740_991
            || serde_json::to_vec(self).map_err(|_| InvalidUiSpec)?.len() > MAX_SPEC_BYTES * 2
        {
            return Err(InvalidUiSpec);
        }
        match &self.action {
            UiAction::Replace { spec } => spec.validate(),
            UiAction::Patch { operations }
                if operations.is_empty() || operations.len() > MAX_ELEMENTS * 2 + 1 =>
            {
                Err(InvalidUiSpec)
            }
            UiAction::Visibility { id, .. } | UiAction::Move { id, .. } if !valid_id(id) => {
                Err(InvalidUiSpec)
            }
            UiAction::Move { offset, .. } if !matches!(offset, -1 | 1) => Err(InvalidUiSpec),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateUiRequest {
    pub source: UiSource,
    pub base_revision: u64,
    pub id: String,
    pub client_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestUiIntent {
    pub client_key: String,
    pub intent: UiIntent,
}

impl RequestUiIntent {
    pub fn validate(&self) -> Result<(), InvalidUiSpec> {
        if !valid_id(&self.client_key) {
            return Err(InvalidUiSpec);
        }
        self.intent.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UiIntentStatus {
    Pending,
    Claimed,
    Applied,
    Failed,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiIntentReceipt {
    pub id: String,
    pub intent: UiIntent,
    pub status: UiIntentStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum InspectUiRequest {
    Catalog,
    Page { source: UiSource },
    Intent { id: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UiInspection {
    Catalog {
        schema: serde_json::Value,
        max_elements: usize,
        max_depth: usize,
        max_spec_bytes: usize,
    },
    Page {
        page: UiPage,
    },
    Intent {
        receipt: UiIntentReceipt,
    },
}

pub fn catalog() -> UiInspection {
    UiInspection::Catalog {
        schema: serde_json::to_value(schemars::schema_for!(UiSpec)).expect("schema serializes"),
        max_elements: MAX_ELEMENTS,
        max_depth: MAX_DEPTH,
        max_spec_bytes: MAX_SPEC_BYTES,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum UiEvent {
    Update { update: UiUpdate },
    Intent { receipt: UiIntentReceipt },
    Resync,
    SessionChanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_spec_rejects_unknown_props_cycles_and_orphans() {
        let mut spec = UiSpec::regression_report();
        let children = std::mem::replace(
            &mut spec.elements.get_mut("report").unwrap().children,
            vec!["row".into()],
        );
        spec.elements.insert(
            "row".into(),
            UiElement {
                component: UiComponent::Row { gap: 2 },
                visible: true,
                children,
            },
        );
        spec.elements.insert(
            "assistant".into(),
            UiElement {
                component: UiComponent::Button {
                    label: "Assistant".into(),
                    intent: UiIntent::ShowPanel {
                        panel: UiPanel::Assistant,
                    },
                },
                visible: true,
                children: vec![],
            },
        );
        spec.elements
            .get_mut("row")
            .unwrap()
            .children
            .push("assistant".into());
        spec.validate().unwrap();
        assert_eq!(
            serde_json::from_value::<UiSpec>(serde_json::to_value(&spec).unwrap()).unwrap(),
            spec
        );
        let mut raw = serde_json::to_value(&spec).unwrap();
        raw["elements"]["report"]["component"]["props"]["script"] = "alert(1)".into();
        assert!(serde_json::from_value::<UiSpec>(raw).is_err());
        let mut cycle = spec.clone();
        cycle
            .elements
            .get_mut("report")
            .unwrap()
            .children
            .push("report".into());
        assert!(cycle.validate().is_err());
        let mut orphan = spec;
        orphan.elements.get_mut("report").unwrap().children.pop();
        assert!(orphan.validate().is_err());
        assert!(
            UiIntent::OpenGraph {
                graph_path: "events/test.yssbi-event".into(),
                node_id: Some("x".repeat(36))
            }
            .validate()
            .is_err()
        );
        assert!(
            UiSource {
                execution_session_id: "x".repeat(36),
                result_id: "1".into()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn diff_only_replaces_changed_elements_and_preserves_the_base_revision() {
        let before = UiPage {
            source: UiSource {
                execution_session_id: "session".into(),
                result_id: "1".into(),
            },
            revision: 1,
            spec: UiSpec::regression_report(),
        };
        let mut after = before.clone();
        after.revision = 2;
        after.spec.elements.get_mut("anova").unwrap().visible = false;
        let UiUpdate::Patch {
            base_revision,
            revision,
            operations,
            ..
        } = diff(&before, &after)
        else {
            panic!()
        };
        assert_eq!((base_revision, revision), (1, 2));
        assert_eq!(operations.len(), 1);
        assert!(matches!(&operations[0], UiPatch::Set { id, .. } if id == "anova"));
    }
}
