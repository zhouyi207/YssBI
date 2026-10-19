use std::collections::BTreeMap;

pub(super) use crate::error::{failed, invalid, parameter};
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl as Control,
    ScientificInputViolation as Violation,
};
use yss_sci_contract::panel::PanelData;
use yss_sci_linalg::Col;

pub(super) type Result<T> = std::result::Result<T, Error>;

/// Subtract group means after the estimator has validated matching lengths.
pub(super) fn within_transform_by_group(v: &[f64], group_id: &[usize]) -> Col<f64> {
    let mut sums: BTreeMap<usize, (f64, usize)> = BTreeMap::new();
    for (i, &gid) in group_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(gid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }
    Col::from_fn(v.len(), |i| {
        let (sum, count) = sums.get(&group_id[i]).copied().unwrap_or((0.0, 0));
        let mean = if count > 0 { sum / count as f64 } else { 0.0 };
        v[i] - mean
    })
}

/// Two-way within transformation z̃_it = z_it - z̄_i - z̄_t + z̄ after the estimator has
/// validated matching lengths.
pub(super) fn within_transform_twoway(
    v: &[f64],
    entity_id: &[usize],
    time_id: &[usize],
) -> Col<f64> {
    let n = v.len();
    let mut e_sums: BTreeMap<usize, (f64, usize)> = BTreeMap::new();
    let mut t_sums: BTreeMap<usize, (f64, usize)> = BTreeMap::new();
    let mut total = 0.0;
    let mut total_cnt = 0usize;
    for (i, &val) in v.iter().enumerate() {
        if !val.is_nan() {
            let e_entry = e_sums.entry(entity_id[i]).or_insert((0.0, 0));
            e_entry.0 += val;
            e_entry.1 += 1;
            let t_entry = t_sums.entry(time_id[i]).or_insert((0.0, 0));
            t_entry.0 += val;
            t_entry.1 += 1;
            total += val;
            total_cnt += 1;
        }
    }
    let z_bar = if total_cnt > 0 {
        total / total_cnt as f64
    } else {
        0.0
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (es, ec) = e_sums.get(&entity_id[i]).copied().unwrap_or((0.0, 0));
        let e_bar = if ec > 0 { es / ec as f64 } else { 0.0 };
        let (ts, tc) = t_sums.get(&time_id[i]).copied().unwrap_or((0.0, 0));
        let t_bar = if tc > 0 { ts / tc as f64 } else { 0.0 };
        out.push(v[i] - e_bar - t_bar + z_bar);
    }
    out.into_iter().collect::<Col<f64>>()
}

/// Validate before indexing, then retain the source order mapping through all transforms.
pub(super) fn groups(data: &PanelData<'_>, control: &Control) -> Result<Vec<Vec<usize>>> {
    control.check()?;
    let n = data.response.len();
    if n == 0
        || data.entity.len() != n
        || data.time.len() != n
        || data.predictors.iter().any(|x| x.len() != n)
    {
        return Err(invalid(Violation::ShapeMismatch));
    }
    for column in [data.response, data.entity, data.time]
        .into_iter()
        .chain(data.predictors.iter().map(Vec::as_slice))
    {
        for (i, value) in column.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if !value.is_finite() {
                return Err(invalid(Violation::NonFiniteInput));
            }
        }
    }
    if data
        .time
        .iter()
        .any(|t| t.fract() != 0.0 || t.abs() > 9_007_199_254_740_991.0)
    {
        return Err(parameter());
    }
    let mut order = (0..n).collect::<Vec<_>>();
    // Finite numbers use numerical identity, including +0 == -0.
    order.sort_unstable_by(|&a, &b| {
        data.entity[a]
            .partial_cmp(&data.entity[b])
            .unwrap()
            .then(data.time[a].partial_cmp(&data.time[b]).unwrap())
    });
    control.check()?;
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, row) in order.into_iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if let Some(group) = groups.last_mut()
            && data.entity[group[0]] == data.entity[row]
        {
            if data.time[row] - data.time[*group.last().unwrap()] != 1.0 {
                return Err(parameter());
            }
            group.push(row);
        } else {
            groups.push(vec![row]);
        }
    }
    if groups.len() < 2 {
        return Err(parameter());
    }
    Ok(groups)
}
