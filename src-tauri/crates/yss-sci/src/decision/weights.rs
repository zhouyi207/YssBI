use super::data::*;

pub fn calculate(
    columns: &[Vec<f64>],
    costs: &[bool],
    method: WeightMethod,
    explicit: &[f64],
    control: &Control,
) -> Result<WeightResult> {
    let (n, p) = dimensions(columns, costs, control)?;
    if n < 2 && !matches!(method, WeightMethod::Equal | WeightMethod::Explicit) {
        return Err(parameter());
    }
    let (importance, statistic) = match method {
        WeightMethod::Equal => (vec![1.; p], vec![]),
        WeightMethod::Explicit => {
            if explicit.len() != p {
                return Err(parameter());
            }
            (explicit.to_vec(), vec![])
        }
        WeightMethod::Entropy => {
            let z = utilities(columns, costs, true, control)?;
            let mut entropy = Vec::with_capacity(p);
            for x in &z {
                control.check()?;
                let sum = x.iter().sum::<f64>();
                let h = if sum > 0. {
                    -x.iter()
                        .filter(|v| **v > 0.)
                        .map(|v| {
                            let q = v / sum;
                            q * q.ln()
                        })
                        .sum::<f64>()
                        / (n as f64).ln()
                } else {
                    1.
                };
                entropy.push(h.clamp(0., 1.));
            }
            (entropy.iter().map(|h| (1. - h).max(0.)).collect(), entropy)
        }
        WeightMethod::Critic => {
            let z = utilities(columns, costs, true, control)?;
            let stats = z.iter().map(|x| moments(x)).collect::<Vec<_>>();
            let mut information = vec![0.; p];
            for j in 0..p {
                control.check()?;
                let (mean, sd) = stats[j];
                if sd == 0. {
                    continue;
                }
                for k in 0..p {
                    if stats[k].1 == 0. {
                        continue;
                    }
                    let r = (z[j]
                        .iter()
                        .zip(&z[k])
                        .map(|(a, b)| (a - mean) * (b - stats[k].0))
                        .sum::<f64>()
                        / ((n - 1) as f64 * sd * stats[k].1))
                        .clamp(-1., 1.);
                    information[j] += sd * (1. - r);
                }
            }
            (information.clone(), information)
        }
        WeightMethod::Information => {
            let cv = columns
                .iter()
                .map(|x| {
                    control.check()?;
                    if x.iter().any(|v| *v < 0.) {
                        return Err(parameter());
                    }
                    let scale = x.iter().copied().fold(0., f64::max);
                    if scale <= 0. {
                        return Err(parameter());
                    }
                    let scaled = x.iter().map(|v| v / scale).collect::<Vec<_>>();
                    let (mean, sd) = moments(&scaled);
                    finite(sd / mean)
                })
                .collect::<Result<Vec<_>>>()?;
            (cv.clone(), cv)
        }
        WeightMethod::Independence => {
            if p < 2 {
                return Err(parameter());
            }
            let diagnostic = crate::diagnostics::design::collinearity(columns, true, control)?;
            let correlations = diagnostic
                .terms
                .iter()
                .skip(1)
                .map(|term| {
                    let tolerance = term.tolerance.ok_or_else(parameter)?;
                    let r = (1. - tolerance).max(0.).sqrt();
                    if r == 0. {
                        return Err(parameter());
                    }
                    finite(r)
                })
                .collect::<Result<Vec<_>>>()?;
            (correlations.iter().map(|r| 1. / r).collect(), correlations)
        }
    };
    control.check()?;
    Ok(WeightResult {
        method,
        weights: normalized_weights(&importance)?,
        statistic,
    })
}
