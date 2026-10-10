use crate::regression::models::common::{Result, finite, parameter};
use statrs::distribution::{Continuous, Normal};
use std::collections::HashMap;
use yss_sci_contract::{
    execution::ScientificExecutionControl as Control,
    regression::{fit::BinaryRegressionLink, postestimation::Evaluation},
};

/// Stores only overrides and column means, never a second observation matrix.
pub(crate) struct EvaluationDesign<'a> {
    columns: &'a [Vec<f64>],
    fixed: Vec<Option<f64>>,
    pub rows: usize,
}
impl<'a> EvaluationDesign<'a> {
    pub fn new(
        columns: &'a [Vec<f64>],
        names: &[String],
        constant: bool,
        evaluation: Evaluation,
        at: &HashMap<String, f64>,
        control: &Control,
    ) -> Result<Self> {
        control.check()?;
        let n = columns.first().ok_or_else(parameter)?.len();
        if n == 0 || columns.len() != names.len() || columns.iter().any(|c| c.len() != n) {
            return Err(parameter());
        }
        let mut fixed = vec![None; columns.len()];
        for (name, &v) in at {
            let matches = names
                .iter()
                .enumerate()
                .filter(|(_, s)| *s == name)
                .map(|(j, _)| j)
                .collect::<Vec<_>>();
            if matches.len() != 1 || !v.is_finite() || (constant && matches[0] == 0) {
                return Err(parameter());
            }
            fixed[matches[0]] = Some(v);
        }
        for (j, column) in columns.iter().enumerate() {
            let mut mean = 0.;
            for (i, &v) in column.iter().enumerate() {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                finite(v)?;
                mean += v / n as f64;
            }
            if evaluation == Evaluation::AtMeans && fixed[j].is_none() {
                fixed[j] = Some(finite(mean)?);
            }
        }
        Ok(Self {
            columns,
            fixed,
            rows: if evaluation == Evaluation::Average {
                n
            } else {
                1
            },
        })
    }
    pub fn value(&self, row: usize, column: usize) -> f64 {
        self.fixed[column].unwrap_or(self.columns[column][row])
    }
}

/// Probability and first/second derivatives; logit complement stays stable in tails.
pub(crate) fn binary_link(
    link: BinaryRegressionLink,
    eta: f64,
    normal: &Normal,
) -> (f64, f64, f64, f64) {
    match link {
        BinaryRegressionLink::Logit => {
            let e = (-eta.abs()).exp();
            let (p, q) = if eta >= 0. {
                (1. / (1. + e), e / (1. + e))
            } else {
                (e / (1. + e), 1. / (1. + e))
            };
            let d = p * q;
            (p, d, d * (q - p), q)
        }
        BinaryRegressionLink::Probit => {
            let p = crate::distribution::normal::cdf(eta);
            let d = normal.pdf(eta);
            (p, d, -eta * d, 0.)
        }
    }
}

pub(crate) fn delta_standard_error(gradient: &[f64], covariance: &[Vec<f64>]) -> Result<f64> {
    let k = gradient.len();
    if covariance.len() != k || covariance.iter().any(|r| r.len() != k) {
        return Err(parameter());
    }
    let (variance, scale) = (0..k)
        .flat_map(|a| (0..k).map(move |b| (a, b)))
        .map(|(a, b)| gradient[a] * covariance[a][b] * gradient[b])
        .fold((0., 0.), |(sum, scale), term| {
            (sum + term, scale + term.abs())
        });
    let roundoff = 8. * f64::EPSILON * (k as f64).powi(2) * scale;
    if !variance.is_finite() || !scale.is_finite() || variance < -roundoff {
        return Err(parameter());
    }
    Ok(variance.max(0.).sqrt())
}
