use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use yss_data_contract::ValueType;

use crate::{
    KernelControl, KernelField, KernelId, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};

#[test]
fn decomposition_follows_local_output_schema_order_without_graph_addresses() {
    let first = RuntimeValue::List(vec![RuntimeValue::Integer(11)].into_boxed_slice());
    let second = RuntimeValue::List(vec![RuntimeValue::Integer(22)].into_boxed_slice());
    let input = RuntimeValue::Record(BTreeMap::from([
        ("a".into(), first.clone()),
        ("b".into(), second.clone()),
    ]));
    let outputs = ["b", "a"].map(|name| KernelOutputSpec {
        data_type: ValueType::DataSeries(Box::new(ValueType::Scalar(
            yss_data_contract::SemanticType::Numeric,
        ))),
        fields: Some(
            vec![KernelField {
                name: name.into(),
                data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
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
