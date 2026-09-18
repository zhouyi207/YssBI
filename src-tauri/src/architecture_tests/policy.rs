use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    ArchitectureAuditError, ArchitectureFinding, ArchitectureFindingKey, CanonicalDependency,
    CanonicalOrigin, ProductionRoot, ProductionRootKind, RustLayer, RustModule,
};

const EXACT_SOURCE_MEMBERSHIP: &[(&str, RustLayer)] = &[
    ("src-tauri/src/lib.rs", RustLayer::CompositionRoot),
    ("src-tauri/src/main.rs", RustLayer::CompositionRoot),
    (
        "src-tauri/crates/yss-application/src/runtime.rs",
        RustLayer::CompositionRoot,
    ),
    (
        "src-tauri/crates/yss-application/src/runtime/harness.rs",
        RustLayer::CompositionRoot,
    ),
    (
        "src-tauri/crates/yss-application/src/ipc/runtime.rs",
        RustLayer::CompositionRoot,
    ),
    (
        "src-tauri/crates/yss-application/src/ipc/mod.rs",
        RustLayer::Commands,
    ),
    (
        "src-tauri/crates/yss-application/src/ipc/activity_panel_sync.rs",
        RustLayer::Transport,
    ),
    (
        "src-tauri/crates/yss-application/src/ipc/graph_editor_sync.rs",
        RustLayer::Transport,
    ),
    (
        "src-tauri/crates/yss-application/src/ipc/graph_projection_runtime.rs",
        RustLayer::Commands,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/lib.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/identity.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/error.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/package_preparation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/result_store.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/run_registry.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/mod.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/identity.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/basis.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/model.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/package.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/parameter.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/observation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/validation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/plan/validation/control.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/ports/mod.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-analysis/src/lib.rs",
        RustLayer::Graph,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/graph_preparation.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-project/src/execution_authority.rs",
        RustLayer::Project,
    ),
    (
        "src-tauri/crates/yss-project/src/project_state/function_mutation.rs",
        RustLayer::Project,
    ),
    (
        "src-tauri/crates/yss-application/src/session/factory.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/crates/yss-application/src/execution/mod.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/crates/yss-application/src/session/slot.rs",
        RustLayer::Application,
    ),
    (
        "src-tauri/crates/yss-graph-editor/src/projection/mod.rs",
        RustLayer::Graph,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/ports/resources.rs",
        RustLayer::Execution,
    ),
    (
        "src-tauri/crates/yss-graph-execution/src/resource_preparation.rs",
        RustLayer::Execution,
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
        repository_relative_source_file: "src-tauri/crates/yss-ipc-contract/src/graph_editing.rs",
        fully_qualified_owner: "yss_ipc_contract::graph_editing",
        canonical_origin_targets: &["yss_function_editor_projection::FunctionEditorProjection"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/channel/graph_activity.rs",
        fully_qualified_owner: "yss_application::ipc::channel::graph_activity",
        canonical_origin_targets: &[
            "yss_application::graph::editing::GraphActivity",
            "yss_application::graph::editing::GraphActivity::Changed",
            "yss_application::graph::editing::GraphActivity::Execution",
            "yss_application::graph::editing::GraphActivitySubscription",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Execution,
        repository_relative_source_file: "src-tauri/crates/yss-graph-execution/src/graph_preparation.rs",
        fully_qualified_owner: "yss_graph_execution::graph_preparation",
        canonical_origin_targets: &[
            "yss_graph_analysis::GraphAnalysis",
            "yss_graph_analysis::GraphSemanticSnapshot",
            "yss_graph_analysis::GraphNodeSemanticFact",
            "yss_graph_analysis::GraphPortSemanticFact",
            "yss_graph_analysis::GraphKernelSpecialization",
            "yss_graph_analysis::GraphPortTypeBinding",
            "yss_graph_analysis::GraphResolvedInputSource",
            "yss_graph_analysis::GraphResolvedInputSource::Output",
            "yss_graph_analysis::GraphResolvedInputSource::Literal",
            "yss_graph_analysis::GraphResolvedParameterValue",
            "yss_graph_analysis::GraphResolvedParameterValue::Resource",
            "yss_graph_analysis::GraphResolvedParameterValue::Literal",
            "yss_graph_analysis::GraphResolvedParameterValue::DefaultLiteral",
            "yss_graph_analysis::result_category::GraphPlotDataKind",
            "yss_graph_analysis::result_category::GraphResultCategory",
            "yss_graph_analysis::result_category::GraphStatisticalReportKind",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/src/lib.rs",
        fully_qualified_owner: "yssbi_lib",
        canonical_origin_targets: &["tauri_plugin_tracing::plugin::init"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Application,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/lib.rs",
        fully_qualified_owner: "yss_application",
        canonical_origin_targets: &[
            "yss_application::runtime::initialize",
            "yss_application::ipc::invoke_handler",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::NodeKernel,
        repository_relative_source_file: "src-tauri/crates/yss-node-kernel/src/builtins/conversion.rs",
        fully_qualified_owner: "yss_node_kernel::builtins::conversion",
        canonical_origin_targets: &["yss_tabular_arrow::conversion::convert_semantic_values"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::NodeKernel,
        repository_relative_source_file: "src-tauri/crates/yss-node-kernel/src/builtins/statistics.rs",
        fully_qualified_owner: "yss_node_kernel::builtins::statistics",
        canonical_origin_targets: &["yss_sci_runtime::computation::linear_regression"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Execution,
        repository_relative_source_file: "src-tauri/crates/yss-graph-execution/src/result/analysis.rs",
        fully_qualified_owner: "yss_graph_execution::result::analysis",
        canonical_origin_targets: &[
            "yss_sci_runtime::computation::acf_pacf",
            "yss_sci_runtime::time_series::serial_tests::compute_serial_tests",
            "yss_sci_runtime::hypothesis::run_hypothesis_test",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Execution,
        repository_relative_source_file: "src-tauri/crates/yss-graph-execution/examples/ols_bench.rs",
        fully_qualified_owner: "ols_bench",
        canonical_origin_targets: &["yss_sci_runtime::computation::ols"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/mod.rs",
        fully_qualified_owner: "yss_application::ipc",
        canonical_origin_targets: &["yss_application::ipc::runtime::CommandRuntime"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_activity_panel.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_activity_panel",
        canonical_origin_targets: &[
            "yss_application::ipc::activity_panel_sync::ActivityPanelSyncState",
            "yss_application::ipc::activity_panel_sync::ActivityPanelUpdateDto",
            "yss_application::activity_panel",
            "yss_application::activity_panel::commands_activity_panel",
            "yss_application::activity_panel::plugins_activity_panel",
            "yss_application::activity_panel::project_activity_panel",
            "yss_application::session::slot::ApplicationState",
            "yss_plugin_runtime::PluginManager",
            "yss_application::ipc::error::CommandError",
            "yss_application::ipc::schema::activity_panel::ActivityPanelDocumentDto",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId::Project",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId::Nodes",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId::Commands",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId::Plugins",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/activity_panel.rs",
        fully_qualified_owner: "yss_application::ipc::schema::activity_panel",
        canonical_origin_targets: &[
            "yss_application::activity_panel",
            "yss_application::activity_panel::ActivityPanelDocument",
            "yss_application::activity_panel::ActivityText",
            "yss_application::activity_panel::ActivityText::Key",
            "yss_application::activity_panel::ActivityText::Literal",
            "yss_application::activity_panel::ActivityTool",
            "yss_application::activity_panel::ActivityRowContent",
            "yss_application::activity_panel::ActivityRowContent::Category",
            "yss_application::activity_panel::ActivityRowContent::Item",
            "yss_application::activity_panel::ActivityRowContent::Message",
            "yss_application::activity_panel::ActivityItem",
            "yss_application::activity_panel::ActivityItem::Node",
            "yss_application::activity_panel::ActivityItem::Graph",
            "yss_application::activity_panel::ActivityItem::Chart",
            "yss_application::activity_panel::ActivityItem::Database",
            "yss_application::activity_panel::ActivityItem::Command",
            "yss_application::activity_panel::ActivityItem::Plugin",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "plugins/julia/native/crates/yss-julia-extension/src/commands.rs",
        fully_qualified_owner: "yss_julia_extension::commands",
        canonical_origin_targets: &[
            "yss_bayes_runtime::BayesApplicationError",
            "yss_bayes_runtime::BayesInferenceService",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "plugins/julia/native/crates/yss-julia-extension/src/main.rs",
        fully_qualified_owner: "yss_julia_extension",
        canonical_origin_targets: &[
            "yss_bayes_runtime::BayesInferenceService",
            "yss_bayes_runtime::required_input_columns",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::BackendAdapter,
        repository_relative_source_file: "plugins/julia/native/crates/yss-bayes-artifact-datafusion/src/plots.rs",
        fully_qualified_owner: "yss_bayes_artifact_datafusion::plots",
        canonical_origin_targets: &[
            "yss_bayes_artifact_contract::BayesArtifactReadError",
            "yss_bayes_artifact_contract::BayesArtifactReader",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::PureLeaf,
        repository_relative_source_file: "src-tauri/crates/yss-graph-document/src/model.rs",
        fully_qualified_owner: "yss_graph_document::model",
        canonical_origin_targets: &[
            "yss_node_protocol::identity::NodeTypeId",
            "yss_node_protocol::identity::ParameterKey",
            "yss_node_protocol::identity::PortKey",
            "yss_node_protocol::types::TypeExpr",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/src/lib.rs",
        fully_qualified_owner: "yssbi_lib",
        canonical_origin_targets: &[
            "yss_application::session::slot::ApplicationState",
            "yss_application::ipc::invoke_handler",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/runtime.rs",
        fully_qualified_owner: "yss_application::runtime",
        canonical_origin_targets: &[
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::ApplicationState::initialize",
            "yss_application::session::factory::ApplicationInitializationError",
            "yss_application::database::samples::SampleCatalog",
            "yss_application::database::samples::SampleCatalog::new",
            "yss_application::plugins::PluginHostServices",
            "yss_application::plugins::PluginHostServices::new",
            "yss_application::project::registry::ProjectManagement",
            "yss_application::project::registry::ProjectManagement::new",
            "yss_plugin_runtime::PluginManager",
            "yss_plugin_runtime::PluginManager::initialize",
            "yss_project_registry_sqlite::SqliteProjectRegistryStore::connect",
            "yss_filesystem::watcher::WatcherState",
            "yss_filesystem::watcher::WatcherState::new",
            "yss_filesystem::watcher::notify::NotifyFileWatcher::with_filter",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/runtime/harness.rs",
        fully_qualified_owner: "yss_application::runtime::harness",
        canonical_origin_targets: &[
            "yss_application::session::slot::ApplicationState",
            "yss_application::harness::HarnessInitializationError",
            "yss_harness_core::host::HarnessHost",
            "yss_harness_core::host::HarnessPorts",
            "yss_harness_rig::ConfigurableAgentDriver::new",
            "yss_harness_sqlite::SqliteHarnessStore::connect",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::CompositionRoot,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/runtime.rs",
        fully_qualified_owner: "yss_application::ipc::runtime",
        canonical_origin_targets: &[
            "yss_application::ipc::channel::graph_activity::GraphActivityChannels",
            "yss_application::ipc::channel::graph_activity::GraphActivityChannels::default",
            "yss_application::ipc::graph_editor_sync::GraphEditorSyncState",
            "yss_application::ipc::graph_editor_sync::GraphEditorSyncState::default",
            "yss_application::session::slot::ApplicationState",
            "yss_ipc_channel::harness::HarnessChannelHub",
            "yss_application::ipc::activity_panel_sync::ActivityPanelSyncState",
            "yss_application::ipc::activity_panel_sync::ActivityPanelSyncState::default",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_harness/gateway.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_harness::gateway",
        canonical_origin_targets: &["yss_application::session::slot::ApplicationState"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_harness.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_harness",
        canonical_origin_targets: &[
            "yss_application::harness::HarnessSessionError",
            "yss_application::harness::HarnessSessionError::SessionCapture",
            "yss_application::harness::HarnessSessionError::Host",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::harness::HarnessEventDto",
            "yss_ipc_contract::harness::HarnessMemoryRecordDto",
            "yss_ipc_contract::harness::HarnessRuntimeStatusDto",
            "yss_ipc_contract::harness::ConfigureHarnessProviderRequestDto",
            "yss_ipc_contract::harness::HarnessSessionDto",
            "yss_ipc_contract::harness::HarnessSubscriptionDto",
            "yss_ipc_contract::harness::HarnessTurnResultDto",
            "yss_ipc_contract::harness::WorkflowRunDto",
            "yss_application::session::slot::ApplicationState",
            "yss_harness_core::host::HarnessError",
            "yss_harness_core::host::HarnessHost",
            "yss_harness_core::host::HarnessHost::delete_session_memory",
            "yss_harness_core::host::HarnessHost::cancel_workflow",
            "yss_harness_core::host::HarnessHost::pause_workflow",
            "yss_harness_core::host::HarnessHost::resume_workflow",
            "yss_harness_core::host::HarnessHost::session_memory",
            "yss_harness_core::workflow::dataset_quality_review_workflow",
            "yss_ipc_channel::harness::HarnessChannelHub",
            "yss_ipc_contract::harness::HarnessMemoryRecordDto::from",
            "yss_ipc_contract::harness::HarnessSessionDto::from",
            "yss_ipc_contract::harness::WorkflowRunDto::from",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Application,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/session/factory.rs",
        fully_qualified_owner: "yss_application::session::factory",
        canonical_origin_targets: &[
            "yss_node_catalog::builtin::build_builtin_node_system",
            "yss_node_catalog::builtin::BuiltinInitializationError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Project,
        repository_relative_source_file: "src-tauri/crates/yss-project/src/project_activation.rs",
        fully_qualified_owner: "yss_project::project_activation",
        canonical_origin_targets: &[
            "yss_dataset_store::DatasetStore",
            "yss_dataset_store::DatasetStoreError",
            "yss_dataset_store::DatasetStore::open",
            "yss_dataset_store::DatasetStore::pending_publications",
            "yss_dataset_store::DatasetStore::acknowledge_publication",
            "yss_dataset_store::DatasetStore::collect_garbage",
            "yss_dataset_store::DatasetPublication",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Project,
        repository_relative_source_file: "src-tauri/crates/yss-project/src/project_lifecycle.rs",
        fully_qualified_owner: "yss_project::project_lifecycle",
        canonical_origin_targets: &[
            "yss_dataset_store::DatasetStore",
            "yss_dataset_store::DatasetStoreError",
            "yss_dataset_store::DatasetStore::open",
            "yss_dataset_store::DatasetStore::create",
            "yss_dataset_store::DatasetStore::catalog_snapshot",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Project,
        repository_relative_source_file: "src-tauri/crates/yss-project/src/project_io.rs",
        fully_qualified_owner: "yss_project::project_io",
        canonical_origin_targets: &[
            "yss_dataset_store::DatasetStore",
            "yss_dataset_store::DatasetStore::open",
            "yss_dataset_store::DatasetStore::catalog_metadata",
            "yss_dataset_store::DatasetMetadata",
            "yss_dataset_store::DatasetStoreError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_plugin.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_plugin",
        canonical_origin_targets: &[
            "yss_plugin_runtime::PluginManager",
            "yss_application::ipc::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_dataframe/error.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_dataframe::error",
        canonical_origin_targets: &[
            "yss_application::database::error::DatabaseOperationError",
            "yss_application::database::error::DatabaseApplicationOperation",
            "yss_application::ipc::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_dataframe/mod.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_dataframe",
        canonical_origin_targets: &[
            "yss_application::database::samples::SampleCatalog",
            "yss_application::database::samples::SampleError",
            "yss_application::database::samples::SampleError::NotFound",
            "yss_application::database::samples::SampleError::VersionMismatch",
            "yss_application::database::samples::SampleError::Integrity",
            "yss_application::database::samples::SampleError::InvalidCatalog",
            "yss_application::database::samples::SampleError::InvalidPath",
            "yss_application::database::samples::SampleError::CatalogDecode",
            "yss_application::database::samples::SampleError::Unavailable",
            "yss_application::database::samples::SampleError::Decode",
            "yss_application::database::samples::SampleImportError",
            "yss_application::database::samples::SampleImportError::Sample",
            "yss_application::database::samples::SampleImportError::Database",
            "yss_application::ipc::schema::database::SampleDatasetDto",
            "yss_application::ipc::schema::database::ColumnSemanticDto",
            "yss_application::database::DatabaseMutation",
            "yss_application::database::LoadDatabaseResult",
            "yss_application::database::error::DatabaseOperationError",
            "yss_application::database::error::DatabaseApplicationOperation::Delete",
            "yss_application::database::error::DatabaseApplicationOperation::Load",
            "yss_application::database::error::DatabaseApplicationOperation::Rename",
            "yss_application::database::error::DatabaseApplicationOperation::Save",
            "yss_application::database::list_excel_sheets",
            "yss_application::database::list_sql_tables",
            "yss_application::database::list_sqlite_tables",
            "yss_application::database::load_database",
            "yss_application::database::mutate_database_resource",
            "yss_application::database::read_column_distributions",
            "yss_application::database::read_column_statistics",
            "yss_application::database::read_database_edit_state",
            "yss_application::database::read_database_meta",
            "yss_application::database::read_database_rows",
            "yss_application::database::read_dataset_overview",
            "yss_application::database::rename_database",
            "yss_application::database::save_database_changes",
            "yss_application::database",
            "yss_application::database::DatabaseUseCaseError",
            "yss_application::database::DatabaseMetaResult",
            "yss_application::database::DatabaseMutationResult",
            "yss_application::database::DatabaseRowsResult",
            "yss_application::database::database",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::event::Event",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_event::emit_project_event_result",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_ipc_contract::project::ResourceMutationCommandResultDto",
            "yss_application::ipc::schema::application_event::resource_mutation_to_transport",
            "yss_application::ipc::schema::database::DatabaseImportSourceDTO",
            "yss_application::ipc::schema::database::DatabaseMetaResultDto",
            "yss_application::ipc::schema::database::DatabaseRowsResultDto",
            "yss_application::ipc::schema::database::LoadDatabaseResultDto",
            "yss_application::ipc::schema::database::column_info_from_schema",
            "yss_database_contract::EditState",
            "yss_ipc_contract::event::Event::Project",
            "yss_ipc_contract::event::event_project::EventProject::ResourceMutationCommitted",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_hypothesis.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_hypothesis",
        canonical_origin_targets: &[
            "yss_sci_contract::hypothesis::HypothesisTestInput",
            "yss_sci_runtime::hypothesis::run_hypothesis_test",
            "yss_application::ipc::schema::statistics::HypothesisTestResponseDto",
            "yss_application::ipc::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/catalog.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::catalog",
        canonical_origin_targets: &[
            "yss_application::ipc::schema::graph_editing::graph_edit_version_from_transport",
            "yss_ipc_contract::graph_editing::GraphEditVersionDto",
            "yss_application::graph::catalog::CatalogQueryApplicationError",
            "yss_application::graph::catalog::CompatibleCatalogRequest",
            "yss_application::graph::catalog::GraphCatalogQueryError",
            "yss_application::graph::catalog::GraphCatalogQueryError::Catalog",
            "yss_application::graph::catalog::GraphCatalogQueryError::CompatibleSourceInvalid",
            "yss_application::graph::catalog::GraphCatalogQueryError::GraphNotLoaded",
            "yss_application::graph::catalog::GraphCatalogQueryError::InvalidDraft",
            "yss_application::graph::catalog::GraphCatalogQueryError::Internal",
            "yss_application::graph::catalog::LocalizedCatalogRequest",
            "yss_application::graph::catalog::ProjectCatalogReadError",
            "yss_application::execution::ApplicationState",
            "yss_application::execution::SessionCaptureError",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::session::slot::SessionRevalidationError",
            "yss_application::session::slot::SessionRevalidationError::Changed",
            "yss_application::session::slot::SessionRevalidationError::Unavailable",
            "yss_application::ipc::error::CommandError",
            "yss_application::ipc::schema::catalog::LocalizedCatalogDto",
            "yss_ipc_contract::graph::PortAddressDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/common.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::common",
        canonical_origin_targets: &[
            "yss_project::project_state::graph_operation::ProjectGraphCommitError::InvalidDocument",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::error::GraphMutationErrorDetailsDto",
            "yss_ipc_contract::error::GraphMutationErrorDetailsDto::VALUE",
            "yss_ipc_contract::event::Event",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_ipc_contract::project::ResourceMutationResultDto",
            "yss_application::graph::resources::ResourceMutationApplicationError",
            "yss_application::graph::resources::ResourceMutationApplicationError::GraphOperation",
            "yss_application::graph::resources::ResourceMutationApplicationError::Resource",
            "yss_application::graph::resources::ResourceMutationApplicationError::Mutation",
            "yss_application::graph::resources::ResourceMutationApplicationError::Project",
            "yss_application::graph::resources::ResourceMutationApplicationError::SessionCapture",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
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
            "yss_project_history::ProjectResourceMutationError",
            "yss_project_history::ProjectResourceMutationError::Mutation",
            "yss_project_history::ProjectResourceMutationError::Projection",
            "yss_project_history::ProjectResourceMutationError::RecoveryRequired",
            "yss_project_history::ProjectResourceMutationError::ResourceMismatch",
            "yss_project_history::ProjectResourceMutationError::StaleProjectLifecycle",
            "yss_project_history::ProjectResourceMutationError::StaleRevision",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::GraphUnavailable",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::ProjectIdentityMismatch",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::RevisionConflict",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::OperationOwnershipChanged",
            "yss_project::project_state::graph_operation::ProjectGraphCommitError",
            "yss_project::project_state::graph_operation::ProjectGraphCommitError::LifecycleChanged",
            "yss_project::project_state::graph_operation::ProjectGraphCommitError::OperationOwnershipChanged",
            "yss_project::project_state::graph_operation::ProjectGraphCommitError::RevisionExhausted",
            "yss_project::project_state::graph_operation::ProjectGraphCommitError::StaleAuthority",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::AdmissionClosed",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::Internal",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::RecoveryRequired",
            "yss_project::project_state::graph_operation::ProjectGraphOperationError::ResourceLifecycleChanged",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/editor.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::editor",
        canonical_origin_targets: &[
            "yss_application::ipc::schema::graph_editing::graph_edit_receipt_to_transport",
            "yss_ipc_contract::graph_editing::GraphEditCommandReceiptDto",
            "yss_application::ipc::graph_editor_sync::Binding",
            "yss_application::ipc::graph_editor_sync::GraphEditorSyncState",
            "yss_application::ipc::schema::graph_editing::encode_graph_edit",
            "yss_application::ipc::schema::graph_editing::encode_graph_session",
            "yss_application::ipc::schema::graph_editing::graph_edit_version_from_transport",
            "yss_application::graph::editing::GraphEditRequest",
            "yss_application::ipc::channel::graph_activity::GraphActivityChannels",
            "yss_ipc_contract::graph_editing::GraphEditVersionDto",
            "yss_ipc_contract::graph_editing::GraphEditorSyncResponseDto",
            "yss_ipc_contract::graph_editing::GraphActivityDto",
            "yss_graph_execution::run_registry::RunState",
            "yss_graph_execution::run_registry::RunState::Admitted",
            "yss_graph_execution::run_registry::RunState::Running",
            "yss_graph_execution::run_registry::RunState::Finalizing",
            "yss_graph_execution::run_registry::RunState::Succeeded",
            "yss_graph_execution::run_registry::RunState::Failed",
            "yss_graph_execution::run_registry::RunState::Cancelled",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::graph::open::OpenGraphApplicationError",
            "yss_application::graph::open::OpenGraphApplicationError::Contract",
            "yss_application::graph::open::OpenGraphApplicationError::Database",
            "yss_application::graph::open::OpenGraphApplicationError::Materialization",
            "yss_application::graph::open::OpenGraphApplicationError::Project",
            "yss_application::graph::open::OpenGraphApplicationError::Projection",
            "yss_application::graph::open::OpenGraphApplicationError::SessionCapture",
            "yss_application::graph::open::OpenGraphApplicationError::SessionChanged",
            "yss_application::graph::open::OpenGraphRequest",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::event::Event",
            "yss_ipc_event::emit_project_event_result",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_application::graph::resources::ResourceMutationApplicationError",
            "yss_application::graph::resources::ResourceMutationApplicationError::GraphOperation",
            "yss_application::graph::resources::ResourceMutationApplicationError::Resource",
            "yss_application::graph::resources::ResourceMutationApplicationError::Mutation",
            "yss_application::graph::resources::ResourceMutationApplicationError::Project",
            "yss_application::graph::resources::ResourceMutationApplicationError::SessionCapture",
            "yss_application::ipc::schema::application_event::graph_delta_to_transport",
            "yss_application::ipc::schema::application_event::graph_mutation_to_transport",
            "yss_graph_editor::EditorGraphMutation",
            "yss_graph_editor::mutation::EditorGraphMutation",
            "yss_graph_document::NodeId",
            "yss_project_history::MutationRequest",
            "yss_application::ipc::schema::application_event::GraphMutationResultDto",
            "yss_application::ipc::schema::catalog::NodeCreationDescriptorDto",
            "yss_application::ipc::schema::editor_projection::map_editor_projection",
            "yss_ipc_contract::editor_projection::EditorGraphProjectionDto",
            "yss_application::ipc::schema::graph_clipboard::ClipboardSubgraphDto",
            "yss_ipc_contract::graph_editing::GraphEditorSessionDto",
            "yss_application::ipc::schema::graph_editing::graph_editor_session_to_transport",
            "yss_application::ipc::schema::graph_mutation::EditorGraphMutationDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/execution.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::execution",
        canonical_origin_targets: &[
            "yss_application::ipc::schema::graph_editing::graph_edit_version_from_transport",
            "yss_ipc_contract::graph_editing::GraphEditVersionDto",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
            "yss_application::session::slot::SessionRevalidationError",
            "yss_application::session::slot::SessionRevalidationError::Changed",
            "yss_application::session::slot::SessionRevalidationError::Unavailable",
            "yss_application::graph::run::CancelRunOutcome",
            "yss_application::graph::run::ExecutionApplicationError",
            "yss_application::graph::run::RunApplicationEvent",
            "yss_application::graph::run::RunApplicationEventKind::RunCancelled",
            "yss_application::graph::run::RunApplicationEventKind::RunCompleted",
            "yss_application::graph::run::RunApplicationEventKind::RunErrored",
            "yss_application::graph::run::RunGraphRequest",
            "yss_application::graph::run::run_graph_with_sink",
            "yss_application::graph::run::cancel_run",
            "yss_application::graph::preview_generation::allocate_pin_preview_generation",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::event::Event",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_project::execution_authority::ProjectExecutionPreparationError",
            "yss_project::execution_authority::ProjectExecutionPreparationError::DuplicateResourceRequirement",
            "yss_project::execution_authority::ProjectExecutionPreparationError::GraphUnavailable",
            "yss_project::execution_authority::ProjectExecutionPreparationError::InvalidGraph",
            "yss_project::execution_authority::ProjectExecutionPreparationError::InvalidResourceIdentity",
            "yss_project::execution_authority::ProjectExecutionPreparationError::ProjectIdentityMismatch",
            "yss_project::execution_authority::ProjectExecutionPreparationError::ResourceKindMismatch",
            "yss_project::execution_authority::ProjectExecutionPreparationError::ResourceRevisionUnavailable",
            "yss_project::execution_authority::ProjectExecutionPreparationError::ResourceUnavailable",
            "yss_project::execution_authority::ProjectExecutionPreparationError::UnsupportedResourceKind",
            "yss_graph_execution::error::RunFailureCode::DivisionByZero",
            "yss_graph_execution::error::RunFailureCode::InvalidNumericInput",
            "yss_graph_execution::error::RunFailureCode::KernelNotFound",
            "yss_graph_execution::error::RunFailureCode::NonFiniteResult",
            "yss_graph_execution::run_registry::RunId",
            "yss_graph_execution::run_registry::RunId::from_existing",
            "yss_graph_execution::state::ExecutePreparedError",
            "yss_graph_execution::state::ExecutePreparedError::Admission",
            "yss_graph_execution::state::ExecutePreparedError::Cancelled",
            "yss_graph_execution::state::ExecutePreparedError::DeadlineExceeded",
            "yss_graph_execution::state::ExecutePreparedError::Kernel",
            "yss_graph_execution::state::ExecutePreparedError::KernelUnavailable",
            "yss_graph_execution::state::ExecutePreparedError::ResourcePreparation",
            "yss_graph_execution::state::ExecutePreparedError::RunRegistry",
            "yss_graph_execution::state::ExecutePreparedError::RuntimeGenerationMismatch",
            "yss_application::ipc::channel::execution::TauriExecutionChannelAdapter",
            "yss_application::ipc::channel::execution::TauriExecutionChannelAdapter::new",
            "yss_ipc_contract::execution::RunEventDto",
            "yss_ipc_contract::execution::ExecutionDemandDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/history.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::history",
        canonical_origin_targets: &[
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
            "yss_application::graph::resources::ResourceMutationApplicationError",
            "yss_application::graph::resources::ResourceMutationApplicationError::Resource",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::event::Event",
            "yss_ipc_event::emit_project_event_result",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_contract::event::event_project::EventProject::ResourceMutationCommitted",
            "yss_application::ipc::schema::application_event::resource_mutation_to_transport",
            "yss_ipc_contract::project::ResourceMutationResultDto",
            "yss_project_history::MutationRequest",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/resources.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::resources",
        canonical_origin_targets: &[
            "yss_application::ipc::graph_editor_sync::Binding",
            "yss_application::ipc::graph_editor_sync::GraphEditorSyncState",
            "yss_application::ipc::schema::graph_editing::encode_graph_edit",
            "yss_application::ipc::schema::graph_editing::graph_edit_version_from_transport",
            "yss_application::graph::editing::GraphEditRequest",
            "yss_ipc_contract::graph_editing::GraphEditVersionDto",
            "yss_ipc_contract::graph_editing::GraphEditorSyncResponseDto",
            "yss_application::session::slot::ApplicationState",
            "yss_application::graph::resources::ResourceMutationApplicationError",
            "yss_application::graph::resources::ResourceMutationApplicationError::GraphOperation",
            "yss_application::graph::resources::ResourceMutationApplicationError::Resource",
            "yss_application::graph::resources::ResourceMutationApplicationError::Mutation",
            "yss_application::graph::resources::ResourceMutationApplicationError::Project",
            "yss_application::graph::resources::ResourceMutationApplicationError::SessionCapture",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::event::Event",
            "yss_ipc_event::emit_project_event_result",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_project_history::MutationRequest",
            "yss_project::project_writers::ProjectSaveResult",
            "yss_ipc_contract::project::ResourceMutationResultDto",
            "yss_application::ipc::schema::application_event::resource_mutation_to_transport",
            "yss_ipc_contract::project::ProjectSaveResultDto",
            "yss_project_history::FunctionDocumentPatch",
            "yss_ipc_contract::event::Event::Project",
            "yss_ipc_contract::event::event_project::EventProject::ResourceMutationCommitted",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/results.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::results",
        canonical_origin_targets: &[
            "yss_application::ipc::schema::result::graph_result_state_to_dto",
            "yss_ipc_contract::execution::GraphResultStateDto",
            "yss_application::ipc::schema::result::ResultReferenceDto",
            "yss_application::ipc::schema::result::LinearRegressionReportDto::from",
            "yss_graph_execution::result::ResultReference",
            "yss_graph_execution::result::ResultRetentionError",
            "yss_application::graph::results::ResultPinQuery",
            "yss_application::graph::results::ResultQueryApplicationError",
            "yss_application::graph::results::ResultQueryApplicationError::SessionCapture",
            "yss_application::graph::results::ResultQueryApplicationError::SessionChanged",
            "yss_application::graph::results::ResultQueryApplicationError::InvalidPageRequest",
            "yss_application::graph::results::ResultQueryApplicationError::PageTooLarge",
            "yss_application::graph::results::ResultQueryApplicationError::Relation",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
            "yss_application::ipc::commands::execution_dto::ResultDescriptorDto",
            "yss_application::ipc::commands::execution_dto::ResultPageDto",
            "yss_application::ipc::commands::execution_dto::ResultValueDto",
            "yss_application::ipc::commands::execution_dto::runtime_value_to_json",
            "yss_application::ipc::error::CommandError",
            "yss_graph_execution::result::ResultId",
            "yss_node_kernel::value::RuntimeValue",
            "yss_ipc_contract::graph::PortAddressDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/leases.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::leases",
        canonical_origin_targets: &[
            "yss_application::ipc::error::CommandError",
            "yss_application::ipc::schema::result::ResultReferenceDto",
            "yss_application::session::slot::ApplicationState",
            "yss_graph_execution::result::StoredResultSnapshot",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_node_system/reports.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_node_system::reports",
        canonical_origin_targets: &[
            "yss_application::ipc::error::CommandError",
            "yss_application::ipc::schema::result::ResultReferenceDto",
            "yss_application::ipc::schema::result::ResultTablePartDto",
            "yss_application::ipc::schema::result::ResultAnalysisRequestDto",
            "yss_application::ipc::schema::result::ResultAnalysisResponseDto",
            "yss_application::session::slot::ApplicationState",
            "yss_application::graph::results::report::ReportQueryError",
            "yss_sci_contract::hypothesis::HypothesisError::InvalidInput",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/result.rs",
        fully_qualified_owner: "yss_application::ipc::schema::result",
        canonical_origin_targets: &[
            "yss_application::graph::results::GraphResultState",
            "yss_application::graph::results::report::LinearRegressionReportProjection",
            "yss_application::graph::results::report::ResultAnalysisProjection",
            "yss_application::graph::results::report::ResultAnalysisRequest",
            "yss_application::graph::results::report::ResultTablePart",
            "yss_graph_execution::identity::ExecutionSessionId",
            "yss_graph_execution::result::ResultCacheState",
            "yss_graph_execution::result::ConnectionCacheState",
            "yss_graph_execution::result::ResultId",
            "yss_graph_execution::result::ResultReference",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/statistics.rs",
        fully_qualified_owner: "yss_application::ipc::schema::statistics",
        canonical_origin_targets: &["yss_sci_contract::hypothesis::HypothesisTestOutput"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/execution_dto.rs",
        fully_qualified_owner: "yss_application::ipc::commands::execution_dto",
        canonical_origin_targets: &[
            "yss_application::graph::run::RunApplicationEvent",
            "yss_application::graph::run::RunApplicationEventKind",
            "yss_application::graph::run::RunDemand",
            "yss_application::graph::results::ResultPageProjection",
            "yss_application::graph::results::ResultPageKind",
            "yss_application::graph::results::ResultPageKind::Scalar",
            "yss_application::graph::results::ResultPageKind::Sequence",
            "yss_graph_execution::error::RunFailure",
            "yss_graph_execution::error::RunFailureCode",
            "yss_graph_execution::error::RunPhase",
            "yss_graph_execution::plan::identity::PlanGraphId",
            "yss_graph_execution::plan::identity::PlanOutputRef",
            "yss_graph_execution::plan::identity::PlanPortAddress",
            "yss_graph_execution::plan::identity::PlanSourceIdentity",
            "yss_graph_execution::result::ResultId",
            "yss_graph_execution::result::StoredResult",
            "yss_graph_execution::result::StoredResultSnapshot",
            "yss_node_kernel::value::RuntimeValue",
            "yss_ipc_contract::graph::PortAddressDto",
            "yss_graph_execution::plan::result_category::PlotDataKind::Scatter",
            "yss_graph_execution::plan::result_category::PlotDataKind::Line",
            "yss_graph_execution::plan::result_category::PlotDataKind::Plot",
            "yss_graph_execution::plan::result_category::PlotDataKind::Ecdf",
            "yss_graph_execution::plan::result_category::PlotDataKind::Kde",
            "yss_graph_execution::plan::result_category::PlotDataKind::Histogram",
            "yss_graph_execution::plan::result_category::PlotDataKind::Correlation",
            "yss_graph_execution::plan::result_category::PlotDataKind::Correlogram",
            "yss_graph_execution::plan::result_category::ResultCategory",
            "yss_graph_execution::plan::result_category::ResultCategory::Value",
            "yss_graph_execution::plan::result_category::ResultCategory::PlotData",
            "yss_graph_execution::plan::result_category::ResultCategory::StatisticalReport",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::LinearRegressionSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::BinarySummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::Iv2slsSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::IvLimlSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::PraisSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::VarSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::VarSoc",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::PanelSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::PanelDid",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::DfAdfSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::DfAdfSummaryList",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::VecSummary",
            "yss_graph_execution::plan::result_category::StatisticalReportKind::VecRankSummary",
            "yss_ipc_contract::execution::GraphOutputRefDto",
            "yss_ipc_contract::execution::ExecutionDemandDto",
            "yss_ipc_contract::execution::ExecutionDemandDto::Default",
            "yss_ipc_contract::execution::ExecutionDemandDto::Outputs",
            "yss_ipc_contract::execution::ExecutionDemandDto::PinPreview",
            "yss_ipc_contract::execution::MAX_SAFE_PREVIEW_GENERATION",
            "yss_application::ipc::channel::execution::RunEventDtoError",
            "yss_application::ipc::channel::execution::RunEventDtoError::InvalidOutput",
            "yss_application::ipc::channel::execution::output_dto",
            "yss_ipc_contract::graph::PortAddressDto::Declared",
            "yss_ipc_contract::graph::PortAddressDto::Instance",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_panel_did.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_panel_did",
        canonical_origin_targets: &[
            "yss_application::ipc::error::CommandError",
            "yss_sci_contract::panel::ComputeDidFakeGroupRequest",
            "yss_sci_contract::panel::DidPlaceboFakeGroupBlock",
            "yss_sci_runtime::panel::did::compute_fake_group_ri",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_parse_at.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_parse_at",
        canonical_origin_targets: &[
            "yss_sci_runtime::hypothesis::parse_at_values",
            "yss_application::ipc::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/project_failure.rs",
        fully_qualified_owner: "yss_application::ipc::commands::project_failure",
        canonical_origin_targets: &[
            "yss_application::project::failure::ApplicationProjectFailure",
            "yss_application::ipc::error::CommandError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_project/lifecycle.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_project::lifecycle",
        canonical_origin_targets: &[
            "yss_application::project::change::ApplicationProjectWatchError",
            "yss_application::project::change::ApplicationProjectWatchError::ProjectIdentityMismatch",
            "yss_application::project::change::ApplicationProjectWatchError::Reconciliation",
            "yss_application::project::change::ApplicationProjectWatchError::SessionCapture",
            "yss_application::project::change::ApplicationProjectWatchError::SessionChanged",
            "yss_filesystem::watcher::WatcherError",
            "yss_filesystem::watcher::WatcherState",
            "yss_filesystem::watcher::ObservedChange",
            "yss_filesystem::watcher::ChangeSink",
            "yss_project::filesystem::project_root_from_path",
            "yss_ipc_contract::event::event_resource::EventResource",
            "yss_application::project::registry::ProjectManagement",
            "yss_ipc_contract::project::ProjectSaveResultDto",
            "yss_application::project::lifecycle::ApplicationProjectLifecycleError",
            "yss_application::project::lifecycle::ApplicationProjectLifecycleError::Lifecycle",
            "yss_application::project::lifecycle::ApplicationProjectLifecycleError::SessionCapture",
            "yss_application::project::lifecycle::ApplicationProjectLifecycleError::SessionChanged",
            "yss_application::project::lifecycle::ApplicationProjectLifecycleError::SessionRefresh",
            "yss_application::events::ProjectLifecycleOutcome::Committed",
            "yss_application::ipc::schema::application_event::project_activation_to_transport",
            "yss_application::ipc::schema::application_event::project_lifecycle_to_transport",
            "yss_ipc_contract::event::Event",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_event::emit_project_event_result",
            "yss_application::session::slot::ApplicationState",
            "yss_application::project::lifecycle::ProjectLifecycleError",
            "yss_application::project::lifecycle::clear_project",
            "yss_application::project::lifecycle::create_project",
            "yss_application::project::lifecycle::load_project",
            "yss_application::project::lifecycle::save_project_as",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::project::LifecycleMutationOutcomeDto",
            "yss_ipc_contract::project::LifecycleMutationResultDto",
            "yss_ipc_contract::project::ProjectActivationResultDto",
            "yss_application::ipc::schema::project::project_save_to_transport",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_project/path.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_project::path",
        canonical_origin_targets: &[
            "yss_application::ipc::error::CommandError",
            "yss_project_registry::default_project_parent_directory",
            "yss_project_registry::validate_new_project_path",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_project/query.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_project::query",
        canonical_origin_targets: &[
            "yss_application::ipc::graph_editor_sync::Binding",
            "yss_application::ipc::graph_editor_sync::GraphEditorSyncState",
            "yss_ipc_contract::graph_editing::GraphEditorSyncResponseDto",
            "yss_application::ipc::schema::graph_editing::encode_graph_session",
            "yss_application::ipc::activity_panel_sync::ActivityPanelSyncState",
            "yss_application::ipc::activity_panel_sync::ActivityPanelUpdateDto",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId",
            "yss_application::ipc::schema::activity_panel::ActivityPanelId::Nodes",
            "yss_application::ipc::schema::activity_panel::ActivityPanelRequest",
            "yss_application::ipc::schema::activity_panel::ActivityPanelDocumentDto",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::graph::open::OpenGraphApplicationError",
            "yss_application::graph::open::OpenGraphRequest",
            "yss_application::project::query::ProjectDatabasesSnapshot",
            "yss_application::project::query::ProjectQueryApplicationError",
            "yss_application::project::query::ProjectActivation",
            "yss_application::project::query::query_project_databases",
            "yss_application::ipc::error::CommandError",
            "yss_project::ProjectIndex",
            "yss_project::project_io::ProjectIndex",
            "yss_project_registry::normalize_existing_path",
            "yss_project::project_state::state::ProjectState",
            "yss_project::resource_reveal::RevealProjectResourceRequest",
            "yss_project::resource_reveal::resolve_reveal_path",
            "yss_ipc_contract::project::ProjectActivationResultDto",
            "yss_application::ipc::schema::database::DatabaseDeclDTO",
            "yss_application::ipc::schema::database::column_info_from_schema",
            "yss_ipc_contract::editor_projection::EditorGraphProjectionDto",
            "yss_application::ipc::schema::editor_projection::map_editor_projection",
            "yss_ipc_contract::graph_editing::GraphEditorSessionDto",
            "yss_application::ipc::schema::graph_editing::graph_editor_session_to_transport",
            "yss_application::ipc::schema::project::ProjectDatabasesDTO",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_project/registry.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_project::registry",
        canonical_origin_targets: &[
            "yss_application::project::lifecycle::delete_registered_project",
            "yss_application::ipc::error::CommandError",
            "yss_project_registry::CleanupInvalidProjectsResult",
            "yss_application::project::registry::ProjectManagement",
            "yss_project_registry::ScanProjectsResult",
            "yss_project::project_state::state::ProjectState",
            "yss_ipc_contract::project::LifecycleMutationResultDto",
            "yss_application::session::slot::ApplicationState",
            "yss_application::ipc::schema::application_event::project_lifecycle_to_transport",
            "yss_ipc_channel::project_progress::ProgressAdapterShutdownControl",
            "yss_ipc_channel::project_progress::ProgressAdapterShutdownControl::new",
            "yss_ipc_channel::project_progress::ProjectProgressAdapterSpawnError",
            "yss_ipc_channel::project_progress::ProjectProgressDrainOutcome",
            "yss_ipc_channel::project_progress::ProjectProgressDrainOutcome::TimedOut",
            "yss_ipc_channel::project_progress::ProjectProgressDrainOutcome::Drained",
            "yss_ipc_contract::project_progress::ProjectProgressDto",
            "yss_ipc_channel::project_progress::bounded_project_progress_adapter",
            "yss_ipc_channel::project_progress::reap_project_progress_worker",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_sci.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_sci",
        canonical_origin_targets: &[
            "yss_application::execution::ApplicationState",
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
            "yss_sci_contract::scientific::AcfPacfResult",
            "yss_sci_contract::scientific::AcfPacfRequest",
            "yss_sci_contract::scientific::ScientificCancellationToken",
            "yss_sci_contract::scientific::ScientificCancellationToken::new",
            "yss_sci_contract::scientific::ScientificExecutionControl",
            "yss_sci_runtime::computation::acf_pacf",
            "yss_sci_contract::scientific::ScientificComputationError",
            "yss_sci_contract::scientific::ScientificComputationError::Cancelled",
            "yss_sci_contract::scientific::ScientificComputationError::ComputationFailed",
            "yss_sci_contract::scientific::ScientificComputationError::DeadlineExceeded",
            "yss_sci_contract::scientific::ScientificComputationError::InvalidInput",
            "yss_application::ipc::error::CommandError",
            "yss_application::ipc::schema::statistics::AcfPacfRequestDto",
            "yss_application::ipc::schema::statistics::AcfPacfResponseDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_serial_tests.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_serial_tests",
        canonical_origin_targets: &[
            "yss_sci_contract::serial_tests::SerialTestsInput",
            "yss_sci_runtime::time_series::serial_tests::compute_serial_tests",
            "yss_application::session::slot::ApplicationState",
            "yss_application::ipc::error::CommandError",
            "yss_application::ipc::schema::statistics::DurbinWatsonResultDto",
            "yss_application::ipc::schema::statistics::SerialTestWithLagDto",
            "yss_application::ipc::schema::statistics::SerialTestsRequestDto",
            "yss_application::ipc::schema::statistics::SerialTestsResponseDto",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Commands,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/commands/command_chart.rs",
        fully_qualified_owner: "yss_application::ipc::commands::command_chart",
        canonical_origin_targets: &[
            "yss_application::session::slot::ApplicationState",
            "yss_application::session::slot::SessionCaptureError",
            "yss_application::session::slot::SessionCaptureError::Inactive",
            "yss_application::session::slot::SessionCaptureError::Recovering",
            "yss_application::session::slot::SessionCaptureError::Replacing",
            "yss_application::chart::resources::ChartApplicationError",
            "yss_application::chart::query::ChartPlotApplicationError",
            "yss_application::chart::query::ChartPlotQuery",
            "yss_application::chart::projection::PlotAxisFormat",
            "yss_application::chart::projection::PlotAxisFormat::Date",
            "yss_application::chart::projection::PlotAxisFormat::Datetime",
            "yss_application::chart::projection::PlotAxisFormat::Number",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::AdmissionClosed",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::ColumnMaterializationFailed",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::DatabaseNotFound",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::GenerationMismatch",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::RuntimeRevisionMismatch",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::SchemaRevisionMismatch",
            "yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::SessionMismatch",
            "yss_application::ipc::error::CommandError",
            "yss_ipc_contract::event::Event",
            "yss_ipc_event::emit_project_event_result",
            "yss_ipc_event::emit_project_event",
            "yss_ipc_contract::event::event_project::EventProject",
            "yss_project::project_state::state::ProjectState",
            "yss_resource_naming::ResourceName",
            "yss_ipc_contract::project::ResourceMutationResultDto",
            "yss_application::ipc::schema::application_event::resource_mutation_to_transport",
            "yss_ipc_contract::event::Event::Project",
            "yss_ipc_contract::event::event_project::EventProject::ResourceMutationCommitted",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/database.rs",
        fully_qualified_owner: "yss_application::ipc::schema::database",
        canonical_origin_targets: &["yss_application::database::samples::SampleDataset"],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/error/mod.rs",
        fully_qualified_owner: "yss_application::ipc::error",
        canonical_origin_targets: &[
            "yss_project::project_error::ProjectError",
            "yss_project::database_authority::ProjectDatabaseError",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/application_event.rs",
        fully_qualified_owner: "yss_application::ipc::schema::application_event",
        canonical_origin_targets: &[
            "yss_application::events::CommittedResourceMutation",
            "yss_application::events::GraphProjectionReplacement",
            "yss_application::events::LifecycleInvalidation",
            "yss_application::events::LifecycleRecovery",
            "yss_application::events::LifecycleRecoveryAction",
            "yss_application::events::ProjectLifecycleApplicationEvent",
            "yss_application::events::ProjectLifecycleKind",
            "yss_application::events::ProjectLifecycleOutcome",
            "yss_application::events::ProjectLifecyclePhase",
            "yss_application::events::ResourceMove",
            "yss_application::events::ResourceProjectionStatus",
            "yss_application::project::query::ProjectActivation",
            "yss_graph_document::GraphDocumentPatch",
            "yss_graph_document::change::GraphDocumentPatch",
            "yss_project_history::ResourceDeltaEvent",
            "yss_project_history::ResourceKey",
            "yss_project_history::ResourceLifecycleKind",
            "yss_function_editor_projection::FunctionEditorProjection",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/catalog.rs",
        fully_qualified_owner: "yss_application::ipc::schema::catalog",
        canonical_origin_targets: &[
            "yss_application::graph::catalog::CatalogQueryResult",
            "yss_node_catalog::localization::CatalogResourcePath",
            "yss_node_catalog::localization::CatalogResourcePath::new",
            "yss_node_catalog::localization::LocalizedCatalog",
            "yss_node_catalog::localization::LocalizedCatalogItem",
            "yss_node_catalog::localization::LocalizedCategory",
            "yss_node_catalog::localization::LocalizedParameter",
            "yss_node_catalog::localization::LocalizedPort",
            "yss_node_catalog::localization::NodeCreation",
            "yss_node_catalog::localization::NodeCreation::ParameterizedStatic",
            "yss_node_catalog::localization::NodeCreation::ResourceBound",
            "yss_node_catalog::localization::NodeCreation::Static",
            "yss_node_catalog::localization::ResourceBoundCreateArgs",
            "yss_node_catalog::localization::ResourceBoundCreateArgs::Database",
            "yss_node_catalog::localization::ResourceBoundCreateArgs::Function",
            "yss_node_protocol::identity::NodeTypeId",
            "yss_node_protocol::identity::NodeTypeId::new",
            "yss_node_protocol::identity::ParameterKey",
            "yss_node_protocol::identity::ParameterKey::new",
            "yss_node_protocol::parameter::ParameterKey",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/editor_projection.rs",
        fully_qualified_owner: "yss_application::ipc::schema::editor_projection",
        canonical_origin_targets: &[
            "yss_graph_editor::projection::EditorConnectionModel",
            "yss_graph_editor::projection::EditorDiagnosticModel",
            "yss_graph_editor::projection::EditorInputModel",
            "yss_graph_editor::projection::EditorNodeCapabilities",
            "yss_graph_editor::projection::EditorNodeDisplay",
            "yss_graph_editor::projection::EditorNodeModel",
            "yss_graph_editor::projection::EditorParameterModel",
            "yss_graph_editor::projection::EditorPortConnectionCapabilities",
            "yss_graph_editor::projection::EditorPortDisplay",
            "yss_graph_editor::projection::EditorPortModel",
            "yss_graph_editor::projection::EditorProjectionModel",
            "yss_graph_editor::projection::EditorProjectionOutcome",
            "yss_graph_editor::projection::EditorProjectionStage",
            "yss_graph_editor::projection::ParameterEditorKind",
            "yss_graph_editor::projection::ParameterValueSource",
            "yss_graph_editor::projection::ResolvedSchemaModel",
            "yss_graph_editor::projection::ResolvedTypeModel",
            "yss_graph_editor::projection::model::EditorConnectionModel",
            "yss_graph_editor::projection::model::EditorDiagnosticModel",
            "yss_graph_editor::projection::model::EditorDiagnosticSeverity",
            "yss_graph_editor::projection::model::EditorEffectiveInputBinding",
            "yss_graph_editor::projection::model::EditorFilterLiteralType",
            "yss_graph_editor::projection::model::EditorInputModel",
            "yss_graph_editor::projection::model::EditorNodeCapabilities",
            "yss_graph_editor::projection::model::EditorNodeDisplay",
            "yss_graph_editor::projection::model::EditorNodeModel",
            "yss_graph_editor::projection::model::EditorParameterConfiguration",
            "yss_graph_editor::projection::model::EditorParameterModel",
            "yss_graph_editor::projection::model::EditorPortConnectionCapabilities",
            "yss_graph_editor::projection::model::EditorPortDisplay",
            "yss_graph_editor::projection::model::EditorPortInstanceAdditionModel",
            "yss_graph_editor::projection::model::EditorPortModel",
            "yss_graph_editor::projection::model::EditorPortStatus",
            "yss_graph_editor::projection::model::EditorPortTypeState",
            "yss_graph_editor::projection::model::EditorProjectionModel",
            "yss_graph_editor::projection::model::EditorSchemaSummary",
            "yss_graph_editor::projection::model::EditorSchemaSummaryKind",
            "yss_graph_editor::projection::model::EditorResolutionOutcome",
            "yss_graph_editor::projection::model::EditorResolutionStage",
            "yss_graph_editor::projection::model::ParameterEditorKind",
            "yss_graph_analysis::GraphDiagnosticLocation",
            "yss_graph_analysis_contract::DiagnosticLocation",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Connection",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Graph",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Node",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Parameter",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Port",
            "yss_graph_analysis_contract::diagnostic::DiagnosticLocation::Resource",
            "yss_node_registry::fingerprint::RegistryFingerprint",
            "yss_node_protocol::PortDirection",
            "yss_node_protocol::PortKey",
            "yss_node_protocol::TypeExpr",
            "yss_node_protocol::model::PortDirection",
            "yss_node_protocol::model::PortDirection::Input",
            "yss_node_protocol::model::PortDirection::Output",
            "yss_node_protocol::types::TypeExpr",
            "yss_node_registry::RegistryFingerprint",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-ipc-contract/src/editor_projection.rs",
        fully_qualified_owner: "yss_ipc_contract::editor_projection",
        canonical_origin_targets: &[
            "yss_graph_analysis_contract::ResourceVersionSet",
            "yss_graph_analysis_contract::basis::ResourceVersionSet",
            "yss_graph_analysis_contract::basis::ResourceObservationSet",
            "yss_node_protocol::dataframe::FilterOperator",
            "yss_node_protocol::parameter::ParameterPresentation",
            "yss_node_protocol::types::TypeExpr",
            "yss_node_protocol::model::ParameterPresentation",
            "yss_node_registry::RegistryFingerprint",
            "yss_node_registry::fingerprint::RegistryFingerprint",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/graph_clipboard.rs",
        fully_qualified_owner: "yss_application::ipc::schema::graph_clipboard",
        canonical_origin_targets: &[
            "yss_node_catalog::localization::ResourceBoundCreateArgs",
            "yss_node_catalog::localization::ResourceBoundCreateArgs::Database",
            "yss_node_catalog::localization::ResourceBoundCreateArgs::Function",
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
            "yss_node_protocol::parameter::ParameterValues",
            "yss_node_protocol::types::TypeExpr",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/graph_editing.rs",
        fully_qualified_owner: "yss_application::ipc::schema::graph_editing",
        canonical_origin_targets: &[
            "yss_application::graph::editing::GraphEditResponse",
            "yss_project::project_state::graph_editing::GraphEditVersion",
            "yss_project::project_state::graph_editing::GraphEditingState",
            "yss_project::project_state::graph_editing::GraphEditCommandKind",
            "yss_project::project_state::graph_editing::GraphEditCommandReceipt",
            "yss_graph_editor::projection::model::EditorProjectionModel",
            "yss_application::graph::edit::GraphDraftSave",
            "yss_application::graph::edit::GraphDocumentChange",
            "yss_graph_document::change::GraphDocumentPatch",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/graph_mutation.rs",
        fully_qualified_owner: "yss_application::ipc::schema::graph_mutation",
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
            "yss_graph_editor::mutation::EditorGraphMutation::MovePortInstance",
            "yss_graph_editor::mutation::PortPlacement",
            "yss_graph_editor::mutation::EditorGraphMutation::RemovePortInstance",
            "yss_graph_editor::mutation::EditorGraphMutation::SetLiteral",
            "yss_graph_editor::mutation::EditorGraphMutation::SetConfiguration",
            "yss_graph_editor::mutation::EditorGraphMutation::SetConstant",
            "yss_graph_editor::mutation::EditorGraphMutation::InsertConstantReference",
            "yss_graph_editor::mutation::EditorGraphMutation::SetParameters",
            "yss_graph_editor::NodePositionMutation",
            "yss_graph_editor::mutation::NodePositionMutation",
            "yss_node_protocol::identity::PortKey",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/schema/project.rs",
        fully_qualified_owner: "yss_application::ipc::schema::project",
        canonical_origin_targets: &[
            "yss_project_history::ResourceKey",
            "yss_project::project_writers::ProjectSaveResult",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-application/src/ipc/channel/execution.rs",
        fully_qualified_owner: "yss_application::ipc::channel::execution",
        canonical_origin_targets: &[
            "yss_application::graph::run::RunApplicationEvent",
            "yss_application::graph::run::RunApplicationEventKind",
            "yss_graph_execution::error::RunFailure",
            "yss_graph_execution::error::RunFailureCode",
            "yss_graph_execution::error::RunPhase",
            "yss_graph_execution::plan::identity::PlanOutputRef",
            "yss_graph_execution::plan::identity::PlanPortAddress",
            "yss_graph_execution::plan::identity::PlanSourceIdentity",
        ],
    },
    InternalDependencyCapability {
        source_layer: RustLayer::Transport,
        repository_relative_source_file: "src-tauri/crates/yss-ipc-contract/src/project.rs",
        fully_qualified_owner: "yss_ipc_contract::project",
        canonical_origin_targets: &[
            "yss_project_history::ResourceDeltaEvent",
            "yss_project_history::ResourceKey",
            "yss_project_history::ResourceLifecycleKind",
            "yss_function_editor_projection::FunctionEditorProjection",
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
        "yss-bayes-artifact-contract"
            | "yss-harness-contract"
            | "yss-plugin-protocol"
            | "yss-bayes-model"
            | "yss-bayes-result"
            | "yss-bayes-worker"
            | "yss-canonical-hash"
            | "yss-data-contract"
            | "yss-database-contract"
            | "yss-database-schema"
            | "yss-display-naming"
            | "yss-graph-document"
            | "yss-graph-resource-contract"
            | "yss-graph-type-mapping"
            | "yss-math-expr"
            | "yss-sci-linalg"
            | "yss-project-identity"
            | "yss-project-layout"
            | "yss-project-progress"
            | "yss-project-registry-contract"
            | "yss-resource-naming"
            | "yss-sci-contract"
            | "yss-relational-contract"
            | "yss-tabular-contract"
            | "yss-chart-document"
    ) {
        layers.insert(RustLayer::PureLeaf);
    } else if package == "yss-filesystem"
        && source_file.starts_with("src-tauri/crates/yss-filesystem/src/")
    {
        layers.insert(RustLayer::Filesystem);
    } else if package == "yss-application" {
        if let Some(layer) = exact_layer {
            layers.insert(layer);
        } else if namespace == "ipc" {
            layers.insert(RustLayer::Commands);
        } else if let Some(ipc_owner) = namespace.strip_prefix("ipc::") {
            match ipc_owner.split("::").next().unwrap_or_default() {
                "commands" => {
                    layers.insert(RustLayer::Commands);
                }
                "schema" | "error" | "activity_panel_sync" | "channel" => {
                    layers.insert(RustLayer::Transport);
                }
                _ => {}
            }
        } else {
            layers.insert(RustLayer::Application);
        }
    } else if matches!(package, "yss-harness-core" | "yss-bayes-runtime") {
        layers.insert(RustLayer::Application);
    } else if matches!(
        package,
        "yss-project" | "yss-project-history" | "yss-project-model"
    ) || package == "yss-function-editor-projection"
        || package == "yss-project-operation"
        || package == "yss-project-registry"
        || package == "yss-resource-lifecycle"
    {
        layers.insert(RustLayer::Project);
    } else if matches!(
        package,
        "yss-node-protocol" | "yss-node-registry" | "yss-node-catalog"
    ) {
        layers.insert(RustLayer::Node);
    } else if matches!(
        package,
        "yss-graph-analysis"
            | "yss-graph-analysis-contract"
            | "yss-graph-diagnostics"
            | "yss-graph-document-edit"
            | "yss-graph-editor"
            | "yss-graph-runtime"
    ) {
        layers.insert(RustLayer::Graph);
    } else if matches!(
        package,
        "yss-database-runtime"
            | "yss-dataset-profile"
            | "yss-dataset-store"
            | "yss-sql-source"
            | "yss-tabular-io"
            | "yss-tabular-arrow"
            | "yss-datafusion"
    ) {
        layers.insert(RustLayer::DatabaseCore);
    } else if matches!(package, "yss-sci" | "yss-sci-runtime") {
        layers.insert(RustLayer::SciCore);
    } else if matches!(
        package,
        "yss-ipc-contract" | "yss-ipc-event" | "yss-ipc-channel"
    ) {
        layers.insert(RustLayer::Transport);
    } else if package == "yss-node-kernel" {
        layers.insert(RustLayer::NodeKernel);
    } else if package == "yss-graph-execution" {
        layers.insert(RustLayer::Execution);
    } else if package == "tauri-plugin-tracing"
        && source_file.starts_with("src-tauri/crates/tauri-plugin-tracing/src/collector/")
    {
        layers.insert(RustLayer::Logging);
    } else if package == "tauri-plugin-tracing"
        && source_file.starts_with("src-tauri/crates/tauri-plugin-tracing/src/")
    {
        layers.insert(RustLayer::PlatformAdapter);
    } else if matches!(
        package,
        "yss-bayes-artifact-datafusion"
            | "yss-harness-rig"
            | "yss-bayes-worker-julia"
            | "yss-julia-runtime"
            | "yss-julia-worker"
            | "yss-julia-extension"
            | "yss-plugin-runtime"
            | "yss-plugin-sdk"
            | "yss-project-registry-sqlite"
            | "yss-harness-sqlite"
    ) {
        layers.insert(RustLayer::BackendAdapter);
    } else if let Some(layer) = cohesive_owner_layer(namespace) {
        layers.insert(layer);
    }
    layers
}

fn exact_source_layer(source_file: &str) -> Option<RustLayer> {
    EXACT_SOURCE_MEMBERSHIP
        .iter()
        .find_map(|(source, layer)| (*source == source_file).then_some(*layer))
}

fn cohesive_owner_layer(namespace: &str) -> Option<RustLayer> {
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
        "graph" => Some(RustLayer::Graph),
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
                package_name,
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
            let crosses_scientific_boundary = (package_name == "yss-sci-linalg"
                && !matches!(
                    dependency.owning_package.as_str(),
                    "yss-sci-linalg" | "yss-sci"
                ))
                || (package_name == "yss-sci"
                    && !matches!(
                        dependency.owning_package.as_str(),
                        "yss-sci" | "yss-sci-runtime"
                    ))
                || (package_name == "yss-sci-runtime"
                    && !(matches!(
                        dependency.owning_package.as_str(),
                        "yss-sci-runtime" | "yss-graph-execution" | "yss-node-kernel"
                    ) || (dependency.owning_package == "yss-application"
                        && source_layer == RustLayer::Commands
                        && dependency
                            .source_file
                            .starts_with("src-tauri/crates/yss-application/src/ipc/commands/"))))
                || (dependency.source_file.starts_with("plugins/")
                    && (package_name == "yss-sci" || package_name.starts_with("yss-sci-")));
            let crosses_filesystem_boundary =
                dependency.owning_package == "yss-filesystem" && package_name != "yss-filesystem";
            let crosses_node_boundary =
                source_layer == RustLayer::Node && package_name.starts_with("yss-graph-");
            let crosses_kernel_boundary = dependency.owning_package == "yss-node-kernel"
                && (package_name.starts_with("yss-graph-")
                    || package_name.starts_with("yss-project")
                    || package_name == "yss-application");
            if !crosses_scientific_boundary
                && !crosses_filesystem_boundary
                && !crosses_node_boundary
                && !crosses_kernel_boundary
                && target_layer.is_some_and(|target| {
                    internal_layer_dependency_is_allowed(source_layer, target)
                        // Node declarations remain available to pure document and transport owners.
                        || (package_name == "yss-node-protocol"
                            && internal_layer_dependency_is_allowed(source_layer, RustLayer::PureLeaf))
                        || capabilities.iter().any(|capability| {
                            capability.source_layer == source_layer
                                && capability.repository_relative_source_file
                                    == dependency.source_file
                                && capability.fully_qualified_owner == dependency.owner
                                && capability
                                    .canonical_origin_targets
                                    .contains(&dependency.canonical_origin_target.as_str())
                        })
                })
            {
                return None;
            }
            Some(ArchitectureFinding {
                key: ArchitectureFindingKey {
                    rule_id: if crosses_filesystem_boundary {
                        "rust.internal.filesystem-boundary"
                    } else if crosses_node_boundary {
                        "rust.internal.node-boundary"
                    } else if crosses_kernel_boundary {
                        "rust.internal.kernel-boundary"
                    } else if crosses_scientific_boundary {
                        "rust.internal.scientific-boundary"
                    } else {
                        "rust.internal.source-layer"
                    }
                    .to_owned(),
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
            RustLayer::Commands | RustLayer::Filesystem | RustLayer::PureLeaf
        ) | (RustLayer::Commands, RustLayer::PureLeaf)
            | (
                RustLayer::PlatformAdapter,
                RustLayer::Logging | RustLayer::PureLeaf
            )
            | (
                RustLayer::Application,
                RustLayer::Project
                    | RustLayer::Filesystem
                    | RustLayer::Node
                    | RustLayer::Graph
                    | RustLayer::Execution
                    | RustLayer::NodeKernel
                    | RustLayer::DatabaseCore
                    | RustLayer::PureLeaf
            )
            | (
                RustLayer::Project,
                RustLayer::PureLeaf | RustLayer::Filesystem
            )
            | (RustLayer::Graph, RustLayer::Node | RustLayer::PureLeaf)
            | (
                RustLayer::Execution,
                RustLayer::NodeKernel | RustLayer::PureLeaf
            )
            | (RustLayer::NodeKernel, RustLayer::PureLeaf)
            | (RustLayer::SciCore, RustLayer::PureLeaf)
            | (RustLayer::DatabaseCore, RustLayer::PureLeaf)
            | (
                RustLayer::BackendAdapter,
                RustLayer::DatabaseCore | RustLayer::PureLeaf
            )
            | (RustLayer::Node, RustLayer::PureLeaf)
            | (RustLayer::Transport, RustLayer::PureLeaf)
    )
}
