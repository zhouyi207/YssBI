//! Van Westendorp curves evaluated at observed prices, with explicit interpolation.
use super::data::*;
use yss_sci_contract::decision::market::*;

pub fn price_sensitivity(
    columns: &[Vec<f64>],
    definition: PriceRangeDefinition,
    control: &Control,
) -> Result<PriceResult> {
    let (n, _) = dimensions(columns, &[false; 4], control)?;
    for (i, &lowest) in columns[0].iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if lowest < 0. || (0..3).any(|j| columns[j][i] > columns[j + 1][i]) {
            return Err(parameter());
        }
    }
    let mut ordered = columns.to_vec();
    for x in &mut ordered {
        x.sort_unstable_by(f64::total_cmp);
        control.check()?;
    }
    let mut prices = ordered.iter().flatten().copied().collect::<Vec<_>>();
    prices.sort_unstable_by(f64::total_cmp);
    prices.dedup();
    let mut positions = [0usize; 4];
    let mut rows = Vec::with_capacity(prices.len());
    for (i, price) in prices.into_iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        for j in 0..4 {
            while positions[j] < n && ordered[j][positions[j]] <= price {
                positions[j] += 1;
                if positions[j].is_multiple_of(1024) {
                    control.check()?;
                }
            }
        }
        let cdf = positions.map(|v| v as f64 / n as f64);
        rows.push(PriceRow {
            price,
            too_cheap: 1. - cdf[0],
            cheap: 1. - cdf[1],
            expensive: cdf[2],
            too_expensive: cdf[3],
        });
    }
    let marginal_cheapness = intersection(
        &rows,
        |r| match definition {
            PriceRangeDefinition::Original => r.too_cheap - (1. - r.cheap),
            PriceRangeDefinition::Narrower => r.too_cheap - r.expensive,
        },
        control,
    )?;
    let marginal_expensiveness = intersection(
        &rows,
        |r| match definition {
            PriceRangeDefinition::Original => (1. - r.expensive) - r.too_expensive,
            PriceRangeDefinition::Narrower => r.cheap - r.too_expensive,
        },
        control,
    )?;
    let indifference = intersection(&rows, |r| r.cheap - r.expensive, control)?;
    let optimal = intersection(&rows, |r| r.too_cheap - r.too_expensive, control)?;
    Ok(PriceResult {
        summary: PriceSummary {
            observations: n,
            range_definition: definition,
            curve_convention: "right_ecdf_linear_between_observed_prices",
            marginal_cheapness,
            marginal_expensiveness,
            indifference,
            optimal,
        },
        rows,
    })
}

/// Each difference is monotone, so its zero set is a single point or closed interval.
fn intersection(
    rows: &[PriceRow],
    difference: impl Fn(&PriceRow) -> f64,
    control: &Control,
) -> Result<Option<PriceIntersection>> {
    let mut result: Option<PriceIntersection> = None;
    for (i, row) in rows.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let d = difference(row);
        if d.abs() <= 8. * f64::EPSILON {
            if let Some(interval) = &mut result {
                interval.upper = row.price;
            } else {
                result = Some(PriceIntersection {
                    lower: row.price,
                    upper: row.price,
                });
            }
        } else if i > 0 && result.is_none() {
            let previous = &rows[i - 1];
            let a = difference(previous);
            if a > 0. && d < 0. {
                let fraction = a / (a - d);
                let price = finite(previous.price * (1. - fraction) + row.price * fraction)?;
                result = Some(PriceIntersection {
                    lower: price,
                    upper: price,
                });
            }
        }
    }
    Ok(result)
}
