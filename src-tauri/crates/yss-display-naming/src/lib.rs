//! Allocation for case-sensitive database and variable display names.
//!
//! Display names intentionally have a looser compatibility contract than
//! filesystem-backed graph and worksheet resource names. Existing `base N` and
//! `base_N` spellings reserve the same numeric suffix, while newly allocated
//! names use the canonical `base N` spelling starting at one.

use std::collections::HashSet;

/// Allocates a display name that does not collide with `base_name` or one of
/// its existing numeric suffixes.
///
/// Matching is case-sensitive and literal. A suffixed name only participates
/// when its suffix is an ASCII `u32`; this preserves the persisted naming
/// behavior of the former regular-expression implementation.
pub fn allocate_unique_display_name(
    base_name: &str,
    existing: impl IntoIterator<Item = impl AsRef<str>>,
) -> String {
    let mut used_suffixes = HashSet::new();
    let mut base_is_used = false;

    for existing_name in existing {
        let existing_name = existing_name.as_ref();
        if existing_name == base_name {
            base_is_used = true;
        } else if let Some(suffix) = numeric_suffix(base_name, existing_name) {
            used_suffixes.insert(suffix);
        }
    }

    if !base_is_used {
        return base_name.to_owned();
    }

    // Use a wider counter so even a fully occupied persisted `u32` suffix
    // space cannot overflow the allocator.
    let mut suffix = 1u64;
    while u32::try_from(suffix).is_ok_and(|suffix| used_suffixes.contains(&suffix)) {
        suffix += 1;
    }
    format!("{base_name} {suffix}")
}

fn numeric_suffix(base_name: &str, existing_name: &str) -> Option<u32> {
    let remainder = existing_name.strip_prefix(base_name)?;
    let digits = remainder
        .strip_prefix(' ')
        .or_else(|| remainder.strip_prefix('_'))?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::allocate_unique_display_name;

    #[test]
    fn returns_the_base_when_the_exact_base_is_available() {
        assert_eq!(
            allocate_unique_display_name("New Event", ["New Function", "New Event 1"]),
            "New Event"
        );
    }

    #[test]
    fn allocates_the_first_gap_across_space_and_underscore_suffixes() {
        assert_eq!(
            allocate_unique_display_name("New Event", ["New Event", "New Event_1", "New Event 3"]),
            "New Event 2"
        );
        assert_eq!(
            allocate_unique_display_name("New Event", ["New Event"]),
            "New Event 1"
        );
    }

    #[test]
    fn matching_is_literal_and_case_sensitive() {
        assert_eq!(
            allocate_unique_display_name("Report[1]", ["report[1]", "Report[1]", "Report[1] 1"]),
            "Report[1] 2"
        );
    }

    #[test]
    fn ignores_non_ascii_signed_and_overflowing_suffixes() {
        assert_eq!(
            allocate_unique_display_name(
                "Value",
                [
                    "Value",
                    "Value +1",
                    "Value -1",
                    "Value １",
                    "Value 4294967296",
                ]
            ),
            "Value 1"
        );
        assert_eq!(
            allocate_unique_display_name("Value", ["Value", "Value 01"]),
            "Value 2"
        );
    }
}
