//! NPS, the classic Kano evaluation table, and tie-preserving RFM quintiles.
use super::data::*;
use yss_sci_contract::decision::preferences::*;

pub fn nps(ratings: &[f64], control: &Control) -> Result<NpsResult> {
    validate(ratings, &[], control)?;
    let mut counts = [0; 11];
    for (i, &rating) in ratings.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if !(0.0..=10.0).contains(&rating) || rating.fract() != 0. {
            return Err(parameter());
        }
        counts[rating as usize] += 1;
    }
    let detractors = counts[..7].iter().sum();
    let passives = counts[7] + counts[8];
    let promoters = counts[9] + counts[10];
    let percent = |count: usize| 100. * count as f64 / ratings.len() as f64;
    Ok(NpsResult {
        observations: ratings.len(),
        detractors,
        passives,
        promoters,
        detractor_percent: percent(detractors),
        passive_percent: percent(passives),
        promoter_percent: percent(promoters),
        net_promoter_score: percent(promoters) - percent(detractors),
        rating_counts: counts,
    })
}

pub fn kano(functional: &[f64], dysfunctional: &[f64], control: &Control) -> Result<KanoResult> {
    validate(functional, &[], control)?;
    validate(dysfunctional, &[], control)?;
    if functional.len() != dysfunctional.len() {
        return Err(
            yss_sci_contract::execution::ScientificComputationError::InvalidInput {
                violation: yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
            },
        );
    }
    // Classic Berger et al. (1993), figure 4: codes are like/expect/neutral/tolerate/dislike.
    const TABLE: [[usize; 5]; 5] = [
        [5, 0, 0, 0, 1],
        [4, 3, 3, 3, 2],
        [4, 3, 3, 3, 2],
        [4, 3, 3, 3, 2],
        [4, 4, 4, 4, 5],
    ];
    const NAMES: [&str; 6] = [
        "attractive",
        "one_dimensional",
        "must_be",
        "indifferent",
        "reverse",
        "questionable",
    ];
    let mut counts = [0usize; 6];
    for (i, (&f, &d)) in functional.iter().zip(dysfunctional).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if [f, d]
            .iter()
            .any(|v| !(1.0..=5.0).contains(v) || v.fract() != 0.)
        {
            return Err(parameter());
        }
        counts[TABLE[f as usize - 1][d as usize - 1]] += 1;
    }
    let valid: usize = counts[..4].iter().sum();
    let maximum = counts.iter().copied().max().unwrap_or(0);
    Ok(KanoResult {
        observations: functional.len(),
        categories: NAMES
            .iter()
            .zip(counts)
            .map(|(&category, count)| KanoCategory {
                category,
                count,
                percent: 100. * count as f64 / functional.len() as f64,
            })
            .collect(),
        dominant_categories: NAMES
            .iter()
            .zip(counts)
            .filter_map(|(&name, count)| (count == maximum).then_some(name))
            .collect(),
        coefficient_observations: valid,
        better: (valid > 0).then(|| (counts[0] + counts[1]) as f64 / valid as f64),
        worse: (valid > 0).then(|| -((counts[2] + counts[1]) as f64) / valid as f64),
    })
}

pub fn rfm(columns: &[Vec<f64>], control: &Control) -> Result<RfmResult> {
    let (n, _) = dimensions(columns, &[false; 3], control)?;
    if columns[0].iter().any(|v| *v < 0.) || columns[1].iter().any(|v| *v < 0. || v.fract() != 0.) {
        return Err(parameter());
    }
    let mut scores = Vec::with_capacity(3);
    let mut counts = [[0; 5]; 3];
    for (j, values) in columns.iter().enumerate() {
        let mut ranks = crate::association::ranks(values, control)?.values;
        for (i, rank) in ranks.iter_mut().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if j == 0 {
                *rank = n as f64 + 1. - *rank;
            }
            // Midpoint plotting positions keep every tie together, including constant columns.
            let tier = (5. * (*rank - 0.5) / n as f64).floor().clamp(0., 4.) as usize;
            counts[j][tier] += 1;
            *rank = (tier + 1) as f64;
        }
        scores.push(ranks);
    }
    let mut rows = Vec::with_capacity(n);
    for (i, ((&r, &f), &m)) in scores[0].iter().zip(&scores[1]).zip(&scores[2]).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let r = r as usize;
        let f = f as usize;
        let m = m as usize;
        rows.push(RfmRow {
            observation: i + 1,
            recency_score: r,
            frequency_score: f,
            monetary_score: m,
            total: r + f + m,
        });
    }
    Ok(RfmResult {
        summary: RfmSummary {
            observations: n,
            scoring: "midrank_quintiles",
            tiers: 5,
            recency_counts: counts[0],
            frequency_counts: counts[1],
            monetary_counts: counts[2],
        },
        rows,
    })
}
