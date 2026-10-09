use super::*;
use crate::{GraphSemanticCache, resolve_graph_semantics, resolve_graph_semantics_with_cache};
use yss_graph_document::{ConnectionId, DocumentConnection, DocumentNode, NodePosition};
use yss_graph_resource_contract::{ColumnSchema, DataSchema};

mod constant_defaults;

fn protocol_with_defaults(
    registry: &NodeRegistry,
    template: &str,
    id: &str,
    defaults: &[(&str, serde_json::Value)],
) -> yss_node_protocol::NodeProtocol {
    let mut protocol = registry
        .protocol(&template.parse().unwrap())
        .unwrap()
        .clone();
    protocol.type_id = id.parse().unwrap();
    for (key, value) in defaults {
        let parameter = protocol
            .parameters
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.parameters)
            .find(|parameter| parameter.key.as_str() == *key)
            .unwrap();
        parameter.default_value = Some(
            yss_node_protocol::normalize_json_literal(value, &parameter.value_type, registry)
                .unwrap(),
        );
    }
    protocol
}

#[test]
fn protocol_defaults_resolve_resource_projection_and_renames_without_document_values() {
    use yss_data_contract::DataValue;
    use yss_node_protocol::TypedValue;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let source = protocol_with_defaults(
        &builtin.registry,
        "yssbi.dataframe.source.get",
        "tests.defaults.source",
        &[("dataframe", serde_json::json!("databases/b"))],
    );
    let project = protocol_with_defaults(
        &builtin.registry,
        "yssbi.dataframe.project",
        "tests.defaults.project",
        &[("columns", serde_json::json!(["amount"]))],
    );
    let rename = protocol_with_defaults(
        &builtin.registry,
        "yssbi.dataframe.rename",
        "tests.defaults.rename",
        &[
            ("from", serde_json::json!("amount")),
            ("to", serde_json::json!("renamed")),
        ],
    );
    let mut mapped = rename.clone();
    mapped.type_id = "tests.defaults.mapping".parse().unwrap();
    let mut mapping = mapped.parameters.iter().next().unwrap().clone();
    mapping.value_type = TypeExpr::Unknown;
    mapping.constraints.retain(|constraint| {
        !matches!(
            constraint,
            yss_node_protocol::ParameterConstraint::ColumnName
        )
    });
    mapping.editor = yss_node_protocol::ParameterEditorSpec::Auto;
    mapping.default_value = Some(TypedValue {
        value_type: TypeExpr::Unknown,
        value: DataValue::Object(BTreeMap::from([(
            "renamed".into(),
            DataValue::String("final".into()),
        )])),
    });
    mapped.parameters.groups[0].parameters = Box::new([mapping]);
    mapped
        .interface
        .ports
        .iter_mut()
        .find(|port| port.key.as_str() == "result")
        .unwrap()
        .schema = Some(SchemaExpr::Rename {
        input: Box::new(SchemaExpr::Input("source".parse().unwrap())),
        mapping: RenameExpr::FromParameter("from".parse().unwrap()),
    });
    let registry = crate::tests::registry_with_protocols([source, project, rename, mapped]);
    let mut document = GraphDocument::default();
    let source = node(&mut document, "tests.defaults.source", &[]);
    let project = node(&mut document, "tests.defaults.project", &[]);
    let rename = node(&mut document, "tests.defaults.rename", &[]);
    let mapped = node(&mut document, "tests.defaults.mapping", &[]);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(project, "source"),
    );
    connect(
        &mut document,
        port(project, "result"),
        port(rename, "source"),
    );
    connect(
        &mut document,
        port(rename, "result"),
        port(mapped, "source"),
    );
    let resources = catalog(None);
    let mut cache = GraphSemanticCache::default();
    let first = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(first.ready().is_some(), "{:?}", first.diagnostics());
    for (id, name) in [(project, "amount"), (rename, "renamed"), (mapped, "final")] {
        let interface = first.concrete_interface();
        let fields = &interface
            .port(&port(id, "result"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields;
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.0.as_ref(), name);
        assert_eq!(
            fields[0].scalar_type,
            RelationalScalarType::Known(yss_data_contract::SemanticType::Numeric)
        );
    }
    document.nodes.get_mut(&mapped).unwrap().parameters.insert(
        "from".parse().unwrap(),
        serde_json::json!({"renamed":"override"}),
    );
    let explicit = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(explicit.ready().is_some());
    assert_eq!(
        explicit
            .concrete_interface()
            .port(&port(mapped, "result"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields[0]
            .name
            .0
            .as_ref(),
        "override"
    );
    document
        .nodes
        .get_mut(&mapped)
        .unwrap()
        .parameters
        .insert("from".parse().unwrap(), serde_json::Value::Null);
    let invalid = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(invalid.ready().is_none());
    assert!(
        invalid
            .concrete_interface()
            .port(&port(mapped, "result"))
            .unwrap()
            .schema_state
            .exact()
            .is_none()
    );
    document.nodes.get_mut(&mapped).unwrap().parameters.clear();
    assert_eq!(
        first,
        assert_matches_full(&document, &registry, &resources, &mut cache)
    );
    assert!(
        document
            .nodes
            .values()
            .all(|node| node.parameters.is_empty())
    );
}

#[test]
fn protocol_defaults_keep_selected_series_type_and_schema_in_agreement() {
    use yss_data_contract::{DataValue, SemanticType};
    use yss_node_protocol::{Parameter, ParameterCondition, ResolvedType};
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let mut selected = protocol_with_defaults(
        &builtin.registry,
        "yssbi.dataframe.series.select",
        "tests.defaults.select",
        &[("column", serde_json::json!("amount"))],
    );
    let mut column = selected.parameters.iter().next().unwrap().clone();
    let mut enabled = Parameter::number("enabled").int().default(1);
    enabled.title_key = column.title_key.clone();
    column.visible_when = Some(ParameterCondition {
        key: enabled.key.clone(),
        values: Box::new([DataValue::Integer(1)]),
    });
    selected.parameters.groups[0].parameters = Box::new([column, enabled]);
    let registry = crate::tests::registry_with_protocols([selected]);
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/a"))],
    );
    let selected = node(&mut document, "tests.defaults.select", &[]);
    let frequency = node(&mut document, "yssbi.dataframe.series.frequency", &[]);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(selected, "dataframe"),
    );
    connect(
        &mut document,
        port(selected, "series"),
        port(frequency, "series"),
    );
    let mut cache = GraphSemanticCache::default();
    for semantic in [SemanticType::Numeric, SemanticType::Categorical] {
        let resources = catalog(Some(ValueType::Scalar(semantic)));
        let snapshot = assert_matches_full(&document, &registry, &resources, &mut cache);
        assert!(snapshot.ready().is_some(), "{:?}", snapshot.diagnostics());
        let interface = snapshot.concrete_interface();
        assert_eq!(
            interface
                .port(&port(selected, "series"))
                .unwrap()
                .type_state
                .exact(),
            Some(&ResolvedType::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: Box::new([ResolvedType::Nominal(semantic.type_id().parse().unwrap())]),
            })
        );
        assert_eq!(
            interface
                .port(&port(frequency, "result"))
                .unwrap()
                .schema_state
                .exact()
                .unwrap()
                .fields[0]
                .scalar_type,
            RelationalScalarType::Known(semantic)
        );
    }
    let resources = catalog(Some(ValueType::Scalar(SemanticType::Numeric)));
    document
        .nodes
        .get_mut(&selected)
        .unwrap()
        .parameters
        .insert("column".parse().unwrap(), serde_json::json!(5));
    let invalid = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(invalid.ready().is_none());
    assert!(
        invalid
            .concrete_interface()
            .port(&port(selected, "series"))
            .unwrap()
            .type_state
            .exact()
            .is_none()
    );
    document
        .nodes
        .get_mut(&selected)
        .unwrap()
        .parameters
        .remove(&"column".parse().unwrap());
    document
        .nodes
        .get_mut(&selected)
        .unwrap()
        .parameters
        .insert("enabled".parse().unwrap(), serde_json::json!(0));
    let hidden = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(hidden.ready().is_none());
    assert!(
        hidden
            .concrete_interface()
            .port(&port(selected, "series"))
            .unwrap()
            .type_state
            .exact()
            .is_none()
    );
    document
        .nodes
        .get_mut(&selected)
        .unwrap()
        .parameters
        .clear();
    assert!(
        assert_matches_full(&document, &registry, &resources, &mut cache)
            .ready()
            .is_some()
    );
    assert!(document.nodes[&selected].parameters.is_empty());
}

#[test]
fn multivariate_coordinate_schemas_refresh_from_dimensions_without_reading_data() {
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
    let mut document = GraphDocument::default();
    let pca = node(&mut document, "yssbi.statistics.multivariate.pca", &[]);
    let canonical = node(&mut document, "yssbi.statistics.association.canonical", &[]);
    let mut cache = GraphSemanticCache::default();
    let fields = |snapshot: &crate::GraphSemanticSnapshot, id| {
        snapshot
            .node(id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(id, "scores"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields
            .iter()
            .map(|field| field.name.0.to_string())
            .collect::<Vec<_>>()
    };
    let first = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(fields(&first, pca), vec!["axis1", "axis2"]);
    assert_eq!(fields(&first, canonical), vec!["x_axis1", "y_axis1"]);
    document
        .nodes
        .get_mut(&pca)
        .unwrap()
        .parameters
        .insert("components".parse().unwrap(), serde_json::json!(3));
    document
        .nodes
        .get_mut(&canonical)
        .unwrap()
        .parameters
        .insert("components".parse().unwrap(), serde_json::json!(2));
    let edited = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(fields(&edited, pca), vec!["axis1", "axis2", "axis3"]);
    assert_eq!(
        fields(&edited, canonical),
        vec!["x_axis1", "x_axis2", "y_axis1", "y_axis2"]
    );
}

#[test]
fn column_parameters_preserve_intent_across_unknown_empty_and_replaced_schemas() {
    use crate::GraphParameterConfigurationFact::ProjectColumns;
    use yss_data_contract::SemanticType;
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("data"))],
    );
    let group = node(
        &mut document,
        "yssbi.dataframe.groupby",
        &[
            ("keys", serde_json::json!(["industry"])),
            ("mean", serde_json::json!([" sales "])),
        ],
    );
    let values = document.nodes[&group].parameters.clone();
    let mut cache = GraphSemanticCache::default();
    let empty = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
    let configuration = |snapshot: &crate::GraphSemanticSnapshot| {
        snapshot
            .node(group)
            .unwrap()
            .parameters
            .iter()
            .find(|parameter| parameter.key.as_str() == "mean")
            .unwrap()
            .configuration
            .clone()
            .unwrap()
    };
    let disconnected = assert_matches_full(&document, &registry, &empty, &mut cache);
    assert!(
        matches!(configuration(&disconnected), ProjectColumns { schema_known: false, options, value, .. }
        if options.is_empty() && value.as_ref() == [Box::<str>::from(" sales ")])
    );
    connect(
        &mut document,
        port(source, "dataframe"),
        port(group, "source"),
    );
    let missing = assert_matches_full(&document, &registry, &empty, &mut cache);
    assert!(
        matches!(configuration(&missing), ProjectColumns { schema_known: false, context_hint: Some(hint), .. }
        if hint.as_ref() == "editors.dataframe.schema_error")
    );
    for (columns, expected_options) in [
        (vec![], 0),
        (
            vec![
                ("industry", SemanticType::Text),
                (" sales ", SemanticType::Numeric),
            ],
            1,
        ),
        (
            vec![
                ("industry", SemanticType::Text),
                (" sales ", SemanticType::Text),
            ],
            0,
        ),
    ] {
        let resources = ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("data"),
                DataSchema {
                    columns: columns
                        .into_iter()
                        .map(|(name, semantic)| ColumnSchema {
                            name: name.into(),
                            data_type: ValueType::Scalar(semantic),
                            physical_type: None,
                            semantic: None,
                        })
                        .collect(),
                },
            )]),
        );
        let resolved = assert_matches_full(&document, &registry, &resources, &mut cache);
        assert!(
            matches!(configuration(&resolved), ProjectColumns { schema_known: true, context_hint: None, options, value, .. }
            if options.len() == expected_options && value.as_ref() == [Box::<str>::from(" sales ")])
        );
        assert_eq!(resolved.has_blocking_diagnostics(), expected_options == 0);
        assert_eq!(document.nodes[&group].parameters, values);
    }
    document.connections.clear();
    let disconnected = assert_matches_full(&document, &registry, &empty, &mut cache);
    assert!(
        matches!(configuration(&disconnected), ProjectColumns { schema_known: false, options, value, .. }
        if options.is_empty() && value.as_ref() == [Box::<str>::from(" sales ")])
    );
    assert_eq!(document.nodes[&group].parameters, values);
}

#[test]
fn aggregate_schemas_and_column_options_use_semantics_and_refresh_on_parameter_edits() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("data"),
            DataSchema {
                columns: [
                    (" amount ", S::Numeric),
                    ("category", S::Categorical),
                    ("level", S::Ordinal),
                    ("flag", S::Binary),
                    ("text", S::Text),
                    ("id", S::Identifier),
                ]
                .map(|(name, kind)| ColumnSchema {
                    name: name.into(),
                    data_type: ValueType::Scalar(kind),
                    physical_type: None,
                    semantic: None,
                })
                .into(),
            },
        )]),
    );
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("data"))],
    );
    let describe = node(&mut document, "yssbi.statistics.describe", &[]);
    let group = node(
        &mut document,
        "yssbi.dataframe.groupby",
        &[
            ("keys", serde_json::json!(["category"])),
            ("mean", serde_json::json!([" amount "])),
        ],
    );
    let series = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("level"))],
    );
    let frequency = node(&mut document, "yssbi.dataframe.series.frequency", &[]);
    let lag = node(&mut document, "yssbi.dataframe.timeseries.lag", &[]);
    let one = node(&mut document, "yssbi.statistics.describe", &[]);
    for (to, input) in [
        (describe, "source"),
        (group, "source"),
        (series, "dataframe"),
    ] {
        connect(&mut document, port(source, "dataframe"), port(to, input));
    }
    connect(&mut document, port(series, "series"), port(lag, "series"));
    connect(&mut document, port(series, "series"), port(one, "source"));
    connect(
        &mut document,
        port(lag, "result"),
        port(frequency, "series"),
    );
    let mut cache = GraphSemanticCache::default();
    let first = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(
        !first.has_blocking_diagnostics(),
        "{:?}",
        first.diagnostics()
    );
    let options = |node: NodeId, key: &str| match &first
        .node(node)
        .unwrap()
        .parameters
        .iter()
        .find(|p| p.key.as_str() == key)
        .unwrap()
        .configuration
    {
        Some(crate::GraphParameterConfigurationFact::ProjectColumns { options, .. }) => options
            .iter()
            .map(|f| f.name.to_string())
            .collect::<Vec<_>>(),
        _ => panic!("column selector required"),
    };
    assert!(first.node(describe).unwrap().parameters.is_empty());
    assert_eq!(options(group, "mean"), [" amount "]);
    assert_eq!(options(group, "count").len(), 6);
    let schema = |snapshot: &crate::GraphSemanticSnapshot, id| {
        snapshot
            .node(id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(id, "result"))
            .unwrap()
            .schema_state
            .clone()
    };
    let fields = schema(&first, group).exact().unwrap().fields.clone();
    assert_eq!(
        fields.iter().map(|f| f.name.0.as_ref()).collect::<Vec<_>>(),
        ["category", "row_count", " amount _mean"]
    );
    assert_eq!(
        schema(&first, frequency).exact().unwrap().fields[0].scalar_type,
        RelationalScalarType::Known(S::Ordinal)
    );
    assert_eq!(schema(&first, describe), GraphSchemaState::NotApplicable);
    assert_eq!(schema(&first, one), GraphSchemaState::NotApplicable);
    for output in [port(source, "dataframe"), port(series, "series")] {
        document
            .connections
            .values_mut()
            .find(|connection| connection.input == port(one, "source"))
            .unwrap()
            .output = output;
        let reconnected = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
        assert_eq!(schema(&reconnected, one), GraphSchemaState::NotApplicable);
        assert!(!reconnected.has_blocking_diagnostics());
        assert!(reconnected.node(one).unwrap().parameters.is_empty());
    }
    document
        .nodes
        .get_mut(&series)
        .unwrap()
        .parameters
        .insert("column".parse().unwrap(), serde_json::json!("flag"));
    let switched = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(
        !switched.has_blocking_diagnostics(),
        "{:?}",
        switched.diagnostics()
    );
    assert_eq!(
        schema(&switched, frequency).exact().unwrap().fields[0].scalar_type,
        RelationalScalarType::Known(S::Binary)
    );
    document
        .nodes
        .get_mut(&group)
        .unwrap()
        .parameters
        .insert("mean".parse().unwrap(), serde_json::json!(["category"]));
    let invalid = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(invalid.has_blocking_diagnostics());
    assert_eq!(
        schema(&invalid, group).issue(),
        Some(GraphSchemaIssue::InvalidParameter)
    );
    document
        .nodes
        .get_mut(&group)
        .unwrap()
        .parameters
        .insert("mean".parse().unwrap(), serde_json::json!([]));
    document
        .nodes
        .get_mut(&group)
        .unwrap()
        .parameters
        .insert("sum".parse().unwrap(), serde_json::json!([" amount "]));
    let changed = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(
        !changed.has_blocking_diagnostics(),
        "{:?}",
        changed.diagnostics()
    );
    assert_eq!(
        schema(&changed, group).exact().unwrap().fields[2]
            .name
            .0
            .as_ref(),
        " amount _sum"
    );
    for (selection, issue) in [
        (
            serde_json::json!([" amount ", " amount "]),
            GraphSchemaIssue::InvalidParameter,
        ),
        (
            serde_json::json!([" \t "]),
            GraphSchemaIssue::InvalidParameter,
        ),
        (
            serde_json::json!(["amount"]),
            GraphSchemaIssue::MissingColumn,
        ),
    ] {
        document
            .nodes
            .get_mut(&group)
            .unwrap()
            .parameters
            .insert("sum".parse().unwrap(), selection);
        let invalid = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
        assert_eq!(schema(&invalid, group).issue(), Some(issue));
    }
}

#[test]
fn series_schema_uses_the_protocol_default_conversion_target() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = catalog(Some(ValueType::Scalar(S::Numeric)));
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/a"))],
    );
    let selected = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("amount"))],
    );
    let labels = node(&mut document, "yssbi.dataframe.labels", &[]);
    let frequency = node(&mut document, "yssbi.dataframe.series.frequency", &[]);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(selected, "dataframe"),
    );
    connect(
        &mut document,
        port(selected, "series"),
        port(labels, "input"),
    );
    connect(
        &mut document,
        port(labels, "output"),
        port(frequency, "series"),
    );

    let snapshot = resolve_graph_semantics(&document, &builtin.registry, &resources);
    let interface = snapshot.concrete_interface();
    let fields = &interface
        .port(&port(frequency, "result"))
        .unwrap()
        .schema_state
        .exact()
        .unwrap()
        .fields;
    assert_eq!(
        fields[0].scalar_type,
        RelationalScalarType::Known(S::Categorical)
    );
}

#[test]
fn schema_cache_tracks_series_meaning_in_composition_and_transforms() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = catalog(Some(ValueType::Scalar(S::Numeric)));
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/a"))],
    );
    let selected = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("amount"))],
    );
    let labels = node(
        &mut document,
        "yssbi.dataframe.labels",
        &[("target_type", serde_json::json!("core.categorical"))],
    );
    let combine = node(&mut document, "yssbi.dataframe.combine", &[]);
    let set_column = node(
        &mut document,
        "yssbi.dataframe.set_column",
        &[("name", serde_json::json!("labels"))],
    );
    let combined_input = PortAddress::instance(
        combine,
        "series".parse().unwrap(),
        yss_graph_document::PortInstanceId::new(),
    );
    document.port_bindings.insert(
        combined_input.clone(),
        yss_graph_document::DynamicPortBinding::UserCreated {
            order: yss_graph_document::OrderKey::new("0"),
        },
    );
    connect(
        &mut document,
        port(source, "dataframe"),
        port(selected, "dataframe"),
    );
    connect(
        &mut document,
        port(selected, "series"),
        port(labels, "input"),
    );
    connect(&mut document, port(labels, "output"), combined_input);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(set_column, "source"),
    );
    connect(
        &mut document,
        port(labels, "output"),
        port(set_column, "series"),
    );

    let mut cache = GraphSemanticCache::default();
    assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    document.nodes.get_mut(&labels).unwrap().parameters.insert(
        "target_type".parse().unwrap(),
        serde_json::json!("core.ordinal"),
    );
    let updated =
        resolve_graph_semantics_with_cache(&document, &builtin.registry, &resources, &mut cache);
    let interface = updated.concrete_interface();
    for (id, output) in [(combine, "dataframe"), (set_column, "result")] {
        let fields = &interface
            .port(&port(id, output))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields;
        assert_eq!(
            fields.last().unwrap().scalar_type,
            RelationalScalarType::Known(S::Ordinal)
        );
    }
    assert!(cache.schemas.reused_outputs > 0);
    assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
}

#[test]
fn fixed_conversion_targets_reach_schema_consumers_and_survive_downstream_conflicts() {
    use yss_data_contract::{DataValue, SemanticType as S};
    use yss_graph_document::{ConstantId, GraphConstant};
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = catalog(None);
    let mut document = GraphDocument::default();
    let source = node(&mut document, "yssbi.constant.get", &[]);
    let demand = node(&mut document, "yssbi.constant.get", &[]);
    let numeric = node(&mut document, "yssbi.constant.get", &[]);
    for (node_id, data_type, data_value) in [
        (
            source,
            ValueType::DataSeries(Box::new(ValueType::Scalar(S::Text))),
            DataValue::String(r#"{"value":["1"]}"#.into()),
        ),
        (demand, ValueType::Scalar(S::Numeric), DataValue::Integer(1)),
        (
            numeric,
            ValueType::Scalar(S::Numeric),
            DataValue::Integer(1),
        ),
    ] {
        let id = ConstantId::from_uuid(node_id.as_uuid());
        document.constants.insert(
            id,
            GraphConstant {
                id,
                name: node_id.to_string(),
                data_type,
                data_value,
                tabular: None,
                description: String::new(),
                tags: vec![],
            },
        );
        document.nodes.get_mut(&node_id).unwrap().parameters.insert(
            "constant".parse().unwrap(),
            serde_json::json!(id.to_string()),
        );
    }
    let mut constraint = port(demand, "value");
    let mut stages = Vec::new();
    for _ in 0..3 {
        let convert = node(&mut document, "yssbi.value.to_numeric", &[]);
        let equal = node(&mut document, "yssbi.logic.equal", &[]);
        let frequency = node(&mut document, "yssbi.dataframe.series.frequency", &[]);
        let selected = node(
            &mut document,
            "yssbi.dataframe.series.select",
            &[("column", serde_json::json!("value"))],
        );
        connect(&mut document, port(source, "value"), port(convert, "input"));
        connect(&mut document, port(convert, "output"), port(equal, "left"));
        connect(&mut document, constraint, port(equal, "right"));
        connect(
            &mut document,
            port(convert, "output"),
            port(frequency, "series"),
        );
        connect(
            &mut document,
            port(frequency, "result"),
            port(selected, "dataframe"),
        );
        stages.push((convert, frequency, selected));
        constraint = port(selected, "series");
    }
    let mut cache = GraphSemanticCache::default();
    for (semantic, value) in [
        (S::Numeric, DataValue::Integer(1)),
        (S::Binary, DataValue::Bool(true)),
    ] {
        let demand = document
            .constants
            .get_mut(&ConstantId::from_uuid(demand.as_uuid()))
            .unwrap();
        demand.data_type = ValueType::Scalar(semantic);
        demand.data_value = value;
        for &(convert, _, _) in &stages {
            document.nodes.get_mut(&convert).unwrap().node_type = format!(
                "yssbi.value.to_{}",
                semantic.type_id().strip_prefix("core.").unwrap()
            )
            .parse()
            .unwrap();
        }
        let snapshot = resolve_graph_semantics_with_cache(
            &document,
            &builtin.registry,
            &resources,
            &mut cache,
        );
        let interface = snapshot.concrete_interface();
        for &(convert, frequency, selected) in &stages {
            let fields = &interface
                .port(&port(frequency, "result"))
                .unwrap()
                .schema_state
                .exact()
                .unwrap()
                .fields;
            assert_eq!(fields[0].scalar_type, RelationalScalarType::Known(semantic));
            for address in [port(convert, "output"), port(selected, "series")] {
                assert_eq!(
                    interface.port(&address).unwrap().type_state.exact(),
                    Some(&yss_node_protocol::ResolvedType::Applied {
                        constructor: "core.data_series".parse().unwrap(),
                        arguments: Box::new([yss_node_protocol::ResolvedType::Nominal(
                            semantic.type_id().parse().unwrap()
                        )]),
                    })
                );
            }
            assert!(document.nodes[&convert].parameters.is_empty());
        }
        assert!(snapshot.ready().is_some());
        assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    }
    let incompatible = node(&mut document, "yssbi.logic.less", &[]);
    connect(
        &mut document,
        port(numeric, "value"),
        port(incompatible, "right"),
    );
    let last = stages.last().unwrap().0;
    let conflicting_edge = connect(
        &mut document,
        port(last, "output"),
        port(incompatible, "left"),
    );
    let conflict = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(conflict.ready().is_none());
    assert_eq!(
        conflict
            .concrete_interface()
            .port(&port(last, "output"))
            .unwrap()
            .type_state,
        yss_node_protocol::TypeState::Exact(yss_node_protocol::ResolvedType::Applied {
            constructor: "core.data_series".parse().unwrap(),
            arguments: Box::new([yss_node_protocol::ResolvedType::Nominal(
                "core.binary".parse().unwrap()
            )]),
        })
    );
    document
        .connections
        .get_mut(&conflicting_edge)
        .unwrap()
        .output = port(numeric, "value");
    let recovered = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(recovered.ready().is_some());
}

#[test]
fn drop_nodes_resolve_remaining_fields_and_refresh_after_upstream_changes() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resource = |extra: bool| {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("data"),
                DataSchema {
                    columns: [
                        ("amount", S::Numeric),
                        ("unused", S::Text),
                        ("id", S::Identifier),
                    ]
                    .into_iter()
                    .chain(extra.then_some(("new", S::Text)))
                    .map(|(name, kind)| ColumnSchema {
                        name: name.into(),
                        data_type: ValueType::Scalar(kind),
                        physical_type: None,
                        semantic: None,
                    })
                    .collect(),
                },
            )]),
        )
    };
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("data"))],
    );
    let columns = node(
        &mut document,
        "yssbi.dataframe.drop.columns",
        &[("columns", serde_json::json!(["unused"]))],
    );
    let rows = node(
        &mut document,
        "yssbi.dataframe.drop.rows",
        &[(
            "predicate",
            serde_json::json!({"column":"amount","operator":"lessThan","value":{"type":"integer","value":"0"}}),
        )],
    );
    connect(
        &mut document,
        port(source, "dataframe"),
        port(columns, "source"),
    );
    connect(&mut document, port(columns, "result"), port(rows, "source"));
    let na_rows = node(&mut document, "yssbi.dataframe.dropna.rows", &[]);
    let na_columns = node(&mut document, "yssbi.dataframe.dropna.columns", &[]);
    let after_na = node(&mut document, "yssbi.dataframe.dropna.rows", &[]);
    connect(&mut document, port(rows, "result"), port(na_rows, "source"));
    connect(
        &mut document,
        port(na_rows, "result"),
        port(na_columns, "source"),
    );
    connect(
        &mut document,
        port(na_columns, "result"),
        port(after_na, "source"),
    );
    let mut cache = GraphSemanticCache::default();
    for extra in [false, true] {
        let snapshot =
            assert_matches_full(&document, &builtin.registry, &resource(extra), &mut cache);
        for id in [na_rows, na_columns] {
            assert!(matches!(
                &snapshot.node(id).unwrap().parameters[0].configuration,
                Some(crate::GraphParameterConfigurationFact::ProjectColumns {
                    allow_empty: true, schema_known: true, options, value, ..
                }) if options.len() == if extra { 3 } else { 2 } && value.is_empty()
            ));
        }
        assert!(matches!(
            &snapshot.node(after_na).unwrap().parameters[0].configuration,
            Some(crate::GraphParameterConfigurationFact::ProjectColumns {
                allow_empty: true, schema_known: false, context_hint: Some(reason), ..
            }) if reason.as_ref() == "editors.dataframe.deferred_columns"
        ));
        assert!(matches!(
            &snapshot.node(columns).unwrap().parameters[0].configuration,
            Some(crate::GraphParameterConfigurationFact::ProjectColumns { schema_known: true, options, .. })
                if options.len() == if extra { 4 } else { 3 }
        ));
        assert!(matches!(
            &snapshot.node(rows).unwrap().parameters[0].configuration,
            Some(crate::GraphParameterConfigurationFact::FilterPredicate { schema_known: true, columns, .. })
                if columns.len() == if extra { 3 } else { 2 }
        ));
        let source_fields = snapshot
            .node(source)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(source, "dataframe"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields
            .clone();
        let expected: Vec<_> = source_fields
            .into_iter()
            .filter(|f| f.name.0.as_ref() != "unused")
            .collect();
        assert!(
            !snapshot.has_blocking_diagnostics(),
            "{:?}",
            snapshot.diagnostics()
        );
        for id in [na_columns, after_na] {
            let result = snapshot
                .node(id)
                .unwrap()
                .ports
                .iter()
                .find(|p| p.address == port(id, "result"))
                .unwrap();
            assert!(matches!(result.schema_state, GraphSchemaState::Deferred));
            assert!(result.schema_state.exact().is_none());
        }
        for id in [columns, rows, na_rows] {
            let result = snapshot
                .node(id)
                .unwrap()
                .ports
                .iter()
                .find(|p| p.address == port(id, "result"))
                .unwrap();
            assert_eq!(result.schema_state.exact().unwrap().fields, expected);
        }
    }
    // A deferred frame is executable, but cannot promise a statically selected column.
    let mut dependent = document.clone();
    let select = node(
        &mut dependent,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("amount"))],
    );
    connect(
        &mut dependent,
        port(na_columns, "result"),
        port(select, "dataframe"),
    );
    let blocked = resolve_graph_semantics(&dependent, &builtin.registry, &resource(false));
    assert!(blocked.has_blocking_diagnostics());
    let observed_field = SchemaField {
        name: SchemaColumnRef("amount".into()),
        scalar_type: RelationalScalarType::Known(S::Numeric),
        lineage: None,
    };
    let mut observations = crate::GraphSchemaObservations::from([(
        port(na_columns, "result"),
        crate::GraphSchemaObservation {
            version: 7,
            fields: vec![observed_field],
        },
    )]);
    let observed = crate::resolve_graph_semantics_with_observations(
        &dependent,
        &builtin.registry,
        &resource(false),
        &mut cache,
        &observations,
    );
    assert!(
        !observed.has_blocking_diagnostics(),
        "{:?}",
        observed.diagnostics()
    );
    assert_eq!(
        blocked.node(na_columns).unwrap().execution_fingerprint(),
        observed.node(na_columns).unwrap().execution_fingerprint(),
        "a producer's own Schema observation is not one of its inputs",
    );
    let field = &observed
        .node(na_columns)
        .unwrap()
        .ports
        .iter()
        .find(|p| p.address == port(na_columns, "result"))
        .unwrap()
        .schema_state
        .exact()
        .unwrap()
        .fields[0];
    assert!(
        field.lineage.is_some(),
        "observed columns preserve source identity"
    );
    let full = crate::resolve_graph_semantics_with_observations(
        &dependent,
        &builtin.registry,
        &resource(false),
        &mut GraphSemanticCache::default(),
        &observations,
    );
    assert_eq!(observed, full);
    observations
        .get_mut(&port(na_columns, "result"))
        .unwrap()
        .fields
        .clear();
    let empty = crate::resolve_graph_semantics_with_observations(
        &dependent,
        &builtin.registry,
        &resource(false),
        &mut cache,
        &observations,
    );
    assert!(
        empty.has_blocking_diagnostics(),
        "known empty Schema cannot supply amount"
    );
    assert!(matches!(
        &empty.node(after_na).unwrap().parameters[0].configuration,
        Some(crate::GraphParameterConfigurationFact::ProjectColumns { schema_known: true, options, .. }) if options.is_empty()
    ));
    let withdrawn = resolve_graph_semantics_with_cache(
        &dependent,
        &builtin.registry,
        &resource(false),
        &mut cache,
    );
    assert_eq!(
        withdrawn, blocked,
        "removed observations cannot survive a cache hit"
    );
    for (selection, issue) in [
        (
            serde_json::json!(["absent"]),
            GraphSchemaIssue::MissingColumn,
        ),
        (
            serde_json::json!(["amount", "unused", "id"]),
            GraphSchemaIssue::InvalidParameter,
        ),
    ] {
        document
            .nodes
            .get_mut(&columns)
            .unwrap()
            .parameters
            .insert("columns".parse().unwrap(), selection);
        let snapshot =
            assert_matches_full(&document, &builtin.registry, &resource(false), &mut cache);
        let result = snapshot
            .node(columns)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(columns, "result"))
            .unwrap();
        assert_eq!(result.schema_state.issue(), Some(issue));
    }
}

#[test]
fn composed_schemas_track_input_order_join_keys_and_mixed_series() {
    use yss_data_contract::SemanticType as S;
    use yss_graph_document::{DynamicPortBinding, OrderKey, PortInstanceId};
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([
            (
                GraphResourceId::new("databases/left"),
                DataSchema {
                    columns: [("left_id", S::Numeric), ("label", S::Text)]
                        .map(|(name, kind)| ColumnSchema {
                            name: name.into(),
                            data_type: ValueType::Scalar(kind),
                            physical_type: None,
                            semantic: None,
                        })
                        .into(),
                },
            ),
            (
                GraphResourceId::new("databases/right"),
                DataSchema {
                    columns: [("right_id", S::Numeric), ("label", S::Text)]
                        .map(|(name, kind)| ColumnSchema {
                            name: name.into(),
                            data_type: ValueType::Scalar(kind),
                            physical_type: None,
                            semantic: None,
                        })
                        .into(),
                },
            ),
        ]),
    );
    let mut document = GraphDocument::default();
    let left = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/left"))],
    );
    let right = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/right"))],
    );
    let join = node(
        &mut document,
        "yssbi.dataframe.join",
        &[
            ("left_keys", serde_json::json!(["left_id"])),
            ("right_keys", serde_json::json!(["right_id"])),
        ],
    );
    connect(&mut document, port(left, "dataframe"), port(join, "left"));
    connect(&mut document, port(right, "dataframe"), port(join, "right"));
    let concat = node(&mut document, "yssbi.dataframe.concat.rows", &[]);
    let mut dynamic = Vec::new();
    for (index, source) in [left, right].into_iter().enumerate() {
        let input = PortAddress::instance(concat, "frames".parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            input.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(index.to_string()),
            },
        );
        connect(&mut document, port(source, "dataframe"), input.clone());
        dynamic.push(input);
    }
    let mut cache = GraphSemanticCache::default();
    let first = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    let fields = |snapshot: &crate::GraphSemanticSnapshot, id| {
        snapshot
            .node(id)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.direction == yss_node_protocol::PortDirection::Output)
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields
            .iter()
            .map(|f| f.name.0.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        fields(&first, join),
        ["left_id", "label", "right_id", "label_right"]
    );
    assert_eq!(fields(&first, concat), ["left_id", "label", "right_id"]);
    let right_keys = first
        .node(join)
        .unwrap()
        .parameters
        .iter()
        .find(|p| p.key.as_str() == "right_keys")
        .unwrap();
    assert!(
        matches!(&right_keys.configuration, Some(crate::GraphParameterConfigurationFact::ProjectColumns { options, .. }) if options[0].name.as_ref() == "right_id")
    );
    document.port_bindings.insert(
        dynamic[0].clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey::new("z"),
        },
    );
    let second = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(fields(&second, concat), ["right_id", "label", "left_id"]);
    let number = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("left_id"))],
    );
    let text = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("label"))],
    );
    for target in [number, text] {
        connect(
            &mut document,
            port(left, "dataframe"),
            port(target, "dataframe"),
        );
    }
    let assemble = node(&mut document, "yssbi.dataframe.combine", &[]);
    for (index, source) in [number, text].into_iter().enumerate() {
        let input =
            PortAddress::instance(assemble, "series".parse().unwrap(), PortInstanceId::new());
        document.port_bindings.insert(
            input.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(index.to_string()),
            },
        );
        connect(&mut document, port(source, "series"), input);
    }
    let third = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(fields(&third, assemble), ["left_id", "label"]);
    document
        .nodes
        .get_mut(&join)
        .unwrap()
        .parameters
        .insert("right_keys".parse().unwrap(), serde_json::json!(["absent"]));
    let invalid = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(
        invalid
            .node(join)
            .unwrap()
            .ports
            .iter()
            .any(|p| p.direction == yss_node_protocol::PortDirection::Output
                && p.schema_state.exact().is_none())
    );
}

#[test]
fn restored_decomposed_columns_refresh_consumers_through_orphan_bindings() {
    use yss_data_contract::SemanticType;
    use yss_graph_document::{DynamicPortBinding, LastKnownPortMetadata, OrderKey, PortInstanceId};

    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/a"))],
    );
    let decompose = node(&mut document, "yssbi.dataframe.decompose", &[]);
    let frequency = node(&mut document, "yssbi.dataframe.series.frequency", &[]);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(decompose, "dataframe"),
    );
    let column =
        PortAddress::instance(decompose, "columns".parse().unwrap(), PortInstanceId::new());
    document.port_bindings.insert(
        column.clone(),
        DynamicPortBinding::Orphan {
            origin: DynamicMemberLocator::SchemaField {
                source: SchemaSourceIdentity::new("databases/a"),
                field: SchemaFieldIdentity::new("amount"),
            },
            order: OrderKey::new("0"),
            last_known: LastKnownPortMetadata::default(),
        },
    );
    connect(&mut document, column.clone(), port(frequency, "series"));
    let mut cache = GraphSemanticCache::default();
    let missing = assert_matches_full(&document, &builtin.registry, &catalog(None), &mut cache);
    assert!(missing.ready().is_none());
    assert!(missing.concrete_interface().port(&column).unwrap().orphan);

    for semantic in [SemanticType::Numeric, SemanticType::Categorical] {
        let resources = catalog(Some(ValueType::Scalar(semantic)));
        let restored = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
        assert!(restored.ready().is_some(), "{:?}", restored.diagnostics());
        let interface = restored.concrete_interface();
        let source = interface.port(&column).unwrap();
        assert!(!source.orphan);
        assert_eq!(
            source.schema_state.exact().unwrap().fields[0].scalar_type,
            RelationalScalarType::Known(semantic)
        );
        let fields = &interface
            .port(&port(frequency, "result"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields;
        assert_eq!(fields[0].name.0.as_ref(), "value");
        assert_eq!(fields[0].scalar_type, RelationalScalarType::Known(semantic));
    }
    assert!(matches!(
        document.port_bindings[&column],
        DynamicPortBinding::Orphan { .. }
    ));
}

#[test]
fn dataframe_decomposition_uses_all_seven_semantics_and_tracks_metadata_changes() {
    use yss_data_contract::{ColumnSemantic, SemanticType};
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/typed"))],
    );
    let decompose = node(&mut document, "yssbi.dataframe.decompose", &[]);
    connect(
        &mut document,
        port(source, "dataframe"),
        port(decompose, "dataframe"),
    );
    let columns = SemanticType::ALL
        .into_iter()
        .map(|semantic| ColumnSchema {
            name: semantic.as_str().into(),
            data_type: ValueType::Scalar(semantic),
            physical_type: Some(
                match semantic {
                    SemanticType::Text => "Utf8",
                    SemanticType::Datetime => "Date",
                    _ => "Int64",
                }
                .into(),
            ),
            semantic: Some(ColumnSemantic::new(semantic)),
        })
        .collect::<Vec<_>>();
    let resources = |columns| {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("databases/typed"),
                DataSchema { columns },
            )]),
        )
    };
    let first = resources(columns.clone());
    let mut cache = GraphSemanticCache::default();
    let snapshot = assert_matches_full(&document, &builtin.registry, &first, &mut cache);
    let outputs = snapshot
        .node(decompose)
        .unwrap()
        .ports
        .iter()
        .filter(|port| port.direction == yss_node_protocol::PortDirection::Output)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 7);
    for (port, semantic) in outputs.into_iter().zip(SemanticType::ALL) {
        assert_eq!(
            port.type_state.exact(),
            Some(&yss_node_protocol::ResolvedType::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: Box::new([yss_node_protocol::ResolvedType::Nominal(
                    semantic.type_id().parse().unwrap()
                )]),
            })
        );
    }
    let observed = first.tracked();
    observed
        .database_schema(&GraphResourceId::new("databases/typed"))
        .unwrap();
    let dependencies = observed.dependencies();
    let mut changed = columns;
    changed[0].physical_type = Some("Float64".into());
    changed[0].semantic.as_mut().unwrap().numeric = Some(yss_data_contract::NumericConstraints {
        integer: true,
        minimum: None,
        maximum: None,
    });
    let second = resources(changed);
    assert!(!second.matches_dependencies(&dependencies));
    assert_matches_full(&document, &builtin.registry, &second, &mut cache);
}

#[test]
fn transform_schemas_are_static_and_refresh_from_declared_categories() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::from([(
            GraphResourceId::new("data"),
            DataSchema {
                columns: [
                    (" id ", S::Identifier),
                    ("x", S::Numeric),
                    ("y", S::Numeric),
                    (" category ", S::Categorical),
                    ("time", S::Datetime),
                ]
                .map(|(name, kind)| ColumnSchema {
                    name: name.into(),
                    data_type: ValueType::Scalar(kind),
                    physical_type: None,
                    semantic: None,
                })
                .into(),
            },
        )]),
    );
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("data"))],
    );
    let long = node(
        &mut document,
        "yssbi.dataframe.unpivot",
        &[
            ("keys", serde_json::json!([" id "])),
            ("columns", serde_json::json!(["x", "y"])),
            ("variable_name", serde_json::json!(" variable ")),
            ("value_name", serde_json::json!(" value ")),
        ],
    );
    let wide = node(
        &mut document,
        "yssbi.dataframe.pivot",
        &[
            ("keys", serde_json::json!([" id "])),
            ("category_column", serde_json::json!(" category ")),
            ("value_column", serde_json::json!("x")),
            ("levels", serde_json::json!(["a", "b"])),
            ("names", serde_json::json!(["a_total", "b_total"])),
        ],
    );
    let resample = node(
        &mut document,
        "yssbi.dataframe.resample",
        &[
            ("time_column", serde_json::json!("time")),
            ("keys", serde_json::json!([" id "])),
            ("columns", serde_json::json!(["x"])),
        ],
    );
    let selected = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!(" category "))],
    );
    let encoded = node(
        &mut document,
        "yssbi.dataframe.encode",
        &[
            ("levels", serde_json::json!(["a", "b"])),
            ("names", serde_json::json!(["is_a", "is_b"])),
        ],
    );
    for target in [long, wide, resample] {
        connect(
            &mut document,
            port(source, "dataframe"),
            port(target, "source"),
        );
    }
    connect(
        &mut document,
        port(source, "dataframe"),
        port(selected, "dataframe"),
    );
    connect(
        &mut document,
        port(selected, "series"),
        port(encoded, "series"),
    );
    let mut cache = GraphSemanticCache::default();
    let snapshot = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(
        !snapshot.has_blocking_diagnostics(),
        "{:?}",
        snapshot.diagnostics()
    );
    let fields = |snapshot: &crate::GraphSemanticSnapshot, node| {
        snapshot
            .node(node)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(node, "result"))
            .unwrap()
            .schema_state
            .exact()
            .unwrap()
            .fields
            .clone()
    };
    assert_eq!(
        fields(&snapshot, long)
            .iter()
            .map(|f| f.name.0.as_ref())
            .collect::<Vec<_>>(),
        vec![" id ", " variable ", " value "]
    );
    assert_eq!(
        fields(&snapshot, wide)
            .iter()
            .map(|f| f.name.0.as_ref())
            .collect::<Vec<_>>(),
        vec![" id ", "a_total", "b_total"]
    );
    assert_eq!(
        fields(&snapshot, resample)
            .iter()
            .map(|f| f.name.0.as_ref())
            .collect::<Vec<_>>(),
        vec!["time", " id ", "x_mean"]
    );
    assert!(
        fields(&snapshot, encoded)
            .iter()
            .all(|f| f.scalar_type == RelationalScalarType::Known(S::Binary))
    );
    document.nodes.get_mut(&wide).unwrap().parameters.insert(
        "names".parse().unwrap(),
        serde_json::json!([" id ", "b_total"]),
    );
    let changed = assert_matches_full(&document, &builtin.registry, &resources, &mut cache);
    assert!(changed.has_blocking_diagnostics());
    assert!(
        changed
            .node(wide)
            .unwrap()
            .ports
            .iter()
            .find(|p| p.address == port(wide, "result"))
            .unwrap()
            .schema_state
            .exact()
            .is_none()
    );
}

#[test]
fn conditional_selection_unifies_scalar_and_series_elements_for_composition() {
    use yss_data_contract::SemanticType as S;
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let resources = catalog(Some(ValueType::Scalar(S::Numeric)));
    let mut document = GraphDocument::default();
    let source = node(
        &mut document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!("databases/a"))],
    );
    let selected = node(
        &mut document,
        "yssbi.dataframe.series.select",
        &[("column", serde_json::json!("amount"))],
    );
    let missing = node(&mut document, "yssbi.dataframe.series.is_null", &[]);
    let choose = node(&mut document, "yssbi.dataframe.series.choose", &[]);
    let scalar = node(&mut document, "yssbi.constant.get", &[]);
    let id = yss_graph_document::ConstantId::from_uuid(scalar.as_uuid());
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: "fallback".into(),
            data_type: ValueType::Scalar(S::Numeric),
            data_value: yss_data_contract::DataValue::Integer(0),
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    document.nodes.get_mut(&scalar).unwrap().parameters.insert(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    );
    let output = node(
        &mut document,
        "yssbi.dataframe.set_column",
        &[("name", serde_json::json!("filled"))],
    );
    connect(
        &mut document,
        port(source, "dataframe"),
        port(selected, "dataframe"),
    );
    connect(
        &mut document,
        port(selected, "series"),
        port(missing, "series"),
    );
    connect(
        &mut document,
        port(missing, "result"),
        port(choose, "condition"),
    );
    connect(
        &mut document,
        port(scalar, "value"),
        port(choose, "when_true"),
    );
    connect(
        &mut document,
        port(selected, "series"),
        port(choose, "when_false"),
    );
    connect(
        &mut document,
        port(source, "dataframe"),
        port(output, "source"),
    );
    connect(
        &mut document,
        port(choose, "result"),
        port(output, "series"),
    );
    let snapshot = resolve_graph_semantics(&document, &builtin.registry, &resources);
    assert!(
        !snapshot.has_blocking_diagnostics(),
        "{:?}",
        snapshot.diagnostics()
    );
    let chosen = snapshot
        .node(choose)
        .unwrap()
        .ports
        .iter()
        .find(|p| p.address == port(choose, "result"))
        .unwrap();
    assert_eq!(chosen.type_state.domain().unwrap().iter().count(), 1);
    let fields = &snapshot
        .node(output)
        .unwrap()
        .ports
        .iter()
        .find(|p| p.address == port(output, "result"))
        .unwrap()
        .schema_state
        .exact()
        .unwrap()
        .fields;
    assert_eq!(
        fields.last().unwrap().scalar_type,
        RelationalScalarType::Known(S::Numeric)
    );
    document.constants.get_mut(&id).unwrap().data_type = ValueType::Scalar(S::Text);
    document.constants.get_mut(&id).unwrap().data_value =
        yss_data_contract::DataValue::String("wrong".into());
    assert!(
        resolve_graph_semantics(&document, &builtin.registry, &resources)
            .has_blocking_diagnostics()
    );
}

fn node(
    document: &mut GraphDocument,
    kind: &str,
    parameters: &[(&str, serde_json::Value)],
) -> NodeId {
    let id = NodeId::new();
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: kind.parse().unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: parameters
                .iter()
                .map(|(key, value)| (key.parse().unwrap(), value.clone()))
                .collect(),
            user_label: None,
        },
    );
    id
}

fn port(node: NodeId, key: &str) -> PortAddress {
    PortAddress::declared(node, key.parse().unwrap())
}

fn connect(document: &mut GraphDocument, output: PortAddress, input: PortAddress) -> ConnectionId {
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output,
            input,
            order: None,
        },
    );
    id
}

fn catalog(a: Option<ValueType>) -> ResourceCatalogSnapshot {
    let mut databases = BTreeMap::from([(
        GraphResourceId::new("databases/b"),
        DataSchema {
            columns: vec![ColumnSchema {
                semantic: None,
                physical_type: None,
                name: "amount".into(),
                data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            }],
        },
    )]);
    if let Some(data_type) = a {
        databases.insert(
            GraphResourceId::new("databases/a"),
            DataSchema {
                columns: vec![ColumnSchema {
                    semantic: None,
                    physical_type: None,
                    name: "amount".into(),
                    data_type,
                }],
            },
        );
    }
    ResourceCatalogSnapshot::new(BTreeMap::new(), databases)
}

fn branch(document: &mut GraphDocument, database: &str) -> [NodeId; 4] {
    let source = node(
        document,
        "yssbi.dataframe.source.get",
        &[("dataframe", serde_json::json!(database))],
    );
    let limit = node(
        document,
        "yssbi.dataframe.limit",
        &[("rows", serde_json::json!(10))],
    );
    let project = node(
        document,
        "yssbi.dataframe.project",
        &[("columns", serde_json::json!(["amount"]))],
    );
    let view = node(document, "yssbi.core.reroute", &[]);
    connect(document, port(source, "dataframe"), port(limit, "source"));
    connect(document, port(limit, "result"), port(project, "source"));
    connect(document, port(project, "result"), port(view, "input"));
    [source, limit, project, view]
}

fn assert_matches_full(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    cache: &mut GraphSemanticCache,
) -> crate::GraphSemanticSnapshot {
    let incremental_resources = resources.tracked();
    let incremental =
        resolve_graph_semantics_with_cache(document, registry, &incremental_resources, cache);
    let full_resources = resources.tracked();
    let full = resolve_graph_semantics(document, registry, &full_resources);
    assert_eq!(incremental, full);
    assert_eq!(
        incremental_resources.dependencies(),
        full_resources.dependencies()
    );
    incremental
}

#[test]
fn reroutes_preserve_result_categories_when_the_source_changes() {
    use crate::{GraphPlotDataKind, GraphResultCategory, GraphStatisticalReportKind};
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let source = node(&mut document, "yssbi.plot.scatter.view", &[]);
    let first = node(&mut document, "yssbi.core.reroute", &[]);
    let second = node(&mut document, "yssbi.core.reroute", &[]);
    connect(&mut document, port(source, "result"), port(first, "input"));
    connect(&mut document, port(first, "output"), port(second, "input"));
    let mut cache = GraphSemanticCache::default();
    for (kind, expected) in [
        (
            "yssbi.plot.scatter.view",
            GraphResultCategory::PlotData(GraphPlotDataKind::Scatter),
        ),
        (
            "yssbi.statistics.linear.summary",
            GraphResultCategory::StatisticalReport(
                GraphStatisticalReportKind::LinearRegressionSummary,
            ),
        ),
    ] {
        document.nodes.get_mut(&source).unwrap().node_type = kind.parse().unwrap();
        let snapshot = assert_matches_full(&document, &registry, &catalog(None), &mut cache);
        for route in [first, second] {
            assert_eq!(
                snapshot
                    .concrete_interface()
                    .port(&port(route, "output"))
                    .unwrap()
                    .result_category,
                expected
            );
        }
    }
}

#[test]
fn missing_target_keeps_existing_schema_and_type_facts() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let resources = catalog(None);
    let mut document = GraphDocument::default();
    let [source, _, projected, _] = branch(&mut document, "databases/b");
    let mut cache = GraphSemanticCache::default();
    let before = assert_matches_full(&document, &registry, &resources, &mut cache);
    let invalid = connect(
        &mut document,
        port(source, "dataframe"),
        port(NodeId::new(), "input"),
    );

    let after = assert_matches_full(&document, &registry, &resources, &mut cache);
    let expected = before
        .concrete_interface()
        .port(&port(projected, "result"))
        .unwrap();
    let actual = after
        .concrete_interface()
        .port(&port(projected, "result"))
        .unwrap();
    assert_eq!(actual.schema_state, expected.schema_state);
    assert_eq!(actual.type_state, expected.type_state);
    assert!(after.ready().is_none());
    assert!(after.diagnostics().iter().any(|issue| {
        issue.primary == crate::GraphDiagnosticLocation::Connection(invalid)
            && issue.code.as_str() == yss_graph_diagnostics::GraphDiagnosticKind::PortUnknown.code()
    }));
}

#[test]
fn missing_source_is_a_connection_error_not_a_value_cycle() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let resources = catalog(None);
    let mut document = GraphDocument::default();
    let [_, _, projected, _] = branch(&mut document, "databases/b");
    let target = node(&mut document, "yssbi.core.reroute", &[]);
    let invalid = connect(
        &mut document,
        port(NodeId::new(), "output"),
        port(target, "input"),
    );
    let mut cache = GraphSemanticCache::default();

    let facts = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(facts.ready().is_none());
    assert!(facts.diagnostics().iter().any(|issue| {
        issue.primary == crate::GraphDiagnosticLocation::Connection(invalid)
            && issue.code.as_str() == yss_graph_diagnostics::GraphDiagnosticKind::PortUnknown.code()
    }));
    assert!(!facts.diagnostics().iter().any(|issue| {
        issue.code.as_str()
            == yss_graph_diagnostics::GraphDiagnosticKind::DependencyValueCycle.code()
    }));
    let unaffected = facts
        .concrete_interface()
        .port(&port(projected, "result"))
        .unwrap();
    assert!(unaffected.schema_state.exact().is_some());
    assert!(unaffected.type_state.exact().is_some());
}

#[test]
fn schema_cache_stops_invalidation_when_upstream_schema_is_unchanged() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let a = branch(&mut document, "databases/a");
    branch(&mut document, "databases/b");
    let mut cache = GraphSemanticCache::default();
    let resources = catalog(Some(ValueType::Scalar(
        yss_data_contract::SemanticType::Numeric,
    )));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    document
        .nodes
        .get_mut(&a[1])
        .unwrap()
        .parameters
        .insert("rows".parse().unwrap(), serde_json::json!(20));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 7);
    assert_matches_full(
        &document,
        &registry,
        &catalog(Some(ValueType::Scalar(
            yss_data_contract::SemanticType::Text,
        ))),
        &mut cache,
    );
    assert_eq!(
        cache.schemas.reused_outputs, 4,
        "only the other branch retains its Schema"
    );
}

#[test]
fn schema_cache_preserves_absent_reads_and_recovers_from_missing_resources() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    branch(&mut document, "databases/a");
    let mut cache = GraphSemanticCache::default();
    assert_matches_full(&document, &registry, &catalog(None), &mut cache);
    assert_matches_full(&document, &registry, &catalog(None), &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 4);
    let recovered = assert_matches_full(
        &document,
        &registry,
        &catalog(Some(ValueType::Scalar(
            yss_data_contract::SemanticType::Numeric,
        ))),
        &mut cache,
    );
    assert_eq!(cache.schemas.reused_outputs, 0);
    assert!(recovered.ready().is_some(), "{:?}", recovered.diagnostics());
    assert_matches_full(&document, &registry, &catalog(None), &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 0);
}

#[test]
fn schema_cache_tracks_rewiring_cycles_and_deleted_outputs() {
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let a = branch(&mut document, "databases/a");
    let b = branch(&mut document, "databases/b");
    let resources = catalog(Some(ValueType::Scalar(
        yss_data_contract::SemanticType::Numeric,
    )));
    let mut cache = GraphSemanticCache::default();
    assert_matches_full(&document, &registry, &resources, &mut cache);
    let edge = document
        .connections
        .values()
        .find(|edge| edge.input == port(a[1], "source"))
        .unwrap()
        .id;
    document.connections.get_mut(&edge).unwrap().output = port(b[0], "dataframe");
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 5);
    document.connections.get_mut(&edge).unwrap().output = port(a[3], "output");
    let cyclic = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(cyclic.nodes_ready(&b.into_iter().collect()));
    assert!(!cyclic.nodes_ready(&a.into_iter().collect()));
    assert!(
        cache
            .schemas
            .outputs
            .keys()
            .all(|address| !a[1..].contains(&address.node_id))
    );
    assert_eq!(cache.schemas.outputs.len(), 5);
    document.connections.get_mut(&edge).unwrap().output = port(a[0], "dataframe");
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 5);
    document.nodes.remove(&a[3]);
    document
        .connections
        .retain(|_, edge| edge.input.node_id != a[3] && edge.output.node_id != a[3]);
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(
        cache
            .schemas
            .outputs
            .keys()
            .all(|address| address.node_id != a[3])
    );
}

#[test]
fn schema_cache_rechecks_tabular_constant_cells_without_recomputing_unchanged_downstream_schema() {
    use yss_data_contract::DataValue;
    use yss_graph_document::{ConstantId, GraphConstant, normalize_constant_value};
    let registry = yss_node_catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let mut document = GraphDocument::default();
    let id = ConstantId::new();
    let definition = |json: &str| {
        let mut constant = GraphConstant {
            id,
            name: "Table".into(),
            data_type: ValueType::DataFrame,
            data_value: DataValue::String(json.into()),
            tabular: None,
            description: String::new(),
            tags: vec![],
        };
        normalize_constant_value(&mut constant).unwrap();
        constant
    };
    document
        .constants
        .insert(id, definition(r#"{"amount":[1]}"#));
    let source = node(
        &mut document,
        "yssbi.constant.get",
        &[("constant", serde_json::json!(id.to_string()))],
    );
    let consumer = node(
        &mut document,
        "yssbi.dataframe.project",
        &[("columns", serde_json::json!(["amount"]))],
    );
    connect(
        &mut document,
        port(source, "value"),
        port(consumer, "source"),
    );
    let mut cache = GraphSemanticCache::default();
    let resources = catalog(None);
    assert_matches_full(&document, &registry, &resources, &mut cache);
    document
        .constants
        .insert(id, definition(r#"{"amount":[2]}"#));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 1);
    document
        .constants
        .insert(id, definition(r#"{"amount":["label"]}"#));
    assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_eq!(cache.schemas.reused_outputs, 0);
}
