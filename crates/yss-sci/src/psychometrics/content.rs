use super::*;
use statrs::distribution::{Binomial, Discrete};
pub fn content_validity(columns: &[Vec<f64>], control: &Control) -> Result<ContentValidityResult> {
    let Some(first) = columns.first() else {
        return Err(parameter());
    };
    validate(first, columns, control)?;
    let n = first.len();
    let trials = u64::try_from(n).map_err(|_| parameter())?;
    let chance = Binomial::new(0.5, trials).map_err(|_| parameter())?;
    let mut rows = Vec::with_capacity(columns.len());
    for (j, column) in columns.iter().enumerate() {
        control.check()?;
        let mut relevant = 0;
        for (i, &v) in column.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if ![1., 2., 3., 4.].contains(&v) {
                return Err(parameter());
            }
            relevant += usize::from(v >= 3.);
        }
        let cvi = relevant as f64 / n as f64;
        let pc = chance.pmf(relevant as u64);
        rows.push(ContentValidityItem {
            item: j + 1,
            relevant_experts: relevant,
            item_cvi: cvi,
            chance_agreement: finite(pc)?,
            modified_kappa: finite((cvi - pc) / (1. - pc))?,
        });
    }
    Ok(ContentValidityResult {
        summary: ContentValiditySummary {
            method: "expert_relevance_cvi",
            experts: n,
            items: columns.len(),
            scale_cvi_average: rows.iter().map(|r| r.item_cvi / rows.len() as f64).sum(),
            scale_cvi_universal_agreement: rows.iter().filter(|r| r.relevant_experts == n).count()
                as f64
                / rows.len() as f64,
        },
        rows,
    })
}
