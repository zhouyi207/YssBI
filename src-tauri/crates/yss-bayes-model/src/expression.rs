//! Bayesian expression parsing and symbol classification.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use yss_math::{
    BinaryOp as MathBinaryOp, ComparisonOp, MathExpr, ParseOptions, UnaryOp as MathUnaryOp,
    parse_relations,
};

use super::model::{BinaryOp, MathFunction, UnaryOp};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawExpression {
    Number {
        value: f64,
    },
    Symbol {
        name: String,
    },
    Unary {
        op: UnaryOp,
        arg: Box<RawExpression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<RawExpression>,
        right: Box<RawExpression>,
    },
    Call {
        function: MathFunction,
        args: Vec<RawExpression>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaDraft {
    pub formula_text: String,
    pub raw_response: RawExpression,
    pub raw_predictor: RawExpression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedExpression {
    pub formula: FormulaDraft,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionParseError {
    pub message: String,
}

impl ExpressionParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ExpressionParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ExpressionParseError {}

pub fn parse_model_expression(
    input: &str,
    options: ParseOptions<'_>,
) -> Result<ParsedExpression, ExpressionParseError> {
    let relations = parse_relations(input, options).map_err(math_error)?;
    if relations.len() != 1 {
        return Err(ExpressionParseError::new(
            "Bayes 公式必须包含且仅包含一个顶层关系",
        ));
    }
    let relation = &relations[0];
    let raw_response = math_expr_to_raw(&relation.left)?;
    let mut response_symbols = BTreeSet::new();
    collect_raw_symbols(&raw_response, &mut response_symbols);
    if response_symbols.len() != 1 {
        return Err(ExpressionParseError::new(
            "Bayes 响应表达式必须且只能引用一个基础数据符号",
        ));
    }

    let predictor = match relation.op {
        ComparisonOp::Eq => &relation.right,
        ComparisonOp::DistributedAs => distribution_predictor(&relation.right)?,
        _ => {
            return Err(ExpressionParseError::new(
                "Bayes 公式仅支持 = 或分布关系 ~ / \\sim",
            ));
        }
    };
    let raw_predictor = math_expr_to_raw(predictor)?;

    let mut symbols = BTreeSet::new();
    collect_math_symbols(&relation.left, &mut symbols);
    collect_math_symbols(&relation.right, &mut symbols);
    Ok(ParsedExpression {
        formula: FormulaDraft {
            formula_text: input.trim().to_string(),
            raw_response,
            raw_predictor,
        },
        symbols: symbols.into_iter().collect(),
    })
}

fn collect_raw_symbols(expression: &RawExpression, symbols: &mut BTreeSet<String>) {
    match expression {
        RawExpression::Number { .. } => {}
        RawExpression::Symbol { name } => {
            symbols.insert(name.clone());
        }
        RawExpression::Unary { arg, .. } => collect_raw_symbols(arg, symbols),
        RawExpression::Binary { left, right, .. } => {
            collect_raw_symbols(left, symbols);
            collect_raw_symbols(right, symbols);
        }
        RawExpression::Call { args, .. } => {
            for arg in args {
                collect_raw_symbols(arg, symbols);
            }
        }
    }
}

fn distribution_predictor(expression: &MathExpr) -> Result<&MathExpr, ExpressionParseError> {
    let MathExpr::Call { name, args } = expression else {
        return Err(ExpressionParseError::new("分布关系右侧必须是分布调用"));
    };
    let expected_args = match name.as_str() {
        "Normal" => 2,
        "BernoulliLogit" | "PoissonLog" => 1,
        _ => {
            return Err(ExpressionParseError::new(format!(
                "不支持 Bayes 分布 {name}"
            )));
        }
    };
    if args.len() != expected_args {
        return Err(ExpressionParseError::new(format!(
            "分布 {name} 必须恰好提供 {expected_args} 个参数，实际为 {} 个",
            args.len()
        )));
    }
    Ok(&args[0])
}

fn math_expr_to_raw(expression: &MathExpr) -> Result<RawExpression, ExpressionParseError> {
    match expression {
        MathExpr::Number(value) => Ok(RawExpression::Number { value: *value }),
        MathExpr::Symbol(name) => Ok(RawExpression::Symbol { name: name.clone() }),
        MathExpr::Unary {
            op: MathUnaryOp::Neg,
            operand,
        } => Ok(RawExpression::Unary {
            op: UnaryOp::Neg,
            arg: Box::new(math_expr_to_raw(operand)?),
        }),
        MathExpr::Binary { op, left, right } => Ok(RawExpression::Binary {
            op: match op {
                MathBinaryOp::Add => BinaryOp::Add,
                MathBinaryOp::Sub => BinaryOp::Sub,
                MathBinaryOp::Mul => BinaryOp::Mul,
                MathBinaryOp::Div => BinaryOp::Div,
                MathBinaryOp::Pow => BinaryOp::Pow,
            },
            left: Box::new(math_expr_to_raw(left)?),
            right: Box::new(math_expr_to_raw(right)?),
        }),
        MathExpr::Call { name, args } => Ok(RawExpression::Call {
            function: math_function(name)?,
            args: args
                .iter()
                .map(math_expr_to_raw)
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }
}

fn math_function(name: &str) -> Result<MathFunction, ExpressionParseError> {
    match name {
        "exp" => Ok(MathFunction::Exp),
        "ln" => Ok(MathFunction::Ln),
        "sqrt" => Ok(MathFunction::Sqrt),
        "abs" => Ok(MathFunction::Abs),
        "sin" => Ok(MathFunction::Sin),
        "cos" => Ok(MathFunction::Cos),
        "min" => Ok(MathFunction::Min),
        "max" => Ok(MathFunction::Max),
        _ => Err(ExpressionParseError::new(format!(
            "分布 {name} 不能用作 predictor 表达式"
        ))),
    }
}

fn collect_math_symbols(expression: &MathExpr, symbols: &mut BTreeSet<String>) {
    match expression {
        MathExpr::Number(_) => {}
        MathExpr::Symbol(name) => {
            symbols.insert(name.clone());
        }
        MathExpr::Unary { operand, .. } => collect_math_symbols(operand, symbols),
        MathExpr::Binary { left, right, .. } => {
            collect_math_symbols(left, symbols);
            collect_math_symbols(right, symbols);
        }
        MathExpr::Call { args, .. } => {
            for arg in args {
                collect_math_symbols(arg, symbols);
            }
        }
    }
}

fn math_error(error: yss_math::MathError) -> ExpressionParseError {
    ExpressionParseError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_math::ParseOptions;

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_plain_complete_formula() {
        let known = names(&["y", "a", "x", "b"]);
        let parsed = parse_model_expression("y = a*x+b", ParseOptions::plain(&known)).unwrap();
        assert_eq!(
            parsed.formula.raw_response,
            RawExpression::Symbol { name: "y".into() }
        );
        assert_eq!(parsed.symbols, ["a", "b", "x", "y"]);
        assert!(matches!(
            parsed.formula.raw_predictor,
            RawExpression::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn parses_latex_distribution_and_all_symbols() {
        let known = names(&["y", "a", "x", "b", "sigma"]);
        let parsed = parse_model_expression(
            r"y \sim \operatorname{Normal}\left(a \cdot x + b, \sigma\right)",
            ParseOptions::latex(&known),
        )
        .unwrap();
        assert_eq!(parsed.symbols, ["a", "b", "sigma", "x", "y"]);
        assert!(matches!(
            parsed.formula.raw_predictor,
            RawExpression::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn parses_latex_ln_response_and_collects_only_base_symbols() {
        let known = names(&["y", "a", "x", "sigma"]);
        for response in [r"\ln y", r"\ln(y)", r"\ln\left(y\right)"] {
            let formula = format!(r"{response} \sim \operatorname{{Normal}}(a * x, \sigma)");
            let parsed = parse_model_expression(&formula, ParseOptions::latex(&known)).unwrap();
            assert_eq!(parsed.symbols, ["a", "sigma", "x", "y"]);
            assert_eq!(
                parsed.formula.raw_response,
                RawExpression::Call {
                    function: MathFunction::Ln,
                    args: vec![RawExpression::Symbol { name: "y".into() }],
                }
            );
        }
    }

    #[test]
    fn rejects_log_without_compatibility_alias() {
        let known = names(&["y", "x"]);
        assert!(parse_model_expression("log(y) = x", ParseOptions::plain(&known)).is_err());
    }

    #[test]
    fn preserves_known_ax_and_splits_latex_ax_by_context() {
        let known_ax = names(&["y", "ax"]);
        let parsed = parse_model_expression("y = ax", ParseOptions::latex(&known_ax)).unwrap();
        assert_eq!(
            parsed.formula.raw_predictor,
            RawExpression::Symbol { name: "ax".into() }
        );

        let known_product = names(&["y", "a", "x"]);
        let parsed = parse_model_expression("y = ax", ParseOptions::latex(&known_product)).unwrap();
        assert!(matches!(
            parsed.formula.raw_predictor,
            RawExpression::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));

        let no_context = Vec::new();
        let parsed = parse_model_expression("y = ax", ParseOptions::latex(&no_context)).unwrap();
        assert!(matches!(
            parsed.formula.raw_predictor,
            RawExpression::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn parses_fraction_subscript_implicit_product_and_precedence() {
        let known = names(&["y", "a", "x", "beta_1"]);
        for formula in [r"y = \frac{a x}{2}", r"y = \beta_1 x", r"y = -x^2"] {
            parse_model_expression(formula, ParseOptions::latex(&known)).unwrap();
        }
        let parsed = parse_model_expression("y = -x^2", ParseOptions::latex(&known)).unwrap();
        assert!(matches!(
            parsed.formula.raw_predictor,
            RawExpression::Unary {
                op: UnaryOp::Neg,
                ..
            }
        ));
    }

    #[test]
    fn preserves_indexed_greek_symbols_as_distinct_parameter_names() {
        let known = names(&[
            "y", "beta_0", "beta_1", "beta_2", "beta_3", "x_1", "x_2", "x_4",
        ]);
        let parsed = parse_model_expression(
            r"y = \beta_{0} + \beta_{1}x_{1} + \beta_{2}x_{2} + \beta_{3}x_{4}",
            ParseOptions::latex(&known),
        )
        .unwrap();

        assert_eq!(
            parsed.symbols,
            [
                "beta_0", "beta_1", "beta_2", "beta_3", "x_1", "x_2", "x_4", "y"
            ]
        );
    }

    #[test]
    fn supports_project_math_function_whitelist() {
        let known = names(&["y", "x"]);
        for function in ["exp", "ln", "sqrt", "abs", "sin", "cos", "min", "max"] {
            let formula = if matches!(function, "min" | "max") {
                format!("y = {function}(x, 1)")
            } else {
                format!("y = {function}(x)")
            };
            parse_model_expression(&formula, ParseOptions::plain(&known)).unwrap();
        }
    }

    #[test]
    fn serializes_frontend_response_contract() {
        let known = names(&["y", "x"]);
        let parsed = parse_model_expression("y = x", ParseOptions::plain(&known)).unwrap();
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            serde_json::json!({
                "formula": {
                    "formulaText": "y = x",
                    "rawResponse": { "type": "symbol", "name": "y" },
                    "rawPredictor": { "type": "symbol", "name": "x" }
                },
                "symbols": ["x", "y"]
            })
        );
    }

    #[test]
    fn enforces_supported_likelihoods_and_exact_arity() {
        let known = names(&["y", "x", "sigma"]);
        for formula in [
            r"y \sim \operatorname{Normal}(x)",
            r"y \sim \operatorname{Normal}(x, \sigma, 1)",
            r"y \sim \operatorname{BernoulliLogit}(x, 1)",
            r"y \sim \operatorname{PoissonLog}()",
        ] {
            assert!(
                parse_model_expression(formula, ParseOptions::latex(&known))
                    .unwrap_err()
                    .to_string()
                    .contains("必须恰好提供"),
                "formula should fail exact arity validation: {formula}"
            );
        }
        for formula in [
            r"y \sim \operatorname{Bernoulli}(x)",
            r"y \sim \operatorname{Poisson}(x)",
        ] {
            assert!(
                parse_model_expression(formula, ParseOptions::latex(&known))
                    .unwrap_err()
                    .to_string()
                    .contains("不支持 Bayes 分布"),
                "formula should reject implicit link semantics: {formula}"
            );
        }
        for formula in [
            r"y \sim \operatorname{Normal}(x, \sigma)",
            r"y \sim \operatorname{BernoulliLogit}(x)",
            r"y \sim \operatorname{PoissonLog}(x)",
        ] {
            parse_model_expression(formula, ParseOptions::latex(&known)).unwrap();
        }
    }

    #[test]
    fn rejects_unknown_function_and_distribution() {
        let known = names(&["y", "x"]);
        assert!(
            parse_model_expression("y = eval(x)", ParseOptions::plain(&known))
                .unwrap_err()
                .to_string()
                .contains("不支持函数")
        );
        assert!(
            parse_model_expression(
                r"y \sim \operatorname{Mystery}(x)",
                ParseOptions::latex(&known),
            )
            .unwrap_err()
            .to_string()
            .contains("不支持函数或分布")
        );
    }
}
