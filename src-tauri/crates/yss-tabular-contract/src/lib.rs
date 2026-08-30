//! Pure, backend-neutral tabular values and ordered column snapshots.

use serde::de::{self, DeserializeSeed, Deserializer, Error as _, MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiniteTabularDecimal(f64);

impl TryFrom<f64> for FiniteTabularDecimal {
    type Error = TabularContractError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        value
            .is_finite()
            .then_some(Self(value))
            .ok_or(TabularContractError::NonFiniteDecimal)
    }
}

impl FiniteTabularDecimal {
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TabularScalar {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Decimal(FiniteTabularDecimal),
    String(Box<str>),
}

impl Serialize for TabularScalar {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Integer(value) => serializer.serialize_i64(*value),
            Self::Unsigned(value) => serializer.serialize_u64(*value),
            Self::Decimal(value) => serializer.serialize_f64(value.as_f64()),
            Self::String(value) => serializer.serialize_str(value),
        }
    }
}

impl<'de> Deserialize<'de> for TabularScalar {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ScalarVisitor;

        impl<'de> Visitor<'de> for ScalarVisitor {
            type Value = TabularScalar;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("null, bool, JSON number, or string")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::Integer(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::Unsigned(value))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                FiniteTabularDecimal::try_from(value)
                    .map(TabularScalar::Decimal)
                    .map_err(E::custom)
            }

            fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_f64(value as f64)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::String(value.into()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TabularScalar::String(value.into_boxed_str()))
            }
        }

        deserializer.deserialize_any(ScalarVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabularColumnName(Box<str>);

impl TryFrom<&str> for TabularColumnName {
    type Error = TabularContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.trim().is_empty() {
            return Err(TabularContractError::InvalidColumnName);
        }
        Ok(Self(value.into()))
    }
}

impl TabularColumnName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabularColumn {
    name: TabularColumnName,
    values: Box<[TabularScalar]>,
}

impl TabularColumn {
    pub fn new(name: TabularColumnName, values: Box<[TabularScalar]>) -> Self {
        Self { name, values }
    }

    pub fn name(&self) -> &TabularColumnName {
        &self.name
    }

    pub fn values(&self) -> &[TabularScalar] {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TabularSnapshot {
    columns: Box<[TabularColumn]>,
}

impl TabularSnapshot {
    pub fn try_from_columns(columns: Box<[TabularColumn]>) -> Result<Self, TabularContractError> {
        let mut seen = BTreeSet::new();
        let mut row_count = None;
        for column in &columns {
            if !seen.insert(column.name().as_str()) {
                return Err(TabularContractError::DuplicateColumnName {
                    column: column.name().clone(),
                });
            }
            match row_count {
                Some(expected) if expected != column.values().len() => {
                    return Err(TabularContractError::UnequalColumnLengths);
                }
                None => row_count = Some(column.values().len()),
                _ => {}
            }
        }
        Ok(Self { columns })
    }

    pub fn columns(&self) -> &[TabularColumn] {
        &self.columns
    }

    pub fn columns_view(&self) -> TabularColumnsView<'_> {
        TabularColumnsView {
            columns: &self.columns,
        }
    }

    pub fn row_count(&self) -> usize {
        self.columns
            .first()
            .map_or(0, |column| column.values().len())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TabularColumnsView<'a> {
    columns: &'a [TabularColumn],
}

impl Serialize for TabularColumnsView<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.columns.len()))?;
        for column in self.columns {
            map.serialize_entry(column.name().as_str(), column.values())?;
        }
        map.end()
    }
}

impl Serialize for TabularSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("TabularSnapshot", 1)?;
        state.serialize_field("columns", &self.columns_view())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TabularSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColumnsVisitor;

        impl<'de> Visitor<'de> for ColumnsVisitor {
            type Value = Box<[TabularColumn]>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an ordered tabular column map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut columns = Vec::new();
                while let Some(name) = map.next_key::<String>()? {
                    let name =
                        TabularColumnName::try_from(name.as_str()).map_err(A::Error::custom)?;
                    let values = map.next_value::<Vec<TabularScalar>>()?;
                    columns.push(TabularColumn::new(name, values.into_boxed_slice()));
                }
                Ok(columns.into_boxed_slice())
            }
        }

        struct ColumnsSeed;

        impl<'de> DeserializeSeed<'de> for ColumnsSeed {
            type Value = Box<[TabularColumn]>;

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_map(ColumnsVisitor)
            }
        }

        struct SnapshotVisitor;

        impl<'de> Visitor<'de> for SnapshotVisitor {
            type Value = TabularSnapshot;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a tabular snapshot object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut columns = None;
                while let Some(field) = map.next_key::<String>()? {
                    if field != "columns" {
                        return Err(A::Error::unknown_field(&field, &["columns"]));
                    }
                    if columns.is_some() {
                        return Err(A::Error::duplicate_field("columns"));
                    }
                    columns = Some(map.next_value_seed(ColumnsSeed)?);
                }
                let columns = columns.ok_or_else(|| A::Error::missing_field("columns"))?;
                TabularSnapshot::try_from_columns(columns).map_err(A::Error::custom)
            }
        }

        deserializer.deserialize_map(SnapshotVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TabularContractError {
    #[error("invalid column name")]
    InvalidColumnName,
    #[error("non-finite decimal")]
    NonFiniteDecimal,
    #[error("duplicate column name")]
    DuplicateColumnName { column: TabularColumnName },
    #[error("unequal column lengths")]
    UnequalColumnLengths,
    #[error("invalid series column count")]
    SeriesColumnCount { actual: usize },
}
