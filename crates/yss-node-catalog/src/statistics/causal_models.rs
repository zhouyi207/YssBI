//! Executable definitions for the existing econometric/causal inventory entries.
use super::*;

const METHODS: &[(&str, &str, &str, &[&str])] = &[
    (
        "econometrics.gmm",
        "Linear IV GMM",
        "线性工具变量 GMM",
        &["GMM", "矩估计"],
    ),
    (
        "causal.rdd",
        "Sharp regression discontinuity",
        "锐断点回归 RDD",
        &["RDD", "断点回归"],
    ),
    (
        "causal.psm",
        "Propensity-score matching",
        "倾向得分匹配 PSM",
        &["PSM", "匹配"],
    ),
    (
        "econometrics.heckman_two_step",
        "Heckman two-step",
        "Heckman 两步法",
        &["Heckman", "样本选择"],
    ),
    (
        "test.heterogeneity",
        "Treatment-effect heterogeneity",
        "处理效应异质性检验",
        &["异质性检验", "interaction"],
    ),
    (
        "econometrics.sfa",
        "Stochastic frontier (half-normal)",
        "随机前沿 SFA（半正态）",
        &["SFA", "效率"],
    ),
    (
        "econometrics.sur",
        "Seemingly unrelated regression",
        "似不相关回归 SUR",
        &["SUR", "联立回归"],
    ),
    (
        "causal.ipw",
        "Inverse-probability weighting",
        "逆概率加权 IPW/IPTW",
        &["IPW", "IPTW"],
    ),
    (
        "causal.regression_adjustment",
        "Regression adjustment",
        "回归调整 RA",
        &["RA", "回归调整"],
    ),
    (
        "causal.aipw",
        "Augmented IPW",
        "双重稳健估计 AIPW",
        &["AIPW", "双重稳健"],
    ),
    (
        "causal.ate",
        "Average treatment effect (ATE)",
        "平均处理效应 ATE",
        &["ATE", "平均处理效应"],
    ),
    (
        "causal.att",
        "Effect on the treated (ATT/ATET)",
        "处理组平均处理效应 ATT/ATET",
        &["ATT", "ATET"],
    ),
    (
        "causal.synthetic_control",
        "Synthetic control",
        "合成控制",
        &["SCM", "synthetic control", "合成控制法"],
    ),
];

fn union_series(ids: &[&'static str]) -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(
        ids.iter()
            .map(|id| concrete(id).map(data_series_type))
            .collect::<Result<Vec<_>, _>>()?,
    ))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "causal input",
        value: e.to_string().into(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh, aliases) in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let effect = matches!(
            method,
            "causal.psm" | "causal.ipw" | "causal.regression_adjustment" | "causal.aipw"
        );
        let projection = matches!(method, "causal.ate" | "causal.att");
        let binary = || union_series(&["core.numeric", "core.binary"]);
        let repeated =
            |key, label, min| bounded_user_data_input(key, label, series_type()?, min, None);
        let mut ports = if projection {
            vec![data_input(
                "effects",
                "Treatment effects",
                concrete("statistics.result.treatment_effect")?,
            )?]
        } else if method == "econometrics.sur" {
            vec![repeated("y", "Y", 2)?, repeated("x", "X", 0)?]
        } else {
            vec![data_input("y", "Y", series_type()?)?]
        };
        if effect || method == "test.heterogeneity" {
            ports.push(data_input("treatment", "Treatment", binary()?)?);
        }
        if method == "test.heterogeneity" {
            ports.push(data_input(
                "groups",
                "Group",
                union_series(&[
                    "core.numeric",
                    "core.binary",
                    "core.categorical",
                    "core.ordinal",
                    "core.text",
                    "core.identifier",
                ])?,
            )?);
        }
        if method == "causal.rdd" {
            ports.push(data_input("running", "Running variable", series_type()?)?);
        }
        if method == "econometrics.heckman_two_step" {
            ports.push(data_input("selected", "Selected", binary()?)?);
        }
        if !projection
            && !matches!(
                method,
                "econometrics.sur" | "causal.rdd" | "causal.synthetic_control"
            )
        {
            ports.push(repeated(
                "x",
                "X",
                if method == "econometrics.gmm" { 1 } else { 0 },
            )?);
        }
        match method {
            "econometrics.gmm" => ports.push(repeated("instruments", "Instrument", 1)?),
            "econometrics.heckman_two_step" => {
                ports.push(repeated("selection_predictors", "Selection predictor", 1)?)
            }
            "causal.synthetic_control" => ports.push(repeated("donors", "Donor outcome", 1)?),
            _ => {}
        }
        ports.push(data_output(
            "result",
            "Result",
            if effect {
                concrete("statistics.result.treatment_effect")?
            } else {
                report_type()?
            },
        )?);
        let mut parameters = vec![];
        if matches!(
            method,
            "econometrics.gmm" | "econometrics.sur" | "econometrics.sfa"
        ) {
            parameters.push(toggle_parameter("constant", true)?);
        }
        match method {
            "econometrics.gmm" => parameters.push(choice_parameter(
                "gmm_steps",
                "two_step",
                &["one_step", "two_step"],
            )?),
            "causal.rdd" => parameters.extend([
                decimal_parameter("rdd_cutoff", "0")?,
                decimal_parameter("rdd_bandwidth", "1")?,
                choice_parameter("rdd_kernel", "triangular", &["triangular", "uniform"])?,
            ]),
            "econometrics.sfa" => parameters.push(choice_parameter(
                "frontier_type",
                "production",
                &["production", "cost"],
            )?),
            "econometrics.sur" => parameters.push(parameter(
                "equation_predictors",
                concrete("core.text")?,
                ParameterEditorSpec::Text { multiline: false },
                DataValue::String("".into()),
                vec![],
            )?),
            "causal.synthetic_control" => {
                parameters.push(minimum_integer_parameter("pre_periods", 10, 1)?)
            }
            _ => {}
        }
        if matches!(method, "causal.psm" | "causal.ipw" | "causal.aipw") {
            parameters.push(decimal_parameter("ps_overlap", "0.000001")?);
        }
        if method == "causal.psm" {
            parameters.push(decimal_parameter("ps_caliper", "0.2")?);
        }
        if matches!(
            method,
            "causal.ipw"
                | "causal.aipw"
                | "causal.regression_adjustment"
                | "econometrics.heckman_two_step"
        ) {
            parameters.extend([
                nonnegative_integer_parameter("bootstrap_replications", 0)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]);
        }
        if matches!(
            method,
            "causal.psm"
                | "causal.ipw"
                | "causal.aipw"
                | "econometrics.heckman_two_step"
                | "econometrics.sfa"
                | "causal.synthetic_control"
        ) {
            parameters.extend([
                minimum_integer_parameter(
                    "max_iterations",
                    if method == "causal.synthetic_control" {
                        5000
                    } else {
                        500
                    },
                    1,
                )?,
                tolerance_parameter("0.0000001")?,
            ]);
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.causal", NodeCategoryId::new)?,
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
    fragment.messages.extend([
        (
            "en-US",
            "types.statistics_result_treatment_effect.title".into(),
            Text("Treatment-effect estimates"),
        ),
        (
            "zh-CN",
            "types.statistics_result_treatment_effect.title".into(),
            Text("处理效应估计结果"),
        ),
    ]);
    for (key, en, zh, en_description, zh_description) in [
        (
            "gmm_steps",
            "GMM steps",
            "GMM 步数",
            "One-step 2SLS weighting or two-step heteroskedastic GMM; default two-step.",
            "一阶段 2SLS 权重或两阶段异方差 GMM；默认两阶段。",
        ),
        (
            "rdd_cutoff",
            "Discontinuity cutoff",
            "断点位置",
            "Default 0; running values at or above the cutoff are treated.",
            "默认 0；运行变量大于或等于断点时归入处理侧。",
        ),
        (
            "rdd_bandwidth",
            "RDD bandwidth",
            "断点带宽",
            "Positive bandwidth in running-variable units; default 1.",
            "运行变量原单位下的正带宽；默认 1。",
        ),
        (
            "rdd_kernel",
            "RDD kernel",
            "断点核权重",
            "Triangular (default) or uniform local-linear weights.",
            "局部线性拟合采用三角核（默认）或均匀核。",
        ),
        (
            "frontier_type",
            "Frontier type",
            "前沿类型",
            "Production (default): noise minus inefficiency; cost: noise plus inefficiency.",
            "生产前沿（默认）：随机扰动减无效率；成本前沿：随机扰动加无效率。",
        ),
        (
            "equation_predictors",
            "Equation predictor indices",
            "各方程自变量索引",
            "Empty: all predictors in every equation. Otherwise semicolon-separated equations, comma-separated one-based indices, e.g. 1,2;1,3. Use - for no predictors.",
            "留空时每个方程使用全部自变量；否则用分号分隔方程、逗号分隔从 1 开始的索引，例如 1,2;1,3；无自变量用 -。",
        ),
        (
            "pre_periods",
            "Pre-treatment periods",
            "干预前期数",
            "First rows used to fit donor weights; default 10, at least 1 and below the row count.",
            "使用前若干行拟合供体权重；默认 10，至少 1 且小于总行数。",
        ),
        (
            "ps_overlap",
            "Propensity overlap bound",
            "倾向得分重叠边界",
            "Default 0.000001; strictly between 0 and 0.5. Reject scores outside [bound,1-bound]; no trimming or clipping.",
            "默认 0.000001，严格介于 0 与 0.5 之间；得分超出 [边界,1−边界] 时拒绝计算，不裁剪或删行。",
        ),
        (
            "ps_caliper",
            "Matching caliper",
            "匹配卡尺",
            "Maximum propensity-probability distance, default 0.2, in (0,1]. Every row must find a match.",
            "倾向概率距离上限，默认 0.2，范围 (0,1]；每行均须有符合条件的匹配。",
        ),
        (
            "bootstrap_replications",
            "Bootstrap replications",
            "自助抽样次数",
            "Default 0 (point estimates only), or at least 2. Resample independent rows and refit all stages; failed replicates abort.",
            "默认 0（仅点估计），启用时至少 2 次。按独立观测整行有放回抽样并重新拟合全部阶段；抽样拟合失败时终止。",
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
