//! Autocorrelation and partial-autocorrelation requests and results.
#[derive(Debug, Clone, PartialEq)]
pub struct AcfPacfRequest {
    pub values: Vec<f64>,
    pub max_lag: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcfPacfResult {
    pub acf: Vec<f64>,
    pub pacf: Vec<f64>,
    pub n: usize,
    /// Half-width of the two-sided 95% white-noise reference band.
    pub ci_half_width: f64,
}
