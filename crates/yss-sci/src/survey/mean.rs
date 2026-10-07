use super::*;
use statrs::distribution::{ContinuousCDF, StudentsT};
use yss_sci_linalg::Mat;
pub fn mean(
    y: &[f64],
    design: SurveyDesign<'_>,
    proportion: bool,
    control: &Control,
) -> Result<SurveyMean> {
    validate(y, &[], control)?;
    if proportion && y.iter().any(|&v| v != 0. && v != 1.) {
        return Err(parameter());
    }
    let design = design::PreparedDesign::new(design, y.len(), control)?;
    let weight_sum = design.weights.iter().sum::<f64>();
    let scale = y.iter().map(|v| v.abs()).fold(1., f64::max);
    let anchor = y[0] / scale;
    let estimate = finite(
        (anchor
            + y.iter()
                .zip(&design.weights)
                .map(|(y, w)| (y / scale - anchor) * (w / weight_sum))
                .sum::<f64>())
            * scale,
    )?;
    let scores = Mat::from_fn(y.len(), 1, |i, _| {
        (design.weights[i] / weight_sum) * (y[i] - estimate)
    });
    let covariance = design.covariance(&scores, control)?;
    let standard_error = finite(covariance[(0, 0)].sqrt())?;
    let t =
        StudentsT::new(0., 1., design.summary.degrees_of_freedom as f64).map_err(|_| failed())?;
    let width = t.inverse_cdf(0.975) * standard_error;
    Ok(SurveyMean {
        kind: if proportion { "proportion" } else { "mean" }.into(),
        estimate,
        standard_error,
        confidence_interval: [finite(estimate - width)?, finite(estimate + width)?],
        boundary_proportion: proportion && y.iter().all(|v| *v == y[0]),
        design: design.summary,
    })
}
