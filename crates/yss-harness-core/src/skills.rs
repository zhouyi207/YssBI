use std::collections::BTreeMap;

use yss_harness_contract::{SkillId, SkillManifest, SkillPackage, SkillVersion, SourceHash};

pub(crate) const STATISTICAL_REPORT_WRITING_ID: &str =
    "yssbi.statistics.statistical-report-writing";
pub(crate) const STATISTICAL_REPORT_WRITING_VERSION: &str = "1.0.0";
const STATISTICAL_REPORT_WRITING_INSTRUCTIONS: &str =
    include_str!("../skills/statistical-report-writing/SKILL.md");

#[derive(Clone, Debug, Default)]
pub struct SkillRegistry {
    packages: BTreeMap<(SkillId, SkillVersion), SkillPackage>,
}

impl SkillRegistry {
    pub fn with_builtins() -> Result<Self, SkillError> {
        let mut registry = Self::default();
        registry.install(builtin_statistical_report_writing()?)?;
        Ok(registry)
    }

    pub fn install(&mut self, package: SkillPackage) -> Result<(), SkillError> {
        validate_package(&package)?;
        let key = (
            package.manifest.id.clone(),
            package.manifest.version.clone(),
        );
        if let Some(existing) = self.packages.get(&key) {
            return if existing == &package {
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
}

fn builtin_statistical_report_writing() -> Result<SkillPackage, SkillError> {
    let id = SkillId::try_new(STATISTICAL_REPORT_WRITING_ID)?;
    let version = SkillVersion::try_new(STATISTICAL_REPORT_WRITING_VERSION)?;
    let digest = yss_canonical_hash::hash_canonical(
        "yssbi.skill.package.v1",
        &(&id, &version, STATISTICAL_REPORT_WRITING_INSTRUCTIONS),
    )
    .map_err(|_| SkillError::HashFailed)?;
    Ok(SkillPackage {
        manifest: SkillManifest {
            id,
            version,
            source_hash: SourceHash::try_new(hex::encode(digest))?,
        },
        instructions: STATISTICAL_REPORT_WRITING_INSTRUCTIONS.to_owned(),
    })
}

fn validate_package(package: &SkillPackage) -> Result<(), SkillError> {
    if package.instructions.trim().is_empty() || package.instructions.len() > 256 * 1024 {
        return Err(SkillError::InvalidManifest);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("skill identity is invalid")]
    Identity(#[from] yss_harness_contract::AutomationIdentityError),
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
    fn builtin_resolution_is_exact_and_rejects_silent_shadowing() {
        let mut registry = SkillRegistry::with_builtins().unwrap();
        let report = registry
            .resolve_exact(
                &SkillId::try_new(STATISTICAL_REPORT_WRITING_ID).unwrap(),
                &SkillVersion::try_new(STATISTICAL_REPORT_WRITING_VERSION).unwrap(),
            )
            .unwrap();
        assert_eq!(report.instructions, STATISTICAL_REPORT_WRITING_INSTRUCTIONS);
        assert!(
            registry
                .resolve_exact(
                    &report.manifest.id,
                    &SkillVersion::try_new("2.0.0").unwrap()
                )
                .is_err()
        );
        let report = report.clone();
        registry.install(report.clone()).unwrap();
        let mut changed = report.clone();
        changed
            .instructions
            .push_str("\nChanged reporting requirements.");
        assert!(matches!(
            registry.install(changed),
            Err(SkillError::SilentShadowing)
        ));
        let mut changed = report;
        changed.manifest.source_hash = SourceHash::try_new("f".repeat(64)).unwrap();
        assert!(matches!(
            registry.install(changed),
            Err(SkillError::SilentShadowing)
        ));
    }
}
