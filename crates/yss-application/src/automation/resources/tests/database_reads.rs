use super::*;
use model::*;
use serde_json::json;

pub(super) fn read(
    f: &mut Fixture,
    input: DatabaseReadInput,
    version: Option<ResourceVersion>,
) -> Result<DatabaseReadResult> {
    match f.call(AutomationCapabilityRequest::ReadDatabase(
        DatabaseReadRequest { input, version },
    ))? {
        AutomationCapabilityResult::DatabaseRead(value) => Ok(value),
        _ => panic!("database read"),
    }
}

#[test]
fn selected_database_tools_page_schema_and_rows_without_losing_identity_or_read_baselines() {
    let mut f = Fixture::new();
    let resource = f.dataset();
    let database = DatabaseResourceRef::new(resource.id.clone());
    let overview = read(
        &mut f,
        DatabaseReadInput::Overview(InspectDatabaseInput {
            database: database.clone(),
        }),
        None,
    )
    .unwrap();
    assert!(matches!(
        &overview.content,
        DatabaseReadContent::Overview {
            row_count: 3,
            column_count: 2,
            ..
        }
    ));
    let visible =
        model::capability_result(&AutomationCapabilityResult::DatabaseRead(overview.clone()))
            .unwrap();
    assert!(visible["payload"].get("version").is_none());
    assert!(visible["payload"]["content"].get("columns").is_none());
    let schema = |offset| {
        DatabaseReadInput::Schema(InspectDatabaseSchemaInput {
            database: database.clone(),
            columns: vec![],
            offset,
            limit: 1,
        })
    };
    let first = read(&mut f, schema(0), Some(overview.version.clone())).unwrap();
    let DatabaseReadContent::Schema { columns, page } = first.content else {
        panic!("schema");
    };
    assert_eq!(columns[0].name, "x");
    assert_eq!(page, InspectionPage::known(0, 1, 2));
    let last = read(&mut f, schema(1), Some(overview.version.clone())).unwrap();
    let DatabaseReadContent::Schema { columns, page } = last.content else {
        panic!("schema");
    };
    assert_eq!(columns[0].name, "label");
    assert!(!page.has_more);
    let rows = |offset| {
        DatabaseReadInput::Rows(serde_json::from_value(json!({
        "database": database, "columns": ["label"], "filters": [{"column":"x","comparison":"greater_equal","value":{"type":"integer","value":"2"}}],
        "order": [{"column":"x","ascending":false,"nullsFirst":false}], "offset":offset,"limit":1,
    })).unwrap())
    };
    let first = read(&mut f, rows(0), Some(overview.version.clone())).unwrap();
    let DatabaseReadContent::Rows {
        columns,
        rows: values,
        row_ids,
        page,
    } = first.content
    else {
        panic!("rows");
    };
    assert_eq!(columns, ["label"]);
    assert_eq!(serde_json::to_value(values).unwrap(), json!([["c"]]));
    assert_eq!(row_ids, [2]);
    assert_eq!(page.total, None);
    assert_eq!(page.next_offset, Some(1));
    let next = read(
        &mut f,
        rows(page.next_offset.unwrap()),
        Some(overview.version.clone()),
    )
    .unwrap();
    let DatabaseReadContent::Rows { row_ids, page, .. } = next.content else {
        panic!("rows");
    };
    assert_eq!(row_ids, [1]);
    assert!(!page.has_more);
    f.call(AutomationCapabilityRequest::EditResource(
        EditResourceRequest {
            resource,
            version: overview.version.clone(),
            edit: ResourceEdit::UpdateCells {
                cells: vec![DatabaseCellEdit {
                    row_id: 2,
                    column: "label".into(),
                    value: serde_json::from_value(json!("edited")).unwrap(),
                }],
            },
        },
    ))
    .unwrap();
    assert_eq!(
        read(&mut f, rows(0), Some(overview.version))
            .unwrap_err()
            .code,
        CapabilityFailureCode::RevisionConflict
    );
    let current = read(&mut f, rows(0), None).unwrap();
    let DatabaseReadContent::Rows {
        rows: values,
        row_ids,
        ..
    } = current.content
    else {
        panic!("rows");
    };
    assert_eq!(row_ids, [2]);
    assert_eq!(serde_json::to_value(values).unwrap(), json!([["edited"]]));
    let invalid = DatabaseReadInput::Rows(
        serde_json::from_value(json!({"database":database,"columns":["missing"]})).unwrap(),
    );
    let error = read(&mut f, invalid, None).unwrap_err();
    assert_eq!(error.code, CapabilityFailureCode::InvalidRequest);
    assert_eq!(
        error.details.get("column").map(String::as_str),
        Some("missing")
    );
}

#[test]
fn database_profiles_select_metrics_and_import_names_belong_to_the_original_commit() {
    let mut f = Fixture::new();
    let csv = f.directory.join("source.csv");
    std::fs::write(&csv, "x,label\n1,a\n2,b\n3,c\n").unwrap();
    let resource = f.create(ResourceCreation::Database {
        name: Some("Chosen name".into()),
        source: DatasetImportSource::Csv {
            path: csv.to_string_lossy().into(),
            delimiter: ',',
            has_header: true,
            infer_schema_length: Some(10),
        },
    });
    assert_eq!(
        f.publications.len(),
        1,
        "the name must not require a second rename commit"
    );
    let database = DatabaseResourceRef::new(resource.id.clone());
    let overview = read(
        &mut f,
        DatabaseReadInput::Overview(InspectDatabaseInput {
            database: database.clone(),
        }),
        None,
    )
    .unwrap();
    assert!(
        matches!(overview.content, DatabaseReadContent::Overview { ref name, .. } if name == "Chosen name")
    );
    let profile = |columns: Vec<&str>, metrics| {
        DatabaseReadInput::Profile(ProfileDatabaseInput {
            database: database.clone(),
            columns: columns.into_iter().map(String::from).collect(),
            metrics,
        })
    };
    let statistics = read(
        &mut f,
        profile(vec!["x"], vec![DatabaseProfileMetric::Statistics]),
        Some(overview.version.clone()),
    )
    .unwrap();
    let visible =
        model::capability_result(&AutomationCapabilityResult::DatabaseRead(statistics)).unwrap();
    let metrics = &visible["payload"]["content"]["metrics"];
    assert!(metrics.get("completeness").is_none());
    assert!(metrics.get("distributions").is_none());
    assert_eq!(metrics["statistics"].as_array().unwrap().len(), 1);
    assert_eq!(metrics["statistics"][0]["column"], "x");
    assert_eq!(metrics["statistics"][0]["mean"], 2.0);
    let all = read(
        &mut f,
        profile(
            vec!["label"],
            vec![
                DatabaseProfileMetric::Completeness,
                DatabaseProfileMetric::Statistics,
                DatabaseProfileMetric::Distribution,
            ],
        ),
        Some(overview.version.clone()),
    )
    .unwrap();
    let DatabaseReadContent::Profile { columns, metrics } = all.content else {
        panic!("profile");
    };
    assert_eq!(columns, ["label"]);
    let completeness = metrics.completeness.unwrap();
    assert_eq!(
        (
            completeness.row_count,
            completeness.column_count,
            completeness.total_nulls
        ),
        (3, 1, 0)
    );
    assert!(
        matches!(&metrics.statistics.unwrap()[0], DatabaseColumnStatistics::Categorical { column, unique: 3, .. } if column == "label")
    );
    assert!(
        matches!(&metrics.distributions.unwrap()[0], DatabaseColumnDistribution::Categorical { column, categories, other_count: 0 } if column == "label" && categories.len() == 3)
    );
    assert_eq!(
        read(
            &mut f,
            profile(vec![], vec![DatabaseProfileMetric::Statistics]),
            None
        )
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );
    assert_eq!(
        read(
            &mut f,
            profile(
                vec!["x"],
                vec![
                    DatabaseProfileMetric::Statistics,
                    DatabaseProfileMetric::Statistics
                ]
            ),
            None
        )
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );
    let cancellation = CancellationToken::default();
    cancellation.cancel(CancellationReason::User);
    assert_eq!(
        f.application
            .as_ref()
            .unwrap()
            .invoke_automation_capability(
                f.context.clone(),
                AutomationCapabilityRequest::ReadDatabase(DatabaseReadRequest {
                    input: profile(vec!["x"], vec![DatabaseProfileMetric::Distribution]),
                    version: Some(overview.version)
                }),
                &CapabilityControl::new(cancellation, Duration::from_secs(30)),
                &mut |_| {},
            )
            .unwrap_err()
            .code,
        CapabilityFailureCode::Cancelled
    );
}
