//! Shared Gaussian kernel density estimation.

/// A point on a kernel density estimate curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KdePoint {
    pub x: f64,
    pub density: f64,
}

/// Standard Gaussian kernel `K(u) = exp(-u² / 2) / sqrt(2π)`.
#[inline]
fn gaussian_kernel(u: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;
    INV_SQRT_2PI * (-0.5 * u * u).exp()
}

/// Silverman's rule-of-thumb bandwidth.
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

/// Computes a Gaussian KDE on an evenly spaced, padded grid.
///
/// `grid_points` is the exact number of returned points. Non-finite samples are
/// ignored, and fewer than two requested grid points produce an empty curve.
pub fn gaussian_kde_grid(values: &[f64], grid_points: usize) -> Vec<KdePoint> {
    gaussian_kde_grid_with_min_x(values, grid_points, None)
}

/// Computes a Gaussian KDE with an optional lower bound for the grid.
pub fn gaussian_kde_grid_with_min_x(
    values: &[f64],
    grid_points: usize,
    min_x: Option<f64>,
) -> Vec<KdePoint> {
    let values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() || grid_points < 2 {
        return Vec::new();
    }

    let bandwidth = silverman_bandwidth(&values);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = ((max - min) * 0.15).max(bandwidth * 2.0).max(0.1);
    let padded_min = min - padding;
    let grid_min = min_x
        .filter(|value| value.is_finite())
        .map_or(padded_min, |value| value.max(padded_min));
    let grid_max = max + padding;
    let denominator = (grid_points - 1) as f64;

    (0..grid_points)
        .map(|index| {
            let x = grid_min + index as f64 / denominator * (grid_max - grid_min);
            KdePoint {
                x,
                density: kde_at(x, &values, bandwidth),
            }
        })
        .collect()
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
    use super::{
        gaussian_kde_grid, gaussian_kde_grid_with_min_x, gaussian_kernel, silverman_bandwidth,
    };

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
        let points = gaussian_kde_grid(&[0.0, 1.0, 2.0, f64::NAN], 32);
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
        let points = gaussian_kde_grid_with_min_x(&[0.1, 0.2, 0.3], 32, Some(0.0));
        assert_eq!(points.len(), 32);
        assert_eq!(points.first().map(|point| point.x), Some(0.0));
    }
}
