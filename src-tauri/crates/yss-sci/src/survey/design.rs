//! Nested stratum/PSU indexing and with-replacement Taylor covariance.
use super::*;
use std::collections::BTreeMap;
use yss_sci_linalg::Mat;
pub(super) struct PreparedDesign {
    pub weights: Vec<f64>,
    pub summary: SurveyDesignSummary,
    row_psu: Vec<usize>,
    strata: Vec<Vec<usize>>,
}
impl PreparedDesign {
    pub fn new(design: SurveyDesign<'_>, n: usize, control: &Control) -> Result<Self> {
        control.check()?;
        if design.weights.len() != n
            || design.strata.is_some_and(|v| v.len() != n)
            || design.clusters.is_some_and(|v| v.len() != n)
        {
            return Err(parameter());
        }
        let summary = super::weights::summary(design.weights, control)?;
        let scaled_sum = design
            .weights
            .iter()
            .map(|w| w / summary.maximum)
            .sum::<f64>();
        let weights = design
            .weights
            .iter()
            .map(|w| finite((w / summary.maximum) / scaled_sum * n as f64))
            .collect::<Result<Vec<_>>>()?;
        if weights.iter().any(|w| *w <= 0.) {
            return Err(parameter());
        }
        let mut ids = BTreeMap::new();
        let mut strata = BTreeMap::<usize, Vec<usize>>::new();
        let mut row_psu = Vec::with_capacity(n);
        for i in 0..n {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            let stratum = design.strata.map_or(0, |s| s[i]);
            let psu = design.clusters.map_or(i, |c| c[i]);
            let next = ids.len();
            let id = *ids.entry((stratum, psu)).or_insert_with(|| {
                strata.entry(stratum).or_default().push(next);
                next
            });
            row_psu.push(id);
        }
        let certainty_strata = strata.values().filter(|g| g.len() == 1).count();
        if (certainty_strata > 0 && design.lonely_psu == LonelyPsu::Fail)
            || ids.len() == strata.len()
        {
            return Err(parameter());
        }
        Ok(Self {
            weights,
            summary: SurveyDesignSummary {
                strata: strata.len(),
                primary_sampling_units: ids.len(),
                degrees_of_freedom: ids.len() - strata.len(),
                certainty_strata,
                variance_method: "single_stage_with_replacement_taylor".into(),
                weights: summary,
            },
            row_psu,
            strata: strata.into_values().collect(),
        })
    }
    pub fn covariance(&self, scores: &Mat<f64>, control: &Control) -> Result<Mat<f64>> {
        control.check()?;
        if scores.nrows() != self.row_psu.len() {
            return Err(parameter());
        }
        let k = scores.ncols();
        let mut totals = Mat::zeros(self.summary.primary_sampling_units, k);
        for (i, &g) in self.row_psu.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            for j in 0..k {
                totals[(g, j)] += scores[(i, j)];
            }
        }
        let mut cov = Mat::zeros(k, k);
        for stratum in &self.strata {
            control.check()?;
            if stratum.len() == 1 {
                continue;
            }
            let mh = stratum.len() as f64;
            let means: Vec<_> = (0..k)
                .map(|j| stratum.iter().map(|&g| totals[(g, j)] / mh).sum::<f64>())
                .collect();
            for &g in stratum {
                for j in 0..k {
                    for l in 0..=j {
                        cov[(j, l)] +=
                            (totals[(g, j)] - means[j]) * (totals[(g, l)] - means[l]) * mh
                                / (mh - 1.);
                    }
                }
            }
        }
        for j in 0..k {
            for l in 0..=j {
                cov[(j, l)] = finite(cov[(j, l)])?;
                cov[(l, j)] = cov[(j, l)];
            }
        }
        control.check()?;
        Ok(cov)
    }
}
