use yssbi_lib::database::{
    dataframe_to_schema, DatabaseDecl, DatabaseEngine, DatabaseInstance, DatabaseState,
};
use yssbi_lib::execution::NodeExecutionContext;
use yssbi_lib::graph::core::SchemaProvider;
use yssbi_lib::graph::node::NodeInstanceParams;
use yssbi_lib::graph::pin::{DataRole, PinRole};
use yssbi_lib::graph::register::NodeRegistry;
use yssbi_lib::graph::value::DataValue;
use yssbi_lib::graph::{GraphInstance, GraphKind, GraphRuntime};
use yssbi_lib::project::{ProjectData, ProjectStore};
use std::sync::{Arc, Mutex, RwLock};

fn create_test_registry() -> Arc<NodeRegistry> {
    let registry = Arc::new(NodeRegistry::new());
    yssbi_lib::graph::register::catalog::register_builtin_nodes(&registry);
    registry
}

fn load_iris_database() -> DatabaseInstance {
    let engine = DatabaseEngine::Csv {
        path: "tests/data/iris.csv".to_string(),
        delimiter: ',',
        has_header: true,
        infer_schema_length: Some(100),
    };
    let decl = DatabaseDecl {
        id: "iris_dataset".to_string(),
        engine: engine.clone(),
        schema_version: 1,
        required: true,
    };
    let lazy_frame = engine.build_lazy().expect("Failed to build lazy frame from CSV");
    DatabaseInstance {
        decl,
        state: DatabaseState::Lazy { lazy_frame },
    }
}

/// Full pipeline: iris.csv -> Get DataFrame -> Decompose DataFrame -> Series Mean -> verify
#[test]
fn test_dataframe_pipeline_iris_mean() {
    // 1. Set up ProjectStore with the iris database
    let project_data = Arc::new(RwLock::new(ProjectData::new()));
    let project_store = Arc::new(RwLock::new(ProjectStore::new()));
    {
        let mut store = project_store.write().unwrap();
        store.databases.insert("iris_dataset".to_string(), load_iris_database());
    }

    // 2. Build a schema provider from the project store
    let schema_provider: SchemaProvider = {
        let store = Arc::clone(&project_store);
        Arc::new(move |dataframe_id: &str| {
            let mut s = store.write().ok()?;
            let db = s.databases.get_mut(dataframe_id)?;
            let df = db.ensure_loaded().ok()?;
            Some(dataframe_to_schema(df))
        })
    };

    // 3. Create the graph with registry and schema provider
    let registry = create_test_registry();
    let mut graph = GraphInstance::new("DataFrame Pipeline Test", GraphKind::Event, registry);
    graph.set_schema_provider(schema_provider);
    let graph = Arc::new(graph);

    // 4. Create "Get DataFrame" node with dataframe_id = "iris_dataset"
    let get_df_node = graph
        .create_node_with_position(
            "get_dataframe",
            0.0,
            0.0,
            Some(NodeInstanceParams {
                dataframe_id: Some("iris_dataset".to_string()),
                ..Default::default()
            }),
        )
        .expect("Failed to create Get DataFrame node");

    // 5. Create "Decompose DataFrame" node
    let decompose_node = graph
        .create_node("decompose_dataframe")
        .expect("Failed to create Decompose DataFrame node");

    // 6. Connect Get DataFrame output -> Decompose DataFrame input
    let get_df_output_pin = graph
        .get_pin_instances_by_node_id(get_df_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output) && p.is_output())
        .expect("Get DataFrame output pin not found");

    let decompose_input_pin = graph
        .get_pin_instances_by_node_id(decompose_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input) && p.is_input())
        .expect("Decompose DataFrame input pin not found");

    let (_, change_sets) = graph
        .connect(get_df_output_pin.id, decompose_input_pin.id)
        .expect("Failed to connect Get DataFrame to Decompose DataFrame");

    // 7. Verify dynamic pins were created
    println!("\n=== Dynamic Pin Changes ===");
    for cs in &change_sets {
        println!("  Node {:?}: removed {} pins, added {} pins", cs.node_id, cs.removed_pin_ids.len(), cs.added_pins.len());
        for pin in &cs.added_pins {
            println!("    + {} ({:?})", pin.definition.name, pin.definition.role);
        }
    }

    let decompose_pins = graph.get_pin_instances_by_node_id(decompose_node);
    let output_pins: Vec<_> = decompose_pins.iter().filter(|p| p.is_output()).collect();
    println!("\nDecompose DataFrame output pins: {}", output_pins.len());
    for pin in &output_pins {
        println!("  - {} ({:?})", pin.definition.name, pin.definition.role);
    }

    assert!(
        output_pins.len() >= 5,
        "Expected at least 5 output pins (one per column), got {}",
        output_pins.len()
    );

    let sepal_length_pin = decompose_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Custom("sepal_length".to_string())) && p.is_output())
        .expect("sepal_length output pin not found on Decompose DataFrame");

    let sepal_width_pin = decompose_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Custom("sepal_width".to_string())) && p.is_output())
        .expect("sepal_width output pin not found on Decompose DataFrame");

    // 8. Create two Series Mean nodes
    let mean1_node = graph
        .create_node("series_mean")
        .expect("Failed to create Series Mean 1");
    let mean2_node = graph
        .create_node("series_mean")
        .expect("Failed to create Series Mean 2");

    // 9. Connect Decompose's sepal_length -> Series Mean 1 input
    let mean1_input = graph
        .get_pin_instances_by_node_id(mean1_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input) && p.is_input())
        .expect("Series Mean 1 input pin not found");

    graph
        .connect(sepal_length_pin.id, mean1_input.id)
        .expect("Failed to connect sepal_length to Series Mean 1");

    // 10. Connect Decompose's sepal_width -> Series Mean 2 input
    let mean2_input = graph
        .get_pin_instances_by_node_id(mean2_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input) && p.is_input())
        .expect("Series Mean 2 input pin not found");

    graph
        .connect(sepal_width_pin.id, mean2_input.id)
        .expect("Failed to connect sepal_width to Series Mean 2");

    // 11. Create GraphRuntime with project data
    let graph_runtime = Arc::new(Mutex::new(GraphRuntime::new(
        graph.clone(),
        project_data,
        project_store,
    )));

    // 12. Execute nodes in topological order using NodeExecutionContext
    let nodes_in_order = [get_df_node, decompose_node, mean1_node, mean2_node];
    for &node_id in &nodes_in_order {
        let node_instance = graph.get_node_instance(node_id).unwrap();
        let def = &node_instance.definition;
        if let Some(evaluator) = &def.data_evaluator {
            let mut ctx = NodeExecutionContext::new(graph_runtime.clone(), node_id);
            evaluator(&mut ctx).unwrap_or_else(|e| {
                panic!(
                    "Failed to evaluate node '{}' ({:?}): {}",
                    def.name, node_id, e
                );
            });
        }
    }

    // 13. Read the output values from Series Mean nodes
    let runtime = graph_runtime.lock().unwrap();

    let mean1_output = runtime.get_pin_data_value_by_pin_role(
        mean1_node,
        &PinRole::Data(DataRole::Output),
    );
    let mean2_output = runtime.get_pin_data_value_by_pin_role(
        mean2_node,
        &PinRole::Data(DataRole::Output),
    );

    println!("\n=== Results ===");
    println!("sepal_length mean: {:?}", mean1_output);
    println!("sepal_width mean:  {:?}", mean2_output);

    // 14. Verify values
    let sepal_length_mean = match mean1_output {
        DataValue::Float64(v) => v,
        other => panic!("Expected Float64 for sepal_length mean, got {:?}", other),
    };
    let sepal_width_mean = match mean2_output {
        DataValue::Float64(v) => v,
        other => panic!("Expected Float64 for sepal_width mean, got {:?}", other),
    };

    // Computed by Polars from the project's iris.csv (150 rows)
    let expected_sepal_length_mean = 5.843333333333333;
    let expected_sepal_width_mean = 3.0540000000000003;

    assert!(
        (sepal_length_mean - expected_sepal_length_mean).abs() < 1e-6,
        "sepal_length mean mismatch: got {}, expected {}",
        sepal_length_mean,
        expected_sepal_length_mean
    );
    assert!(
        (sepal_width_mean - expected_sepal_width_mean).abs() < 1e-6,
        "sepal_width mean mismatch: got {}, expected {}",
        sepal_width_mean,
        expected_sepal_width_mean
    );

    println!("\n=== Test Passed ===");
    println!(
        "sepal_length mean = {} (expected {})",
        sepal_length_mean, expected_sepal_length_mean
    );
    println!(
        "sepal_width mean  = {} (expected {})",
        sepal_width_mean, expected_sepal_width_mean
    );
}
