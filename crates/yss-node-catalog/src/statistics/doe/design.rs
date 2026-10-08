use super::*;
const RESOLVER: &str = "yssbi.statistics.doe.schema.design";
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment
        .schema_resolvers
        .push(sid(RESOLVER, SchemaResolverId::new)?);
    for (id, en, zh, uniform) in [
        (
            "yssbi.statistics.doe.family",
            "DOE: full factorial design",
            "DOE 试验（全析因设计）",
            false,
        ),
        (
            "yssbi.statistics.doe.orthogonal",
            "Orthogonal design (equal levels)",
            "正交设计（等水平）",
            false,
        ),
        (
            "yssbi.statistics.doe.uniform_design",
            "Uniform design (centered Latin hypercube)",
            "均匀设计（中心拉丁超立方）",
            true,
        ),
    ] {
        let mut parameters = vec![positive_integer_parameter("factors", 3)?];
        if uniform {
            parameters.extend([
                positive_integer_parameter("runs", 12)?,
                positive_integer_parameter("candidates", 32)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]);
        } else {
            parameters.push(minimum_integer_parameter("levels", 2, 2)?);
        }
        for parameter in &mut parameters {
            let (en_title, zh_title, en_help, zh_help) = match parameter.key.as_str() {
                "factors" => (
                    "Factors",
                    "因素数",
                    "One or more factors; output has one numeric column per factor.",
                    "至少一个因素，每个因素输出一列。",
                ),
                "levels" => (
                    "Levels per factor",
                    "每个因素的水平数",
                    "At least two equally coded levels. Composite levels use full factorial construction for orthogonal balance.",
                    "至少两个水平。正交设计在非质数水平下使用全析因构造，试验次数可能较多。",
                ),
                "runs" => (
                    "Experimental runs",
                    "试验次数",
                    "Every factor uses each coded level from 1 to runs exactly once.",
                    "每个因素的 1 至试验次数各水平恰好使用一次。",
                ),
                "candidates" => (
                    "Candidate designs",
                    "候选设计数",
                    "Choose the smallest centered L2 discrepancy among this many randomized Latin hypercubes; not a global optimality guarantee.",
                    "从这些随机拉丁超立方中选取中心化 L2 偏差最小者，不保证全局最优。",
                ),
                _ => (
                    "Random seed",
                    "随机种子",
                    "Reproducible candidate generation.",
                    "用于复现候选设计。",
                ),
            };
            parameter.title_key =
                node_key(id, &format!("parameters.{}.title", parameter.key.as_str()))?;
            parameter.description_key = Some(node_key(
                id,
                &format!("parameters.{}.description", parameter.key.as_str()),
            )?);
            for (locale, title, help) in
                [("en-US", en_title, en_help), ("zh-CN", zh_title, zh_help)]
            {
                fragment.messages.extend([
                    (
                        locale,
                        parameter.title_key.as_str().to_string(),
                        Text(title),
                    ),
                    (
                        locale,
                        parameter
                            .description_key
                            .as_ref()
                            .unwrap()
                            .as_str()
                            .to_string(),
                        Text(help),
                    ),
                ]);
            }
        }
        let mut design = data_output("design", "Coded design", concrete("tabular.dataframe")?)?;
        design.schema = Some(SchemaExpr::Derived {
            resolver: sid(RESOLVER, SchemaResolverId::new)?,
            dependencies: vec![SchemaDependency::Parameter(sid(
                "factors",
                ParameterKey::new,
            )?)],
        });
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: None,
                    category_id: sid("statistics.design_quality", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(
                    id,
                    vec![
                        data_output("result", "Design summary", report_type()?)?,
                        design,
                    ],
                    vec![],
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
        for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
            fragment.messages.extend([
                (locale, node_key_text(id, "title"), Text(title)),
                (locale, node_key_text(id, "documentation"), Text(title)),
            ]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_node_protocol::{ParameterIssueKind, ParameterValues, validate_parameter_values};

    #[test]
    fn factorial_and_orthogonal_parameters_reject_single_level_factors() {
        let system = crate::build_builtin_node_system().unwrap();
        for id in [
            "yssbi.statistics.doe.family",
            "yssbi.statistics.doe.orthogonal",
        ] {
            let protocol = system.registry.protocol(&id.parse().unwrap()).unwrap();
            let mut values = ParameterValues::new();
            let levels = ParameterKey::new("levels").unwrap();
            values.insert(levels.clone(), 1.into());
            let issues = validate_parameter_values(protocol, &values, system.registry.as_ref());
            assert!(
                issues.iter().any(|issue| issue.key.as_str() == "levels"
                    && matches!(issue.kind, ParameterIssueKind::Constraint)),
                "{id} must reject the single-level design that SCI cannot execute"
            );

            values.insert(levels, 2.into());
            assert!(
                validate_parameter_values(protocol, &values, system.registry.as_ref()).is_empty()
            );
        }
    }
}
