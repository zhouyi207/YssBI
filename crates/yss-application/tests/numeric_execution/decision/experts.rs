use super::*;
#[test]
fn delphi_reports_all_experts_and_a_pageable_item_summary() {
    let mut document = GraphDocument::default();
    let kind = "yssbi.statistics.workflow.delphi";
    let target = node(&mut document, kind, json!({"full_score":5}));
    let inputs = columns(
        &mut document,
        &[
            ("item1", json!(vec![1; 640])),
            ("item2", json!(vec![3; 640])),
            ("item3", json!(vec![5; 640])),
        ],
    );
    for (j, key) in ["item1", "item2", "item3"].into_iter().enumerate() {
        connect(
            &mut document,
            inputs[key],
            "series",
            target,
            "criteria",
            Some(j),
        );
    }
    let report = execute(&document, kind).unwrap();
    assert_eq!(number(field(&report, "experts")), 640.);
    assert_eq!(
        number(field(field(&report, "concordance"), "coefficient")),
        1.
    );
    let limit = node(&mut document, "yssbi.dataframe.limit", json!({"rows":1000}));
    connect(&mut document, target, "items", limit, "source", None);
    let RuntimeValue::Relation(table) = execute(&document, "yssbi.dataframe.limit").unwrap() else {
        panic!("Delphi items")
    };
    let c = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(10),
        max_input_bytes: 4 * 1024 * 1024,
    };
    let page = table.page(2, 1, &c).unwrap();
    assert_eq!(page.data.columns().len(), 10);
    assert_eq!(
        page.data.columns()[9].values(),
        &[TabularScalar::Float64(100_f64.try_into().unwrap())]
    );
}
