//! Logit (binary logistic regression) nodes

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{CategoricalRole, DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, Series};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::regression::discrete::{Logit, LogitConfig};

use super::info_nodes::{compute_classification_table, Coefficient, DiagnosticInfo, ModelBasicInfo, OLSResult};
use super::ols_nodes::VariableSpec;

// ======================== 结构体 ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitConfigure {
    pub constant: bool,
}

impl Default for LogitConfigure {
    fn default() -> Self {
        Self { constant: true }
    }
}

/// Logit model for prediction (same structure as OLSModel)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
}

struct LogitFitResult {
    logit_result: OLSResult,
    logit_model: LogitModel,
}

// ======================== 共享辅助函数 ========================

fn logit_input_slots() -> Vec<PinSlot> {
    let exog_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Float64,
        DataType::Categorical,
    ])));

    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                DataType::Float64,
                DataType::Int64,
                DataType::Boolean,
            ])))),
        )),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(exog_type),
            ),
            "X",
            1,
            None,
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Time",
                DataRole::Custom("time".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                    DataType::Date,
                    DataType::Int64,
                ])))),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Config",
                DataRole::Custom("logit_config".to_string()),
                PinDataTypeDefinition::concrete(DataType::Struct("LogitConfigure".to_string())),
            )
            .with_optional(true),
        ),
    ]
}

fn run_logit_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<LogitFitResult, String> {
    // ---- Extract endog ----
    let endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        other => {
            let got = match other {
                DataValue::Null => "Null (unconnected or upstream not executed)",
                DataValue::Boolean(_) => "Boolean",
                DataValue::Int32(_) => "Int32",
                DataValue::Int64(_) => "Int64",
                DataValue::Float32(_) => "Float32",
                DataValue::Float64(_) => "Float64",
                _ => "other",
            };
            return Err(format!(
                "Logit: Y input is not a DataSeries (got {}). Ensure Y is connected to a DataSeries output.",
                got
            ));
        }
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() {
            "y".to_string()
        } else {
            raw
        }
    };
    let endog_f64_series = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("Logit: cannot cast Y to Float64: {}", e))?;

    // ---- Get config ----
    let config = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("logit_config".to_string()))) {
        Ok(config_value) => match config_value.as_handle_id() {
            Some(id) => {
                let handle = ctx.get_handle(&id.to_string())?;
                handle
                    .downcast_ref::<LogitConfigure>()
                    .ok_or("Logit: config handle is not a LogitConfigure")?
                    .clone()
            }
            None => LogitConfigure::default(),
        },
        Err(_) => LogitConfigure::default(),
    };
    let has_constant = config.constant;

    // ---- Extract exog (same as OLS) ----
    let exog_data_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;

    if exog_data_values.is_empty() {
        return Err("Logit: at least one X input is required".to_string());
    }

    let time_series = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string()))) {
        Ok(DataValue::DataSeries(v)) => {
            let ts = ctx.get_series(&v.id)?;
            if ts.len() != endog_f64_series.len() {
                return Err(format!(
                    "Logit: Time has {} observations, expected {}",
                    ts.len(),
                    endog_f64_series.len()
                ));
            }
            Some(ts)
        }
        _ => None,
    };

    let n_raw = endog_f64_series.len();
    let mut df_cols: Vec<Column> = vec![Column::from(endog_f64_series.with_name("__endog__".into()))];
    if let Some(ref ts) = time_series {
        df_cols.push(Column::from(ts.clone().with_name("__time__".into())));
    }
    let mut exog_meta: Vec<(String, bool, DataSeriesValue)> = Vec::new();
    for (i, val) in exog_data_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("Logit: X input {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
        let series_name = {
            let raw = series.name().to_string();
            if raw.is_empty() {
                format!("x{}", i + 1)
            } else {
                raw
            }
        };
        if series.len() != n_raw {
            return Err(format!(
                "Logit: X '{}' has {} observations, expected {}",
                series_name,
                series.len(),
                n_raw
            ));
        }
        let is_categorical = matches!(
            series.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        );
        let col_series = if is_categorical {
            series
                .cast(&polars::prelude::DataType::String)
                .map_err(|e| format!("Logit: cannot cast X {} to String: {}", i, e))?
        } else {
            series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Logit: cannot cast X {} to Float64: {}", i, e))?
        };
        df_cols.push(Column::from(col_series.with_name(series_name.as_str().into())));
        exog_meta.push((series_name, is_categorical, dsv));
    }
    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("Logit: failed to build DataFrame: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("Logit: drop_nulls failed: {}", e))?;
    let n = df.height();
    if n == 0 {
        return Err("Logit: no valid observations after dropping null/NaN".to_string());
    }

    let endog_vec: Vec<f64> = df
        .column("__endog__")
        .map_err(|e| format!("Logit: {}", e))?
        .f64()
        .map_err(|e| format!("Logit: {}", e))?
        .into_no_null_iter()
        .collect();

    // Validate binary 0/1
    for (i, &yi) in endog_vec.iter().enumerate() {
        if yi != 0.0 && yi != 1.0 {
            return Err(format!(
                "Logit: Y must be 0/1 (binary). Got {} at observation {}",
                yi,
                i + 1
            ));
        }
    }

    let endog = Array1::from(endog_vec);

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut col_labels: Vec<(String, Option<String>)> = Vec::new();
    let mut variable_specs: Vec<VariableSpec> = Vec::new();

    for (series_name, is_categorical, dsv) in exog_meta {
        let col = df.column(&series_name).map_err(|e| format!("Logit: {}", e))?;

        if is_categorical {
            let str_ca = col.str().map_err(|e| format!("Logit: X '{}': {}", series_name, e))?;
            let values: Vec<String> = str_ca.into_no_null_iter().map(|s: &str| s.to_string()).collect();
            let mut unique_ordered: Vec<String> = Vec::new();
            for v in &values {
                if !unique_ordered.contains(v) {
                    unique_ordered.push(v.clone());
                }
            }
            if unique_ordered.len() < 2 {
                return Err(format!(
                    "Logit: categorical '{}' must have at least 2 unique values",
                    series_name
                ));
            }
            let dummy_info = dsv.dummy_info.as_ref();
            let role = dummy_info
                .map(|di| di.role.clone())
                .unwrap_or(CategoricalRole::General);
            let drop_cat = if let Some(di) = dummy_info {
                if let Some(ref specified) = di.drop_category {
                    if !unique_ordered.contains(specified) {
                        return Err(format!(
                            "Logit: drop_category '{}' not found in '{}'",
                            specified, series_name
                        ));
                    }
                    specified.clone()
                } else {
                    unique_ordered[0].clone()
                }
            } else if has_constant {
                unique_ordered[0].clone()
            } else {
                String::new()
            };
            let categories_to_include: Vec<&String> = if drop_cat.is_empty() {
                unique_ordered.iter().collect()
            } else {
                unique_ordered.iter().filter(|c| **c != drop_cat).collect()
            };
            for cat in &categories_to_include {
                let col: Vec<f64> = values.iter().map(|v| if v == *cat { 1.0 } else { 0.0 }).collect();
                exog_columns.push(col);
                col_labels.push((series_name.clone(), Some((*cat).clone())));
            }
            variable_specs.push(VariableSpec::Categorical {
                name: series_name,
                categories: unique_ordered,
                dropped: if drop_cat.is_empty() {
                    String::new()
                } else {
                    drop_cat
                },
                role,
            });
        } else {
            let values: Vec<f64> = col
                .f64()
                .map_err(|e| format!("Logit: X '{}': {}", series_name, e))?
                .into_no_null_iter()
                .collect();
            exog_columns.push(values);
            col_labels.push((series_name.clone(), None));
            variable_specs.push(VariableSpec::Numeric {
                name: series_name,
            });
        }
    }

    let k = if has_constant {
        exog_columns.len() + 1
    } else {
        exog_columns.len()
    };
    let mut exog_raw = Vec::with_capacity(n * k);

    let mut all_labels: Vec<(String, Option<String>)> = Vec::new();
    if has_constant {
        all_labels.push(("const".to_string(), None));
    }
    all_labels.extend(col_labels);

    for i in 0..n {
        if has_constant {
            exog_raw.push(1.0);
        }
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }

    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("Logit: failed to build exog matrix: {}", e))?;

    // ---- Run Logit ----
    let sci_config = LogitConfig {
        constant: has_constant,
    };
    let logit = Logit {
        endog: endog.clone(),
        exog: exog.clone(),
        config: sci_config,
    };
    let result = logit.fit()?;

    // Fitted = probabilities
    let fitted_values: Vec<f64> = (0..n)
        .map(|i| {
            let eta: f64 = exog.row(i).iter().zip(result.betas.iter()).map(|(x, b)| x * b).sum();
            1.0 / (1.0 + (-eta).exp())
        })
        .collect();
    let residuals: Vec<f64> = endog
        .iter()
        .zip(fitted_values.iter())
        .map(|(y, p)| y - p)
        .collect();

    let num_coeff = result.betas.len();
    let mut coefficients = Vec::with_capacity(num_coeff);
    for i in 0..num_coeff {
        let (var_name, category) = all_labels
            .get(i)
            .cloned()
            .unwrap_or_else(|| (format!("x{}", i), None));
        coefficients.push(Coefficient {
            variable: var_name,
            category,
            coef: result.betas[i],
            std_err: result.stds[i],
            t_value: result.zvalues[i],
            p_value: result.pvalues[i],
            ci_lower: result.conf_int_left[i],
            ci_upper: result.conf_int_right[i],
            is_significant: result.pvalues[i] < 0.05,
        });
    }

    let df_model = if has_constant { k - 1 } else { k };
    let df_residual = n - k;
    let df_total = n;

    let classification_table = compute_classification_table(
        endog.as_slice().unwrap_or(&[]),
        &fitted_values,
        0.5,
    );

    // exog_means for margins at means; exog for average marginal effects
    let exog_means: Vec<f64> = (0..k)
        .map(|j| exog.column(j).iter().sum::<f64>() / n as f64)
        .collect();
    let exog_vec: Vec<Vec<f64>> = (0..n)
        .map(|i| exog.row(i).iter().cloned().collect())
        .collect();

    let logit_result = OLSResult {
        title: "Logit Regression Results".to_string(),
        endog_name,
        model_basic_info: ModelBasicInfo {
            model_type: "Logit".to_string(),
            method: "IRLS".to_string(),
            num_observation: result.num_observation,
            r_squared: result.pseudo_r2,
            adj_r_squared: result.pseudo_r2,
            f_statistic: result.lr_chi2,
            prob_f_statistic: result.lr_p_value,
            wald_chi2: Some(result.lr_chi2),
            prob_wald_chi2: Some(result.lr_p_value),
            df_model,
            df_residual,
            df_total,
            ss_model: 0.0,
            ss_residual: 0.0,
            ss_total: 0.0,
            ms_model: 0.0,
            ms_residual: 0.0,
            ms_total: 0.0,
            covariance_type: "nonrobust".to_string(),
            aic: result.aic,
            bic: result.bic,
        },
        coefficients,
        diagnostic_info: DiagnosticInfo {
            cond_no: 0.0,
            vif: None,
            bp_tests: None,
            ov_tests: None,
            im_test: None,
            normality_tests: None,
            fitted_values,
            residuals,
            leverage: vec![],
            residual_scatter: None,
            exog: Some(exog_vec),
            timing: None,
            prais_info: None,
            iv2sls_first_stage: None,
            iv2sls_first_stage_summary: None,
            iv2sls_overid: None,
            iv2sls_overid_dims: None,
            iv2sls_hausman: None,
            iv2sls_endogenous: None,
            ivliml_kappa: None,
            ivliml_overid: None,
            classification_table: Some(classification_table),
            exog_means: Some(exog_means),
        },
        betas: result.betas.to_vec(),
        cov_beta: (0..result.cov_beta.nrows())
            .map(|i| result.cov_beta.row(i).iter().cloned().collect())
            .collect(),
    };

    let logit_model = LogitModel {
        betas: result.betas.to_vec(),
        has_constant,
        variable_specs,
    };

    Ok(LogitFitResult {
        logit_result,
        logit_model,
    })
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_logit_configure(registry);
    register_logit(registry);
    register_logit_summary(registry);
}

fn register_logit_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Logit Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Logit regression configuration — Constant term")
    .with_pin_slots(vec![
        PinSlot::fixed(
            PinDefinition::data_input(
                "Constant",
                DataRole::Custom("constant".to_string()),
                PinDataTypeDefinition::concrete(DataType::Boolean),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("LogitConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let constant = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let config = LogitConfigure { constant };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("LogitConfigure", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_logit(registry: &NodeRegistry) {
    let mut slots = logit_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Model",
        DataRole::Custom("logit_model".to_string()),
        PinDataTypeDefinition::concrete(DataType::Struct("LogitModel".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Fitted",
        DataRole::Custom("logit_fitted".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Residuals",
        DataRole::Custom("logit_residuals".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "Logit",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Binary logistic regression (IRLS) — outputs fitted model for prediction")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_logit_regression(ctx)?;

        let model_handle_id = ctx.put_handle(Box::new(fit.logit_model));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("logit_model".to_string())),
            DataValue::new_struct("LogitModel", model_handle_id),
        )?;

        let fitted_series = Series::from_iter(
            fit.logit_result.diagnostic_info.fitted_values.into_iter(),
        )
        .with_name("fitted".into());
        let fitted_id = ctx.put_series(fitted_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("logit_fitted".to_string())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(fitted_id, DataType::Float64)),
        )?;

        let residuals_series = Series::from_iter(
            fit.logit_result.diagnostic_info.residuals.into_iter(),
        )
        .with_name("residuals".into());
        let residuals_id = ctx.put_series(residuals_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("logit_residuals".to_string())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(residuals_id, DataType::Float64)),
        )?;

        ctx.log("Logit: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}

fn register_logit_summary(registry: &NodeRegistry) {
    let mut slots = logit_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "Logit Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Binary logistic regression — outputs results and opens summary window")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_logit_regression(ctx)?;

        let json_data = serde_json::to_string(&fit.logit_result)
            .map_err(|e| format!("Logit Summary: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(fit.logit_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", result_handle_id),
        )?;

        ctx.open_window("ols_summary".to_string(), json_data);

        ctx.log("Logit Summary: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
