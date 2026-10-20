//! Shared catalog and locale bounds for Assistant capability inputs.

pub const MAX_CATALOG_QUERY_BYTES: usize = 256;

pub const MAX_LOCALE_BYTES: usize = 32;

/// Locale of the graph projections that internal Assistant edits and reads resolve against.
pub const AUTOMATION_LOCALE: &str = "en-US";

pub const MAX_CATALOG_RESULTS: u16 = 100;
