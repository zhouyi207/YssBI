use super::super::effects::{self, AffineEffect};
use super::super::preparation::range;
use super::*;
use statrs::distribution::StudentsT;
use yss_sci_contract::regression::models::RegressionModelResult;
pub(super) fn probe(
    model: &RegressionModelResult,
    columns: &[Vec<f64>],
    prepared: &CenteredColumns,
    options: ModerationOptions,
    control: &Control,
) -> Result<ModerationDiagnostics> {
    let centers = &prepared.centers;
    let centered = &prepared.values;
    let beta: Vec<_> = model.coefficients.iter().map(|c| c.estimate).collect();
    let covariance = model.covariance.as_ref().ok_or_else(parameter)?;
    let t = StudentsT::new(
        0.,
        1.,
        model.statistics.df_residual.ok_or_else(parameter)? as f64,
    )
    .map_err(|_| parameter())?;
    let wsd = super::super::preparation::sample_sd(&centered[1])?;
    let wrange = range(&columns[1]);
    let zsd = if options.second_moderator {
        super::super::preparation::sample_sd(&centered[2])?
    } else {
        0.
    };
    let zrange = if options.second_moderator {
        range(&columns[2])
    } else {
        [0., 0.]
    };
    let zprobes = if options.second_moderator {
        vec![-options.probe_sd * zsd, 0., options.probe_sd * zsd]
    } else {
        vec![0.]
    };
    let mut effects = Vec::new();
    let mut johnson_neyman = Vec::new();
    for z in zprobes {
        control.check()?;
        let zvalue = if options.second_moderator {
            Some(finite(z + centers[2])?)
        } else {
            None
        };
        let mut a = vec![0.; beta.len()];
        let mut b = a.clone();
        a[1] = 1.;
        if options.second_moderator {
            a[5] = z;
            b[4] = 1.;
            b[7] = z;
        } else {
            b[3] = 1.;
        }
        for w in [-options.probe_sd * wsd, 0., options.probe_sd * wsd] {
            let weights = a
                .iter()
                .zip(&b)
                .map(|(a, b)| finite(a + w * b))
                .collect::<Result<Vec<_>>>()?;
            let moderator = finite(w + centers[1])?;
            effects.push(ConditionalEffect {
                moderator,
                second_moderator: zvalue,
                in_observed_ranges: moderator >= wrange[0]
                    && moderator <= wrange[1]
                    && zvalue.is_none_or(|v| v >= zrange[0] && v <= zrange[1]),
                effect: effects::estimate(&beta, &weights, covariance, &t)?,
            });
        }
        let at_zero = effects::estimate(&beta, &a, covariance, &t)?.estimate;
        let gradient = effects::estimate(&beta, &b, covariance, &t)?.estimate;
        johnson_neyman.push(effects::johnson_neyman(
            AffineEffect {
                intercept: at_zero,
                slope: gradient,
                variance_intercept: effects::covariance(&a, &a, covariance)?,
                covariance: effects::covariance(&a, &b, covariance)?,
                variance_slope: effects::covariance(&b, &b, covariance)?,
            },
            &t,
            centers[1],
            wrange,
            zvalue,
        )?);
    }
    Ok(ModerationDiagnostics {
        input_centers: centers.clone(),
        probe_sd: options.probe_sd,
        effects,
        johnson_neyman,
    })
}
