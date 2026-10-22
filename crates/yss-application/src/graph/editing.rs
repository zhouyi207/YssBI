//! Live graph editing against Project's current resident document.
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_document_edit::prepare_graph_document_patch;
use yss_graph_editor::{EditorGraphMutation, MutationConflict};
use yss_project::{GraphEditVersion, GraphEditingState, GraphHistoryAction};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

use super::edit::{GraphDocumentChange, GraphDocumentEditor};
use super::resources::ResourceMutationApplicationError;
use crate::session::{ApplicationSession, ApplicationState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphChangeCause {
    Edit { operation_id: OperationId },
    Dependencies,
    Results,
}

#[derive(Clone)]
pub enum GraphActivity {
    Changed {
        graph_path: String,
        editing: GraphEditingState,
        cause: GraphChangeCause,
    },
    Execution(super::run::RunApplicationEvent),
    /// Finished runs whose notices were dismissed; their records stay in the journal.
    RunsAcknowledged(Vec<super::run::RunIdentity>),
}

pub type GraphActivityObserver = Arc<dyn Fn(GraphActivity) + Send + Sync>;

/// Finished runs kept beyond the active ones and each graph's newest run.
const MAX_FINISHED_RUNS: usize = 100;

/// One run of the current execution session, as Application delivered it.
#[derive(Clone, Debug)]
pub struct RunRecord {
    started: super::run::RunApplicationEvent,
    terminal: Option<super::run::RunApplicationEvent>,
    started_at: std::time::SystemTime,
    finished_at: Option<std::time::SystemTime>,
    acknowledged: bool,
}

impl RunRecord {
    pub fn identity(&self) -> &super::run::RunIdentity {
        self.started.identity()
    }

    pub fn outputs(&self) -> &[yss_graph_execution::plan::PlanOutputRef] {
        match self.started.kind() {
            super::run::RunApplicationEventKind::RunStarted { outputs } => outputs,
            _ => &[],
        }
    }

    /// The terminal event once the run finished, otherwise its start.
    pub fn latest(&self) -> &super::run::RunApplicationEvent {
        self.terminal.as_ref().unwrap_or(&self.started)
    }

    pub fn events(&self) -> impl Iterator<Item = &super::run::RunApplicationEvent> {
        std::iter::once(&self.started).chain(self.terminal.as_ref())
    }

    pub fn started_at(&self) -> std::time::SystemTime {
        self.started_at
    }

    pub fn finished_at(&self) -> Option<std::time::SystemTime> {
        self.finished_at
    }

    pub fn acknowledged(&self) -> bool {
        self.acknowledged
    }
}

#[derive(Default)]
pub(crate) struct GraphActivitySource {
    observers: Arc<Mutex<BTreeMap<uuid::Uuid, GraphActivityObserver>>>,
    runs: Mutex<BTreeMap<yss_graph_execution::run_registry::RunId, RunRecord>>,
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
    /// Every active run and each graph's newest run, in run order.
    pub(crate) fn execution_records(&self) -> Vec<RunRecord> {
        let runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let newest = newest_runs(&runs);
        runs.iter()
            .filter(|(id, record)| {
                record.terminal.is_none() || newest.get(record.identity().graph_path()) == Some(*id)
            })
            .map(|(_, record)| record.clone())
            .collect()
    }

    pub(crate) fn execution_snapshot(&self) -> Vec<super::run::RunApplicationEvent> {
        self.execution_records()
            .iter()
            .flat_map(RunRecord::events)
            .cloned()
            .collect()
    }

    pub(crate) fn run_journal(&self) -> Vec<RunRecord> {
        self.runs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Marks finished runs as seen and returns the identities that changed.
    pub(crate) fn acknowledge(
        &self,
        identities: &[super::run::RunIdentity],
    ) -> Vec<super::run::RunIdentity> {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        identities
            .iter()
            .filter(|identity| {
                runs.get_mut(&identity.run_id()).is_some_and(|record| {
                    let changed = record.identity() == *identity
                        && record.terminal.is_some()
                        && !record.acknowledged;
                    record.acknowledged |= changed;
                    changed
                })
            })
            .cloned()
            .collect()
    }

    pub(crate) fn publish(&self, activity: GraphActivity) {
        if let GraphActivity::Execution(event) = &activity {
            use super::run::RunApplicationEventKind;
            let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
            let id = event.identity().run_id();
            match event.kind() {
                RunApplicationEventKind::RunStarted { .. } => {
                    runs.insert(
                        id,
                        RunRecord {
                            started: event.clone(),
                            terminal: None,
                            started_at: std::time::SystemTime::now(),
                            finished_at: None,
                            acknowledged: false,
                        },
                    );
                }
                RunApplicationEventKind::RunCompleted
                | RunApplicationEventKind::RunCancelled
                | RunApplicationEventKind::RunErrored { .. } => {
                    if let Some(record) = runs.get_mut(&id)
                        && record.terminal.is_none()
                    {
                        record.terminal = Some(event.clone());
                        record.finished_at = Some(std::time::SystemTime::now());
                    }
                }
                RunApplicationEventKind::ResultInspectionRequested { .. } => {}
            }
            let newest = newest_runs(&runs);
            let recent = runs
                .iter()
                .rev()
                .filter(|(_, record)| record.terminal.is_some())
                .take(MAX_FINISHED_RUNS)
                .map(|(id, _)| *id)
                .collect::<std::collections::BTreeSet<_>>();
            runs.retain(|id, record| {
                record.terminal.is_none()
                    || recent.contains(id)
                    || newest.get(record.identity().graph_path()) == Some(id)
            });
        }
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

fn newest_runs(
    runs: &BTreeMap<yss_graph_execution::run_registry::RunId, RunRecord>,
) -> BTreeMap<GraphResourcePath, yss_graph_execution::run_registry::RunId> {
    runs.iter()
        .map(|(id, record)| (record.identity().graph_path().clone(), *id))
        .collect()
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

#[derive(Clone)]
pub struct GraphEditRequest {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub version: GraphEditVersion,
    pub operation_id: OperationId,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct GraphEditResponse {
    pub update: GraphDocumentChange,
    pub editing: GraphEditingState,
    pub result_state: super::results::GraphResultState,
    pub operation_id: OperationId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphSaveResponse {
    pub project_instance_id: ProjectInstanceId,
    /// A replay retains the original saved revision even when `graph` includes later edits.
    pub resource_revision: ResourceRevision,
    pub graph: GraphEditResponse,
}

fn command_fingerprint(
    request: &GraphEditRequest,
    kind: &str,
    payload: &impl serde::Serialize,
) -> Result<[u8; 32], ResourceMutationApplicationError> {
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-edit-command.v1",
        &(
            &request.project_instance_id,
            &request.graph_path,
            request.version,
            &request.locale,
            kind,
            payload,
        ),
    )
    .map_err(|error| {
        ResourceMutationApplicationError::GraphCommit(
            yss_project::ProjectGraphCommitError::InvalidDocument(error.to_string()),
        )
    })
}

fn existing_command(
    captured: &ApplicationSession,
    request: &GraphEditRequest,
    fingerprint: [u8; 32],
) -> Result<Option<yss_project::GraphEditCommandReceipt>, ResourceMutationApplicationError> {
    let receipt = captured.project().graph_edit_command_receipt(
        &request.project_instance_id,
        &request.graph_path,
        request.version.session_id,
        request.operation_id,
    )?;
    if receipt.as_ref().is_some_and(|receipt| {
        receipt.fingerprint != fingerprint || receipt.request_version != request.version
    }) {
        return Err(ResourceMutationApplicationError::GraphOperation(
            yss_project::ProjectGraphOperationError::OperationOwnershipChanged {
                operation_id: request.operation_id,
            },
        ));
    }
    Ok(receipt)
}

impl ApplicationState {
    fn current_graph_response(
        &self,
        captured: &Arc<ApplicationSession>,
        path: &GraphResourcePath,
        locale: &str,
        operation_id: OperationId,
    ) -> Result<GraphEditResponse, ResourceMutationApplicationError> {
        let snapshot = captured
            .project()
            .read_graph_editing(captured.project_instance_id(), path)?;
        let editor = GraphDocumentEditor::new(captured, path, locale, snapshot.document)?;
        let update = editor.finish(self)?;
        let result_state = super::results::observe_graph_result_inputs(
            captured,
            path.as_str(),
            update.result_inputs.clone(),
        );
        Ok(GraphEditResponse {
            operation_id,
            result_state,
            update,
            editing: snapshot.state,
        })
    }

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
        let fingerprint = command_fingerprint(&request, "edit", &[&mutation])?;
        self.commit_graph_mutations(request, std::iter::once(mutation), fingerprint)
    }

    /// Commit ordered native mutations atomically with one projection and history entry.
    pub fn edit_graph_batch(
        &self,
        request: GraphEditRequest,
        mutations: Vec<EditorGraphMutation>,
    ) -> Result<GraphEditResponse, ResourceMutationApplicationError> {
        let fingerprint = command_fingerprint(&request, "edit", &mutations)?;
        self.commit_graph_mutations(request, mutations, fingerprint)
    }

    fn commit_graph_mutations(
        &self,
        request: GraphEditRequest,
        mutations: impl IntoIterator<Item = EditorGraphMutation>,
        fingerprint: [u8; 32],
    ) -> Result<GraphEditResponse, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&request.project_instance_id)?;
        let _editing = captured
            .coordinate_graph_edit(&request.graph_path)
            .map_err(|_| ResourceMutationApplicationError::EditingBusy)?;
        if let Some(receipt) = existing_command(&captured, &request, fingerprint)? {
            let mut response = self.current_graph_response(
                &captured,
                &request.graph_path,
                &request.locale,
                request.operation_id,
            )?;
            response.update.changed = receipt.commit.invalidations.graph;
            return Ok(response);
        }
        let operation = captured
            .project()
            .capture_graph_edit(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
                request.operation_id,
                fingerprint,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let mut editor = GraphDocumentEditor::new(
            &captured,
            &request.graph_path,
            &request.locale,
            Arc::clone(&operation.document),
        )?;
        for mutation in mutations {
            editor.apply(mutation)?;
        }
        let update = editor.finish(self)?;
        let receipt = captured
            .project()
            .commit_graph_edit(
                operation,
                Arc::clone(&update.document),
                GraphHistoryAction::Edit(update.patch.clone()),
            )
            .map_err(ResourceMutationApplicationError::GraphCommit)?;
        let result_state = super::results::observe_graph_result_inputs(
            &captured,
            request.graph_path.as_str(),
            update.result_inputs.clone(),
        );
        captured.publish_graph_activity(GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: receipt.editing.clone(),
            cause: GraphChangeCause::Edit {
                operation_id: request.operation_id,
            },
        });
        Ok(GraphEditResponse {
            operation_id: request.operation_id,
            result_state,
            update,
            editing: receipt.editing,
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
        let fingerprint = command_fingerprint(&request, if redo { "redo" } else { "undo" }, &())?;
        if let Some(receipt) = existing_command(&captured, &request, fingerprint)? {
            let mut response = self.current_graph_response(
                &captured,
                &request.graph_path,
                &request.locale,
                request.operation_id,
            )?;
            response.update.changed = receipt.commit.invalidations.graph;
            return Ok(response);
        }
        let operation = captured
            .project()
            .capture_graph_edit(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
                request.operation_id,
                fingerprint,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let patch = captured.project().graph_history_patch(
            &request.project_instance_id,
            &request.graph_path,
            request.version,
            redo,
        )?;
        let mut document = Arc::clone(&operation.document);
        if let Some(patch) = &patch {
            document = Arc::new(
                prepare_graph_document_patch(document.as_ref().clone(), patch).map_err(
                    |error| {
                        ResourceMutationApplicationError::Mutation(MutationConflict::Document(
                            error,
                        ))
                    },
                )?,
            );
        }
        let editor =
            GraphDocumentEditor::new(&captured, &request.graph_path, &request.locale, document)?;
        let mut update = editor.finish(self)?;
        update.changed = patch.is_some();
        let action = if redo {
            GraphHistoryAction::Redo
        } else {
            GraphHistoryAction::Undo
        };
        let receipt = captured
            .project()
            .commit_graph_edit(operation, Arc::clone(&update.document), action)
            .map_err(ResourceMutationApplicationError::GraphCommit)?;
        let result_state = super::results::observe_graph_result_inputs(
            &captured,
            request.graph_path.as_str(),
            update.result_inputs.clone(),
        );
        captured.publish_graph_activity(GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: receipt.editing.clone(),
            cause: GraphChangeCause::Edit {
                operation_id: request.operation_id,
            },
        });
        Ok(GraphEditResponse {
            operation_id: request.operation_id,
            result_state,
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
        let fingerprint = command_fingerprint(&request, "save", &())?;
        if let Some(receipt) = existing_command(&captured, &request, fingerprint)? {
            return Ok(GraphSaveResponse {
                project_instance_id: request.project_instance_id.clone(),
                resource_revision: receipt.commit.to_revision,
                graph: self.current_graph_response(
                    &captured,
                    &request.graph_path,
                    &request.locale,
                    request.operation_id,
                )?,
            });
        }
        let operation = captured
            .project()
            .capture_graph_edit(
                &request.project_instance_id,
                &request.graph_path,
                request.version,
                request.operation_id,
                fingerprint,
            )
            .map_err(ResourceMutationApplicationError::GraphOperation)?;
        let editor = GraphDocumentEditor::new(
            &captured,
            &request.graph_path,
            &request.locale,
            Arc::clone(&operation.document),
        )?;
        let update = editor.finish(self)?;
        let receipt = captured
            .project()
            .save_graph_candidate(operation, Arc::clone(&update.document))
            .map_err(|error| match error {
                yss_project::ProjectGraphSaveError::Filesystem(error) => {
                    ResourceMutationApplicationError::Project(error)
                }
                yss_project::ProjectGraphSaveError::Commit(error) => {
                    ResourceMutationApplicationError::GraphCommit(error)
                }
            })?;
        let result_state = super::results::observe_graph_result_inputs(
            &captured,
            request.graph_path.as_str(),
            update.result_inputs.clone(),
        );
        captured.publish_graph_activity(GraphActivity::Changed {
            graph_path: request.graph_path.as_str().into(),
            editing: receipt.editing.clone(),
            cause: GraphChangeCause::Edit {
                operation_id: request.operation_id,
            },
        });
        Ok(GraphSaveResponse {
            project_instance_id: request.project_instance_id,
            resource_revision: receipt.to_revision,
            graph: GraphEditResponse {
                operation_id: request.operation_id,
                result_state,
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

    /// Every active run and each graph's newest run.
    pub fn execution_snapshot(
        &self,
        project: &ProjectInstanceId,
    ) -> Result<Vec<RunRecord>, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(project)?;
        let snapshot = captured.graph_activity().execution_records();
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(snapshot)
    }

    /// Recent runs of every graph in this execution session, in run order.
    pub fn run_journal(
        &self,
        project: &ProjectInstanceId,
    ) -> Result<Vec<RunRecord>, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(project)?;
        let journal = captured.graph_activity().run_journal();
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(journal)
    }

    /// Dismisses the notices of finished runs for every view of their graphs.
    pub fn acknowledge_runs(
        &self,
        project: &ProjectInstanceId,
        runs: &[super::run::RunIdentity],
    ) -> Result<(), ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(project)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        let acknowledged = captured.graph_activity().acknowledge(runs);
        if !acknowledged.is_empty() {
            captured.publish_graph_activity(GraphActivity::RunsAcknowledged(acknowledged));
        }
        Ok(())
    }
}
