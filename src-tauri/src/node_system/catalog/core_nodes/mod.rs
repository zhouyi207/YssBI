mod control;
mod debug;
mod math;
pub(super) mod reroute;
mod support;
mod value;

use super::builtin::{BuiltinAssemblyError, ProviderFragment};
use support::{category, empty_classes, i18n, semantic};

use crate::node_system::protocol::{TypeConstructorId, TypeId};
use crate::node_system::registry::{TypeConstructorRegistration, TypeRegistration};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoverageDisposition {
    ExistingCoreNode,
    MigratedHere,
    ExpandedFamily,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegacyCoverage {
    pub legacy_node_type: &'static str,
    pub stable_ids: &'static [&'static str],
    pub disposition: CoverageDisposition,
}

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut fragment = ProviderFragment::default();
    fragment.types.push(TypeRegistration {
        id: semantic("core.categorical", TypeId::new)?,
        title_key: i18n("types.categorical.title")?,
        classes: empty_classes(),
    });
    fragment
        .type_constructors
        .push(TypeConstructorRegistration {
            id: semantic("core.data_series", TypeConstructorId::new)?,
            title_key: i18n("types.data_series.title")?,
            arity: 1,
        });
    fragment.categories.extend([
        category("conversion", "categories.conversion.title", 25)?,
        category("debug", "categories.debug.title", 60)?,
    ]);
    for (key, en, zh) in [
        ("types.categorical.title", "Categorical", "分类"),
        ("types.data_series.title", "DataSeries", "数据序列"),
        ("categories.conversion.title", "Conversion", "转换"),
        ("categories.debug.title", "Debug", "调试"),
    ] {
        let key = i18n(key)?;
        fragment.text("en-US", key.clone(), en);
        fragment.text("zh-CN", key, zh);
    }

    value::register(&mut fragment)?;
    math::register(&mut fragment)?;
    control::register(&mut fragment)?;
    debug::register(&mut fragment)?;
    reroute::register(&mut fragment)?;
    Ok(fragment)
}

#[cfg(test)]
pub(crate) fn legacy_coverage() -> Vec<LegacyCoverage> {
    use CoverageDisposition::{
        ExistingCoreNode as Existing, ExpandedFamily as Family, MigratedHere as Migrated,
    };

    vec![
        coverage(
            "Value:Constants:Boolean",
            &["yssbi.constant.bool"],
            Existing,
        ),
        coverage("Value:Constants:Int64", &["yssbi.constant.int64"], Existing),
        coverage(
            "Value:Constants:Float64",
            &["yssbi.constant.float64"],
            Existing,
        ),
        coverage(
            "Value:Constants:String",
            &["yssbi.constant.string"],
            Existing,
        ),
        coverage(
            "Value:Conversion:Convert",
            &["yssbi.value.convert"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:String to Categorical",
            &["yssbi.data_series.convert.string_to_categorical"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:String to Float64",
            &["yssbi.data_series.convert.string_to_float64"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:String to Int64",
            &["yssbi.data_series.convert.string_to_int64"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Int64 to String",
            &["yssbi.data_series.convert.int64_to_string"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Float64 to String",
            &["yssbi.data_series.convert.float64_to_string"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Int64 to Float64",
            &["yssbi.data_series.convert.int64_to_float64"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Float64 to Int64",
            &["yssbi.data_series.convert.float64_to_int64"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Int64 to Boolean",
            &["yssbi.data_series.convert.int64_to_bool"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Float64 to Boolean",
            &["yssbi.data_series.convert.float64_to_bool"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Categorical to String",
            &["yssbi.data_series.convert.categorical_to_string"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Int64 to Categorical",
            &["yssbi.data_series.convert.int64_to_categorical"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Categorical to Int64",
            &["yssbi.data_series.convert.categorical_to_int64"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Float64 to Categorical",
            &["yssbi.data_series.convert.float64_to_categorical"],
            Migrated,
        ),
        coverage(
            "Data:Conversion:Categorical to Float64",
            &["yssbi.data_series.convert.categorical_to_float64"],
            Migrated,
        ),
        coverage(
            "Math:Operators:Add (+)",
            &[
                "yssbi.numeric.add.int64",
                "yssbi.numeric.add.float64",
                "yssbi.numeric.series.add",
            ],
            Family,
        ),
        coverage(
            "Math:Operators:Subtract (-)",
            &[
                "yssbi.numeric.subtract.int64",
                "yssbi.numeric.subtract.float64",
                "yssbi.numeric.series.subtract",
            ],
            Family,
        ),
        coverage(
            "Math:Operators:Multiply (*)",
            &[
                "yssbi.numeric.multiply.int64",
                "yssbi.numeric.multiply.float64",
                "yssbi.numeric.series.multiply",
            ],
            Family,
        ),
        coverage(
            "Math:Operators:Divide (/)",
            &[
                "yssbi.numeric.divide.int64",
                "yssbi.numeric.divide.float64",
                "yssbi.numeric.series.divide",
            ],
            Family,
        ),
        coverage("Math:Functions:Ln", &["yssbi.numeric.ln"], Migrated),
        coverage("Math:Functions:Log2", &["yssbi.numeric.log2"], Migrated),
        coverage("Math:Functions:Log10", &["yssbi.numeric.log10"], Migrated),
        coverage("Math:Functions:Exp", &["yssbi.numeric.exp"], Migrated),
        coverage("Math:Functions:Sqrt", &["yssbi.numeric.sqrt"], Migrated),
        coverage("Math:Functions:Square", &["yssbi.numeric.square"], Migrated),
        coverage(
            "Logic:Comparison:Equal (==)",
            &["yssbi.logic.equal"],
            Existing,
        ),
        coverage(
            "Logic:Comparison:Not Equal (!=)",
            &["yssbi.logic.not_equal"],
            Existing,
        ),
        coverage("Logic:Boolean:And (&&)", &["yssbi.logic.and"], Existing),
        coverage("Logic:Boolean:Or (||)", &["yssbi.logic.or"], Existing),
        coverage("Logic:Boolean:Not (!)", &["yssbi.logic.not"], Existing),
        coverage("Control Flow:Branch", &["yssbi.control.branch"], Existing),
        coverage(
            "Control Flow:Sequence",
            &["yssbi.control.sequence"],
            Existing,
        ),
        coverage("Control Flow:Do", &["yssbi.control.do"], Migrated),
        coverage("Control Flow:Merge", &["yssbi.control.merge"], Migrated),
        coverage("Control Flow:Sleep", &["yssbi.control.sleep"], Migrated),
        coverage("Control Flow:For Loop", &["yssbi.control.loop"], Family),
        coverage(
            "Control Flow:Switch",
            &["yssbi.control.branch", "yssbi.control.sequence"],
            Family,
        ),
        coverage("Control Flow:While Loop", &["yssbi.control.loop"], Existing),
        coverage("Debug:Print", &["yssbi.debug.print"], Migrated),
        coverage("Debug:Data:View", &["yssbi.debug.view"], Migrated),
    ]
}

#[cfg(test)]
const fn coverage(
    legacy_node_type: &'static str,
    stable_ids: &'static [&'static str],
    disposition: CoverageDisposition,
) -> LegacyCoverage {
    LegacyCoverage {
        legacy_node_type,
        stable_ids,
        disposition,
    }
}

#[cfg(test)]
mod coverage_tests;
