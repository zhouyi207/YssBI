//! Complete the existing panel-category entries without duplicating their identities.
use super::*;
use yss_sci_contract::panel::MAX_COINTEGRATION_PREDICTORS;

const SPECS: &[(&str, &str)] = &[
    ("fe", "yssbi.statistics.econometrics.panel.fe"),
    ("re", "yssbi.statistics.econometrics.panel.re"),
    ("fd", "yssbi.statistics.econometrics.panel.fd"),
    ("between", "yssbi.statistics.econometrics.panel.between"),
    ("dynamic", "yssbi.statistics.econometrics.panel.dynamic"),
    ("unit_root", "yssbi.statistics.econometrics.panel.unit_root"),
    (
        "cointegration",
        "yssbi.statistics.econometrics.panel.cointegration",
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
    for &(method, id) in SPECS {
        let model = matches!(method, "fe" | "re" | "fd" | "between");
        let mut ports = vec![data_input(
            if method == "unit_root" { "series" } else { "y" },
            if method == "unit_root" { "Series" } else { "Y" },
            numeric_data_series_type(),
        )?];
        if method != "unit_root" {
            ports.push(bounded_user_data_input(
                "x",
                "X",
                numeric_data_series_type(),
                if method == "dynamic" { 0 } else { 1 },
                if method == "cointegration" {
                    Some(MAX_COINTEGRATION_PREDICTORS as u16)
                } else {
                    None
                },
            )?);
        }
        ports.extend([
            data_input("entity", "Entity", numeric_data_series_type())?,
            data_input("time", "Time", numeric_data_series_type())?,
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
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.panel", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, parameters(method)?)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
    }
    Ok(())
}
