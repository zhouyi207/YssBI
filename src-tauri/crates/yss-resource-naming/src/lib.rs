use std::collections::HashSet;
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

pub const MAX_RESOURCE_NAME_CHARACTERS: usize = 80;

static LETTER_OR_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[\p{L}\p{N}]$").expect("resource-name Unicode category regex is valid")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceNameValidationError {
    Empty,
    NotNfc,
    ForbiddenCharacter(char),
    InvalidSpacing,
    Reserved,
    TooLong,
}

impl fmt::Display for ResourceNameValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("resource name cannot be empty"),
            Self::NotNfc => formatter.write_str("resource name must be in Unicode NFC form"),
            Self::ForbiddenCharacter(character) => {
                write!(
                    formatter,
                    "resource name contains forbidden character '{character}'"
                )
            }
            Self::InvalidSpacing => formatter
                .write_str("resource name cannot have leading, trailing, or consecutive spaces"),
            Self::Reserved => formatter.write_str("resource name is reserved"),
            Self::TooLong => {
                formatter.write_str("resource name cannot exceed 80 Unicode characters")
            }
        }
    }
}

impl std::error::Error for ResourceNameValidationError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceName(String);

impl ResourceName {
    pub fn parse(input: &str) -> Result<Self, ResourceNameValidationError> {
        validate_resource_name(input)?;
        Ok(Self(input.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn portable_key(&self) -> String {
        self.0.case_fold().nfc().collect()
    }
}

pub fn validate_resource_name(input: &str) -> Result<(), ResourceNameValidationError> {
    if input.is_empty() {
        return Err(ResourceNameValidationError::Empty);
    }
    if input.nfc().ne(input.chars()) {
        return Err(ResourceNameValidationError::NotNfc);
    }
    if input.chars().count() > MAX_RESOURCE_NAME_CHARACTERS {
        return Err(ResourceNameValidationError::TooLong);
    }
    if is_reserved(input) {
        return Err(ResourceNameValidationError::Reserved);
    }
    if input.starts_with(' ') || input.ends_with(' ') || input.contains("  ") {
        return Err(ResourceNameValidationError::InvalidSpacing);
    }
    if let Some(character) = input.chars().find(|character| !is_allowed(*character)) {
        return Err(ResourceNameValidationError::ForbiddenCharacter(character));
    }
    Ok(())
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

    unreachable!("the numeric resource-name suffix space cannot be exhausted")
}

fn is_allowed(character: char) -> bool {
    if matches!(character, ' ' | '-' | '_' | '(' | ')') {
        return true;
    }

    let mut encoded = [0; 4];
    LETTER_OR_NUMBER.is_match(character.encode_utf8(&mut encoded))
}

fn is_reserved(input: &str) -> bool {
    if matches!(input, "." | "..") {
        return true;
    }

    let uppercase = input.to_ascii_uppercase();
    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || reserved_numbered_name(&uppercase, "COM")
        || reserved_numbered_name(&uppercase, "LPT")
}

fn reserved_numbered_name(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|number| matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RESOURCE_NAME_CHARACTERS, ResourceName, ResourceNameValidationError,
        allocate_unique_resource_name,
    };

    #[test]
    fn resource_name_enforces_the_portable_filesystem_contract() {
        for value in ["销售分析 2", "Report_2026", "Revenue (Net)", "Résumé"] {
            let name = ResourceName::parse(value).unwrap();
            assert_eq!(name.as_str(), value);
        }

        for (value, error) in [
            (
                "Sales/Report",
                ResourceNameValidationError::ForbiddenCharacter('/'),
            ),
            (" Sales", ResourceNameValidationError::InvalidSpacing),
            ("Re\u{301}sume\u{301}", ResourceNameValidationError::NotNfc),
            ("CON", ResourceNameValidationError::Reserved),
        ] {
            assert_eq!(ResourceName::parse(value), Err(error));
        }

        let overlong = "界".repeat(MAX_RESOURCE_NAME_CHARACTERS + 1);
        assert_eq!(
            ResourceName::parse(&overlong),
            Err(ResourceNameValidationError::TooLong)
        );
    }

    #[test]
    fn portable_key_uses_unicode_case_folding_and_nfc() {
        let mixed_case = ResourceName::parse("Straße").unwrap();
        let uppercase = ResourceName::parse("STRASSE").unwrap();

        assert_eq!(mixed_case.portable_key(), uppercase.portable_key());
        assert_eq!(mixed_case.portable_key(), "strasse");
    }

    #[test]
    fn allocation_uses_the_first_free_portable_numeric_suffix() {
        let base = ResourceName::parse("销售分析").unwrap();
        let existing = [
            ResourceName::parse("销售分析").unwrap(),
            ResourceName::parse("销售分析 3").unwrap(),
        ];
        assert_eq!(
            allocate_unique_resource_name(&base, &existing).as_str(),
            "销售分析 2"
        );

        let case_insensitive = [ResourceName::parse("REPORT").unwrap()];
        assert_eq!(
            allocate_unique_resource_name(
                &ResourceName::parse("report").unwrap(),
                &case_insensitive
            )
            .as_str(),
            "report 2"
        );

        let long_base = ResourceName::parse(&format!("{} BC", "A".repeat(77))).unwrap();
        assert_eq!(
            allocate_unique_resource_name(&long_base, [&long_base]).as_str(),
            format!("{} 2", "A".repeat(77))
        );
    }
}
