use super::*;
use crate::graph::test_support::{TestProject, staged_session};

use std::sync::atomic::AtomicBool;
use yss_data_contract::{DataValue, ValueType};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, NodeId, NodePosition,
    ParameterValues, PortAddress,
};
use yss_graph_execution::plan::{PlanGraphId, PlanOutputRef, PlanPortAddress};
use yss_graph_execution::result::ResultCacheState;
use yss_graph_runtime::GraphRuntimeTestControl;
use yss_project_model::{GraphResourceDocument, ProjectData};

fn node(document: &mut GraphDocument, id: NodeId, node_type: &str) {
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: node_type.parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
}

/// A DataFrame constant decomposed into columns: every output is computable.
fn pipeline(source: NodeId, decompose: NodeId) -> GraphDocument {
    let mut document = GraphDocument::default();
    node(&mut document, source, "yssbi.constant.get");
    let constant = yss_graph_document::ConstantId::from_uuid(source.as_uuid());
    let mut value = yss_graph_document::GraphConstant {
        id: constant,
        name: "frame".into(),
        data_type: ValueType::DataFrame,
        data_value: DataValue::String(
            r#"{"species":["setosa","versicolor"],"sepal":[5.1,7.0]}"#.into(),
        ),
        tabular: None,
        description: String::new(),
        tags: vec![],
    };
    yss_graph_document::normalize_constant_value(&mut value).unwrap();
    document.constants.insert(constant, value);
    document.nodes.get_mut(&source).unwrap().parameters = ParameterValues::from([(
        "constant".parse().unwrap(),
        serde_json::json!(constant.to_string()),
    )]);
    node(&mut document, decompose, "yssbi.dataframe.decompose");
    let connection = ConnectionId::new();
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: PortAddress::declared(decompose, "dataframe".parse().unwrap()),
            order: None,
        },
    );
    document
}

fn project(graphs: [(&GraphResourcePath, GraphDocument); 2]) -> ProjectData {
    let mut data = ProjectData::new();
    for (path, document) in graphs {
        let mut resource =
            GraphResourceDocument::new(path.display_name(), GraphResourceKind::EventGraph);
        resource.document = Arc::new(document);
        data.graphs.insert(path.clone(), resource);
    }
    data
}

fn output(graph: &GraphResourcePath, address: &PortAddress) -> PlanOutputRef {
    PlanOutputRef::new(
        PlanGraphId::from_existing(graph.as_str().into()),
        PlanPortAddress::from_existing(address.to_string().into()),
    )
}

#[test]
fn overview_reads_unopened_graphs_and_computes_requested_outputs() {
    let pipeline_path = GraphResourcePath::new("events/Pipeline.yssbi-event").unwrap();
    let broken_path = GraphResourcePath::new("events/Broken.yssbi-event").unwrap();
    let (source, decompose, viewer) = (NodeId::new(), NodeId::new(), NodeId::new());
    let mut broken = GraphDocument::default();
    node(&mut broken, viewer, "yssbi.debug.view");
    let session = staged_session(
        TestProject::unloaded(
            "overview",
            project([
                (&pipeline_path, pipeline(source, decompose)),
                (&broken_path, broken),
            ]),
        ),
        GraphRuntimeTestControl::default(),
    );
    let app = &session.application;
    let instance = session.session.project_instance_id().clone();
    let resident = |path| session.session.project().has_resident_graph(path).unwrap();

    let overview = app.graph_overviews(&instance, None, "en-US").unwrap();
    assert_eq!(overview.project_graphs.len(), 2);
    assert!(
        app.graph_overviews(
            &instance,
            Some(&[GraphResourcePath::new("events/Missing.yssbi-event").unwrap()]),
            "en-US"
        )
        .unwrap()
        .graphs
        .is_empty(),
        "paths outside the project index are not read"
    );
    assert!(overview.unavailable.is_empty());
    let graph = |overview: &GraphOverviews, path: &GraphResourcePath| {
        overview
            .graphs
            .iter()
            .find(|graph| &graph.graph_path == path)
            .cloned()
            .unwrap()
    };
    assert!(
        !graph(&overview, &broken_path)
            .projection
            .diagnostics
            .is_empty()
    );
    let current = graph(&overview, &pipeline_path);
    let results = current.results.as_ref().unwrap();
    assert!(!results.outputs.is_empty());
    assert!(
        results
            .outputs
            .values()
            .all(|state| *state == ResultCacheState::Missing)
    );
    assert!(!resident(&pipeline_path) && !resident(&broken_path));

    let species = current
        .projection
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .and_then(|node| {
            node.ports
                .iter()
                .find(|port| port.display.label.as_ref() == "species")
        })
        .unwrap()
        .address
        .clone();
    let requested = output(&pipeline_path, &species);
    let receipt = crate::graph::run::run_graph_outputs(
        app,
        &instance,
        &pipeline_path,
        Box::new([requested.clone()]),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert!(
        resident(&pipeline_path),
        "execution reads resident documents"
    );
    assert!(!resident(&broken_path));

    let hash = current.projection.basis.semantic_input_hash;
    let states = app
        .graph_result_states(&instance, &[(pipeline_path.clone(), hash)])
        .unwrap();
    assert!(matches!(
        states[0].1.outputs[&requested],
        ResultCacheState::Valid { .. }
    ));
    let source_value = output(
        &pipeline_path,
        &PortAddress::declared(source, "value".parse().unwrap()),
    );
    assert_eq!(
        states[0].1.outputs[&source_value],
        ResultCacheState::Missing,
        "only the requested output is materialized"
    );
    let reread = app
        .graph_overviews(
            &instance,
            Some(std::slice::from_ref(&pipeline_path)),
            "en-US",
        )
        .unwrap();
    assert_eq!(reread.graphs.len(), 1);
    assert_eq!(
        reread.graphs[0].results.as_ref().unwrap().outputs[&requested],
        states[0].1.outputs[&requested]
    );

    let journal = app.run_journal(&instance).unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].identity(), &receipt.identity);
    assert!(journal[0].outputs().contains(&requested));
    assert!(journal[0].finished_at().is_some() && !journal[0].acknowledged());
    app.acknowledge_runs(&instance, std::slice::from_ref(&receipt.identity))
        .unwrap();
    assert!(app.run_journal(&instance).unwrap()[0].acknowledged());
    assert!(app.execution_snapshot(&instance).unwrap()[0].acknowledged());
}
