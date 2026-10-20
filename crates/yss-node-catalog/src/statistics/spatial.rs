//! Executable spatial-analysis protocols and weight design objects.
use super::*;

const METHODS: &[&str] = &[
    "weights", "moran", "ols", "slm", "sem", "sac", "sdm", "sdem", "slx", "panel",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
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
        ports.push(data_input("units", "Unit identifier", label_series()?)?);
        if weights {
            ports.extend([
                data_input("x", "X coordinate", numeric_data_series_type())?,
                data_input("y", "Y coordinate", numeric_data_series_type())?,
            ]);
        } else {
            if method == "panel" {
                ports.push(data_input("periods", "Period identifier", label_series()?)?);
            }
            ports.push(data_input("y", "Y", numeric_data_series_type())?);
            if method != "moran" {
                ports.push(bounded_user_data_input(
                    "x",
                    "X",
                    numeric_data_series_type(),
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
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.spatial", NodeCategoryId::new)?,
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
