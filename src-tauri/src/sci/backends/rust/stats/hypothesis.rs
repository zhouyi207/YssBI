use yss_sci::stats::{
    Alternative as YssSciAlternative, t_test as yss_t_test, wald_test as yss_wald_test,
};

use crate::sci::api::stats::hypothesis::{
    Alternative, LinearHypothesisTestInput, TTestOutput, WaldTestOutput,
};
use crate::sci::error::{SciError, SciInputViolation, SciOperationCode};

pub fn t_test(input: LinearHypothesisTestInput<'_>) -> Result<TTestOutput, SciError> {
    let result = yss_t_test(
        input.betas,
        input.cov_beta,
        input.r,
        input.r_vec,
        input.df_residual,
        convert_alternative(input.alternative),
        input.constraint_desc,
    )
    .map_err(|_| SciError::InvalidInput {
        operation: SciOperationCode::TTest,
        violation: SciInputViolation::ShapeMismatch,
    })?;

    Ok(TTestOutput {
        alternative: result.alternative,
        r_beta_minus_r: result.r_beta_minus_r,
        stat: result.stat,
        df: result.df,
        p_value: result.p_value,
    })
}

pub fn wald_test(input: LinearHypothesisTestInput<'_>) -> Result<WaldTestOutput, SciError> {
    let result = yss_wald_test(
        input.betas,
        input.cov_beta,
        input.r,
        input.r_vec,
        input.df_residual,
        convert_alternative(input.alternative),
        input.constraint_desc,
    )
    .map_err(|_| SciError::InvalidInput {
        operation: SciOperationCode::WaldTest,
        violation: SciInputViolation::ShapeMismatch,
    })?;

    Ok(WaldTestOutput {
        alternative: result.alternative,
        r_beta_minus_r: result.r_beta_minus_r,
        stat: result.stat,
        df1: result.df1,
        df2: result.df2,
        p_value: result.p_value,
    })
}

fn convert_alternative(alternative: Alternative) -> YssSciAlternative {
    match alternative {
        Alternative::TwoSided => YssSciAlternative::TwoSided,
        Alternative::Greater => YssSciAlternative::Greater,
        Alternative::Less => YssSciAlternative::Less,
    }
}
