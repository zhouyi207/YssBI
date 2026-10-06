//! Internal read identity and explicit Markdown business facts.
use crate::*;
use model::{DocumentCharacterRange, DocumentResourceRef, DocumentSectionRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentReadRequest {
    pub input: model::DocumentReadInput,
    pub version: Option<ResourceVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentReadResult {
    pub document: DocumentResourceRef,
    pub version: ResourceVersion,
    pub dirty: bool,
    pub content: DocumentReadContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DocumentReadContent {
    Outline {
        character_count: usize,
        heading_count: usize,
        headings: Vec<DocumentHeading>,
        page: InspectionPage,
    },
    Text {
        markdown: String,
        range: DocumentCharacterRange,
        scope: DocumentCharacterRange,
        page: InspectionPage,
        complete: bool,
        section: Option<DocumentSectionRef>,
    },
    Search {
        matches: Vec<DocumentSearchMatch>,
        page: InspectionPage,
    },
}

impl DocumentReadContent {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::Outline { .. } => CapabilityId::InspectDocument,
            Self::Text { .. } => CapabilityId::ReadDocument,
            Self::Search { .. } => CapabilityId::SearchDocument,
        }
    }
    pub fn sections(&self) -> Vec<&DocumentSectionRef> {
        match self {
            Self::Outline { headings, .. } => {
                headings.iter().map(|heading| &heading.section).collect()
            }
            Self::Text { section, .. } => section.iter().collect(),
            Self::Search { matches, .. } => matches
                .iter()
                .filter_map(|found| found.section.as_ref())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHeading {
    pub section: DocumentSectionRef,
    pub title: String,
    pub title_complete: bool,
    pub level: u8,
    pub parent_heading_index: Option<usize>,
    pub body_start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchMatch {
    pub range: DocumentCharacterRange,
    pub context: String,
    pub context_range: DocumentCharacterRange,
    pub section: Option<DocumentSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEditReceipt {
    /// Ranges at each ordered edit step, counted in Unicode characters.
    pub changes: Vec<DocumentTextChange>,
    pub character_count: usize,
    pub dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTextChange {
    pub before: DocumentCharacterRange,
    pub after: DocumentCharacterRange,
}
