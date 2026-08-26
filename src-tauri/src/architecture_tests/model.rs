use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProductionRootKind {
    Library,
    Binary,
    Example,
    BuildScript,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProductionRoot {
    pub(super) package_id: String,
    pub(super) package: String,
    pub(super) target: String,
    pub(super) kind: ProductionRootKind,
    pub(super) source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RustModule {
    pub(super) root_package_id: String,
    pub(super) root_target: String,
    pub(super) root_kind: ProductionRootKind,
    pub(super) repository_relative_source_file: String,
    pub(super) fully_qualified_owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum RustLayer {
    CompositionRoot,
    BuildScript,
    Commands,
    PlatformAdapter,
    Application,
    Project,
    Graph,
    Execution,
    SciCore,
    DatabaseCore,
    BackendAdapter,
    BuiltinComposition,
    Transport,
    Diagnostics,
    PureLeaf,
}

impl RustLayer {
    pub(super) const ALL: [Self; 15] = [
        Self::CompositionRoot,
        Self::BuildScript,
        Self::Commands,
        Self::PlatformAdapter,
        Self::Application,
        Self::Project,
        Self::Graph,
        Self::Execution,
        Self::SciCore,
        Self::DatabaseCore,
        Self::BackendAdapter,
        Self::BuiltinComposition,
        Self::Transport,
        Self::Diagnostics,
        Self::PureLeaf,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CargoDependencyScope {
    Runtime,
    Build,
    Development,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum RustDependencyMode {
    Runtime,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum RustDependencyKind {
    Use,
    ReExport,
    Path,
    Macro,
    Include,
    Attribute,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RawDependency {
    pub(super) owning_package: String,
    pub(super) repository_relative_source_file: String,
    pub(super) fully_qualified_owner: String,
    pub(super) kind: RustDependencyKind,
    pub(super) mode: RustDependencyMode,
    pub(super) written_target: String,
    pub(super) line: usize,
    pub(super) column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ExternalDependencyOrigin {
    pub(super) declared_name: String,
    pub(super) package_name: String,
    pub(super) declaration_scope: CargoDependencyScope,
    pub(super) target_condition: Option<String>,
    pub(super) canonical_subpath: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CanonicalOrigin {
    Repository {
        package_name: String,
        repository_relative_declaration_file: String,
        fully_qualified_target: String,
        symbol: String,
    },
    LanguageBuiltin {
        crate_name: String,
        canonical_subpath: Option<String>,
    },
    RepositoryAsset {
        repository_relative_path: String,
    },
    External(ExternalDependencyOrigin),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CanonicalDependency {
    pub(super) owning_package: String,
    pub(super) source_file: String,
    pub(super) owner: String,
    pub(super) kind: RustDependencyKind,
    pub(super) mode: RustDependencyMode,
    pub(super) origin: CanonicalOrigin,
    pub(super) canonical_origin_target: String,
    pub(super) line: usize,
    pub(super) column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct DebtKey {
    pub(super) rule_id: String,
    pub(super) repository_relative_source_file: String,
    pub(super) fully_qualified_owner: String,
    pub(super) dependency_kind: RustDependencyKind,
    pub(super) canonical_origin_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ArchitectureFinding {
    pub(super) key: DebtKey,
    pub(super) source_layer: RustLayer,
    pub(super) target_layer: Option<RustLayer>,
    pub(super) line: usize,
    pub(super) column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RustDebtEntry {
    pub(super) key: DebtKey,
    pub(super) expected_occurrences: usize,
    pub(super) owning_migration_spec: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CargoDependencyAuthority {
    WorkspaceMember { member_package_id: String },
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CargoDependencyDeclaration {
    pub(super) owning_package_id: String,
    pub(super) owning_package: String,
    pub(super) declared_name: String,
    pub(super) package_name: String,
    pub(super) authority: CargoDependencyAuthority,
    pub(super) scope: CargoDependencyScope,
    pub(super) target_condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct WorkspaceMemberCrateAlias {
    pub(super) owning_package_id: String,
    pub(super) owning_package: String,
    pub(super) declared_name: String,
    pub(super) member_package_id: String,
    pub(super) member_package: String,
    pub(super) library_crate_name: String,
    pub(super) library_root: PathBuf,
    pub(super) root_owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RustWorkspaceModel {
    pub(super) repository_root: PathBuf,
    pub(super) roots: Vec<ProductionRoot>,
    pub(super) dependency_declarations: Vec<CargoDependencyDeclaration>,
    pub(super) workspace_member_crate_aliases: Vec<WorkspaceMemberCrateAlias>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ArchitectureAuditError {
    #[error("failed to read architecture audit path '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cargo metadata failed with status {status}: {stderr}")]
    MetadataProcess { status: String, stderr: String },
    #[error("cargo metadata returned invalid JSON: {source}")]
    MetadataJson {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to parse Rust source '{path}': {source}")]
    SourceParse {
        path: PathBuf,
        #[source]
        source: syn::Error,
    },
    #[error("unresolved include target '{target}' in '{source_file}'")]
    UnresolvedInclude {
        source_file: PathBuf,
        target: String,
    },
    #[error("invalid Cargo metadata: {message}")]
    InvalidMetadata { message: String },
    #[error("metadata source path '{path}' escapes repository root '{repository_root}'")]
    SourceEscapesRepository {
        path: PathBuf,
        repository_root: PathBuf,
    },
    #[error("workspace member '{package}' has no unambiguous library target")]
    MissingLibraryTarget { package: String },
    #[error("workspace member package name '{package}' is ambiguous")]
    AmbiguousWorkspacePackage { package: String },
    #[error("unknown dependency '{target}' in package '{owning_package}'")]
    UnknownDependencyTarget {
        owning_package: String,
        target: String,
    },
    #[error("dependency '{target}' in package '{owning_package}' uses an unavailable scope")]
    DependencyScopeMismatch {
        owning_package: String,
        target: String,
    },
    #[error("development dependency '{target}' is used by production code")]
    DevelopmentDependencyInProduction { target: String },
    #[error("workspace member dependency '{target}' could not be resolved")]
    UnresolvedWorkspaceMember { target: String },
    #[error("repository dependency target '{target}' could not be resolved")]
    UnresolvedRepositoryTarget { target: String },
    #[error("repository dependency target cycle at '{target}'")]
    RepositoryTargetCycle { target: String },
    #[error(
        "dependency resolver exceeded {steps} steps at '{target}' from '{source_file}' in module '{current_module}'"
    )]
    ResolverStepLimit {
        steps: usize,
        source_file: PathBuf,
        current_module: String,
        target: String,
    },
    #[error("source include cycle at '{source_file}'")]
    IncludeCycle { source_file: PathBuf },
    #[error("repository symbol '{symbol}' is declared in multiple files: {declaration_files:?}")]
    AmbiguousRepositorySymbol {
        symbol: String,
        declaration_files: Vec<PathBuf>,
    },
    #[error("import alias '{alias}' is ambiguous in '{source_file}'")]
    AmbiguousImportAlias { source_file: PathBuf, alias: String },
    #[error("import alias '{alias}' forms a cycle while resolving '{target}' in '{source_file}'")]
    ImportAliasCycle {
        source_file: PathBuf,
        alias: String,
        target: String,
    },
    #[error("invalid dependency target '{target}'")]
    InvalidDependencyTarget { target: String },
    #[error("production module '{source_file}' names an unknown Cargo root")]
    UnknownProductionRoot { source_file: String },
    #[error("invalid internal dependency capability: {message}")]
    InvalidInternalCapability { message: String },
    #[error("unclassified production sources: {source_files:?}")]
    UnclassifiedProductionSource { source_files: Vec<String> },
    #[error("multiply classified production sources: {source_files:?}")]
    MultiplyClassifiedProductionSource { source_files: Vec<String> },
    #[error("unknown external package '{package_name}'")]
    UnknownExternalPackage { package_name: String },
    #[error("invalid external dependency policy: {message}")]
    InvalidExternalDependencyPolicy { message: String },
    #[error(
        "external dependency declarations differ from policy; missing={missing:?}, unexpected={unexpected:?}"
    )]
    ExternalDependencyDeclarationSetMismatch {
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
}
