use super::*;
use crate::graph::inputs::{ProjectGraphResourceSnapshot, build_resource_catalog};
use std::num::NonZeroU64;
use yss_data_contract::ValueType;
use yss_database_contract::{
    DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseSessionIdentity,
    DatabaseSessionOpenRequest,
};
use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
use yss_graph_document::GraphResourcePath;
use yss_graph_resource_contract::FunctionParameterContract;
use yss_graph_resource_contract::{FunctionSignature, GraphResourceId};

#[test]
fn project_and_database_snapshots_map_to_complete_graph_catalog_and_settings() {
    let mut fixture = yss_database_runtime::test_support::DatasetFixture::single_f64(
        yss_database_runtime::test_support::SALES_ID,
        "amount",
        vec![1.0],
    );
    fixture.instance.decl.required = true;
    fixture.instance.decl.name = "Sales".into();
    let database = fixture.instance.decl.clone();
    let observations = DatabaseDeclarationObservationSet::try_from_iter([(
        database.id.clone(),
        DatabaseDeclarationObservation::new(
            DatabaseDeclarationRevision::from_existing(1),
            DatabaseDeclarationFingerprint::from_decl(&database),
        ),
    )])
    .unwrap();
    let runtime = DatabaseRuntimeRegistry::new()
        .open_session_with_instances(
            DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing("session".into()),
                NonZeroU64::new(1).unwrap(),
                vec![database.clone()].into(),
                observations,
            ),
            [fixture.instance.clone()],
        )
        .unwrap();
    let schema = yss_database_runtime::session_api::catalog_snapshot(&runtime).unwrap();
    let function_path = GraphResourcePath::new("functions/forecast.yssbi-function").unwrap();
    let mut functions = BTreeMap::new();
    functions.insert(
        function_path.clone(),
        FunctionSignature::new(
            vec![FunctionParameterContract::new(
                yss_graph_document::FunctionParameterId::new("x"),
                "X",
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            )],
            Some(ValueType::Scalar(yss_data_contract::SemanticType::Numeric)),
        ),
    );
    let mut databases = BTreeMap::new();
    databases.insert(database.id.clone(), database);

    let project = ProjectGraphResourceSnapshot::new(
        yss_project_identity::ProjectInstanceId::from_existing("project".into()),
        7,
        functions,
        databases,
    );
    let catalog = build_resource_catalog(&project, &schema).unwrap();
    assert!(catalog.function_signature(&function_path).is_some());
    assert!(
        catalog
            .database_schema(&GraphResourceId::new(format!(
                "databases/{}",
                yss_database_runtime::test_support::SALES_ID
            )))
            .is_some()
    );
}
