use std::num::NonZeroU64;
use std::path::PathBuf;

use serde_json::json;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationObservationSetError,
    DatabaseDeclarationRevision, DatabaseEngine, DatabaseExportFormat, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest, DatabaseSessionOpenRequestError,
};

fn declaration(id: &str, name: &str) -> DatabaseDecl {
    DatabaseDecl {
        id: DatabaseId::from_existing(id.into()),
        engine: DatabaseEngine::InMemory { name: id.into() },
        schema_version: 1,
        required: false,
        name: name.into(),
    }
}

#[test]
fn export_formats_parse_case_insensitively_and_reject_unknown_values() {
    assert_eq!("csv".parse(), Ok(DatabaseExportFormat::Csv));
    assert_eq!("PARQUET".parse(), Ok(DatabaseExportFormat::Parquet));
    assert!("xlsx".parse::<DatabaseExportFormat>().is_err());
}

fn observations_for(declarations: &[DatabaseDecl]) -> DatabaseDeclarationObservationSet {
    DatabaseDeclarationObservationSet::try_from_iter(declarations.iter().map(|declaration| {
        (
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(4),
                DatabaseDeclarationFingerprint::from_decl(declaration),
            ),
        )
    }))
    .unwrap()
}

fn request(
    identity: &str,
    declarations: Vec<DatabaseDecl>,
    observations: DatabaseDeclarationObservationSet,
) -> DatabaseSessionOpenRequest {
    DatabaseSessionOpenRequest::new(
        DatabaseSessionIdentity::from_existing(identity.into()),
        NonZeroU64::new(7).unwrap(),
        Some(PathBuf::from("project")),
        declarations.into(),
        observations,
    )
}

#[test]
fn declaration_preserves_wire_bytes_and_requires_the_display_name() {
    let declaration = declaration("sales", "Sales");
    let bytes = serde_json::to_vec(&declaration).unwrap();
    assert_eq!(
        bytes,
        br#"{"id":"sales","engine":{"inMemory":{"name":"sales"}},"schemaVersion":1,"required":false,"name":"Sales"}"#
    );
    assert_eq!(
        serde_json::from_slice::<DatabaseDecl>(&bytes).unwrap(),
        declaration
    );

    let mut value = serde_json::to_value(declaration).unwrap();
    value.as_object_mut().unwrap().remove("name");
    let error = serde_json::from_value::<DatabaseDecl>(value).unwrap_err();
    assert!(error.to_string().contains("missing field `name`"));
}

#[test]
fn engines_preserve_in_memory_and_duckdb_wire_shapes() {
    assert_eq!(
        serde_json::to_value(DatabaseEngine::InMemory {
            name: "sales".into(),
        })
        .unwrap(),
        json!({"inMemory": {"name": "sales"}})
    );

    let duckdb = DatabaseEngine::DuckDb {
        path: "database/project.duckdb".into(),
        table: "sales".into(),
    };
    assert_eq!(
        duckdb.duckdb_table(),
        Some(("database/project.duckdb", "sales"))
    );
    assert_eq!(
        serde_json::to_value(duckdb).unwrap(),
        json!({"duckDb": {"path": "database/project.duckdb", "table": "sales"}})
    );
}

#[test]
fn declaration_fingerprint_is_deterministic_and_decl_specific() {
    let original = declaration("sales", "Sales");
    let same = declaration("sales", "Sales");
    let renamed = declaration("sales", "Renamed");

    assert_eq!(
        DatabaseDeclarationFingerprint::from_decl(&original),
        DatabaseDeclarationFingerprint::from_decl(&same)
    );
    assert_ne!(
        DatabaseDeclarationFingerprint::from_decl(&original),
        DatabaseDeclarationFingerprint::from_decl(&renamed)
    );
}

#[test]
fn duplicate_observations_are_rejected_before_set_creation() {
    let id = DatabaseId::from_existing("sales".into());
    let observation = DatabaseDeclarationObservation::new(
        DatabaseDeclarationRevision::from_existing(1),
        DatabaseDeclarationFingerprint::from_decl(&declaration("sales", "Sales")),
    );

    let error = DatabaseDeclarationObservationSet::try_from_iter([
        (id.clone(), observation.clone()),
        (id, observation),
    ])
    .unwrap_err();

    assert!(matches!(
        error,
        DatabaseDeclarationObservationSetError::DuplicateId(id) if id.as_str() == "sales"
    ));
}

#[test]
fn validated_parts_preserve_complete_session_facts() {
    let declarations = vec![declaration("sales", "Sales")];
    let parts = request(
        "session-1",
        declarations.clone(),
        observations_for(&declarations),
    )
    .into_validated_parts()
    .unwrap();

    assert_eq!(parts.identity.as_str(), "session-1");
    assert_eq!(parts.generation.get(), 7);
    assert_eq!(parts.root, Some(PathBuf::from("project")));
    assert_eq!(parts.declarations.as_ref(), declarations);
    assert_eq!(parts.observations.iter().count(), 1);
}

#[test]
fn open_request_rejects_a_declaration_observation_fingerprint_mismatch() {
    let declarations = vec![declaration("sales", "Sales")];
    let observations = DatabaseDeclarationObservationSet::try_from_iter([(
        DatabaseId::from_existing("sales".into()),
        DatabaseDeclarationObservation::new(
            DatabaseDeclarationRevision::from_existing(4),
            DatabaseDeclarationFingerprint::from_decl(&declaration("sales", "Other")),
        ),
    )])
    .unwrap();

    let error = request("session-1", declarations, observations)
        .validate()
        .unwrap_err();
    assert!(matches!(
        error,
        DatabaseSessionOpenRequestError::FingerprintMismatch(id) if id.as_str() == "sales"
    ));
}

#[test]
fn open_request_rejects_empty_identity_and_incomplete_observations() {
    let declarations = vec![declaration("sales", "Sales")];
    let observations = observations_for(&declarations);
    assert_eq!(
        request("", declarations.clone(), observations)
            .validate()
            .unwrap_err(),
        DatabaseSessionOpenRequestError::EmptyIdentity
    );

    let empty = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty()).unwrap();
    assert!(matches!(
        request("session-1", declarations, empty)
            .validate()
            .unwrap_err(),
        DatabaseSessionOpenRequestError::MissingObservation(id) if id.as_str() == "sales"
    ));
}
