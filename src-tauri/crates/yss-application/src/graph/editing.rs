//! Live graph editing against Project's current resident document.
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};
use yss_graph_document::GraphDocumentPatch;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_document_edit::apply_graph_document_patch;
use yss_graph_editor::{EditorGraphMutation, MutationConflict};
use yss_project::{GraphEditVersion, GraphEditingState, GraphHistoryAction};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

use super::edit::{GraphDocumentChange, GraphDocumentEditor};
use super::resources::ResourceMutationApplicationError;
use crate::session::ApplicationState;

#[derive(Clone)]
pub enum GraphActivity {
    Changed {
        graph_path: String,
        editing: GraphEditingState,
    },
    Execution(super::run::RunApplicationEvent),
}

pub type GraphActivityObserver = Arc<dyn Fn(GraphActivity) + Send + Sync>;

#[derive(Default)]
pub(crate) struct GraphActivitySource {
    observers: Arc<Mutex<BTreeMap<uuid::Uuid, GraphActivityObserver>>>,
}

pub struct GraphActivitySubscription {
    source: Weak<Mutex<BTreeMap<uuid::Uuid, GraphActivityObserver>>>,
    id: uuid::Uuid,
}

impl Drop for GraphActivitySubscription {
    fn drop(&mut self) {
        if let Some(source) = self.source.upgrade() {
            source
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(&self.id);
        }
    }
}

impl GraphActivitySource {
    pub(crate) fn publish(&self, activity: GraphActivity) {
        let observers = self
            .observers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .map(|(id, observer)| (*id, observer.clone()))
            .collect::<Vec<_>>();
        for (id, observer) in observers {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observer(activity.clone())))
                .is_err()
            {
                self.observers
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .remove(&id);
            }
        }
    }

    pub(crate) fn subscribe(
        &self,
        observer: GraphActivityObserver,
    ) -> Result<GraphActivitySubscription, ResourceMutationApplicationError> {
        let mut observers = self
            .observers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if observers.len() >= 64 {
            return Err(ResourceMutationApplicationError::EditingBusy);
        }
        let id = uuid::Uuid::new_v4();
        observers.insert(id, observer);
        Ok(GraphActivitySubscription {
            source: Arc::downgrade(&self.observers),
            id,
        })
    }
}

#[derive(Default)]
pub(crate) struct GraphEditingCoordinator(Mutex<BTreeMap<GraphResourcePath, Weak<EditGate>>>);

#[derive(Default)]
struct EditGate {
    queue: Mutex<(bool, VecDeque<uuid::Uuid>)>,
    available: Condvar,
}

pub(crate) struct GraphEditingPermit(Arc<EditGate>);

#[derive(Debug, thiserror::Error)]
#[error("graph editing queue is busy")]
pub(crate) struct GraphEditingBusy;

impl Drop for GraphEditingPermit {
    fn drop(&mut self) {
        self.0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .0 = false;
        self.0.available.notify_all();
    }
}

impl GraphEditingCoordinator {
    /// Reservations serialize publication without holding a data lock during Resolve or file I/O.
    pub(crate) fn acquire(
        &self,
        path: &GraphResourcePath,
    ) -> Result<GraphEditingPermit, GraphEditingBusy> {
        let gate = {
            let mut gates = self.0.lock().unwrap_or_else(|error| error.into_inner());
            if gates.len() >= 128 {
                gates.retain(|_, gate| gate.strong_count() > 0);
            }
            match gates.get(path).and_then(Weak::upgrade) {
                Some(gate) => gate,
                None => {
                    if gates.len() >= 128 {
                        return Err(GraphEditingBusy);
                    }
                    let gate = Arc::new(EditGate::default());
                    gates.insert(path.clone(), Arc::downgrade(&gate));
                    gate
                }
            }
        };
        let mut queue = gate.queue.lock().unwrap_or_else(|error| error.into_inner());
        if queue.1.len() >= 64 {
            return Err(GraphEditingBusy);
        }
        let ticket = uuid::Uuid::new_v4();
        queue.1.push_back(ticket);
        let deadline = Instant::now() + Duration::from_secs(15);
        while queue.0 || queue.1.front() != Some(&ticket) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                queue.1.retain(|pending| *pending != ticket);
                gate.available.notify_all();
                return Err(GraphEditingBusy);
            }
            queue = gate
                .available
                .wait_timeout(queue, remaining)
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
        queue.1.pop_front();
        queue.0 = true;
        drop(queue);
        Ok(GraphEditingPermit(gate))
    }
}

pub struct GraphEditRequest {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub version: GraphEditVersion,
    pub operation_id: OperationId,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphEditResponse {
    pub update: GraphDocumentChange,
    pub editing: GraphEditingState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphSaveResponse {
    pub project_instance_id: ProjectInstanceId,
    pub resource_revision: ResourceRevision,
    pub graph: GraphEditResponse,
}

impl ApplicationState {
    pub fn current_graph_document(
        &self,
        project: &ProjectInstanceId,
        path: &GraphResourcePath,
        version: GraphEditVersion,
    ) -> Result<Arc<GraphDocument>, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(project)?;
        let snapshot = captured.project().read_graph_editing(project, path)?;
        if snapshot.state.version != version {
            return Err(ResourceMutationApplicationError::GraphOperation(
                yss_project::ProjectGraphOperationError::RevisionConflict {
                    graph: path.clone(),
                    expected: version.revision,
                    current: snapshot.state.version.revision,
                },
            ));
        }
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(snapshot.document)
    }

    pub fn edit_graph(
        &self,
        request: GraphEditRequest,
        mutation: EditorGraphMutation,
    ) -> Result<GraphEditResponse, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&request.project_instance_id)?;
        let _editing = captured
            .coordinate_graph_edit(&request.graph_path)
            .map_err(|_| ResourceMutationApplicationError::EditingBusy)?;
        let operation = captured
            .project()
            .capture_graph_edit(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
                request.operation_id,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let mut editor = GraphDocumentEditor::new(
            &captured,
            &request.graph_path,
            &request.locale,
            (*operation.document).clone(),
        )?;
        editor.apply(mutation)?;
        let update = editor.finish(self)?;
        let receipt = captured
            .project()
            .commit_graph_edit(
                operation,
                Arc::new(update.document.clone()),
                GraphHistoryAction::Edit(update.patch.clone()),
            )
            .map_err(ResourceMutationApplicationError::GraphCommit)?;
        captured
            .execution()
            .observe_graph_result_inputs(request.graph_path.as_str(), update.result_inputs.clone());
        captured.publish_graph_activity(GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: receipt.editing.clone(),
        });
        Ok(GraphEditResponse {
            update,
            editing: receipt.editing,
        })
    }

    pub fn resolve_editor_graph(
        &self,
        project: ProjectInstanceId,
        path: GraphResourcePath,
        version: GraphEditVersion,
        locale: String,
    ) -> Result<GraphEditResponse, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project)?;
        let _editing = captured
            .coordinate_graph_edit(&path)
            .map_err(|_| ResourceMutationApplicationError::EditingBusy)?;
        let snapshot = captured.project().read_graph_editing(&project, &path)?;
        if snapshot.state.version != version {
            return Err(ResourceMutationApplicationError::GraphOperation(
                yss_project::ProjectGraphOperationError::RevisionConflict {
                    graph: path,
                    expected: version.revision,
                    current: snapshot.state.version.revision,
                },
            ));
        }
        let editor =
            GraphDocumentEditor::new(&captured, &path, &locale, (*snapshot.document).clone())?;
        let update = editor.finish(self)?;
        captured
            .execution()
            .observe_graph_result_inputs(path.as_str(), update.result_inputs.clone());
        Ok(GraphEditResponse {
            update,
            editing: snapshot.state,
        })
    }

    pub fn change_graph_history(
        &self,
        request: GraphEditRequest,
        redo: bool,
    ) -> Result<GraphEditResponse, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&request.project_instance_id)?;
        let _editing = captured
            .coordinate_graph_edit(&request.graph_path)
            .map_err(|_| ResourceMutationApplicationError::EditingBusy)?;
        let operation = captured
            .project()
            .capture_graph_edit(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
                request.operation_id,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let patch = captured.project().graph_history_patch(
            &request.project_instance_id,
            &request.graph_path,
            request.version,
            redo,
        )?;
        let mut document = (*operation.document).clone();
        if let Some(patch) = &patch {
            apply_graph_document_patch(&mut document, patch).map_err(|error| {
                ResourceMutationApplicationError::Mutation(MutationConflict::Document(error))
            })?;
        }
        let editor =
            GraphDocumentEditor::new(&captured, &request.graph_path, &request.locale, document)?;
        let mut update = editor.finish(self)?;
        update.changed = patch.is_some();
        let action = if patch.is_none() {
            GraphHistoryAction::Edit(GraphDocumentPatch::new(Vec::new()))
        } else if redo {
            GraphHistoryAction::Redo
        } else {
            GraphHistoryAction::Undo
        };
        let receipt = captured
            .project()
            .commit_graph_edit(operation, Arc::new(update.document.clone()), action)
            .map_err(ResourceMutationApplicationError::GraphCommit)?;
        captured
            .execution()
            .observe_graph_result_inputs(request.graph_path.as_str(), update.result_inputs.clone());
        captured.publish_graph_activity(GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: receipt.editing.clone(),
        });
        Ok(GraphEditResponse {
            update,
            editing: receipt.editing,
        })
    }

    pub fn save_current_graph(
        &self,
        request: GraphEditRequest,
    ) -> Result<GraphSaveResponse, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&request.project_instance_id)?;
        let _editing = captured
            .coordinate_graph_edit(&request.graph_path)
            .map_err(|_| ResourceMutationApplicationError::EditingBusy)?;
        let operation = captured
            .project()
            .capture_graph_edit(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
                request.operation_id,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let editor = GraphDocumentEditor::new(
            &captured,
            &request.graph_path,
            &request.locale,
            (*operation.document).clone(),
        )?;
        let update = editor.finish(self)?;
        let receipt = captured
            .project()
            .save_graph_candidate(operation, Arc::new(update.document.clone()))
            .map_err(|error| match error {
                yss_project::ProjectGraphSaveError::Filesystem(error) => {
                    ResourceMutationApplicationError::Project(error)
                }
                yss_project::ProjectGraphSaveError::Commit(error) => {
                    ResourceMutationApplicationError::GraphCommit(error)
                }
            })?;
        captured.publish_graph_activity(GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: receipt.editing.clone(),
        });
        Ok(GraphSaveResponse {
            project_instance_id: request.project_instance_id,
            resource_revision: receipt.to_revision,
            graph: GraphEditResponse {
                update,
                editing: receipt.editing,
            },
        })
    }

    pub fn subscribe_graph_activity(
        &self,
        project: &ProjectInstanceId,
        observer: GraphActivityObserver,
    ) -> Result<GraphActivitySubscription, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(project)?;
        captured.subscribe_graph_activity(observer)
    }

    pub fn execution_run_state(
        &self,
        project: &ProjectInstanceId,
        execution_session: &str,
        run_id: u64,
    ) -> Result<Option<yss_graph_execution::run_registry::RunState>, ResourceMutationApplicationError>
    {
        let captured = self.capture_resource_session(project)?;
        if captured.execution_session_id().as_uuid().to_string() != execution_session {
            return Err(ResourceMutationApplicationError::SessionChanged(
                crate::session::SessionRevalidationError::Changed,
            ));
        }
        Ok(captured.execution().runs().state(
            yss_graph_execution::run_registry::RunId::from_existing(run_id),
        ))
    }
}
