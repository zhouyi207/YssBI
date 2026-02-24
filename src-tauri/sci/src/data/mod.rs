use ndarray::{Array1, Array2};

pub struct StatisticDataMeta {
    pub description: String
}

pub struct StatisticData {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub meta: StatisticDataMeta
}
