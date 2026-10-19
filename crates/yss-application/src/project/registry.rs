//! Process-wide project registration.

use std::sync::Arc;
use yss_project_registry::{ProjectRegistry, ProjectRegistryError};
use yss_project_registry_contract::{ProjectRecord, ProjectRegistryStore};

pub struct ProjectManagement {
    pub(super) registry: ProjectRegistry,
}

impl ProjectManagement {
    pub fn new(store: Arc<dyn ProjectRegistryStore>) -> Self {
        Self {
            registry: ProjectRegistry::new(store),
        }
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, ProjectRegistryError> {
        self.registry.list_projects().await
    }

    pub async fn register_project(
        &self,
        name: &str,
        path: &str,
    ) -> Result<ProjectRecord, ProjectRegistryError> {
        self.registry.register_project(name, path).await
    }
}
