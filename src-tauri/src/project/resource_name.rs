use std::collections::HashSet;

use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;
use yss_graph_document::{
    MAX_RESOURCE_NAME_CHARACTERS, ResourceNameValidationError, validate_resource_name,
};

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
    use super::{ResourceName, allocate_unique_resource_name};
    use yss_graph_document::ResourceNameValidationError;

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
                Err(ResourceNameValidationError::ForbiddenCharacter(character))
            );
        }
    }

    #[test]
    fn resource_name_rejects_spacing_punctuation_and_emoji() {
        for value in [" Sales", "Sales ", "Sales  Report"] {
            assert_eq!(
                ResourceName::parse(value),
                Err(ResourceNameValidationError::InvalidSpacing)
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
                Err(ResourceNameValidationError::ForbiddenCharacter(character))
            );
        }
    }

    #[test]
    fn resource_name_rejects_non_nfc_reserved_and_overlong_names() {
        assert_eq!(
            ResourceName::parse("Re\u{301}sume\u{301}"),
            Err(ResourceNameValidationError::NotNfc)
        );

        for value in [
            ".", "..", "con", "PRN", "Aux", "nul", "COM1", "com9", "LPT1", "lpt9",
        ] {
            assert_eq!(
                ResourceName::parse(value),
                Err(ResourceNameValidationError::Reserved)
            );
        }

        let overlong = "界".repeat(81);
        assert_eq!(
            ResourceName::parse(&overlong),
            Err(ResourceNameValidationError::TooLong)
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
