mod control;
mod debug;
mod math;
mod support;
mod value;

pub(crate) use support::KernelFragment;
pub(super) use value::canonical_float;
pub use value::{ConvertParameters, ConvertTarget};

pub(crate) fn build_kernel_fragment() -> KernelFragment {
    let mut fragment = KernelFragment::default();
    value::register(&mut fragment);
    math::register(&mut fragment);
    control::register(&mut fragment);
    debug::register(&mut fragment);
    fragment
}

#[cfg(test)]
mod tests {
    use super::build_kernel_fragment;
    use std::collections::BTreeSet;

    #[test]
    fn kernel_fragment_covers_every_migrated_leaf_kernel() {
        let expected = [
            "yssbi.value.convert",
            "yssbi.data_series.convert.string_to_categorical",
            "yssbi.data_series.convert.string_to_float64",
            "yssbi.data_series.convert.string_to_int64",
            "yssbi.data_series.convert.int64_to_string",
            "yssbi.data_series.convert.float64_to_string",
            "yssbi.data_series.convert.int64_to_float64",
            "yssbi.data_series.convert.float64_to_int64",
            "yssbi.data_series.convert.int64_to_bool",
            "yssbi.data_series.convert.float64_to_bool",
            "yssbi.data_series.convert.categorical_to_string",
            "yssbi.data_series.convert.int64_to_categorical",
            "yssbi.data_series.convert.categorical_to_int64",
            "yssbi.data_series.convert.float64_to_categorical",
            "yssbi.data_series.convert.categorical_to_float64",
            "yssbi.numeric.series.add",
            "yssbi.numeric.series.subtract",
            "yssbi.numeric.series.multiply",
            "yssbi.numeric.series.divide",
            "yssbi.numeric.ln",
            "yssbi.numeric.log2",
            "yssbi.numeric.log10",
            "yssbi.numeric.exp",
            "yssbi.numeric.sqrt",
            "yssbi.numeric.square",
            "yssbi.control.do",
            "yssbi.control.sleep",
            "yssbi.debug.print",
            "yssbi.debug.view",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let fragment = build_kernel_fragment();
        let actual = fragment
            .handles()
            .map(|handle| handle.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
    }
}
