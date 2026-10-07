//! Paired-row percentile inference for composite path effects. Retains effects, not fits.
use super::*;
use crate::descriptive::quantile_sorted;
use rand::{RngExt, SeedableRng, rngs::StdRng};
pub(super) fn percentile_effects(
    n: usize,
    points: &[f64],
    options: MediationOptions,
    control: &Control,
    estimate: impl Fn(&[usize]) -> Result<Vec<f64>>,
) -> Result<Vec<PathEffect>> {
    if options.replications == 1 {
        return Err(parameter());
    }
    let mut samples = vec![Vec::with_capacity(options.replications); points.len()];
    let mut rng = StdRng::seed_from_u64(options.seed);
    for _ in 0..options.replications {
        control.check()?;
        let indices: Vec<_> = (0..n).map(|_| rng.random_range(0..n)).collect();
        // All stages use identical sampled rows. A failed fit aborts inference.
        let values = estimate(&indices)?;
        if values.len() != points.len() {
            return Err(parameter());
        }
        for (sample, value) in samples.iter_mut().zip(values) {
            sample.push(finite(value)?);
        }
    }
    points
        .iter()
        .zip(samples)
        .map(|(&point, mut sample)| {
            control.check()?;
            let (standard_error, confidence_interval) = if sample.is_empty() {
                (None, None)
            } else {
                let mean = sample.iter().map(|v| v / sample.len() as f64).sum::<f64>();
                let sd = finite(
                    (sample
                        .iter()
                        .map(|v| (v - mean).powi(2) / (sample.len() - 1) as f64)
                        .sum::<f64>())
                    .sqrt(),
                )?;
                sample.sort_by(f64::total_cmp);
                (
                    Some(sd),
                    Some([
                        quantile_sorted(&sample, 0.025),
                        quantile_sorted(&sample, 0.975),
                    ]),
                )
            };
            Ok(PathEffect {
                estimate: finite(point)?,
                standard_error,
                statistic: None,
                p_value: None,
                confidence_interval,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        time::{Duration, Instant},
    };
    #[test]
    fn percentile_effect_interpolation_and_failed_replications_are_not_silently_discarded() {
        let c = Control {
            cancellation: Default::default(),
            deadline: Instant::now() + Duration::from_secs(10),
        };
        let options = MediationOptions {
            moderated_stage: MediatedStage::None,
            probe_sd: 1.,
            replications: 5,
            seed: 1,
        };
        let count = Cell::new(0);
        let result = percentile_effects(640, &[2.], options, &c, |rows| {
            assert_eq!(rows.len(), 640);
            assert!(rows.iter().all(|&i| i < 640));
            let n = count.get();
            count.set(n + 1);
            Ok(vec![n as f64])
        })
        .unwrap();
        let e = &result[0];
        assert_eq!(e.confidence_interval, Some([0.1, 3.9]));
        assert!((e.standard_error.unwrap() - 2.5_f64.sqrt()).abs() < 1e-12);
        assert!(percentile_effects(640, &[2.], options, &c, |_| Err(parameter())).is_err());
    }
}
