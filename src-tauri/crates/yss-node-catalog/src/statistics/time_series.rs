//! Executable entries for the remaining time-series inventory.
use super::*;

const METHODS: &[(&str, &str, &str, &[&str])] = &[
    (
        "timeseries.arima",
        "ARIMA forecast",
        "ARIMA 预测",
        &["ARIMA", "差分自回归移动平均"],
    ),
    (
        "timeseries.sarima",
        "Seasonal ARIMA forecast",
        "季节 SARIMA 预测",
        &["SARIMA", "季节预测"],
    ),
    (
        "plot.correlogram",
        "Correlogram",
        "偏（自）相关图",
        &["ACF", "PACF", "相关图"],
    ),
    (
        "plot.time_series",
        "Time-series plot",
        "时序图",
        &["time series", "时间序列图"],
    ),
    (
        "timeseries.ecm",
        "Error-correction model",
        "ECM 误差修正模型",
        &["ECM", "误差修正"],
    ),
    (
        "timeseries.arch",
        "ARCH model",
        "ARCH 模型",
        &["ARCH", "条件异方差"],
    ),
    (
        "timeseries.grey_prediction",
        "Grey prediction GM(1,1)",
        "灰色预测 GM(1,1)",
        &["GM(1,1)", "灰色预测"],
    ),
    (
        "timeseries.exponential_smoothing",
        "Simple exponential smoothing",
        "指数平滑",
        &["SES", "指数平滑"],
    ),
    (
        "timeseries.markov_prediction",
        "Markov forecast",
        "马尔可夫预测",
        &["Markov", "马尔可夫"],
    ),
    (
        "timeseries.phillips_perron",
        "Phillips–Perron test",
        "Phillips–Perron 检验",
        &["PP", "单位根"],
    ),
    (
        "timeseries.kpss",
        "KPSS stationarity test",
        "KPSS 平稳性检验",
        &["KPSS", "平稳性"],
    ),
    (
        "timeseries.ets",
        "Additive ETS model",
        "ETS 指数平滑状态空间模型",
        &["ETS", "状态空间平滑"],
    ),
    (
        "timeseries.holt_winters",
        "Holt–Winters forecast",
        "Holt–Winters 预测",
        &["Holt-Winters", "三次指数平滑"],
    ),
    (
        "timeseries.garch",
        "GARCH model",
        "GARCH 模型",
        &["GARCH", "波动率"],
    ),
    (
        "timeseries.egarch",
        "EGARCH model",
        "EGARCH 模型",
        &["EGARCH", "对数波动率"],
    ),
    (
        "timeseries.gjr_garch",
        "GJR-GARCH model",
        "GJR-GARCH 模型",
        &["GJR", "非对称波动率"],
    ),
];
pub(super) fn implemented(id: &str) -> bool {
    id.strip_prefix("yssbi.statistics.")
        .is_some_and(|id| METHODS.iter().any(|m| m.0 == id))
}
fn parameters(method: &str) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut p = match method {
        "timeseries.arima" | "timeseries.sarima" => vec![
            nonnegative_integer_parameter("ts_p", 1)?,
            nonnegative_integer_parameter("ts_d", 1)?,
            nonnegative_integer_parameter("ts_q", 0)?,
            toggle_parameter("constant", true)?,
            decimal_parameter("ts_confidence", "0.95")?,
        ],
        "timeseries.ecm" => vec![
            nonnegative_integer_parameter("ts_lags", 0)?,
            toggle_parameter("constant", true)?,
        ],
        "timeseries.arch" | "timeseries.garch" | "timeseries.egarch" | "timeseries.gjr_garch" => {
            vec![
                positive_integer_parameter("ts_p", 1)?,
                toggle_parameter("constant", true)?,
            ]
        }
        "timeseries.exponential_smoothing" => vec![
            decimal_parameter("ts_alpha", "0.2")?,
            toggle_parameter("ts_optimize", true)?,
        ],
        "timeseries.ets" | "timeseries.holt_winters" => vec![
            decimal_parameter("ts_alpha", "0.2")?,
            decimal_parameter("ts_beta", "0.1")?,
            decimal_parameter("ts_gamma", "0.1")?,
            decimal_parameter("ts_phi", "0.98")?,
            toggle_parameter("ts_optimize", true)?,
            toggle_parameter("ts_trend", true)?,
            toggle_parameter("ts_damped", false)?,
            choice_parameter(
                "ts_seasonality",
                if method == "timeseries.ets" {
                    "none"
                } else {
                    "additive"
                },
                if method == "timeseries.ets" {
                    &["none", "additive"]
                } else {
                    &["additive", "multiplicative"]
                },
            )?,
            minimum_integer_parameter("ts_period", 12, 2)?,
        ],
        "timeseries.markov_prediction" => vec![decimal_parameter("ts_pseudocount", "0")?],
        "timeseries.phillips_perron" | "timeseries.kpss" => vec![
            nonnegative_integer_parameter("ts_bandwidth", 4)?,
            choice_parameter(
                "ts_deterministic",
                "constant",
                if method == "timeseries.kpss" {
                    &["constant", "trend"]
                } else {
                    &["none", "constant", "trend"]
                },
            )?,
        ],
        "plot.correlogram" => vec![bounded_integer_parameter("ts_maximum_lag", 20, 1, 40)?],
        _ => vec![],
    };
    if method == "timeseries.sarima" {
        p.extend([
            nonnegative_integer_parameter("ts_seasonal_p", 0)?,
            nonnegative_integer_parameter("ts_seasonal_d", 1)?,
            nonnegative_integer_parameter("ts_seasonal_q", 1)?,
            minimum_integer_parameter("ts_period", 12, 2)?,
        ]);
    }
    if matches!(
        method,
        "timeseries.garch" | "timeseries.egarch" | "timeseries.gjr_garch"
    ) {
        p.push(nonnegative_integer_parameter("ts_q", 1)?);
    }
    if method == "timeseries.egarch" {
        p.extend([
            positive_integer_parameter("ts_simulations", 1000)?,
            nonnegative_integer_parameter("ts_seed", 42)?,
        ]);
    }
    if !matches!(
        method,
        "timeseries.ecm"
            | "timeseries.phillips_perron"
            | "timeseries.kpss"
            | "plot.correlogram"
            | "plot.time_series"
    ) {
        p.push(positive_integer_parameter("ts_horizon", 10)?);
    }
    if matches!(
        method,
        "timeseries.arima"
            | "timeseries.sarima"
            | "timeseries.arch"
            | "timeseries.garch"
            | "timeseries.egarch"
            | "timeseries.gjr_garch"
            | "timeseries.exponential_smoothing"
            | "timeseries.ets"
            | "timeseries.holt_winters"
    ) {
        p.extend([
            positive_integer_parameter("max_iterations", 500)?,
            tolerance_parameter("0.000001")?,
        ]);
    }
    Ok(p)
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh, aliases) in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let input = if method == "timeseries.markov_prediction" {
            normalize_type_expr(TypeExpr::Union(
                [
                    "core.numeric",
                    "core.binary",
                    "core.categorical",
                    "core.ordinal",
                    "core.text",
                    "core.identifier",
                ]
                .iter()
                .map(|id| concrete(id).map(data_series_type))
                .collect::<Result<Vec<_>, _>>()?,
            ))
            .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                context: "time-series states",
                value: e.to_string().into(),
            })?
        } else {
            series_type()?
        };
        let mut ports = vec![data_input("series", "Series", input)?];
        if method == "timeseries.ecm" {
            ports.push(bounded_user_data_input(
                "predictors",
                "Long-run predictor",
                series_type()?,
                1,
                None,
            )?);
        }
        if method == "plot.time_series" {
            ports.push(bounded_user_data_input(
                "time",
                "Time (optional)",
                series_type()?,
                0,
                Some(1),
            )?);
        }
        ports.push(data_output(
            "result",
            "Result",
            if method.starts_with("plot.") {
                concrete("plot.data")?
            } else {
                report_type()?
            },
        )?);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.timeseries", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters(method)?)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
        for (locale, title, help) in [
            (
                "en-US",
                en,
                "Analyze an ordered time series. See help for assumptions, estimation and forecast indexing.",
            ),
            (
                "zh-CN",
                zh,
                "分析按时间排序的序列；模型假设、估计方法及预测索引见节点帮助。",
            ),
        ] {
            fragment.messages.extend([
                (locale, node_key_text(&id, "title"), Text(title)),
                (locale, node_key_text(&id, "documentation"), Text(help)),
                (locale, node_key_text(&id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
    for (key, en, zh) in [
        ("ts_p", "AR / ARCH order", "AR / ARCH 阶数"),
        ("ts_d", "Difference order", "普通差分阶数"),
        ("ts_q", "MA / variance order", "MA / 方差滞后阶数"),
        ("ts_seasonal_p", "Seasonal AR order", "季节 AR 阶数"),
        ("ts_seasonal_d", "Seasonal difference order", "季节差分阶数"),
        ("ts_seasonal_q", "Seasonal MA order", "季节 MA 阶数"),
        ("ts_period", "Seasonal period", "季节周期"),
        ("ts_horizon", "Forecast steps", "预测期数"),
        (
            "ts_confidence",
            "Prediction interval coverage",
            "预测区间置信水平",
        ),
        ("ts_lags", "Short-run difference lags", "短期差分滞后阶数"),
        ("ts_alpha", "Level smoothing alpha", "水平平滑系数 alpha"),
        ("ts_beta", "Trend smoothing beta", "趋势平滑系数 beta"),
        ("ts_gamma", "Seasonal smoothing gamma", "季节平滑系数 gamma"),
        ("ts_phi", "Trend damping phi", "趋势阻尼系数 phi"),
        (
            "ts_optimize",
            "Estimate smoothing parameters",
            "估计平滑系数",
        ),
        ("ts_trend", "Additive trend", "加性趋势"),
        ("ts_damped", "Damped trend", "阻尼趋势"),
        ("ts_seasonality", "Seasonality", "季节形式"),
        ("ts_pseudocount", "Transition pseudocount", "转移伪计数"),
        ("ts_bandwidth", "Newey–West bandwidth", "Newey–West 带宽"),
        ("ts_deterministic", "Deterministic terms", "确定性项"),
        ("ts_maximum_lag", "Displayed lags", "显示滞后数"),
        (
            "ts_simulations",
            "EGARCH forecast simulations",
            "EGARCH 预测模拟次数",
        ),
        ("ts_seed", "Forecast random seed", "预测随机种子"),
    ] {
        for (locale, title, help) in [
            (
                "en-US",
                en,
                "See node help for this parameter's default, valid range and estimation meaning.",
            ),
            ("zh-CN", zh, "默认值、有效范围及统计含义见节点帮助。"),
        ] {
            fragment.messages.extend([
                (
                    locale,
                    format!("parameters.statistics.{key}.title"),
                    Text(title),
                ),
                (
                    locale,
                    format!("parameters.statistics.{key}.description"),
                    Text(help),
                ),
            ]);
        }
    }
    Ok(())
}
