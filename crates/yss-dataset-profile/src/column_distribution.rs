use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramBin {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub label: String,
    pub value: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericDistribution {
    pub column_name: String,
    pub kind: &'static str,
    pub bins: Vec<HistogramBin>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringDistribution {
    pub column_name: String,
    pub kind: &'static str,
    pub categories: Vec<CategoryCount>,
    pub other_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ColumnDistribution {
    Numeric(NumericDistribution),
    String(StringDistribution),
}
