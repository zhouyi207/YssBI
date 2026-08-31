//! Context-free Gaussian kernel density estimation API.

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

pub fn compute_kernel_density(input: KernelDensityInput<'_>) -> KernelDensityOutput {
    let values = input
        .values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() || input.grid_points < 2 {
        return KernelDensityOutput { points: Vec::new() };
    }

    let bandwidth = silverman_bandwidth(&values);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = ((max - min) * 0.15).max(bandwidth * 2.0).max(0.1);
    let padded_min = min - padding;
    let grid_min = input
        .min_x
        .filter(|value| value.is_finite())
        .map_or(padded_min, |value| value.max(padded_min));
    let grid_max = max + padding;
    let denominator = (input.grid_points - 1) as f64;
    let points = (0..input.grid_points)
        .map(|index| {
            let x = grid_min + index as f64 / denominator * (grid_max - grid_min);
            DensityPoint {
                x,
                density: kde_at(x, &values, bandwidth),
            }
        })
        .collect();
    KernelDensityOutput { points }
}

#[inline]
fn gaussian_kernel(u: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;
    INV_SQRT_2PI * (-0.5 * u * u).exp()
}

fn silverman_bandwidth(values: &[f64]) -> f64 {
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    let n = finite.len() as f64;
    if n < 2.0 {
        return 1.0;
    }

    let mean = finite.iter().sum::<f64>() / n;
    let variance = finite
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (n - 1.0);
    let sigma = variance.sqrt();
    if sigma <= 0.0 || !sigma.is_finite() {
        return 1.0;
    }

    1.06 * sigma * n.powf(-0.2)
}

fn kde_at(x: f64, values: &[f64], bandwidth: f64) -> f64 {
    let kernel_sum = values
        .iter()
        .map(|value| gaussian_kernel((x - value) / bandwidth))
        .sum::<f64>();
    kernel_sum / (values.len() as f64 * bandwidth)
}

#[cfg(test)]
mod tests {
    use super::{KernelDensityInput, compute_kernel_density, gaussian_kernel, silverman_bandwidth};

    #[test]
    fn gaussian_kernel_is_symmetric_and_normalized_at_zero() {
        assert!((gaussian_kernel(0.0) - 0.398_942_280_401_432_7).abs() < 1e-15);
        assert!((gaussian_kernel(-1.5) - gaussian_kernel(1.5)).abs() < 1e-15);
    }

    #[test]
    fn silverman_bandwidth_ignores_non_finite_values() {
        let finite = silverman_bandwidth(&[1.0, 2.0, 3.0]);
        let mixed = silverman_bandwidth(&[1.0, f64::NAN, 2.0, f64::INFINITY, 3.0]);
        assert!((finite - mixed).abs() < 1e-15);
    }

    #[test]
    fn kde_grid_returns_requested_finite_ordered_points() {
        let points = compute_kernel_density(KernelDensityInput {
            values: &[0.0, 1.0, 2.0, f64::NAN],
            grid_points: 32,
            min_x: None,
        })
        .points;
        assert_eq!(points.len(), 32);
        assert!(points.windows(2).all(|pair| pair[0].x < pair[1].x));
        assert!(
            points.iter().all(|point| point.x.is_finite()
                && point.density.is_finite()
                && point.density >= 0.0)
        );
    }

    #[test]
    fn kde_grid_respects_finite_min_x_without_changing_point_count() {
        let points = compute_kernel_density(KernelDensityInput {
            values: &[0.1, 0.2, 0.3],
            grid_points: 32,
            min_x: Some(0.0),
        })
        .points;
        assert_eq!(points.len(), 32);
        assert_eq!(points.first().map(|point| point.x), Some(0.0));
    }
}
