//! Columnar relation conversion; row details never grow the structured summary report.
use super::*;

pub(super) fn effects(
    rows: &[StudyEffect],
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    numeric_table(
        rows,
        1,
        [
            "study",
            "effect",
            "variance",
            "standard_error",
            "lower",
            "upper",
        ],
        |s| {
            [
                s.study as f64,
                s.effect,
                s.variance,
                s.standard_error,
                s.lower,
                s.upper,
            ]
        },
        inv,
    )
}
pub(super) fn studies(
    rows: &[MetaStudy],
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    numeric_table(
        rows,
        1,
        [
            "study",
            "effect",
            "variance",
            "standard_error",
            "lower",
            "upper",
            "weight",
            "fitted",
            "residual",
        ],
        |s| {
            [
                s.effect.study as f64,
                s.effect.effect,
                s.effect.variance,
                s.effect.standard_error,
                s.effect.lower,
                s.effect.upper,
                s.weight,
                s.fitted,
                s.residual,
            ]
        },
        inv,
    )
}
pub(super) fn omissions(
    rows: &[OmissionResult],
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    numeric_table(
        rows,
        1,
        [
            "omitted_study",
            "estimate",
            "standard_error",
            "lower",
            "upper",
            "tau_squared",
            "i_squared_percent",
        ],
        |s| {
            [
                s.omitted_study as f64,
                s.estimate,
                s.standard_error,
                s.lower,
                s.upper,
                s.tau_squared,
                s.i_squared_percent,
            ]
        },
        inv,
    )
}
