//! Complete the existing panel-category entries without duplicating their identities.
use super::*;
use yss_sci_contract::panel::MAX_COINTEGRATION_PREDICTORS;

const SPECS: &[(&str, &str, &str, &str, &[&str])] = &[
    (
        "fe",
        "yssbi.statistics.econometrics.panel.fe",
        "Fixed effects (FE)",
        "固定效应 FE",
        &["econometrics.panel.fe", "within", "固定效应FE"],
    ),
    (
        "re",
        "yssbi.statistics.econometrics.panel.re",
        "Random effects (RE)",
        "随机效应 RE",
        &["econometrics.panel.re", "FGLS", "随机效应RE"],
    ),
    (
        "fd",
        "yssbi.statistics.econometrics.panel.fd",
        "First differences",
        "第一差分模型",
        &["econometrics.panel.fd", "FD", "第一差分模型"],
    ),
    (
        "between",
        "yssbi.statistics.econometrics.panel.between",
        "Between estimator",
        "组间估计 Between",
        &["econometrics.panel.between", "组间估计Between Estimator"],
    ),
    (
        "dynamic",
        "yssbi.statistics.econometrics.panel.dynamic",
        "Dynamic panel (difference GMM)",
        "动态面板（差分 GMM）",
        &["econometrics.panel.dynamic", "Arellano Bond", "动态面板"],
    ),
    (
        "unit_root",
        "yssbi.statistics.econometrics.panel.unit_root",
        "Panel unit root (Fisher ADF)",
        "面板单位根检验（Fisher ADF）",
        &[
            "econometrics.panel.unit_root",
            "Maddala Wu",
            "面板单位根检验",
        ],
    ),
    (
        "cointegration",
        "yssbi.statistics.econometrics.panel.cointegration",
        "Panel cointegration (Fisher Engle–Granger)",
        "面板协整检验（Fisher Engle–Granger）",
        &[
            "econometrics.panel.cointegration",
            "Engle Granger",
            "面板协整检验",
        ],
    ),
];

fn parameters(method: &str) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let covariance = |default| {
        choice_parameter(
            "covariance",
            default,
            &["nonrobust", "HC0", "HC1", "HC2", "HC3", "cluster"],
        )
    };
    Ok(match method {
        "fe" | "re" => vec![
            toggle_parameter("constant", true)?,
            choice_parameter("effects", "entity", &["entity", "time", "two_way"])?,
            covariance(if method == "fe" {
                "cluster"
            } else {
                "nonrobust"
            })?,
        ],
        "fd" => vec![covariance("nonrobust")?],
        "between" => vec![
            toggle_parameter("constant", true)?,
            choice_parameter("effects", "entity", &["entity", "time"])?,
        ],
        "dynamic" => vec![
            minimum_integer_parameter("max_instrument_lag", 3, 2)?,
            choice_parameter("covariance", "robust", &["robust", "nonrobust"])?,
        ],
        "unit_root" | "cointegration" => vec![
            nonnegative_integer_parameter("lags", 1)?,
            choice_parameter(
                "regression",
                "constant",
                if method == "unit_root" {
                    &["none", "constant", "trend"]
                } else {
                    &["constant", "trend"]
                },
            )?,
        ],
        _ => unreachable!(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, id, en, zh, aliases) in SPECS {
        let model = matches!(method, "fe" | "re" | "fd" | "between");
        let mut ports = vec![data_input(
            if method == "unit_root" { "series" } else { "y" },
            if method == "unit_root" { "Series" } else { "Y" },
            series_type()?,
        )?];
        if method != "unit_root" {
            ports.push(bounded_user_data_input(
                "x",
                "X",
                series_type()?,
                if method == "dynamic" { 0 } else { 1 },
                if method == "cointegration" {
                    Some(MAX_COINTEGRATION_PREDICTORS as u16)
                } else {
                    None
                },
            )?);
        }
        ports.extend([
            data_input("entity", "Entity", series_type()?)?,
            data_input("time", "Time", series_type()?)?,
            data_output(
                if model { "model" } else { "result" },
                if model { "Model" } else { "Result" },
                if model {
                    concrete("statistics.model.panel")?
                } else {
                    report_type()?
                },
            )?,
        ]);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.panel", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, parameters(method)?)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
        for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
            fragment.messages.extend([
                (locale, node_key_text(id, "title"), Text(title)),
                (locale, node_key_text(id, "documentation"), Text(title)),
                (locale, node_key_text(id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
    fragment.messages.extend([
        ("en-US", "parameters.statistics.max_instrument_lag.title".into(), Text("Maximum instrument lag")),
        ("zh-CN", "parameters.statistics.max_instrument_lag.title".into(), Text("工具变量最大滞后阶")),
        ("en-US", "parameters.statistics.max_instrument_lag.description".into(), Text("Collapsed response-level instruments from lag 2 to this lag; default 3, at least 2 and below the period count.")),
        ("zh-CN", "parameters.statistics.max_instrument_lag.description".into(), Text("使用因变量从第 2 阶到指定阶的折叠水平工具变量；默认 3，至少 2 且小于实际期数。")),
    ]);
    Ok(())
}
