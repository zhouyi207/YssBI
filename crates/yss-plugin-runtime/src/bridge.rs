use crate::{
    ContextBinding, PluginManager, RuntimeState, fail, package, read_bounded, resolve_data_file,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Duration};
use yss_plugin_protocol::{
    CallContext, MAX_ASSET_BYTES, PluginFailure, RpcRequest, ViewScope, ViewSession,
};

impl RuntimeState {
    fn ensure_active_process(&self, context: &CallContext) -> Result<(), PluginFailure> {
        if !self
            .processes
            .get(&context.plugin_id)
            .is_some_and(|process| {
                process.is_running() && process.instance_id == context.instance_id
            })
        {
            return Err(fail("plugin_process_exited"));
        }
        Ok(())
    }

    fn active_context(&self, id: &str) -> Result<&ContextBinding, PluginFailure> {
        let binding = self
            .contexts
            .get(id)
            .ok_or_else(|| fail("plugin_stale_context"))?;
        self.ensure_active_process(&binding.context)?;
        Ok(binding)
    }
}

impl PluginManager {
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
        state.ensure_active_process(&context)?;
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
        // Detach and process failure revoke contexts under this same lock.
        state.active_context(session_id)?;
        if state.exports.len() >= 16 {
            return Err(fail("plugin_resource_exhausted"));
        }
        state.exports.insert(id.clone(), (session_id.into(), path));
        Ok(id)
    }
    pub(super) fn context(&self, id: &str) -> Result<ContextBinding, PluginFailure> {
        let binding = {
            let state = self
                .inner
                .state
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?;
            state.active_context(id)?.clone()
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
    pub(super) fn host_request(
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
            if fs::copy(source, &temporary).is_err() {
                let _ = fs::remove_file(&temporary);
                return Err(fail("plugin_export_failed"));
            }
            package::finish_file_publication(
                &temporary,
                atomicwrites::replace_atomic(&temporary, &target),
            )?;
            return Ok(Value::Null);
        }
        let response = self.inner.services.invoke(
            &binding.context,
            &request.method,
            request.params["input"].clone(),
            &self.inner.root.join("data").join(plugin),
        )?;
        if matches!(request.method.as_str(), "data.list" | "data.snapshot")
            && let Err(error) = self.context(context_id)
        {
            // A revoked read may register host resources after the first context release.
            self.inner.services.release_context(context_id);
            return Err(error);
        }
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
                let encoded =
                    serde_json::to_vec(&input).map_err(|_| fail("plugin_state_invalid"))?;
                if encoded.len() > 64 * 1024 {
                    return Err(fail("plugin_resource_exhausted"));
                }
                let incoming = encoded.len() as u64;
                let previous = fs::metadata(&path).map_or(0, |metadata| metadata.len());
                self.enforce_private_budget(id, incoming.saturating_sub(previous))?;
                package::atomic_bytes(&path, &encoded)?;
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
