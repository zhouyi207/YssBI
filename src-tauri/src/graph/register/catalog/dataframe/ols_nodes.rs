//! OLS 回归节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::regression::linear_model::{OLSConfig, OLS};

use super::info_nodes::{Coefficient, DiagnosticInfo, ModelBasicInfo, OLSResult};

// ======================== OLS Configure 结构体 ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSConfigure {
    pub constant: bool,
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_ols_configure(registry);
    register_ols(registry);
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
        PinSlot::fixed(PinDefinition::data_input(
            "Constant",
            DataRole::Custom("constant".to_string()),
            PinDataTypeDefinition::concrete(DataType::Boolean),
        )),
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
    let definition = NodeDefinition::new(
        "OLS",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Ordinary Least Squares regression")
    .with_pin_slots(vec![
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
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            ),
            "Exog",
            1,
            None,
        ),
        PinSlot::fixed(PinDefinition::data_input(
            "Config",
            DataRole::Custom("ols_config".to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_flow_processor(Arc::new(|ctx| {
        // ---- Extract endog ----
        let endog_value = ctx.get_input_by_role(
            &PinRole::Data(DataRole::Custom("endog".to_string())),
        )?;
        let endog_id = match &endog_value {
            DataValue::DataSeries(v) => v.id.clone(),
            _ => return Err("OLS: Endog input is not a DataSeries".to_string()),
        };
        let endog_series = ctx.get_series(&endog_id)?;
        let endog_f64 = endog_series
            .f64()
            .map_err(|e| format!("OLS: cannot cast Endog to Float64: {}", e))?;
        let endog_values: Vec<f64> = endog_f64.into_no_null_iter().collect();
        let n = endog_values.len();
        let endog = Array1::from(endog_values);

        // ---- Extract exog (multiple series) ----
        let exog_data_values = ctx.get_inputs_by_family(
            &PinRole::Data(DataRole::Inputs(0)),
        )?;

        if exog_data_values.is_empty() {
            return Err("OLS: at least one Exog input is required".to_string());
        }

        let mut exog_columns: Vec<Vec<f64>> = Vec::new();
        let mut exog_names: Vec<String> = Vec::new();

        for (i, val) in exog_data_values.iter().enumerate() {
            let series_id = match val {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err(format!("OLS: Exog input {} is not a DataSeries", i)),
            };
            let series = ctx.get_series(&series_id)?;
            let name = series.name().to_string();
            exog_names.push(if name.is_empty() {
                format!("x{}", i + 1)
            } else {
                name
            });

            let f64_ca = series
                .f64()
                .map_err(|e| format!("OLS: cannot cast Exog {} to Float64: {}", i, e))?;
            let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
            if values.len() != n {
                return Err(format!(
                    "OLS: Exog {} has {} observations, expected {}",
                    i,
                    values.len(),
                    n
                ));
            }
            exog_columns.push(values);
        }

        // ---- Get config ----
        let config_value = ctx.get_input_by_role(
            &PinRole::Data(DataRole::Custom("ols_config".to_string())),
        )?;
        let config_handle_id = config_value
            .as_handle_id()
            .ok_or("OLS: config input is not a Struct handle")?
            .to_string();
        let config_handle = ctx.get_handle(&config_handle_id)?;
        let config = config_handle
            .downcast_ref::<OLSConfigure>()
            .ok_or("OLS: config handle is not an OLSConfigure")?;

        let has_constant = config.constant;

        // ---- Build exog matrix (optionally prepend intercept column) ----
        let k = if has_constant {
            exog_columns.len() + 1
        } else {
            exog_columns.len()
        };
        let mut exog_raw = Vec::with_capacity(n * k);

        let mut var_names = Vec::new();
        if has_constant {
            var_names.push("const".to_string());
        }
        var_names.extend(exog_names);

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
            endog,
            exog,
            config: sci_config,
        };
        let result = ols.fit();

        // ---- Build coefficient table ----
        let num_coeff = result.betas.len();
        let mut coefficients = Vec::with_capacity(num_coeff);
        for i in 0..num_coeff {
            let var_name = var_names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("x{}", i));
            coefficients.push(Coefficient {
                variable: var_name,
                coef: result.betas[i],
                std_err: result.stds[i],
                t_value: result.tvalues[i],
                p_value: result.pvalues[i],
                ci_lower: result.conf_int_left[i],
                ci_upper: result.conf_int_right[i],
                is_significant: result.pvalues[i] < 0.05,
            });
        }

        // ---- Build display result ----
        let ols_result = OLSResult {
            title: "OLS Regression Results".to_string(),
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
            },
        };

        let handle_id = ctx.put_handle(Box::new(ols_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", handle_id),
        )?;

        ctx.log("OLS: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
