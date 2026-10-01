//! Executable variance-analysis nodes retain their inventory identities.
use super::*;

const METHODS: &[(&str, &str, &str)] = &[
    ("one_way", "One-way ANOVA", "单因素方差分析"),
    ("two_way", "Two-way ANOVA", "双因素方差分析"),
    ("three_way", "Three-way ANOVA", "三因素方差分析"),
    ("factorial", "Factorial ANOVA", "多因素方差分析"),
    (
        "ancova",
        "Analysis of covariance (ANCOVA)",
        "协方差分析（ANCOVA）",
    ),
    (
        "manova",
        "Multivariate ANOVA (MANOVA)",
        "多元方差分析（MANOVA）",
    ),
    (
        "repeated_measures",
        "Repeated-measures ANOVA",
        "重复测量方差分析",
    ),
];

pub(super) fn implemented(id: &str) -> bool {
    id.strip_prefix("yssbi.statistics.anova.")
        .is_some_and(|method| METHODS.iter().any(|spec| spec.0 == method))
}

fn label_series() -> Result<TypeExpr, BuiltinAssemblyError> {
    let members = [
        "core.numeric",
        "core.categorical",
        "core.ordinal",
        "core.binary",
        "core.text",
        "core.identifier",
    ]
    .iter()
    .map(|id| concrete(id).map(data_series_type))
    .collect::<Result<Vec<_>, _>>()?;
    normalize_type_expr(TypeExpr::Union(members)).map_err(|error| {
        BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "ANOVA factor input",
            value: error.to_string().into(),
        }
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in METHODS {
        let id = format!("yssbi.statistics.anova.{method}");
        let mut ports = if method == "manova" {
            vec![bounded_user_data_input(
                "responses",
                "Response",
                series_type()?,
                2,
                None,
            )?]
        } else {
            vec![data_input("response", "Response", series_type()?)?]
        };
        if method == "repeated_measures" {
            ports.push(data_input("subjects", "Subject", label_series()?)?);
        }
        let (min, max) = match method {
            "one_way" => (1, Some(1)),
            "two_way" => (2, Some(2)),
            "three_way" => (3, Some(3)),
            _ => (1, None),
        };
        ports.push(bounded_user_data_input(
            "factors",
            "Factor",
            label_series()?,
            min as u16,
            max,
        )?);
        if method == "ancova" {
            ports.push(bounded_user_data_input(
                "covariates",
                "Covariate",
                series_type()?,
                1,
                None,
            )?);
        }
        ports.push(data_output("result", "Result", report_type()?)?);
        let parameters = match method {
            "one_way" => vec![],
            "repeated_measures" => vec![choice_parameter(
                "sphericity_correction",
                "greenhouse_geisser",
                &["none", "greenhouse_geisser"],
            )?],
            _ => vec![
                choice_parameter(
                    "model_terms",
                    "full_factorial",
                    &["main_effects", "full_factorial"],
                )?,
                choice_parameter(
                    "sums_of_squares",
                    "type_iii",
                    &["type_i", "type_ii", "type_iii"],
                )?,
            ],
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.anova", NodeCategoryId::new)?,
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
            "one_way" => &["anova.one_way", "ANOVA", "单因素方差", "方差"],
            "two_way" => &["anova.two_way", "two way ANOVA", "双因素方差"],
            "three_way" => &["anova.three_way", "three way ANOVA", "三因素方差"],
            "factorial" => &[
                "anova.factorial",
                "factorial ANOVA",
                "多因素方差",
                "交互效应",
            ],
            "ancova" => &["anova.ancova", "ANCOVA", "协方差分析"],
            "manova" => &["anova.manova", "MANOVA", "多元方差"],
            "repeated_measures" => &[
                "anova.repeated_measures",
                "repeated measures ANOVA",
                "重复测量方差",
                "Greenhouse-Geisser",
            ],
            _ => unreachable!(),
        };
        for (locale, title, help) in [
            (
                "en-US",
                en,
                "Analyze aligned responses and categorical factors. See node help for the design and inference assumptions.",
            ),
            (
                "zh-CN",
                zh,
                "分析对齐的响应变量与分类因素；设计要求与推断假设见节点帮助。",
            ),
        ] {
            fragment.messages.extend([
                (locale, node_key_text(&id, "title"), Text(title)),
                (locale, node_key_text(&id, "documentation"), Text(help)),
                (locale, node_key_text(&id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
    for (key, en, zh, en_help, zh_help) in [
        (
            "model_terms",
            "Factor terms",
            "因素效应",
            "Full factorial includes all factor interactions; main effects includes only additive factor effects. Default: full factorial.",
            "完整因子模型包含所有因素交互；主效应模型只包含各因素的加性效应。默认完整因子模型。",
        ),
        (
            "sums_of_squares",
            "Sums of squares",
            "平方和类型",
            "I: sequential (covariates first); II: adjust for other terms except higher-order relatives; III: adjust for every other term, with sum contrasts. Default: III.",
            "I：顺序检验（协变量优先）；II：调整除本项高阶关联项外的其他项；III：使用和为零对比，调整所有其他项。默认 III。",
        ),
        (
            "sphericity_correction",
            "Sphericity correction",
            "球形性校正",
            "Apply Greenhouse-Geisser epsilon to each within-subject effect's degrees of freedom, or use uncorrected inference. Default: Greenhouse-Geisser.",
            "对每个受试者内效应的自由度应用 Greenhouse-Geisser 校正，或使用未校正推断。默认 Greenhouse-Geisser。",
        ),
    ] {
        for (locale, title, help) in [("en-US", en, en_help), ("zh-CN", zh, zh_help)] {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anova_catalog_registers_complete_interfaces_defaults_and_bilingual_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for locale in ["en-US", "zh-CN"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for &(method, _, _) in METHODS {
                let id = format!("yssbi.statistics.anova.{method}");
                let protocol = system
                    .registry
                    .protocol(&id.as_str().parse().unwrap())
                    .unwrap();
                assert_eq!(
                    protocol
                        .interface
                        .ports
                        .iter()
                        .filter(|port| port.direction == PortDirection::Output)
                        .count(),
                    1
                );
                assert_eq!(
                    protocol.interface.ports.last().unwrap().key.as_str(),
                    "result"
                );
                let item = catalog
                    .items
                    .iter()
                    .find(|item| item.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "statistics.anova");
                let help = item.documentation.as_deref().unwrap();
                assert!(help.contains("$$"), "{id} {locale}");
                assert!(!help.contains("范围待确认"));
                for (key, expected) in if method == "one_way" {
                    vec![]
                } else if method == "repeated_measures" {
                    vec![("sphericity_correction", "greenhouse_geisser")]
                } else {
                    vec![
                        ("model_terms", "full_factorial"),
                        ("sums_of_squares", "type_iii"),
                    ]
                } {
                    let field = protocol
                        .parameters
                        .iter()
                        .find(|p| p.key.as_str() == key)
                        .unwrap();
                    assert_eq!(
                        field.default_value.as_ref().map(|default| &default.value),
                        Some(&DataValue::String(expected.into()))
                    );
                }
            }
        }
    }
}
