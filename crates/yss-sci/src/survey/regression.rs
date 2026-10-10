//! Pseudo-likelihood coefficients with design-based score covariance and survey t inference.
use super::*;
use crate::regression::models::{
    common::{Design, coefficient_table, gram, inverse, names},
    glm::weighted_glm,
};
use yss_sci_contract::regression::models::{
    GlmFamily, GlmLink, GlmOptions, ModelStatistics, RegressionDetails,
};
use yss_sci_linalg::Mat;
pub fn regression(
    y: &[f64],
    predictors: &[Vec<f64>],
    survey: SurveyDesign<'_>,
    options: SurveyRegressionOptions,
    control: &Control,
) -> Result<SurveyRegression> {
    validate(y, predictors, control)?;
    let link = match options.family {
        GlmFamily::Gaussian => GlmLink::Identity,
        GlmFamily::Binomial => GlmLink::Logit,
        GlmFamily::Poisson => GlmLink::Log,
        _ => return Err(parameter()),
    };
    let design = design::PreparedDesign::new(survey, y.len(), control)?;
    let x = Design::new(predictors, y.len(), options.constant, true, true, control)?;
    let df = design
        .summary
        .degrees_of_freedom
        .checked_add(1)
        .and_then(|d| d.checked_sub(x.x.ncols()))
        .filter(|d| *d > 0)
        .ok_or_else(parameter)?;
    let mut model = weighted_glm(
        y,
        predictors,
        &design.weights,
        GlmOptions {
            constant: options.constant,
            family: options.family,
            link,
            fractional: false,
            iteration: options.iteration,
        },
        control,
    )?;
    let fisher_weights: Vec<_> = model
        .fitted
        .iter()
        .zip(&design.weights)
        .map(|(&m, &w)| {
            w * match options.family {
                GlmFamily::Gaussian => 1.,
                GlmFamily::Binomial => m * (1. - m),
                GlmFamily::Poisson => m,
                _ => unreachable!(),
            }
        })
        .collect();
    let bread = inverse(&gram(&x.x, Some(&fisher_weights), control)?)?;
    // Identity, logit and log are canonical links: individual scores are w*x*(y-mu).
    let scores = Mat::from_fn(y.len(), x.x.ncols(), |i, j| {
        design.weights[i] * x.x[(i, j)] * (y[i] - model.fitted[i])
    });
    let meat = design.covariance(&scores, control)?;
    let covariance = bread.as_ref() * meat.as_ref() * bread.as_ref();
    let (_, raw_cov) = x.raw(&vec![0.; x.x.ncols()], Some(covariance));
    let raw_cov = raw_cov.ok_or_else(failed)?;
    let beta: Vec<_> = model.coefficients.iter().map(|c| c.estimate).collect();
    model.coefficients = coefficient_table(
        &beta,
        names(predictors.len(), options.constant),
        Some(&raw_cov),
        Some(df),
        0.95,
    )?;
    model.covariance = Some(
        (0..raw_cov.nrows())
            .map(|i| (0..raw_cov.ncols()).map(|j| raw_cov[(i, j)]).collect())
            .collect(),
    );
    model.statistics = ModelStatistics {
        rss: None,
        rmse: None,
        r_squared: None,
        adjusted_r_squared: None,
        df_residual: Some(df),
        log_likelihood: None,
        aic: None,
        bic: None,
    };
    model.method = match options.family {
        GlmFamily::Gaussian => "survey_linear",
        GlmFamily::Binomial => "survey_logistic",
        GlmFamily::Poisson => "survey_poisson",
        _ => unreachable!(),
    }
    .into();
    if let RegressionDetails::Glm {
        covariance_method, ..
    } = &mut model.details
    {
        *covariance_method = "single_stage_with_replacement_taylor".into();
    }
    Ok(SurveyRegression {
        model,
        diagnostics: SurveyRegressionDiagnostics {
            design: design.summary,
            coefficient_degrees_of_freedom: df,
            family: options.family,
        },
    })
}
