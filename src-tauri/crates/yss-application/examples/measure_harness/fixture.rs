//! Deterministic graph setup, outside measured model turns, through existing owners.

use std::time::Duration;

use serde_json::json;
use yss_application::ApplicationState;
use yss_harness_contract::{model::*, *};
use yss_project_identity::OperationId;

use super::Error;

pub fn regression_graph(application: &ApplicationState, database_id: &str) -> Result<(), Error> {
    let captured = application.capture_session()?;
    application.create_event_graph(
        captured.project_instance_id().clone(),
        "Harness Regression".into(),
        OperationId::new(),
    )?;
    let index = application
        .query_project_index(captured.project_instance_id().clone(), "en-US", false)?
        .index;
    let path = &index
        .event_graphs
        .iter()
        .find(|entry| entry.name == "Harness Regression")
        .ok_or("fixture graph missing")?
        .path;
    let graph = GraphResourceRef::for_path(path);
    let input: CreateNodesInput = serde_json::from_value(json!({
        "graph": graph,
        "nodes": [
            {"clientId":"source", "typeId":"yssbi.dataframe.source.get", "resourcePath":format!("databases/{database_id}")},
            {"clientId":"columns", "typeId":"yssbi.dataframe.decompose"},
            {"clientId":"fit", "typeId":"yssbi.statistics.linear.fit", "parameters":{"method":"OLS"}, "portCounts":{"x":5}, "label":"Target regression"},
            {"clientId":"summary", "typeId":"yssbi.statistics.linear.summary", "parameters":{"observations":true,"diagnostics":true}, "label":"Target regression summary"},
            {"clientId":"diagnostic", "typeId":"yssbi.statistics.diagnostic.breusch_pagan", "label":"Target heteroskedasticity"}
        ],
        "connections": [{"output":port("$source", "dataframe"), "input":port("$columns", "dataframe")}]
    }))?;
    let created = edit(application, GraphMutationInput::CreateNodes(input))?;
    let columns = created
        .changes
        .nodes
        .iter()
        .find(|node| node.node_id == created.created_nodes["columns"])
        .ok_or("fixture columns missing")?;
    let column = |name: &str| -> Result<GraphEditPortRef, Error> {
        Ok(columns
            .ports
            .iter()
            .find(|port| port.direction == "output" && port.schema.contains_key(name))
            .ok_or("fixture CSV must have numeric x1..x5,y columns")?
            .address
            .clone())
    };
    let fit = &created.created_nodes["fit"];
    let mut connections = vec![connection(column("y")?, port(fit, "y"))];
    for i in 0..5 {
        connections.push(connection(
            column(&format!("x{}", i + 1))?,
            created.created_ports[&format!("fit.x[{i}]")].clone(),
        ));
    }
    for name in ["summary", "diagnostic"] {
        connections.push(connection(
            port(fit, "model"),
            port(&created.created_nodes[name], "model"),
        ));
    }
    edit(
        application,
        GraphMutationInput::CreateConnections(CreateConnectionsInput {
            graph: graph.clone(),
            connections,
        }),
    )?;
    // One unrelated branch remains invalid; the others use literals. Scoped
    // execution must not demand that branch or run the unrelated calculations.
    let nodes = (0..200)
        .map(|index| {
            serde_json::from_value(json!({
                "clientId":format!("noise-{index:03}"), "typeId":"yssbi.numeric.multiply", "label":format!("Unrelated {index:03}"),
                "position":{"x":1000 + (index % 10) * 180, "y":(index / 10) * 120}
            }))
        })
        .collect::<Result<Vec<NodeDeclaration>, _>>()?;
    for batch in nodes.chunks(20) {
        let created = edit(
            application,
            GraphMutationInput::CreateNodes(CreateNodesInput {
                graph: graph.clone(),
                nodes: batch.to_vec(),
                connections: vec![],
            }),
        )?;
        let nodes = created.created_nodes.iter().filter(|(alias, _)| alias.as_str() != "noise-000")
            .map(|(_, id)| serde_json::from_value(json!({
                "nodeId": id,
                "literals": [{"address":port(id, "left"),"value":1}, {"address":port(id, "right"),"value":2}]
            }))).collect::<Result<Vec<NodeUpdate>, _>>()?;
        edit(
            application,
            GraphMutationInput::UpdateNodes(UpdateNodesInput {
                graph: graph.clone(),
                nodes,
            }),
        )?;
    }
    println!("Prepared Harness Regression: 205 nodes, 5 X inputs, 9 connections");
    Ok(())
}

fn edit(
    application: &ApplicationState,
    input: GraphMutationInput,
) -> Result<GraphEditReceipt, Error> {
    let AutomationCapabilityResult::GraphInspectionPage(observation) = invoke(
        application,
        AutomationCapabilityRequest::InspectGraph(InspectGraphRequest::summary(
            input.graph().clone(),
        )),
    )?
    else {
        return Err("fixture graph observation unavailable".into());
    };
    let AutomationCapabilityResult::GraphEditReceipt(receipt) = invoke(
        application,
        AutomationCapabilityRequest::GraphMutation(GraphMutationRequest {
            input,
            base_revision: observation.version.revision,
            graph_hash: observation.graph_hash,
            client_key: uuid::Uuid::new_v4().to_string(),
        }),
    )?
    else {
        return Err("fixture edit receipt unavailable".into());
    };
    Ok(receipt)
}

fn invoke(
    application: &ApplicationState,
    request: AutomationCapabilityRequest,
) -> Result<AutomationCapabilityResult, Error> {
    let captured = application.capture_session()?;
    let context = CapabilityInvocationContext::new(
        PrincipalId::try_new("measurement-fixture")?,
        HarnessSessionId::try_new("measurement-fixture")?,
        CapabilityInvocationId::try_new(uuid::Uuid::new_v4().to_string())?,
        ProjectSessionBinding::new(
            captured.project_instance_id().clone(),
            captured.project_session_id().clone(),
        ),
    );
    Ok(application.invoke_automation_capability(
        context,
        request,
        &CapabilityControl::new(CancellationToken::default(), Duration::from_secs(60)),
        &mut |_| {},
    )?)
}

fn port(node: &str, key: &str) -> GraphEditPortRef {
    GraphEditPortRef::Declared {
        node_id: node.into(),
        port_key: key.into(),
    }
}

fn connection(output: GraphEditPortRef, input: GraphEditPortRef) -> ConnectionDeclaration {
    ConnectionDeclaration {
        output,
        input,
        order: None,
    }
}
