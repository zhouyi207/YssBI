//! Pseudo-likelihood coefficients with design-based score covariance and survey t inference.
use super::*;
use crate::regression::{
    design::covariance_rows,
    models::{
        common::{coefficient_table, gram, inverse, names},
        glm::PreparedGlm,
    },
};
use yss_sci_contract::regression::models::{
    GlmFamily, GlmLink, GlmOptions, ModelStatistics, RegressionDetails, RegressionModelResult,
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
    let prepared = PreparedGlm::new(
        y,
        predictors,
        GlmOptions {
            constant: options.constant,
            family: options.family,
            link,
            fractional: false,
            iteration: options.iteration,
        },
        Some(&design.weights),
        control,
    )?;
    let df = design
        .summary
        .degrees_of_freedom
        .checked_add(1)
        .and_then(|d| d.checked_sub(prepared.design.x.ncols()))
        .filter(|d| *d > 0)
        .ok_or_else(parameter)?;
    let fit = prepared.fit(control)?;
    let x = &fit.prepared.design;
    let fisher_weights: Vec<_> = fit
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
        design.weights[i] * x.x[(i, j)] * (y[i] - fit.fitted[i])
    });
    let meat = design.covariance(&scores, control)?;
    let covariance = bread.as_ref() * meat.as_ref() * bread.as_ref();
    let (beta, raw_cov) = x.raw(&fit.beta, Some(covariance));
    let raw_cov = raw_cov.ok_or_else(failed)?;
    let coefficients = coefficient_table(
        &beta,
        names(predictors.len(), options.constant),
        Some(&raw_cov),
        Some(df),
        0.95,
    )?;
    let dispersion = fit.dispersion()?;
    let residuals = y.iter().zip(&fit.fitted).map(|(y, mu)| y - mu).collect();
    let model = RegressionModelResult {
        method: match options.family {
            GlmFamily::Gaussian => "survey_linear",
            GlmFamily::Binomial => "survey_logistic",
            GlmFamily::Poisson => "survey_poisson",
            _ => unreachable!(),
        }
        .into(),
        observations: y.len(),
        constant: options.constant,
        coefficients,
        covariance: Some(covariance_rows(&raw_cov)),
        fitted: fit.fitted,
        residuals,
        categories: vec![],
        fitted_categories: vec![],
        probabilities: vec![],
        statistics: ModelStatistics {
            rss: None,
            rmse: None,
            r_squared: None,
            adjusted_r_squared: None,
            df_residual: Some(df),
            log_likelihood: None,
            aic: None,
            bic: None,
        },
        iterations: fit.iterations,
        converged: true,
        details: RegressionDetails::Glm {
            family: options.family,
            link,
            dispersion,
            deviance: fit.deviance,
            covariance_method: "single_stage_with_replacement_taylor".into(),
        },
    };
    control.check()?;
    Ok(SurveyRegression {
        model,
        diagnostics: SurveyRegressionDiagnostics {
            design: design.summary,
            coefficient_degrees_of_freedom: df,
            family: options.family,
        },
    })
}
