use polars::prelude::*;

pub struct DatabaseView {
    pub dataframe: DataFrame,
}

impl DatabaseView {
    pub fn new(dataframe: DataFrame) -> Self {
        Self { dataframe }
    }
}
