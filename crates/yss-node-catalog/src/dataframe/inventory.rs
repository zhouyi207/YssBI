//! Data preparation entries awaiting interface and kernel implementation.

use super::*;
use crate::catalog_entry::{self, Entry};

pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        id: "yssbi.dataframe.impute.multiple",
        method: "missing.multiple_imputation",
        source_ids: &[290],
        category: "statistics.imputation",
        form: "node",
    },
    Entry {
        id: "yssbi.dataframe.impute.mice",
        method: "missing.mice",
        source_ids: &[291],
        category: "statistics.imputation",
        form: "node",
    },
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    catalog_entry::append(ENTRIES, fragment, "builtin.dataframe", "builtin.dataframe")
}
