use super::DataType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CategoricalRole {
    General,
    Individual,
    Time,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimeSeriesState {
    Aligned,
    Unaligned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DummyInfo {
    pub drop_category: Option<String>,
    pub role: CategoricalRole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataSeriesValue {
    pub id: String,
    pub element_type: Option<DataType>,
    pub dummy_info: Option<DummyInfo>,
    pub time_series_state: Option<TimeSeriesState>,
}

impl DataSeriesValue {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            element_type: None,
            dummy_info: None,
            time_series_state: None,
        }
    }

    pub fn with_element_type(id: impl Into<String>, element_type: DataType) -> Self {
        Self {
            id: id.into(),
            element_type: Some(element_type),
            dummy_info: None,
            time_series_state: None,
        }
    }
}

impl Serialize for DataSeriesValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.element_type.is_none()
            && self.dummy_info.is_none()
            && self.time_series_state.is_none()
        {
            serializer.serialize_str(&self.id)
        } else {
            use serde::ser::SerializeStruct;
            let field_count = 1
                + self.element_type.is_some() as usize
                + self.dummy_info.is_some() as usize
                + self.time_series_state.is_some() as usize;
            let mut state = serializer.serialize_struct("DataSeries", field_count)?;
            state.serialize_field("id", &self.id)?;
            if self.element_type.is_some() {
                state.serialize_field("elementType", &self.element_type)?;
            }
            if self.dummy_info.is_some() {
                state.serialize_field("dummyInfo", &self.dummy_info)?;
            }
            if self.time_series_state.is_some() {
                state.serialize_field("timeSeriesState", &self.time_series_state)?;
            }
            state.end()
        }
    }
}

impl<'de> Deserialize<'de> for DataSeriesValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Payload {
            IdOnly(String),
            Full {
                id: String,
                #[serde(rename = "elementType")]
                element_type: Option<DataType>,
                #[serde(rename = "dummyInfo")]
                dummy_info: Option<DummyInfo>,
                #[serde(rename = "timeSeriesState", default)]
                time_series_state: Option<TimeSeriesState>,
            },
        }

        match Payload::deserialize(deserializer)? {
            Payload::IdOnly(id) => Ok(DataSeriesValue::new(id)),
            Payload::Full {
                id,
                element_type,
                dummy_info,
                time_series_state,
            } => Ok(DataSeriesValue {
                id,
                element_type,
                dummy_info,
                time_series_state,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataValue {
    Boolean(bool),
    Int64(i64),
    Float64(f64),
    String(String),
    Array(Vec<DataValue>),
    Object(HashMap<String, DataValue>),
    DataFrame(String),
    DataSeries(DataSeriesValue),
    Struct {
        #[serde(rename = "typeKey")]
        type_key: String,
        #[serde(rename = "handleId")]
        handle_id: String,
    },
    Null,
}

impl Default for DataValue {
    fn default() -> Self {
        DataValue::Null
    }
}
