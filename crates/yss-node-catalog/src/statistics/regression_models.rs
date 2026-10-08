//! Executable estimators and model-building analyses in the regression category.
use super::*;
const SPECS: &[[&str; 3]] = &[
    ["regression.robust", "Regression Robust", "Robust回归"],
    [
        "regression.hierarchical",
        "Regression Hierarchical",
        "分层回归",
    ],
    ["regression.stepwise", "Regression Stepwise", "逐步回归"],
    ["regression.curve", "Regression Curve", "曲线回归"],
    ["regression.nonlinear", "Regression Nonlinear", "非线性回归"],
    [
        "regression.nonlinear_formula",
        "Regression Nonlinear Formula",
        "非线性回归（自定义公式）",
    ],
    ["regression.ridge", "Regression Ridge", "岭回归"],
    ["regression.lasso", "Regression Lasso", "Lasso回归"],
    ["regression.pls", "Regression PLS", "PLS回归"],
    [
        "regression.logit.multinomial",
        "Regression Logit Multinomial",
        "多分类Logit",
    ],
    [
        "regression.logit.ordinal",
        "Regression Logit Ordinal",
        "有序Logit",
    ],
    [
        "regression.logit.firth",
        "Regression Logit Firth",
        "Firth惩罚Logit回归",
    ],
    ["regression.poisson", "Regression Poisson", "Poisson回归"],
    [
        "regression.negative_binomial",
        "Regression Negative Binomial",
        "负二项回归",
    ],
    [
        "regression.zero_inflated_poisson",
        "Regression Zero Inflated Poisson",
        "零膨胀泊松回归",
    ],
    [
        "regression.zero_inflated_negative_binomial",
        "Regression Zero Inflated Negative Binomial",
        "零膨胀负二项回归",
    ],
    ["regression.tobit", "Regression Tobit", "Tobit模型"],
    [
        "regression.logit.conditional",
        "Regression Logit Conditional",
        "条件Logit回归",
    ],
    ["regression.deming", "Regression Deming", "Deming回归"],
    ["regression.quantile", "Regression Quantile", "分位数回归"],
    [
        "workflow.regression.univariate_multivariable",
        "Workflow Regression Univariate Multivariable",
        "单因素与多因素回归",
    ],
    [
        "workflow.regression.grouped",
        "Workflow Regression Grouped",
        "分组回归",
    ],
    [
        "workflow.regression.baseline",
        "Workflow Regression Baseline",
        "基准回归",
    ],
    ["regression.threshold", "Regression Threshold", "门槛回归"],
    ["transform.rcs", "Transform RCS", "RCS样条分析"],
    ["regression.glm", "Regression GLM", "GLM广义线性模型"],
    ["regression.gamma", "Regression Gamma", "Gamma回归"],
    [
        "regression.inverse_gaussian",
        "Regression Inverse Gaussian",
        "逆高斯回归",
    ],
    [
        "regression.cloglog",
        "Regression Complementary Log-log",
        "Complementary log-log模型",
    ],
    ["regression.beta", "Regression Beta", "Beta回归"],
    [
        "regression.fractional_response",
        "Regression Fractional Response",
        "Fractional Response模型",
    ],
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for aliases in SPECS {
        let [method, en, zh] = *aliases;
        let id = format!("yssbi.statistics.{method}");
        let (ports, parameters) = interface(method)?;
        let mut parameters = parameters;
        for parameter in &mut parameters {
            let key = parameter.key.as_str();
            let (title_en, title_zh, help_en, help_zh) = parameter_text(key);
            parameter.title_key = node_key(&id, &format!("parameters.{key}.title"))?;
            parameter.description_key =
                Some(node_key(&id, &format!("parameters.{key}.description"))?);
            for (locale, title, help) in
                [("en-US", title_en, help_en), ("zh-CN", title_zh, help_zh)]
            {
                fragment.messages.push((
                    locale,
                    parameter.title_key.as_str().to_owned(),
                    Text(title),
                ));
                fragment.messages.push((
                    locale,
                    parameter
                        .description_key
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .to_owned(),
                    Text(help),
                ));
            }
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.regression", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
        for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
            fragment.messages.extend([
                (locale, node_key_text(&id, "title"), Text(title)),
                (locale, node_key_text(&id, "documentation"), Text(title)),
                (locale, node_key_text(&id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
    Ok(())
}
fn when(key: &str, value: &str) -> Result<ParameterCondition, BuiltinAssemblyError> {
    Ok(ParameterCondition {
        key: sid(key, ParameterKey::new)?,
        values: vec![DataValue::String(value.into())].into_boxed_slice(),
    })
}
fn list(key: &'static str, default: &[i64]) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        series_type()?,
        ParameterEditorSpec::Auto,
        DataValue::List(default.iter().copied().map(DataValue::Integer).collect()),
        vec![],
    )
}
fn text_field(key: &'static str, default: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Text { multiline: true },
        DataValue::String(default.into()),
        vec![],
    )
}
fn iteration(max: i64) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(vec![
        positive_integer_parameter("max_iterations", max)?,
        decimal_parameter("tolerance", "0.0000001")?,
    ])
}
fn union_series(ids: &[&'static str]) -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(
        ids.iter()
            .map(|id| concrete(id).map(data_series_type))
            .collect::<Result<Vec<_>, _>>()?,
    ))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "regression series",
        value: e.to_string().into(),
    })
}
fn interface(method: &str) -> Result<(Vec<PortSpec>, Vec<Parameter>), BuiltinAssemblyError> {
    let response = match method {
        "regression.logit.multinomial" => label_series()?,
        "regression.logit.ordinal" => union_series(&["core.numeric", "core.ordinal"])?,
        "regression.logit.firth"
        | "regression.logit.conditional"
        | "regression.cloglog"
        | "regression.glm" => union_series(&["core.numeric", "core.binary"])?,
        _ => series_type()?,
    };
    let mut ports = vec![data_input("y", "Y", response)?];
    if matches!(
        method,
        "regression.curve" | "regression.nonlinear" | "regression.deming" | "transform.rcs"
    ) {
        ports.push(data_input("x", "X", series_type()?)?);
    } else {
        ports.push(bounded_user_data_input("x", "X", series_type()?, 1, None)?);
    }
    if matches!(
        method,
        "regression.logit.conditional" | "workflow.regression.grouped"
    ) {
        ports.push(data_input("groups", "Group", label_series()?)?);
    }
    if method == "regression.threshold" {
        ports.push(data_input(
            "threshold_variable",
            "Threshold variable",
            series_type()?,
        )?);
    }
    if matches!(
        method,
        "regression.zero_inflated_poisson" | "regression.zero_inflated_negative_binomial"
    ) {
        ports.push(bounded_user_data_input(
            "inflation_predictors",
            "Inflation predictor",
            series_type()?,
            0,
            None,
        )?);
    }
    ports.push(data_output("result", "Result", report_type()?)?);
    let mut params = vec![];
    if !matches!(
        method,
        "regression.logit.ordinal"
            | "regression.logit.conditional"
            | "regression.curve"
            | "regression.nonlinear"
            | "regression.nonlinear_formula"
            | "regression.deming"
            | "regression.pls"
            | "transform.rcs"
    ) {
        params.push(toggle_parameter("constant", true)?);
    }
    match method {
        "regression.robust" => {
            params.extend([
                choice_parameter("robust_loss", "huber", &["huber", "tukey"])?,
                decimal_parameter("tuning", "1.345")?,
            ]);
            params.extend(iteration(500)?);
        }
        "regression.ridge" | "regression.lasso" => {
            params.extend([
                decimal_parameter("lambda", "1")?,
                toggle_parameter("standardize", true)?,
            ]);
            if method.ends_with("lasso") {
                params.extend(iteration(5000)?);
            }
        }
        "regression.pls" => params.extend([
            positive_integer_parameter("components", 1)?,
            toggle_parameter("standardize", true)?,
        ]),
        "regression.curve" => params.extend([
            choice_parameter(
                "curve_family",
                "polynomial",
                &[
                    "polynomial",
                    "logarithmic",
                    "inverse",
                    "exponential",
                    "power",
                ],
            )?,
            bounded_integer_parameter("degree", 2, 1, 8)?.when(when("curve_family", "polynomial")?),
        ]),
        "regression.nonlinear" => {
            params.extend([
                choice_parameter(
                    "nonlinear_family",
                    "exponential",
                    &["exponential", "logistic", "michaelis_menten", "gompertz"],
                )?,
                list("initial_values", &[])?,
            ]);
            params.extend(iteration(500)?);
        }
        "regression.nonlinear_formula" => {
            params.extend([
                text_field("formula", "b1 + b2*x1")?,
                list("initial_values", &[0, 1])?,
                list("lower_bounds", &[])?,
                list("upper_bounds", &[])?,
            ]);
            params.extend(iteration(500)?);
        }
        "regression.logit.multinomial"
        | "regression.logit.ordinal"
        | "regression.logit.firth"
        | "regression.logit.conditional"
        | "regression.poisson"
        | "regression.negative_binomial"
        | "regression.zero_inflated_poisson"
        | "regression.zero_inflated_negative_binomial"
        | "regression.gamma"
        | "regression.inverse_gaussian"
        | "regression.cloglog"
        | "regression.beta" => params.extend(iteration(500)?),
        "regression.glm" => {
            params.extend([
                choice_parameter(
                    "glm_family",
                    "poisson",
                    &[
                        "gaussian",
                        "binomial",
                        "poisson",
                        "gamma",
                        "inverse_gaussian",
                    ],
                )?,
                choice_parameter("gaussian_link", "identity", &["identity", "log"])?
                    .when(when("glm_family", "gaussian")?),
                choice_parameter("binomial_link", "logit", &["logit", "probit", "cloglog"])?
                    .when(when("glm_family", "binomial")?),
            ]);
            params.extend(iteration(500)?);
        }
        "regression.fractional_response" => {
            params.push(choice_parameter(
                "response_link",
                "logit",
                &["logit", "probit", "cloglog"],
            )?);
            params.extend(iteration(500)?);
        }
        "regression.tobit" => {
            params.extend([
                choice_parameter("censoring", "left", &["left", "both"])?,
                decimal_parameter("lower", "0")?,
                decimal_parameter("upper", "1")?.when(when("censoring", "both")?),
            ]);
            params.extend(iteration(500)?);
        }
        "regression.deming" => params.push(decimal_parameter("variance_ratio", "1")?),
        "regression.quantile" => {
            params.push(decimal_parameter("quantile", "0.5")?);
            params.extend(iteration(5000)?);
        }
        "regression.hierarchical" => params.push(list("block_sizes", &[])?),
        "regression.stepwise" => params.extend([
            choice_parameter("direction", "both", &["forward", "backward", "both"])?,
            choice_parameter("criterion", "aic", &["aic", "bic"])?,
        ]),
        "regression.threshold" => params.extend([
            decimal_parameter("trimming", "0.15")?,
            bounded_integer_parameter("max_candidates", 100, 1, 200)?,
        ]),
        "transform.rcs" => params.extend([
            choice_parameter("knot_mode", "auto", &["auto", "manual"])?,
            bounded_integer_parameter("knot_count", 4, 3, 8)?.when(when("knot_mode", "auto")?),
            list("knots", &[])?.when(when("knot_mode", "manual")?),
        ]),
        "workflow.regression.univariate_multivariable"
        | "workflow.regression.grouped"
        | "workflow.regression.baseline" => {}
        _ => unreachable!("known regression method"),
    }
    Ok((ports, params))
}
fn parameter_text(key: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match key {
        "constant" => (
            "Include intercept",
            "包含常数项",
            "The intercept is not penalized.",
            "截距不受惩罚。",
        ),
        "max_iterations" => (
            "Maximum iterations",
            "最大迭代次数",
            "Positive integer; nonconvergence fails the calculation.",
            "正整数；不收敛时计算失败。",
        ),
        "tolerance" => (
            "Convergence tolerance",
            "收敛容差",
            "1e-12–0.01; default 1e-7.",
            "1e-12–0.01，默认 1e-7。",
        ),
        "robust_loss" => (
            "Robust loss",
            "稳健损失",
            "Huber or Tukey bisquare M-estimation with MAD scale.",
            "采用 MAD 尺度的 Huber 或 Tukey 双权 M 估计。",
        ),
        "tuning" => (
            "Loss tuning constant",
            "损失调节常数",
            "Positive; Huber commonly uses 1.345, Tukey 4.685.",
            "必须为正；Huber 常用 1.345，Tukey 常用 4.685。",
        ),
        "lambda" => (
            "Penalty strength",
            "惩罚强度",
            "Nonnegative; the objective uses RSS/(2n).",
            "非负；目标函数采用 RSS/(2n)。",
        ),
        "standardize" => (
            "Standardize predictors",
            "标准化自变量",
            "Return coefficients in original units.",
            "输出原始单位的系数。",
        ),
        "components" => (
            "PLS components",
            "PLS 成分数",
            "Positive integer, at most the predictor count; univariate response.",
            "正整数，不能超过自变量数；当前为单响应 PLS。",
        ),
        "curve_family" => (
            "Curve family",
            "曲线族",
            "Exponential and power models fit log(response).",
            "指数和幂函数拟合响应的对数。",
        ),
        "degree" => (
            "Polynomial degree",
            "多项式次数",
            "1–8; requires enough distinct predictor values.",
            "1–8；须有足够多的自变量不同取值。",
        ),
        "nonlinear_family" => (
            "Nonlinear model",
            "非线性模型",
            "Exponential, logistic growth, Michaelis–Menten or Gompertz.",
            "指数、Logistic 生长、Michaelis–Menten 或 Gompertz。",
        ),
        "initial_values" => (
            "Initial parameter values",
            "参数初始值",
            "b1, b2, …; empty uses automatic starts for built-in models.",
            "按 b1、b2……排列；内置模型留空时自动初始化。",
        ),
        "formula" => (
            "Model formula",
            "模型公式",
            "Use x1, x2, … and parameters b1, b2, ….",
            "用 x1、x2……引用自变量，用 b1、b2……引用参数。",
        ),
        "lower_bounds" => (
            "Parameter lower bounds",
            "参数下界",
            "Empty means unbounded; otherwise one finite bound per parameter.",
            "留空表示无下界，否则每个参数填写一个有限下界。",
        ),
        "upper_bounds" => (
            "Parameter upper bounds",
            "参数上界",
            "Empty means unbounded; otherwise one finite bound per parameter.",
            "留空表示无上界，否则每个参数填写一个有限上界。",
        ),
        "glm_family" => (
            "GLM distribution",
            "GLM 分布族",
            "Gaussian, binomial, Poisson, Gamma or inverse Gaussian.",
            "Gaussian、Binomial、Poisson、Gamma 或逆高斯。",
        ),
        "gaussian_link" => (
            "Gaussian link",
            "Gaussian 链接",
            "Identity or log; log requires a positive mean.",
            "恒等或对数链接；对数链接要求正均值。",
        ),
        "binomial_link" | "response_link" => (
            "Response link",
            "响应链接",
            "Logit, probit or complementary log-log.",
            "Logit、Probit 或互补 log-log。",
        ),
        "censoring" => (
            "Censoring boundaries",
            "删失边界",
            "Left or both bounds; this is censoring, not truncation.",
            "左删失或双侧删失，不是截断抽样。",
        ),
        "lower" => (
            "Lower censoring boundary",
            "下删失点",
            "Observed values must be at least this bound.",
            "观测响应不得小于此边界。",
        ),
        "upper" => (
            "Upper censoring boundary",
            "上删失点",
            "Must exceed the lower bound.",
            "必须大于下界。",
        ),
        "variance_ratio" => (
            "Measurement-error variance ratio",
            "测量误差方差比",
            "Var(response error)/Var(predictor error); positive, default 1.",
            "响应误差方差/自变量误差方差；必须为正，默认 1。",
        ),
        "quantile" => (
            "Conditional quantile",
            "条件分位点",
            "Strictly between 0 and 1; default 0.5.",
            "严格位于 0 与 1 之间，默认 0.5。",
        ),
        "block_sizes" => (
            "Predictor block sizes",
            "自变量分块大小",
            "Positive integers summing to the predictor count; empty enters one variable at a time.",
            "正整数之和须等于自变量数；留空表示每次进入一个变量。",
        ),
        "direction" => (
            "Selection direction",
            "选择方向",
            "Forward, backward or both.",
            "向前、向后或双向选择。",
        ),
        "criterion" => (
            "Selection criterion",
            "选择准则",
            "Minimize AIC or BIC; final inference is conditional on selection.",
            "最小化 AIC 或 BIC；最终推断以选定模型为条件。",
        ),
        "trimming" => (
            "Minimum regime fraction",
            "最小区间样本比例",
            "0.05–0.45; each regime requires positive residual degrees of freedom.",
            "0.05–0.45；两侧均须有正的残差自由度。",
        ),
        "max_candidates" => (
            "Threshold search candidates",
            "门槛搜索候选数",
            "1–200; larger candidate sets are sampled evenly in sorted order.",
            "1–200；候选更多时按排序后位置均匀取样。",
        ),
        "knot_mode" => (
            "Spline knot specification",
            "样条节点方式",
            "Automatic sample quantiles or explicit knot locations.",
            "自动样本分位点或显式节点位置。",
        ),
        "knot_count" => (
            "Number of spline knots",
            "样条节点数",
            "3–8; repeated automatic quantiles are rejected.",
            "3–8；自动分位点重复时须调整节点。",
        ),
        "knots" => (
            "Spline knot locations",
            "样条节点位置",
            "3–8 finite, strictly increasing values.",
            "3–8 个有限且严格递增的数值。",
        ),
        _ => unreachable!("regression parameter localization"),
    }
}
