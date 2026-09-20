use super::TypeExpr;
use serde::{Deserialize, Serialize};
use yss_data_contract::DataValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedValue {
    pub value_type: TypeExpr,
    pub value: DataValue,
}
