use yss_tabular_contract::{
    FiniteTabularDecimal, TabularColumn, TabularColumnName, TabularContractError, TabularScalar,
    TabularSnapshot,
};

fn column(name: &str, values: &[TabularScalar]) -> TabularColumn {
    TabularColumn::new(
        TabularColumnName::try_from(name).expect("test column names are valid"),
        values.to_vec().into_boxed_slice(),
    )
}

#[test]
fn ordered_contract_preserves_wire_shapes_numeric_kinds_and_validates_shape() {
    let snapshot = TabularSnapshot::try_from_columns(
        [
            column("b", &[TabularScalar::Integer(3), TabularScalar::Integer(4)]),
            column("a", &[TabularScalar::Integer(1), TabularScalar::Integer(2)]),
        ]
        .into(),
    )
    .expect("ordered columns should be accepted");

    assert_eq!(
        snapshot
            .columns()
            .iter()
            .map(|column| column.name().as_str())
            .collect::<Vec<_>>(),
        ["b", "a"]
    );
    assert_eq!(snapshot.row_count(), 2);
    assert_eq!(
        serde_json::to_string(&snapshot).expect("snapshot wire should serialize"),
        r#"{"columns":{"b":[3,4],"a":[1,2]}}"#
    );
    assert_eq!(
        serde_json::to_string(&snapshot.columns_view()).expect("literal columns should serialize"),
        r#"{"b":[3,4],"a":[1,2]}"#
    );

    assert!(matches!(
        serde_json::from_str::<TabularScalar>("null"),
        Ok(TabularScalar::Null)
    ));
    assert!(matches!(
        serde_json::from_str::<TabularScalar>("true"),
        Ok(TabularScalar::Bool(true))
    ));
    assert!(matches!(
        serde_json::from_str::<TabularScalar>("-1"),
        Ok(TabularScalar::Integer(-1))
    ));
    assert!(matches!(
        serde_json::from_str::<TabularScalar>("2"),
        Ok(TabularScalar::Unsigned(2))
    ));
    assert!(matches!(
        serde_json::from_str::<TabularScalar>("1.5"),
        Ok(TabularScalar::Decimal(value)) if (value.as_f64() - 1.5).abs() < f64::EPSILON
    ));
    assert!(matches!(
        serde_json::from_str::<TabularScalar>(r#""text""#),
        Ok(TabularScalar::String(value)) if value.as_ref() == "text"
    ));
    assert_eq!(
        FiniteTabularDecimal::try_from(f64::INFINITY),
        Err(TabularContractError::NonFiniteDecimal)
    );
    assert!(serde_json::from_str::<TabularScalar>("1e999").is_err());

    assert!(matches!(
        TabularSnapshot::try_from_columns(
            [
                column("a", &[TabularScalar::Integer(1)]),
                column("a", &[TabularScalar::Integer(2)]),
            ]
            .into()
        ),
        Err(TabularContractError::DuplicateColumnName { .. })
    ));
    let duplicate = serde_json::from_str::<TabularSnapshot>(r#"{"columns":{"a":[1],"a":[2]}}"#)
        .expect_err("duplicate JSON keys must be rejected");
    assert!(duplicate.to_string().contains("duplicate column name"));
    assert_eq!(
        TabularSnapshot::try_from_columns(
            [column("a", &[TabularScalar::Integer(1)]), column("b", &[]),].into()
        ),
        Err(TabularContractError::UnequalColumnLengths)
    );
}
