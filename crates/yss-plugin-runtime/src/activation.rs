use crate::{PluginManager, Registration, diagnostics, fail, package, process::PluginProcess};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, atomic::Ordering},
    time::Duration,
};
use yss_plugin_protocol::{PluginFailure, RpcRequest};

#[derive(Default)]
struct StartOutcome {
    result: Option<Result<Arc<PluginProcess>, PluginFailure>>,
    waiters: usize,
}
#[derive(Default)]
pub(super) struct ProcessStart {
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
    pub(super) fn stop_process(&self, id: &str) {
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
    pub(super) fn revoke_contexts(&self, id: &str) {
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
