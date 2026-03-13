//! Panel 对齐与差分
//!
//! 复用 TS align 思路：按 entity 分组，每组内补齐时间轴到规则网格，缺失为 NaN。
//! panel_diff 在 align 后的数据上对相邻行做一阶差分（仅当两侧均非 NaN 时输出）。

use std::collections::HashMap;

use polars::prelude::*;

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
        entity_times
            .entry(eid)
            .or_default()
            .push((tid, i));
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
pub fn panel_diff(aligned: &AlignedPanel) -> Result<(Vec<usize>, Vec<usize>, Vec<Vec<f64>>), String> {
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

/// 对齐面板 DataFrame：按 (entity, time) 补齐到规则时间网格，缺失为 null
///
/// 与 TS align 类似，但按 entity 分组，每组内补齐时间轴。
/// * `df` - 输入 DataFrame
/// * `entity_col` - 实体列名（Categorical、Int64 或 String）
/// * `time_col` - 时间列名（Int64 或 Date）
/// * `interval` - 时间步长，None 时自动推断
pub fn align_dataframe(
    df: &DataFrame,
    entity_col: &str,
    time_col: &str,
    interval: Option<i64>,
) -> Result<DataFrame, String> {
    let entity_series = df
        .column(entity_col)
        .map_err(|e| format!("XT Align: 列 '{}' 不存在: {}", entity_col, e))?
        .clone();
    let time_series = df
        .column(time_col)
        .map_err(|e| format!("XT Align: 列 '{}' 不存在: {}", time_col, e))?
        .clone()
        .take_materialized_series();

    let n = df.height();
    if n == 0 {
        return Err("XT Align: DataFrame 为空".to_string());
    }

    // 映射 entity 到 usize
    let (entity_id, entity_names): (Vec<usize>, Vec<String>) = {
        let s = entity_series.cast(&DataType::String).map_err(|e| e.to_string())?;
        let ca = s.str().map_err(|e| e.to_string())?;
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut idx_to_name: Vec<String> = Vec::new();
        let mut out = Vec::with_capacity(n);
        for opt in ca.into_iter() {
            let key = opt.ok_or("XT Align: entity 列含 null")?.to_string();
            let idx = *m.entry(key.clone()).or_insert_with(|| {
                let i = idx_to_name.len();
                idx_to_name.push(key);
                i
            });
            out.push(idx);
        }
        (out, idx_to_name)
    };

    // 映射 time 到 usize（sorted unique index）
    let time_id: Vec<usize> = {
        let dtype = time_series.dtype();
        match dtype {
            DataType::Int64 => {
                let ca = time_series.i64().map_err(|e| e.to_string())?;
                let values: Vec<i64> = ca
                    .into_iter()
                    .map(|o| o.ok_or("XT Align: time 列含 null"))
                    .collect::<Result<_, _>>()
                    .map_err(|e| e.to_string())?;
                let mut unique: Vec<i64> = values.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
                unique.sort_unstable();
                let m: HashMap<i64, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
                values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
            }
            DataType::Date => {
                let ca = time_series.date().map_err(|e| e.to_string())?;
                let physical = ca.physical();
                let values: Vec<i32> = physical
                    .into_iter()
                    .map(|o| o.ok_or("XT Align: time 列含 null"))
                    .collect::<Result<_, _>>()
                    .map_err(|e| e.to_string())?;
                let mut unique: Vec<i32> = values.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
                unique.sort_unstable();
                let m: HashMap<i32, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
                values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
            }
            _ => return Err(format!("XT Align: time 列需为 Int64 或 Date，当前为 {:?}", dtype)),
        }
    };

    let interval = interval.unwrap_or(1).max(1) as usize;

    // 收集所有数值列（排除 entity 和 time）
    let value_cols: Vec<String> = df
        .get_column_names()
        .iter()
        .filter(|&c| *c != entity_col && *c != time_col)
        .map(|s| s.to_string())
        .collect();

    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(value_cols.len());
    for col_name in &value_cols {
        let col = df.column(col_name).map_err(|e| e.to_string())?;
        let f64_col = col.cast(&DataType::Float64).map_err(|e| e.to_string())?;
        let vec: Vec<f64> = f64_col
            .f64()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|o| o.unwrap_or(f64::NAN))
            .collect();
        columns.push(vec);
    }

    let aligned = align_panel(&entity_id, &time_id, &columns, Some(interval))?;

    // 构建输出：entity 用原名，time 用原始值
    let time_dtype = time_series.dtype().clone();
    let unique_times: Vec<i64> = match time_dtype {
        DataType::Int64 => {
            let ca = time_series.i64().map_err(|e| e.to_string())?;
            let mut unique: Vec<i64> = ca.into_no_null_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
            unique.sort_unstable();
            unique
        }
        DataType::Date => {
            let ca = time_series.date().map_err(|e| e.to_string())?;
            let physical = ca.physical();
            let mut unique: Vec<i32> = physical.into_no_null_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
            unique.sort_unstable();
            unique.iter().map(|&v| v as i64).collect()
        }
        _ => return Err("XT Align: time 类型异常".to_string()),
    };
    let time_orig: Vec<i64> = aligned
        .time_id
        .iter()
        .map(|&i| unique_times.get(i).copied().unwrap_or(0))
        .collect();

    let entity_out: Vec<String> = aligned
        .entity_id
        .iter()
        .map(|&i| entity_names.get(i).cloned().unwrap_or_default())
        .collect();

    let mut out_cols: Vec<Column> = vec![
        Column::from(Series::from_iter(entity_out).with_name(entity_col.into())),
        Column::from(
            match time_dtype {
                DataType::Int64 => Series::from_iter(time_orig.iter().map(|&v| Some(v))).with_name(time_col.into()),
                DataType::Date => {
                    Int32Chunked::from_vec(
                        time_col.into(),
                        time_orig.iter().map(|&v| v as i32).collect::<Vec<_>>(),
                    )
                    .into_series()
                    .cast(&DataType::Date)
                    .map_err(|e| e.to_string())?
                }
                _ => return Err("XT Align: time 类型异常".to_string()),
            },
        ),
    ];

    for (c, col_name) in value_cols.iter().enumerate() {
        let vals = &aligned.columns[c];
        let s = Series::from_iter(vals.iter().map(|&v| if v.is_nan() { None } else { Some(v) }))
            .with_name(col_name.as_str().into());
        out_cols.push(Column::from(s));
    }

    DataFrame::new(aligned.entity_id.len(), out_cols).map_err(|e| format!("XT Align: {}", e))
}

/// 在 align 后的 DataFrame 上按 entity 做一阶差分，输出新 DataFrame
///
/// 与 Stata D. 算子一致。仅保留有有效差分的行。
pub fn diff_dataframe(
    aligned_df: &DataFrame,
    entity_col: &str,
    time_col: &str,
) -> Result<DataFrame, String> {
    let entity_series = aligned_df
        .column(entity_col)
        .map_err(|e| format!("XT Diff: {}", e))?
        .clone();
    let time_series = aligned_df
        .column(time_col)
        .map_err(|e| format!("XT Diff: {}", e))?
        .clone();
    let n = aligned_df.height();

    let entity_id: Vec<usize> = {
        let s = entity_series.cast(&DataType::String).map_err(|e| e.to_string())?;
        let ca = s.str().map_err(|e| e.to_string())?;
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut idx = 0usize;
        let mut out = Vec::with_capacity(n);
        for opt in ca.into_iter() {
            let key = opt.ok_or("XT Diff: entity 含 null")?.to_string();
            let i = *m.entry(key).or_insert_with(|| {
                let i = idx;
                idx += 1;
                i
            });
            out.push(i);
        }
        out
    };

    let time_id: Vec<usize> = {
        let dtype = time_series.dtype();
        match dtype {
            DataType::Int64 => {
                let ca = time_series.i64().map_err(|e| e.to_string())?;
                let values: Vec<i64> = ca.into_iter().map(|o| o.unwrap_or(0)).collect();
                let mut unique: Vec<i64> = values.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
                unique.sort_unstable();
                let m: HashMap<i64, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
                values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
            }
            DataType::Date => {
                let ca = time_series.date().map_err(|e| e.to_string())?;
                let physical = ca.physical();
                let values: Vec<i32> = physical.into_iter().map(|o| o.unwrap_or(0)).collect();
                let mut unique: Vec<i32> = values.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
                unique.sort_unstable();
                let m: HashMap<i32, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
                values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
            }
            _ => return Err(format!("XT Diff: time 需为 Int64 或 Date")),
        }
    };

    let value_cols: Vec<String> = aligned_df
        .get_column_names()
        .iter()
        .filter(|&c| *c != entity_col && *c != time_col)
        .map(|s| s.to_string())
        .collect();

    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(value_cols.len());
    for col_name in &value_cols {
        let col = aligned_df.column(col_name).map_err(|e| e.to_string())?;
        let f64_col = col.cast(&DataType::Float64).map_err(|e| e.to_string())?;
        let vec: Vec<f64> = f64_col
            .f64()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|o| o.unwrap_or(f64::NAN))
            .collect();
        columns.push(vec);
    }

    let aligned = AlignedPanel {
        entity_id: entity_id.clone(),
        time_id,
        columns,
    };

    let (diff_entity, diff_time_id, diff_cols) = panel_diff(&aligned)?;

    let n_fd = diff_entity.len();
    let entity_names: Vec<String> = {
        let s = entity_series.cast(&DataType::String).map_err(|e| e.to_string())?;
        let ca = s.str().map_err(|e| e.to_string())?;
        let mut seen: HashMap<String, ()> = HashMap::new();
        let mut idx_to_name: Vec<String> = Vec::new();
        for opt in ca.into_iter() {
            let s = opt.ok_or("")?.to_string();
            if !seen.contains_key(&s) {
                seen.insert(s.clone(), ());
                idx_to_name.push(s);
            }
        }
        idx_to_name
    };

    let entity_out: Vec<String> = diff_entity
        .iter()
        .map(|&i| entity_names.get(i).cloned().unwrap_or_default())
        .collect();

    let time_dtype = time_series.dtype().clone();
    let time_col_series = aligned_df.column(time_col).map_err(|e| e.to_string())?.clone();
    let unique_times: Vec<i64> = match time_dtype {
        DataType::Int64 => {
            let ca = time_col_series.i64().map_err(|e| e.to_string())?;
            let mut unique: Vec<i64> = ca
                .into_iter()
                .filter_map(|o| o)
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            unique.sort_unstable();
            unique
        }
        DataType::Date => {
            let ca = time_col_series.date().map_err(|e| e.to_string())?;
            let physical = ca.physical();
            let mut unique: Vec<i32> = physical
                .into_iter()
                .filter_map(|o| o)
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            unique.sort_unstable();
            unique.iter().map(|&v| v as i64).collect()
        }
        _ => return Err("XT Diff: 暂仅支持 Int64 或 Date 时间".to_string()),
    };

    let time_out_series = match time_dtype {
        DataType::Int64 => {
            let time_out: Vec<i64> = diff_time_id
                .iter()
                .map(|&idx| unique_times.get(idx).copied().unwrap_or(0))
                .collect();
            Series::from_iter(time_out.iter().map(|&v| Some(v))).with_name(time_col.into())
        }
        DataType::Date => {
            let time_out: Vec<i32> = diff_time_id
                .iter()
                .map(|&idx| unique_times.get(idx).copied().unwrap_or(0) as i32)
                .collect();
            Int32Chunked::from_vec(time_col.into(), time_out)
                .into_series()
                .cast(&DataType::Date)
                .map_err(|e| e.to_string())?
        }
        _ => return Err("XT Diff: 暂仅支持 Int64 或 Date 时间".to_string()),
    };

    let mut out_cols: Vec<Column> = vec![
        Column::from(Series::from_iter(entity_out).with_name(entity_col.into())),
        Column::from(time_out_series),
    ];
    for (c, col_name) in value_cols.iter().enumerate() {
        let vals = &diff_cols[c];
        let s = Series::from_iter(vals.iter().cloned()).with_name(col_name.as_str().into());
        out_cols.push(Column::from(s));
    }

    DataFrame::new(n_fd, out_cols).map_err(|e| format!("XT Diff: {}", e))
}
