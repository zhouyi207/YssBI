use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl as Control,
    ScientificInputViolation as Violation,
};
use yss_sci_contract::panel::PanelData;

pub(super) type Result<T> = std::result::Result<T, Error>;
pub(super) fn invalid(violation: Violation) -> Error {
    Error::InvalidInput { violation }
}
pub(super) fn parameter() -> Error {
    invalid(Violation::ParameterOutOfRange)
}
pub(super) fn failed() -> Error {
    Error::ComputationFailed
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
