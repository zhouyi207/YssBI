//! Aligned, budgeted tabular preparation shared by statistical adapters.
use crate::builtins::series;
use crate::{KernelError, KernelInvocation, RuntimeValue};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::{ConversionMetadata, SemanticType, SemanticValue, TabularScalar};

pub(in crate::builtins::statistics) struct Category<'a>(
    pub(in crate::builtins::statistics) &'a TabularScalar,
);
impl PartialEq for Category<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Category<'_> {}
impl PartialOrd for Category<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Category<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Keep exact mixed integer/float equality; distinct scalar families never alias.
        fn family(value: &TabularScalar) -> u8 {
            match value {
                TabularScalar::Null => 0,
                TabularScalar::Bool(_) => 1,
                TabularScalar::Integer(_)
                | TabularScalar::Unsigned(_)
                | TabularScalar::Float64(_) => 2,
                TabularScalar::String(_) => 3,
            }
        }
        self.0
            .compare(other.0)
            .unwrap_or_else(|| family(self.0).cmp(&family(other.0)))
    }
}

pub(in crate::builtins::statistics) fn materialize(
    inv: &KernelInvocation<'_>,
) -> Result<(Vec<series::Column>, usize), KernelError> {
    inv.check_control()?;
    let columns = series::columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, true)?;
    let n = columns.first().map_or(0, |column| column.values.len());
    let mut retained = inv.control.check_bytes(
        n.checked_mul(columns.len())
            .and_then(|n| n.checked_mul(size_of::<RuntimeValue>() * 8)),
    )?;
    for column in &columns {
        for (i, scalar) in column.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            if let TabularScalar::String(label) = scalar {
                retained = inv.control.check_bytes(
                    label
                        .len()
                        .checked_mul(8)
                        .and_then(|n| retained.checked_add(n)),
                )?;
            }
        }
    }
    Ok((columns, retained))
}

pub(in crate::builtins::statistics) fn numeric(
    column: &series::Column,
    binary: bool,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<f64>, KernelError> {
    column
        .values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if i % 1024 == 0 {
                inv.check_control()?;
            }
            if binary && let TabularScalar::Bool(v) = v {
                return Ok(f64::from(u8::from(*v)));
            }
            crate::builtins::numeric_input(Some(&RuntimeValue::Scalar(v.clone())))
        })
        .collect()
}
pub(in crate::builtins::statistics) fn category_code(
    v: &TabularScalar,
) -> Result<Cow<'_, str>, KernelError> {
    Ok(match v {
        TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
        TabularScalar::Bool(v) => Cow::Owned(v.to_string()),
        TabularScalar::Integer(v) => Cow::Owned(v.to_string()),
        TabularScalar::Unsigned(v) => Cow::Owned(v.to_string()),
        TabularScalar::Float64(v) => Cow::Owned(v.as_f64().to_string()),
        TabularScalar::String(v) => Cow::Borrowed(v),
    })
}

pub(in crate::builtins::statistics) fn ordinal_domain(
    metadata: Option<&ConversionMetadata>,
) -> Option<&[SemanticValue]> {
    metadata
        .filter(|metadata| metadata.semantic.kind == SemanticType::Ordinal)
        .map(|metadata| metadata.semantic.values.as_slice())
}

pub(in crate::builtins::statistics) fn ordinal_index<'a>(
    domain: &'a [SemanticValue],
    inv: &KernelInvocation<'_>,
    retained: usize,
) -> Result<BTreeMap<&'a str, usize>, KernelError> {
    // Borrow codes; charge conservative tree storage together with live preparation buffers.
    inv.control.check_bytes(
        domain
            .len()
            .checked_mul(128)
            .and_then(|bytes| retained.checked_add(bytes)),
    )?;
    let mut index = BTreeMap::new();
    for (position, value) in domain.iter().enumerate() {
        if position.is_multiple_of(1024) {
            inv.check_control()?;
        }
        index.insert(value.value.as_str(), position);
    }
    inv.check_control()?;
    Ok(index)
}
pub(in crate::builtins::statistics) fn categories(
    column: &series::Column,
    ordered: bool,
    inv: &KernelInvocation<'_>,
) -> Result<(Vec<usize>, Vec<TabularScalar>), KernelError> {
    let mut labels: Vec<TabularScalar> = vec![];
    let mut seen = BTreeSet::new();
    let working = inv.control.check_bytes(
        column
            .values
            .len()
            .checked_mul(size_of::<usize>() + size_of::<TabularScalar>() + 96),
    )?;
    for (i, v) in column.values.iter().enumerate() {
        if i % 1024 == 0 {
            inv.check_control()?;
        }
        if matches!(v, TabularScalar::Null) {
            return Err(KernelError::InvalidNumericInput);
        }
        if seen.insert(Category(v)) {
            labels.push(v.clone());
        }
    }
    if ordered {
        if let Some(domain) = ordinal_domain(column.metadata.as_deref()) {
            let retained = inv
                .control
                .check_bytes(column.bytes().and_then(|bytes| bytes.checked_add(working)))?;
            let order = ordinal_index(domain, inv, retained)?;
            if order.is_empty() {
                return Err(KernelError::InvalidParameter);
            }
            let mut ranked = inv.control.reserve(labels.len())?;
            for (position, label) in labels.drain(..).enumerate() {
                if position.is_multiple_of(1024) {
                    inv.check_control()?;
                }
                let rank = order
                    .get(category_code(&label)?.as_ref())
                    .copied()
                    .ok_or(KernelError::InvalidParameter)?;
                ranked.push((rank, label));
            }
            ranked.sort_by_key(|(rank, _)| *rank);
            inv.check_control()?;
            labels.extend(ranked.into_iter().map(|(_, label)| label));
        } else {
            if labels.iter().any(|v| matches!(v, TabularScalar::String(_)))
                || labels
                    .first()
                    .is_some_and(|a| labels.iter().any(|b| a.compare(b).is_none()))
            {
                return Err(KernelError::InvalidParameter);
            }
            labels.sort_by(|a, b| a.compare(b).expect("comparable categories"));
        }
    }
    drop(seen);
    let positions = labels
        .iter()
        .enumerate()
        .map(|(i, v)| (Category(v), i))
        .collect::<BTreeMap<_, _>>();
    let codes = column
        .values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            positions
                .get(&Category(v))
                .copied()
                .ok_or(KernelError::InvalidNumericInput)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((codes, labels))
}
