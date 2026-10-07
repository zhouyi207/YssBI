//! Data preparation entries awaiting interface and kernel implementation.

use super::*;
use crate::catalog_entry::{self, Entry};

const ENTRIES: &[Entry] = &[
    Entry {
        id: "yssbi.dataframe.impute.multiple",
        method: "missing.multiple_imputation",
        source_ids: &[290],
        category: "statistics.imputation",
        en: "Multiple Imputation",
        zh: "多重插补MI",
        aliases: &["多重插补MI", "multiple imputation", "MI"],
        product_form: "独立节点",
        scope_note: "插补数据集、合并推断与随机种子必须一起定义。",
    },
    Entry {
        id: "yssbi.dataframe.impute.mice",
        method: "missing.mice",
        source_ids: &[291],
        category: "statistics.imputation",
        en: "MICE Imputation",
        zh: "MICE链式方程多重插补",
        aliases: &["MICE链式方程多重插补", "MICE", "chained equations"],
        product_form: "独立节点",
        scope_note: "MICE 是 MI 的具体算法，不能与通用 MI 入口直接视为同一实现。",
    },
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    catalog_entry::append(ENTRIES, fragment, "builtin.dataframe", "builtin.dataframe")
}

pub(crate) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    catalog_entry::documentation(ENTRIES, id, locale)
}
