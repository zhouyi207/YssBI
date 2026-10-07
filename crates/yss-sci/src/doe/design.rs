//! Deterministic coded designs; callers admit the returned dimensions before allocation.
use super::uniform;
use super::*;

fn prime(n: usize, control: &Control) -> Result<bool> {
    if n < 2 {
        return Ok(false);
    }
    if n.is_multiple_of(2) {
        return Ok(n == 2);
    }
    let mut d = 3;
    while d <= n / d {
        control.check()?;
        if n.is_multiple_of(d) {
            return Ok(false);
        }
        d += 2;
    }
    Ok(true)
}

fn factorial_runs(factors: usize, levels: usize) -> Result<usize> {
    if factors == 0 || levels < 2 {
        return Err(parameter());
    }
    levels
        .checked_pow(u32::try_from(factors).map_err(|_| parameter())?)
        .ok_or_else(parameter)
}

fn orthogonal_shape(factors: usize, levels: usize) -> Result<(usize, usize)> {
    let (mut runs, mut dimensions) = (levels, 1);
    while (runs - 1) / (levels - 1) < factors {
        runs = runs.checked_mul(levels).ok_or_else(parameter)?;
        dimensions += 1;
    }
    Ok((runs, dimensions))
}

pub fn design_dimensions(spec: DesignSpecification, control: &Control) -> Result<(usize, usize)> {
    control.check()?;
    let (runs, factors) = match spec {
        DesignSpecification::FullFactorial { factors, levels } => {
            (factorial_runs(factors, levels)?, factors)
        }
        DesignSpecification::Orthogonal { factors, levels } => {
            if factors == 0 || levels < 2 {
                return Err(parameter());
            }
            let runs = if prime(levels, control)? {
                orthogonal_shape(factors, levels)?.0
            } else {
                factorial_runs(factors, levels)?
            };
            (runs, factors)
        }
        DesignSpecification::Uniform {
            factors,
            runs,
            candidates,
            ..
        } => {
            if factors == 0 || runs == 0 || candidates == 0 {
                return Err(parameter());
            }
            (runs, factors)
        }
    };
    // Codes and run identifiers must retain exact integer identity in numeric columns.
    if runs as u128 > (1_u128 << 53) || factors as u128 > (1_u128 << 53) {
        return Err(parameter());
    }
    runs.checked_mul(factors.checked_add(1).ok_or_else(parameter)?)
        .and_then(|cells| cells.checked_mul(size_of::<f64>()))
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(parameter)?;
    Ok((runs, factors))
}

pub fn generate_design(spec: DesignSpecification, control: &Control) -> Result<DesignResult> {
    let (runs, factors) = design_dimensions(spec, control)?;
    let (rows, method, levels, candidates, seed, discrepancy) = match spec {
        DesignSpecification::FullFactorial { levels, .. } => (
            full_factorial(runs, factors, levels, control)?,
            "full_factorial",
            levels,
            None,
            None,
            None,
        ),
        DesignSpecification::Orthogonal { levels, .. } => {
            let (rows, method) = if prime(levels, control)? {
                (
                    orthogonal(runs, factors, levels, control)?,
                    "orthogonal_prime_field",
                )
            } else {
                (
                    full_factorial(runs, factors, levels, control)?,
                    "orthogonal_full_factorial",
                )
            };
            (rows, method, levels, None, None, None)
        }
        DesignSpecification::Uniform {
            candidates, seed, ..
        } => {
            let (rows, discrepancy) = uniform::search(runs, factors, candidates, seed, control)?;
            (
                rows,
                "uniform_centered_latin_hypercube",
                runs,
                Some(candidates),
                Some(seed),
                Some(discrepancy),
            )
        }
    };
    control.check()?;
    Ok(DesignResult {
        rows,
        summary: DesignSummary {
            method,
            runs,
            factors,
            levels,
            candidates,
            seed,
            centered_l2_discrepancy: discrepancy,
        },
    })
}

fn full_factorial(
    runs: usize,
    factors: usize,
    levels: usize,
    control: &Control,
) -> Result<Vec<Vec<f64>>> {
    let mut rows = Vec::with_capacity(runs);
    for i in 0..runs {
        if i.is_multiple_of(256) {
            control.check()?;
        }
        let mut code = i;
        let mut row = Vec::with_capacity(factors + 1);
        row.push((i + 1) as f64);
        for _ in 0..factors {
            row.push((code % levels + 1) as f64);
            code /= levels;
        }
        rows.push(row);
    }
    Ok(rows)
}

fn orthogonal(
    runs: usize,
    factors: usize,
    levels: usize,
    control: &Control,
) -> Result<Vec<Vec<f64>>> {
    let dimensions = orthogonal_shape(factors, levels)?.1;
    // One representative of each one-dimensional subspace: first nonzero digit = 1.
    // Any two distinct representatives are independent over the prime field.
    let mut directions = Vec::with_capacity(factors);
    for code in 1..runs {
        if code.is_multiple_of(256) {
            control.check()?;
        }
        let mut v = code;
        let digits: Vec<_> = (0..dimensions)
            .map(|_| {
                let d = v % levels;
                v /= levels;
                d
            })
            .collect();
        if digits.iter().find(|&&d| d != 0) == Some(&1) {
            directions.push(digits);
        }
        if directions.len() == factors {
            break;
        }
    }
    let mut rows = Vec::with_capacity(runs);
    for i in 0..runs {
        control.check()?;
        let mut v = i;
        let digits: Vec<_> = (0..dimensions)
            .map(|_| {
                let d = v % levels;
                v /= levels;
                d
            })
            .collect();
        let mut row = Vec::with_capacity(factors + 1);
        row.push((i + 1) as f64);
        for (j, direction) in directions.iter().enumerate() {
            if j.is_multiple_of(256) {
                control.check()?;
            }
            let code = digits.iter().zip(direction).fold(0_u128, |s, (&a, &b)| {
                (s + a as u128 * b as u128) % levels as u128
            });
            row.push((code + 1) as f64);
        }
        rows.push(row);
    }
    Ok(rows)
}
