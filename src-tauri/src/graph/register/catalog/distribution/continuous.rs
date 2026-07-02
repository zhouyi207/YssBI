//! 连续分布节点：Normal, Uniform, Exp, Gamma, Beta, StudentsT, Cauchy, ChiSquared, LogNormal, Weibull, Laplace, Pareto, Gumbel, InverseGamma, Triangular, FisherSnedecor, Erlang

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::Series;
use rand::SeedableRng;
use rand::distributions::Distribution;
use rand::rngs::StdRng;
use std::sync::Arc;

fn float_input(
    ctx: &dyn crate::execution::NodeExecutionContextTrait,
    role: DataRole,
) -> Result<f64, String> {
    let v = ctx.get_input_by_role(&PinRole::Data(role))?;
    match v {
        DataValue::Float64(f) => Ok(f),
        DataValue::Float32(f) => Ok(f as f64),
        DataValue::Int64(i) => Ok(i as f64),
        DataValue::Int32(i) => Ok(i as f64),
        _ => Err(format!("Expected numeric, got {:?}", v.value_type())),
    }
}

fn int_input(
    ctx: &dyn crate::execution::NodeExecutionContextTrait,
    role: DataRole,
) -> Result<i64, String> {
    let v = ctx.get_input_by_role(&PinRole::Data(role))?;
    match v {
        DataValue::Int64(i) => Ok(i),
        DataValue::Int32(i) => Ok(i as i64),
        DataValue::Float64(f) => Ok(f as i64),
        DataValue::Float32(f) => Ok(f as i64),
        _ => Err(format!("Expected integer, got {:?}", v.value_type())),
    }
}

fn emit_float_series(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
    values: Vec<f64>,
    name: &str,
) -> Result<(), String> {
    let s = Series::from_iter(values.into_iter()).with_name(name.into());
    let id = ctx.put_data_series(s)?;
    ctx.emit_output_by_role(
        &PinRole::Data(DataRole::Output),
        DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Float64)),
    )?;
    Ok(())
}

fn float_type() -> PinDataTypeDefinition {
    PinDataTypeDefinition::concrete(DataType::Float64)
}

fn int_type() -> PinDataTypeDefinition {
    PinDataTypeDefinition::concrete(DataType::Int64)
}

pub fn register(registry: &NodeRegistry) {
    register_normal(registry);
    register_uniform(registry);
    register_exp(registry);
    register_gamma(registry);
    register_beta(registry);
    register_students_t(registry);
    register_cauchy(registry);
    register_chi_squared(registry);
    register_log_normal(registry);
    register_weibull(registry);
    register_laplace(registry);
    register_pareto(registry);
    register_inverse_gamma(registry);
    register_triangular(registry);
    register_fisher_snedecor(registry);
    register_erlang(registry);
}

fn register_normal(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Normal",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Normal(μ, σ) 分布抽样",
        "Sample from Normal(mean, std) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Mean",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Std",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let mean = float_input(ctx, DataRole::Inputs(0))?;
        let std = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Normal: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Normal::new(mean, std)
            .map_err(|e| format!("Normal: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "normal")
    }));
    registry.register(def);
}

fn register_uniform(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Uniform",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Uniform(low, high) 分布抽样",
        "Sample from Uniform(low, high) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Low",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "High",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let low = float_input(ctx, DataRole::Inputs(0))?;
        let high = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Uniform: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Uniform::new(low, high)
            .map_err(|e| format!("Uniform: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "uniform")
    }));
    registry.register(def);
}

fn register_exp(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Exponential",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Exp(rate) 分布抽样",
        "Sample from Exp(rate) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Rate",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let rate = float_input(ctx, DataRole::Inputs(0))?;
        let n = int_input(ctx, DataRole::Inputs(1))?;
        if n < 0 {
            return Err("Exponential: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Exp::new(rate)
            .map_err(|e| format!("Exponential: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "exp")
    }));
    registry.register(def);
}

fn register_gamma(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Gamma",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Gamma(shape, rate) 分布抽样",
        "Sample from Gamma(shape, rate) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Shape",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Rate",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let shape = float_input(ctx, DataRole::Inputs(0))?;
        let rate = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Gamma: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Gamma::new(shape, rate)
            .map_err(|e| format!("Gamma: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "gamma")
    }));
    registry.register(def);
}

fn register_beta(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Beta",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Beta(α, β) 分布抽样",
        "Sample from Beta(alpha, beta) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Alpha",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Beta",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let alpha = float_input(ctx, DataRole::Inputs(0))?;
        let beta = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Beta: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Beta::new(alpha, beta)
            .map_err(|e| format!("Beta: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "beta")
    }));
    registry.register(def);
}

fn register_students_t(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "StudentsT",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Student t(df) 分布抽样",
        "Sample from Student's t(df) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "DF",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let df = float_input(ctx, DataRole::Inputs(0))?;
        let n = int_input(ctx, DataRole::Inputs(1))?;
        if n < 0 {
            return Err("StudentsT: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::StudentsT::new(0.0, 1.0, df)
            .map_err(|e| format!("StudentsT: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "students_t")
    }));
    registry.register(def);
}

fn register_cauchy(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Cauchy",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Cauchy(location, scale) 分布抽样",
        "Sample from Cauchy(location, scale) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Location",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Scale",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let loc = float_input(ctx, DataRole::Inputs(0))?;
        let scale = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Cauchy: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Cauchy::new(loc, scale)
            .map_err(|e| format!("Cauchy: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "cauchy")
    }));
    registry.register(def);
}

fn register_chi_squared(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "ChiSquared",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 ChiSquared(df) 分布抽样",
        "Sample from ChiSquared(df) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "DF",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(1),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let df = float_input(ctx, DataRole::Inputs(0))?;
        let n = int_input(ctx, DataRole::Inputs(1))?;
        if n < 0 {
            return Err("ChiSquared: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::ChiSquared::new(df)
            .map_err(|e| format!("ChiSquared: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "chi_squared")
    }));
    registry.register(def);
}

fn register_log_normal(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "LogNormal",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 LogNormal(μ, σ) 分布抽样",
        "Sample from LogNormal(mu, sigma) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Mu",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Sigma",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let mu = float_input(ctx, DataRole::Inputs(0))?;
        let sigma = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("LogNormal: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::LogNormal::new(mu, sigma)
            .map_err(|e| format!("LogNormal: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "log_normal")
    }));
    registry.register(def);
}

fn register_weibull(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Weibull",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Weibull(shape, scale) 分布抽样",
        "Sample from Weibull(shape, scale) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Shape",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Scale",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let shape = float_input(ctx, DataRole::Inputs(0))?;
        let scale = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Weibull: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Weibull::new(shape, scale)
            .map_err(|e| format!("Weibull: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "weibull")
    }));
    registry.register(def);
}

fn register_laplace(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Laplace",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Laplace(location, scale) 分布抽样",
        "Sample from Laplace(location, scale) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Location",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Scale",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let loc = float_input(ctx, DataRole::Inputs(0))?;
        let scale = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Laplace: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Laplace::new(loc, scale)
            .map_err(|e| format!("Laplace: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "laplace")
    }));
    registry.register(def);
}

fn register_pareto(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Pareto",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Pareto(shape, scale) 分布抽样",
        "Sample from Pareto(shape, scale) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Shape",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Scale",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let shape = float_input(ctx, DataRole::Inputs(0))?;
        let scale = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Pareto: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Pareto::new(shape, scale)
            .map_err(|e| format!("Pareto: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "pareto")
    }));
    registry.register(def);
}

fn register_inverse_gamma(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "InverseGamma",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 InverseGamma(shape, scale) 分布抽样",
        "Sample from InverseGamma(shape, scale) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Shape",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Scale",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let shape = float_input(ctx, DataRole::Inputs(0))?;
        let scale = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("InverseGamma: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::InverseGamma::new(shape, scale)
            .map_err(|e| format!("InverseGamma: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "inverse_gamma")
    }));
    registry.register(def);
}

fn register_triangular(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Triangular",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Triangular(a, b, c) 分布抽样",
        "Sample from Triangular(a, b, c) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "A",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "B",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "C",
            DataRole::Inputs(2),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(3),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let a = float_input(ctx, DataRole::Inputs(0))?;
        let b = float_input(ctx, DataRole::Inputs(1))?;
        let c = float_input(ctx, DataRole::Inputs(2))?;
        let n = int_input(ctx, DataRole::Inputs(3))?;
        if n < 0 {
            return Err("Triangular: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::Triangular::new(a, b, c)
            .map_err(|e| format!("Triangular: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "triangular")
    }));
    registry.register(def);
}

fn register_fisher_snedecor(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "FisherSnedecor",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 F(d1, d2) 分布抽样",
        "Sample from F(d1, d2) distribution",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "D1",
            DataRole::Inputs(0),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "D2",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let d1 = float_input(ctx, DataRole::Inputs(0))?;
        let d2 = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("FisherSnedecor: N must be non-negative".to_string());
        }
        let dist = statrs::distribution::FisherSnedecor::new(d1, d2)
            .map_err(|e| format!("FisherSnedecor: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "fisher_snedecor")
    }));
    registry.register(def);
}

fn register_erlang(registry: &NodeRegistry) {
    let def = NodeDefinition::new(
        "Erlang",
        vec!["Distribution".to_string(), "Random".to_string()],
    )
    .with_ui_style("value")
    .with_localized_description(
        "从 Erlang(k, rate) 分布抽样，k 为形状（整数）",
        "Sample from Erlang(k, rate) distribution, k is shape (integer)",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "K",
            DataRole::Inputs(0),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Rate",
            DataRole::Inputs(1),
            float_type(),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "N",
            DataRole::Inputs(2),
            int_type(),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Samples",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let k = int_input(ctx, DataRole::Inputs(0))?;
        let rate = float_input(ctx, DataRole::Inputs(1))?;
        let n = int_input(ctx, DataRole::Inputs(2))?;
        if n < 0 {
            return Err("Erlang: N must be non-negative".to_string());
        }
        if k < 1 {
            return Err("Erlang: K must be >= 1".to_string());
        }
        let dist = statrs::distribution::Erlang::new(k as u64, rate)
            .map_err(|e| format!("Erlang: invalid params: {}", e))?;
        let mut rng = StdRng::from_entropy();
        let values: Vec<f64> = (0..n).map(|_| dist.sample(&mut rng)).collect();
        emit_float_series(ctx, values, "erlang")
    }));
    registry.register(def);
}
