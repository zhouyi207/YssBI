//! Neutral kernel-density computation contracts.

pub struct KernelDensityInput<'a> {
    pub values: &'a [f64],
    pub grid_points: usize,
    pub min_x: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DensityPoint {
    pub x: f64,
    pub density: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KernelDensityOutput {
    pub points: Vec<DensityPoint>,
}
