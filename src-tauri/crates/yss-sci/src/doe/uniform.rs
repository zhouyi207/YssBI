//! Candidate search minimizes squared centered L2 discrepancy, not a global optimum.
use super::*;
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};

pub(super) fn search(
    runs: usize,
    factors: usize,
    candidates: usize,
    seed: u64,
    control: &Control,
) -> Result<(Vec<Vec<f64>>, f64)> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut best = Vec::new();
    let mut best_discrepancy = f64::INFINITY;
    let mut permutation: Vec<_> = (1..=runs).collect();
    for _ in 0..candidates {
        control.check()?;
        let mut rows: Vec<_> = (1..=runs)
            .map(|i| {
                let mut r = Vec::with_capacity(factors + 1);
                r.push(i as f64);
                r
            })
            .collect();
        for _ in 0..factors {
            control.check()?;
            permutation.shuffle(&mut rng);
            for (row, &level) in rows.iter_mut().zip(&permutation) {
                row.push(level as f64);
            }
        }
        let discrepancy = centered_discrepancy(&rows, control)?;
        if discrepancy < best_discrepancy {
            best = rows;
            best_discrepancy = discrepancy;
        }
    }
    Ok((best, best_discrepancy))
}

fn centered_discrepancy(rows: &[Vec<f64>], control: &Control) -> Result<f64> {
    let n = rows.len() as f64;
    let p = rows[0].len() - 1;
    let point = |level: f64| (level - 0.5) / n;
    let mut single = 0.;
    let mut pairs = 0.;
    for (i, row) in rows.iter().enumerate() {
        control.check()?;
        let product = row[1..]
            .iter()
            .map(|&l| {
                let a = (point(l) - 0.5).abs();
                1. + 0.5 * a - 0.5 * a * a
            })
            .product::<f64>();
        single += product / n;
        for (k, other) in rows.iter().enumerate().take(i + 1) {
            if k.is_multiple_of(128) {
                control.check()?;
            }
            let product = row[1..]
                .iter()
                .zip(&other[1..])
                .map(|(&a, &b)| {
                    let (x, y) = (point(a), point(b));
                    1. + 0.5 * (x - 0.5).abs() + 0.5 * (y - 0.5).abs() - 0.5 * (x - y).abs()
                })
                .product::<f64>();
            pairs += product / n / n * if i == k { 1. } else { 2. };
        }
    }
    // SciPy qmc.discrepancy(method="CD") convention: do not take the square root.
    let base = (13_f64 / 12.).powf(p as f64);
    let result = finite(base - 2. * single + pairs)?;
    if result < -128. * f64::EPSILON * (base + 2. * single + pairs) {
        return Err(parameter());
    }
    Ok(result.max(0.))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centered_discrepancy_matches_scipy_documented_reference() {
        let rows = [[1., 3.], [2., 6.], [3., 2.], [4., 5.], [5., 1.], [6., 4.]]
            .iter()
            .enumerate()
            .map(|(i, r)| vec![(i + 1) as f64, r[0], r[1]])
            .collect::<Vec<_>>();
        let c = Control {
            cancellation: Default::default(),
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        };
        assert!((centered_discrepancy(&rows, &c).unwrap() - 0.008142039609053464).abs() < 1e-14);
    }
}
