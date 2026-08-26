use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_interface, assembled_parameters, sid,
};
use super::localization::Message;
use crate::graph_document::PortRef;
use crate::node_system::compiler::{
    FragmentMetadata, FragmentResult, KernelFragment as LoweredKernelFragment, LoweredKernel,
    LoweredNode, LoweringContext, LoweringError, LoweringInvariant, NodeImplementation,
    NodeLowerer,
};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, KernelHandle, ResourceAccess, ResourceId,
    ResourceKind,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{CategoryRegistration, RegisteredNode};
use std::sync::Arc;

const CATEGORY: &str = "plot";

const PLOT_SINK: &str = "yssbi.runtime.plot_sink";

#[derive(Clone, Copy)]
enum PlotInputs {
    Pair,
    NumericSeries,
    CorrelationSeries,
    Correlogram,
}

#[derive(Clone, Copy)]
struct PlotSpec {
    id: &'static str,
    kernel: &'static str,
    en: &'static str,
    zh: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
    inputs: PlotInputs,
}

const SPECS: &[PlotSpec] = &[
    PlotSpec {
        id: "yssbi.plot.scatter.view",
        kernel: "yssbi.plot.scatter.view",
        en: "Scatter Plot",
        zh: "散点图",
        aliases: &["scatterplot", "XY plot", "points"],
        zh_aliases: &["散点图", "XY图", "点图"],
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.line.view",
        kernel: "yssbi.plot.line.view",
        en: "Line Plot",
        zh: "折线图",
        aliases: &["line chart", "time series plot", "curve"],
        zh_aliases: &["折线图", "时间序列图", "曲线图"],
        inputs: PlotInputs::Pair,
    },
    PlotSpec {
        id: "yssbi.plot.ecdf.view",
        kernel: "yssbi.plot.ecdf.view",
        en: "Empirical CDF",
        zh: "经验累积分布图",
        aliases: &["ECDF", "empirical cumulative distribution function", "CDF"],
        zh_aliases: &["经验分布函数", "累积分布", "ECDF"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.kde.view",
        kernel: "yssbi.plot.kde.view",
        en: "Kernel Density Estimate",
        zh: "核密度估计图",
        aliases: &[
            "KDE",
            "kernel density estimation",
            "density plot",
            "Silverman bandwidth",
        ],
        zh_aliases: &["核密度估计", "密度图", "KDE"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.histogram.view",
        kernel: "yssbi.plot.histogram.view",
        en: "Histogram",
        zh: "直方图",
        aliases: &["frequency distribution", "bins", "Sturges rule"],
        zh_aliases: &["频数分布", "分箱", "斯特吉斯规则"],
        inputs: PlotInputs::NumericSeries,
    },
    PlotSpec {
        id: "yssbi.plot.correlation.view",
        kernel: "yssbi.plot.correlation.view",
        en: "Correlation Plot",
        zh: "相关性图",
        aliases: &[
            "correlation matrix",
            "Pearson correlation",
            "p-value",
            "heatmap",
        ],
        zh_aliases: &["相关矩阵", "皮尔逊相关", "P值", "热力图"],
        inputs: PlotInputs::CorrelationSeries,
    },
    PlotSpec {
        id: "yssbi.plot.correlogram.view",
        kernel: "yssbi.plot.correlogram.view",
        en: "Correlogram (ACF & PACF)",
        zh: "相关图（ACF 与 PACF）",
        aliases: &["correlogram", "ACF", "PACF", "Ljung-Box", "autocorrelation"],
        zh_aliases: &["相关图", "自相关", "偏自相关", "Ljung-Box检验"],
        inputs: PlotInputs::Correlogram,
    },
];

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut nodes = Vec::with_capacity(SPECS.len());
    let mut messages = vec![
        ("en-US", "categories.plot.title", Message::Text("Plots")),
        ("zh-CN", "categories.plot.title", Message::Text("绘图")),
    ];
    let categories = vec![CategoryRegistration {
        id: category_id(CATEGORY)?,
        title_key: i18n_key("categories.plot.title")?,
        parent: None,
        order: 70,
    }];
    for spec in SPECS {
        add_messages(&mut messages, spec);
        nodes.push(RegisteredNode::leaf(
            Arc::new(protocol(spec)?),
            Arc::new(NodeImplementation::new(PlotLowerer {
                kernel: spec.kernel,
            })),
        ));
    }
    Ok(ProviderFragment {
        categories,
        nodes,
        messages,
        ..ProviderFragment::default()
    })
}

fn protocol(spec: &PlotSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let mut ports = vec![control_port("enter", "Enter", PortDirection::Input)?];
    match spec.inputs {
        PlotInputs::Pair => {
            ports.push(data_port(
                "x",
                "X",
                PortDirection::Input,
                numeric_data_series_type(),
                PortInstances::Declared,
                None,
            )?);
            ports.push(data_port(
                "y",
                "Y",
                PortDirection::Input,
                numeric_data_series_type(),
                PortInstances::Declared,
                None,
            )?);
        }
        PlotInputs::NumericSeries => ports.push(data_port(
            "values",
            "Values",
            PortDirection::Input,
            numeric_data_series_type(),
            PortInstances::Declared,
            None,
        )?),
        PlotInputs::CorrelationSeries => ports.push(data_port(
            "series",
            "DataSeries",
            PortDirection::Input,
            numeric_data_series_type(),
            PortInstances::UserCreated { min: 2, max: None },
            None,
        )?),
        PlotInputs::Correlogram => {
            ports.push(data_port(
                "values",
                "DataSeries",
                PortDirection::Input,
                numeric_data_series_type(),
                PortInstances::Declared,
                None,
            )?);
            ports.push(data_port(
                "maximum_lag",
                "Lags",
                PortDirection::Input,
                concrete("core.int64")?,
                PortInstances::Declared,
                Some(TypedValue {
                    value_type: concrete("core.int64")?,
                    value: Value::Integer(20),
                }),
            )?);
        }
    }
    ports.push(control_port("then", "Then", PortDirection::Output)?);
    ports.push(data_port(
        "result",
        "Result",
        PortDirection::Output,
        concrete("core.string")?,
        PortInstances::Declared,
        None,
    )?);
    Ok(NodeProtocol {
        type_id: node_id(spec.id)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            documentation_key: Some(node_key(spec.id, "documentation")?),
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: category_id(CATEGORY)?,
            icon_id: icon_id("builtin.plot")?,
            style_id: style_id("builtin.plot")?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, vec![], vec![], vec![])?,
        parameters: assembled_parameters(spec.id, vec![])?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::EnvironmentDependent,
            purity: Purity::Effectful,
            evaluation: EvaluationPolicy::EagerWhenRegionEntered,
            cache: CachePolicy::Disabled,
            effects: EffectSemantics::Ordered,
            idempotent: false,
            retry: None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn control_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
        direction,
        PortKind::Control,
        TypeExpr::Unknown,
        PortInstances::Declared,
        None,
    )
}

fn data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
    default_value: Option<TypedValue>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
        direction,
        PortKind::Data,
        value_type,
        instances,
        default_value,
    )
}

fn port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
    instances: PortInstances,
    default_value: Option<TypedValue>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        title: title.into(),
        direction,
        kind,
        value_type,
        instances,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (kind == PortKind::Data && direction == PortDirection::Input).then_some(
            InputBindingSpec {
                literal_policy: LiteralPolicy::Allowed,
                default_value,
            },
        ),
        consumption: (kind == PortKind::Data && direction == PortDirection::Input)
            .then_some(InputConsumption::FullyMaterialized),
        production: (kind == PortKind::Data && direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
}

struct PlotLowerer {
    kernel: &'static str,
}

impl NodeLowerer for PlotLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let result = context
            .outputs
            .iter()
            .find(|(address, _)| matches!(&address.port, PortRef::Declared { key } if key.as_str() == "result"))
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
        Ok(LoweredNode {
            kernel: LoweredKernel::Kernel(LoweredKernelFragment {
                kernel: KernelHandle::new(self.kernel)
                    .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))?,
                metadata: FragmentMetadata {
                    effect: EffectSemantics::Ordered,
                    resources: vec![CompiledResourceRequirement {
                        resource: ResourceId::new(PLOT_SINK).map_err(|_| {
                            LoweringError::internal(LoweringInvariant::InvalidStaticHandle)
                        })?,
                        kind: ResourceKind::ExternalArtifact,
                        access: ResourceAccess::Shared,
                        optional: false,
                    }]
                    .into_boxed_slice(),
                    results: vec![FragmentResult {
                        name: "result".into(),
                        output: result,
                    }]
                    .into_boxed_slice(),
                },
            }),
            parameters: CompiledParameterHandle::new(format!("node.{}", context.node_id))
                .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))?,
        })
    }
}

fn add_messages(out: &mut Vec<(&'static str, &'static str, Message)>, spec: &PlotSpec) {
    let title = key_text(spec.id, "title");
    let documentation = key_text(spec.id, "documentation");
    let aliases = key_text(spec.id, "aliases");
    out.extend([
        ("en-US", title, Message::Text(spec.en)),
        ("zh-CN", title, Message::Text(spec.zh)),
        ("en-US", documentation, Message::Text("This effectful view node requires the plot-sink resource and returns the published presentation result.")),
        ("zh-CN", documentation, Message::Text("此视图节点具有显式副作用，需要绘图接收器资源，并返回已发布的展示结果。")),
        ("en-US", aliases, Message::Aliases(spec.aliases)),
        ("zh-CN", aliases, Message::Aliases(spec.zh_aliases)),
    ]);
}

fn concrete(value: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(type_id(value)?))
}
fn node_id(value: &'static str) -> Result<NodeTypeId, BuiltinAssemblyError> {
    sid(value, NodeTypeId::new)
}
fn type_id(value: &'static str) -> Result<TypeId, BuiltinAssemblyError> {
    sid(value, TypeId::new)
}
fn port_key(value: &'static str) -> Result<PortKey, BuiltinAssemblyError> {
    sid(value, PortKey::new)
}
fn category_id(value: &'static str) -> Result<NodeCategoryId, BuiltinAssemblyError> {
    sid(value, NodeCategoryId::new)
}
fn icon_id(value: &'static str) -> Result<IconId, BuiltinAssemblyError> {
    sid(value, IconId::new)
}
fn style_id(value: &'static str) -> Result<NodeStyleId, BuiltinAssemblyError> {
    sid(value, NodeStyleId::new)
}
fn i18n_key(value: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    sid(value, I18nKey::new)
}
fn node_key(id: &'static str, suffix: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    i18n_key(key_text(id, suffix))
}
fn key_text(id: &'static str, suffix: &'static str) -> &'static str {
    Box::leak(format!("nodes.{id}.{suffix}").into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_protocol_localization_key_exists_in_both_locales() {
        let fragment = build_provider_fragment().expect("plot fixture must assemble");
        let localized_keys = fragment
            .messages
            .iter()
            .map(|(locale, key, _)| (*locale, *key))
            .collect::<BTreeSet<_>>();
        for node in &fragment.nodes {
            let protocol = node.protocol();
            let keys = [
                Some(&protocol.catalog.title_key),
                protocol.catalog.documentation_key.as_ref(),
                protocol.catalog.aliases_key.as_ref(),
            ]
            .into_iter()
            .flatten();
            for key in keys {
                assert!(localized_keys.contains(&("en-US", key.as_str())));
                assert!(localized_keys.contains(&("zh-CN", key.as_str())));
            }
        }
    }

    fn input_type(node: &NodeProtocol, key: &str) -> TypeExpr {
        node.interface
            .ports
            .iter()
            .find(|port| port.direction == PortDirection::Input && port.key.as_str() == key)
            .unwrap()
            .value_type
            .clone()
    }

    #[test]
    fn plot_protocols_use_canonical_numeric_series_and_exclude_date() {
        let numeric = numeric_data_series_type();
        for id in [
            "yssbi.plot.scatter.view",
            "yssbi.plot.line.view",
            "yssbi.plot.ecdf.view",
            "yssbi.plot.kde.view",
            "yssbi.plot.histogram.view",
        ] {
            let spec = SPECS.iter().find(|spec| spec.id == id).unwrap();
            let node = protocol(spec).unwrap();
            let keys: &[&str] = if matches!(spec.inputs, PlotInputs::Pair) {
                &["x", "y"]
            } else {
                &["values"]
            };
            for key in keys {
                let value_type = input_type(&node, key);
                assert_eq!(value_type, numeric);
            }
        }

        let correlogram = protocol(
            SPECS
                .iter()
                .find(|spec| spec.id == "yssbi.plot.correlogram.view")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(input_type(&correlogram, "values"), numeric);
    }

    #[test]
    fn every_view_declares_effect_resource_and_result() {
        for spec in SPECS {
            let node = protocol(spec).expect("plot built-in fixture must assemble");
            assert_eq!(node.execution.effects, EffectSemantics::Ordered);
            assert_eq!(node.execution.purity, Purity::Effectful);
            assert!(node.interface.ports.iter().any(
                |port| port.key.as_str() == "result" && port.direction == PortDirection::Output
            ));
            let fragment = PlotLowerer {
                kernel: spec.kernel,
            };
            assert_eq!(fragment.kernel, spec.kernel);
        }
        assert_eq!(PLOT_SINK, "yssbi.runtime.plot_sink");
    }
}
