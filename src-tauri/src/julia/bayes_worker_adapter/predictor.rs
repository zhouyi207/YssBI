//! Typed Julia model source generation.

use std::collections::BTreeMap;

use crate::sci::api::bayes::{
    BayesModelSpec, BinaryOp, Expression, LikelihoodSpec, MathFunction, UnaryOp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JuliaMathFunction {
    Exp,
    Ln,
    Sqrt,
    Abs,
    Sin,
    Cos,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(super) enum JuliaModelGenerationError {
    #[error("Julia model contains a non-finite constant")]
    NonFiniteConstant,
    #[error("Julia model references an undeclared parameter")]
    ParameterNotDeclared,
    #[error("Julia model references an unbound data variable")]
    DataVariableNotBound,
    #[error("Julia model function has invalid arity")]
    InvalidFunctionArity {
        function: JuliaMathFunction,
        expected: usize,
        actual: usize,
    },
    #[error("Julia likelihood references an undeclared parameter")]
    LikelihoodParameterNotDeclared,
}

#[derive(Debug)]
pub(super) struct JuliaGeneratedModel {
    pub(super) predictor: String,
    pub(super) likelihood: String,
    pub(super) columns: Vec<String>,
}

pub(super) fn generate_julia_model(
    model: &BayesModelSpec,
) -> Result<JuliaGeneratedModel, JuliaModelGenerationError> {
    let parameters = model
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.name.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    let mut generator = JuliaExpressionGenerator {
        parameters,
        data_variables: model.data_variables(),
        columns: Vec::new(),
    };
    let expression = generator.emit(model.predictor())?;
    let likelihood = generate_likelihood(model, &expression)?;
    Ok(JuliaGeneratedModel {
        predictor: format!(
            "function (theta, columns, row_index)\n    @inbounds return {expression}\nend\n"
        ),
        likelihood,
        columns: generator.columns,
    })
}

struct JuliaExpressionGenerator<'a> {
    parameters: BTreeMap<&'a str, usize>,
    data_variables: &'a BTreeMap<String, String>,
    columns: Vec<String>,
}

impl JuliaExpressionGenerator<'_> {
    fn emit(&mut self, expression: &Expression) -> Result<String, JuliaModelGenerationError> {
        match expression {
            Expression::Number { value } => emit_number(*value),
            Expression::Parameter { name } => self.emit_parameter(name),
            Expression::DataVariable { name } => {
                let column = self
                    .data_variables
                    .get(name)
                    .ok_or(JuliaModelGenerationError::DataVariableNotBound)?;
                self.emit_column(column)
            }
            Expression::Column { name } => self.emit_column(name),
            Expression::Unary {
                op: UnaryOp::Neg,
                arg,
            } => Ok(format!("(-{})", self.emit(arg)?)),
            Expression::Binary { op, left, right } => {
                let operator = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Pow => "^",
                };
                Ok(format!(
                    "({} {operator} {})",
                    self.emit(left)?,
                    self.emit(right)?
                ))
            }
            Expression::Call { function, args } => self.emit_call(*function, args),
        }
    }

    fn emit_parameter(&self, name: &str) -> Result<String, JuliaModelGenerationError> {
        let index = self
            .parameters
            .get(name)
            .ok_or(JuliaModelGenerationError::ParameterNotDeclared)?;
        Ok(format!("theta[{index}]"))
    }

    fn emit_column(&mut self, name: &str) -> Result<String, JuliaModelGenerationError> {
        let index = match self.columns.iter().position(|column| column == name) {
            Some(index) => index + 1,
            None => {
                self.columns.push(name.to_owned());
                self.columns.len()
            }
        };
        Ok(format!("columns[row_index, {index}]"))
    }

    fn emit_call(
        &mut self,
        function: MathFunction,
        args: &[Expression],
    ) -> Result<String, JuliaModelGenerationError> {
        let (name, typed_function, expected, exact) = match function {
            MathFunction::Exp => ("exp", JuliaMathFunction::Exp, 1, true),
            MathFunction::Ln => ("log", JuliaMathFunction::Ln, 1, true),
            MathFunction::Sqrt => ("sqrt", JuliaMathFunction::Sqrt, 1, true),
            MathFunction::Abs => ("abs", JuliaMathFunction::Abs, 1, true),
            MathFunction::Sin => ("sin", JuliaMathFunction::Sin, 1, true),
            MathFunction::Cos => ("cos", JuliaMathFunction::Cos, 1, true),
            MathFunction::Min => ("min", JuliaMathFunction::Min, 2, false),
            MathFunction::Max => ("max", JuliaMathFunction::Max, 2, false),
        };
        if (exact && args.len() != expected) || (!exact && args.len() < expected) {
            return Err(JuliaModelGenerationError::InvalidFunctionArity {
                function: typed_function,
                expected,
                actual: args.len(),
            });
        }
        let arguments = args
            .iter()
            .map(|argument| self.emit(argument))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(format!("{name}({})", arguments.join(", ")))
    }
}

fn generate_likelihood(
    model: &BayesModelSpec,
    predictor: &str,
) -> Result<String, JuliaModelGenerationError> {
    let contribution = match model.likelihood() {
        LikelihoodSpec::Normal { sigma, .. } => {
            let index = model
                .parameters()
                .iter()
                .position(|parameter| parameter.name == sigma.parameter)
                .ok_or(JuliaModelGenerationError::LikelihoodParameterNotDeclared)?
                + 1;
            format!("logpdf(Normal({predictor}, theta[{index}]), y[row_index])")
        }
        LikelihoodSpec::BernoulliLogit { .. } => {
            format!("logpdf(Bernoulli(bayes_logistic({predictor})), y[row_index])")
        }
        LikelihoodSpec::PoissonLog { .. } => {
            format!("logpdf(Poisson(exp({predictor})), y[row_index])")
        }
    };
    Ok(format!(
        "function (theta, columns, y)\n    log_probability = zero(eltype(theta))\n    @inbounds for row_index in eachindex(y)\n        log_probability += {contribution}\n    end\n    return log_probability\nend\n"
    ))
}

fn emit_number(value: f64) -> Result<String, JuliaModelGenerationError> {
    value
        .is_finite()
        .then(|| format!("{value:?}"))
        .ok_or(JuliaModelGenerationError::NonFiniteConstant)
}

#[cfg(test)]
mod tests {
    use super::{JuliaMathFunction, JuliaModelGenerationError, generate_julia_model};
    use crate::sci::api::bayes::BayesModelSpec;

    fn model(predictor: serde_json::Value) -> BayesModelSpec {
        serde_json::from_value(serde_json::json!({
            "dataset": { "sourceType": "table", "sourceId": "dataset" },
            "response": {
                "expression": { "type": "data_variable", "name": "y" },
                "dataVariables": { "y": "response" }
            },
            "predictor": predictor,
            "dataVariables": { "x": "time" },
            "likelihood": {
                "type": "normal",
                "mean": { "source": "predictor" },
                "sigma": { "parameter": "sigma" }
            },
            "parameters": [
                {
                    "name": "beta",
                    "constraint": { "type": "real" },
                    "prior": { "distribution": "normal", "args": [0.0, 1.0] }
                },
                {
                    "name": "sigma",
                    "constraint": { "type": "positive" },
                    "prior": { "distribution": "exponential", "args": [1.0] }
                }
            ],
            "sampler": {
                "algorithm": "nuts",
                "chains": 2,
                "samples": 100,
                "warmup": 50,
                "seed": 7,
                "targetAccept": 0.8,
                "maxTreeDepth": 10,
                "saveSamples": false
            },
            "displayFormula": "response ~ beta * time"
        }))
        .expect("model fixture must deserialize")
    }

    #[test]
    fn valid_model_generates_indexed_julia_sources_from_final_projection() {
        let generated = generate_julia_model(&model(serde_json::json!({
            "type": "binary",
            "op": "mul",
            "left": { "type": "parameter", "name": "beta" },
            "right": { "type": "data_variable", "name": "x" }
        })))
        .expect("valid model must generate Julia source");

        assert_eq!(generated.columns, ["time"]);
        assert!(generated.predictor.contains("theta[1]"));
        assert!(generated.predictor.contains("columns[row_index, 1]"));
        assert!(generated.likelihood.contains("theta[2]"));
        assert!(generated.likelihood.contains("logpdf(Normal("));
    }

    #[test]
    fn invalid_function_arity_returns_typed_error_without_backend_prose() {
        let error = generate_julia_model(&model(serde_json::json!({
            "type": "call",
            "function": "ln",
            "args": [
                { "type": "data_variable", "name": "x" },
                { "type": "number", "value": 2.0 }
            ]
        })))
        .expect_err("invalid arity must fail generation");

        assert_eq!(
            error,
            JuliaModelGenerationError::InvalidFunctionArity {
                function: JuliaMathFunction::Ln,
                expected: 1,
                actual: 2,
            }
        );
    }
}
