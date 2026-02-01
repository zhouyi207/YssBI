//! Schema Pin Types 测试

use yssbi_lib::schema::{check_type_compatibility, TypeConversion};

#[test]
fn test_same_type() {
    assert_eq!(check_type_compatibility("int", "int"), TypeConversion::Same);
}

#[test]
fn test_implicit_conversion() {
    assert_eq!(
        check_type_compatibility("int", "float64"),
        TypeConversion::Implicit
    );
    assert_eq!(
        check_type_compatibility("int", "string"),
        TypeConversion::Implicit
    );
}

#[test]
fn test_object_accepts_any() {
    assert_eq!(
        check_type_compatibility("int", "object"),
        TypeConversion::Implicit
    );
    assert_eq!(
        check_type_compatibility("string", "object"),
        TypeConversion::Implicit
    );
    assert_eq!(
        check_type_compatibility("exec", "object"),
        TypeConversion::Incompatible
    );
}