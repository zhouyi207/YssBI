use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_decimal, assembled_interface,
    assembled_parameters, leaf, pure, sid,
};
use crate::builtin::node_key;
use crate::builtin::{category_id, concrete, data_port, icon_id, node_type_id, type_id};
use yss_data_contract::DataValue;
use yss_node_protocol::*;
use yss_node_registry::{CategoryRegistration, TypeRegistration};

const CATEGORY: &str = "plot";

#[derive(Clone, Copy)]
enum PlotInputs {
    Pair,
    NumericSeries,
    CorrelationSeries,
    Correlogram,
    Groups,
    Words,
    ErrorBars,
    Roc,
    Categories,
    Combination,
    Bubble,
    Model,
}

#[derive(Clone, Copy)]
struct PlotSpec {
    id: &'static str,
    inputs: PlotInputs,
}

const SPECS: &[PlotSpec] = &[
    PlotSpec {
        id: "yssbi.plot.scatter.view",
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.line.view",
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.ecdf.view",
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.kde.view",
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.histogram.view",
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.correlation.view",
        inputs: PlotInputs::CorrelationSeries,
    },
    PlotSpec {
        id: "yssbi.plot.correlogram.view",
        inputs: PlotInputs::Correlogram,
    },
    PlotSpec {
        id: "yssbi.plot.boxplot.view",
        inputs: PlotInputs::Groups,
    },
    PlotSpec {
        id: "yssbi.plot.wordcloud.view",
        inputs: PlotInputs::Words,
    },
    PlotSpec {
        id: "yssbi.plot.errorbar.view",
        inputs: PlotInputs::ErrorBars,
    },
    PlotSpec {
        id: "yssbi.plot.pp_qq.view",
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.roc.view",
        inputs: PlotInputs::Roc,
    },
    PlotSpec {
        id: "yssbi.plot.quadrant.view",
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.pareto.view",
        inputs: PlotInputs::Categories,
    },
    PlotSpec {
        id: "yssbi.plot.combination.view",
        inputs: PlotInputs::Combination,
    },
    PlotSpec {
        id: "yssbi.plot.bubble.view",
        inputs: PlotInputs::Bubble,
    },
    PlotSpec {
        id: "yssbi.plot.violin.view",
        inputs: PlotInputs::Groups,
    },
    PlotSpec {
        id: "yssbi.plot.heatmap.view",
        inputs: PlotInputs::Groups,
    },
    PlotSpec {
        id: "yssbi.plot.coefficient.view",
        inputs: PlotInputs::Model,
    },
];

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut nodes = Vec::with_capacity(SPECS.len());
    let categories = vec![CategoryRegistration {
        id: category_id(CATEGORY)?,
        title_key: i18n_key("categories.plot.title")?,
        parent: None,
        order: 70,
    }];
    for spec in SPECS {
        nodes.push(leaf(protocol(spec)?, spec.id));
    }
    let fragment = ProviderFragment {
        categories,
        nodes,
        types: vec![TypeRegistration {
            id: type_id("plot.data")?,
            title_key: i18n_key("types.plot_data.title")?,
            classes: Default::default(),
        }],
        ..ProviderFragment::default()
    };
    Ok(fragment)
}

fn protocol(spec: &PlotSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let mut ports = Vec::new();
    match spec.inputs {
        PlotInputs::Pair => {
            ports.push(data_port(
                "x",
                "X",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?);
            ports.push(data_port(
                "y",
                "Y",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?);
        }
        PlotInputs::NumericSeries => ports.push(data_port(
            "values",
            "Values",
            PortDirection::Input,
            numeric_data_series_type(),
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?),
        PlotInputs::CorrelationSeries => ports.push(data_port(
            "series",
            "DataSeries",
            PortDirection::Input,
            numeric_data_series_type(),
            PortCardinality::UserCreated {
                min: 2,
                max: Some(64),
            },
            LiteralPolicy::Allowed,
        )?),
        PlotInputs::Correlogram => {
            ports.push(data_port(
                "values",
                "DataSeries",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?);
        }
        PlotInputs::Groups => ports.push(data_port(
            "series",
            "Series",
            PortDirection::Input,
            numeric_data_series_type(),
            PortCardinality::UserCreated {
                min: 1,
                max: Some(64),
            },
            LiteralPolicy::Allowed,
        )?),
        PlotInputs::Words | PlotInputs::Categories => ports.push(data_port(
            if matches!(spec.inputs, PlotInputs::Words) {
                "words"
            } else {
                "categories"
            },
            "Text",
            PortDirection::Input,
            data_series_type(concrete("core.text")?),
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?),
        PlotInputs::ErrorBars => {
            for (key, title) in [
                ("x", "X"),
                ("y", "Estimate"),
                ("lower", "Lower"),
                ("upper", "Upper"),
            ] {
                ports.push(data_port(
                    key,
                    title,
                    PortDirection::Input,
                    numeric_data_series_type(),
                    PortCardinality::Declared,
                    LiteralPolicy::Allowed,
                )?);
            }
        }
        PlotInputs::Roc => {
            ports.push(data_port(
                "labels",
                "Labels",
                PortDirection::Input,
                data_series_type(concrete("core.binary")?),
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?);
            ports.push(data_port(
                "scores",
                "Scores",
                PortDirection::Input,
                numeric_data_series_type(),
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?);
        }
        PlotInputs::Combination => {
            ports.push(data_port(
                "categories",
                "Categories",
                PortDirection::Input,
                data_series_type(concrete("core.text")?),
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?);
            for (key, title) in [("bars", "Bars"), ("line", "Line")] {
                ports.push(data_port(
                    key,
                    title,
                    PortDirection::Input,
                    numeric_data_series_type(),
                    PortCardinality::Declared,
                    LiteralPolicy::Allowed,
                )?);
            }
        }
        PlotInputs::Bubble => {
            for (key, title) in [("x", "X"), ("y", "Y"), ("size", "Size")] {
                ports.push(data_port(
                    key,
                    title,
                    PortDirection::Input,
                    numeric_data_series_type(),
                    PortCardinality::Declared,
                    LiteralPolicy::Allowed,
                )?);
            }
        }
        PlotInputs::Model => ports.push(data_port(
            "model",
            "Linear model",
            PortDirection::Input,
            concrete("statistics.model.linear")?,
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?),
    }
    ports.push(data_port(
        "result",
        "Result",
        PortDirection::Output,
        concrete("plot.data")?,
        PortCardinality::Declared,
        LiteralPolicy::Allowed,
    )?);
    Ok(NodeProtocol {
        type_id: node_type_id(spec.id)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: category_id(CATEGORY)?,
            icon_id: icon_id("builtin.plot")?,
            style_id: style_id("builtin.plot")?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters(spec)?)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: pure(),
        typing: NodeTypingSpec::Fixed,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    value: DataValue,
    constraints: Vec<ParameterConstraint>,
    editor: ParameterEditorSpec,
) -> Result<Parameter, BuiltinAssemblyError> {
    Ok(Parameter {
        key: sid(key, ParameterKey::new)?,
        title_key: i18n_key(format!("parameters.plot.{key}.title"))?,
        description_key: None,
        default_value: Some(TypedValue {
            value_type: value_type.clone(),
            value,
        }),
        value_type,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
        visible_when: None,
    })
}

fn integer_parameter(
    key: &'static str,
    default: i64,
    min: i64,
    max: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        DataValue::Integer(default),
        vec![ParameterConstraint::IntegerRange {
            min: Some(min),
            max: Some(max),
        }],
        ParameterEditorSpec::Number,
    )
}

fn number_parameter(
    key: &'static str,
    default: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        DataValue::Decimal(assembled_decimal("plot.parameter", default)?),
        if matches!(key, "reference_standard_deviation" | "confidence_level") {
            vec![ParameterConstraint::Positive]
        } else {
            vec![]
        },
        ParameterEditorSpec::Number,
    )
}

fn boolean_parameter(key: &'static str, default: bool) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.binary")?,
        DataValue::Bool(default),
        vec![],
        ParameterEditorSpec::Toggle,
    )
}

fn parameters(spec: &PlotSpec) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(match spec.id {
        "yssbi.plot.histogram.view" => vec![integer_parameter("bins", 0, 0, 128)?],
        "yssbi.plot.kde.view" => vec![integer_parameter("grid_points", 256, 16, 512)?],
        "yssbi.plot.correlogram.view" => vec![integer_parameter("maximum_lag", 20, 1, 40)?],
        "yssbi.plot.wordcloud.view" => vec![integer_parameter("max_words", 100, 1, 256)?],
        "yssbi.plot.pp_qq.view" => vec![
            parameter(
                "mode",
                concrete("core.text")?,
                DataValue::String("qq".into()),
                vec![ParameterConstraint::OneOf(vec![
                    DataValue::String("qq".into()),
                    DataValue::String("pp".into()),
                ])],
                ParameterEditorSpec::Select,
            )?,
            boolean_parameter("estimate_parameters", true)?,
            number_parameter("reference_mean", "0")?,
            number_parameter("reference_standard_deviation", "1")?,
        ],
        "yssbi.plot.quadrant.view" => vec![
            number_parameter("x_cut", "0")?,
            number_parameter("y_cut", "0")?,
        ],
        "yssbi.plot.combination.view" => vec![boolean_parameter("dual_axis", true)?],
        "yssbi.plot.coefficient.view" => vec![
            number_parameter("confidence_level", "0.95")?,
            boolean_parameter("include_intercept", false)?,
        ],
        _ => vec![],
    })
}

fn style_id(value: &'static str) -> Result<NodeStyleId, BuiltinAssemblyError> {
    sid(value, NodeStyleId::new)
}
use crate::builtin::iid as i18n_key;
