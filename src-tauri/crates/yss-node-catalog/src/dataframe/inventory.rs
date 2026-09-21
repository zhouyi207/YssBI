//! Data preparation entries awaiting interface and kernel implementation.

use super::*;
use crate::catalog_entry::{self, Entry};

const ENTRIES: &[Entry] = &[
    Entry {
        id: "yssbi.dataframe.labels",
        method: "data.labels",
        source_ids: &[2],
        category: "data_processing",
        en: "Data Labels",
        zh: "数据标签",
        aliases: &["数据标签", "variable labels", "value labels"],
        product_form: "数据管理操作",
        scope_note: "区分变量标签、取值标签和图节点显示名；持久化位置需定义。",
    },
    Entry {
        id: "yssbi.dataframe.encode",
        method: "data.encode",
        source_ids: &[3],
        category: "data_processing",
        en: "Data Encoding",
        zh: "数据编码",
        aliases: &["数据编码", "categorical encoding", "label encoding"],
        product_form: "独立节点",
        scope_note: "区分类型转换、类别编码、标签编码和虚拟变量生成。",
    },
    Entry {
        id: "yssbi.dataframe.impute.single",
        method: "missing.single_imputation",
        source_ids: &[289],
        category: "data_processing",
        en: "Single Imputation",
        zh: "单次插补",
        aliases: &["单次插补", "single imputation"],
        product_form: "独立节点",
        scope_note: "",
    },
    Entry {
        id: "yssbi.dataframe.impute.multiple",
        method: "missing.multiple_imputation",
        source_ids: &[290],
        category: "data_processing",
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
        category: "data_processing",
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
