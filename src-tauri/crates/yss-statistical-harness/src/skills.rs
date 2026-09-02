use std::collections::{BTreeMap, BTreeSet};

use yss_automation_contract::{
    CapabilityId, SkillId, SkillManifest, SkillPackage, SkillScope, SkillVersion, SourceHash,
    WorkflowId,
};

const DATASET_QUALITY_REVIEW_INSTRUCTIONS: &str =
    include_str!("../skills/dataset-quality-review/SKILL.md");

#[derive(Clone, Debug, Default)]
pub struct SkillRegistry {
    packages: BTreeMap<(SkillId, SkillVersion), SkillPackage>,
}

impl SkillRegistry {
    pub fn with_builtins() -> Result<Self, SkillError> {
        let mut registry = Self::default();
        registry.install(builtin_dataset_quality_review()?)?;
        Ok(registry)
    }

    pub fn install(&mut self, package: SkillPackage) -> Result<(), SkillError> {
        validate_package(&package)?;
        let key = (
            package.manifest.id.clone(),
            package.manifest.version.clone(),
        );
        if let Some(existing) = self.packages.get(&key) {
            return if existing.manifest.source_hash == package.manifest.source_hash {
                Ok(())
            } else {
                Err(SkillError::SilentShadowing)
            };
        }
        self.packages.insert(key, package);
        Ok(())
    }

    pub fn resolve_exact(
        &self,
        id: &SkillId,
        version: &SkillVersion,
    ) -> Result<&SkillPackage, SkillError> {
        self.packages
            .get(&(id.clone(), version.clone()))
            .ok_or(SkillError::NotFound)
    }

    pub fn effective_capabilities(
        package: &SkillPackage,
        principal_policy: &BTreeSet<CapabilityId>,
        tool_policy: &BTreeSet<CapabilityId>,
        approval_scope: &BTreeSet<CapabilityId>,
    ) -> BTreeSet<CapabilityId> {
        package
            .manifest
            .allowed_capabilities
            .iter()
            .copied()
            .filter(|capability| {
                principal_policy.contains(capability)
                    && tool_policy.contains(capability)
                    && approval_scope.contains(capability)
            })
            .collect()
    }
}

fn builtin_dataset_quality_review() -> Result<SkillPackage, SkillError> {
    let id = SkillId::try_new("yssbi.statistics.dataset-quality-review")?;
    let version = SkillVersion::try_new("1.0.0")?;
    let entry_workflow = WorkflowId::try_new("dataset_quality_review")?;
    let allowed_capabilities = vec![
        CapabilityId::InspectDatasetSchema,
        CapabilityId::InspectDatasetProfile,
        CapabilityId::InspectGraph,
    ];
    let knowledge_scopes = vec![
        "statistics.data_quality".to_owned(),
        "statistics.missingness".to_owned(),
    ];
    let digest = yss_canonical_hash::hash_canonical(
        "yssbi.skill.package.v1",
        &(
            &id,
            &version,
            &entry_workflow,
            &allowed_capabilities,
            &knowledge_scopes,
            DATASET_QUALITY_REVIEW_INSTRUCTIONS,
        ),
    )
    .map_err(|_| SkillError::HashFailed)?;
    let source_hash = SourceHash::try_new(hex(&digest))?;
    Ok(SkillPackage {
        manifest: SkillManifest {
            id,
            version,
            scope: SkillScope::Builtin,
            domain: "data-quality".to_owned(),
            entry_workflow,
            allowed_capabilities,
            knowledge_scopes,
            source_hash,
        },
        instructions: DATASET_QUALITY_REVIEW_INSTRUCTIONS.to_owned(),
    })
}

fn validate_package(package: &SkillPackage) -> Result<(), SkillError> {
    if package.manifest.domain.trim().is_empty()
        || package.manifest.allowed_capabilities.is_empty()
        || package.manifest.knowledge_scopes.is_empty()
        || package.instructions.trim().is_empty()
        || package.instructions.len() > 256 * 1024
    {
        return Err(SkillError::InvalidManifest);
    }
    let unique_capabilities = package
        .manifest
        .allowed_capabilities
        .iter()
        .collect::<BTreeSet<_>>();
    let unique_scopes = package
        .manifest
        .knowledge_scopes
        .iter()
        .collect::<BTreeSet<_>>();
    if unique_capabilities.len() != package.manifest.allowed_capabilities.len()
        || unique_scopes.len() != package.manifest.knowledge_scopes.len()
    {
        return Err(SkillError::InvalidManifest);
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("skill identity is invalid")]
    Identity(#[from] yss_automation_contract::AutomationIdentityError),
    #[error("skill package is invalid")]
    InvalidManifest,
    #[error("skill package hash failed")]
    HashFailed,
    #[error("skill package would silently shadow an installed version")]
    SilentShadowing,
    #[error("skill exact version was not found")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_resolution_is_exact_and_permissions_only_narrow() {
        let registry = SkillRegistry::with_builtins().unwrap();
        let package = registry
            .resolve_exact(
                &SkillId::try_new("yssbi.statistics.dataset-quality-review").unwrap(),
                &SkillVersion::try_new("1.0.0").unwrap(),
            )
            .unwrap();
        let principal = BTreeSet::from([
            CapabilityId::InspectDatasetSchema,
            CapabilityId::InspectGraph,
        ]);
        let tool_policy = BTreeSet::from([CapabilityId::InspectDatasetSchema]);
        let approval = principal.clone();

        assert_eq!(
            SkillRegistry::effective_capabilities(package, &principal, &tool_policy, &approval),
            BTreeSet::from([CapabilityId::InspectDatasetSchema])
        );
        assert!(
            registry
                .resolve_exact(
                    &package.manifest.id,
                    &SkillVersion::try_new("2.0.0").unwrap()
                )
                .is_err()
        );
    }
}
