use super::validate_resource_name;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

const EVENTS_DIR: &str = "events";
const FUNCTIONS_DIR: &str = "functions";
const EVENT_EXTENSION: &str = "yssbi-event";
const FUNCTION_EXTENSION: &str = "yssbi-function";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphResourceKind {
    Event,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphResourcePathError {
    Empty,
    Nested,
    WrongDirectoryOrExtension,
    InvalidName,
}

impl fmt::Display for GraphResourcePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("graph resource path cannot be empty"),
            Self::Nested => formatter.write_str("graph resource path must not be nested"),
            Self::WrongDirectoryOrExtension => {
                formatter.write_str("graph resource path has an invalid directory or extension")
            }
            Self::InvalidName => formatter.write_str("graph resource path has an invalid name"),
        }
    }
}

impl std::error::Error for GraphResourcePathError {}

/// Opaque serialized identity for an event or function graph resource.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GraphResourcePath(String);

impl GraphResourcePath {
    pub fn new(path: impl Into<String>) -> Result<Self, GraphResourcePathError> {
        let normalized = normalize_graph_resource_path(&path.into());
        validate_graph_resource_path(&normalized)?;
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn kind(&self) -> GraphResourceKind {
        if self.0.starts_with("events/") {
            GraphResourceKind::Event
        } else {
            GraphResourceKind::Function
        }
    }

    pub fn display_name(&self) -> &str {
        self.as_str()
            .rsplit('/')
            .next()
            .and_then(|file| file.rsplit_once('.'))
            .map(|(stem, _)| stem)
            .unwrap_or(self.as_str())
    }

    pub fn encode_for_resource_key(&self) -> String {
        self.0.replace('/', "::")
    }

    pub fn decode_from_resource_key(encoded: &str) -> Result<Self, GraphResourcePathError> {
        Self::new(encoded.replace("::", "/"))
    }
}

impl fmt::Display for GraphResourcePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<String> for GraphResourcePath {
    type Error = GraphResourcePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GraphResourcePath> for String {
    fn from(value: GraphResourcePath) -> Self {
        value.0
    }
}

impl FromStr for GraphResourcePath {
    type Err = GraphResourcePathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
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

fn validate_graph_resource_path(path: &str) -> Result<(), GraphResourcePathError> {
    if path.is_empty() {
        return Err(GraphResourcePathError::Empty);
    }
    if path.split('/').count() != 2 {
        return Err(GraphResourcePathError::Nested);
    }
    let stem = if let Some(file) = path.strip_prefix(&format!("{EVENTS_DIR}/")) {
        file.strip_suffix(&format!(".{EVENT_EXTENSION}"))
    } else if let Some(file) = path.strip_prefix(&format!("{FUNCTIONS_DIR}/")) {
        file.strip_suffix(&format!(".{FUNCTION_EXTENSION}"))
    } else {
        return Err(GraphResourcePathError::WrongDirectoryOrExtension);
    }
    .ok_or(GraphResourcePathError::WrongDirectoryOrExtension)?;

    validate_resource_name(stem).map_err(|_| GraphResourcePathError::InvalidName)
}
