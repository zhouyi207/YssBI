use crate::file::{FileContent, FilePath, FileState, bounded};
use serde::{Deserialize, Serialize};
pub type DocPath = FilePath<DocDocument>;
pub type DocState = FileState<DocDocument>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocDocument(pub String);
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocEdit {
    SetMarkdown { markdown: String },
}
impl FileContent for DocDocument {
    type Edit = DocEdit;
    const KIND: &'static str = "doc";
    const DIRECTORY: &'static str = yss_project_layout::DOCS_DIR;
    const EXTENSION: &'static str = yss_project_layout::DOC_EXTENSION;
    fn new(_title: &str, _next_id: &mut dyn FnMut() -> String) -> Self {
        Self(String::new())
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        String::from_utf8(bounded(bytes.to_vec())?)
            .map(Self)
            .map_err(|e| e.to_string())
    }
    fn encode(&self) -> Result<Vec<u8>, String> {
        bounded(self.0.as_bytes().to_vec())
    }
    fn apply(&mut self, edit: DocEdit) -> Result<(), String> {
        let DocEdit::SetMarkdown { markdown } = edit;
        self.0 = markdown;
        self.encode().map(|_| ())
    }
    fn duplicate(&self, _next_id: &mut dyn FnMut() -> String) -> Self {
        self.clone()
    }
}
