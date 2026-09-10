//! Panel 对齐与差分
//!
//! 复用 TS align 思路：按 entity 分组，每组内补齐时间轴到规则网格，缺失为 NaN。
//! panel_diff 在 align 后的数据上对相邻行做一阶差分（仅当两侧均非 NaN 时输出）。

use std::collections::HashMap;

use super::time_series::align::{time_array, time_numbers};
use crate::data::{MAX_PREPARED_BYTES, PreparationError, check_size};
use arrow::array::{Array, Float64Array, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Schema};
use arrow::record_batch::RecordBatch;
use std::sync::Arc;

/// 对齐后的面板数据：(entity_id, time_id, value_columns)
/// 已按 (entity, time) 排序，缺失时间点为 NaN
#[derive(Debug, Clone)]
pub struct AlignedPanel {
    pub entity_id: Vec<usize>,
    pub time_id: Vec<usize>,
    pub columns: Vec<Vec<f64>>,
}

/// 按 entity 分组补齐时间轴到规则网格
///
/// * `entity_id` - 实体 ID
/// * `time_id` - 时间 ID（usize，通常为 0,1,2,... 索引）
/// * `columns` - 数值列，每列与 entity_id/time_id 等长
/// * `interval` - 时间步长，默认 1
pub fn align_panel(
    entity_id: &[usize],
    time_id: &[usize],
    columns: &[Vec<f64>],
    interval: Option<usize>,
) -> Result<AlignedPanel, String> {
    let n = entity_id.len();
    if time_id.len() != n {
        return Err(format!(
            "align_panel: entity_id len {} != time_id len {}",
            n,
            time_id.len()
        ));
    }
    for (i, col) in columns.iter().enumerate() {
        if col.len() != n {
            return Err(format!(
                "align_panel: column {} len {} != n {}",
                i,
                col.len(),
                n
            ));
        }
    }

    let interval = interval.unwrap_or(1).max(1);

    // 收集每个 entity 的 (time, row_idx)
    let mut entity_times: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for i in 0..n {
        let eid = entity_id[i];
        let tid = time_id[i];
        entity_times.entry(eid).or_default().push((tid, i));
    }

    // 对每个 entity 排序并生成完整时间网格
    let mut out_entity = Vec::new();
    let mut out_time = Vec::new();
    let mut out_cols: Vec<Vec<f64>> = (0..columns.len()).map(|_| Vec::new()).collect();

    let mut eids: Vec<_> = entity_times.keys().copied().collect();
    eids.sort_unstable();

    for eid in eids {
        let mut rows = entity_times[&eid].clone();
        rows.sort_by_key(|(t, _)| *t);

        if rows.is_empty() {
            continue;
        }

        let min_t = rows.iter().map(|(t, _)| *t).min().unwrap();
        let max_t = rows.iter().map(|(t, _)| *t).max().unwrap();

        // 生成完整时间网格
        let mut full_times = Vec::new();
        let mut t = min_t;
        while t <= max_t {
            full_times.push(t);
            t = match t.checked_add(interval) {
                Some(next) => next,
                None => break,
            };
        }

        // 建立 time -> row_idx 映射
        let time_to_idx: HashMap<usize, usize> = rows.into_iter().map(|(t, i)| (t, i)).collect();

        for &tid in &full_times {
            out_entity.push(eid);
            out_time.push(tid);

            if let Some(&row_idx) = time_to_idx.get(&tid) {
                for (c, out_col) in out_cols.iter_mut().enumerate() {
                    out_col.push(columns[c][row_idx]);
                }
            } else {
                for out_col in out_cols.iter_mut() {
                    out_col.push(f64::NAN);
                }
            }
        }
    }

    Ok(AlignedPanel {
        entity_id: out_entity,
        time_id: out_time,
        columns: out_cols,
    })
}

/// 在 align 后的数据上按 entity 分组做一阶差分
///
/// 对每个 entity 内，用「当前观测 - 上一个非 NaN 观测」做 diff。
/// 与 Stata D. 算子一致：reg D.y D.x, nocons（xtset id time 后）
/// 支持时间有缺失时仍正确计算 Δy_t = y_t - y_{t'}，其中 t' 为上一期有效观测时间。
///
/// 返回 (diff_entity, diff_time_id, diff_cols)，其中 diff_time_id 为每个 diff 行对应的 time_id（当前观测时间）
pub fn panel_diff(
    aligned: &AlignedPanel,
) -> Result<(Vec<usize>, Vec<usize>, Vec<Vec<f64>>), String> {
    let n = aligned.entity_id.len();
    if n == 0 {
        return Err("panel_diff: empty aligned panel".to_string());
    }

    let k = aligned.columns.len();
    let mut diff_entity = Vec::new();
    let mut diff_time_id = Vec::new();
    let mut diff_cols: Vec<Vec<f64>> = (0..k).map(|_| Vec::new()).collect();

    let mut i = 0;
    while i < n {
        let eid = aligned.entity_id[i];
        // 记录该 entity 内上一组有效值（用于跨 gap 的 diff）
        let mut prev_vals: Option<Vec<f64>> = None;

        // 检查当前行是否全列有效
        let row_valid = |idx: usize| (0..k).all(|c| !aligned.columns[c][idx].is_nan());

        if row_valid(i) {
            prev_vals = Some((0..k).map(|c| aligned.columns[c][i]).collect());
        }

        let mut j = i + 1;
        while j < n && aligned.entity_id[j] == eid {
            if row_valid(j) {
                if let Some(ref pv) = prev_vals {
                    diff_entity.push(eid);
                    diff_time_id.push(aligned.time_id[j]);
                    for c in 0..k {
                        diff_cols[c].push(aligned.columns[c][j] - pv[c]);
                    }
                }
                prev_vals = Some((0..k).map(|c| aligned.columns[c][j]).collect());
            }
            j += 1;
        }
        i = j;
    }

    if diff_entity.is_empty() {
        return Err(
            "panel_diff: no valid first-differenced observations. Ensure (entity, time) has consecutive periods."
                .to_string(),
        );
    }

    Ok((diff_entity, diff_time_id, diff_cols))
}

struct PanelInput {
    panel: AlignedPanel,
    entity_rows: Vec<u64>,
    unique_times: Vec<i64>,
    value_indices: Vec<usize>,
    entity_index: usize,
    time_index: usize,
}

fn prepare_panel(
    batch: &RecordBatch,
    entity: &str,
    time: &str,
) -> Result<PanelInput, PreparationError> {
    let n = batch.num_rows();
    if n == 0 {
        return Err(PreparationError::Empty);
    }
    let width = batch
        .num_columns()
        .checked_mul(16)
        .and_then(|width| width.checked_add(96))
        .ok_or(PreparationError::MemoryLimit)?;
    check_size(n, width)?;
    if batch.get_array_memory_size() > MAX_PREPARED_BYTES {
        return Err(PreparationError::MemoryLimit);
    }
    let entity_index = batch
        .schema()
        .index_of(entity)
        .map_err(|_| PreparationError::Column)?;
    let time_index = batch
        .schema()
        .index_of(time)
        .map_err(|_| PreparationError::Column)?;
    if entity_index == time_index {
        return Err(PreparationError::Column);
    }
    let names = arrow::compute::cast(batch.column(entity_index).as_ref(), &DataType::Utf8)?;
    let names = names
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or(PreparationError::ValueType)?;
    if names.null_count() != 0 {
        return Err(PreparationError::ValueType);
    }
    let mut ids = HashMap::new();
    let mut entity_rows = Vec::new();
    let entity_id = names
        .iter()
        .enumerate()
        .map(|(row, value)| {
            *ids.entry(value).or_insert_with(|| {
                let id = entity_rows.len();
                entity_rows.push(row as u64);
                id
            })
        })
        .collect();
    let times = time_numbers(batch.column(time_index).as_ref())?;
    let mut unique_times = times.clone();
    unique_times.sort_unstable();
    unique_times.dedup();
    let time_id = times
        .iter()
        .map(|time| {
            unique_times
                .binary_search(time)
                .map_err(|_| PreparationError::TimeType)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value_indices = (0..batch.num_columns())
        .filter(|index| *index != entity_index && *index != time_index)
        .collect::<Vec<_>>();
    let columns = value_indices
        .iter()
        .map(|index| {
            if !batch.column(*index).data_type().is_numeric() {
                return Err(PreparationError::ValueType);
            }
            let values = arrow::compute::cast(batch.column(*index).as_ref(), &DataType::Float64)?;
            let values = values
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or(PreparationError::ValueType)?;
            Ok(values
                .iter()
                .map(|value| value.unwrap_or(f64::NAN))
                .collect())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PanelInput {
        panel: AlignedPanel {
            entity_id,
            time_id,
            columns,
        },
        entity_rows,
        unique_times,
        value_indices,
        entity_index,
        time_index,
    })
}

fn panel_batch(
    batch: &RecordBatch,
    input: &PanelInput,
    panel: &AlignedPanel,
) -> Result<RecordBatch, PreparationError> {
    let entities =
        UInt64Array::from_iter_values(panel.entity_id.iter().map(|id| input.entity_rows[*id]));
    let entity = arrow::compute::take(batch.column(input.entity_index).as_ref(), &entities, None)?;
    let times = panel
        .time_id
        .iter()
        .map(|time| {
            input
                .unique_times
                .get(*time)
                .copied()
                .ok_or(PreparationError::TimeType)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let time = time_array(times, batch.column(input.time_index).data_type())?;
    let mut arrays = vec![entity, time];
    let mut fields = vec![
        batch.schema().field(input.entity_index).clone(),
        batch.schema().field(input.time_index).clone(),
    ];
    for (index, column) in input.value_indices.iter().zip(&panel.columns) {
        arrays.push(Arc::new(Float64Array::from_iter(
            column
                .iter()
                .map(|value| (!value.is_nan()).then_some(*value)),
        )));
        fields.push(
            batch
                .schema()
                .field(*index)
                .clone()
                .with_data_type(DataType::Float64)
                .with_nullable(true),
        );
    }
    Ok(RecordBatch::try_new(
        Arc::new(Schema::new_with_metadata(
            fields,
            batch.schema().metadata().clone(),
        )),
        arrays,
    )?)
}

/// Align within each entity on the existing shared ordinal time grid.
pub fn align_batch(
    batch: &RecordBatch,
    entity: &str,
    time: &str,
    interval: Option<i64>,
) -> Result<RecordBatch, PreparationError> {
    let interval =
        usize::try_from(interval.unwrap_or(1)).map_err(|_| PreparationError::Interval)?;
    if interval == 0 {
        return Err(PreparationError::Interval);
    }
    let input = prepare_panel(batch, entity, time)?;
    let mut bounds: HashMap<usize, (usize, usize)> = HashMap::new();
    for (&entity, &time) in input.panel.entity_id.iter().zip(&input.panel.time_id) {
        bounds
            .entry(entity)
            .and_modify(|(lo, hi)| {
                *lo = (*lo).min(time);
                *hi = (*hi).max(time);
            })
            .or_insert((time, time));
    }
    let rows = bounds.values().try_fold(0usize, |rows, (lo, hi)| {
        rows.checked_add((hi - lo) / interval + 1)
            .ok_or(PreparationError::MemoryLimit)
    })?;
    check_size(
        rows,
        batch
            .num_columns()
            .checked_mul(24)
            .and_then(|width| width.checked_add(32))
            .ok_or(PreparationError::MemoryLimit)?,
    )?;
    let aligned = align_panel(
        &input.panel.entity_id,
        &input.panel.time_id,
        &input.panel.columns,
        Some(interval),
    )
    .map_err(|_| PreparationError::Panel)?;
    panel_batch(batch, &input, &aligned)
}

/// The existing panel difference routine owns the treatment of gaps and valid observations.
pub fn diff_batch(
    batch: &RecordBatch,
    entity: &str,
    time: &str,
) -> Result<RecordBatch, PreparationError> {
    let input = prepare_panel(batch, entity, time)?;
    let (entity_id, time_id, columns) =
        panel_diff(&input.panel).map_err(|_| PreparationError::Panel)?;
    panel_batch(
        batch,
        &input,
        &AlignedPanel {
            entity_id,
            time_id,
            columns,
        },
    )
}
