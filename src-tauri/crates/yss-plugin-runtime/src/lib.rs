mod diagnostics;
mod ledger;
mod package;
mod process;
mod storage;
mod tasks;
use process::PluginProcess;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, atomic::Ordering},
    time::Duration,
};

fn same_release(left: &str, right: &str) -> Result<bool, PluginFailure> {
    let left = semver::Version::parse(left).map_err(|_| fail("plugin_manifest_invalid"))?;
    let right = semver::Version::parse(right).map_err(|_| fail("plugin_manifest_invalid"))?;
    Ok(left.major == right.major
        && left.minor == right.minor
        && left.patch == right.patch
        && left.pre == right.pre)
}
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
    #[serde(default)]
    tasks: BTreeMap<String, tasks::TaskRecord>,
    #[serde(default)]
    installations: BTreeMap<String, InstalledPlugin>,
    #[serde(default)]
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
    #[serde(default)]
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

#[derive(Default)]
struct StartOutcome {
    result: Option<Result<Arc<PluginProcess>, PluginFailure>>,
    waiters: usize,
}
#[derive(Default)]
struct ProcessStart {
    outcome: Mutex<StartOutcome>,
    ready: Condvar,
}
impl ProcessStart {
    fn complete(&self, result: Result<Arc<PluginProcess>, PluginFailure>) {
        self.outcome
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .result = Some(result);
        self.ready.notify_all();
    }
    fn wait(&self, limit: usize) -> Result<Arc<PluginProcess>, PluginFailure> {
        let mut state = self
            .outcome
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        if state.waiters >= limit {
            return Err(fail("plugin_resource_exhausted"));
        }
        state.waiters += 1;
        let (mut state, _) = self
            .ready
            .wait_timeout_while(state, Duration::from_secs(30), |state| {
                state.result.is_none()
            })
            .map_err(|_| fail("plugin_state_unavailable"))?;
        state.waiters -= 1;
        state
            .result
            .clone()
            .unwrap_or_else(|| Err(fail("plugin_start_timeout")))
    }
}

#[cfg(test)]
mod activation_tests {
    use super::*;
    #[test]
    fn startup_failure_wakes_every_waiter_with_the_same_result() {
        let flight = Arc::new(ProcessStart::default());
        assert_eq!(
            flight.wait(0).err().unwrap().code,
            "plugin_resource_exhausted"
        );
        let (ready, receiver) = std::sync::mpsc::channel();
        let waiters = (0..3)
            .map(|_| {
                let flight = flight.clone();
                let ready = ready.clone();
                std::thread::spawn(move || {
                    ready.send(()).unwrap();
                    flight.wait(3).err().unwrap()
                })
            })
            .collect::<Vec<_>>();
        for _ in 0..3 {
            receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        flight.complete(Err(fail("plugin_start_failed")));
        for waiter in waiters {
            assert_eq!(waiter.join().unwrap(), fail("plugin_start_failed"));
        }
    }
}
#[derive(Clone)]
struct ContextBinding {
    context: CallContext,
    window: String,
    view_id: String,
}
pub struct PluginLease {
    pub process: Arc<PluginProcess>,
    pub data_dir: PathBuf,
}
impl Drop for PluginLease {
    fn drop(&mut self) {
        self.process.leases.fetch_sub(1, Ordering::AcqRel);
    }
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
        let mut registry: Registry = if let Some(registry) = ledger.load()? {
            registry
        } else if root.join("registry.json").exists() {
            serde_json::from_slice(&read_bounded(
                &root.join("registry.json"),
                16 * 1024 * 1024,
            )?)
            .map_err(|_| fail("plugin_registry_invalid"))?
        } else {
            Registry::default()
        };
        for (id, entry) in &mut registry.entries {
            entry.granted_budget = entry.manifest.resource_budget.grant()?;
            registry
                .signers
                .entry(id.clone())
                .or_insert_with(|| entry.signer.clone());
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
    pub fn inspect(&self, path: &Path) -> Result<PackageInspection, PluginFailure> {
        let mut description = package::inspect(path)?.description;
        description.previous_signer_key = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .signers
            .get(&description.manifest.id)
            .cloned();
        Ok(description)
    }
    pub fn install(
        &self,
        path: &Path,
        expected_digest: &str,
        operation_id: &str,
        approve_native: bool,
        approved_previous_signer: Option<&str>,
    ) -> Result<InstalledPlugin, PluginFailure> {
        if !approve_native {
            return Err(fail("plugin_trust_required"));
        }
        self.available()?;
        self.ledger()?.prune()?;
        if let Some(receipt) = self.ledger()?.installation(operation_id)? {
            if receipt.package_digest != expected_digest {
                return Err(fail("plugin_operation_conflict"));
            }
            return Ok(receipt);
        }
        validate_operation_id(operation_id, ledger::now_ms())?;
        let inspected = package::inspect(path)?;
        if inspected.description.package_digest != expected_digest {
            return Err(fail("plugin_package_changed"));
        }
        let id = inspected.description.manifest.id.clone();
        let reservation = self.reserve(&id)?;
        let previous_signer = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .signers
            .get(&id)
            .cloned();
        if previous_signer
            .as_deref()
            .is_some_and(|signer| signer != inspected.description.signer_key)
            && approved_previous_signer != previous_signer.as_deref()
        {
            return Err(fail("plugin_signer_change_requires_approval"));
        }
        if let Ok(previous) = self.registration(&id)
            && same_release(
                &previous.manifest.version,
                &inspected.description.manifest.version,
            )?
            && previous.digest != expected_digest
        {
            return Err(fail("plugin_version_content_conflict"));
        }
        let granted_budget = inspected.description.manifest.resource_budget.grant()?;
        let staging = self
            .inner
            .root
            .join("staging")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&staging).map_err(|_| fail("plugin_storage_failed"))?;
        let target = package::package_path(&self.inner.root, expected_digest)?;
        let result = (|| {
            package::extract(path, &staging, &inspected)?;
            if !target.exists() {
                fs::rename(&staging, &target).map_err(|_| fail("plugin_storage_failed"))?;
            }
            self.stop_process(&id);
            self.update_registry(|registry| {
                if let Some(previous) = self.ledger()?.installation(operation_id)? {
                    return if previous.package_digest == expected_digest {
                        Ok(())
                    } else {
                        Err(fail("plugin_operation_conflict"))
                    };
                }
                registry.revision += 1;
                registry
                    .signers
                    .insert(id.clone(), inspected.description.signer_key.clone());
                registry.entries.insert(
                    id.clone(),
                    Registration {
                        manifest: inspected.description.manifest.clone(),
                        digest: expected_digest.into(),
                        generation: registry.revision,
                        signer: inspected.description.signer_key.clone(),
                        enabled: true,
                        files: inspected.files.clone(),
                        granted_budget: granted_budget.clone(),
                    },
                );
                registry.installations.insert(
                    operation_id.into(),
                    InstalledPlugin {
                        manifest: inspected.description.manifest.clone(),
                        package_digest: expected_digest.into(),
                        installation_generation: registry.revision.to_string(),
                        enabled: true,
                        process_state: "stopped".into(),
                        granted_budget: granted_budget.clone(),
                        signer_key: inspected.description.signer_key.clone(),
                    },
                );
                Ok(())
            })?;
            self.revoke_contexts(&id);
            let receipt = self
                .list()?
                .into_iter()
                .find(|entry| entry.manifest.id == id)
                .ok_or_else(|| fail("plugin_registry_invalid"))?;
            Ok(receipt)
        })();
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        drop(reservation);
        let _ = self.collect_garbage();
        result
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), PluginFailure> {
        let reservation = self.reserve(id)?;
        self.stop_process(id);
        self.update_registry(|registry| {
            registry.revision += 1;
            let entry = registry
                .entries
                .get_mut(id)
                .ok_or_else(|| fail("plugin_not_installed"))?;
            entry.enabled = enabled;
            entry.generation = registry.revision;
            Ok(())
        })?;
        self.revoke_contexts(id);
        drop(reservation);
        Ok(())
    }
    pub fn uninstall(&self, id: &str) -> Result<(), PluginFailure> {
        let reservation = self.reserve(id)?;
        self.stop_process(id);
        self.update_registry(|registry| {
            registry
                .entries
                .remove(id)
                .ok_or_else(|| fail("plugin_not_installed"))?;
            registry.revision += 1;
            Ok(())
        })?;
        self.revoke_contexts(id);
        // Content-addressed packages and private data remain recoverable. They
        // are never removed together with project resources or system runtimes.
        drop(reservation);
        Ok(())
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
    fn stop_process(&self, id: &str) {
        let process = self
            .inner
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.processes.remove(id));
        if let Some(process) = process {
            process.stop();
        }
    }
    fn revoke_contexts(&self, id: &str) {
        let removed = if let Ok(mut state) = self.inner.state.lock() {
            let removed = state
                .contexts
                .iter()
                .filter(|(_, binding)| binding.context.plugin_id == id)
                .map(|(id, _)| id.clone())
                .collect::<BTreeSet<_>>();
            state
                .contexts
                .retain(|context, _| !removed.contains(context));
            state
                .exports
                .retain(|_, (context, _)| !removed.contains(context));
            removed
        } else {
            BTreeSet::new()
        };
        for context in removed {
            self.inner.services.release_context(&context);
        }
    }
    pub fn acquire(&self, id: &str) -> Result<PluginLease, PluginFailure> {
        self.enforce_private_budget(id, 0)?;
        let data_dir = self.inner.root.join("data").join(id);
        let (entry, flight, leader, retired) = {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
            if state.maintenance || state.mutating.contains(id) {
                return Err(fail("plugin_busy"));
            }
            // Registry reads are memory-only. The state lock excludes mutation admission
            // until we have either granted a lease or published the single startup flight.
            let entry = self.registration(id)?;
            if !entry.enabled {
                return Err(fail("plugin_disabled"));
            }
            if let Some(process) = state
                .processes
                .get(id)
                .filter(|process| process.is_running())
            {
                process.leases.fetch_add(1, Ordering::AcqRel);
                return Ok(PluginLease {
                    process: process.clone(),
                    data_dir,
                });
            }
            if let Some(flight) = state.starting.get(id) {
                (entry, flight.clone(), false, None)
            } else {
                let flight = Arc::new(ProcessStart::default());
                let retired = state.processes.remove(id);
                state.starting.insert(id.into(), flight.clone());
                (entry, flight, true, retired)
            }
        };
        if !leader {
            let process = flight.wait(entry.granted_budget.pending_requests as usize)?;
            let state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
            if state.mutating.contains(id) {
                return Err(fail("plugin_busy"));
            }
            if !process.is_running()
                || !state
                    .processes
                    .get(id)
                    .is_some_and(|current| Arc::ptr_eq(current, &process))
            {
                return Err(fail("plugin_stale_context"));
            }
            process.leases.fetch_add(1, Ordering::AcqRel);
            return Ok(PluginLease { process, data_dir });
        }
        // Process shutdown, file verification, lease release and handshake never hold the registry/state lock.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(process) = retired {
                process.stop();
            }
            self.revoke_contexts(id);
            self.start_process(&entry, &data_dir)
        }))
        .unwrap_or_else(|_| Err(fail("plugin_start_failed")));
        let published = match self.inner.state.lock() {
            Ok(mut state) => {
                if let Ok(process) = &result {
                    process.leases.fetch_add(1, Ordering::AcqRel);
                    state.processes.insert(id.into(), process.clone());
                }
                state.starting.remove(id);
                flight.complete(result.clone());
                result
            }
            Err(_) => {
                let error = fail("plugin_state_unavailable");
                flight.complete(Err(error.clone()));
                Err(error)
            }
        };
        published.map(|process| PluginLease { process, data_dir })
    }

    fn start_process(
        &self,
        entry: &Registration,
        data_dir: &Path,
    ) -> Result<Arc<PluginProcess>, PluginFailure> {
        let package = package::package_path(&self.inner.root, &entry.digest)?;
        for file in &entry.files {
            package::verify_file(&package, file)?;
        }
        fs::create_dir_all(data_dir).map_err(|_| fail("plugin_storage_failed"))?;
        let instance_id = uuid::Uuid::new_v4().to_string();
        let diagnostics = Arc::new(diagnostics::DiagnosticBuffer::new(
            entry.manifest.id.clone(),
            instance_id.clone(),
        ));
        {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
            while state
                .diagnostics
                .iter()
                .filter(|buffer| buffer.plugin == entry.manifest.id)
                .count()
                >= 4
            {
                let oldest = state
                    .diagnostics
                    .iter()
                    .position(|buffer| buffer.plugin == entry.manifest.id)
                    .unwrap();
                state.diagnostics.remove(oldest);
            }
            state.diagnostics.push_back(diagnostics.clone());
            while state.diagnostics.len() > 64 {
                state.diagnostics.pop_front();
            }
        }
        let weak = Arc::downgrade(&self.inner);
        let plugin_id = entry.manifest.id.clone();
        let handler_instance = instance_id.clone();
        let handler = Arc::new(move |_: &yss_plugin_sdk::Peer, request: RpcRequest| {
            let inner = weak
                .upgrade()
                .ok_or_else(|| fail("plugin_process_exited"))?;
            PluginManager { inner }.host_request(&plugin_id, &handler_instance, request)
        });
        let mut effective_manifest = entry.manifest.clone();
        effective_manifest.resource_budget = entry.granted_budget.clone();
        PluginProcess::spawn(
            &effective_manifest,
            &package,
            data_dir,
            instance_id,
            handler,
            diagnostics,
        )
    }
    pub fn attach_view(
        &self,
        id: &str,
        view_id: &str,
        window: &str,
    ) -> Result<ViewSession, PluginFailure> {
        let entry = self.registration(id)?;
        let view = entry
            .manifest
            .contributes
            .views
            .iter()
            .find(|view| view.id == view_id)
            .ok_or_else(|| fail("plugin_view_missing"))?;
        let root = package::package_path(&self.inner.root, &entry.digest)?;
        let asset = entry
            .files
            .iter()
            .find(|file| file.path == view.entry)
            .ok_or_else(|| fail("plugin_asset_invalid"))?;
        package::verify_file(&root, asset)?;
        let html = String::from_utf8(read_bounded(
            &resolve_data_file(&root, &view.entry)?,
            MAX_ASSET_BYTES,
        )?)
        .map_err(|_| fail("plugin_asset_invalid"))?;
        // The host enforces policy even when a signed publisher omits its own CSP.
        let html = format!(
            "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; font-src data:; img-src data:; connect-src 'none'; base-uri 'none'; form-action 'none'\">{html}"
        );
        let lease = self.acquire(id)?;
        if self.registration(id)?.generation != entry.generation {
            return Err(fail("plugin_stale_context"));
        }
        let context = CallContext {
            context_id: uuid::Uuid::new_v4().to_string(),
            plugin_id: id.into(),
            installation_generation: entry.generation.to_string(),
            instance_id: lease.process.instance_id.clone(),
            package_digest: entry.digest.clone(),
            project: if view.scope == ViewScope::Project {
                self.inner.services.current_project()?
            } else {
                None
            },
            task_id: None,
            operation_id: None,
            parameters_hash: None,
            granted_budget: entry.granted_budget.clone(),
        };
        let session_id = context.context_id.clone();
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        if state
            .contexts
            .values()
            .filter(|binding| binding.context.plugin_id == id && binding.context.task_id.is_none())
            .count()
            >= entry.granted_budget.views as usize
        {
            return Err(fail("plugin_view_limit"));
        }
        state.contexts.insert(
            session_id.clone(),
            ContextBinding {
                context,
                window: window.into(),
                view_id: view_id.into(),
            },
        );
        Ok(ViewSession {
            session_id,
            html,
            installation_generation: entry.generation.to_string(),
        })
    }
    pub fn detach_view(&self, session_id: &str, window: &str) -> Result<(), PluginFailure> {
        let removed = {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
            if state.contexts.get(session_id).is_some_and(|binding| {
                binding.window != window || binding.context.task_id.is_some()
            }) {
                return Err(fail("plugin_permission_denied"));
            }
            if state.contexts.get(session_id).is_some_and(|binding| {
                binding.window == window && binding.context.task_id.is_none()
            }) {
                state.contexts.remove(session_id);
                state
                    .exports
                    .retain(|_, (context, _)| context != session_id);
                true
            } else {
                false
            }
        };
        if removed {
            self.inner.services.release_context(session_id);
        }
        Ok(())
    }
    pub fn grant_export(
        &self,
        session_id: &str,
        window: &str,
        path: PathBuf,
    ) -> Result<String, PluginFailure> {
        let binding = self.context(session_id)?;
        if binding.window != window
            || !path.is_absolute()
            || !self
                .manifest(&binding.context.plugin_id)?
                .permissions
                .iter()
                .any(|permission| permission == "files.export")
        {
            return Err(fail("plugin_permission_denied"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        if state.exports.len() >= 16 {
            return Err(fail("plugin_resource_exhausted"));
        }
        state.exports.insert(id.clone(), (session_id.into(), path));
        Ok(id)
    }
    fn context(&self, id: &str) -> Result<ContextBinding, PluginFailure> {
        let binding = {
            let state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
            let binding = state
                .contexts
                .get(id)
                .ok_or_else(|| fail("plugin_stale_context"))?;
            if !state
                .processes
                .get(&binding.context.plugin_id)
                .is_some_and(|process| {
                    process.is_running() && process.instance_id == binding.context.instance_id
                })
            {
                return Err(fail("plugin_process_exited"));
            }
            binding.clone()
        };
        let entry = self.registration(&binding.context.plugin_id)?;
        if !entry.enabled
            || entry.generation.to_string() != binding.context.installation_generation
            || binding.context.project.is_some()
                && binding.context.project != self.inner.services.current_project()?
        {
            return Err(fail("plugin_stale_context"));
        }
        Ok(binding)
    }
    fn host_request(
        &self,
        plugin: &str,
        instance: &str,
        request: RpcRequest,
    ) -> Result<Value, PluginFailure> {
        let context_id = request.params["context"]["contextId"]
            .as_str()
            .ok_or_else(|| fail("plugin_context_required"))?;
        let binding = self.context(context_id)?;
        if binding.context.plugin_id != plugin || binding.context.instance_id != instance {
            return Err(fail("plugin_permission_denied"));
        }
        let permission = match request.method.as_str() {
            "data.list" | "data.snapshot" | "data.release" => "data.read",
            "results.commit" => "results.write",
            "files.export" => "files.export",
            _ => return Err(fail("plugin_method_unknown")),
        };
        let entry = self.registration(plugin)?;
        if request.method == "data.snapshot" {
            self.enforce_private_budget(plugin, 0)?;
        }
        if !entry
            .manifest
            .permissions
            .iter()
            .any(|value| value == permission)
        {
            return Err(fail("plugin_permission_denied"));
        }
        if request.method == "files.export" {
            let grant = request.params["input"]["grant"]
                .as_str()
                .ok_or_else(|| fail("plugin_permission_denied"))?;
            let target = {
                let mut state = self
                    .inner
                    .state
                    .lock()
                    .map_err(|_| fail("plugin_state_unavailable"))?;
                if state
                    .exports
                    .get(grant)
                    .is_none_or(|(owner, _)| owner != context_id)
                {
                    return Err(fail("plugin_permission_denied"));
                }
                state
                    .exports
                    .remove(grant)
                    .map(|(_, path)| path)
                    .ok_or_else(|| fail("plugin_permission_denied"))?
            };
            let source = resolve_data_file(
                &self.inner.root.join("data").join(plugin),
                request.params["input"]["source"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_path_invalid"))?,
            )?;
            if fs::metadata(&source)
                .map_err(|_| fail("plugin_file_unavailable"))?
                .len()
                > binding.context.granted_budget.snapshot_bytes
            {
                return Err(fail("plugin_resource_exhausted"));
            }
            let temporary = target.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let result = fs::copy(source, &temporary)
                .and_then(|_| yss_file_replace::atomic_replace(&temporary, &target));
            if result.is_err() {
                let _ = fs::remove_file(temporary);
                return Err(fail("plugin_export_failed"));
            }
            return Ok(Value::Null);
        }
        let response = self.inner.services.invoke(
            &binding.context,
            &request.method,
            request.params["input"].clone(),
            &self.inner.root.join("data").join(plugin),
        )?;
        if request.method == "data.snapshot"
            && let Err(error) = self.enforce_private_budget(plugin, 0)
        {
            if let Some(lease) = response.get("leaseId") {
                let _ = self.inner.services.invoke(
                    &binding.context,
                    "data.release",
                    json!({"leaseId":lease}),
                    &self.inner.root.join("data").join(plugin),
                );
            }
            return Err(error);
        }
        Ok(response)
    }
    pub fn call_view(
        &self,
        session_id: &str,
        window: &str,
        method: &str,
        input: Value,
    ) -> Result<Value, PluginFailure> {
        let binding = self.context(session_id)?;
        if binding.window != window || binding.context.task_id.is_some() {
            return Err(fail("plugin_permission_denied"));
        }
        let id = &binding.context.plugin_id;
        let manifest = self.manifest(id)?;
        if !manifest.ui_methods.iter().any(|allowed| allowed == method) {
            return Err(fail("plugin_method_denied"));
        }
        let context = json!({"contextId":session_id,"remainingBudgetMs":30_000,"project":binding.context.project});
        match method {
            "views.get_state" | "views.set_state" => {
                let scope = package::hash(
                    &serde_json::to_vec(&binding.context.project)
                        .map_err(|_| fail("plugin_state_unavailable"))?,
                );
                let directory = self.inner.root.join("data").join(id).join("view-state");
                fs::create_dir_all(&directory).map_err(|_| fail("plugin_storage_failed"))?;
                let path = directory.join(format!("{}-{scope}.json", binding.view_id));
                if method == "views.get_state" {
                    return if path.exists() {
                        serde_json::from_slice(&read_bounded(&path, 64 * 1024)?)
                            .map_err(|_| fail("plugin_state_invalid"))
                    } else {
                        Ok(Value::Null)
                    };
                }
                if serde_json::to_vec(&input)
                    .map_err(|_| fail("plugin_state_invalid"))?
                    .len()
                    > 64 * 1024
                {
                    return Err(fail("plugin_resource_exhausted"));
                }
                let incoming = serde_json::to_vec(&input)
                    .map_err(|_| fail("plugin_state_invalid"))?
                    .len() as u64;
                let previous = fs::metadata(&path).map_or(0, |metadata| metadata.len());
                self.enforce_private_budget(id, incoming.saturating_sub(previous))?;
                package::atomic_json(&path, &input)?;
                Ok(Value::Null)
            }
            "system.save_file" => {
                if !manifest
                    .permissions
                    .iter()
                    .any(|permission| permission == "files.export")
                {
                    return Err(fail("plugin_permission_denied"));
                }
                Ok(json!({"hostUi":{"kind":"saveFile","options":input}}))
            }
            "system.reveal_artifact" => {
                let path = input["path"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_path_invalid"))?;
                let root = fs::canonicalize(self.inner.root.join("data").join(id))
                    .map_err(|_| fail("plugin_path_invalid"))?;
                let path = fs::canonicalize(path).map_err(|_| fail("plugin_path_invalid"))?;
                if !path.starts_with(&root) {
                    return Err(fail("plugin_permission_denied"));
                }
                Ok(json!({"hostUi":{"kind":"reveal","path":path}}))
            }
            "data.list" => self.host_request(
                id,
                &binding.context.instance_id,
                RpcRequest {
                    jsonrpc: "2.0".into(),
                    id: "view".into(),
                    method: method.into(),
                    params: json!({"context":context,"input":input}),
                },
            ),
            "tasks.start" => self.start_task(binding, input),
            "tasks.get" | "tasks.cancel" | "tasks.result" => {
                self.task_view_call(&binding, method, input)
            }
            "views.open" => {
                let view_id = input["viewId"]
                    .as_str()
                    .ok_or_else(|| fail("plugin_view_missing"))?;
                let view = manifest
                    .contributes
                    .views
                    .iter()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| fail("plugin_view_missing"))?;
                Ok(json!({"openView":{"pluginId":id,"view":view}}))
            }
            "commands.execute" | "dependencies.inspect" => {
                if method == "commands.execute"
                    && !manifest
                        .contributes
                        .commands
                        .iter()
                        .any(|command| input["commandId"].as_str() == Some(&command.id))
                {
                    return Err(fail("plugin_method_denied"));
                }
                let lease = self.acquire(id)?;
                if lease.process.instance_id != binding.context.instance_id {
                    return Err(fail("plugin_stale_context"));
                }
                lease.process.request(
                    method,
                    json!({"context":context,"input":input}),
                    Duration::from_secs(30),
                )
            }
            _ => Err(fail("plugin_method_unknown")),
        }
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
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, PluginFailure> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|_| fail("plugin_file_unavailable"))?;
    if file
        .metadata()
        .map_err(|_| fail("plugin_file_unavailable"))?
        .len()
        > limit
    {
        return Err(fail("plugin_payload_too_large"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("plugin_file_unavailable"))?;
    if bytes.len() as u64 > limit {
        return Err(fail("plugin_payload_too_large"));
    }
    Ok(bytes)
}
pub fn resolve_data_file(root: &Path, relative: &str) -> Result<PathBuf, PluginFailure> {
    if !valid_relative_path(relative) {
        return Err(fail("plugin_path_invalid"));
    }
    let meta = fs::symlink_metadata(root).map_err(|_| fail("plugin_path_invalid"))?;
    if meta.file_type().is_symlink() {
        return Err(fail("plugin_path_invalid"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(fail("plugin_path_invalid"));
        }
    }
    let root = fs::canonicalize(root).map_err(|_| fail("plugin_path_invalid"))?;
    let mut candidate = root.clone();
    for component in relative.split('/') {
        candidate.push(component);
        let meta = fs::symlink_metadata(&candidate).map_err(|_| fail("plugin_path_invalid"))?;
        if meta.file_type().is_symlink() {
            return Err(fail("plugin_path_invalid"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(fail("plugin_path_invalid"));
            }
        }
    }
    let path = fs::canonicalize(candidate).map_err(|_| fail("plugin_path_invalid"))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(fail("plugin_path_invalid"));
    }
    Ok(path)
}
