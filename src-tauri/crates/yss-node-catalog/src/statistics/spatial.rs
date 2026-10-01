//! Executable spatial-analysis protocols and weight design objects.
use super::*;

const METHODS: &[(&str, &str, &str, &[&str])] = &[
    (
        "weights",
        "Spatial weights",
        "空间权重构造",
        &["spatial.weights", "空间权重构造"],
    ),
    (
        "moran",
        "Moran's I",
        "莫兰指数",
        &["spatial.moran", "莫兰指数", "Moran"],
    ),
    (
        "ols",
        "Spatial OLS regression",
        "空间 OLS 回归",
        &["spatial.ols", "空间OLS回归"],
    ),
    (
        "slm",
        "Spatial lag model (SLM)",
        "空间滞后 SLM",
        &["spatial.slm", "空间滞后SLM", "SAR"],
    ),
    (
        "sem",
        "Spatial error model (SEM)",
        "空间误差 SEM",
        &["spatial.sem", "空间误差SEM"],
    ),
    (
        "sac",
        "Spatial lag/error model (SAC)",
        "空间滞后误差 SAC",
        &["spatial.sac", "空间滞后误差模型SAC", "SARAR"],
    ),
    (
        "sdm",
        "Spatial Durbin model (SDM)",
        "空间杜宾 SDM",
        &["spatial.sdm", "空间杜宾SDM"],
    ),
    (
        "sdem",
        "Spatial Durbin error model (SDEM)",
        "空间杜宾误差 SDEM",
        &["spatial.sdem", "空间杜宾误差SDEM"],
    ),
    (
        "slx",
        "Spatially lagged X (SLX)",
        "自变量空间滞后 SLX",
        &["spatial.slx", "自变量空间滞后SLX"],
    ),
    (
        "panel",
        "Spatial panel: entity fixed effects",
        "空间面板模型（实体固定效应）",
        &["spatial.panel", "空间面板模型"],
    ),
];
pub(super) fn implemented(id: &str) -> bool {
    id.strip_prefix("yssbi.statistics.spatial.")
        .is_some_and(|s| METHODS.iter().any(|m| m.0 == s))
}
fn labels() -> Result<TypeExpr, BuiltinAssemblyError> {
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
        context: "spatial identifiers",
        value: e.to_string().into(),
    })
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh, aliases) in METHODS {
        let id = format!("yssbi.statistics.spatial.{method}");
        let weights = method == "weights";
        let mut ports = vec![];
        if !weights {
            ports.push(data_input(
                "weights",
                "Spatial weights",
                concrete("statistics.design.spatial_weights")?,
            )?);
        }
        ports.push(data_input("units", "Unit identifier", labels()?)?);
        if weights {
            ports.extend([
                data_input("x", "X coordinate", series_type()?)?,
                data_input("y", "Y coordinate", series_type()?)?,
            ]);
        } else {
            if method == "panel" {
                ports.push(data_input("periods", "Period identifier", labels()?)?);
            }
            ports.push(data_input("response", "Response", series_type()?)?);
            if method != "moran" {
                ports.push(bounded_user_data_input(
                    "predictors",
                    "Predictor",
                    series_type()?,
                    1,
                    None,
                )?);
            }
        }
        ports.push(data_output(
            "result",
            "Result",
            if weights {
                concrete("statistics.design.spatial_weights")?
            } else {
                report_type()?
            },
        )?);
        let parameters = if weights {
            vec![
                choice_parameter(
                    "spatial_weight_rule",
                    "knn",
                    &["knn", "distance_band", "inverse_distance"],
                )?,
                minimum_integer_parameter("spatial_neighbors", 4, 1)?,
                decimal_parameter("spatial_radius", "1")?,
                decimal_parameter("spatial_power", "1")?,
                toggle_parameter("spatial_symmetrize", false)?,
                toggle_parameter("spatial_row_standardize", true)?,
            ]
        } else if method == "moran" {
            vec![
                nonnegative_integer_parameter("spatial_permutations", 999)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]
        } else {
            let mut p = if method == "panel" {
                vec![choice_parameter(
                    "spatial_panel_model",
                    "slm",
                    &["slm", "sem"],
                )?]
            } else {
                vec![toggle_parameter("constant", true)?]
            };
            if !matches!(method, "ols" | "slx") {
                p.extend([
                    minimum_integer_parameter("max_iterations", 500, 1)?,
                    tolerance_parameter("0.0000001")?,
                ]);
            }
            p
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.spatial", NodeCategoryId::new)?,
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
            "types.statistics_spatial_weights.title".into(),
            Text("Spatial weights design"),
        ),
        (
            "zh-CN",
            "types.statistics_spatial_weights.title".into(),
            Text("空间权重设计"),
        ),
    ]);
    for (key, en, zh, en_description, zh_description) in [
        (
            "spatial_weight_rule",
            "Neighbor rule",
            "邻接规则",
            "KNN (default), distance band, or inverse distance within the radius; planar Euclidean coordinates.",
            "默认 K 近邻；也支持距离阈值或阈值内反距离，使用平面欧氏坐标。",
        ),
        (
            "spatial_neighbors",
            "Nearest neighbors",
            "近邻数",
            "Default 4; at least 1 and smaller than the unit count. Used by KNN.",
            "默认 4，至少 1 且小于地区数；用于 K 近邻。",
        ),
        (
            "spatial_radius",
            "Distance threshold",
            "距离阈值",
            "Positive distance in coordinate units; default 1. Used by distance band and inverse distance.",
            "坐标单位下的正距离，默认 1；用于距离阈值和反距离。",
        ),
        (
            "spatial_power",
            "Inverse-distance power",
            "反距离幂次",
            "Positive exponent p in distance^(-p); default 1.",
            "反距离权重 distance^(-p) 中的正幂次，默认 1。",
        ),
        (
            "spatial_symmetrize",
            "Symmetrize neighbors",
            "邻接对称化",
            "Default false. Take the union of directed neighbors before standardization.",
            "默认关闭；标准化之前取双向邻接的并集。",
        ),
        (
            "spatial_row_standardize",
            "Row standardization",
            "行标准化",
            "Default true. Divide each nonzero row by its sum; islands remain zero rows.",
            "默认开启；非零行除以行和，孤立地区保留为零行。",
        ),
        (
            "spatial_permutations",
            "Moran permutations",
            "Moran 置换次数",
            "Default 999; 0 disables permutation inference. Two-sided deviations from the null expectation, with a plus-one p-value.",
            "默认 999；0 关闭置换推断。以偏离零假设期望的绝对值作双侧比较，p 值采用加一校正。",
        ),
        (
            "spatial_panel_model",
            "Panel spatial structure",
            "面板空间结构",
            "SLM (default) or SEM; balanced panel, entity fixed effects removed by orthonormal time contrasts.",
            "默认 SLM，也可选 SEM；平衡面板，通过正交时间对比消去实体固定效应。",
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
