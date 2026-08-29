use super::{TypeConstructorId, TypeExpr, TypeId, normalize_type_expr};

pub const DATA_SERIES_CONSTRUCTOR_ID: &str = "core.data_series";
pub const NUMERIC_TYPE_CLASS_ID: &str = "core.numeric";

pub fn data_series_type(element: TypeExpr) -> TypeExpr {
    TypeExpr::Applied {
        constructor: TypeConstructorId::new(DATA_SERIES_CONSTRUCTOR_ID)
            .expect("canonical DataSeries constructor ID"),
        arguments: vec![element],
    }
}

pub fn numeric_data_series_type() -> TypeExpr {
    normalize_type_expr(TypeExpr::Union(vec![
        data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
        data_series_type(TypeExpr::Concrete(TypeId::new("core.float64").unwrap())),
    ]))
    .expect("canonical numeric DataSeries union")
}
