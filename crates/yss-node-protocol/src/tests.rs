use super::{
    TypeClassId, TypeExpr, TypeId, TypeNormalizationError, TypeParameterId, data_series_type,
    normalize_type_expr, numeric_data_series_type,
};

#[test]
fn numeric_data_series_has_one_semantic_element_type() {
    assert_eq!(
        numeric_data_series_type(),
        data_series_type(TypeExpr::Concrete(TypeId::new("core.numeric").unwrap()))
    );
}

#[test]
fn type_union_is_flattened_deduplicated_and_deterministically_sorted() {
    let int = TypeExpr::Concrete(TypeId::new("core.text").unwrap());
    let float = TypeExpr::Concrete(TypeId::new("core.numeric").unwrap());
    let normalized = normalize_type_expr(TypeExpr::Union(vec![
        int.clone(),
        TypeExpr::Union(vec![float.clone(), int.clone()]),
    ]))
    .unwrap();
    assert_eq!(normalized, TypeExpr::Union(vec![float, int]));
}

#[test]
fn empty_type_union_is_rejected() {
    assert!(matches!(
        normalize_type_expr(TypeExpr::Union(Vec::new())),
        Err(TypeNormalizationError::EmptyUnion)
    ));
}

#[test]
fn type_normalization_preserves_unknown_generic_and_class_patterns() {
    let generic = TypeExpr::Generic(TypeParameterId::new("element").unwrap());
    let class = TypeExpr::Class(TypeClassId::new("core.numeric").unwrap());

    assert_eq!(
        normalize_type_expr(TypeExpr::Unknown).unwrap(),
        TypeExpr::Unknown
    );
    assert_eq!(
        normalize_type_expr(data_series_type(generic.clone())).unwrap(),
        data_series_type(generic)
    );
    assert_eq!(normalize_type_expr(class.clone()).unwrap(), class);
}
