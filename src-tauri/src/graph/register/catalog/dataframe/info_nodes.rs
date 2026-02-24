//! 信息展示节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSResult {
    pub title: String,
    pub model_basic_info: ModelBasicInfo,
    pub coefficients: Vec<Coefficient>,
    pub diagnostic_info: DiagnosticInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBasicInfo {
    pub dep_variable: String,
    pub r_squared: f64,
    pub model_type: String,
    pub adj_r_squared: f64,
    pub method: String,
    pub f_statistic: f64,
    pub date: String,
    pub prob_f_statistic: f64,
    pub time: String,
    pub log_likelihood: f64,
    pub no_observations: u32,
    pub aic: f64,
    pub df_residuals: u32,
    pub bic: f64,
    pub df_model: u32,
    pub covariance_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coefficient {
    pub variable: String,
    pub coef: f64,
    pub std_err: f64,
    pub t_value: f64,
    pub p_value: f64,
    #[serde(rename = "confidence_interval_0.025")]
    pub ci_lower: f64,
    #[serde(rename = "confidence_interval_0.975")]
    pub ci_upper: f64,
    pub is_significant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub omnibus: f64,
    pub durbin_watson: f64,
    pub prob_omnibus: f64,
    #[serde(rename = "jarque_bera_jb")]
    pub jarque_bera: f64,
    pub skew: f64,
    pub prob_jb: f64,
    pub kurtosis: f64,
    pub cond_no: String,
}

pub fn register(registry: &NodeRegistry) {
    register_ols_result_constant(registry);
    register_ols_summary(registry);
}

fn build_sample_ols_result() -> OLSResult {
    OLSResult {
        title: "OLS Regression Results".to_string(),
        model_basic_info: ModelBasicInfo {
            dep_variable: "EXECUTIONS".to_string(),
            r_squared: 0.664,
            model_type: "OLS".to_string(),
            adj_r_squared: 0.462,
            method: "Least Squares".to_string(),
            f_statistic: 3.292,
            date: "Tue, 24 Feb 2026".to_string(),
            prob_f_statistic: 0.0469,
            time: "18:36:15".to_string(),
            log_likelihood: -51.108,
            no_observations: 17,
            aic: 116.2,
            df_residuals: 10,
            bic: 122.0,
            df_model: 6,
            covariance_type: "nonrobust".to_string(),
        },
        coefficients: vec![
            Coefficient {
                variable: "INCOME".to_string(),
                coef: 0.0022, std_err: 0.001, t_value: 3.348, p_value: 0.007,
                ci_lower: 0.001, ci_upper: 0.004, is_significant: true,
            },
            Coefficient {
                variable: "PERPOVERTY".to_string(),
                coef: 0.9077, std_err: 0.726, t_value: 1.250, p_value: 0.240,
                ci_lower: -0.710, ci_upper: 2.525, is_significant: false,
            },
            Coefficient {
                variable: "PERBLACK".to_string(),
                coef: -0.8879, std_err: 0.307, t_value: -2.894, p_value: 0.016,
                ci_lower: -1.572, ci_upper: -0.204, is_significant: true,
            },
            Coefficient {
                variable: "VC100k96".to_string(),
                coef: 0.0095, std_err: 0.009, t_value: 1.025, p_value: 0.329,
                ci_lower: -0.011, ci_upper: 0.030, is_significant: false,
            },
            Coefficient {
                variable: "SOUTH".to_string(),
                coef: 15.6562, std_err: 5.129, t_value: 3.053, p_value: 0.012,
                ci_lower: 4.228, ci_upper: 27.084, is_significant: true,
            },
            Coefficient {
                variable: "DEGREE".to_string(),
                coef: -160.6786, std_err: 55.825, t_value: -2.878, p_value: 0.016,
                ci_lower: -285.065, ci_upper: -36.292, is_significant: true,
            },
            Coefficient {
                variable: "const".to_string(),
                coef: -49.7374, std_err: 25.764, t_value: -1.931, p_value: 0.082,
                ci_lower: -107.143, ci_upper: 7.668, is_significant: false,
            },
        ],
        diagnostic_info: DiagnosticInfo {
            omnibus: 1.029,
            durbin_watson: 1.307,
            prob_omnibus: 0.598,
            jarque_bera: 0.829,
            skew: 0.488,
            prob_jb: 0.661,
            kurtosis: 2.532,
            cond_no: "1.29e+06".to_string(),
        },
    }
}

fn register_ols_result_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS Result",
        vec!["Value".to_string(), "Constants".to_string()],
    )
    .with_ui_style("value")
    .with_description("Constant OLS regression result (sample data)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let ols_result = build_sample_ols_result();
        let handle_id = ctx.put_handle(Box::new(ols_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_ols_summary(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Display OLS regression results in a new window")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "OLS Result",
            DataRole::Custom("ols_result".to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_flow_processor(Arc::new(|ctx| {
        let input_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("ols_result".to_string())))?;

        let handle_id = input_value
            .as_handle_id()
            .ok_or("OLS Summary: input is not a Struct handle")?
            .to_string();

        let handle = ctx.get_handle(&handle_id)?;
        let ols_result = handle
            .downcast_ref::<OLSResult>()
            .ok_or("OLS Summary: handle is not an OLSResult")?;

        let json_data = serde_json::to_string(ols_result)
            .map_err(|e| format!("OLS Summary: failed to serialize: {}", e))?;

        ctx.open_window("ols_summary".to_string(), json_data);

        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
