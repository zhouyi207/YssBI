//! Executable survival-analysis inventory entries.
use super::*;

const METHODS: &[(&str, &str, &str)] = &[
    (
        "survival.kaplan_meier",
        "Kaplan–Meier survival",
        "Kaplan–Meier 生存估计",
    ),
    (
        "survival.nelson_aalen",
        "Nelson–Aalen cumulative hazard",
        "Nelson–Aalen 累积风险",
    ),
    ("survival.logrank", "Log-rank test", "Log-rank 检验"),
    (
        "survival.cox",
        "Cox proportional hazards",
        "Cox 比例风险回归",
    ),
    (
        "survival.exponential",
        "Exponential survival regression",
        "指数生存回归",
    ),
    (
        "survival.weibull",
        "Weibull survival regression",
        "Weibull 生存回归",
    ),
    (
        "survival.lognormal",
        "Lognormal survival regression",
        "对数正态生存回归",
    ),
    (
        "survival.loglogistic",
        "Log-logistic survival regression",
        "对数逻辑斯蒂生存回归",
    ),
    (
        "survival.aft",
        "Accelerated failure time (AFT)",
        "加速失效时间模型 AFT",
    ),
    (
        "survival.competing_risks",
        "Competing risks (Aalen–Johansen)",
        "竞争风险（Aalen–Johansen）",
    ),
    (
        "survival.time_dependent_cox",
        "Time-dependent Cox regression",
        "时变 Cox 回归",
    ),
    (
        "workflow.subgroup",
        "Stratified Cox subgroup analysis",
        "分层 Cox 亚组分析",
    ),
    ("plot.nomogram", "Cox nomogram", "Cox 列线图"),
    (
        "plot.calibration",
        "Survival calibration curve",
        "生存校准曲线",
    ),
    (
        "plot.decision_curve",
        "Survival decision curve",
        "生存决策曲线",
    ),
];
fn union_series(ids: &[&'static str]) -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(
        ids.iter()
            .map(|id| concrete(id).map(data_series_type))
            .collect::<Result<Vec<_>, _>>()?,
    ))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "survival input",
        value: e.to_string().into(),
    })
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let curve = matches!(method, "survival.kaplan_meier" | "survival.nelson_aalen");
        let cox = matches!(
            method,
            "survival.cox" | "survival.time_dependent_cox" | "workflow.subgroup"
        );
        let parametric = matches!(
            method,
            "survival.exponential"
                | "survival.weibull"
                | "survival.lognormal"
                | "survival.loglogistic"
                | "survival.aft"
        );
        let evaluation = matches!(method, "plot.calibration" | "plot.decision_curve");
        let binary = || union_series(&["core.numeric", "core.binary"]);
        let mut ports = if method == "plot.nomogram" {
            vec![data_input(
                "model",
                "Cox model",
                concrete("statistics.model.cox")?,
            )?]
        } else {
            let mut ports = vec![];
            if method == "survival.time_dependent_cox" {
                ports.push(data_input("start", "Interval start", series_type()?)?);
            }
            ports.push(data_input(
                if method == "survival.time_dependent_cox" {
                    "stop"
                } else {
                    "time"
                },
                "Observed time",
                series_type()?,
            )?);
            ports.push(data_input(
                if method == "survival.competing_risks" {
                    "status"
                } else {
                    "event"
                },
                "Event status",
                if method == "survival.competing_risks" {
                    series_type()?
                } else {
                    binary()?
                },
            )?);
            ports
        };
        if curve {
            ports.push(bounded_user_data_input(
                "groups",
                "Group (optional)",
                label_series()?,
                0,
                Some(1),
            )?);
        }
        if method == "workflow.subgroup" {
            ports.push(data_input("treatment", "Treatment", binary()?)?);
        }
        if matches!(method, "workflow.subgroup" | "survival.logrank") {
            ports.push(data_input("groups", "Group", label_series()?)?);
        }
        if method == "survival.time_dependent_cox" {
            ports.push(data_input(
                "subjects",
                "Subject identifier",
                label_series()?,
            )?);
        }
        if cox || parametric {
            ports.push(bounded_user_data_input(
                "x",
                "X",
                series_type()?,
                if cox && method != "workflow.subgroup" {
                    1
                } else {
                    0
                },
                None,
            )?);
        }
        if evaluation {
            ports.push(data_input(
                "predicted_risk",
                "Predicted event probability",
                series_type()?,
            )?);
        }
        ports.push(data_output(
            "result",
            "Result",
            if method == "survival.cox" {
                concrete("statistics.model.cox")?
            } else if method.starts_with("plot.") {
                concrete("plot.data")?
            } else {
                report_type()?
            },
        )?);
        if method == "survival.cox" || parametric {
            ports.push(fixed_numeric_table(
                "predictions",
                "Time, event and predicted risk",
                &["time", "event", "risk"],
            )?);
        }
        let mut parameters = vec![];
        if cox {
            parameters.push(choice_parameter(
                "survival_ties",
                "efron",
                &["efron", "breslow"],
            )?);
        }
        if cox || parametric {
            parameters.extend([
                minimum_integer_parameter("max_iterations", 500, 1)?,
                tolerance_parameter("0.0000001")?,
            ]);
        }
        if method == "survival.cox" || parametric || method.starts_with("plot.") {
            parameters.push(decimal_parameter("survival_horizon", "1")?);
        }
        match method {
            "survival.aft" => parameters.push(choice_parameter(
                "aft_distribution",
                "weibull",
                &["exponential", "weibull", "lognormal", "loglogistic"],
            )?),
            "plot.nomogram" => parameters.push(minimum_integer_parameter("nomogram_ticks", 5, 2)?),
            "plot.calibration" => {
                parameters.push(minimum_integer_parameter("calibration_bins", 10, 2)?)
            }
            "plot.decision_curve" => parameters.extend([
                decimal_parameter("decision_threshold_min", "0.01")?,
                decimal_parameter("decision_threshold_max", "0.99")?,
                minimum_integer_parameter("decision_points", 99, 2)?,
            ]),
            _ => {}
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.survival", NodeCategoryId::new)?,
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
        let aliases: &'static [&'static str] = match method {
            "survival.kaplan_meier" => &["survival.kaplan_meier", "Kaplan-Meier", "KM"],
            "survival.nelson_aalen" => &["survival.nelson_aalen", "Nelson–Aalen累计风险", "NA"],
            "survival.logrank" => &["survival.logrank", "Log-rank检验"],
            "survival.cox" => &["survival.cox", "Cox回归", "PH"],
            "survival.exponential" => &["survival.exponential", "指数分布生存模型"],
            "survival.weibull" => &["survival.weibull", "Weibull生存模型"],
            "survival.lognormal" => &["survival.lognormal", "Lognormal生存模型"],
            "survival.loglogistic" => &["survival.loglogistic", "Log-logistic生存模型"],
            "survival.aft" => &["survival.aft", "AFT加速失效时间模型"],
            "survival.competing_risks" => &[
                "survival.competing_risks",
                "竞争风险模型",
                "CIF",
                "Aalen-Johansen",
            ],
            "survival.time_dependent_cox" => &["survival.time_dependent_cox", "时间依赖Cox模型"],
            "workflow.subgroup" => &["workflow.subgroup", "亚组分析"],
            "plot.nomogram" => &["plot.nomogram", "列线图"],
            "plot.calibration" => &["plot.calibration", "校准曲线"],
            "plot.decision_curve" => &["plot.decision_curve", "DCA曲线"],
            _ => unreachable!("survival method table"),
        };
        for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
            fragment.messages.extend([
                (locale, node_key_text(&id, "title"), Text(title)),
                (locale, node_key_text(&id, "documentation"), Text(title)),
                (locale, node_key_text(&id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
    fragment.messages.extend([
        (
            "en-US",
            "types.statistics_model_cox.title".into(),
            Text("Cox survival model"),
        ),
        (
            "zh-CN",
            "types.statistics_model_cox.title".into(),
            Text("Cox 生存模型"),
        ),
    ]);
    for (key, en, zh, en_description, zh_description) in [
        (
            "survival_ties",
            "Tied events",
            "并列事件",
            "Efron (default) or Breslow partial likelihood; baseline hazards always use Breslow.",
            "偏似然采用 Efron（默认）或 Breslow；基线累积风险均采用 Breslow。",
        ),
        (
            "survival_horizon",
            "Prediction horizon",
            "预测时点",
            "Positive time in the input time units; default 1. Evaluation risks must refer to the same horizon.",
            "输入时间单位下的正时点，默认 1；评估用的预测风险必须对应同一时点。",
        ),
        (
            "aft_distribution",
            "AFT distribution",
            "AFT 分布",
            "Weibull (default), exponential, lognormal or loglogistic. All include an intercept.",
            "Weibull（默认）、指数、对数正态或对数逻辑斯蒂分布；均包含截距。",
        ),
        (
            "nomogram_ticks",
            "Ticks per axis",
            "每轴刻度数",
            "Default 5, at least 2. Points and survival scales are derived from the connected Cox model.",
            "默认 5，至少 2；积分与生存概率刻度由连接的 Cox 模型计算。",
        ),
        (
            "calibration_bins",
            "Calibration bins",
            "校准分组数",
            "Default 10, at least 2. Equal predictions stay together, so the actual number can be smaller.",
            "默认 10，至少 2；相同预测值不拆组，实际组数可能更少。",
        ),
        (
            "decision_threshold_min",
            "Minimum threshold",
            "最小阈值",
            "Default 0.01; strictly positive and below the maximum threshold.",
            "默认 0.01，必须大于 0 且小于最大阈值。",
        ),
        (
            "decision_threshold_max",
            "Maximum threshold",
            "最大阈值",
            "Default 0.99; strictly below 1 and above the minimum threshold.",
            "默认 0.99，必须小于 1 且大于最小阈值。",
        ),
        (
            "decision_points",
            "Threshold points",
            "阈值点数",
            "Default 99, at least 2; evenly spaced including both endpoints.",
            "默认 99，至少 2；包含两端点的等距阈值网格。",
        ),
    ] {
        fragment.messages.extend([
            (
                "en-US",
                format!("parameters.statistics.{key}.title"),
                Text(en),
            ),
            (
                "zh-CN",
                format!("parameters.statistics.{key}.title"),
                Text(zh),
            ),
            (
                "en-US",
                format!("parameters.statistics.{key}.description"),
                Text(en_description),
            ),
            (
                "zh-CN",
                format!("parameters.statistics.{key}.description"),
                Text(zh_description),
            ),
        ]);
    }
    Ok(())
}
