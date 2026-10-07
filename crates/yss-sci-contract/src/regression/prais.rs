//! Configuration of AR(1) error regression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PraisTransform {
    PraisWinsten,
    CochraneOrcutt,
}

pub struct PraisConfig {
    pub constant: bool,
    pub transform: PraisTransform,
    pub max_iter: usize,
    pub tol: f64,
}

impl Default for PraisConfig {
    fn default() -> Self {
        Self {
            constant: true,
            transform: PraisTransform::PraisWinsten,
            max_iter: 100,
            tol: 1e-6,
        }
    }
}
