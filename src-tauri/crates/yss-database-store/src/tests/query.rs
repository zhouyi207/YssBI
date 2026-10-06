use super::*;
use arrow::array::{Int64Array, StringArray};
use yss_database_engine::DatasetRowsQuery;
use yss_relational_contract::{RelationComparison, RelationError, RelationPredicate, SortColumn};

#[test]
fn selected_pages_filter_before_projection_and_keep_stable_ids_and_semantic_order() {
    let directory = Directory::new();
    let store = DatasetStore::create(directory.path()).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("score", DataType::Int64, true),
        Field::new("label", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![Some(2), Some(1), Some(2), None])),
            Arc::new(StringArray::from(vec!["high", "low", "low", "high"])),
        ],
    )
    .unwrap();
    let original = store
        .commit(
            store
                .prepare_import(identity(), "Queries", "import", schema, [Ok(batch)])
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let engine = yss_database_engine::DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let (insert, ids) = store
        .prepare_insert_rows(
            &original,
            &engine,
            "insert",
            DatasetRowInsertion {
                position: DatasetInsertPosition::BeforeRow(0),
                rows: &[[
                    ("score".into(), serde_json::json!(2)),
                    ("label".into(), serde_json::json!("high")),
                ]
                .into()],
            },
            &control(),
        )
        .unwrap();
    assert_eq!(ids, [4]);
    let current = store.commit(insert).unwrap().snapshot;
    let semantic = yss_data_contract::ColumnSemantic {
        kind: yss_data_contract::SemanticType::Ordinal,
        values: ["low", "high"]
            .into_iter()
            .map(|value| yss_data_contract::SemanticValue {
                value: value.into(),
                label: value.into(),
            })
            .collect(),
        positive_value: None,
        numeric: None,
    };
    let current = store
        .commit(
            store
                .prepare_column_semantic(
                    &current,
                    &engine,
                    "ordinal",
                    "label",
                    &semantic,
                    &control(),
                )
                .unwrap(),
        )
        .unwrap()
        .snapshot;
    let query = current.query(&engine, "selected").unwrap();
    let rows = yss_database_arrow::dataset_row_columns(&current.metadata().schema)
        .unwrap()
        .unwrap();
    let ids = |page: &yss_database_engine::DatasetQueryPage| {
        page.batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column_by_name(&rows.row_id)
                    .unwrap()
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect::<Vec<_>>()
    };
    let mut request = DatasetRowsQuery {
        columns: vec!["label".into()],
        filters: vec![RelationPredicate {
            column: "score".into(),
            comparison: RelationComparison::GreaterEqual,
            value: Some(FilterLiteral::Integer(2)),
        }],
        order: vec![SortColumn {
            column: "score".into(),
            ascending: false,
            nulls_first: false,
        }],
        offset: 0,
        limit: 2,
    };
    let first = query.page_query(&request, &control()).unwrap();
    assert_eq!(ids(&first), [4, 0]);
    assert!(first.has_more);
    assert_eq!(first.schema.fields().len(), 3); // Two private identity fields plus one selected user field.
    assert!(first.schema.field_with_name("score").is_err());
    request.offset = 2;
    let second = query.page_query(&request, &control()).unwrap();
    assert_eq!(ids(&second), [2]);
    assert!(!second.has_more);
    request.offset = 20;
    let empty = query.page_query(&request, &control()).unwrap();
    assert_eq!(empty.row_count, 0);
    assert!(!empty.has_more);
    assert_eq!(empty.schema, first.schema);
    request.offset = 0;
    request.limit = 10;
    request.filters.clear();
    request.order.insert(
        0,
        SortColumn {
            column: "label".into(),
            ascending: true,
            nulls_first: false,
        },
    );
    assert_eq!(
        ids(&query.page_query(&request, &control()).unwrap()),
        [2, 1, 4, 0, 3]
    );
    request.filters = vec![RelationPredicate {
        column: "label".into(),
        comparison: RelationComparison::Less,
        value: Some(FilterLiteral::String("high".into())),
    }];
    assert_eq!(
        ids(&query.page_query(&request, &control()).unwrap()),
        [2, 1]
    );
    request.filters[0] = RelationPredicate {
        column: "score".into(),
        comparison: RelationComparison::IsNull,
        value: None,
    };
    assert_eq!(ids(&query.page_query(&request, &control()).unwrap()), [3]);
    request.filters[0].value = Some(FilterLiteral::Integer(2));
    assert!(matches!(
        query.page_query(&request, &control()),
        Err(RelationError::InvalidInput)
    ));
    request.filters.clear();
    request.columns = vec!["label".into(), "label".into()];
    assert!(query.page_query(&request, &control()).is_err());
    request.columns = vec![rows.row_id.clone()];
    assert!(query.page_query(&request, &control()).is_err());
    request.columns = vec!["missing".into()];
    request.offset = 99;
    assert!(query.page_query(&request, &control()).is_err());
    request.columns = vec!["label".into()];
    request.order.push(request.order[0].clone());
    assert!(query.page_query(&request, &control()).is_err());
    request.order.clear();
    let cancelled = control();
    cancelled
        .cancellation
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(matches!(
        query.page_query(&request, &cancelled),
        Err(RelationError::Cancelled)
    ));
    let selected = query.project_columns(&["score".into()]).unwrap();
    let overview = selected.dataset_overview(&control()).unwrap();
    assert_eq!(
        (
            overview.size_shape.n_rows,
            overview.size_shape.n_columns,
            overview.data_completeness.total_nulls
        ),
        (5, 1, 1)
    );
    assert_eq!(selected.column_stats(&control()).unwrap().len(), 1);
    assert_eq!(selected.column_distributions(&control()).unwrap().len(), 1);
    // A subsequent write does not alter the captured query or its identities.
    store
        .commit(
            store
                .prepare_cell_edits(
                    &current,
                    &engine,
                    "edit-selected",
                    vec![DatasetCellEdit {
                        row_id: 2,
                        column: "score",
                        value: serde_json::json!(9),
                    }],
                    &control(),
                )
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        query
            .page_query(
                &DatasetRowsQuery {
                    columns: vec!["label".into()],
                    filters: vec![RelationPredicate {
                        column: "score".into(),
                        comparison: RelationComparison::Equal,
                        value: Some(FilterLiteral::Integer(2))
                    }],
                    order: vec![],
                    offset: 0,
                    limit: 10
                },
                &control()
            )
            .map(|page| ids(&page))
            .unwrap(),
        [4, 0, 2]
    );
}
