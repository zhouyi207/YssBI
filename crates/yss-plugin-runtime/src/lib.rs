mod activation;
mod bridge;
mod diagnostics;
mod installation;
mod ledger;
mod package;
mod process;
mod storage;
mod tasks;
pub use activation::PluginLease;
use activation::ProcessStart;
pub use package::current_target;
use process::PluginProcess;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub use storage::{read_bounded, resolve_data_file};

pub use yss_plugin_protocol::*;

#[derive(Clone)]
pub struct PluginManager {
    inner: Arc<ManagerInner>,
}
struct ManagerInner {
    initialization_error: Option<PluginFailure>,
    root: PathBuf,
    services: Arc<dyn HostServices>,
    registry: Mutex<Registry>,
    commit: Mutex<()>,
    state: Mutex<RuntimeState>,
    ledger: Option<ledger::Ledger>,
}
#[derive(Default, Clone, Serialize, Deserialize)]
struct Registry {
    revision: u64,
    entries: BTreeMap<String, Registration>,
    tasks: BTreeMap<String, tasks::TaskRecord>,
    installations: BTreeMap<String, InstalledPlugin>,
    signers: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Registration {
    manifest: PluginManifest,
    digest: String,
    generation: u64,
    signer: String,
    enabled: bool,
    files: Vec<FileEntry>,
    granted_budget: ResourceBudget,
}
#[derive(Default)]
struct RuntimeState {
    maintenance: bool,
    processes: BTreeMap<String, Arc<PluginProcess>>,
    starting: BTreeMap<String, Arc<ProcessStart>>,
    mutating: BTreeSet<String>,
    contexts: BTreeMap<String, ContextBinding>,
    exports: BTreeMap<String, (String, PathBuf)>,
    diagnostics: std::collections::VecDeque<Arc<diagnostics::DiagnosticBuffer>>,
}

#[derive(Clone)]
struct ContextBinding {
    context: CallContext,
    window: String,
    view_id: String,
}
impl PluginManager {
    pub fn initialize(app_data_dir: impl AsRef<Path>, services: Arc<dyn HostServices>) -> Self {
        match Self::new(app_data_dir.as_ref(), services.clone()) {
            Ok(manager) => manager,
            Err(error) => Self {
                inner: Arc::new(ManagerInner {
                    initialization_error: Some(error),
                    root: app_data_dir.as_ref().join("extensions"),
                    services,
                    registry: Mutex::new(Registry::default()),
                    commit: Mutex::new(()),
                    state: Mutex::new(RuntimeState::default()),
                    ledger: None,
                }),
            },
        }
    }
    fn available(&self) -> Result<(), PluginFailure> {
        self.inner.initialization_error.clone().map_or(Ok(()), Err)
    }
    pub fn new(
        app_data_dir: impl AsRef<Path>,
        services: Arc<dyn HostServices>,
    ) -> Result<Self, PluginFailure> {
        let root = app_data_dir.as_ref().join("extensions");
        for directory in ["packages", "data", "staging"] {
            fs::create_dir_all(root.join(directory)).map_err(|_| fail("plugin_storage_failed"))?;
        }
        let root = fs::canonicalize(root).map_err(|_| fail("plugin_storage_failed"))?;
        let ledger = ledger::Ledger::open(&root)?;
        let mut registry = ledger.load()?.unwrap_or_default();
        for entry in registry.entries.values_mut() {
            entry.granted_budget = entry.manifest.resource_budget.grant()?;
        }
        for task in registry
            .tasks
            .values_mut()
            .filter(|task| !task.snapshot.state.terminal())
        {
            task.snapshot.state = TaskState::OutcomeUnknown;
            task.snapshot.error = Some(fail("plugin_process_restarted"));
        }
        let registry = ledger.write(registry)?;
        ledger.prune()?;
        let manager = Self {
            inner: Arc::new(ManagerInner {
                initialization_error: None,
                root,
                services,
                registry: Mutex::new(registry),
                commit: Mutex::new(()),
                state: Mutex::new(RuntimeState::default()),
                ledger: Some(ledger),
            }),
        };
        let _ = manager.collect_garbage();
        Ok(manager)
    }
    fn ledger(&self) -> Result<&ledger::Ledger, PluginFailure> {
        self.available()?;
        self.inner
            .ledger
            .as_ref()
            .ok_or_else(|| fail("plugin_storage_failed"))
    }
    fn registration(&self, id: &str) -> Result<Registration, PluginFailure> {
        self.available()?;
        if !valid_id(id) {
            return Err(fail("plugin_id_invalid"));
        }
        self.inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .entries
            .get(id)
            .cloned()
            .ok_or_else(|| fail("plugin_not_installed"))
    }
    pub fn manifest(&self, id: &str) -> Result<PluginManifest, PluginFailure> {
        Ok(self.registration(id)?.manifest)
    }
    pub fn list(&self) -> Result<Vec<InstalledPlugin>, PluginFailure> {
        self.available()?;
        let entries = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .entries
            .clone();
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        Ok(entries
            .into_iter()
            .map(|(id, entry)| InstalledPlugin {
                granted_budget: entry.granted_budget,
                signer_key: entry.signer,
                manifest: entry.manifest,
                package_digest: entry.digest,
                installation_generation: entry.generation.to_string(),
                enabled: entry.enabled,
                process_state: state
                    .processes
                    .get(&id)
                    .map_or("stopped", |process| {
                        if process.is_running() {
                            "running"
                        } else {
                            "crashed"
                        }
                    })
                    .into(),
            })
            .collect())
    }
    fn update_registry(
        &self,
        update: impl FnOnce(&mut Registry) -> Result<(), PluginFailure>,
    ) -> Result<(), PluginFailure> {
        self.available()?;
        let _commit = self
            .inner
            .commit
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        let mut next = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .clone();
        update(&mut next)?;
        let next = self.ledger()?.write(next)?;
        *self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))? = next;
        Ok(())
    }
    fn reserve(&self, id: &str) -> Result<Reservation, PluginFailure> {
        self.available()?;
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        if state.maintenance
            || state.mutating.contains(id)
            || state.starting.contains_key(id)
            || state
                .processes
                .get(id)
                .is_some_and(|process| process.busy())
        {
            return Err(fail("plugin_busy"));
        }
        state.mutating.insert(id.into());
        Ok(Reservation {
            manager: self.clone(),
            id: id.into(),
        })
    }
}
fn fail(code: &str) -> PluginFailure {
    PluginFailure::new(code)
}
struct Reservation {
    manager: PluginManager,
    id: String,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if let Ok(mut state) = self.manager.inner.state.lock() {
            state.mutating.remove(&self.id);
        }
    }
}
