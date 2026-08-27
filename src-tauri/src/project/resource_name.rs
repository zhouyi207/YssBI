use std::collections::HashSet;

use thiserror::Error;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

const MAX_RESOURCE_NAME_CHARACTERS: usize = 80;
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResourceNameError {
    #[error("resource name cannot be empty")]
    Empty,
    #[error("resource name must be in Unicode NFC form")]
    NotNfc,
    #[error("resource name contains forbidden character '{0}'")]
    ForbiddenCharacter(char),
    #[error("resource name cannot have leading, trailing, or consecutive spaces")]
    InvalidSpacing,
    #[error("resource name is reserved")]
    Reserved,
    #[error("resource name cannot exceed 80 Unicode characters")]
    TooLong,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceName(String);

impl ResourceName {
    pub fn parse(input: &str) -> Result<Self, ResourceNameError> {
        crate::graph_document::validate_resource_name(input).map_err(ResourceNameError::from)?;
        Ok(Self(input.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn portable_key(&self) -> String {
        self.0.case_fold().nfc().collect()
    }
}

impl From<crate::graph_document::ResourceNameValidationError> for ResourceNameError {
    fn from(source: crate::graph_document::ResourceNameValidationError) -> Self {
        match source {
            crate::graph_document::ResourceNameValidationError::Empty => Self::Empty,
            crate::graph_document::ResourceNameValidationError::NotNfc => Self::NotNfc,
            crate::graph_document::ResourceNameValidationError::ForbiddenCharacter(character) => {
                Self::ForbiddenCharacter(character)
            }
            crate::graph_document::ResourceNameValidationError::InvalidSpacing => {
                Self::InvalidSpacing
            }
            crate::graph_document::ResourceNameValidationError::Reserved => Self::Reserved,
            crate::graph_document::ResourceNameValidationError::TooLong => Self::TooLong,
        }
    }
}

pub fn allocate_unique_resource_name<'a>(
    base: &ResourceName,
    existing: impl IntoIterator<Item = &'a ResourceName>,
) -> ResourceName {
    let existing_keys = existing
        .into_iter()
        .map(ResourceName::portable_key)
        .collect::<HashSet<_>>();
    if !existing_keys.contains(&base.portable_key()) {
        return base.clone();
    }

    for number in 2_u64.. {
        let suffix = format!(" {number}");
        let retained = MAX_RESOURCE_NAME_CHARACTERS - suffix.chars().count();
        let mut prefix = base.as_str().chars().take(retained).collect::<String>();
        while prefix.ends_with(' ') {
            prefix.pop();
        }
        let candidate = format!("{prefix}{suffix}");
        let candidate = ResourceName::parse(&candidate)
            .expect("a validated resource name with a numeric suffix remains valid");
        if !existing_keys.contains(&candidate.portable_key()) {
            return candidate;
        }
    }

    unreachable!("the numeric resource-name suffix space is finite but cannot be exhausted")
}

#[cfg(test)]
mod tests {
    use super::{ResourceName, ResourceNameError, allocate_unique_resource_name};

    #[test]
    fn resource_name_accepts_portable_unicode_names() {
        for value in ["销售分析 2", "Report_2026", "Revenue (Net)", "Résumé"] {
            let name = ResourceName::parse(value).unwrap();
            assert_eq!(name.as_str(), value);
        }
    }

    #[test]
    fn resource_name_rejects_forbidden_characters_and_controls() {
        for (value, character) in [
            ("Sales/Report", '/'),
            (r"Sales\Report", '\\'),
            ("Sales:Report", ':'),
            ("Sales*Report", '*'),
            ("Sales?Report", '?'),
            ("Sales\"Report", '"'),
            ("Sales<Report", '<'),
            ("Sales>Report", '>'),
            ("Sales|Report", '|'),
            ("Sales\tReport", '\t'),
            ("Sales\nReport", '\n'),
            ("Sales\0Report", '\0'),
        ] {
            assert_eq!(
                ResourceName::parse(value),
                Err(ResourceNameError::ForbiddenCharacter(character))
            );
        }
    }

    #[test]
    fn resource_name_rejects_spacing_punctuation_and_emoji() {
        for value in [" Sales", "Sales ", "Sales  Report"] {
            assert_eq!(
                ResourceName::parse(value),
                Err(ResourceNameError::InvalidSpacing)
            );
        }

        for (value, character) in [
            ("Sales.Report", '.'),
            ("Sales,Report", ','),
            ("Sales📊", '📊'),
            ("Sales\u{0345}", '\u{0345}'),
        ] {
            assert_eq!(
                ResourceName::parse(value),
                Err(ResourceNameError::ForbiddenCharacter(character))
            );
        }
    }

    #[test]
    fn resource_name_rejects_non_nfc_reserved_and_overlong_names() {
        assert_eq!(
            ResourceName::parse("Re\u{301}sume\u{301}"),
            Err(ResourceNameError::NotNfc)
        );

        for value in [
            ".", "..", "con", "PRN", "Aux", "nul", "COM1", "com9", "LPT1", "lpt9",
        ] {
            assert_eq!(ResourceName::parse(value), Err(ResourceNameError::Reserved));
        }

        let overlong = "界".repeat(81);
        assert_eq!(
            ResourceName::parse(&overlong),
            Err(ResourceNameError::TooLong)
        );
    }

    #[test]
    fn resource_name_portable_key_is_case_insensitive() {
        let mixed_case = ResourceName::parse("Straße").unwrap();
        let uppercase = ResourceName::parse("STRASSE").unwrap();

        assert_eq!(mixed_case.portable_key(), uppercase.portable_key());
        assert_eq!(mixed_case.portable_key(), "strasse");
    }

    #[test]
    fn unique_resource_name_uses_first_free_numeric_suffix() {
        let base = ResourceName::parse("销售分析").unwrap();
        let existing = [
            ResourceName::parse("销售分析").unwrap(),
            ResourceName::parse("销售分析 3").unwrap(),
        ];

        let allocated = allocate_unique_resource_name(&base, &existing);

        assert_eq!(allocated.as_str(), "销售分析 2");

        let long_base = ResourceName::parse(&format!("{} BC", "A".repeat(77))).unwrap();
        let long_existing = [long_base.clone()];
        let long_allocated = allocate_unique_resource_name(&long_base, &long_existing);

        assert_eq!(long_allocated.as_str(), format!("{} 2", "A".repeat(77)));
    }
}
