//! Inspect typed numbers before JSON can turn NaN/infinity into null.
use serde::{Serialize, Serializer, ser};

use crate::KernelError;

pub(super) fn validate(value: &impl Serialize) -> Result<(), KernelError> {
    value.serialize(FiniteNumbers).map_err(|error| match error {
        ValidationError::NonFinite => KernelError::NonFiniteResult,
        ValidationError::Serialization => KernelError::ScientificFailure,
    })
}

#[derive(Debug, thiserror::Error)]
enum ValidationError {
    #[error("nonfinite statistical result")]
    NonFinite,
    #[error("statistical result serialization failed")]
    Serialization,
}

impl ser::Error for ValidationError {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self::Serialization
    }
}

#[derive(Clone, Copy)]
struct FiniteNumbers;

macro_rules! scalar {
    ($($method:ident: $type:ty),* $(,)?) => {
        $(fn $method(self, _: $type) -> Result<(), ValidationError> { Ok(()) })*
    };
}

impl Serializer for FiniteNumbers {
    type Ok = ();
    type Error = ValidationError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    scalar! {
        serialize_bool: bool, serialize_i8: i8, serialize_i16: i16,
        serialize_i32: i32, serialize_i64: i64, serialize_i128: i128,
        serialize_u8: u8, serialize_u16: u16, serialize_u32: u32,
        serialize_u64: u64, serialize_u128: u128, serialize_char: char,
        serialize_str: &str, serialize_bytes: &[u8],
    }

    fn serialize_f32(self, value: f32) -> Result<(), ValidationError> {
        self.serialize_f64(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Result<(), ValidationError> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(ValidationError::NonFinite)
        }
    }

    fn serialize_none(self) -> Result<(), ValidationError> {
        Ok(())
    }
    fn serialize_unit(self) -> Result<(), ValidationError> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), ValidationError> {
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), ValidationError> {
        value.serialize(self)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), ValidationError> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Result<(), ValidationError> {
        value.serialize(self)
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self, ValidationError> {
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, ValidationError> {
        Ok(self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, ValidationError> {
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, ValidationError> {
        Ok(self)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self, ValidationError> {
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, ValidationError> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, ValidationError> {
        Ok(self)
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl ser::$trait for FiniteNumbers {
            type Ok = ();
            type Error = ValidationError;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), ValidationError> {
                value.serialize(*self)
            }
            fn end(self) -> Result<(), ValidationError> {
                Ok(())
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);

impl ser::SerializeMap for FiniteNumbers {
    type Ok = ();
    type Error = ValidationError;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), ValidationError> {
        key.serialize(*self)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), ValidationError> {
        value.serialize(*self)
    }
    fn end(self) -> Result<(), ValidationError> {
        Ok(())
    }
}

macro_rules! record {
    ($trait:ident) => {
        impl ser::$trait for FiniteNumbers {
            type Ok = ();
            type Error = ValidationError;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                _: &'static str,
                value: &T,
            ) -> Result<(), ValidationError> {
                value.serialize(*self)
            }
            fn end(self) -> Result<(), ValidationError> {
                Ok(())
            }
        }
    };
}
record!(SerializeStruct);
record!(SerializeStructVariant);
