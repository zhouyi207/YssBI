use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    ArchitectureAuditError, ArchitectureFinding, ArchitectureFindingKey, CanonicalDependency,
    CanonicalOrigin, CargoDependencyAuthority, CargoDependencyDeclaration, CargoDependencyScope,
    RustDependencyMode, RustLayer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ExternalDependencyDeclarationAllowance {
    pub(super) owning_package: &'static str,
    pub(super) mode: RustDependencyMode,
    pub(super) package_name: &'static str,
    pub(super) target_condition: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ExternalDependencyUseAllowance {
    pub(super) source_layer: RustLayer,
    pub(super) mode: RustDependencyMode,
    pub(super) package_name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ExternalDependencyPolicy {
    pub(super) declarations: &'static [ExternalDependencyDeclarationAllowance],
    pub(super) uses: &'static [ExternalDependencyUseAllowance],
}

const RUST_EXTERNAL_DECLARATIONS: &[ExternalDependencyDeclarationAllowance] = &[
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Build,
        package_name: "tauri-build",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "calamine",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "duckdb",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "ndarray",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "notify",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "polars",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "polars-arrow",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "polars-dtype",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "rand",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "sqlx",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "statrs",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-clipboard-manager",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-dialog",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-fs",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-opener",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tokio",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "trash",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-casefold",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-normalization",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yssbi",
        mode: RustDependencyMode::Runtime,
        package_name: "windows-sys",
        target_condition: Some("cfg(windows)"),
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-data-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-data-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-database-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-database-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "sha2",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-database-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-diagnostics",
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-diagnostics",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-diagnostics",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-diagnostics",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-diagnostics",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-diagnostics",
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-canonical-hash",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-canonical-hash",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-canonical-hash",
        mode: RustDependencyMode::Runtime,
        package_name: "sha2",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-document",
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-document",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-document",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-document",
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-normalization",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-document",
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-protocol",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-protocol",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-registry",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-graph-registry",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-math",
        mode: RustDependencyMode::Runtime,
        package_name: "mathlex",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "faer",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "ndarray",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "polars",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-sci",
        mode: RustDependencyMode::Runtime,
        package_name: "statrs",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tabular-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tabular-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "file-rotate",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "tracing-log",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-tracing",
        mode: RustDependencyMode::Runtime,
        package_name: "tracing-subscriber",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-variable-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-variable-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-variable-contract",
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-window-state",
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-window-state",
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-window-state",
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-window-state",
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-window-state",
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
        target_condition: None,
    },
    ExternalDependencyDeclarationAllowance {
        owning_package: "yss-window-state",
        mode: RustDependencyMode::Runtime,
        package_name: "windows-sys",
        target_condition: Some("cfg(windows)"),
    },
];

const RUST_EXTERNAL_USES: &[ExternalDependencyUseAllowance] = &[
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BuildScript,
        mode: RustDependencyMode::Build,
        package_name: "tauri-build",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-clipboard-manager",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-clipboard-manager",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-clipboard-manager",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-clipboard-manager",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-dialog",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-dialog",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-dialog",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-dialog",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-fs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-fs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-fs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-fs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-opener",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-opener",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-opener",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri-plugin-opener",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "notify",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "windows-sys",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "file-rotate",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing-log",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing-subscriber",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "duckdb",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "duckdb",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "sqlx",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "sqlx",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "calamine",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "calamine",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "polars",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "polars",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "polars",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "polars-arrow",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "polars-arrow",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "polars-arrow",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "polars-dtype",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "polars-dtype",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "polars-dtype",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "faer",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "ndarray",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "ndarray",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "ndarray",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "statrs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "statrs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "statrs",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "rand",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "rand",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "rand",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "num-traits",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "regex",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-casefold",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-casefold",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-casefold",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-normalization",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-normalization",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "unicode-normalization",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Diagnostics,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Application,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Diagnostics,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "sha2",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Application,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "tokio",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Diagnostics,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Application,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Graph,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Execution,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::SciCore,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::DatabaseCore,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BuiltinComposition,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Transport,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Logging,
        mode: RustDependencyMode::Runtime,
        package_name: "tracing",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Application,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Application,
        mode: RustDependencyMode::Runtime,
        package_name: "ndarray",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Application,
        mode: RustDependencyMode::Runtime,
        package_name: "windows-sys",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "chrono",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::BackendAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "windows-sys",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Commands,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Diagnostics,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Diagnostics,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Diagnostics,
        mode: RustDependencyMode::Runtime,
        package_name: "tauri",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "serde",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "serde_json",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PlatformAdapter,
        mode: RustDependencyMode::Runtime,
        package_name: "uuid",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::CompositionRoot,
        mode: RustDependencyMode::Runtime,
        package_name: "thiserror",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "windows-sys",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::Project,
        mode: RustDependencyMode::Runtime,
        package_name: "trash",
    },
    ExternalDependencyUseAllowance {
        source_layer: RustLayer::PureLeaf,
        mode: RustDependencyMode::Runtime,
        package_name: "mathlex",
    },
];

pub(super) const RUST_EXTERNAL_DEPENDENCY_POLICY: ExternalDependencyPolicy =
    ExternalDependencyPolicy {
        declarations: RUST_EXTERNAL_DECLARATIONS,
        uses: RUST_EXTERNAL_USES,
    };

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DeclarationKey {
    owning_package: String,
    mode: RustDependencyMode,
    package_name: String,
    target_condition: Option<String>,
}

pub(super) fn rust_external_dependency_findings(
    declarations: &[CargoDependencyDeclaration],
    dependencies: &[CanonicalDependency],
    classification: &BTreeMap<String, RustLayer>,
    policy: &ExternalDependencyPolicy,
) -> Result<Vec<ArchitectureFinding>, ArchitectureAuditError> {
    validate_policy(policy)?;
    compare_external_declarations(declarations, policy)?;

    let mut findings = Vec::new();
    for dependency in dependencies {
        let CanonicalOrigin::External(origin) = &dependency.origin else {
            continue;
        };
        validate_dependency_declaration(declarations, dependency, origin)?;
        let source_layer = classification
            .get(&dependency.source_file)
            .copied()
            .ok_or_else(|| ArchitectureAuditError::UnclassifiedProductionSource {
                source_files: vec![dependency.source_file.clone()],
            })?;
        let allowed = policy.uses.iter().any(|candidate| {
            candidate.source_layer == source_layer
                && candidate.mode == dependency.mode
                && candidate.package_name == origin.package_name
        });
        if allowed {
            continue;
        }
        findings.push(ArchitectureFinding {
            key: ArchitectureFindingKey {
                rule_id: match dependency.mode {
                    RustDependencyMode::Runtime => "rust.external.runtime-source-layer",
                    RustDependencyMode::Build => "rust.external.build-source-layer",
                }
                .to_owned(),
                repository_relative_source_file: dependency.source_file.clone(),
                fully_qualified_owner: dependency.owner.clone(),
                dependency_kind: dependency.kind,
                canonical_origin_target: dependency.canonical_origin_target.clone(),
            },
            source_layer,
            target_layer: None,
            line: dependency.line,
            column: dependency.column,
        });
    }
    findings.sort();
    Ok(findings)
}

fn validate_policy(policy: &ExternalDependencyPolicy) -> Result<(), ArchitectureAuditError> {
    let mut declarations = BTreeSet::new();
    for allowance in policy.declarations {
        validate_literal_package_name(allowance.package_name)?;
        if !declarations.insert(*allowance) {
            return Err(ArchitectureAuditError::InvalidExternalDependencyPolicy {
                message: format!(
                    "duplicate declaration {}/{:?}/{}",
                    allowance.owning_package, allowance.mode, allowance.package_name
                ),
            });
        }
    }
    let mut uses = BTreeSet::new();
    for allowance in policy.uses {
        validate_literal_package_name(allowance.package_name)?;
        if !uses.insert(*allowance) {
            return Err(ArchitectureAuditError::InvalidExternalDependencyPolicy {
                message: format!(
                    "duplicate use {:?}/{:?}/{}",
                    allowance.source_layer, allowance.mode, allowance.package_name
                ),
            });
        }
    }
    Ok(())
}

fn validate_literal_package_name(package_name: &str) -> Result<(), ArchitectureAuditError> {
    if package_name.is_empty()
        || package_name.contains('*')
        || package_name.contains('/')
        || package_name.contains("::")
    {
        return Err(ArchitectureAuditError::InvalidExternalDependencyPolicy {
            message: format!("package name '{package_name}' is not a literal Cargo package"),
        });
    }
    Ok(())
}

fn compare_external_declarations(
    declarations: &[CargoDependencyDeclaration],
    policy: &ExternalDependencyPolicy,
) -> Result<(), ArchitectureAuditError> {
    let actual = declarations
        .iter()
        .filter(|declaration| declaration.authority == CargoDependencyAuthority::External)
        .filter_map(declaration_key)
        .collect::<BTreeSet<_>>();
    let expected = policy
        .declarations
        .iter()
        .map(|allowance| DeclarationKey {
            owning_package: allowance.owning_package.to_owned(),
            mode: allowance.mode,
            package_name: allowance.package_name.to_owned(),
            target_condition: allowance.target_condition.map(str::to_owned),
        })
        .collect::<BTreeSet<_>>();
    if actual == expected {
        return Ok(());
    }
    Err(
        ArchitectureAuditError::ExternalDependencyDeclarationSetMismatch {
            missing: expected
                .difference(&actual)
                .map(declaration_key_label)
                .collect(),
            unexpected: actual
                .difference(&expected)
                .map(declaration_key_label)
                .collect(),
        },
    )
}

fn declaration_key(declaration: &CargoDependencyDeclaration) -> Option<DeclarationKey> {
    let mode = match declaration.scope {
        CargoDependencyScope::Runtime => RustDependencyMode::Runtime,
        CargoDependencyScope::Build => RustDependencyMode::Build,
        CargoDependencyScope::Development => return None,
    };
    Some(DeclarationKey {
        owning_package: declaration.owning_package.clone(),
        mode,
        package_name: declaration.package_name.clone(),
        target_condition: declaration.target_condition.clone(),
    })
}

fn declaration_key_label(key: &DeclarationKey) -> String {
    format!(
        "{}/{:?}/{}{}",
        key.owning_package,
        key.mode,
        key.package_name,
        key.target_condition
            .as_deref()
            .map(|condition| format!("@{condition}"))
            .unwrap_or_default()
    )
}

fn validate_dependency_declaration(
    declarations: &[CargoDependencyDeclaration],
    dependency: &CanonicalDependency,
    origin: &super::model::ExternalDependencyOrigin,
) -> Result<(), ArchitectureAuditError> {
    let candidates = declarations
        .iter()
        .filter(|declaration| declaration.authority == CargoDependencyAuthority::External)
        .filter(|declaration| declaration.owning_package == dependency.owning_package)
        .filter(|declaration| {
            declaration.declared_name == origin.declared_name
                && declaration.package_name == origin.package_name
                && declaration.target_condition == origin.target_condition
        })
        .collect::<Vec<_>>();
    if candidates
        .iter()
        .any(|declaration| declaration.scope == CargoDependencyScope::Development)
        || origin.declaration_scope == CargoDependencyScope::Development
    {
        return Err(ArchitectureAuditError::DevelopmentDependencyInProduction {
            target: origin.package_name.clone(),
        });
    }
    if candidates.is_empty() {
        return Err(ArchitectureAuditError::UnknownExternalPackage {
            package_name: origin.package_name.clone(),
        });
    }
    let expected_scope = match dependency.mode {
        RustDependencyMode::Runtime => CargoDependencyScope::Runtime,
        RustDependencyMode::Build => CargoDependencyScope::Build,
    };
    if origin.declaration_scope != expected_scope
        || !candidates
            .iter()
            .any(|declaration| declaration.scope == expected_scope)
    {
        return Err(ArchitectureAuditError::DependencyScopeMismatch {
            owning_package: dependency.owning_package.clone(),
            target: origin.package_name.clone(),
        });
    }
    Ok(())
}
