use super::*;
use yss_data_contract::{DataValue, SemanticType, ValueType};

#[test]
fn transformation_graph_keeps_series_lazy_through_conditional_projection_and_row_operations() {
    let mut document = GraphDocument::default();
    let [
        source,
        select,
        missing,
        zero,
        choose,
        set,
        sort,
        deduplicate,
        limit,
    ] = std::array::from_fn(|_| NodeId::new());
    for (id, kind, parameters) in [
        (source, "yssbi.constant.get", serde_json::json!({})),
        (
            select,
            "yssbi.dataframe.series.select",
            serde_json::json!({"column":"x"}),
        ),
        (
            missing,
            "yssbi.dataframe.series.is_null",
            serde_json::json!({}),
        ),
        (zero, "yssbi.constant.get", serde_json::json!({})),
        (
            choose,
            "yssbi.dataframe.series.choose",
            serde_json::json!({}),
        ),
        (
            set,
            "yssbi.dataframe.set_column",
            serde_json::json!({"name":"x"}),
        ),
        (
            sort,
            "yssbi.dataframe.sort",
            serde_json::json!({"columns":["x"]}),
        ),
        (
            deduplicate,
            "yssbi.dataframe.deduplicate",
            serde_json::json!({"keys":["x"]}),
        ),
        (
            limit,
            "yssbi.dataframe.limit",
            serde_json::json!({"offset":1,"rows":2}),
        ),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: serde_json::from_value(parameters).unwrap(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::String(r#"{"x":[3,null,1,3],"group":["a","b","c","d"]}"#.into()),
    );
    set_constant(
        &mut document,
        zero,
        ValueType::Scalar(SemanticType::Numeric),
        DataValue::Integer(0),
    );
    for constant in document.constants.values_mut() {
        yss_graph_document::normalize_constant_value(constant).unwrap();
    }
    for ((from, output), (to, input)) in [
        ((source, "value"), (select, "dataframe")),
        ((select, "series"), (missing, "series")),
        ((missing, "result"), (choose, "condition")),
        ((zero, "value"), (choose, "when_true")),
        ((select, "series"), (choose, "when_false")),
        ((source, "value"), (set, "source")),
        ((choose, "result"), (set, "series")),
        ((set, "result"), (sort, "source")),
        ((sort, "result"), (deduplicate, "source")),
        ((deduplicate, "result"), (limit, "source")),
    ] {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(from, output.parse().unwrap()),
                input: PortAddress::declared(to, input.parse().unwrap()),
                order: None,
            },
        );
    }
    assert!(matches!(
        execute(&document, "yssbi.dataframe.series.choose").unwrap(),
        RuntimeValue::Series(_)
    ));
    let RuntimeValue::Relation(result) = execute(&document, "yssbi.dataframe.limit").unwrap()
    else {
        panic!("lazy dataframe expected");
    };
    let control = yss_relational_contract::RelationControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(30),
        max_input_bytes: 1024 * 1024,
    };
    let page = result.page(0, 10, &control).unwrap();
    assert_eq!(page.row_count, 2);
    assert_eq!(
        page.data.columns()[0].values(),
        &[TabularScalar::Unsigned(1), TabularScalar::Unsigned(3)]
    );
    assert_eq!(
        page.data.columns()[1].values(),
        &[
            TabularScalar::String("c".into()),
            TabularScalar::String("a".into())
        ]
    );
}
