use regex::Regex;
use std::fmt;
use std::sync::LazyLock;
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
