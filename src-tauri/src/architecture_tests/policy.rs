use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    ArchitectureAuditError, ArchitectureFinding, ArchitectureFindingKey, CanonicalDependency,
    CanonicalOrigin, ProductionRoot, ProductionRootKind, RustLayer, RustModule,
};

const EXACT_SOURCE_MEMBERSHIP: &[(&str, RustLayer)] = &[
    ("src-tauri/src/lib.rs", RustLayer::CompositionRoot),
    ("src-tauri/src/main.rs", RustLayer::CompositionRoot),
    (
        "src-tauri/crates/yss-execution/src/lib.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/value.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/identity.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/error.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/canonical.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/package_preparation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/result_store.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/run_registry.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/settings.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/mod.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/identity.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/basis.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/model.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/package.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/parameter.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/observation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/validation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/plan/validation/control.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/ports/mod.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-analysis/src/lib.rs",
        RustLayer::Graph,
    ),
    (
        "src-tauri/crates/yss-graph-compiler/src/lib.rs",
        RustLayer::Graph,
    ),
    (
        "src-tauri/src/project/execution_authority.rs",
        RustLayer::Project,
    ),
    ("src-tauri/src/project/history.rs", RustLayer::Project),
    (
        "src-tauri/src/application/execution/session_factory.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/src/application/execution/mod.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/src/application/execution/session_slot.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/src/application/editor_projection/mod.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/crates/yss-execution/src/ports/scientific.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/ports/relational.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/ports/resources.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-execution/src/resource_preparation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/backend_adapters/mod.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/backend_adapters/execution/mod.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/backend_adapters/execution/scientific.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/backend_adapters/execution/relational.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/backend_adapters/execution/resources.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/crates/yss-graph-catalog/src/builtin.rs",
        RustLayer::BuiltinComposition,
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct InternalDependencyCapability {
    pub(super) source_layer: RustLayer,
    pub(super) repository_relative_source_file: &'static str,
    pub(super) fully_qualified_owner: &'static str,
    pub(super) canonical_origin_targets: &'static [&'static str],
}

const RUST_INTERNAL_CAPABILITIES: &[InternalDependencyCapability] = &[
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/database.rs",
        fully_qualified_owner: "yssbi_lib::schema::database",
        canonical_origin_targets: &["yss_database_schema::DatabaseColumnFact"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::DatabaseCore,
        repository_relative_source_file: "src-tauri/crates/yss-database-runtime/src/database_instance.rs",
        fully_qualified_owner: "yss_database_runtime::database_instance",
        canonical_origin_targets: &[
            "yss_tabular_polars::anyvalue_to_json",
            "yss_tabular_polars::edit::apply_operation",
            "yss_tabular_polars::edit::capture_column_data",
            "yss_tabular_polars::edit::capture_row_data",
            "yss_tabular_polars::edit::cast_column",
            "yss_tabular_polars::edit::dtype_from_string",
            "yss_tabular_polars::edit::dtype_to_string",
            "yss_tabular_polars::edit::reverse_operation",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::DatabaseCore,
        repository_relative_source_file: "src-tauri/crates/yss-database-runtime/src/plot_query.rs",
        fully_qualified_owner: "yss_database_runtime::plot_query",
        canonical_origin_targets: &["yss_tabular_polars::column_to_series"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/backend_adapters/execution/scientific.rs",
        fully_qualified_owner: "yssbi_lib::backend_adapters::execution::scientific",
        canonical_origin_targets: &[
            "yss_execution::ports::scientific::AcfPacfRequest",
            "yss_execution::ports::scientific::AcfPacfResult",
            "yss_execution::ports::scientific::BackendExecutionControl",
            "yss_execution::ports::scientific::ExecutionInstrumentalVariableKind",
            "yss_execution::ports::scientific::ExecutionRegressionKind",
            "yss_execution::ports::scientific::ExecutionStatisticalTrend",
            "yss_execution::ports::scientific::KdePoint",
            "yss_execution::ports::scientific::KernelDensityRequest",
            "yss_execution::ports::scientific::KernelDensityResult",
            "yss_execution::ports::scientific::ScientificBackend",
            "yss_execution::ports::scientific::ScientificBackendError",
            "yss_execution::ports::scientific::ScientificInputViolation",
            "yss_execution::ports::scientific::StatisticsOperation",
            "yss_execution::ports::scientific::StatisticsParameters",
            "yss_execution::ports::scientific::StatisticsRequest",
            "yss_execution::ports::scientific::StatisticsResult",
            "yss_execution::settings::ExecutionMissingValuePolicy",
            "yss_execution::settings::ExecutionSettings",
            "yss_sci_runtime::api::density::KernelDensityInput",
            "yss_sci_runtime::api::density::compute_kernel_density",
            "yss_sci_runtime::api::node_statistics::InstrumentalVariableKind",
            "yss_sci_runtime::api::node_statistics::RegressionKind",
            "yss_sci_runtime::api::node_statistics::augmented_dickey_fuller",
            "yss_sci_runtime::api::node_statistics::fit_instrumental_variables",
            "yss_sci_runtime::api::node_statistics::fit_panel",
            "yss_sci_runtime::api::node_statistics::fit_regression",
            "yss_sci_runtime::api::node_statistics::var_fit",
            "yss_sci_runtime::api::node_statistics::var_lag_order",
            "yss_sci_runtime::api::node_statistics::vec_fit",
            "yss_sci_runtime::api::node_statistics::vec_rank_test",
            "yss_sci_runtime::api::time_series::acf_pacf::AcfPacfInput",
            "yss_sci_runtime::api::time_series::acf_pacf::compute_acf_pacf",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/backend_adapters/execution/bayes_artifacts.rs",
        fully_qualified_owner: "yssbi_lib::backend_adapters::execution::bayes_artifacts",
        canonical_origin_targets: &[
            "yssbi_lib::application::bayes::BayesArtifactReadError",
            "yssbi_lib::application::bayes::BayesArtifactReader",
            "yss_sci_runtime::api::density::KernelDensityInput",
            "yss_sci_runtime::api::density::compute_kernel_density",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/backend_adapters/execution/relational.rs",
        fully_qualified_owner: "yssbi_lib::backend_adapters::execution::relational",
        canonical_origin_targets: &[
            "yss_execution::ports::relational::RelationalBackend",
            "yss_execution::ports::relational::RelationalError",
            "yss_execution::ports::relational::RelationalExecutionControl",
            "yss_execution::ports::relational::RelationalRequest",
            "yss_execution::ports::relational::RelationalResult",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/backend_adapters/execution/resources.rs",
        fully_qualified_owner: "yssbi_lib::backend_adapters::execution::resources",
        canonical_origin_targets: &[
            "yss_execution::plan::identity::PlanProjectSessionId",
            "yss_execution::resource_preparation::ResourceProviderFactory",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::PureLeaf,
        repository_relative_source_file: "src-tauri/crates/yss-graph-document/src/model.rs",
        fully_qualified_owner: "yss_graph_document::model",
        canonical_origin_targets: &[
            "yss_graph_protocol::identity::NodeTypeId",
            "yss_graph_protocol::identity::ParameterKey",
            "yss_graph_protocol::identity::PortKey",
            "yss_graph_protocol::types::TypeExpr",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/src/lib.rs",
        fully_qualified_owner: "yssbi_lib",
        canonical_origin_targets: &[
            "yssbi_lib::application::bayes::BayesInferenceService::with_worker",
            "yss_bayes_worker_julia::JuliaBayesWorkerAdapter::new",
            "yssbi_lib::backend_adapters::execution::scientific::SciApiScientificBackend::new",
            "yss_julia_worker::JuliaWorkerManager::new",
            "yss_project_registry_sqlite::SqliteProjectRegistryStore::connect",
            "yss_project_registry::ProjectRegistry::new",
            "yssbi_lib::project::project_state::state::ProjectState",
            "yss_project_watcher::ProjectWatcherState::new",
            "yssbi_lib::application::execution::session_factory::ProjectSessionCandidateError",
            "yssbi_lib::application::execution::session_factory::SessionResourceFactoryBuilder::from_composition",
            "yssbi_lib::application::execution::session_factory::build_current_project_candidate",
            "yssbi_lib::application::execution::session_slot::ApplicationSessionEpoch::INITIAL",
            "yssbi_lib::application::execution::session_slot::ApplicationSessionSlot::new",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::ApplicationState::from_composition",
            "yssbi_lib::backend_adapters::execution::bayes_artifacts::PolarsBayesArtifactReader",
            "yssbi_lib::backend_adapters::execution::resources::database_resource_provider_factory",
            "yss_execution::ports::scientific::ScientificBackend",
            "yssbi_lib::project::project_state::state::ProjectState::new",
            "yss_project_watcher_notify::NotifyProjectFileWatcher::new",
            "yss_window_state::WindowStateStore::load",
            "yss_window_state::apply_main_window_state",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Application,
        repository_relative_source_file: "src-tauri/src/application/bayes.rs",
        fully_qualified_owner: "yssbi_lib::application::bayes",
        canonical_origin_targets: &["yssbi_lib::error::new_diagnostic_incident_id"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Application,
        repository_relative_source_file: "src-tauri/src/application/execution/session_factory.rs",
        fully_qualified_owner: "yssbi_lib::application::execution::session_factory",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::build_builtin_node_system",
            "yss_graph_catalog::builtin::BuiltinInitializationError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Project,
        repository_relative_source_file: "src-tauri/src/project/project_io.rs",
        fully_qualified_owner: "yssbi_lib::project::project_io",
        canonical_origin_targets: &[
            "yss_duckdb::table::list_data_tables",
            "yss_duckdb::table::read_display_name",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_bayes.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_bayes",
        canonical_origin_targets: &[
            "yssbi_lib::application::bayes::BayesApplicationError",
            "yssbi_lib::application::bayes::BayesInferenceService",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_dataframe/error.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_dataframe::error",
        canonical_origin_targets: &[
            "yssbi_lib::application::database::error::DatabaseApplicationError",
            "yssbi_lib::application::database::error::DatabaseApplicationOperation",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_dataframe/mod.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_dataframe",
        canonical_origin_targets: &[
            "yssbi_lib::application::database::DatabaseMutation",
            "yssbi_lib::application::database::LoadDatabaseResult",
            "yssbi_lib::application::database::error::DatabaseApplicationError",
            "yssbi_lib::application::database::error::DatabaseApplicationOperation::Delete",
            "yssbi_lib::application::database::error::DatabaseApplicationOperation::Load",
            "yssbi_lib::application::database::error::DatabaseApplicationOperation::Rename",
            "yssbi_lib::application::database::error::DatabaseApplicationOperation::Save",
            "yssbi_lib::application::database::list_excel_sheets",
            "yssbi_lib::application::database::list_sql_tables",
            "yssbi_lib::application::database::list_sqlite_tables",
            "yssbi_lib::application::database::load_database",
            "yssbi_lib::application::database::mutate_database_resource",
            "yssbi_lib::application::database::read_column_distributions",
            "yssbi_lib::application::database::read_column_statistics",
            "yssbi_lib::application::database::read_database_edit_state",
            "yssbi_lib::application::database::read_database_meta",
            "yssbi_lib::application::database::read_database_rows",
            "yssbi_lib::application::database::read_dataset_overview",
            "yssbi_lib::application::database::rename_database",
            "yssbi_lib::application::database::save_database_changes",
            "yssbi_lib::application::database",
            "yssbi_lib::application::database::ApplicationDatabaseError",
            "yssbi_lib::application::database::DatabaseMetaResult",
            "yssbi_lib::application::database::DatabaseMutationResult",
            "yssbi_lib::application::database::DatabaseRowsResult",
            "yssbi_lib::application::database::database",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::schema::application_event::ResourceMutationCommandResultDto",
            "yssbi_lib::schema::application_event::resource_mutation_to_transport",
            "yssbi_lib::schema::database::DatabaseEngineDTO",
            "yssbi_lib::schema::database::DatabaseImportSourceDTO",
            "yssbi_lib::schema::database::DatabaseMetaResultDto",
            "yssbi_lib::schema::database::DatabaseRowsResultDto",
            "yssbi_lib::schema::database::LoadDatabaseResultDto",
            "yssbi_lib::schema::database::column_info_from_schema",
            "yss_database_edit::EditState",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_diagnostics/mod.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_diagnostics",
        canonical_origin_targets: &[
            "yss_diagnostics::dto::DiagnosticBatchDto",
            "yss_diagnostics::dto::DiagnosticSubscriptionDto",
            "yss_diagnostics::dto::FrontendDiagnosticEntryDto",
            "yss_diagnostics::runtime::DiagnosticsRuntime",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_hypothesis.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_hypothesis",
        canonical_origin_targets: &[
            "yssbi_lib::application::hypothesis::HypothesisTestInput",
            "yssbi_lib::application::hypothesis::HypothesisTestOutput",
            "yssbi_lib::application::hypothesis::run_hypothesis_test",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_julia.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_julia",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yss_julia_runtime::JuliaRuntimeStatus",
            "yss_julia_runtime::get_runtime_status",
            "yss_julia_runtime::install_latest_julia",
            "yss_julia_worker::JuliaWorkerManager",
            "yss_julia_worker::JuliaWorkerStatus",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/catalog.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::catalog",
        canonical_origin_targets: &[
            "yssbi_lib::application::catalog_query::CatalogQueryApplicationError",
            "yssbi_lib::application::catalog_query::CompatibleCatalogRequest",
            "yssbi_lib::application::catalog_query::GraphCatalogQueryError",
            "yssbi_lib::application::catalog_query::GraphCatalogQueryError::Catalog",
            "yssbi_lib::application::catalog_query::GraphCatalogQueryError::CompatibleSourceInvalid",
            "yssbi_lib::application::catalog_query::GraphCatalogQueryError::GraphNotLoaded",
            "yssbi_lib::application::catalog_query::GraphCatalogQueryError::Internal",
            "yssbi_lib::application::catalog_query::GraphCatalogQueryError::RevisionConflict",
            "yssbi_lib::application::catalog_query::LocalizedCatalogRequest",
            "yssbi_lib::application::catalog_query::ProjectCatalogReadError",
            "yssbi_lib::application::execution::ApplicationState",
            "yssbi_lib::application::execution::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionRevalidationError",
            "yssbi_lib::application::execution::session_slot::SessionRevalidationError::Changed",
            "yssbi_lib::application::execution::session_slot::SessionRevalidationError::Unavailable",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::schema::catalog::LocalizedCatalogDto",
            "yssbi_lib::schema::graph_mutation::PortAddressDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/common.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::common",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::error::GraphMutationErrorDetailsDto",
            "yssbi_lib::error::GraphMutationErrorDetailsDto::VALUE",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::schema::application_event::ResourceMutationResultDto",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::GraphOperation",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::History",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::Mutation",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::Project",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::SessionCapture",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yss_graph_document_edit::DocumentError::ConnectionNotFound",
            "yss_graph_document_edit::error::DocumentError::ConnectionNotFound",
            "yss_graph_editor::MutationConflict",
            "yss_graph_editor::mutation::MutationConflict",
            "yss_graph_editor::mutation::MutationConflict::CatalogDescriptorInvalid",
            "yss_graph_editor::mutation::MutationConflict::CatalogResourceStale",
            "yss_graph_editor::mutation::MutationConflict::ClipboardSubgraphInvalid",
            "yss_graph_editor::mutation::MutationConflict::ReferencedResourceUnavailable",
            "yss_graph_editor::mutation::MutationConflict::Document",
            "yss_graph_editor::mutation::MutationConflict::Editor",
            "yss_project_history::ProjectHistoryMutationError",
            "yss_project_history::ProjectHistoryMutationError::History",
            "yss_project_history::ProjectHistoryMutationError::Projection",
            "yss_project_history::ProjectHistoryMutationError::RecoveryRequired",
            "yss_project_history::ProjectHistoryMutationError::ResourceMismatch",
            "yss_project_history::ProjectHistoryMutationError::StaleProjectLifecycle",
            "yss_project_history::ProjectHistoryMutationError::StaleRevision",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::GraphUnavailable",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::ProjectIdentityMismatch",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::RevisionConflict",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::OperationOwnershipChanged",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphCommitError",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphCommitError::LifecycleChanged",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphCommitError::OperationOwnershipChanged",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphCommitError::RevisionExhausted",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphCommitError::StaleAuthority",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::AdmissionClosed",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::Internal",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::RecoveryRequired",
            "yssbi_lib::project::project_state::graph_operation::ProjectGraphOperationError::ResourceLifecycleChanged",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/editor.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::editor",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::Contract",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::Database",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::Materialization",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::Project",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::Projection",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::SessionCapture",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError::SessionChanged",
            "yssbi_lib::application::graph_open::OpenGraphRequest",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::GraphOperation",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::History",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::Mutation",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::Project",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::SessionCapture",
            "yssbi_lib::schema::application_event::graph_delta_to_transport",
            "yssbi_lib::schema::application_event::graph_mutation_to_transport",
            "yss_graph_editor::EditorGraphMutation",
            "yss_graph_editor::mutation::EditorGraphMutation",
            "yss_graph_document::NodeId",
            "yss_project_history::MutationRequest",
            "yssbi_lib::schema::application_event::GraphMutationResultDto",
            "yssbi_lib::schema::catalog::NodeCreationDescriptorDto",
            "yssbi_lib::schema::editor_projection::map_editor_projection",
            "yssbi_lib::schema::editor_projection_types::EditorGraphProjectionDto",
            "yssbi_lib::schema::graph_clipboard::ClipboardSubgraphDto",
            "yssbi_lib::schema::graph_mutation::EditorGraphMutationDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/execution.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::execution",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::application::execution::session_slot::SessionRevalidationError",
            "yssbi_lib::application::execution::session_slot::SessionRevalidationError::Changed",
            "yssbi_lib::application::execution::session_slot::SessionRevalidationError::Unavailable",
            "yssbi_lib::application::execution::run_graph::CancelRunOutcome",
            "yssbi_lib::application::execution::run_graph::ExecutionApplicationError",
            "yssbi_lib::application::execution::run_graph::RunApplicationEvent",
            "yssbi_lib::application::execution::run_graph::RunGraphRequest",
            "yssbi_lib::application::execution::run_graph::run_graph_with_sink",
            "yssbi_lib::application::execution::run_graph::cancel_run",
            "yssbi_lib::application::pin_preview_generation::allocate_pin_preview_generation",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::DuplicateResourceRequirement",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::GraphRevisionUnavailable",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::GraphUnavailable",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::InvalidGraph",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::InvalidResourceIdentity",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::ProjectIdentityMismatch",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::ResourceKindMismatch",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::ResourceRevisionUnavailable",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::ResourceUnavailable",
            "yssbi_lib::project::execution_authority::ProjectExecutionPreparationError::UnsupportedResourceKind",
            "yss_execution::run_registry::RunId",
            "yss_execution::run_registry::RunId::from_existing",
            "yss_execution::state::ExecutePreparedError",
            "yss_execution::state::ExecutePreparedError::Admission",
            "yss_execution::state::ExecutePreparedError::Cancelled",
            "yss_execution::state::ExecutePreparedError::DeadlineExceeded",
            "yss_execution::state::ExecutePreparedError::Kernel",
            "yss_execution::state::ExecutePreparedError::KernelUnavailable",
            "yss_execution::state::ExecutePreparedError::ResourcePreparation",
            "yss_execution::state::ExecutePreparedError::RunRegistry",
            "yss_execution::state::ExecutePreparedError::RuntimeGenerationMismatch",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/history.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::history",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::History",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject::ResourceMutationCommitted",
            "yssbi_lib::schema::application_event::resource_mutation_to_transport",
            "yssbi_lib::schema::application_event::ResourceMutationResultDto",
            "yss_project_history::HistoryMutation",
            "yss_project_history::HistoryStatusDto",
            "yss_project_history::MutationRequest",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/resources.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::resources",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::GraphOperation",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::History",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::Mutation",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::Project",
            "yssbi_lib::application::resource_mutation::ResourceMutationApplicationError::SessionCapture",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yss_project_history::MutationRequest",
            "yssbi_lib::project::project_writers::ProjectSaveResult",
            "yssbi_lib::schema::application_event::ResourceMutationResultDto",
            "yssbi_lib::schema::application_event::resource_mutation_to_transport",
            "yssbi_lib::schema::project::ProjectSaveResultDto",
            "yss_project_history::FunctionDocumentPatch",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/results.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::results",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::result_query::ResultPinQuery",
            "yssbi_lib::application::execution::result_query::ResultQueryApplicationError",
            "yssbi_lib::application::execution::result_query::ResultQueryApplicationError::Execution",
            "yssbi_lib::application::execution::result_query::ResultQueryApplicationError::SessionCapture",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::commands::execution_dto::PinResultEntryDto",
            "yssbi_lib::commands::execution_dto::ResultDescriptorDto",
            "yssbi_lib::commands::execution_dto::ResultPageDto",
            "yssbi_lib::commands::execution_dto::ResultValueDto",
            "yssbi_lib::commands::execution_dto::ResultValueKindDto",
            "yssbi_lib::error::CommandError",
            "yss_execution::result::ExecutionResultQueryError::ResultSourceReadFailed",
            "yss_execution::result::ResultId",
            "yss_execution::result::StoredResult",
            "yss_execution::value::RuntimeValue",
            "yssbi_lib::schema::graph_mutation::PortAddressDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/execution_dto.rs",
        fully_qualified_owner: "yssbi_lib::commands::execution_dto",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::run_graph::RunApplicationEvent",
            "yssbi_lib::application::execution::run_graph::RunApplicationEventKind",
            "yssbi_lib::application::execution::run_graph::RunDemand",
            "yss_execution::plan::identity::PlanGraphId",
            "yss_execution::plan::identity::PlanOutputRef",
            "yss_execution::plan::identity::PlanPortAddress",
            "yss_execution::result::PinResultEntry",
            "yss_execution::result::ResultId",
            "yss_execution::result::ResultUsage",
            "yss_execution::result::StoredResult",
            "yss_execution::value::RuntimeValue",
            "yssbi_lib::schema::graph_mutation::PortAddressDto",
            "yss_execution::plan::result_category::PlotDataKind::Scatter",
            "yss_execution::plan::result_category::PlotDataKind::Line",
            "yss_execution::plan::result_category::PlotDataKind::Plot",
            "yss_execution::plan::result_category::PlotDataKind::Ecdf",
            "yss_execution::plan::result_category::PlotDataKind::Kde",
            "yss_execution::plan::result_category::PlotDataKind::Histogram",
            "yss_execution::plan::result_category::PlotDataKind::Correlation",
            "yss_execution::plan::result_category::PlotDataKind::Correlogram",
            "yss_execution::plan::result_category::ResultCategory",
            "yss_execution::plan::result_category::ResultCategory::Value",
            "yss_execution::plan::result_category::ResultCategory::PlotData",
            "yss_execution::plan::result_category::ResultCategory::StatisticalReport",
            "yss_execution::plan::result_category::StatisticalReportKind::OlsSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::BinarySummary",
            "yss_execution::plan::result_category::StatisticalReportKind::Iv2slsSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::IvLimlSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::PraisSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::VarSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::VarSoc",
            "yss_execution::plan::result_category::StatisticalReportKind::PanelSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::PanelDid",
            "yss_execution::plan::result_category::StatisticalReportKind::DfAdfSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::DfAdfSummaryList",
            "yss_execution::plan::result_category::StatisticalReportKind::VecSummary",
            "yss_execution::plan::result_category::StatisticalReportKind::VecRankSummary",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_panel_did.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_panel_did",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yss_sci_runtime::models::panel_did::ComputeDidFakeGroupRequest",
            "yss_sci_runtime::models::panel_did::DidPlaceboFakeGroupBlock",
            "yss_sci_runtime::models::panel_did::compute_fake_group_ri",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_parse_at.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_parse_at",
        canonical_origin_targets: &[
            "yssbi_lib::application::hypothesis::parse_at_values",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/project_failure.rs",
        fully_qualified_owner: "yssbi_lib::commands::project_failure",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_failure::ApplicationProjectFailure",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/lifecycle.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::lifecycle",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::project_lifecycle::ProjectLifecycleError",
            "yssbi_lib::application::project_lifecycle::clear_project",
            "yssbi_lib::application::project_lifecycle::create_project",
            "yssbi_lib::application::project_lifecycle::load_project",
            "yssbi_lib::application::project_lifecycle::save_project_as",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::schema::application_event::LifecycleMutationOutcomeDto",
            "yssbi_lib::schema::application_event::LifecycleMutationResultDto",
            "yssbi_lib::schema::application_event::ProjectActivationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/lifecycle.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::lifecycle",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_change::ApplicationProjectWatchError",
            "yssbi_lib::application::project_change::ApplicationProjectWatchError::ProjectIdentityMismatch",
            "yssbi_lib::application::project_change::ApplicationProjectWatchError::Reconciliation",
            "yssbi_lib::application::project_change::ApplicationProjectWatchError::SessionCapture",
            "yssbi_lib::application::project_change::ApplicationProjectWatchError::SessionChanged",
            "yss_project_watcher::ProjectWatcherError",
            "yss_project_watcher::ProjectWatcherState",
            "yss_project_watcher::ObservedProjectChange",
            "yss_project_watcher::ProjectChangeSink",
            "yssbi_lib::event::event_resource::EventResource",
            "yss_project_registry::ProjectRegistry",
            "yssbi_lib::schema::project::ProjectSaveResultDto",
            "yssbi_lib::application::project_lifecycle::ApplicationProjectLifecycleError",
            "yssbi_lib::application::project_lifecycle::ApplicationProjectLifecycleError::Lifecycle",
            "yssbi_lib::application::project_lifecycle::ApplicationProjectLifecycleError::SessionCapture",
            "yssbi_lib::application::project_lifecycle::ApplicationProjectLifecycleError::SessionChanged",
            "yssbi_lib::application::project_lifecycle::ApplicationProjectLifecycleError::SessionRefresh",
            "yssbi_lib::application::events::ProjectLifecycleOutcome::Committed",
            "yssbi_lib::schema::application_event::project_activation_to_transport",
            "yssbi_lib::schema::application_event::project_lifecycle_to_transport",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::PlatformAdapter,
        repository_relative_source_file: "src-tauri/crates/yss-project-watcher-notify/src/lib.rs",
        fully_qualified_owner: "yss_project_watcher_notify",
        canonical_origin_targets: &[
            "yss_project_watcher::FileWatcherStartError",
            "yss_project_watcher::ObservedProjectChange",
            "yss_project_watcher::ProjectChangeSink",
            "yss_project_watcher::ProjectFileWatcherDrain",
            "yss_project_watcher::ProjectFileWatcherDrainOutcome",
            "yss_project_watcher::ProjectFileWatcherFactory",
            "yss_project_watcher::ProjectFileWatcherSession",
            "yss_project_watcher::ProjectWatcherEpoch",
            "yss_project_watcher::WatcherShutdownControl",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/path.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::path",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yss_project_registry::default_project_parent_directory",
            "yss_project_registry::validate_new_project_path",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/query.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::query",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::graph_open::OpenGraphApplicationError",
            "yssbi_lib::application::graph_open::OpenGraphRequest",
            "yssbi_lib::application::project_query::ProjectDatabasesVariablesSnapshot",
            "yssbi_lib::application::project_query::ProjectQueryApplicationError",
            "yssbi_lib::application::project_query::ProjectActivation",
            "yssbi_lib::application::project_query::query_project_databases_variables",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::project::ProjectIndex",
            "yssbi_lib::project::project_io::ProjectIndex",
            "yss_project_registry::normalize_existing_path",
            "yssbi_lib::project::project_state::state::ProjectState",
            "yssbi_lib::project::resource_reveal::RevealProjectResourceRequest",
            "yssbi_lib::project::resource_reveal::resolve_reveal_path",
            "yssbi_lib::schema::application_event::ProjectActivationResultDto",
            "yssbi_lib::schema::database::DatabaseDeclDTO",
            "yssbi_lib::schema::database::column_info_from_schema",
            "yssbi_lib::schema::editor_projection_types::EditorGraphProjectionDto",
            "yssbi_lib::schema::editor_projection::map_editor_projection",
            "yssbi_lib::schema::project::DatabasesVariablesDTO",
            "yssbi_lib::schema::variables::VariableInstanceDTO",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/registry.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::registry",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_lifecycle::delete_registered_project",
            "yssbi_lib::error::CommandError",
            "yss_project_registry::CleanupInvalidProjectsResult",
            "yss_project_registry::ProjectRegistry",
            "yss_project_registry::ScanProjectsResult",
            "yssbi_lib::project::project_state::state::ProjectState",
            "yssbi_lib::schema::application_event::LifecycleMutationResultDto",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::schema::application_event::project_lifecycle_to_transport",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/settings.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::settings",
        canonical_origin_targets: &[
            "yssbi_lib::application::computation_settings::ComputationSettingsApplicationError",
            "yssbi_lib::application::computation_settings::ComputationSettingsApplicationError::SessionCapture",
            "yssbi_lib::application::computation_settings::ComputationSettingsApplicationError::SessionChanged",
            "yssbi_lib::application::computation_settings::ComputationSettingsApplicationError::SessionRefresh",
            "yssbi_lib::application::computation_settings::ComputationSettingsApplicationError::Project",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yss_computation_settings::ComputationSettingsMutationReceipt",
            "yss_computation_settings::ComputationSettingsMutationRequest",
            "yss_computation_settings::ComputationSettingsSnapshot",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/progress.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::progress",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yss_project_progress::ProjectCleanupProgress",
            "yss_project_progress::ProjectProgress",
            "yss_project_progress::ProjectProgressSink",
            "yss_project_progress::ProjectScanProgress",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_sci.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_sci",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::ApplicationState",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::statistics::AcfPacfApplicationError",
            "yssbi_lib::application::statistics::compute_acf_pacf",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yss_execution::ports::scientific::AcfPacfResult",
            "yss_execution::ports::scientific::ScientificBackendError",
            "yss_execution::ports::scientific::ScientificBackendError::Cancelled",
            "yss_execution::ports::scientific::ScientificBackendError::ComputationFailed",
            "yss_execution::ports::scientific::ScientificBackendError::DeadlineExceeded",
            "yss_execution::ports::scientific::ScientificBackendError::InvalidInput",
            "yss_execution::ports::scientific::ScientificBackendError::Unavailable",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::schema::statistics::AcfPacfRequestDto",
            "yssbi_lib::schema::statistics::AcfPacfResponseDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_serial_tests.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_serial_tests",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::statistics::SerialTestsApplicationError",
            "yssbi_lib::application::statistics::SerialTestsRequest",
            "yssbi_lib::application::statistics::compute_serial_tests",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::schema::statistics::DurbinWatsonResultDto",
            "yssbi_lib::schema::statistics::SerialTestWithLagDto",
            "yssbi_lib::schema::statistics::SerialTestsRequestDto",
            "yssbi_lib::schema::statistics::SerialTestsResponseDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_variable/mod.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_variable",
        canonical_origin_targets: &[
            "yssbi_lib::application::events::ApplicationEvent",
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError::InvalidDataType",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError::Project",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError::ProjectIdentityMismatch",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError::SessionCapture",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError::SessionChanged",
            "yssbi_lib::application::variable_mutation::VariableMutationApplicationError::VariableNotFound",
            "yssbi_lib::application::variable_mutation::VariableMutationRequest::Create",
            "yssbi_lib::application::variable_mutation::VariableMutationRequest::Delete",
            "yssbi_lib::application::variable_mutation::VariableMutationRequest::Update",
            "yssbi_lib::application::variable_mutation::VariableQueryApplicationError",
            "yssbi_lib::application::variable_mutation::VariableQueryApplicationError::Project",
            "yssbi_lib::application::variable_mutation::VariableQueryApplicationError::ProjectIdentityMismatch",
            "yssbi_lib::application::variable_mutation::VariableQueryApplicationError::SessionCapture",
            "yssbi_lib::application::variable_mutation::VariableQueryApplicationError::SessionChanged",
            "yssbi_lib::application::variable_mutation::VariableQueryApplicationError::VariableNotFound",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::schema::application_event::application_event_to_transport",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::project::project_state::state::ProjectState",
            "yssbi_lib::schema::application_event::ResourceMutationResultDto",
            "yssbi_lib::schema::variables::VariableInstanceDTO",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_window.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_window",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yss_window_state::WindowStateStore",
            "yss_window_state::kind::WindowKind",
            "yss_window_state::kind::WindowState",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_worksheet.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_worksheet",
        canonical_origin_targets: &[
            "yssbi_lib::application::execution::session_slot::ApplicationState",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Inactive",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Recovering",
            "yssbi_lib::application::execution::session_slot::SessionCaptureError::Replacing",
            "yssbi_lib::application::worksheet::WorksheetApplicationError",
            "yssbi_lib::application::worksheet_plot::WorksheetPlotApplicationError",
            "yssbi_lib::application::worksheet_plot::WorksheetPlotQuery",
            "yssbi_lib::application::worksheet_plot::PlotAxisFormat",
            "yssbi_lib::application::worksheet_plot::PlotAxisFormat::Date",
            "yssbi_lib::application::worksheet_plot::PlotAxisFormat::Datetime",
            "yssbi_lib::application::worksheet_plot::PlotAxisFormat::Number",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::AdmissionClosed",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::ColumnMaterializationFailed",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::DatabaseNotFound",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::GenerationMismatch",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::RuntimeRevisionMismatch",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::SchemaRevisionMismatch",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::SessionMismatch",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::project::project_state::state::ProjectState",
            "yss_resource_naming::ResourceName",
            "yssbi_lib::schema::application_event::ResourceMutationResultDto",
            "yssbi_lib::schema::application_event::resource_mutation_to_transport",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BuiltinComposition,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/builtin.rs",
        fully_qualified_owner: "yss_graph_catalog::builtin",
        canonical_origin_targets: &[
            "yss_graph_catalog::control",
            "yss_graph_catalog::core_nodes",
            "yss_graph_catalog::dataframe",
            "yss_graph_catalog::distribution",
            "yss_graph_catalog::localization::Aliases",
            "yss_graph_catalog::localization::BuiltinCatalog",
            "yss_graph_catalog::localization::I18nBundleValidationError",
            "yss_graph_catalog::localization::Message",
            "yss_graph_catalog::localization::Text",
            "yss_graph_catalog::plot",
            "yss_graph_catalog::project",
            "yss_graph_catalog::statistics",
            "yss_graph_compiler_diagnostics::COMPILER_DIAGNOSTIC_DEFINITIONS",
            "yss_graph_compiler_diagnostics::CompilerDiagnosticDefinitionError",
            "yss_graph_compiler_diagnostics::validate_compiler_diagnostic_definitions",
            "yss_graph_registry::LeafImplementation",
            "yss_graph_registry::NodeRegistrationError",
            "yss_graph_registry::NodeRegistry",
            "yss_graph_registry::NodeRegistryBuilder",
            "yss_graph_registry::ProviderRegistration",
            "yss_graph_registry::RegisteredNode",
            "yss_graph_registry::TypeConstructorRegistration",
            "yss_graph_registry::TypeRegistration",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/error/mod.rs",
        fully_qualified_owner: "yssbi_lib::error",
        canonical_origin_targets: &[
            "yssbi_lib::project::project_error::ProjectError",
            "yssbi_lib::project::database_authority::ProjectDatabaseError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/event/event_project.rs",
        fully_qualified_owner: "yssbi_lib::event::event_project",
        canonical_origin_targets: &[
            "yss_graph_document_edit::GraphDocumentPatch",
            "yss_graph_document_edit::patch::GraphDocumentPatch",
            "yss_computation_settings::ComputationSettingsMutationReceipt",
            "yssbi_lib::schema::project::ProjectSaveResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/application_event.rs",
        fully_qualified_owner: "yssbi_lib::schema::application_event",
        canonical_origin_targets: &[
            "yssbi_lib::application::events::ApplicationEvent",
            "yssbi_lib::application::events::CommittedResourceMutation",
            "yssbi_lib::application::events::GraphDeltaEvent",
            "yssbi_lib::application::events::GraphMutationResult",
            "yssbi_lib::application::events::GraphProjectionReplacement",
            "yssbi_lib::application::events::HistoryStatus",
            "yssbi_lib::application::events::LifecycleInvalidation",
            "yssbi_lib::application::events::LifecycleRecovery",
            "yssbi_lib::application::events::LifecycleRecoveryAction",
            "yssbi_lib::application::events::ProjectLifecycleApplicationEvent",
            "yssbi_lib::application::events::ProjectLifecycleKind",
            "yssbi_lib::application::events::ProjectLifecycleOutcome",
            "yssbi_lib::application::events::ProjectLifecyclePhase",
            "yssbi_lib::application::events::ResourceMove",
            "yssbi_lib::application::events::ResourceProjectionStatus",
            "yssbi_lib::application::project_query::ProjectActivation",
            "yss_graph_document_edit::GraphDocumentPatch",
            "yss_graph_document_edit::patch::GraphDocumentPatch",
            "yss_project_history::HistoryStatusDto",
            "yss_project_history::ResourceDeltaEvent",
            "yss_project_history::ResourceKey",
            "yss_project_history::ResourceLifecycleKind",
            "yss_function_editor_projection::FunctionEditorProjection",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/catalog.rs",
        fully_qualified_owner: "yssbi_lib::schema::catalog",
        canonical_origin_targets: &[
            "yssbi_lib::application::catalog_query::CatalogQueryResult",
            "yss_graph_catalog::localization::CatalogResourcePath",
            "yss_graph_catalog::localization::CatalogResourcePath::new",
            "yss_graph_catalog::localization::LocalizedCatalog",
            "yss_graph_catalog::localization::LocalizedCatalogItem",
            "yss_graph_catalog::localization::LocalizedCategory",
            "yss_graph_catalog::localization::LocalizedParameter",
            "yss_graph_catalog::localization::LocalizedPort",
            "yss_graph_catalog::localization::NodeCreation",
            "yss_graph_catalog::localization::NodeCreation::ParameterizedStatic",
            "yss_graph_catalog::localization::NodeCreation::ResourceBound",
            "yss_graph_catalog::localization::NodeCreation::Static",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs::Database",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs::Function",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs::Variable",
            "yss_graph_protocol::identity::NodeTypeId",
            "yss_graph_protocol::identity::NodeTypeId::new",
            "yss_graph_protocol::identity::ParameterKey",
            "yss_graph_protocol::identity::ParameterKey::new",
            "yss_graph_protocol::parameter::ParameterKey",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/editor_projection.rs",
        fully_qualified_owner: "yssbi_lib::schema::editor_projection",
        canonical_origin_targets: &[
            "yssbi_lib::application::editor_projection::EditorConnectionModel",
            "yssbi_lib::application::editor_projection::EditorDiagnosticModel",
            "yssbi_lib::application::editor_projection::EditorInputModel",
            "yssbi_lib::application::editor_projection::EditorNodeCapabilities",
            "yssbi_lib::application::editor_projection::EditorNodeDisplay",
            "yssbi_lib::application::editor_projection::EditorNodeModel",
            "yssbi_lib::application::editor_projection::EditorParameterModel",
            "yssbi_lib::application::editor_projection::EditorPortConnectionCapabilities",
            "yssbi_lib::application::editor_projection::EditorPortDisplay",
            "yssbi_lib::application::editor_projection::EditorPortModel",
            "yssbi_lib::application::editor_projection::EditorProjectionModel",
            "yssbi_lib::application::editor_projection::EditorProjectionOutcome",
            "yssbi_lib::application::editor_projection::EditorProjectionStage",
            "yssbi_lib::application::editor_projection::ParameterEditorKind",
            "yssbi_lib::application::editor_projection::ParameterValueSource",
            "yssbi_lib::application::editor_projection::ResolvedSchemaModel",
            "yssbi_lib::application::editor_projection::ResolvedTypeModel",
            "yssbi_lib::application::editor_projection::model::EditorConnectionModel",
            "yssbi_lib::application::editor_projection::model::EditorDiagnosticModel",
            "yssbi_lib::application::editor_projection::model::EditorDiagnosticSeverity",
            "yssbi_lib::application::editor_projection::model::EditorEffectiveInputBinding",
            "yssbi_lib::application::editor_projection::model::EditorInputModel",
            "yssbi_lib::application::editor_projection::model::EditorNodeCapabilities",
            "yssbi_lib::application::editor_projection::model::EditorNodeDisplay",
            "yssbi_lib::application::editor_projection::model::EditorNodeModel",
            "yssbi_lib::application::editor_projection::model::EditorParameterModel",
            "yssbi_lib::application::editor_projection::model::EditorPortConnectionCapabilities",
            "yssbi_lib::application::editor_projection::model::EditorPortDisplay",
            "yssbi_lib::application::editor_projection::model::EditorPortInstanceKind",
            "yssbi_lib::application::editor_projection::model::EditorPortModel",
            "yssbi_lib::application::editor_projection::model::EditorPortStatus",
            "yssbi_lib::application::editor_projection::model::EditorProjectionModel",
            "yssbi_lib::application::editor_projection::model::EditorCompilationOutcome",
            "yssbi_lib::application::editor_projection::model::EditorCompilationStage",
            "yssbi_lib::application::editor_projection::model::ParameterEditorKind",
            "yss_graph_analysis::GraphDiagnosticLocation",
            "yss_graph_analysis_contract::DiagnosticLocation",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Connection",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Graph",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Node",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Parameter",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Port",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Resource",
            "yss_graph_registry::fingerprint::RegistryFingerprint",
            "yss_graph_protocol::PortDirection",
            "yss_graph_protocol::PortKey",
            "yss_graph_protocol::PortKind",
            "yss_graph_protocol::TypeExpr",
            "yss_graph_protocol::model::PortDirection",
            "yss_graph_protocol::model::PortKind",
            "yss_graph_protocol::model::PortDirection::Input",
            "yss_graph_protocol::model::PortDirection::Output",
            "yss_graph_protocol::model::PortKind::Control",
            "yss_graph_protocol::model::PortKind::Data",
            "yss_graph_protocol::model::PortKind::Effect",
            "yss_graph_protocol::types::TypeExpr",
            "yss_graph_registry::RegistryFingerprint",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/editor_projection_types.rs",
        fully_qualified_owner: "yssbi_lib::schema::editor_projection_types",
        canonical_origin_targets: &[
            "yss_graph_analysis_contract::ResourceVersionSet",
            "yss_graph_analysis_contract::basis::ResourceVersionSet",
            "yss_graph_protocol::dataframe::FilterOperator",
            "yss_graph_protocol::parameter::ParameterPresentation",
            "yss_graph_protocol::types::TypeExpr",
            "yss_graph_protocol::model::ParameterPresentation",
            "yss_graph_registry::RegistryFingerprint",
            "yss_graph_registry::fingerprint::RegistryFingerprint",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/graph_clipboard.rs",
        fully_qualified_owner: "yssbi_lib::schema::graph_clipboard",
        canonical_origin_targets: &[
            "yss_graph_catalog::localization::ResourceBoundCreateArgs",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs::Database",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs::Function",
            "yss_graph_catalog::localization::ResourceBoundCreateArgs::Variable",
            "yss_graph_editor::subgraph::clipboard::ClipboardConnection",
            "yss_graph_editor::subgraph::clipboard::ClipboardDynamicMemberOrigin",
            "yss_graph_editor::subgraph::clipboard::ClipboardDynamicPortBinding",
            "yss_graph_editor::subgraph::clipboard::ClipboardInputState",
            "yss_graph_editor::subgraph::clipboard::ClipboardLastKnownPortMetadata",
            "yss_graph_editor::subgraph::clipboard::ClipboardNode",
            "yss_graph_editor::subgraph::clipboard::ClipboardNodeCreation",
            "yss_graph_editor::subgraph::clipboard::ClipboardPortAddress",
            "yss_graph_editor::subgraph::clipboard::ClipboardPortBinding",
            "yss_graph_editor::subgraph::clipboard::ClipboardPortRef",
            "yss_graph_editor::subgraph::clipboard::ClipboardSubgraph",
            "yss_graph_editor::subgraph::clipboard::deserialize_clipboard_subgraph",
            "yss_graph_editor::MutationConflict",
            "yss_graph_editor::mutation::MutationConflict",
            "yss_graph_editor::mutation::MutationConflict::ClipboardSubgraphInvalid",
            "yss_graph_protocol::parameter::ParameterValues",
            "yss_graph_protocol::types::TypeExpr",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/graph_mutation.rs",
        fully_qualified_owner: "yssbi_lib::schema::graph_mutation",
        canonical_origin_targets: &[
            "yss_graph_editor::EditorGraphMutation",
            "yss_graph_editor::mutation::EditorGraphMutation",
            "yss_graph_editor::mutation::EditorGraphMutation::AddPortInstance",
            "yss_graph_editor::mutation::EditorGraphMutation::Connect",
            "yss_graph_editor::mutation::EditorGraphMutation::CreateNode",
            "yss_graph_editor::mutation::EditorGraphMutation::DeleteNodes",
            "yss_graph_editor::mutation::EditorGraphMutation::DisconnectConnections",
            "yss_graph_editor::mutation::EditorGraphMutation::DisconnectNode",
            "yss_graph_editor::mutation::EditorGraphMutation::DisconnectPort",
            "yss_graph_editor::mutation::EditorGraphMutation::DuplicateSubgraph",
            "yss_graph_editor::mutation::EditorGraphMutation::InsertReroute",
            "yss_graph_editor::mutation::EditorGraphMutation::InsertSubgraph",
            "yss_graph_editor::mutation::EditorGraphMutation::MoveConnections",
            "yss_graph_editor::mutation::EditorGraphMutation::MoveNodes",
            "yss_graph_editor::mutation::EditorGraphMutation::RemovePortInstance",
            "yss_graph_editor::mutation::EditorGraphMutation::SetLiteral",
            "yss_graph_editor::mutation::EditorGraphMutation::SetParameters",
            "yss_graph_editor::NodePositionMutation",
            "yss_graph_editor::mutation::NodePositionMutation",
            "yss_graph_protocol::identity::PortKey",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/schema/project.rs",
        fully_qualified_owner: "yssbi_lib::schema::project",
        canonical_origin_targets: &[
            "yss_project_history::HistoryStatusDto",
            "yss_project_history::ResourceKey",
            "yssbi_lib::project::project_writers::ProjectSaveResult",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/control.rs",
        fully_qualified_owner: "yss_graph_catalog::control",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::iid",
            "yss_graph_catalog::builtin::sid",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/core_nodes/mod.rs",
        fully_qualified_owner: "yss_graph_catalog::core_nodes",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/core_nodes/reroute.rs",
        fully_qualified_owner: "yss_graph_catalog::core_nodes::reroute",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/core_nodes/support.rs",
        fully_qualified_owner: "yss_graph_catalog::core_nodes::support",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
            "yss_graph_catalog::builtin::assembled_decimal",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::leaf",
            "yss_graph_catalog::builtin::sid",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/dataframe/mod.rs",
        fully_qualified_owner: "yss_graph_catalog::dataframe",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::iid",
            "yss_graph_catalog::builtin::leaf",
            "yss_graph_catalog::builtin::sid",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/distribution/mod.rs",
        fully_qualified_owner: "yss_graph_catalog::distribution",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::leaf",
            "yss_graph_catalog::builtin::sid",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/plot/mod.rs",
        fully_qualified_owner: "yss_graph_catalog::plot",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::leaf",
            "yss_graph_catalog::builtin::sid",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/project.rs",
        fully_qualified_owner: "yss_graph_catalog::project",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::iid",
            "yss_graph_catalog::builtin::leaf",
            "yss_graph_catalog::builtin::sid",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/lib.rs",
        fully_qualified_owner: "yss_graph_catalog",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::BuiltinInitializationError",
            "yss_graph_catalog::builtin::BuiltinNodeSystem",
            "yss_graph_catalog::builtin::build_builtin_node_system",
            "yss_graph_catalog::builtin::builtin_bundle_parts_for_test",
            "yss_graph_catalog::builtin::validate_builtin_bundle_for_test",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Graph,
        repository_relative_source_file: "src-tauri/crates/yss-graph-catalog/src/statistics/mod.rs",
        fully_qualified_owner: "yss_graph_catalog::statistics",
        canonical_origin_targets: &[
            "yss_graph_catalog::builtin::BuiltinAssemblyError",
            "yss_graph_catalog::builtin::ProviderFragment",
            "yss_graph_catalog::builtin::assembled_decimal",
            "yss_graph_catalog::builtin::assembled_interface",
            "yss_graph_catalog::builtin::assembled_parameters",
            "yss_graph_catalog::builtin::iid",
            "yss_graph_catalog::builtin::leaf",
            "yss_graph_catalog::builtin::sid",
        ],
    },
];

pub(super) fn classify_rust_sources(
    roots: &[ProductionRoot],
    modules: &[RustModule],
) -> Result<BTreeMap<String, RustLayer>, ArchitectureAuditError> {
    let root_keys = roots
        .iter()
        .map(|root| (&root.package_id, &root.target, root.kind))
        .collect::<BTreeSet<_>>();
    let package_by_root = roots
        .iter()
        .map(|root| {
            (
                (root.package_id.as_str(), root.target.as_str(), root.kind),
                root.package.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut memberships = RustLayer::ALL
        .into_iter()
        .map(|layer| (layer, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for module in modules {
        let source_file = normalize_source_file(&module.repository_relative_source_file);
        let root_key = (
            &module.root_package_id,
            &module.root_target,
            module.root_kind,
        );
        if !root_keys.contains(&root_key) {
            return Err(ArchitectureAuditError::UnknownProductionRoot { source_file });
        }
        let package = package_by_root[&(
            module.root_package_id.as_str(),
            module.root_target.as_str(),
            module.root_kind,
        )];

        if module.root_kind == ProductionRootKind::BuildScript {
            memberships
                .get_mut(&RustLayer::BuildScript)
                .expect("all Rust layers are initialized")
                .insert(source_file.clone());
        }
        for layer in non_build_memberships(module, package, &source_file) {
            memberships
                .get_mut(&layer)
                .expect("all Rust layers are initialized")
                .insert(source_file.clone());
        }
    }

    let all_sources = modules
        .iter()
        .map(|module| normalize_source_file(&module.repository_relative_source_file))
        .collect::<BTreeSet<_>>();
    let mut layers_by_source = BTreeMap::<String, Vec<RustLayer>>::new();
    for (layer, sources) in &memberships {
        for source in sources {
            layers_by_source
                .entry(source.clone())
                .or_default()
                .push(*layer);
        }
    }

    let unclassified = all_sources
        .iter()
        .filter(|source| !layers_by_source.contains_key(*source))
        .cloned()
        .collect::<Vec<_>>();
    if !unclassified.is_empty() {
        return Err(ArchitectureAuditError::UnclassifiedProductionSource {
            source_files: unclassified,
        });
    }
    let multiply_classified = layers_by_source
        .iter()
        .filter(|(_, layers)| layers.len() > 1)
        .map(|(source, _)| source.clone())
        .collect::<Vec<_>>();
    if !multiply_classified.is_empty() {
        return Err(ArchitectureAuditError::MultiplyClassifiedProductionSource {
            source_files: multiply_classified,
        });
    }

    Ok(layers_by_source
        .into_iter()
        .map(|(source, mut layers)| {
            let layer = layers
                .pop()
                .expect("total classification was checked before map construction");
            (source, layer)
        })
        .collect())
}

fn non_build_memberships(
    module: &RustModule,
    package: &str,
    source_file: &str,
) -> BTreeSet<RustLayer> {
    let mut layers = BTreeSet::new();
    for (exact_source, layer) in EXACT_SOURCE_MEMBERSHIP {
        if source_file == *exact_source {
            layers.insert(*layer);
        }
    }

    let namespace = module
        .fully_qualified_owner
        .split_once("::")
        .map(|(_, namespace)| namespace)
        .unwrap_or_default();
    let exact_layer = exact_source_layer(source_file);
    if matches!(
        package,
        "yss-bayes-model"
            | "yss-bayes-result"
            | "yss-bayes-worker"
            | "yss-canonical-hash"
            | "yss-computation-settings"
            | "yss-data-contract"
            | "yss-database-contract"
            | "yss-display-naming"
            | "yss-graph-document"
            | "yss-graph-protocol"
            | "yss-graph-resource-contract"
            | "yss-graph-type-mapping"
            | "yss-math"
            | "yss-path-display"
            | "yss-project-change"
            | "yss-project-identity"
            | "yss-project-layout"
            | "yss-project-manifest"
            | "yss-project-progress"
            | "yss-project-registry-contract"
            | "yss-resource-naming"
            | "yss-sci-contract"
            | "yss-tabular-contract"
            | "yss-variable-contract"
            | "yss-variable-value"
            | "yss-worksheet-document"
    ) {
        layers.insert(RustLayer::PureLeaf);
    } else if package == "yss-project-watcher" {
        layers.insert(RustLayer::Application);
    } else if matches!(
        package,
        "yss-project-discovery" | "yss-project-history" | "yss-project-model"
    ) || package == "yss-function-editor-projection"
        || package == "yss-project-filesystem"
        || package == "yss-project-operation"
        || package == "yss-project-registry"
        || package == "yss-resource-lifecycle"
    {
        layers.insert(RustLayer::Project);
    } else if package == "yss-graph-catalog" {
        layers.insert(exact_layer.unwrap_or(RustLayer::Graph));
    } else if matches!(
        package,
        "yss-graph-analysis"
            | "yss-graph-analysis-contract"
            | "yss-graph-compiler"
            | "yss-graph-compiler-diagnostics"
            | "yss-graph-document-edit"
            | "yss-graph-editor"
            | "yss-graph-registry"
            | "yss-graph-runtime"
    ) {
        layers.insert(RustLayer::Graph);
    } else if matches!(
        package,
        "yss-database-edit"
            | "yss-database-runtime"
            | "yss-database-schema"
            | "yss-dataset-profile"
            | "yss-duckdb"
            | "yss-sql-source"
            | "yss-tabular-io"
    ) {
        layers.insert(RustLayer::DatabaseCore);
    } else if matches!(package, "yss-sci" | "yss-sci-runtime") {
        layers.insert(RustLayer::SciCore);
    } else if package == "yss-diagnostics" {
        layers.insert(RustLayer::Diagnostics);
    } else if package == "yss-execution" {
        layers.insert(RustLayer::Execution);
    } else if package == "yss-tracing" {
        layers.insert(RustLayer::Logging);
    } else if matches!(package, "yss-project-watcher-notify" | "yss-window-state") {
        layers.insert(RustLayer::PlatformAdapter);
    } else if matches!(
        package,
        "yss-bayes-worker-julia"
            | "yss-julia-runtime"
            | "yss-julia-worker"
            | "yss-project-registry-sqlite"
            | "yss-tabular-polars"
    ) {
        layers.insert(RustLayer::BackendAdapter);
    } else if let Some(layer) = cohesive_owner_layer(namespace, exact_layer) {
        layers.insert(layer);
    }
    layers
}

fn exact_source_layer(source_file: &str) -> Option<RustLayer> {
    EXACT_SOURCE_MEMBERSHIP
        .iter()
        .find_map(|(source, layer)| (*source == source_file).then_some(*layer))
}

fn cohesive_owner_layer(namespace: &str, exact_layer: Option<RustLayer>) -> Option<RustLayer> {
    if namespace.is_empty() {
        return None;
    }
    let owner = namespace.split("::").next().unwrap_or_default();
    match owner {
        "application" => Some(RustLayer::Application),
        "commands" => Some(RustLayer::Commands),
        "project" => Some(RustLayer::Project),
        "database" => Some(RustLayer::DatabaseCore),
        "execution" => Some(RustLayer::Execution),
        "sci" => Some(RustLayer::SciCore),
        "julia" => Some(RustLayer::BackendAdapter),
        "event" | "schema" | "error" => Some(RustLayer::Transport),
        "graph" if exact_layer == Some(RustLayer::BuiltinComposition) => None,
        "graph" => Some(RustLayer::Graph),
        "backend_adapters" => Some(RustLayer::BackendAdapter),
        "platform" => Some(RustLayer::PlatformAdapter),
        _ => None,
    }
}

fn normalize_source_file(source_file: &str) -> String {
    source_file.replace('\\', "/")
}

pub(super) fn rust_dependency_findings(
    dependencies: &[CanonicalDependency],
    classification: &BTreeMap<String, RustLayer>,
) -> Result<Vec<ArchitectureFinding>, ArchitectureAuditError> {
    rust_dependency_findings_with_capabilities(
        dependencies,
        classification,
        RUST_INTERNAL_CAPABILITIES,
    )
}

pub(super) fn rust_dependency_findings_with_capabilities(
    dependencies: &[CanonicalDependency],
    classification: &BTreeMap<String, RustLayer>,
    capabilities: &[InternalDependencyCapability],
) -> Result<Vec<ArchitectureFinding>, ArchitectureAuditError> {
    validate_internal_capabilities(capabilities)?;
    let mut findings = dependencies
        .iter()
        .filter_map(|dependency| {
            let CanonicalOrigin::Repository {
                repository_relative_declaration_file,
                ..
            } = &dependency.origin
            else {
                return None;
            };
            let source_layer = classification.get(&dependency.source_file).copied()?;
            let target_layer = classification
                .get(repository_relative_declaration_file)
                .copied();
            if target_layer.is_some_and(|target| {
                internal_layer_dependency_is_allowed(source_layer, target)
                    || capabilities.iter().any(|capability| {
                        capability.source_layer == source_layer
                            && capability.repository_relative_source_file == dependency.source_file
                            && capability.fully_qualified_owner == dependency.owner
                            && capability
                                .canonical_origin_targets
                                .contains(&dependency.canonical_origin_target.as_str())
                    })
            }) {
                return None;
            }
            Some(ArchitectureFinding {
                key: ArchitectureFindingKey {
                    rule_id: "rust.internal.source-layer".to_owned(),
                    repository_relative_source_file: dependency.source_file.clone(),
                    fully_qualified_owner: dependency.owner.clone(),
                    dependency_kind: dependency.kind,
                    canonical_origin_target: dependency.canonical_origin_target.clone(),
                },
                source_layer,
                target_layer,
                line: dependency.line,
                column: dependency.column,
            })
        })
        .collect::<Vec<_>>();
    findings.sort();
    Ok(findings)
}

fn validate_internal_capabilities(
    capabilities: &[InternalDependencyCapability],
) -> Result<(), ArchitectureAuditError> {
    let mut unique = BTreeSet::new();
    for capability in capabilities {
        let invalid_literal = capability.repository_relative_source_file.is_empty()
            || capability
                .repository_relative_source_file
                .contains(['*', '\\'])
            || capability.fully_qualified_owner.is_empty()
            || capability.fully_qualified_owner.contains('*')
            || capability.canonical_origin_targets.is_empty();
        if invalid_literal {
            return Err(ArchitectureAuditError::InvalidInternalCapability {
                message: format!("{capability:?}"),
            });
        }
        for target in capability.canonical_origin_targets {
            if target.is_empty() || target.contains('*') || target.starts_with("external:") {
                return Err(ArchitectureAuditError::InvalidInternalCapability {
                    message: format!("{capability:?}"),
                });
            }
            let key = (
                capability.source_layer,
                capability.repository_relative_source_file,
                capability.fully_qualified_owner,
                *target,
            );
            if !unique.insert(key) {
                return Err(ArchitectureAuditError::InvalidInternalCapability {
                    message: format!("duplicate {capability:?}"),
                });
            }
        }
    }
    Ok(())
}

fn internal_layer_dependency_is_allowed(source: RustLayer, target: RustLayer) -> bool {
    if source == target {
        return source != RustLayer::BuildScript;
    }
    matches!(
        (source, target),
        (
            RustLayer::CompositionRoot,
            RustLayer::Commands
                | RustLayer::Logging
                | RustLayer::Diagnostics
                | RustLayer::PureLeaf
        ) | (RustLayer::Commands, RustLayer::PureLeaf)
            | (
                RustLayer::PlatformAdapter,
                RustLayer::Diagnostics | RustLayer::PureLeaf
            )
            | (
                RustLayer::Application,
                RustLayer::Project
                    | RustLayer::Graph
                    | RustLayer::Execution
                    | RustLayer::SciCore
                    | RustLayer::DatabaseCore
                    | RustLayer::Diagnostics
                    | RustLayer::PureLeaf
            )
            | (RustLayer::Project, RustLayer::PureLeaf)
            | (RustLayer::Graph, RustLayer::PureLeaf)
            | (RustLayer::Execution, RustLayer::PureLeaf)
            | (RustLayer::SciCore, RustLayer::PureLeaf)
            | (RustLayer::DatabaseCore, RustLayer::PureLeaf)
            | (
                RustLayer::BackendAdapter,
                RustLayer::DatabaseCore | RustLayer::Diagnostics | RustLayer::PureLeaf
            )
            // The built-in catalog is the one named assembly seam that wires
            // Graph-owned protocol/registry modules into the final catalog.
            // This is a layer edge, not a wildcard capability; individual
            // Graph submodule -> built-in imports remain exact capabilities.
            | (
                RustLayer::BuiltinComposition,
                RustLayer::Graph | RustLayer::PureLeaf,
            )
            | (RustLayer::Transport, RustLayer::PureLeaf)
            | (
                RustLayer::Diagnostics,
                RustLayer::Logging | RustLayer::PureLeaf
            )
    )
}
