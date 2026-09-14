use crate::automation::{AutomationGraphAction, AutomationGraphUpdate};
use std::{collections::BTreeMap, sync::Mutex};
use tauri::ipc::Channel;
use tokio::sync::oneshot;
use yss_automation_contract::{
    AutomationCapabilityRequest, AutomationCapabilityResult, CapabilityControl, CapabilityFailure,
    CapabilityFailureCode, CapabilityId, CapabilityInvocationContext, HarnessSessionId,
};
use yss_ipc_contract::harness_graph::HarnessGraphToolRequestDto;
use yss_project_identity::ProjectInstanceId;

type Outcome = Result<AutomationCapabilityResult, CapabilityFailure>;
struct PendingGraphTool {
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
    control: CapabilityControl,
    reply: oneshot::Sender<Outcome>,
    preparing: bool,
    prepared: Option<AutomationCapabilityResult>,
    interruption: Option<CapabilityFailure>,
    adopting: bool,
}
#[derive(Default)]
struct GraphClientState {
    clients: BTreeMap<HarnessSessionId, (String, Channel<HarnessGraphToolRequestDto>)>,
    pending: BTreeMap<String, PendingGraphTool>,
}

/// The webview owns unsaved drafts and their FIFO/history. Only Rust prepares tool results;
/// the client acknowledges adopting an already validated projection, never supplies results.
#[derive(Default)]
pub struct HarnessGraphClientHub {
    state: Mutex<GraphClientState>,
}

struct GraphRequestLease<'a> {
    hub: &'a HarnessGraphClientHub,
    id: String,
}
impl Drop for GraphRequestLease<'_> {
    fn drop(&mut self) {
        self.hub.abandon(&self.id);
    }
}

impl HarnessGraphClientHub {
    pub fn subscribe(
        &self,
        session: HarnessSessionId,
        channel: Channel<HarnessGraphToolRequestDto>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clients
            .insert(session, (id.clone(), channel));
        id
    }

    pub fn unsubscribe(&self, id: &str) {
        let ids = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let session = state
                .clients
                .iter()
                .find(|(_, (key, _))| key == id)
                .map(|(session, _)| session.clone());
            let Some(session) = session else {
                return;
            };
            state.clients.remove(&session);
            state
                .pending
                .iter()
                .filter(|(_, pending)| pending.context.harness_session_id() == &session)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
        };
        for id in ids {
            self.interrupt(&id, failure(CapabilityFailureCode::GraphClientUnavailable));
        }
    }

    pub async fn invoke(
        &self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        control: CapabilityControl,
    ) -> Outcome {
        control.check()?;
        let graph_path = graph_path(&request)
            .ok_or_else(|| failure(CapabilityFailureCode::InvalidRequest))?
            .to_owned();
        let id = uuid::Uuid::new_v4().to_string();
        let (reply, mut response) = oneshot::channel();
        let dto = HarnessGraphToolRequestDto {
            request_id: id.clone(),
            session_id: context.harness_session_id().to_string(),
            project_instance_id: context.project().project_instance_id().to_string(),
            graph_path,
            capability_id: request.capability_id(),
        };
        let channel = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let channel = state
                .clients
                .get(context.harness_session_id())
                .map(|(_, channel)| channel.clone())
                .ok_or_else(|| failure(CapabilityFailureCode::GraphClientUnavailable))?;
            if state.pending.len() >= 128 {
                return Err(failure(CapabilityFailureCode::InvocationConflict));
            }
            state.pending.insert(
                id.clone(),
                PendingGraphTool {
                    context,
                    request,
                    control: control.clone(),
                    reply,
                    preparing: false,
                    prepared: None,
                    interruption: None,
                    adopting: false,
                },
            );
            channel
        };
        let _lease = GraphRequestLease {
            hub: self,
            id: id.clone(),
        };
        if channel.send(dto).is_err() {
            self.interrupt(&id, failure(CapabilityFailureCode::GraphClientUnavailable));
        }
        tokio::select! {
            result = &mut response => return result.unwrap_or_else(|_| Err(failure(CapabilityFailureCode::GraphClientUnavailable))),
            reason = control.cancellation().cancelled() => self.interrupt(&id, failure(if reason == yss_automation_contract::CancellationReason::DeadlineElapsed { CapabilityFailureCode::DeadlineElapsed } else { CapabilityFailureCode::Cancelled })),
            _ = tokio::time::sleep_until(control.deadline().into()) => self.interrupt(&id, failure(CapabilityFailureCode::DeadlineElapsed)),
        }
        match tokio::time::timeout(std::time::Duration::from_secs(5), &mut response).await {
            Ok(result) => result
                .unwrap_or_else(|_| Err(failure(CapabilityFailureCode::GraphClientUnavailable))),
            Err(_) => {
                self.abandon(&id);
                Err(failure(CapabilityFailureCode::OutcomeUnknown))
            }
        }
    }

    pub fn begin(
        &self,
        id: &str,
        project: &ProjectInstanceId,
    ) -> Result<
        (
            CapabilityInvocationContext,
            AutomationCapabilityRequest,
            CapabilityControl,
        ),
        CapabilityFailure,
    > {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let pending = state
            .pending
            .get_mut(id)
            .ok_or_else(|| failure(CapabilityFailureCode::GraphClientUnavailable))?;
        pending.control.check()?;
        if pending.context.project().project_instance_id() != project
            || pending.preparing
            || pending.prepared.is_some()
        {
            return Err(failure(CapabilityFailureCode::InvocationConflict));
        }
        pending.preparing = true;
        Ok((
            pending.context.clone(),
            pending.request.clone(),
            pending.control.clone(),
        ))
    }

    pub fn stage(
        &self,
        id: &str,
        action: Result<AutomationGraphAction, CapabilityFailure>,
    ) -> Result<AutomationGraphUpdate, CapabilityFailure> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(pending) = state.pending.get_mut(id) else {
            return Err(failure(CapabilityFailureCode::GraphClientUnavailable));
        };
        pending.preparing = false;
        let action = action.and_then(|action| {
            let path = match &action.result {
                AutomationCapabilityResult::GraphInspection(r) => Some(r.graph_path.as_str()),
                AutomationCapabilityResult::GraphEditReceipt(r) => Some(r.graph_path.as_str()),
                AutomationCapabilityResult::GraphCompilation(r) => Some(r.graph_path.as_str()),
                AutomationCapabilityResult::GraphExecution(r) => Some(r.graph_path.as_str()),
                AutomationCapabilityResult::GraphSaved(r) => Some(r.graph_path.as_str()),
                _ => None,
            };
            let update_matches = matches!(
                (pending.request.capability_id(), &action.update),
                (CapabilityId::InspectGraph, AutomationGraphUpdate::None)
                    | (
                        CapabilityId::ApplyGraphEdit,
                        AutomationGraphUpdate::Draft(_)
                    )
                    | (
                        CapabilityId::CompileGraph,
                        AutomationGraphUpdate::Compilation(_)
                    )
                    | (
                        CapabilityId::ExecuteGraph,
                        AutomationGraphUpdate::Execution { .. }
                    )
                    | (CapabilityId::SaveGraph, AutomationGraphUpdate::Saved(_))
            );
            if action.result.capability_id() != pending.request.capability_id()
                || path != graph_path(&pending.request)
                || !update_matches
            {
                return Err(failure(CapabilityFailureCode::InternalFailure));
            }
            action.result.validate_budget(1_048_576)?;
            Ok(action)
        });
        match action {
            Ok(action) => {
                pending.prepared = Some(action.result);
                if let Some(interruption) = pending.interruption.clone() {
                    let pending = state.pending.remove(id).unwrap();
                    let outcome = match pending.prepared {
                        Some(
                            result @ (AutomationCapabilityResult::GraphSaved(_)
                            | AutomationCapabilityResult::GraphExecution(_)),
                        ) => Ok(result),
                        _ => Err(interruption.clone()),
                    };
                    let _ = pending.reply.send(outcome);
                    return Err(interruption);
                }
                Ok(action.update)
            }
            Err(error) => {
                let pending = state.pending.remove(id).unwrap();
                let _ = pending.reply.send(Err(error.clone()));
                Err(error)
            }
        }
    }

    pub fn complete(&self, id: &str, applied: bool) {
        let pending = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state
                .pending
                .get(id)
                .is_some_and(|pending| pending.preparing)
            {
                return;
            }
            state.pending.remove(id)
        };
        if let Some(pending) = pending {
            let outcome = match pending.prepared {
                Some(result)
                    if applied
                        || matches!(
                            result,
                            AutomationCapabilityResult::GraphSaved(_)
                                | AutomationCapabilityResult::GraphExecution(_)
                        ) =>
                {
                    Ok(result)
                }
                _ => Err(pending.interruption.unwrap_or_else(|| {
                    pending
                        .control
                        .check()
                        .err()
                        .unwrap_or_else(|| failure(CapabilityFailureCode::GraphDraftChanged))
                })),
            };
            let _ = pending.reply.send(outcome);
        }
    }

    fn interrupt(&self, id: &str, error: CapabilityFailure) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(pending) = state.pending.get_mut(id) else {
            return;
        };
        pending.control.cancel_query();
        if pending.preparing || pending.adopting {
            pending.interruption = Some(error);
            return;
        }
        let pending = state.pending.remove(id).unwrap();
        let outcome = match pending.prepared {
            Some(
                result @ (AutomationCapabilityResult::GraphSaved(_)
                | AutomationCapabilityResult::GraphExecution(_)),
            ) => Ok(result),
            _ => Err(error),
        };
        let _ = pending.reply.send(outcome);
    }

    pub fn claim(&self, id: &str) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(pending) = state.pending.get_mut(id) else {
            return false;
        };
        if pending.prepared.is_none() || pending.adopting || pending.control.check().is_err() {
            return false;
        }
        pending.adopting = true;
        true
    }

    fn abandon(&self, id: &str) {
        if let Some(pending) = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            .remove(id)
        {
            pending.control.cancel_query();
            let _ = pending
                .reply
                .send(Err(failure(CapabilityFailureCode::OutcomeUnknown)));
        }
    }
}

pub fn graph_path(request: &AutomationCapabilityRequest) -> Option<&str> {
    match request {
        AutomationCapabilityRequest::InspectGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ApplyGraphEdit(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::CompileGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ExecuteGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::SaveGraph(r) => Some(&r.graph_path),
        _ => None,
    }
}
fn failure(code: CapabilityFailureCode) -> CapabilityFailure {
    CapabilityFailure::new(code)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::GraphProjectionReplacement,
        resource_mutation::{GraphDraftSave, GraphDraftTransform},
    };
    use std::{sync::Arc, time::Duration};
    use yss_automation_contract::{
        ApplyGraphEditRequest, CancellationReason, CancellationToken, CapabilityInvocationId,
        GraphEditOperation, GraphEditPosition, GraphEditReceipt, GraphSaved, PrincipalId,
        ProjectSessionBinding, SaveGraphRequest,
    };
    use yss_graph_document::GraphDocument;
    use yss_graph_editor::projection::{
        EditorProjectionBasis, EditorProjectionModel, EditorResolutionOutcome,
    };
    use yss_project_identity::{ProjectSessionId, ResourceRevision};
    const PATH: &str = "events/Main.yssbi-event";
    fn context() -> CapabilityInvocationContext {
        CapabilityInvocationContext::new(
            PrincipalId::try_new("user-1").unwrap(),
            HarnessSessionId::try_new("session-1").unwrap(),
            CapabilityInvocationId::try_new("capability-1").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
        )
    }
    fn request() -> AutomationCapabilityRequest {
        AutomationCapabilityRequest::ApplyGraphEdit(ApplyGraphEditRequest {
            graph_path: PATH.into(),
            base_revision: 0,
            graph_hash: "0".repeat(64),
            client_key: "edit-1".into(),
            locale: "en-US".into(),
            operations: vec![GraphEditOperation::MoveNodes {
                positions: vec![GraphEditPosition {
                    node_id: "00000000-0000-0000-0000-000000000001".into(),
                    x: 1.,
                    y: 2.,
                }],
            }],
        })
    }
    fn projection() -> GraphProjectionReplacement {
        let path = yss_graph_document::GraphResourcePath::new(PATH).unwrap();
        GraphProjectionReplacement {
            graph_path: PATH.into(),
            function_editor_projection: None,
            projection: EditorProjectionModel {
                basis: EditorProjectionBasis {
                    graph_path: path.clone(),
                    registry_fingerprint: [0; 32],
                    semantic_input_hash: [0; 32],
                    resource_versions: Default::default(),
                    resource_observations: Default::default(),
                },
                graph_path: path,
                nodes: Box::new([]),
                connections: Box::new([]),
                diagnostics: Box::new([]),
                outcome: EditorResolutionOutcome::Complete,
            },
        }
    }
    fn edit() -> AutomationGraphAction {
        AutomationGraphAction {
            result: AutomationCapabilityResult::GraphEditReceipt(GraphEditReceipt {
                graph_path: PATH.into(),
                from_revision: 0,
                to_revision: 1,
                client_key: "edit-1".into(),
                graph_hash: "1".repeat(64),
                created_nodes: BTreeMap::new(),
                created_ports: BTreeMap::new(),
            }),
            update: AutomationGraphUpdate::Draft(GraphDraftTransform {
                changed: true,
                document: GraphDocument::default(),
                projection_replacement: projection(),
            }),
        }
    }
    fn subscribed() -> (
        Arc<HarnessGraphClientHub>,
        String,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let hub = Arc::new(HarnessGraphClientHub::default());
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let subscription = hub.subscribe(
            context().harness_session_id().clone(),
            Channel::new(move |body| {
                let tauri::ipc::InvokeResponseBody::Json(body) = body else {
                    panic!("JSON request")
                };
                let value: serde_json::Value = serde_json::from_str(&body).unwrap();
                sender
                    .send(value["requestId"].as_str().unwrap().to_owned())
                    .unwrap();
                Ok(())
            }),
        );
        (hub, subscription, receiver)
    }

    #[tokio::test]
    async fn graph_edits_return_the_rust_receipt_only_after_bound_client_adoption() {
        let (hub, _, mut requests) = subscribed();
        let token = CancellationToken::default();
        let control = CapabilityControl::new(token.clone(), Duration::from_secs(2));
        let (outcome, ()) = tokio::join!(hub.invoke(context(), request(), control), async {
            let id = requests.recv().await.unwrap();
            assert!(
                hub.begin(
                    &id,
                    &ProjectInstanceId::from_existing("other-project".into())
                )
                .is_err()
            );
            hub.begin(&id, context().project().project_instance_id())
                .unwrap();
            assert!(
                hub.begin(&id, context().project().project_instance_id())
                    .is_err()
            );
            hub.stage(&id, Ok(edit())).unwrap();
            assert!(hub.claim(&id));
            token.cancel(CancellationReason::User);
            tokio::task::yield_now().await;
            assert!(hub.state.lock().unwrap().pending.contains_key(&id));
            hub.complete(&id, true);
        });
        assert!(
            matches!(outcome.unwrap(), AutomationCapabilityResult::GraphEditReceipt(receipt) if receipt.graph_hash == "1".repeat(64))
        );
        assert!(hub.state.lock().unwrap().pending.is_empty());
    }

    #[tokio::test]
    async fn a_closed_client_cannot_recast_an_already_saved_graph_as_unsaved() {
        let (hub, subscription, mut requests) = subscribed();
        let request = AutomationCapabilityRequest::SaveGraph(SaveGraphRequest {
            graph_path: PATH.into(),
            graph_hash: "0".repeat(64),
        });
        let (outcome, ()) = tokio::join!(
            hub.invoke(
                context(),
                request,
                CapabilityControl::new(CancellationToken::default(), Duration::from_secs(2))
            ),
            async {
                let id = requests.recv().await.unwrap();
                hub.begin(&id, context().project().project_instance_id())
                    .unwrap();
                hub.unsubscribe(&subscription);
                let action = AutomationGraphAction {
                    result: AutomationCapabilityResult::GraphSaved(GraphSaved {
                        graph_path: PATH.into(),
                        graph_hash: "0".repeat(64),
                        resource_revision: 7,
                    }),
                    update: AutomationGraphUpdate::Saved(GraphDraftSave {
                        project_instance_id: context().project().project_instance_id().clone(),
                        resource_revision: ResourceRevision::new(7),
                        document: GraphDocument::default(),
                        projection_replacement: projection(),
                    }),
                };
                assert!(hub.stage(&id, Ok(action)).is_err());
            }
        );
        assert!(
            matches!(outcome.unwrap(), AutomationCapabilityResult::GraphSaved(saved) if saved.resource_revision == 7)
        );
        assert!(hub.state.lock().unwrap().pending.is_empty());
    }

    #[tokio::test]
    async fn cancelled_and_misrouted_graph_actions_cannot_be_adopted() {
        let (hub, _, mut requests) = subscribed();
        let token = CancellationToken::default();
        let (outcome, id) = tokio::join!(
            hub.invoke(
                context(),
                request(),
                CapabilityControl::new(token.clone(), Duration::from_secs(2))
            ),
            async {
                let id = requests.recv().await.unwrap();
                token.cancel(CancellationReason::User);
                id
            }
        );
        assert_eq!(outcome.unwrap_err().code, CapabilityFailureCode::Cancelled);
        assert!(!hub.claim(&id));
        let (outcome, ()) = tokio::join!(
            hub.invoke(
                context(),
                request(),
                CapabilityControl::new(CancellationToken::default(), Duration::from_secs(2))
            ),
            async {
                let id = requests.recv().await.unwrap();
                hub.begin(&id, context().project().project_instance_id())
                    .unwrap();
                let mut action = edit();
                if let AutomationCapabilityResult::GraphEditReceipt(receipt) = &mut action.result {
                    receipt.graph_path = "events/Other.yssbi-event".into();
                }
                assert!(hub.stage(&id, Ok(action)).is_err());
                assert!(!hub.claim(&id));
            }
        );
        assert_eq!(
            outcome.unwrap_err().code,
            CapabilityFailureCode::InternalFailure
        );
    }
}
