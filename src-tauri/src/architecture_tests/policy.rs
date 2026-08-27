use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    ArchitectureAuditError, ArchitectureFinding, CanonicalDependency, CanonicalOrigin, DebtKey,
    ProductionRoot, ProductionRootKind, RustLayer, RustModule,
};

const EXACT_SOURCE_MEMBERSHIP: &[(&str, RustLayer)] = &[
    ("src-tauri/src/lib.rs", RustLayer::CompositionRoot),
    ("src-tauri/src/main.rs", RustLayer::CompositionRoot),
    ("src-tauri/src/execution/mod.rs", RustLayer::Execution),
    ("src-tauri/src/execution/value.rs", RustLayer::Execution),
    ("src-tauri/src/execution/identity.rs", RustLayer::Execution),
    ("src-tauri/src/execution/error.rs", RustLayer::Execution),
    ("src-tauri/src/execution/canonical.rs", RustLayer::Execution),
    ("src-tauri/src/execution/settings.rs", RustLayer::Execution),
    ("src-tauri/src/execution/plan/mod.rs", RustLayer::Execution),
    (
        "src-tauri/src/execution/plan/identity.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/basis.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/model.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/package.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/parameter.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/observation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/validation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/plan/validation/control.rs",
        RustLayer::Execution,
    ),
    ("src-tauri/src/execution/ports/mod.rs", RustLayer::Execution),
    ("src-tauri/src/graph/value/semantics.rs", RustLayer::Graph),
    (
        "src-tauri/src/project/variable_defaults.rs",
        RustLayer::Project,
    ),
    (
        "src-tauri/src/application/execution/session_factory.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/src/execution/ports/scientific.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/ports/relational.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/ports/resources.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/src/execution/resource_preparation.rs",
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
        "src-tauri/src/backend_adapters/tabular/mod.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/backend_adapters/tabular/polars.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/backend_adapters/project_registry_sqlite.rs",
        RustLayer::BackendAdapter,
    ),
    ("src-tauri/src/julia/mod.rs", RustLayer::BackendAdapter),
    ("src-tauri/src/julia/worker.rs", RustLayer::BackendAdapter),
    (
        "src-tauri/src/julia/worker/assets.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/julia/worker/error.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/julia/worker/task_directory.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/julia/bayes_worker_adapter/mod.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/julia/bayes_worker_adapter/fit.rs",
        RustLayer::BackendAdapter,
    ),
    (
        "src-tauri/src/julia/bayes_worker_adapter/predictor.rs",
        RustLayer::BackendAdapter,
    ),
    ("src-tauri/src/data_contract/mod.rs", RustLayer::PureLeaf),
    (
        "src-tauri/src/data_contract/data_type.rs",
        RustLayer::PureLeaf,
    ),
    (
        "src-tauri/src/data_contract/data_value.rs",
        RustLayer::PureLeaf,
    ),
    ("src-tauri/src/graph_document/mod.rs", RustLayer::PureLeaf),
    (
        "src-tauri/src/graph_document/identity.rs",
        RustLayer::PureLeaf,
    ),
    ("src-tauri/src/graph_document/model.rs", RustLayer::PureLeaf),
    (
        "src-tauri/src/graph_document/resource_path.rs",
        RustLayer::PureLeaf,
    ),
    (
        "src-tauri/src/node_system/protocol/identity.rs",
        RustLayer::PureLeaf,
    ),
    (
        "src-tauri/src/node_system/protocol/types.rs",
        RustLayer::PureLeaf,
    ),
    ("src-tauri/src/graph/value/type_system.rs", RustLayer::Graph),
    ("src-tauri/src/tabular/mod.rs", RustLayer::PureLeaf),
    ("src-tauri/src/tabular/contract.rs", RustLayer::PureLeaf),
    (
        "src-tauri/src/database_contract/mod.rs",
        RustLayer::PureLeaf,
    ),
    (
        "src-tauri/src/database_contract/declaration.rs",
        RustLayer::PureLeaf,
    ),
    (
        "src-tauri/src/database_contract/engine.rs",
        RustLayer::PureLeaf,
    ),
    (
        "src-tauri/src/database/runtime/mod.rs",
        RustLayer::DatabaseCore,
    ),
    (
        "src-tauri/src/database/runtime/registry.rs",
        RustLayer::DatabaseCore,
    ),
    (
        "src-tauri/src/database/session_api.rs",
        RustLayer::DatabaseCore,
    ),
    (
        "src-tauri/src/database/schema_snapshot.rs",
        RustLayer::DatabaseCore,
    ),
    ("src-tauri/src/platform/mod.rs", RustLayer::PlatformAdapter),
    (
        "src-tauri/src/platform/project_file_watcher.rs",
        RustLayer::PlatformAdapter,
    ),
    (
        "src-tauri/src/node_system/catalog/builtin.rs",
        RustLayer::BuiltinComposition,
    ),
    (
        "src-tauri/src/node_system/runtime/builtin.rs",
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
        canonical_origin_targets: &["yssbi_lib::database::schema_snapshot::DatabaseColumnFact"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::DatabaseCore,
        repository_relative_source_file: "src-tauri/src/database/edit_operation.rs",
        fully_qualified_owner: "yssbi_lib::database::edit_operation",
        canonical_origin_targets: &[
            "yssbi_lib::backend_adapters::tabular::polars::json_to_anyvalue",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/backend_adapters/execution/scientific.rs",
        fully_qualified_owner: "yssbi_lib::backend_adapters::execution::scientific",
        canonical_origin_targets: &[
            "yssbi_lib::execution::ports::scientific::AcfPacfRequest",
            "yssbi_lib::execution::ports::scientific::AcfPacfResult",
            "yssbi_lib::execution::ports::scientific::BackendExecutionControl",
            "yssbi_lib::execution::ports::scientific::ExecutionInstrumentalVariableKind",
            "yssbi_lib::execution::ports::scientific::ExecutionRegressionKind",
            "yssbi_lib::execution::ports::scientific::ExecutionStatisticalTrend",
            "yssbi_lib::execution::ports::scientific::KdePoint",
            "yssbi_lib::execution::ports::scientific::KernelDensityRequest",
            "yssbi_lib::execution::ports::scientific::KernelDensityResult",
            "yssbi_lib::execution::ports::scientific::ScientificBackend",
            "yssbi_lib::execution::ports::scientific::ScientificBackendError",
            "yssbi_lib::execution::ports::scientific::ScientificInputViolation",
            "yssbi_lib::execution::ports::scientific::StatisticsOperation",
            "yssbi_lib::execution::ports::scientific::StatisticsParameters",
            "yssbi_lib::execution::ports::scientific::StatisticsRequest",
            "yssbi_lib::execution::ports::scientific::StatisticsResult",
            "yssbi_lib::execution::settings::ExecutionMissingValuePolicy",
            "yssbi_lib::execution::settings::ExecutionSettings",
            "yssbi_lib::sci::api::computation::MissingValuePolicy",
            "yssbi_lib::sci::api::computation::NumericTolerance",
            "yssbi_lib::sci::api::computation::SciComputationSettings",
            "yssbi_lib::sci::api::computation::StatisticalObservationMetadata",
            "yssbi_lib::sci::api::computation::StatisticalSettingSource",
            "yssbi_lib::sci::api::control::AbsoluteDeadline",
            "yssbi_lib::sci::api::control::ExecutionControl",
            "yssbi_lib::sci::api::control::SciCancellationSource",
            "yssbi_lib::sci::api::density::KernelDensityInput",
            "yssbi_lib::sci::api::density::compute_kernel_density",
            "yssbi_lib::sci::api::node_statistics::InstrumentalVariableKind",
            "yssbi_lib::sci::api::node_statistics::RegressionKind",
            "yssbi_lib::sci::api::node_statistics::augmented_dickey_fuller",
            "yssbi_lib::sci::api::node_statistics::fit_instrumental_variables",
            "yssbi_lib::sci::api::node_statistics::fit_panel",
            "yssbi_lib::sci::api::node_statistics::fit_regression",
            "yssbi_lib::sci::api::node_statistics::var_fit",
            "yssbi_lib::sci::api::node_statistics::var_lag_order",
            "yssbi_lib::sci::api::node_statistics::vec_fit",
            "yssbi_lib::sci::api::node_statistics::vec_rank_test",
            "yssbi_lib::sci::api::time_series::acf_pacf::AcfPacfInput",
            "yssbi_lib::sci::api::time_series::acf_pacf::compute_acf_pacf",
            "yssbi_lib::sci::error::SciError",
            "yssbi_lib::sci::error::SciInputViolation",
            "yssbi_lib::sci::error::SciOperationCode",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/julia/bayes_worker_adapter/mod.rs",
        fully_qualified_owner: "yssbi_lib::julia::bayes_worker_adapter",
        canonical_origin_targets: &[
            "yssbi_lib::sci::api::bayes::worker::BayesArtifact",
            "yssbi_lib::sci::api::bayes::worker::BayesArtifactHandle",
            "yssbi_lib::sci::api::bayes::worker::BayesCancelTerminal",
            "yssbi_lib::sci::api::bayes::worker::BayesTaskHandle",
            "yssbi_lib::sci::api::bayes::worker::BayesTaskId",
            "yssbi_lib::sci::api::bayes::worker::BayesTaskResult",
            "yssbi_lib::sci::api::bayes::worker::BayesWorkerError",
            "yssbi_lib::sci::api::bayes::worker::BayesWorkerPhase",
            "yssbi_lib::sci::api::bayes::worker::BayesWorkerPort",
            "yssbi_lib::sci::api::bayes::worker::BayesWorkerTerminalCode",
            "yssbi_lib::sci::api::bayes::worker::ValidatedBayesTask",
            "yssbi_lib::sci::api::control::CancelDeliveryControl",
            "yssbi_lib::sci::api::control::ExecutionControl",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/julia/bayes_worker_adapter/fit.rs",
        fully_qualified_owner: "yssbi_lib::julia::bayes_worker_adapter::fit",
        canonical_origin_targets: &[
            "yssbi_lib::sci::api::bayes::contract::InferenceDiagnostics",
            "yssbi_lib::sci::api::bayes::contract::ParameterSummary",
            "yssbi_lib::sci::api::bayes::model::Expression",
            "yssbi_lib::sci::api::bayes::model::InferenceConfig",
            "yssbi_lib::sci::api::bayes::model::LikelihoodSpec",
            "yssbi_lib::sci::api::bayes::model::ParameterSpec",
            "yssbi_lib::sci::api::bayes::worker::ArtifactId",
            "yssbi_lib::sci::api::bayes::worker::BayesArtifactHandle",
            "yssbi_lib::sci::api::bayes::worker::BayesArtifactMediaType",
            "yssbi_lib::sci::api::bayes::worker::BayesInferenceSnapshot",
            "yssbi_lib::sci::api::bayes::worker::BayesTaskHandle",
            "yssbi_lib::sci::api::bayes::worker::BayesTaskResult",
            "yssbi_lib::sci::api::bayes::worker::BayesWorkerError",
            "yssbi_lib::sci::api::bayes::worker::BayesWorkerTerminalCode::Failed",
            "yssbi_lib::sci::api::bayes::worker::ValidatedBayesTask",
            "yssbi_lib::sci::api::computation::CategoricalRole",
            "yssbi_lib::sci::api::computation::StatisticalInput",
            "yssbi_lib::sci::api::computation::StatisticalScalar",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "src-tauri/src/julia/bayes_worker_adapter/predictor.rs",
        fully_qualified_owner: "yssbi_lib::julia::bayes_worker_adapter::predictor",
        canonical_origin_targets: &[
            "yssbi_lib::sci::api::bayes::model::BayesModelSpec",
            "yssbi_lib::sci::api::bayes::model::BinaryOp",
            "yssbi_lib::sci::api::bayes::model::Expression",
            "yssbi_lib::sci::api::bayes::model::LikelihoodSpec",
            "yssbi_lib::sci::api::bayes::model::MathFunction",
            "yssbi_lib::sci::api::bayes::model::UnaryOp",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/src/lib.rs",
        fully_qualified_owner: "yssbi_lib",
        canonical_origin_targets: &[
            "yssbi_lib::application::bayes::BayesInferenceService::with_backend",
            "yssbi_lib::julia::worker::JuliaWorkerManager::new",
            "yssbi_lib::node_system::catalog::builtin::BuiltinInitializationError",
            "yssbi_lib::project::project_picker_task::ProjectPickerTaskCancelRegistry::new",
            "yssbi_lib::project::project_registry::ProjectRegistry::init",
            "yssbi_lib::project::project_state::state::ProjectState",
            "yssbi_lib::project::project_state::state::ProjectState::try_new",
            "yssbi_lib::application::project_watcher::ProjectWatcherState::new",
            "yssbi_lib::platform::NotifyProjectFileWatcher::new",
            "yssbi_lib::sci::backends::julia::bayes::fit::JuliaBayesBackend::new",
            "yssbi_lib::window_state::WindowStateStore::load",
            "yssbi_lib::window_state::apply_main_window_state",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_bayes.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_bayes",
        canonical_origin_targets: &[
            "yssbi_lib::application::bayes::BayesApplicationError",
            "yssbi_lib::application::bayes::BayesInferenceService",
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
            "yssbi_lib::application::database::export_database_for_project",
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
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::ResourceMutationCommandResultDto",
            "yssbi_lib::schema::database::DatabaseImportSourceDTO",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_diagnostics/mod.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_diagnostics",
        canonical_origin_targets: &[
            "yssbi_lib::diagnostics::dto::DiagnosticBatchDto",
            "yssbi_lib::diagnostics::dto::DiagnosticSubscriptionDto",
            "yssbi_lib::diagnostics::dto::FrontendDiagnosticEntryDto",
            "yssbi_lib::diagnostics::runtime::DiagnosticsRuntime",
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
        canonical_origin_targets: &["yssbi_lib::error::CommandError"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/catalog.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::catalog",
        canonical_origin_targets: &[
            "yssbi_lib::application::catalog_compatibility::CatalogCompatibilityError",
            "yssbi_lib::application::catalog_compatibility::CatalogCompatibilityRequest",
            "yssbi_lib::application::catalog_compatibility::get_compatible_node_catalog",
            "yssbi_lib::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/common.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::common",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::error::GraphMutationErrorDetailsDto",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::ResourceMutationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/editor.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::editor",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::GraphMutationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/execution.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::execution",
        canonical_origin_targets: &[
            "yssbi_lib::application::graph_execution::GraphExecutionDeliveryReport",
            "yssbi_lib::application::graph_execution::GraphExecutionRequest",
            "yssbi_lib::application::graph_execution::GraphExecutionStreamEvent",
            "yssbi_lib::application::graph_execution::TerminalRunEventKind",
            "yssbi_lib::application::graph_execution::execute_graph",
            "yssbi_lib::application::pin_preview_generation::allocate_pin_preview_generation",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/history.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::history",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::ResourceMutationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/resources.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::resources",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::ResourceMutationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_node_system/results.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_node_system::results",
        canonical_origin_targets: &["yssbi_lib::error::CommandError"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_panel_did.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_panel_did",
        canonical_origin_targets: &["yssbi_lib::error::CommandError"],
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
        repository_relative_source_file: "src-tauri/src/commands/command_project/lifecycle.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::lifecycle",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_lifecycle::ProjectLifecycleError",
            "yssbi_lib::application::project_lifecycle::clear_project",
            "yssbi_lib::application::project_lifecycle::create_project",
            "yssbi_lib::application::project_lifecycle::load_project",
            "yssbi_lib::application::project_lifecycle::save_project_as",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::emit_project_event_result",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::LifecycleMutationOutcomeDto",
            "yssbi_lib::event::event_project::LifecycleMutationResultDto",
            "yssbi_lib::event::event_project::ProjectActivationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/lifecycle.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::lifecycle",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_watcher::ProjectWatcherError",
            "yssbi_lib::application::project_watcher::ProjectWatcherSink",
            "yssbi_lib::application::project_watcher::ProjectWatcherSinkError",
            "yssbi_lib::application::project_watcher::ProjectWatcherState",
            "yssbi_lib::event::event_resource::EventResource",
            "yssbi_lib::project::project_change::ProjectIndexInvalidation",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::PlatformAdapter,
        repository_relative_source_file: "src-tauri/src/platform/project_file_watcher.rs",
        fully_qualified_owner: "yssbi_lib::platform::project_file_watcher",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_watcher::ProjectFileChangeCallback",
            "yssbi_lib::application::project_watcher::ProjectFileWatcher",
            "yssbi_lib::application::project_watcher::ProjectFileWatcherHandle",
            "yssbi_lib::application::project_watcher::ProjectWatcherSourceError",
            "yssbi_lib::project::project_change::ProjectFileChange",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/path.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::path",
        canonical_origin_targets: &["yssbi_lib::error::CommandError"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/query.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::query",
        canonical_origin_targets: &[
            "yssbi_lib::application::database_schema::project_databases_variables",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::event_project::ProjectActivationResultDto",
            "yssbi_lib::schema::project::DatabasesVariablesDTO",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/registry.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::registry",
        canonical_origin_targets: &[
            "yssbi_lib::application::project_lifecycle::delete_registered_project",
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::event_project::LifecycleMutationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_project/settings.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_project::settings",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_sci.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_sci",
        canonical_origin_targets: &["yssbi_lib::error::CommandError"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_serial_tests.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_serial_tests",
        canonical_origin_targets: &["yssbi_lib::error::CommandError"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_variable/mod.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_variable",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::ResourceMutationResultDto",
            "yssbi_lib::schema::variables::VariableInstanceDTO",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_window.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_window",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::window_state::WindowStateStore",
            "yssbi_lib::window_state::kind::WindowKind",
            "yssbi_lib::window_state::kind::WindowState",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/src/commands/command_worksheet.rs",
        fully_qualified_owner: "yssbi_lib::commands::command_worksheet",
        canonical_origin_targets: &[
            "yssbi_lib::error::CommandError",
            "yssbi_lib::event::Event",
            "yssbi_lib::event::emit_project_event",
            "yssbi_lib::event::event_project::EventProject",
            "yssbi_lib::event::event_project::ResourceMutationResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BuiltinComposition,
        repository_relative_source_file: "src-tauri/src/node_system/catalog/builtin.rs",
        fully_qualified_owner: "yssbi_lib::node_system::catalog::builtin",
        canonical_origin_targets: &[
            "yssbi_lib::node_system::catalog::localization::BuiltinCatalog",
            "yssbi_lib::node_system::catalog::localization::Message",
            "yssbi_lib::node_system::catalog::localization::Message::Aliases",
            "yssbi_lib::node_system::catalog::localization::Message::Text",
            "yssbi_lib::node_system::compiler::diagnostics::COMPILER_DIAGNOSTIC_DEFINITIONS",
            "yssbi_lib::node_system::compiler::diagnostics::CompilerDiagnosticDefinitionError",
            "yssbi_lib::node_system::compiler::diagnostics::validate_compiler_diagnostic_definitions",
            "yssbi_lib::node_system::compiler::lowering::LoweredKernel",
            "yssbi_lib::node_system::compiler::lowering::LoweredNode",
            "yssbi_lib::node_system::compiler::lowering::LoweringContext",
            "yssbi_lib::node_system::compiler::lowering::LoweringError",
            "yssbi_lib::node_system::compiler::lowering::LoweringInvariant",
            "yssbi_lib::node_system::compiler::lowering::NodeImplementation",
            "yssbi_lib::node_system::compiler::lowering::NodeLowerer",
            "yssbi_lib::node_system::compiler::project::builtin_function_interface_resolver_ids",
            "yssbi_lib::node_system::plan::model::CompiledParameterHandle",
            "yssbi_lib::node_system::plan::model::KernelHandle",
            "yssbi_lib::node_system::catalog::control::register",
            "yssbi_lib::node_system::catalog::core_nodes::build_provider_fragment",
            "yssbi_lib::node_system::catalog::dataframe::DataframeNominalHandles",
            "yssbi_lib::node_system::catalog::dataframe::bind_nominal_handles",
            "yssbi_lib::node_system::catalog::dataframe::build_provider_fragment",
            "yssbi_lib::node_system::catalog::distribution::build_provider_fragment",
            "yssbi_lib::node_system::catalog::localization::I18nBundleValidationError",
            "yssbi_lib::node_system::catalog::plot::build_provider_fragment",
            "yssbi_lib::node_system::catalog::project::register",
            "yssbi_lib::node_system::catalog::statistics::build_provider_fragment",
            "yssbi_lib::node_system::protocol::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION",
            "yssbi_lib::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID",
            "yssbi_lib::node_system::protocol::dataframe::FILTER_PREDICATE_VALIDATOR_ID",
            "yssbi_lib::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID",
            "yssbi_lib::node_system::protocol::dataframe::PROJECT_COLUMNS_VALIDATOR_ID",
            "yssbi_lib::node_system::protocol::dataframe::prepare_filter_predicate_json",
            "yssbi_lib::node_system::protocol::dataframe::prepare_project_columns_json",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BuiltinComposition,
        repository_relative_source_file: "src-tauri/src/node_system/runtime/builtin.rs",
        fully_qualified_owner: "yssbi_lib::node_system::runtime::builtin",
        canonical_origin_targets: &[
            "yssbi_lib::node_system::plan::model::KernelHandle",
            "yssbi_lib::node_system::plan::model::ResourceId",
            "yssbi_lib::node_system::runtime::kernel::Kernel",
            "yssbi_lib::node_system::runtime::kernel::KernelRegistry",
            "yssbi_lib::node_system::runtime::kernels::build_kernel_fragments",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/error/mod.rs",
        fully_qualified_owner: "yssbi_lib::error",
        canonical_origin_targets: &[
            "yssbi_lib::project::project_error::ProjectError",
            "yssbi_lib::project::project_error::ProjectFilesystemError",
            "yssbi_lib::project::project_state_database::ProjectDatabaseError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/event/event_project.rs",
        fully_qualified_owner: "yssbi_lib::event::event_project",
        canonical_origin_targets: &[
            "yssbi_lib::node_system::analysis::projection::function_editor::FunctionEditorProjectionDto",
            "yssbi_lib::node_system::analysis::projection::types::EditorGraphProjectionDto",
            "yssbi_lib::node_system::document::history::HistoryStatusDto",
            "yssbi_lib::node_system::document::history::ResourceDeltaEvent",
            "yssbi_lib::node_system::document::history::ResourceLifecycleKind",
            "yssbi_lib::project::identity::OperationId",
            "yssbi_lib::node_system::document::mutation::GraphDeltaEvent",
            "yssbi_lib::node_system::document::patch::GraphDocumentPatch",
            "yssbi_lib::project::computation_settings::ComputationSettingsMutationReceipt",
            "yssbi_lib::project::project_registry::ProjectRecord",
            "yssbi_lib::project::project_writers::ProjectSaveResultDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/src/event/event_resource.rs",
        fully_qualified_owner: "yssbi_lib::event::event_resource",
        canonical_origin_targets: &["yssbi_lib::project::project_session::ProjectInstanceId"],
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
    if package == "yss-sci" {
        layers.insert(RustLayer::SciCore);
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
        "diagnostics" => Some(RustLayer::Diagnostics),
        "execution" => Some(RustLayer::Execution),
        "sci" if namespace.starts_with("sci::backends::julia") => Some(RustLayer::BackendAdapter),
        "sci" => Some(RustLayer::SciCore),
        "julia" => Some(RustLayer::BackendAdapter),
        "event" | "schema" | "error" => Some(RustLayer::Transport),
        "window_state" => Some(RustLayer::PlatformAdapter),
        "math" => Some(RustLayer::Execution),
        "graph" => Some(RustLayer::Graph),
        "graph_document" | "tabular" | "variable" => Some(RustLayer::PureLeaf),
        "node_system" if exact_layer == Some(RustLayer::BuiltinComposition) => None,
        "node_system" if namespace.starts_with("node_system::runtime") => {
            Some(RustLayer::Execution)
        }
        "node_system" if exact_layer == Some(RustLayer::PureLeaf) => None,
        "node_system" => Some(RustLayer::Graph),
        "backend_adapters" => Some(RustLayer::BackendAdapter),
        "database_contract" => Some(RustLayer::PureLeaf),
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
                key: DebtKey {
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
            RustLayer::Diagnostics | RustLayer::PureLeaf
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
            | (RustLayer::BuiltinComposition, RustLayer::PureLeaf)
            | (RustLayer::Transport, RustLayer::PureLeaf)
            | (RustLayer::Diagnostics, RustLayer::PureLeaf)
    )
}
