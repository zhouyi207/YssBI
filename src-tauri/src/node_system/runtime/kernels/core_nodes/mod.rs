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
    fn kernel_fragment_matches_current_core_catalog_inventory() {
        fn belongs_to_core_kernel_inventory(handle: &str) -> bool {
            handle == "yssbi.value.convert"
                || handle.starts_with("yssbi.data_series.convert.")
                || handle.starts_with("yssbi.numeric.series.")
                || matches!(
                    handle,
                    "yssbi.numeric.ln"
                        | "yssbi.numeric.log2"
                        | "yssbi.numeric.log10"
                        | "yssbi.numeric.exp"
                        | "yssbi.numeric.sqrt"
                        | "yssbi.numeric.square"
                        | "yssbi.control.do"
                        | "yssbi.control.sleep"
                        | "yssbi.debug.print"
                )
        }

        let node_system = crate::graph::catalog::build_builtin_node_system().unwrap();
        let catalog_handles = node_system
            .registry
            .iter()
            .map(|(id, _)| id.as_str())
            .filter(|handle| belongs_to_core_kernel_inventory(handle))
            .collect::<BTreeSet<_>>();
        let fragment = build_kernel_fragment();
        let fragment_handles = fragment
            .handles()
            .map(|handle| handle.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(fragment_handles, catalog_handles);
    }
}
