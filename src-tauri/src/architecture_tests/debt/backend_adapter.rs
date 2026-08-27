use crate::architecture_tests::model::RustDebtEntry;

use super::{BACKEND_ADAPTER_SPEC, StagedAdapterDebt};

pub(super) const STAGED_ADAPTER_DEBT: &[StagedAdapterDebt] = &[
    StagedAdapterDebt {
        adapter: "yssbi_lib::julia::bayes_worker_adapter::JuliaBayesWorkerAdapter",
        activation_owner: "Execution Task 8",
        owning_migration_spec: BACKEND_ADAPTER_SPEC,
    },
    StagedAdapterDebt {
        adapter: "yssbi_lib::backend_adapters::execution::scientific::SciApiScientificBackend",
        activation_owner: "Execution Task 8",
        owning_migration_spec: BACKEND_ADAPTER_SPEC,
    },
];

pub(super) fn extend(entries: &mut Vec<RustDebtEntry>) {
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/backend_adapters/tabular/polars.rs",
        "yssbi_lib::backend_adapters::tabular::polars",
        [
            (Use, 1, "external:chrono::Datelike"),
            (Use, 1, "external:chrono::NaiveDate"),
            (Use, 1, "external:chrono::NaiveDateTime"),
            (Use, 1, "external:chrono::Utc"),
            (
                Path,
                1,
                "external:chrono::DateTime::from_naive_utc_and_offset"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/application/bayes.rs",
        "yssbi_lib::application::bayes",
        [
            (Path, 1, "external:uuid::Uuid::new_v4"),
            (Use, 1, "external:polars::prelude::DataFrame"),
            (Use, 1, "external:polars::prelude::Float64Chunked"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/application/database.rs",
        "yssbi_lib::application::database",
        [
            (Use, 1, "external:serde::Serialize"),
            (Use, 1, "external:uuid::Uuid"),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::MoveFileExW"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/application/hypothesis.rs",
        "yssbi_lib::application::hypothesis",
        [
            (Use, 1, "external:ndarray::Array1"),
            (Use, 1, "external:ndarray::Array2"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/database/sql_reader.rs",
        "yssbi_lib::database::sql_reader",
        [(Path, 4, "external:tauri::async_runtime::block_on")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/database/sqlite_reader.rs",
        "yssbi_lib::database::sqlite_reader",
        [(Path, 2, "external:tauri::async_runtime::block_on")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/julia/worker.rs",
        "yssbi_lib::julia::worker",
        [(Use, 1, "external:uuid::Uuid")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/julia/worker/assets.rs",
        "yssbi_lib::julia::worker::assets",
        [
            (Use, 1, "external:uuid::Uuid"),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::MoveFileExW"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/node_system/runtime/builtin.rs",
        "yssbi_lib::node_system::runtime::builtin",
        [
            (Macro, 2, "external:serde_json::json"),
            (Path, 1, "external:serde_json::from_str"),
            (Path, 1, "external:serde_json::to_string"),
            (Path, 1, "external:serde_json::Value"),
            (Path, 1, "external:serde_json::Value::Bool"),
            (Path, 1, "external:serde_json::Value::Null"),
            (Path, 1, "external:serde_json::Value::String"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/node_system/runtime/kernels/dataframe/mod.rs",
        "yssbi_lib::node_system::runtime::kernels::dataframe",
        [
            (Path, 1, "external:polars::prelude::AnyValue"),
            (Path, 2, "external:polars::prelude::DataFrame"),
            (Use, 1, "external:polars::prelude::AnyValue"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/node_system/runtime/production_relational.rs",
        "yssbi_lib::node_system::runtime::production_relational",
        [(Use, 1, "external:polars::prelude::DataFrame")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/node_system/runtime/project_resource.rs",
        "yssbi_lib::node_system::runtime::project_resource",
        [(Use, 1, "external:polars::prelude::DataFrame")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/node_system/runtime/relational_dataframe.rs",
        "yssbi_lib::node_system::runtime::relational_dataframe",
        [
            (Use, 1, "external:polars::prelude::BooleanChunked"),
            (Use, 1, "external:polars::prelude::Column"),
            (Use, 1, "external:polars::prelude::DataFrame"),
            (Use, 1, "external:polars::prelude::DataType"),
            (Use, 1, "external:polars::prelude::NamedFrom"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/project/filesystem/root.rs",
        "yssbi_lib::project::filesystem::root",
        [
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::FILE_SHARE_READ"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Storage::FileSystem::GetFileInformationByHandle"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/project/filesystem/windows_path_identity.rs",
        "yssbi_lib::project::filesystem::windows_path_identity",
        [
            (
                Use,
                1,
                "external:windows-sys::Win32::Globalization::LCMAP_UPPERCASE"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Globalization::LCMapStringEx"
            ),
            (
                Use,
                1,
                "external:windows-sys::Win32::Globalization::LOCALE_NAME_INVARIANT"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/project/project_lifecycle.rs",
        "yssbi_lib::project::project_lifecycle",
        [(Path, 1, "external:trash::delete")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/project/project_registry.rs",
        "yssbi_lib::project::project_registry",
        [
            (Path, 7, "external:sqlx::Error"),
            (Path, 1, "external:sqlx::Error::Decode"),
            (Path, 6, "external:sqlx::query"),
            (Path, 4, "external:sqlx::query_as"),
            (Path, 1, "external:sqlx::query_scalar"),
            (Path, 2, "external:tauri::ipc::Channel"),
            (Use, 1, "external:sqlx::FromRow"),
            (Use, 1, "external:sqlx::sqlite::SqliteConnectOptions"),
            (Use, 1, "external:sqlx::sqlite::SqliteJournalMode"),
            (Use, 1, "external:sqlx::sqlite::SqlitePoolOptions"),
            (Use, 1, "external:sqlx::sqlite::SqliteSynchronous"),
            (Use, 1, "external:sqlx::SqlitePool"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/project/project_watcher.rs",
        "yssbi_lib::project::project_watcher",
        [
            (Path, 1, "external:notify::recommended_watcher"),
            (Path, 1, "external:notify::Result"),
            (Use, 1, "external:notify::Event"),
            (Use, 1, "external:notify::RecommendedWatcher"),
            (Use, 1, "external:notify::RecursiveMode"),
            (Use, 1, "external:notify::Watcher"),
            (Use, 1, "external:tauri::AppHandle"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/schema/database.rs",
        "yssbi_lib::schema::database",
        [(Path, 1, "external:polars::prelude::Schema")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/window_state/kind.rs",
        "yssbi_lib::window_state::kind",
        [
            (Use, 1, "external:serde::de::Error"),
            (Use, 1, "external:serde::de::IgnoredAny"),
            (Use, 1, "external:serde::de::MapAccess"),
            (Use, 1, "external:serde::de::Visitor"),
            (Use, 1, "external:serde::Deserialize"),
            (Use, 1, "external:serde::Deserializer"),
            (Use, 1, "external:serde::ser::SerializeMap"),
            (Use, 1, "external:serde::Serialize"),
            (Use, 1, "external:serde::Serializer"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/window_state/mod.rs",
        "yssbi_lib::window_state",
        [(Path, 1, "external:serde_json::from_str")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.external.runtime-source-layer",
        "src-tauri/src/window_state/persistence.rs",
        "yssbi_lib::window_state::persistence",
        [
            (Path, 1, "external:serde_json::to_vec_pretty"),
            (Path, 1, "external:uuid::Uuid::new_v4"),
            (Use, 1, "external:serde::Serialize"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/application/bayes.rs",
        "yssbi_lib::application::bayes",
        [(
            Use,
            1,
            "yssbi_lib::julia::worker::task_directory::JuliaWorkerTaskDirectory"
        )],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/node_system/catalog/builtin.rs",
        "yssbi_lib::node_system::catalog::builtin",
        [
            (Use, 1, "yssbi_lib::node_system::catalog::control"),
            (Use, 1, "yssbi_lib::node_system::catalog::core_nodes"),
            (Use, 1, "yssbi_lib::node_system::catalog::dataframe"),
            (Use, 1, "yssbi_lib::node_system::catalog::distribution"),
            (Use, 1, "yssbi_lib::node_system::catalog::plot"),
            (Use, 1, "yssbi_lib::node_system::catalog::project"),
            (Use, 1, "yssbi_lib::node_system::catalog::statistics"),
            (Use, 1, "yssbi_lib::node_system::protocol::*"),
            (Use, 1, "yssbi_lib::node_system::registry::*"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/node_system/runtime/builtin.rs",
        "yssbi_lib::node_system::runtime::builtin",
        [
            (
                Use,
                1,
                "yssbi_lib::node_system::protocol::value::CanonicalDecimal"
            ),
            (Use, 1, "yssbi_lib::node_system::protocol::value::Value"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_activation.rs",
        "yssbi_lib::project::project_activation",
        [
            (
                Use,
                1,
                "yssbi_lib::database::database_instance::DatabaseInstance"
            ),
            (Use, 1, "yssbi_lib::database::database_state::DatabaseState"),
            (
                Use,
                1,
                "yssbi_lib::database::project_storage::bind_duckdb_instance"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_io.rs",
        "yssbi_lib::project::project_io",
        [
            (
                Path,
                1,
                "yssbi_lib::database::duckdb_reader::list_data_tables"
            ),
            (
                Path,
                1,
                "yssbi_lib::database::duckdb_reader::read_display_name"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_reads.rs",
        "yssbi_lib::project::project_reads",
        [(
            Use,
            1,
            "yssbi_lib::database::database_instance::DatabaseInstance"
        ),],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_state.rs",
        "yssbi_lib::project::project_state",
        [(Use, 1, "yssbi_lib::database::database_state::DatabaseState")],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_state/execution.rs",
        "yssbi_lib::project::project_state::execution",
        [(
            Path,
            1,
            "yssbi_lib::database::database_instance::DatabaseInstance"
        ),],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_state/projection.rs",
        "yssbi_lib::project::project_state::projection",
        [(
            Path,
            1,
            "yssbi_lib::database::duckdb_reader::read_table_meta"
        ),],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_state_database.rs",
        "yssbi_lib::project::project_state_database",
        [
            (
                Path,
                1,
                "yssbi_lib::database::project_storage::remove_duckdb_table_if_needed"
            ),
            (Use, 1, "yssbi_lib::database::*"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/project/project_store.rs",
        "yssbi_lib::project::project_store",
        [(
            Use,
            1,
            "yssbi_lib::database::database_instance::DatabaseInstance"
        )],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/schema/database.rs",
        "yssbi_lib::schema::database",
        [
            (
                Path,
                1,
                "yssbi_lib::database::database_schema::polars_dtype_to_raw_string"
            ),
            (
                Path,
                1,
                "yssbi_lib::database::duckdb_reader::DuckDbColumnMeta"
            ),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/sci/api/bayes/result.rs",
        "yssbi_lib::sci::api::bayes::result",
        [(
            Use,
            1,
            "yssbi_lib::julia::worker::task_directory::JuliaWorkerTaskDirectory"
        )],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/sci/backends/julia/bayes/fit.rs",
        "yssbi_lib::sci::backends::julia::bayes::fit",
        [
            (Use, 1, "yssbi_lib::sci::api::bayes::backend::BayesBackend"),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::backend::BayesBackendError"
            ),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::backend::BayesBackendRequest"
            ),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::backend::BayesProgressCallback"
            ),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::exchange::BayesDataExchangeManifest"
            ),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::exchange::BayesExchangeColumn"
            ),
            (Use, 1, "yssbi_lib::sci::api::bayes::model::BayesModelSpec"),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::result::InferenceResult"
            ),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::result::ResultArtifactKind"
            ),
            (
                Use,
                1,
                "yssbi_lib::sci::api::bayes::result::TaskErrorDetails"
            ),
            (Use, 1, "yssbi_lib::sci::api::bayes::result::TaskProgress"),
        ],
    );
    debt_group!(
        entries,
        BACKEND_ADAPTER_SPEC,
        "rust.internal.source-layer",
        "src-tauri/src/sci/backends/julia/bayes/predictor.rs",
        "yssbi_lib::sci::backends::julia::bayes::predictor",
        [
            (Use, 1, "yssbi_lib::sci::api::bayes::model::BayesModelSpec"),
            (Use, 1, "yssbi_lib::sci::api::bayes::model::BinaryOp"),
            (Use, 1, "yssbi_lib::sci::api::bayes::model::Expression"),
            (Use, 1, "yssbi_lib::sci::api::bayes::model::LikelihoodSpec"),
            (Use, 1, "yssbi_lib::sci::api::bayes::model::MathFunction"),
            (Use, 1, "yssbi_lib::sci::api::bayes::model::UnaryOp"),
        ],
    );
}
