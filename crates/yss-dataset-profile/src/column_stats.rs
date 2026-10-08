#[derive(Debug, Clone)]
pub struct NumericColumnStats {
    pub column_name: String,
    pub column_type: String,
    pub count: usize,
    pub null_count: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub std: Option<f64>,
    pub variance: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct StringColumnStats {
    pub column_name: String,
    pub column_type: String,
    pub count: usize,
    pub null_count: usize,
    pub empty_count: usize,
    pub valid_ratio: f64,
    pub unique: usize,
    pub mode: Option<String>,
    pub mode_count: usize,
}

#[derive(Debug, Clone)]
pub enum ColumnStats {
    Numeric(NumericColumnStats),
    String(StringColumnStats),
}
