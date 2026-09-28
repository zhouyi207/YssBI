//! Configuration of binary-response estimation and prediction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinaryOptions {
    pub constant: bool,
    pub max_iterations: usize,
    pub tolerance: f64,
}

impl Default for BinaryOptions {
    fn default() -> Self {
        Self {
            constant: true,
            max_iterations: 100,
            tolerance: 1e-8,
        }
    }
}
