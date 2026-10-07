//! Shared centering and probe coordinates for observed-variable path models.
use super::*;
pub(super) struct CenteredColumns {
    pub centers: Vec<f64>,
    pub values: Vec<Vec<f64>>,
}
pub(super) fn center_columns(columns: &[Vec<f64>], control: &Control) -> Result<CenteredColumns> {
    let mut centers = Vec::with_capacity(columns.len());
    let mut centered = Vec::with_capacity(columns.len());
    for x in columns {
        control.check()?;
        let scale = x.iter().map(|x| x.abs()).fold(0., f64::max).max(1.);
        let anchor = x[0] / scale;
        let shift = x
            .iter()
            .map(|v| (v / scale - anchor) / x.len() as f64)
            .sum::<f64>();
        centers.push(finite((anchor + shift) * scale)?);
        centered.push(
            x.iter()
                .map(|x| finite((x / scale - anchor - shift) * scale))
                .collect::<Result<_>>()?,
        );
    }
    Ok(CenteredColumns {
        centers,
        values: centered,
    })
}
pub(super) fn sample_sd(centered: &[f64]) -> Result<f64> {
    let scale = centered.iter().map(|v| v.abs()).fold(0., f64::max);
    if centered.len() < 2 || scale == 0. {
        return Err(parameter());
    }
    finite(
        scale
            * (centered
                .iter()
                .map(|v| (v / scale).powi(2) / (centered.len() - 1) as f64)
                .sum::<f64>())
            .sqrt(),
    )
}
pub(super) fn range(values: &[f64]) -> [f64; 2] {
    [
        values.iter().copied().fold(f64::INFINITY, f64::min),
        values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    ]
}
