use super::*;

#[test]
fn variable_data_series_materializes_artifact_without_serialized_runtime_internals() {
    let id = yss_variable_contract::VariableId::new();
    let variable = yss_variable_contract::VariableInstance {
        id,
        name: "observations".into(),
        data_type: yss_data_contract::DataType::DataSeries(Box::new(
            yss_data_contract::DataType::Int64,
        )),
        data_value: yss_data_contract::DataValue::DataSeries(
            yss_data_contract::DataSeriesValue::new(
                crate::project::variable_tabular::variable_handle(&id),
            ),
        ),
        tabular: Some(serde_json::from_str(r#"{"columns":{"observations":[1,null,3]}}"#).unwrap()),
        description: String::new(),
        scope: yss_variable_contract::VariableScope::Global,
        tags: Vec::new(),
    };

    let (result, _) = execute_variable_kernel("yssbi.project.variable.get", variable.clone(), &[]);
    let output = result.unwrap();
    let artifact = require_data_series(&output[0]).unwrap();

    assert_eq!(
        artifact.data_series_metadata(),
        Some(&DataSeriesMetadata {
            element_type: DataSeriesElementType::Int64,
            length: 3,
            null_count: 1,
            name: Some("observations".into()),
            format: None,
        })
    );
    assert_eq!(
        serde_json::to_value(variable).unwrap()["dataValue"],
        serde_json::json!({"DataSeries": crate::project::variable_tabular::variable_handle(&id)})
    );
}

#[test]
fn variable_data_series_set_serializes_payload_without_artifact_internals() {
    let id = yss_variable_contract::VariableId::new();
    let variable = yss_variable_contract::VariableInstance {
        id,
        name: "observations".into(),
        data_type: yss_data_contract::DataType::DataSeries(Box::new(
            yss_data_contract::DataType::Float64,
        )),
        data_value: yss_data_contract::DataValue::Null,
        tabular: None,
        description: String::new(),
        scope: yss_variable_contract::VariableScope::Global,
        tags: Vec::new(),
    };
    let input = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .values([decimal("1.5"), Value::Null, decimal("3.5")])
            .name("fitted")
            .format("number")
            .build(ArtifactKind::Collected)
            .unwrap(),
    );

    let (result, snapshot) =
        execute_variable_kernel("yssbi.project.variable.set", variable, &[input]);
    result.unwrap();
    let effects = snapshot.variable_effects();

    assert_eq!(effects.len(), 1);
    let yss_data_contract::DataValue::DataSeries(value) = &effects[0].after else {
        panic!("DataSeries assignment must persist a serializable DataSeries value");
    };
    assert_eq!(
        value.element_type,
        Some(yss_data_contract::DataType::Float64)
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&value.id).unwrap(),
        serde_json::json!({"fitted": [1.5, null, 3.5]})
    );
    assert!(!value.id.contains("artifact"));
}

#[test]
fn data_series_variable_get_set_flows_into_statistics() {
    let id = yss_variable_contract::VariableId::new();
    let empty = yss_variable_contract::VariableInstance {
        id,
        name: "observations".into(),
        data_type: yss_data_contract::DataType::DataSeries(Box::new(
            yss_data_contract::DataType::Float64,
        )),
        data_value: yss_data_contract::DataValue::Null,
        tabular: None,
        description: String::new(),
        scope: yss_variable_contract::VariableScope::Global,
        tags: Vec::new(),
    };
    let assigned = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .values([decimal("1"), decimal("2"), decimal("4")])
            .name("observations")
            .format("number")
            .build(ArtifactKind::Collected)
            .unwrap(),
    );
    let (set, snapshot) =
        execute_variable_kernel("yssbi.project.variable.set", empty.clone(), &[assigned]);
    set.unwrap();
    let after = snapshot.variable_effects().remove(0).after;
    let yss_data_contract::DataValue::DataSeries(series) = &after else {
        panic!("variable effect must persist a DataSeries payload");
    };
    let series_json = series.id.clone();
    let persisted = yss_variable_contract::VariableInstance {
        data_value: after,
        tabular: Some(serde_json::from_str(&format!(r#"{{"columns":{series_json}}}"#)).unwrap()),
        ..empty
    };

    let (get, _) = execute_variable_kernel("yssbi.project.variable.get", persisted, &[]);
    let values = get.unwrap();
    let metadata = require_data_series(&values[0])
        .unwrap()
        .data_series_metadata()
        .unwrap();
    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.length, 3);
    assert_eq!(metadata.name.as_deref(), Some("observations"));
    let fit = execute_ols_fit(int_series([1, 2, 3]), vec![values[0].clone()]).unwrap();
    assert_eq!(series_element_type(&fit[1]), DataSeriesElementType::Float64);
    assert_eq!(
        require_data_series(&fit[1])
            .unwrap()
            .data_series_metadata()
            .unwrap()
            .length,
        3
    );
}

#[test]
fn scatter_consumes_two_numeric_data_series_artifacts() {
    let x = named_data_series(
        DataSeriesElementType::Int64,
        "observed x",
        [Value::Integer(1), Value::Integer(2), Value::Integer(3)],
    );
    let y = named_data_series(
        DataSeriesElementType::Float64,
        "observed y",
        [decimal("1.5"), decimal("2.5"), decimal("3.5")],
    );

    let (result, sink) = execute_plot_kernel("yssbi.plot.scatter.view", &[x, y]);

    assert_eq!(
        result.unwrap(),
        vec![RuntimeValue::Scalar(Value::String(
            "presentation:test".into()
        ))]
    );
    let publications = sink.publications.lock().unwrap();
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].0, PlotKind::Scatter);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&publications[0].1).unwrap()["data"],
        serde_json::json!([
            { "x": 1.0, "y": 1.5 },
            { "x": 2.0, "y": 2.5 },
            { "x": 3.0, "y": 3.5 }
        ])
    );
}

#[test]
fn correlation_serializes_undefined_statistics_as_null() {
    let left = named_data_series(
        DataSeriesElementType::Int64,
        "constant left",
        [Value::Integer(1), Value::Integer(1), Value::Integer(1)],
    );
    let right = named_data_series(
        DataSeriesElementType::Int64,
        "constant right",
        [Value::Integer(2), Value::Integer(2), Value::Integer(2)],
    );

    let (result, sink) = execute_plot_kernel("yssbi.plot.correlation.view", &[left, right]);

    result.unwrap();
    let publications = sink.publications.lock().unwrap();
    let payload = serde_json::from_str::<serde_json::Value>(&publications[0].1).unwrap();
    assert_eq!(
        payload["matrix"],
        serde_json::json!([[null, null], [null, null]])
    );
    assert_eq!(
        payload["pMatrix"],
        serde_json::json!([[null, null], [null, null]])
    );
    assert_ne!(payload["matrix"][0][0], serde_json::json!(0.0));
    assert_ne!(payload["pMatrix"][0][0], serde_json::json!(0.0));
}

#[test]
fn plot_rejects_scalar_list_series_input() {
    let input = RuntimeValue::Scalar(Value::List(vec![Value::Integer(1)]));

    let (result, sink) = execute_plot_kernel("yssbi.plot.ecdf.view", &[input]);

    assert_eq!(
        result.unwrap_err().message(),
        "expected DataSeries Artifact, received scalar"
    );
    assert!(sink.publications.lock().unwrap().is_empty());
}

#[test]
fn plot_preserves_data_series_name_and_format_metadata() {
    let x = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Int64)
            .name("calendar period")
            .format("0000")
            .values([Value::Integer(2024), Value::Integer(2025)])
            .build(ArtifactKind::Collected)
            .unwrap(),
    );
    let y = RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .name("revenue")
            .format("$0.00")
            .values([decimal("10.5"), decimal("11.75")])
            .build(ArtifactKind::Collected)
            .unwrap(),
    );

    let (result, sink) = execute_plot_kernel("yssbi.plot.line.view", &[x, y]);

    result.unwrap();
    let publications = sink.publications.lock().unwrap();
    let payload = serde_json::from_str::<serde_json::Value>(&publications[0].1).unwrap();
    assert_eq!(payload["xLabel"], "calendar period");
    assert_eq!(payload["yLabel"], "revenue");
    assert_eq!(payload["xFormat"], "0000");
    assert_eq!(payload["yFormat"], "$0.00");
}
