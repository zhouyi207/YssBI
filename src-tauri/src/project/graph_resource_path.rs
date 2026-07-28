//! Event/Function 项目资源的稳定身份：相对于项目根目录的规范化路径。

use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::project_error::ProjectError;
use super::{EVENT_EXTENSION, EVENTS_DIR, FUNCTION_EXTENSION, FUNCTIONS_DIR, GraphDocumentKind};

/// 规范化相对路径，例如 `events/MyEvent.yssbi-event`。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphResourcePath(String);

impl GraphResourcePath {
    pub fn new(path: impl Into<String>) -> Result<Self, ProjectError> {
        let normalized = normalize_graph_resource_path(&path.into());
        validate_graph_resource_path(&normalized)?;
        Ok(Self(normalized))
    }

    pub fn from_normalized_unchecked(path: String) -> Self {
        Self(path)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn kind(&self) -> Result<GraphDocumentKind, ProjectError> {
        graph_kind_from_path(self.as_str())
    }

    pub fn display_name(&self) -> &str {
        self.as_str()
            .rsplit('/')
            .next()
            .and_then(|file| file.rsplit_once('.'))
            .map(|(stem, _)| stem)
            .unwrap_or(self.as_str())
    }

    /// 供前端 ResourceKey 使用，避免裸 `/` 破坏分隔符。
    pub fn encode_for_resource_key(&self) -> String {
        self.0.replace('/', "::")
    }

    pub fn decode_from_resource_key(encoded: &str) -> Result<Self, ProjectError> {
        Self::new(encoded.replace("::", "/"))
    }
}

impl Hash for GraphResourcePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Display for GraphResourcePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for GraphResourcePath {
    type Err = ProjectError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for GraphResourcePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

pub fn normalize_graph_resource_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn validate_graph_resource_path(path: &str) -> Result<(), ProjectError> {
    if path.is_empty() {
        return Err(ProjectError::InvalidProjectFormat(
            "graph resource path cannot be empty".into(),
        ));
    }
    graph_kind_from_path(path)?;
    Ok(())
}

pub fn graph_kind_from_path(path: &str) -> Result<GraphDocumentKind, ProjectError> {
    let normalized = normalize_graph_resource_path(path);
    if normalized.starts_with(&format!("{EVENTS_DIR}/"))
        && normalized.ends_with(&format!(".{EVENT_EXTENSION}"))
    {
        return Ok(GraphDocumentKind::Event);
    }
    if normalized.starts_with(&format!("{FUNCTIONS_DIR}/"))
        && normalized.ends_with(&format!(".{FUNCTION_EXTENSION}"))
    {
        return Ok(GraphDocumentKind::Function);
    }
    Err(ProjectError::InvalidProjectFormat(format!(
        "invalid graph resource path '{path}'"
    )))
}

/// Logical URI for graph resources (`yssbi://graph/{kind}/{encodedPath}`).
pub fn to_graph_resource_uri(kind: GraphDocumentKind, path: &GraphResourcePath) -> String {
    let kind_str = match kind {
        GraphDocumentKind::Event => "event",
        GraphDocumentKind::Function => "function",
    };
    format!(
        "yssbi://graph/{kind_str}/{}",
        path.encode_for_resource_key()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates_event_path() {
        let path = GraphResourcePath::new(r"events\Foo.yssbi-event").unwrap();
        assert_eq!(path.as_str(), "events/Foo.yssbi-event");
        assert_eq!(path.display_name(), "Foo");
        assert_eq!(path.kind().unwrap(), GraphDocumentKind::Event);
    }

    #[test]
    fn resource_key_round_trip() {
        let path = GraphResourcePath::new("functions/Helper.yssbi-function").unwrap();
        let encoded = path.encode_for_resource_key();
        let decoded = GraphResourcePath::decode_from_resource_key(&encoded).unwrap();
        assert_eq!(decoded, path);
    }

    #[test]
    fn graph_resource_uri_encodes_path_segments() {
        let path = GraphResourcePath::new("functions/My Fn.yssbi-function").unwrap();
        assert_eq!(
            to_graph_resource_uri(GraphDocumentKind::Function, &path),
            "yssbi://graph/function/functions::My Fn.yssbi-function"
        );
    }

    #[test]
    fn rejects_untitled_graph_path() {
        assert!(GraphResourcePath::new("untitled:function:Untitled-1").is_err());
    }
}
