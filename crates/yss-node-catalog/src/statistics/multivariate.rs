//! Executable multivariate analyses and parameter-derived coordinate tables.
use super::*;

const RESOLVER: &str = "yssbi.statistics.multivariate.schema.coordinates";
const SPECS: &[(&str, &str, &str)] = &[
    (
        "yssbi.statistics.association.canonical",
        "Canonical correlation",
        "典型相关分析",
    ),
    (
        "yssbi.statistics.multivariate.exploratory_factor",
        "Exploratory factor analysis",
        "探索性因子分析",
    ),
    (
        "yssbi.statistics.multivariate.pca",
        "Principal component analysis",
        "主成分分析",
    ),
    (
        "yssbi.statistics.multivariate.correspondence",
        "Correspondence analysis",
        "对应分析",
    ),
    (
        "yssbi.statistics.multivariate.discriminant",
        "Discriminant analysis",
        "判别分析",
    ),
    (
        "yssbi.statistics.multivariate.rda",
        "Redundancy analysis (RDA)",
        "冗余分析（RDA）",
    ),
    (
        "yssbi.statistics.multivariate.mds",
        "Multidimensional scaling (MDS)",
        "多维尺度分析（MDS）",
    ),
];
fn numeric_group(
    key: &'static str,
    label: &'static str,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    bounded_user_data_input(key, label, series_type()?, min, None)
}
fn coordinate_output(
    key: &'static str,
    label: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut output = data_output(key, label, concrete("tabular.dataframe")?)?;
    output.schema = Some(SchemaExpr::Derived {
        resolver: sid(RESOLVER, SchemaResolverId::new)?,
        dependencies: vec![SchemaDependency::Parameter(sid(
            "components",
            ParameterKey::new,
        )?)],
    });
    Ok(output)
}
fn condition(key: &str, value: &str) -> Result<ParameterCondition, BuiltinAssemblyError> {
    Ok(ParameterCondition {
        key: sid(key, ParameterKey::new)?,
        values: vec![DataValue::String(value.into())].into_boxed_slice(),
    })
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment
        .schema_resolvers
        .push(sid(RESOLVER, SchemaResolverId::new)?);
    for &(id, en, zh) in SPECS {
        let method = id.rsplit('.').next().unwrap();
        let label = TypeExpr::Generic(sid("class_label", TypeParameterId::new)?);
        let mut ports = match method {
            "canonical" => vec![
                numeric_group("x", "X variable", 1)?,
                numeric_group("y", "Y variable", 1)?,
            ],
            "correspondence" => vec![numeric_group("columns", "Count column", 2)?],
            "discriminant" => vec![
                data_input("groups", "Class", data_series_type(label.clone()))?,
                numeric_group("variables", "Training variable", 1)?,
                numeric_group("new_variables", "New variable", 0)?,
            ],
            "rda" => vec![
                numeric_group("y", "Y", 1)?,
                numeric_group("constraints", "Constraint", 1)?,
            ],
            "mds" => vec![numeric_group("variables", "Variable / distance column", 1)?],
            _ => vec![numeric_group(
                "variables",
                "Variable",
                if method == "exploratory_factor" { 3 } else { 2 },
            )?],
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        match method {
            "correspondence" => {
                ports.push(coordinate_output("row_coordinates", "Row coordinates")?);
                ports.push(coordinate_output(
                    "column_coordinates",
                    "Column coordinates",
                )?);
            }
            "discriminant" => ports.push(data_output(
                "predictions",
                "Predicted class",
                data_series_type(label),
            )?),
            "mds" => ports.push(coordinate_output("coordinates", "Coordinates")?),
            _ => ports.push(coordinate_output("scores", "Scores")?),
        }
        let default_components = if matches!(
            method,
            "canonical" | "exploratory_factor" | "correspondence" | "rda"
        ) {
            1
        } else {
            2
        };
        let mut parameters = if method == "discriminant" {
            vec![
                choice_parameter("discriminant_method", "linear", &["linear", "quadratic"])?,
                choice_parameter("class_priors", "empirical", &["empirical", "equal"])?,
                decimal_parameter("shrinkage", "0")?,
            ]
        } else {
            vec![positive_integer_parameter(
                "components",
                default_components,
            )?]
        };
        match method {
            "pca" => parameters.push(toggle_parameter("standardize", true)?),
            "exploratory_factor" => parameters.extend([
                choice_parameter("rotation", "varimax", &["none", "varimax"])?,
                positive_integer_parameter("max_iterations", 500)?,
                decimal_parameter("tolerance", "0.000001")?,
            ]),
            "rda" => parameters.extend([
                toggle_parameter("standardize", false)?,
                bounded_integer_parameter("permutations", 199, 0, 9999)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]),
            "mds" => {
                parameters.push(choice_parameter(
                    "input_kind",
                    "observations",
                    &["observations", "dissimilarity_matrix"],
                )?);
                parameters.push(
                    toggle_parameter("standardize", false)?
                        .when(condition("input_kind", "observations")?),
                );
            }
            _ => {}
        }
        // Per-node text avoids changing shared regression parameter descriptions.
        for parameter in &mut parameters {
            let key = parameter.key.as_str();
            let (en, zh, eh, zhh) = parameter_text(key);
            parameter.title_key = node_key(id, &format!("parameters.{key}.title"))?;
            parameter.description_key =
                Some(node_key(id, &format!("parameters.{key}.description"))?);
            for (locale, title, help) in [("en-US", en, eh), ("zh-CN", zh, zhh)] {
                fragment.messages.extend([
                    (locale, parameter.title_key.as_str().to_owned(), Text(title)),
                    (
                        locale,
                        parameter
                            .description_key
                            .as_ref()
                            .unwrap()
                            .as_str()
                            .to_owned(),
                        Text(help),
                    ),
                ]);
            }
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.multivariate", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(
                    id,
                    ports,
                    if method == "discriminant" {
                        vec![sid("class_label", TypeParameterId::new)?]
                    } else {
                        vec![]
                    },
                    vec![],
                )?,
                parameters: assembled_parameters(id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
        for (locale, title, help) in [
            (
                "en-US",
                en,
                "Analyze aligned numerical variables; inspect the summary and connect the numerical output. See node help for model assumptions.",
            ),
            (
                "zh-CN",
                zh,
                "分析对齐的数值变量，查看摘要并连接计算输出；方法假设见节点帮助。",
            ),
        ] {
            fragment.messages.extend([
                (locale, node_key_text(id, "title"), Text(title)),
                (locale, node_key_text(id, "documentation"), Text(help)),
                (
                    locale,
                    node_key_text(id, "aliases"),
                    Aliases(aliases(method)),
                ),
            ]);
        }
    }
    Ok(())
}
fn aliases(method: &str) -> &'static [&'static str] {
    match method {
        "canonical" => &["association.canonical", "canonical correlation", "典型相关"],
        "pca" => &["multivariate.pca", "PCA", "主成分"],
        "exploratory_factor" => &[
            "multivariate.exploratory_factor",
            "EFA",
            "探索性因子",
            "主轴因子",
            "varimax",
        ],
        "correspondence" => &[
            "multivariate.correspondence",
            "correspondence analysis",
            "对应分析",
        ],
        "discriminant" => &["multivariate.discriminant", "LDA", "QDA", "判别分析"],
        "rda" => &["multivariate.rda", "RDA", "冗余分析"],
        "mds" => &["multivariate.mds", "MDS", "多维尺度", "classical MDS"],
        _ => unreachable!(),
    }
}
fn parameter_text(key: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match key {
        "components" => (
            "Retained dimensions",
            "保留维数",
            "Positive retained dimension count, no greater than the method's available dimensions.",
            "保留维数须为正，且不能超过该方法可用的维数。",
        ),
        "standardize" => (
            "Standardize variables",
            "标准化变量",
            "Center variables and divide by their sample standard deviations; otherwise use original units.",
            "按样本标准差标准化变量；关闭时使用原始单位。",
        ),
        "rotation" => (
            "Factor rotation",
            "因子旋转",
            "Orthogonal varimax or no rotation; varimax does not apply Kaiser normalization.",
            "选择正交 varimax 或不旋转；varimax 不使用 Kaiser 归一化。",
        ),
        "max_iterations" => (
            "Maximum iterations",
            "最大迭代次数",
            "Principal-axis and rotation iteration limit; default 500, positive integer.",
            "主轴因子与旋转的迭代上限，默认 500，正整数。",
        ),
        "tolerance" => (
            "Convergence tolerance",
            "收敛容差",
            "Positive tolerance no greater than 0.1; default 1e-6.",
            "正容差且不超过 0.1，默认 1e-6。",
        ),
        "discriminant_method" => (
            "Discriminant model",
            "判别模型",
            "Linear pooled covariance or quadratic class-specific covariances.",
            "选择线性共同协方差模型或二次类别专属协方差模型。",
        ),
        "class_priors" => (
            "Class priors",
            "类别先验",
            "Use observed class frequencies or equal class probabilities.",
            "使用训练类别频数占比或各类别等概率先验。",
        ),
        "shrinkage" => (
            "Covariance shrinkage",
            "协方差收缩",
            "A value from 0 to 1 toward an isotropic target in standardized feature units.",
            "取值 0–1，按标准化变量单位向各向同性目标收缩。",
        ),
        "permutations" => (
            "Permutation draws",
            "置换次数",
            "0 disables inference; otherwise 1–9999 row permutations with a plus-one p-value.",
            "0 关闭推断；否则使用 1–9999 次行置换和加一修正的 p 值。",
        ),
        "seed" => (
            "Random seed",
            "随机种子",
            "Nonnegative seed for reproducible permutation inference.",
            "用于可复现置换推断的非负随机种子。",
        ),
        "input_kind" => (
            "MDS input",
            "MDS 输入形式",
            "Observation variables or a nonnegative symmetric square dissimilarity matrix.",
            "选择观测变量或非负、对称的方形相异度矩阵。",
        ),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multivariate_catalog_has_registered_score_schemas_and_bilingual_method_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for locale in ["en-US", "zh-CN"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for &(id, _, _) in SPECS {
                let protocol = system.registry.protocol(&id.parse().unwrap()).unwrap();
                let item = catalog
                    .items
                    .iter()
                    .find(|item| item.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "statistics.multivariate");
                assert!(
                    item.documentation.as_deref().unwrap().contains("$$"),
                    "{id} {locale}"
                );
                let outputs = protocol
                    .interface
                    .ports
                    .iter()
                    .filter(|port| port.direction == PortDirection::Output)
                    .collect::<Vec<_>>();
                assert_eq!(outputs[0].key.as_str(), "result");
                if id.ends_with("discriminant") {
                    assert_eq!(outputs[1].key.as_str(), "predictions");
                    assert_eq!(protocol.interface.type_parameters.len(), 1);
                } else {
                    for output in &outputs[1..] {
                        assert!(
                            matches!(output.schema.as_ref(),Some(SchemaExpr::Derived{resolver,..})if resolver.as_str()==RESOLVER)
                        );
                    }
                }
            }
        }
    }
}
