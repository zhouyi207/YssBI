//! Shared k-class fit facts for 2SLS and LIML.
use crate::distribution::{fisher_snedecor_sf, normal_two_sided_p, student_t_probability};
use crate::regression::design::covariance_rows;
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal, StudentsT};
use yss_sci_contract::causal::iv::{InstrumentalVariableModelTest, InstrumentalVariableStatistics};
use yss_sci_contract::hypothesis::Alternative;
use yss_sci_contract::regression::fit::RegressionCoefficientStatistics;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve};

#[derive(Debug)]
pub struct IvEstimate {
    pub betas: Col<f64>,
    pub fitted: Col<f64>,
    pub residuals: Col<f64>,
    pub inference: RegressionCoefficientStatistics,
    pub statistics: InstrumentalVariableStatistics,
}

pub(super) fn coefficient_inference(
    betas: &Col<f64>,
    covariance: &Mat<f64>,
    df_residual: usize,
    small: bool,
) -> Result<RegressionCoefficientStatistics, String> {
    let mut standard_errors = Vec::with_capacity(betas.nrows());
    let mut statistic_values = Vec::with_capacity(betas.nrows());
    for (j, &beta) in betas.iter().enumerate() {
        let variance = covariance[(j, j)];
        if !variance.is_finite() || variance < 0.0 {
            return Err("IV: coefficient variance is invalid".into());
        }
        let standard_error = variance.sqrt();
        let statistic = beta / standard_error;
        if !statistic.is_finite() {
            return Err("IV: coefficient inference is undefined".into());
        }
        standard_errors.push(standard_error);
        statistic_values.push(statistic);
    }

    let student = small
        .then(|| StudentsT::new(0.0, 1.0, df_residual as f64))
        .transpose()
        .map_err(|error| format!("IV: {error}"))?;
    let critical = if let Some(student) = &student {
        student.inverse_cdf(0.975)
    } else {
        Normal::new(0.0, 1.0)
            .map_err(|error| format!("IV: {error}"))?
            .inverse_cdf(0.975)
    };
    let p_values = statistic_values
        .iter()
        .map(|&statistic| match &student {
            Some(student) => student_t_probability(student, statistic, Alternative::TwoSided),
            None => normal_two_sided_p(statistic),
        })
        .collect();
    let mut confidence_interval_lower = Vec::with_capacity(betas.nrows());
    let mut confidence_interval_upper = Vec::with_capacity(betas.nrows());
    for (&beta, &standard_error) in betas.iter().zip(&standard_errors) {
        let half_width = critical * standard_error;
        let lower = beta - half_width;
        let upper = beta + half_width;
        if !lower.is_finite() || !upper.is_finite() {
            return Err("IV: coefficient confidence interval is undefined".into());
        }
        confidence_interval_lower.push(lower);
        confidence_interval_upper.push(upper);
    }
    Ok(RegressionCoefficientStatistics {
        covariance: covariance_rows(covariance),
        standard_errors,
        statistic_values,
        p_values,
        confidence_interval_lower,
        confidence_interval_upper,
    })
}

pub(super) fn model_test(
    betas: &Col<f64>,
    covariance: &Mat<f64>,
    constant: bool,
    df_residual: usize,
    small: bool,
) -> Result<InstrumentalVariableModelTest, String> {
    let start = usize::from(constant);
    let df = betas.nrows() - start;
    if df == 0 {
        return Err("IV: joint test requires a nonconstant coefficient".into());
    }
    let slopes = betas.subrows(start, df);
    let slope_covariance = covariance.submatrix(start, start, df, df);
    let solution = slope_covariance
        .checked_cholesky()
        .map_err(|_| "IV: joint-test covariance is not positive definite".to_string())?
        .solve(&slopes);
    let wald = slopes.transpose() * solution.as_ref();
    if !wald.is_finite() || wald < 0.0 {
        return Err("IV: joint test is undefined".into());
    }
    if small {
        let statistic = wald / df as f64;
        let distribution = FisherSnedecor::new(df as f64, df_residual as f64)
            .map_err(|error| format!("IV: {error}"))?;
        Ok(InstrumentalVariableModelTest::F {
            statistic,
            df_numerator: df,
            df_denominator: df_residual,
            p_value: fisher_snedecor_sf(&distribution, statistic),
        })
    } else {
        let distribution = ChiSquared::new(df as f64).map_err(|error| format!("IV: {error}"))?;
        Ok(InstrumentalVariableModelTest::ChiSquared {
            statistic: wald,
            df,
            p_value: distribution.sf(wald),
        })
    }
}
