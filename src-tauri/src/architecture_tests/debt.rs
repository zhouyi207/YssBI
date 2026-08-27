use std::collections::{BTreeMap, BTreeSet};

use super::model::{ArchitectureFinding, DebtKey, RustDebtEntry};
use super::semantic_guards::{
    PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE, SemanticGuardViolation, SemanticGuardViolationReason,
};

const APPROVED_MIGRATION_SPECS: &[&str] = &[
    BACKEND_ADAPTER_SPEC,
    PROJECT_GRAPH_SPEC,
    EXECUTION_RUNTIME_SPEC,
    PRESENTATION_COMMAND_SPEC,
];

const BACKEND_ADAPTER_SPEC: &str = "docs/architecture/RUST_BACKEND_ADAPTER_BOUNDARIES.md";
const PROJECT_GRAPH_SPEC: &str = "docs/architecture/PROJECT_GRAPH_OWNERSHIP_BOUNDARIES.md";
const EXECUTION_RUNTIME_SPEC: &str = "docs/architecture/EXECUTION_RUNTIME_BOUNDARIES.md";
const PRESENTATION_COMMAND_SPEC: &str = "docs/architecture/PRESENTATION_COMMAND_BOUNDARIES.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StagedAdapterDebt {
    pub(super) adapter: &'static str,
    pub(super) activation_owner: &'static str,
    pub(super) owning_migration_spec: &'static str,
}

macro_rules! debt_group {
    (
        $entries:ident,
        $migration_spec:expr,
        $rule_id:literal,
        $source_file:literal,
        $owner:literal,
        [$(($kind:ident, $count:literal, $target:literal)),* $(,)?]
        $(,)?
    ) => {
        $(
            $entries.push(RustDebtEntry {
                key: $crate::architecture_tests::model::DebtKey {
                    rule_id: $rule_id.to_owned(),
                    repository_relative_source_file: $source_file.to_owned(),
                    fully_qualified_owner: $owner.to_owned(),
                    dependency_kind: $crate::architecture_tests::model::RustDependencyKind::$kind,
                    canonical_origin_target: $target.to_owned(),
                },
                expected_occurrences: $count,
                owning_migration_spec: $migration_spec,
            });
        )*
    };
}

mod backend_adapter;
mod execution_runtime;
mod presentation_command;
mod project_graph;
pub(super) fn rust_architecture_debt() -> Vec<RustDebtEntry> {
    let mut entries = Vec::new();
    backend_adapter::extend(&mut entries);
    execution_runtime::extend(&mut entries);
    presentation_command::extend(&mut entries);
    project_graph::extend(&mut entries);
    entries
}

pub(super) fn pure_leaf_graph_document_json_debt() -> Vec<SemanticGuardViolation> {
    use super::model::RustDependencyKind::{Path, Use};

    // Backend-adapter Task 5 owns the atomic split of the legacy mixed tabular
    // snapshot. Keep its already-existing JSON dependencies visible and
    // bidirectionally ratcheted until that owner replacement lands.
    [
        (Use, "external:serde_json::Value"),
        (Path, "external:serde_json::from_str"),
        (Path, "external:serde_json::to_string"),
    ]
    .into_iter()
    .map(
        |(dependency_kind, canonical_origin_target)| SemanticGuardViolation {
            rule_id: PURE_LEAF_GRAPH_DOCUMENT_JSON_RULE,
            source_file: "src-tauri/src/tabular/snapshot.rs".to_owned(),
            reason: SemanticGuardViolationReason::UnexpectedSerdeJsonDependency {
                dependency_kind,
                canonical_origin_target: canonical_origin_target.to_owned(),
            },
        },
    )
    .collect()
}

pub(super) fn staged_backend_adapter_debt() -> &'static [StagedAdapterDebt] {
    backend_adapter::STAGED_ADAPTER_DEBT
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DebtCountDifference {
    pub(super) key: DebtKey,
    pub(super) actual_occurrences: usize,
    pub(super) declared_occurrences: usize,
    pub(super) actual_locations: Vec<(usize, usize)>,
    pub(super) owning_migration_spec: Option<&'static str>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum DebtMismatch {
    #[error("architecture debt counts differ from the declared manifest")]
    Counts {
        new_or_increased: Vec<DebtCountDifference>,
        stale_or_decreased: Vec<DebtCountDifference>,
    },
    #[error("duplicate declared architecture debt key: {key:?}")]
    DuplicateDeclaredKey { key: DebtKey },
    #[error("architecture debt entry must declare a positive occurrence count: {key:?}")]
    InvalidExpectedOccurrences { key: DebtKey },
    #[error("unapproved architecture debt migration spec '{owning_migration_spec}'")]
    InvalidMigrationSpec {
        key: DebtKey,
        owning_migration_spec: &'static str,
    },
}

impl DebtMismatch {
    pub(super) fn new_or_increased(&self) -> &[DebtCountDifference] {
        match self {
            Self::Counts {
                new_or_increased, ..
            } => new_or_increased,
            Self::DuplicateDeclaredKey { .. }
            | Self::InvalidExpectedOccurrences { .. }
            | Self::InvalidMigrationSpec { .. } => &[],
        }
    }

    pub(super) fn stale_or_decreased(&self) -> &[DebtCountDifference] {
        match self {
            Self::Counts {
                stale_or_decreased, ..
            } => stale_or_decreased,
            Self::DuplicateDeclaredKey { .. }
            | Self::InvalidExpectedOccurrences { .. }
            | Self::InvalidMigrationSpec { .. } => &[],
        }
    }
}

pub(super) fn compare_exact_rust_debt(
    actual: &[ArchitectureFinding],
    declared: &[RustDebtEntry],
) -> Result<(), DebtMismatch> {
    let mut actual_counts = BTreeMap::<DebtKey, usize>::new();
    let mut actual_locations = BTreeMap::<DebtKey, Vec<(usize, usize)>>::new();
    for finding in actual {
        *actual_counts.entry(finding.key.clone()).or_default() += 1;
        actual_locations
            .entry(finding.key.clone())
            .or_default()
            .push((finding.line, finding.column));
    }
    for locations in actual_locations.values_mut() {
        locations.sort_unstable();
    }

    let approved_specs = APPROVED_MIGRATION_SPECS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut declared_counts = BTreeMap::<DebtKey, usize>::new();
    let mut migration_specs = BTreeMap::<DebtKey, &'static str>::new();
    for entry in declared {
        if entry.expected_occurrences == 0 {
            return Err(DebtMismatch::InvalidExpectedOccurrences {
                key: entry.key.clone(),
            });
        }
        if !approved_specs.contains(entry.owning_migration_spec) {
            return Err(DebtMismatch::InvalidMigrationSpec {
                key: entry.key.clone(),
                owning_migration_spec: entry.owning_migration_spec,
            });
        }
        if declared_counts
            .insert(entry.key.clone(), entry.expected_occurrences)
            .is_some()
        {
            return Err(DebtMismatch::DuplicateDeclaredKey {
                key: entry.key.clone(),
            });
        }
        migration_specs.insert(entry.key.clone(), entry.owning_migration_spec);
    }

    let keys = actual_counts
        .keys()
        .chain(declared_counts.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut new_or_increased = Vec::new();
    let mut stale_or_decreased = Vec::new();
    for key in keys {
        let actual_occurrences = actual_counts.get(&key).copied().unwrap_or_default();
        let declared_occurrences = declared_counts.get(&key).copied().unwrap_or_default();
        if actual_occurrences == declared_occurrences {
            continue;
        }
        let difference = DebtCountDifference {
            actual_locations: actual_locations.get(&key).cloned().unwrap_or_default(),
            owning_migration_spec: migration_specs.get(&key).copied(),
            key,
            actual_occurrences,
            declared_occurrences,
        };
        if actual_occurrences > declared_occurrences {
            new_or_increased.push(difference);
        } else {
            stale_or_decreased.push(difference);
        }
    }

    if new_or_increased.is_empty() && stale_or_decreased.is_empty() {
        Ok(())
    } else {
        Err(DebtMismatch::Counts {
            new_or_increased,
            stale_or_decreased,
        })
    }
}
