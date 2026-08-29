use super::{
    TypeExpr, TypeId, TypeNormalizationError, TypeParameterId, data_series_type,
    normalize_type_expr, numeric_data_series_type,
};
use crate::graph::catalog::build_builtin_node_system;
use crate::graph::registry::NodeRegistry;
use std::sync::Arc;

fn production_type_registry() -> Arc<NodeRegistry> {
    build_builtin_node_system()
        .expect("production built-in registry must assemble")
        .registry
}

#[test]
fn numeric_data_series_is_an_outer_union_of_homogeneous_series() {
    assert_eq!(
        numeric_data_series_type(),
        TypeExpr::Union(vec![
            data_series_type(TypeExpr::Concrete(TypeId::new("core.float64").unwrap())),
            data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
        ])
    );
}

#[test]
fn type_union_is_flattened_deduplicated_and_deterministically_sorted() {
    let int = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
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
fn type_normalization_preserves_unknown_and_generic_arguments() {
    let generic = TypeExpr::Generic(TypeParameterId::new("element").unwrap());

    assert_eq!(
        normalize_type_expr(TypeExpr::Unknown).unwrap(),
        TypeExpr::Unknown
    );
    assert_eq!(
        normalize_type_expr(data_series_type(generic.clone())).unwrap(),
        data_series_type(generic)
    );
}

#[test]
fn numeric_type_class_contains_only_int64_and_float64() {
    let registry = production_type_registry();
    let members = registry
        .types()
        .iter()
        .filter(|(_, registration)| {
            registration
                .classes
                .iter()
                .any(|class| class.as_str() == "core.numeric")
        })
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(members, ["core.float64", "core.int64"]);
}
