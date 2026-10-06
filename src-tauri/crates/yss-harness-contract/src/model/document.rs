//! Business-facing Markdown reads and edits. Source and edit versions stay with the owners.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod reads;
pub use reads::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentResourceKind {
    Doc,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentResourceRef {
    pub kind: DocumentResourceKind,
    pub id: String,
}
impl DocumentResourceRef {
    pub fn new(id: String) -> Self {
        Self {
            kind: DocumentResourceKind::Doc,
            id,
        }
    }
    pub fn resource(&self) -> ProjectResourceRef {
        ProjectResourceRef {
            kind: ProjectResourceKind::Doc,
            id: self.id.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentSectionRef {
    /// Use the complete reference returned by inspect_document or search_document.
    pub heading_index: usize,
    /// Absolute Unicode character position of the heading, not a byte or UTF-16 offset.
    pub start: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentCharacterRange {
    pub start: usize,
    /// Exclusive Unicode character position.
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceDocumentTextInput {
    pub document: DocumentResourceRef,
    /// Applied in order to one candidate. Each oldText must have exactly one occurrence at that step.
    #[schemars(length(min = 1, max = 200))]
    pub replacements: Vec<DocumentTextReplacement>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentTextReplacement {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppendDocumentInput {
    pub document: DocumentResourceRef,
    /// Appended exactly, without inserting separators. Include any required newlines.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteDocumentInput {
    pub document: DocumentResourceRef,
    /// The complete new Markdown, including content outside any previously read page.
    /// Use only when whole-document replacement is intended. The original file-size limit applies.
    pub markdown: String,
}
