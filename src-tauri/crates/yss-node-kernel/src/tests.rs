use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_data_contract::DataType;

use crate::{
    KernelControl, KernelField, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};

#[test]
fn builtin_capability_manifest_preserves_cache_identity() {
    let registry = KernelRegistry::default();
    let fingerprint = registry
        .fingerprint()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        fingerprint,
        "b29ca05bbef3b02d54874c4f3c985157ebd0e3746bd1be3cf7b0add77fdbe620"
    );
}

#[test]
fn decomposition_follows_local_output_schema_order_without_graph_addresses() {
    let first = RuntimeValue::List(vec![RuntimeValue::Integer(11)].into_boxed_slice());
    let second = RuntimeValue::List(vec![RuntimeValue::Integer(22)].into_boxed_slice());
    let input = RuntimeValue::Record(BTreeMap::from([
        ("a".into(), first.clone()),
        ("b".into(), second.clone()),
    ]));
    let outputs = ["b", "a"].map(|name| KernelOutputSpec {
        data_type: DataType::DataSeries(Box::new(DataType::Int64)),
        fields: Some(
            vec![KernelField {
                name: name.into(),
                data_type: DataType::Int64,
            }]
            .into_boxed_slice(),
        ),
    });
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let result = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.dataframe.decompose".into()).unwrap(),
            &KernelInvocation {
                inputs: &[input],
                input_groups: &[None],
                parameters: BTreeMap::new(),
                outputs: &outputs,
                control: &control,
            },
        )
        .unwrap();
    assert_eq!(result, vec![second, first]);
}
