use super::*;
fn outline_limit() -> usize {
    50
}
fn text_limit() -> usize {
    8192
}
fn search_limit() -> usize {
    20
}
fn context_limit() -> usize {
    120
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDocumentInput {
    pub document: DocumentResourceRef,
    /// Omit to refresh the complete document's outline. A supplied section must still match the current document contents.
    #[serde(default)]
    pub section: Option<DocumentSectionRef>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "outline_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadDocumentInput {
    pub document: DocumentResourceRef,
    #[serde(default)]
    pub section: Option<DocumentSectionRef>,
    /// Absolute Unicode range. Cannot be combined with section.
    #[serde(default)]
    pub range: Option<DocumentCharacterRange>,
    /// Unicode character offset relative to the selected section/range or whole document.
    #[serde(default)]
    pub offset: usize,
    /// Actual pages may finish earlier at a paragraph or heading boundary. Follow page.nextOffset.
    #[serde(default = "text_limit")]
    #[schemars(range(min = 1, max = 16384))]
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchDocumentInput {
    pub document: DocumentResourceRef,
    /// Case-sensitive literal search of Markdown source, including overlapping matches.
    pub query: String,
    #[serde(default)]
    pub section: Option<DocumentSectionRef>,
    #[serde(default)]
    pub range: Option<DocumentCharacterRange>,
    /// Offset in matching occurrences.
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "search_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
    /// Maximum Unicode context on each side of a match, within the selected scope.
    #[serde(default = "context_limit")]
    #[schemars(range(max = 500))]
    pub context_characters: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "query",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DocumentReadInput {
    Outline(InspectDocumentInput),
    Text(ReadDocumentInput),
    Search(SearchDocumentInput),
}

impl DocumentReadInput {
    pub fn document(&self) -> &DocumentResourceRef {
        match self {
            Self::Outline(v) => &v.document,
            Self::Text(v) => &v.document,
            Self::Search(v) => &v.document,
        }
    }
    pub fn section(&self) -> Option<&DocumentSectionRef> {
        match self {
            Self::Outline(v) => v.section.as_ref(),
            Self::Text(v) => v.section.as_ref(),
            Self::Search(v) => v.section.as_ref(),
        }
    }
    pub fn range(&self) -> Option<&DocumentCharacterRange> {
        match self {
            Self::Outline(_) => None,
            Self::Text(v) => v.range.as_ref(),
            Self::Search(v) => v.range.as_ref(),
        }
    }
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::Outline(_) => CapabilityId::InspectDocument,
            Self::Text(_) => CapabilityId::ReadDocument,
            Self::Search(_) => CapabilityId::SearchDocument,
        }
    }
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("document.id", &self.document().id)?;
        if self.section().is_some() && self.range().is_some() {
            return Err(CapabilityContractError::InvalidField("range"));
        }
        if self.range().is_some_and(|range| range.start > range.end) {
            return Err(CapabilityContractError::InvalidField("range"));
        }
        let (limit, maximum) = match self {
            Self::Outline(v) => (v.limit, 100),
            Self::Text(v) => (v.limit, 16_384),
            Self::Search(v) => {
                if v.query.is_empty() || v.query.len() > 4096 {
                    return Err(CapabilityContractError::InvalidField("query"));
                }
                if v.context_characters > 500 {
                    return Err(CapabilityContractError::InvalidField("contextCharacters"));
                }
                (v.limit, 100)
            }
        };
        if limit == 0 || limit > maximum {
            return Err(CapabilityContractError::InvalidLimit {
                maximum: maximum as u16,
            });
        }
        Ok(())
    }
}
