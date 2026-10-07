use super::*;
use yss_data_contract::TabularScalar as V;
use yss_relational_contract::{GroupMapMode, GroupedRelationHandle, SortColumn};

fn source(engine: &Arc<DataFusionRuntime>) -> RelationHandle {
    engine
        .batch_relation(
            binding(),
            RecordBatch::try_new(
                Arc::new(Schema::new(vec![
                    Field::new("key", DataType::Utf8, true),
                    Field::new("x", DataType::Int64, false),
                ])),
                vec![
                    Arc::new(StringArray::from(vec![
                        Some("b"),
                        Some("a"),
                        None,
                        Some("a"),
                        Some("b"),
                        None,
                    ])),
                    Arc::new(Int64Array::from(vec![10, 20, 30, 40, 50, 60])),
                ],
            )
            .unwrap(),
        )
        .unwrap()
}
fn groups(source: RelationHandle) -> GroupedRelationHandle {
    GroupedRelationHandle::new(source, Arc::from(["key".into()])).unwrap()
}
fn apply() -> GroupMapMode {
    GroupMapMode::Apply {
        key_prefix: "group.".into(),
    }
}
fn assert_released(engine: &DataFusionRuntime) {
    assert_eq!(
        engine
            .environment
            .disk_manager
            .spilling_progress()
            .active_files_count,
        0
    );
    assert_eq!(engine.environment.disk_manager.used_disk_space(), 0);
}

#[test]
fn grouped_apply_keeps_null_keys_and_validates_each_returned_schema() {
    let engine = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = source(&engine);
    assert!(GroupedRelationHandle::new(source.clone(), Arc::from(["missing".into()])).is_err());
    assert!(
        GroupedRelationHandle::new(source.clone(), Arc::from(["key".into(), "key".into()]))
            .is_err()
    );
    let groups = groups(source);
    let mut mapping = groups.begin_map(apply(), &control()).unwrap();
    let mut seen = Vec::new();
    while let Some(group) = mapping.next(&control()).unwrap() {
        seen.push(group.ordinal);
        mapping.append(&group.relation, &control()).unwrap();
    }
    assert_eq!(seen, [1, 2, 3]);
    let result = mapping.finish(None, &control()).unwrap();
    let page = result.page(0, 10, &control()).unwrap();
    assert_eq!(page.data.columns()[0].name().as_str(), "group.key");
    assert_eq!(
        page.data.columns()[0].values(),
        &[
            V::Null,
            V::Null,
            V::String("a".into()),
            V::String("a".into()),
            V::String("b".into()),
            V::String("b".into())
        ]
    );
    assert_eq!(
        page.data.columns()[2].values(),
        &[30, 60, 20, 40, 10, 50].map(V::Unsigned)
    );
    drop(result);

    let mut mapping = groups.begin_map(apply(), &control()).unwrap();
    while let Some(group) = mapping.next(&control()).unwrap() {
        let returned = group
            .relation
            .limit(0, (group.ordinal - 1) as usize)
            .unwrap();
        mapping.append(&returned, &control()).unwrap();
    }
    let result = mapping.finish(None, &control()).unwrap();
    let page = result.page(0, 10, &control()).unwrap();
    assert_eq!(page.row_count, 3);
    assert_eq!(
        page.data.columns()[0].values(),
        &[
            V::String("a".into()),
            V::String("b".into()),
            V::String("b".into())
        ]
    );
    assert_eq!(
        page.data.columns()[2].values(),
        &[20, 10, 50].map(V::Unsigned)
    );
    drop(result);

    let mut mapping = groups.begin_map(apply(), &control()).unwrap();
    let group = mapping.next(&control()).unwrap().unwrap();
    mapping.append(&group.relation, &control()).unwrap();
    drop(group);
    let group = mapping.next(&control()).unwrap().unwrap();
    assert_eq!(
        mapping.append(&group.relation.rename("x", "other").unwrap(), &control()),
        Err(RelationError::GroupSchemaMismatch)
    );
    drop(group);
    drop(mapping);
    let mut mapping = groups
        .begin_map(
            GroupMapMode::Apply {
                key_prefix: "".into(),
            },
            &control(),
        )
        .unwrap();
    let group = mapping.next(&control()).unwrap().unwrap();
    assert_eq!(
        mapping.append(&group.relation, &control()),
        Err(RelationError::GroupKeyCollision)
    );
    drop(group);
    drop(mapping);
    assert_released(&engine);
}

#[test]
fn grouped_transform_proves_alignment_across_snapshots_and_restores_source_order() {
    let engine = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = source(&engine);
    let groups = groups(source.clone());
    let mut mapping = groups
        .begin_map(GroupMapMode::Transform, &control())
        .unwrap();
    while let Some(group) = mapping.next(&control()).unwrap() {
        let projected = group
            .relation
            .project(&["x".into()])
            .unwrap()
            .rename("x", "value")
            .unwrap();
        let frozen = engine.snapshot_relation(&projected, &control()).unwrap();
        assert!(frozen.same_rows_and_order(&group.relation));
        mapping.append(&frozen, &control()).unwrap();
    }
    let result = mapping.finish(None, &control()).unwrap();
    assert!(result.same_rows_and_order(&source));
    let page = result.page(0, 10, &control()).unwrap();
    assert_eq!(page.data.columns().len(), 1);
    assert_eq!(
        page.data.columns()[0].values(),
        &[10, 20, 30, 40, 50, 60].map(V::Unsigned)
    );
    drop(result);
    let mut mapping = groups
        .begin_map(GroupMapMode::Transform, &control())
        .unwrap();
    let group = mapping.next(&control()).unwrap().unwrap();
    let unrelated = source.limit(0, 2).unwrap();
    assert_eq!(
        mapping.append(&unrelated, &control()),
        Err(RelationError::UnalignedSeries)
    );
    let sorted = group
        .relation
        .sort_rows(&[SortColumn {
            column: "x".into(),
            ascending: false,
            nulls_first: false,
        }])
        .unwrap();
    assert_eq!(
        mapping.append(&sorted, &control()),
        Err(RelationError::UnalignedSeries)
    );
    drop(sorted);
    drop(group);
    drop(mapping);
    assert_released(&engine);
}

#[test]
fn grouped_empty_schema_probe_and_failed_runs_release_spill_storage() {
    let engine = DataFusionRuntime::new(64 * 1024 * 1024, 2).unwrap();
    let source = source(&engine);
    let empty = source.limit(0, 0).unwrap();
    for mode in [apply(), GroupMapMode::Transform] {
        let mut mapping = groups(empty.clone()).begin_map(mode, &control()).unwrap();
        assert!(mapping.next(&control()).unwrap().is_none());
        let result = mapping
            .finish(Some(&empty.project(&["x".into()]).unwrap()), &control())
            .unwrap();
        assert_eq!(result.page(0, 10, &control()).unwrap().row_count, 0);
        assert_eq!(result.schema().fields().last().unwrap().name(), "x");
    }
    assert_released(&engine);
    let control = control();
    let mut mapping = groups(source.clone()).begin_map(apply(), &control).unwrap();
    let group = mapping.next(&control).unwrap().unwrap();
    mapping.append(&group.relation, &control).unwrap();
    drop(group);
    control
        .cancellation
        .store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        mapping.next(&control).unwrap_err(),
        RelationError::Cancelled
    );
    drop(mapping);
    assert_released(&engine);
    let mut control = super::control();
    control.max_input_bytes = 1;
    let mut mapping = groups(source).begin_map(apply(), &control).unwrap();
    assert_eq!(
        mapping.next(&control).unwrap_err(),
        RelationError::MemoryLimitExceeded
    );
    drop(mapping);
    assert_released(&engine);
}
