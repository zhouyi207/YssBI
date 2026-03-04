//! OLS 回归节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{CategoricalRole, DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::Series;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::regression::linear_model::{OLSConfig, OLS};

use super::info_nodes::{Coefficient, DiagnosticInfo, ModelBasicInfo, OLSResult};

// ======================== 结构体 ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSConfigure {
    pub constant: bool,
}

impl Default for OLSConfigure {
    fn default() -> Self {
        Self { constant: true }
    }
}

/// 变量规格：记录每个输入变量在拟合时的处理方式（供预测复用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VariableSpec {
    Numeric {
        name: String,
    },
    Categorical {
        name: String,
        /// 拟合时的所有 unique 值（有序）
        categories: Vec<String>,
        /// 被剔除的参考类别
        dropped: String,
        /// 语义角色
        role: CategoricalRole,
    },
}

/// OLS 模型（拟合产物，携带完整的变量编码规格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
}

/// OLS 回归拟合结果（内部 helper 返回值）
struct OLSFitResult {
    ols_result: OLSResult,
    ols_model: OLSModel,
}

// ======================== 共享辅助函数 ========================

fn ols_input_slots() -> Vec<PinSlot> {
    let exog_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Float64,
        DataType::String,
    ])));

    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Endog",
            DataRole::Custom("endog".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(exog_type),
            ),
            "Exog",
            1,
            None,
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Config",
                DataRole::Custom("ols_config".to_string()),
                PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
            )
            .with_optional(true),
        ),
    ]
}

use crate::execution::context::NodeExecutionContextTrait;

fn run_ols_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<OLSFitResult, String> {
    // ---- Extract endog ----
    let endog_value = ctx.get_input_by_role(
        &PinRole::Data(DataRole::Custom("endog".to_string())),
    )?;
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
                DataValue::String(_) => "String",
                DataValue::Array(_) => "Array",
                DataValue::Object(_) => "Object",
                DataValue::DataFrame(_) => "DataFrame",
                DataValue::Struct { type_key, .. } => {
                    return Err(format!(
                        "OLS: Endog input is not a DataSeries (got Struct<{}>). Check that Endog is connected to Add/DataSeries output, not Config.",
                        type_key
                    ));
                }
                DataValue::DataSeries(_) => unreachable!(),
            };
            return Err(format!(
                "OLS: Endog input is not a DataSeries (got {}). Ensure Endog is connected to a DataSeries output (e.g. Add result).",
                got
            ));
        }
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .f64()
        .map_err(|e| format!("OLS: cannot cast Endog to Float64: {}", e))?;
    let endog_values: Vec<f64> = endog_f64.into_no_null_iter().collect();
    let n = endog_values.len();
    let endog = Array1::from(endog_values);

    // ---- Get config (optional — falls back to OLSConfigure::default()) ----
    let config = match ctx.get_input_by_role(
        &PinRole::Data(DataRole::Custom("ols_config".to_string())),
    ) {
        Ok(config_value) => match config_value.as_handle_id() {
            Some(id) => {
                let handle = ctx.get_handle(&id.to_string())?;
                handle
                    .downcast_ref::<OLSConfigure>()
                    .ok_or("OLS: config handle is not an OLSConfigure")?
                    .clone()
            }
            None => OLSConfigure::default(),
        },
        Err(_) => OLSConfigure::default(),
    };
    let has_constant = config.constant;

    // ---- Extract exog (mixed numeric + categorical) ----
    let exog_data_values = ctx.get_inputs_by_family(
        &PinRole::Data(DataRole::Inputs(0)),
    )?;

    if exog_data_values.is_empty() {
        return Err("OLS: at least one Exog input is required".to_string());
    }

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut col_labels: Vec<(String, Option<String>)> = Vec::new();
    let mut variable_specs: Vec<VariableSpec> = Vec::new();

    for (i, val) in exog_data_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v,
            _ => return Err(format!("OLS: Exog input {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
        let series_name = {
            let raw = series.name().to_string();
            if raw.is_empty() { format!("x{}", i + 1) } else { raw }
        };

        let is_string = series.dtype() == &polars::prelude::DataType::String;

        if is_string {
            let str_ca = series.str().map_err(|e| {
                format!("OLS: cannot cast Exog {} to String: {}", i, e)
            })?;

            let mut unique_ordered: Vec<String> = Vec::new();
            for opt_val in str_ca.into_iter() {
                let val_str = opt_val
                    .ok_or_else(|| format!("OLS: Exog {} contains null values", i))?
                    .to_string();
                if !unique_ordered.contains(&val_str) {
                    unique_ordered.push(val_str);
                }
            }

            if unique_ordered.len() < 2 {
                return Err(format!(
                    "OLS: categorical variable '{}' must have at least 2 unique values",
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
                            "OLS: drop_category '{}' not found in unique values of '{}'",
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

            let values: Vec<String> = str_ca
                .into_iter()
                .map(|opt| opt.unwrap().to_string())
                .collect();

            if values.len() != n {
                return Err(format!(
                    "OLS: Exog '{}' has {} observations, expected {}",
                    series_name, values.len(), n
                ));
            }

            for cat in &categories_to_include {
                let col: Vec<f64> = values.iter().map(|v| if v == *cat { 1.0 } else { 0.0 }).collect();
                exog_columns.push(col);
                col_labels.push((series_name.clone(), Some((*cat).clone())));
            }

            variable_specs.push(VariableSpec::Categorical {
                name: series_name,
                categories: unique_ordered,
                dropped: if drop_cat.is_empty() { String::new() } else { drop_cat },
                role,
            });
        } else {
            let f64_ca = series.f64().map_err(|e| {
                format!("OLS: cannot cast Exog {} to Float64: {}", i, e)
            })?;
            let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
            if values.len() != n {
                return Err(format!(
                    "OLS: Exog '{}' has {} observations, expected {}",
                    series_name, values.len(), n
                ));
            }
            exog_columns.push(values);
            col_labels.push((series_name.clone(), None));
            variable_specs.push(VariableSpec::Numeric { name: series_name });
        }
    }

    // ---- Build exog matrix ----
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
        .map_err(|e| format!("OLS: failed to build exog matrix: {}", e))?;

    // ---- Run OLS regression ----
    let sci_config = OLSConfig { constant: has_constant };
    let ols = OLS {
        endog: endog.clone(),
        exog: exog.clone(),
        config: sci_config,
    };
    let result = ols.fit()?;

    // ---- Compute fitted values & residuals ----
    let fitted_values: Vec<f64> = (0..n)
        .map(|i| {
            exog.row(i)
                .iter()
                .zip(result.betas.iter())
                .map(|(x, b)| x * b)
                .sum()
        })
        .collect();
    let residuals: Vec<f64> = endog
        .iter()
        .zip(fitted_values.iter())
        .map(|(y, yhat)| y - yhat)
        .collect();

    // ---- Build coefficient table ----
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
            t_value: result.tvalues[i],
            p_value: result.pvalues[i],
            ci_lower: result.conf_int_left[i],
            ci_upper: result.conf_int_right[i],
            is_significant: result.pvalues[i] < 0.05,
        });
    }

    let ols_result = OLSResult {
        title: "OLS Regression Results".to_string(),
        endog_name: endog_name,
        model_basic_info: ModelBasicInfo {
            model_type: "OLS".to_string(),
            method: "Least Squares".to_string(),
            num_observation: result.num_observation,
            r_squared: result.r2,
            adj_r_squared: result.r2_adjusted,
            f_statistic: result.fvalue,
            prob_f_statistic: result.f_p_value,
            df_model: result.df_model,
            df_residual: result.df_residual,
            df_total: result.df_total,
            ss_model: result.ss_model,
            ss_residual: result.ss_residual,
            ss_total: result.ss_total,
            ms_model: result.ms_model,
            ms_residual: result.ms_residual,
            ms_total: result.ms_total,
            covariance_type: result.covariance_type.to_string(),
        },
        coefficients,
        diagnostic_info: DiagnosticInfo {
            cond_no: result.cond_no,
            fitted_values,
            residuals,
        },
        betas: result.betas.to_vec(),
        cov_beta: (0..result.cov_beta.nrows())
            .map(|i| result.cov_beta.row(i).iter().cloned().collect())
            .collect(),
    };

    let ols_model = OLSModel {
        betas: result.betas.to_vec(),
        has_constant,
        variable_specs,
    };

    Ok(OLSFitResult { ols_result, ols_model })
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_ols_configure(registry);
    register_ols(registry);
    register_ols_summary(registry);
}

// ======================== OLS Configure 节点 ========================

fn register_ols_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("OLS regression configuration — input pins compose the output Config")
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
            PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let constant = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))?
            .as_bool()
            .ok_or("OLS Configure: Constant must be a boolean")?;

        let config = OLSConfigure { constant };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSConfigure", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== OLS 节点 ========================

fn register_ols(registry: &NodeRegistry) {
    let mut slots = ols_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Model",
        DataRole::Custom("ols_model".to_string()),
        PinDataTypeDefinition::concrete(DataType::Struct("OLSModel".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Fitted",
        DataRole::Custom("ols_fitted".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Residuals",
        DataRole::Custom("ols_residuals".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "OLS",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Ordinary Least Squares regression — outputs the fitted model for prediction")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_ols_regression(ctx)?;

        let model_handle_id = ctx.put_handle(Box::new(fit.ols_model));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("ols_model".to_string())),
            DataValue::new_struct("OLSModel", model_handle_id),
        )?;

        let fitted_series = Series::from_iter(fit.ols_result.diagnostic_info.fitted_values.into_iter())
            .with_name("fitted".into());
        let fitted_id = ctx.put_series(fitted_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("ols_fitted".to_string())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(fitted_id, DataType::Float64)),
        )?;

        let residuals_series = Series::from_iter(fit.ols_result.diagnostic_info.residuals.into_iter())
            .with_name("residuals".into());
        let residuals_id = ctx.put_series(residuals_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("ols_residuals".to_string())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(residuals_id, DataType::Float64)),
        )?;

        ctx.log("OLS: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}

// ======================== OLS Summary 节点 ========================

fn register_ols_summary(registry: &NodeRegistry) {
    let mut slots = ols_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "OLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Ordinary Least Squares regression — outputs results and opens the summary window")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_ols_regression(ctx)?;

        let json_data = serde_json::to_string(&fit.ols_result)
            .map_err(|e| format!("OLS Summary: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(fit.ols_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", result_handle_id),
        )?;

        ctx.open_window("ols_summary".to_string(), json_data);

        ctx.log("OLS Summary: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
