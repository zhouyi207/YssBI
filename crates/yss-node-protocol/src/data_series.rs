use super::{TypeConstructorId, TypeExpr, TypeId};

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
    data_series_type(TypeExpr::Concrete(
        TypeId::new(super::SemanticType::Numeric.type_id()).expect("Numeric semantic ID"),
    ))
}
