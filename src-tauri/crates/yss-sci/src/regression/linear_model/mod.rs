pub mod gls;
pub mod iv2sls;
pub mod ivliml;
pub mod ols;
pub mod prais;
pub mod wls;

pub use gls::*;
pub use iv2sls::*;
pub use ivliml::*;
pub use ols::*;
pub use prais::*;
pub use wls::*;

/// Total variation after whitening, centered along the transformed intercept.
fn transformed_total_ss(
    y: &yss_sci_linalg::Col<f64>,
    intercept: Option<&yss_sci_linalg::Col<f64>>,
) -> f64 {
    match intercept {
        Some(c) => {
            let mean = y.iter().zip(c.iter()).map(|(y, c)| y * c).sum::<f64>()
                / c.iter().map(|c| c * c).sum::<f64>();
            y.iter()
                .zip(c.iter())
                .map(|(y, c)| (y - mean * c).powi(2))
                .sum()
        }
        None => y.iter().map(|y| y * y).sum(),
    }
}
