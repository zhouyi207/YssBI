use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    expression::RawExpression,
    model::{DatasetSourceType, Expression, InferenceConfig, LikelihoodSpec, ParameterSpec},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BayesModelDraft {
    pub formula_text: String,
    pub raw_response: RawExpression,
    pub bound_response: Option<Expression>,
    pub symbols: Vec<SymbolDraft>,
    pub dataset: Option<DatasetSelection>,
    pub response_binding: Option<ResponseBinding>,
    #[serde(default)]
    pub data_bindings: BTreeMap<String, String>,
    pub bound_predictor: Option<Expression>,
    pub likelihood: LikelihoodSpec,
    #[serde(default)]
    pub parameters: Vec<ParameterSpec>,
    pub sampler: InferenceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSelection {
    pub source_type: DatasetSourceType,
    pub source_id: String,
    #[serde(default)]
    pub columns: Vec<ColumnMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub dtype: ColumnDType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnDType {
    Number,
    Integer,
    Boolean,
    String,
    Date,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBinding {
    pub symbol: String,
    pub column: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDraft {
    pub name: String,
    pub role: SymbolRole,
    pub inferred_role: SymbolRole,
    pub user_edited: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolRole {
    Dependent,
    Independent,
    Parameter,
}
