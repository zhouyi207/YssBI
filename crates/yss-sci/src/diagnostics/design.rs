//! Design-matrix collinearity and the unrotated PCA version of Harman's diagnostic.
use crate::regression::models::common::{
    Design, Result, failed, finite, names, parameter, validate,
};
use yss_sci_contract::{
    diagnostics::model::*, execution::ScientificExecutionControl as Control,
    multivariate::PcaOptions,
};
use yss_sci_linalg::{Mat, Svd};

pub fn collinearity(
    columns: &[Vec<f64>],
    constant: bool,
    control: &Control,
) -> Result<CollinearityResult> {
    let n = columns.first().map_or(0, Vec::len);
    if columns.is_empty() || n < 2 {
        return Err(parameter());
    }
    validate(&columns[0], columns, control)?;
    let design = Design::new(columns, n, constant, true, false, control)?;
    let p = design.x.ncols();
    let norms = (0..p)
        .map(|j| (0..n).map(|i| design.x[(i, j)].powi(2)).sum::<f64>().sqrt())
        .collect::<Vec<_>>();
    let x = Mat::from_fn(n, p, |i, j| {
        if norms[j] == 0.0 {
            0.0
        } else {
            design.x[(i, j)] / norms[j]
        }
    });
    // Work on the design itself: forming X'X loses the small eigenvalues
    // that distinguish severe collinearity from exact linear dependence.
    let svd = Svd::factor_thin(x.as_ref()).map_err(|_| failed())?;
    control.check()?;
    let singular = svd.values();
    if singular.iter().any(|s| !s.is_finite() || *s < 0.0) {
        return Err(failed());
    }
    let largest = singular[0];
    let tolerance = largest * (n.max(p) as f64 * f64::EPSILON);
    let rank = singular.iter().filter(|&&s| s > tolerance).count();
    let eigenvalues = (0..p)
        .map(|j| if j < rank { singular[j].powi(2) } else { 0.0 })
        .collect();
    let condition_indices = (0..p)
        .map(|j| {
            if j < rank {
                Some(largest / singular[j])
            } else {
                None
            }
        })
        .collect();
    let condition = if rank == p {
        Some(largest / singular[p - 1])
    } else {
        None
    };
    let labels = names(columns.len(), constant);
    let terms = (0..p)
        .map(|j| {
            let is_constant = (constant && j == 0) || norms[j] == 0.0;
            if is_constant {
                return Ok(CollinearityTerm {
                    term: labels[j].clone(),
                    constant: true,
                    vif: None,
                    tolerance: None,
                });
            }
            let null_component = 1.0
                - (0..rank)
                    .map(|k| svd.right_vectors()[(j, k)].powi(2))
                    .sum::<f64>();
            if null_component > 1e-10 {
                return Ok(CollinearityTerm {
                    term: labels[j].clone(),
                    constant: false,
                    vif: None,
                    tolerance: Some(0.0),
                });
            }
            let vif = finite(
                (0..rank)
                    .map(|k| (svd.right_vectors()[(j, k)] / singular[k]).powi(2))
                    .sum::<f64>(),
            )?
            .max(1.0);
            Ok(CollinearityTerm {
                term: labels[j].clone(),
                constant: false,
                vif: Some(vif),
                tolerance: Some(1.0 / vif),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CollinearityResult {
        observations: n,
        columns: p,
        rank,
        full_column_rank: rank == p,
        centered: constant,
        condition_number: condition,
        eigenvalues,
        condition_indices,
        terms,
    })
}

pub fn harman(columns: &[Vec<f64>], control: &Control) -> Result<HarmanResult> {
    if columns.len() < 2 {
        return Err(parameter());
    }
    let pca = crate::multivariate::pca(
        columns,
        PcaOptions {
            components: 1,
            standardize: true,
        },
        control,
    )?
    .report;
    let first = pca.eigenvalues[0];
    Ok(HarmanResult {
        extraction: "unrotated_correlation_pca".into(),
        observations: pca.observations,
        variables: pca.variables,
        first_component_ratio: pca.explained_variance_ratio[0],
        first_component_loadings: pca.weights.iter().map(|w| w[0] * first.sqrt()).collect(),
        eigenvalues_above_one: pca.eigenvalues.iter().filter(|&&v| v > 1.0).count(),
        eigenvalues: pca.eigenvalues,
        explained_variance_ratio: pca.explained_variance_ratio,
    })
}
