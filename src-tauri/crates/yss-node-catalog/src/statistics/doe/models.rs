use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (id, en, zh, surface) in [
        (
            "yssbi.statistics.doe.response_surface",
            "Response surface (full quadratic)",
            "响应面分析（完整二次模型）",
            true,
        ),
        (
            "yssbi.statistics.doe.dose_response",
            "Dose response (four-parameter log-logistic)",
            "剂量反应（四参数对数逻辑曲线）",
            false,
        ),
    ] {
        let mut ports = vec![data_input("response", "Response", series_type()?)?];
        ports.push(if surface {
            bounded_user_data_input("factors", "Continuous factor", series_type()?, 1, None)?
        } else {
            data_input("dose", "Nonnegative dose", series_type()?)?
        });
        ports.extend([
            data_output("result", "Fit and interpretation", report_type()?)?,
            fixed_numeric_table(
                "observations",
                "Observed and fitted responses",
                &["observation", "response", "fitted", "residual"],
            )?,
        ]);
        let mut parameters = if surface {
            vec![]
        } else {
            vec![
                positive_integer_parameter("max_iterations", 500)?,
                decimal_parameter("tolerance", "0.0000001")?,
            ]
        };
        for parameter in &mut parameters {
            let (en_title, zh_title, en_help, zh_help) =
                if parameter.key.as_str() == "max_iterations" {
                    (
                        "Maximum iterations",
                        "最大迭代次数",
                        "The nonlinear solver fails explicitly if it does not converge.",
                        "非线性求解不收敛时明确返回失败。",
                    )
                } else {
                    (
                        "Convergence tolerance",
                        "收敛容差",
                        "Relative nonlinear solver tolerance, from 1e-12 to 0.01.",
                        "非线性求解相对收敛容差，范围为 1e-12 至 0.01。",
                    )
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
                interface: assembled_interface(id, ports, vec![], vec![])?,
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
