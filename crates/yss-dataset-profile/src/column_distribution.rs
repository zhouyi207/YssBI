#[derive(Debug, Clone)]
pub struct HistogramBin {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct CategoryCount {
    pub label: String,
    pub value: usize,
}

#[derive(Debug, Clone)]
pub struct NumericDistribution {
    pub column_name: String,
    pub bins: Vec<HistogramBin>,
}

#[derive(Debug, Clone)]
pub struct StringDistribution {
    pub column_name: String,
    pub categories: Vec<CategoryCount>,
    pub other_count: usize,
}

#[derive(Debug, Clone)]
pub enum ColumnDistribution {
    Numeric(NumericDistribution),
    String(StringDistribution),
}
