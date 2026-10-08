use crate::file::{FileContent, FilePath, FileState, validate_file_size};
use serde::{Deserialize, Serialize};
mod query;
pub use query::*;
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
        validate_file_size(bytes.len())?;
        String::from_utf8(bytes.to_vec())
            .map(Self)
            .map_err(|e| e.to_string())
    }
    fn encode(&self) -> Result<Vec<u8>, String> {
        validate_file_size(self.0.len())?;
        Ok(self.0.as_bytes().to_vec())
    }
    fn apply(&mut self, edit: DocEdit) -> Result<(), String> {
        let DocEdit::SetMarkdown { markdown } = edit;
        validate_file_size(markdown.len())?;
        self.0 = markdown;
        Ok(())
    }
    fn duplicate(&self, _next_id: &mut dyn FnMut() -> String) -> Self {
        self.clone()
    }
    fn fingerprint(&self) -> Result<String, String> {
        validate_file_size(self.0.len())?;
        Ok(yss_canonical_hash::content_sha256(self.0.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::MAX_FILE_BYTES;

    #[test]
    fn oversized_markdown_edit_preserves_current_content_and_exact_identity() {
        let mut document = DocDocument("# 报告\n🙂\r\n".into());
        let original = document.clone();
        let identity = yss_canonical_hash::content_sha256(document.0.as_bytes());
        assert_eq!(document.fingerprint().unwrap(), identity);
        let error = document.apply(DocEdit::SetMarkdown {
            markdown: "é".repeat(MAX_FILE_BYTES / 2 + 1),
        });
        assert!(error.is_err());
        assert!(
            document == original,
            "rejected edit replaced the current body"
        );
        assert_eq!(document.fingerprint().unwrap(), identity);
    }
}
