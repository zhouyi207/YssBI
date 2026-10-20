//! Survey interfaces; sampling weights are distinct from precision and treatment weights.
use super::*;
mod parameters;
const METHODS: &[(&str, bool)] = &[
    ("weights", false),
    ("mean_proportion", false),
    ("stratified", false),
    ("clustered", false),
    ("linear_regression", true),
    ("logistic", true),
    ("poisson", true),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, regression) in METHODS {
        let id = format!("yssbi.statistics.survey.{method}");
        let ports = if method == "weights" {
            vec![
                data_input(
                    "values",
                    "Weights or inclusion probabilities",
                    numeric_data_series_type(),
                )?,
                data_output("result", "Weight summary", report_type()?)?,
                data_output(
                    "weights",
                    "Validated sampling weights",
                    numeric_data_series_type(),
                )?,
            ]
        } else {
            estimate_ports(method, regression)?
        };
        let parameters = parameters::build(&id, method, regression)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid("statistics.survey", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
    }
    Ok(())
}
fn estimate_ports(method: &str, regression: bool) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    let response = data_series_type(TypeExpr::Union(vec![
        concrete("core.numeric")?,
        concrete("core.binary")?,
    ]));
    let mut ports = vec![
        data_input("y", "Y", response)?,
        data_input("weights", "Sampling weights", numeric_data_series_type())?,
    ];
    for (key, title, required) in [
        ("strata", "Stratum", method == "stratified"),
        (
            "clusters",
            "Primary sampling unit (PSU)",
            method == "clustered",
        ),
    ] {
        ports.push(if required {
            data_input(key, title, label_series()?)?
        } else {
            bounded_user_data_input(key, title, label_series()?, 0, Some(1))?
        });
    }
    if regression {
        ports.push(bounded_user_data_input(
            "x",
            "X",
            numeric_data_series_type(),
            0,
            None,
        )?);
    }
    ports.push(data_output(
        "result",
        "Survey estimate and design",
        report_type()?,
    )?);
    if regression {
        ports.push(fixed_numeric_table(
            "observations",
            "Observed and fitted responses",
            &["observation", "response", "fitted", "residual"],
        )?);
    }
    Ok(ports)
}
