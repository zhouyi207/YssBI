use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use yss_resource_naming::{ResourceName, ResourceNameValidationError};

use super::{WORKSHEET_EXTENSION, WORKSHEETS_DIR};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorksheetResourcePathError {
    #[error("worksheet resource path must be in the worksheets directory")]
    WrongDirectory,
    #[error("worksheet resource path must not be nested")]
    Nested,
    #[error("worksheet resource path must use the .yssbi-worksheet extension")]
    WrongExtension,
    #[error("worksheet resource path has an invalid name")]
    InvalidName(#[source] ResourceNameValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorksheetResourcePath {
    value: String,
    display_name: ResourceName,
}

impl WorksheetResourcePath {
    pub fn parse(value: &str) -> Result<Self, WorksheetResourcePathError> {
        let relative = value
            .strip_prefix(&format!("{WORKSHEETS_DIR}/"))
            .ok_or(WorksheetResourcePathError::WrongDirectory)?;
        if relative.contains('/') || relative.contains('\\') {
            return Err(WorksheetResourcePathError::Nested);
        }
        let stem = relative
            .strip_suffix(&format!(".{WORKSHEET_EXTENSION}"))
            .ok_or(WorksheetResourcePathError::WrongExtension)?;
        let display_name =
            ResourceName::parse(stem).map_err(WorksheetResourcePathError::InvalidName)?;

        Ok(Self {
            value: value.to_owned(),
            display_name,
        })
    }

    pub fn from_name(name: &ResourceName) -> Self {
        Self {
            value: format!("{WORKSHEETS_DIR}/{}.{WORKSHEET_EXTENSION}", name.as_str()),
            display_name: name.clone(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn display_name(&self) -> &ResourceName {
        &self.display_name
    }

    pub fn relative_path(&self) -> &Path {
        Path::new(&self.value)
    }
}

impl TryFrom<String> for WorksheetResourcePath {
    type Error = WorksheetResourcePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<WorksheetResourcePath> for String {
    fn from(value: WorksheetResourcePath) -> Self {
        value.value
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{WorksheetResourcePath, WorksheetResourcePathError};
    use yss_resource_naming::{ResourceName, ResourceNameValidationError};

    #[test]
    fn worksheet_path_round_trips_canonical_resource_identity() {
        let name = ResourceName::parse("Sales Report").unwrap();
        let path = WorksheetResourcePath::from_name(&name);

        assert_eq!(path.as_str(), "worksheets/Sales Report.yssbi-worksheet");
        assert_eq!(path.display_name(), &name);
        assert_eq!(
            path.relative_path(),
            Path::new("worksheets/Sales Report.yssbi-worksheet")
        );

        let serialized = serde_json::to_string(&path).unwrap();
        assert_eq!(serialized, r#""worksheets/Sales Report.yssbi-worksheet""#);
        assert_eq!(
            serde_json::from_str::<WorksheetResourcePath>(&serialized).unwrap(),
            path
        );
    }

    #[test]
    fn worksheet_path_rejects_nested_wrong_extension_and_invalid_stem() {
        assert_eq!(
            WorksheetResourcePath::parse("reports/Sales Report.yssbi-worksheet"),
            Err(WorksheetResourcePathError::WrongDirectory)
        );
        assert_eq!(
            WorksheetResourcePath::parse("worksheets/Regional/Sales Report.yssbi-worksheet"),
            Err(WorksheetResourcePathError::Nested)
        );
        assert_eq!(
            WorksheetResourcePath::parse("worksheets/Sales Report.json"),
            Err(WorksheetResourcePathError::WrongExtension)
        );
        assert_eq!(
            WorksheetResourcePath::parse("worksheets/Sales?.yssbi-worksheet"),
            Err(WorksheetResourcePathError::InvalidName(
                ResourceNameValidationError::ForbiddenCharacter('?')
            ))
        );
    }
}
