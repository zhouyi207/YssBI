//! Session-scoped presentation state. Business data stays with Project and Results.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use yss_graph_execution::result::ResultId;
use yss_node_kernel::RuntimeValue;
use yss_project_identity::ProjectInstanceId;
use yss_ui_contract::*;

use crate::session::{ApplicationSession, ApplicationState};

#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("ui_spec_invalid")]
    Invalid,
    #[error("ui_revision_conflict")]
    Conflict,
    #[error("ui_target_unavailable")]
    Unavailable,
    #[error("ui_session_changed")]
    Session,
    #[error("ui_capacity_exceeded")]
    Capacity,
    #[error("ui_workbench_unavailable")]
    Workbench,
}

impl UiError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "ui_spec_invalid",
            Self::Conflict => "ui_revision_conflict",
            Self::Unavailable => "ui_target_unavailable",
            Self::Session => "ui_session_changed",
            Self::Capacity => "ui_capacity_exceeded",
            Self::Workbench => "ui_workbench_unavailable",
        }
    }
}

struct IntentEntry {
    key: String,
    receipt: UiIntentReceipt,
    created: Instant,
    claimant: Option<String>,
}

#[derive(Default)]
struct PresentationState {
    pages: BTreeMap<UiSource, UiPage>,
    intents: VecDeque<IntentEntry>,
}

type UiObserver = Arc<dyn Fn(UiEvent) + Send + Sync>;

#[derive(Default)]
struct UiObservers {
    closed: bool,
    observers: BTreeMap<uuid::Uuid, UiObserver>,
}

pub(crate) struct UiSubscription {
    source: Arc<Mutex<UiObservers>>,
    id: uuid::Uuid,
}

impl Drop for UiSubscription {
    fn drop(&mut self) {
        self.source
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .observers
            .remove(&self.id);
    }
}

pub(crate) struct PresentationSession {
    state: Mutex<PresentationState>,
    events: Arc<Mutex<UiObservers>>,
    workbenches: AtomicUsize,
}

impl Default for PresentationSession {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            events: Arc::default(),
            workbenches: AtomicUsize::new(0),
        }
    }
}

impl PresentationSession {
    pub(crate) fn subscribe(&self, observer: UiObserver) -> Result<UiSubscription, UiError> {
        let mut source = self.events.lock().map_err(|_| UiError::Unavailable)?;
        if source.closed {
            return Err(UiError::Session);
        }
        if source.observers.len() >= 64 {
            return Err(UiError::Capacity);
        }
        let id = uuid::Uuid::new_v4();
        source.observers.insert(id, observer);
        Ok(UiSubscription {
            source: self.events.clone(),
            id,
        })
    }
    fn publish(&self, event: UiEvent) {
        let observers = self
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .observers
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for observer in observers {
            observer(event.clone());
        }
    }
    pub(crate) fn session_changed(&self) {
        let observers = {
            let mut source = self
                .events
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            source.closed = true;
            std::mem::take(&mut source.observers)
        };
        for observer in observers.into_values() {
            observer(UiEvent::SessionChanged);
        }
    }
    pub(crate) fn attach_workbench(&self) {
        self.workbenches.fetch_add(1, Ordering::SeqCst);
    }
    pub(crate) fn detach_workbench(&self) {
        self.workbenches.fetch_sub(1, Ordering::SeqCst);
    }

    fn page(&self, source: &UiSource) -> Result<UiPage, UiError> {
        let mut state = self.state.lock().map_err(|_| UiError::Unavailable)?;
        if !state.pages.contains_key(source) && state.pages.len() >= 64 {
            return Err(UiError::Capacity);
        }
        Ok(state
            .pages
            .entry(source.clone())
            .or_insert_with(|| UiPage {
                source: source.clone(),
                revision: 1,
                spec: UiSpec::regression_report(),
            })
            .clone())
    }

    fn update(&self, request: UpdateUiRequest) -> Result<UiUpdate, UiError> {
        let mut state = self.state.lock().map_err(|_| UiError::Unavailable)?;
        let previous = state.pages.get(&request.source).ok_or(UiError::Conflict)?;
        if previous.revision != request.base_revision {
            return Err(UiError::Conflict);
        }
        let mut next = previous.clone();
        match request.action {
            UiAction::Replace { spec } => next.spec = spec,
            UiAction::Reset => next.spec = UiSpec::regression_report(),
            UiAction::Visibility { id, visible } => {
                next.spec
                    .elements
                    .get_mut(&id)
                    .ok_or(UiError::Invalid)?
                    .visible = visible
            }
            UiAction::Move { id, offset } => {
                if offset != -1 && offset != 1 {
                    return Err(UiError::Invalid);
                }
                let parent = next
                    .spec
                    .elements
                    .values_mut()
                    .find(|element| element.children.contains(&id))
                    .ok_or(UiError::Invalid)?;
                let index = parent
                    .children
                    .iter()
                    .position(|child| child == &id)
                    .ok_or(UiError::Invalid)?;
                let target = index
                    .checked_add_signed(isize::from(offset))
                    .filter(|target| *target < parent.children.len())
                    .ok_or(UiError::Invalid)?;
                parent.children.swap(index, target);
            }
            UiAction::Patch { operations } => {
                if operations.is_empty() || operations.len() > MAX_ELEMENTS * 2 + 1 {
                    return Err(UiError::Invalid);
                }
                for operation in operations {
                    match operation {
                        UiPatch::Set { id, element } => {
                            next.spec.elements.insert(id, element);
                        }
                        UiPatch::Remove { id } => {
                            next.spec.elements.remove(&id).ok_or(UiError::Invalid)?;
                        }
                        UiPatch::Root { id } => next.spec.root = id,
                    }
                }
            }
        }
        next.spec.validate().map_err(|_| UiError::Invalid)?;
        if next.spec != previous.spec {
            next.revision = previous
                .revision
                .checked_add(1)
                .filter(|v| *v <= 9_007_199_254_740_991)
                .ok_or(UiError::Capacity)?;
        }
        let update = diff(previous, &next);
        let changed = next.revision != previous.revision;
        state.pages.insert(request.source, next);
        // Publish under the commit lock so concurrent GUI / Harness mutations keep their order.
        if changed {
            self.publish(UiEvent::Update {
                update: update.clone(),
            });
        }
        Ok(update)
    }

    fn expire(state: &mut PresentationState) {
        for entry in &mut state.intents {
            if entry.created.elapsed() > Duration::from_secs(30)
                && matches!(
                    entry.receipt.status,
                    UiIntentStatus::Pending | UiIntentStatus::Claimed
                )
            {
                entry.receipt.status = UiIntentStatus::Expired;
            }
        }
    }

    fn request_intent(&self, key: String, intent: UiIntent) -> Result<UiIntentReceipt, UiError> {
        let mut state = self.state.lock().map_err(|_| UiError::Unavailable)?;
        Self::expire(&mut state);
        if let Some(entry) = state.intents.iter().find(|entry| entry.key == key) {
            return if entry.receipt.intent == intent {
                Ok(entry.receipt.clone())
            } else {
                Err(UiError::Conflict)
            };
        }
        if self.workbenches.load(Ordering::SeqCst) == 0 {
            return Err(UiError::Workbench);
        }
        if state.intents.len() >= 128 {
            let index = state
                .intents
                .iter()
                .position(|entry| {
                    !matches!(
                        entry.receipt.status,
                        UiIntentStatus::Pending | UiIntentStatus::Claimed
                    )
                })
                .ok_or(UiError::Capacity)?;
            state.intents.remove(index);
        }
        let receipt = UiIntentReceipt {
            id: uuid::Uuid::new_v4().to_string(),
            intent,
            status: UiIntentStatus::Pending,
        };
        state.intents.push_back(IntentEntry {
            key,
            receipt: receipt.clone(),
            created: Instant::now(),
            claimant: None,
        });
        self.publish(UiEvent::Intent {
            receipt: receipt.clone(),
        });
        Ok(receipt)
    }

    pub(crate) fn pending(&self) -> Result<Vec<UiIntentReceipt>, UiError> {
        let mut state = self.state.lock().map_err(|_| UiError::Unavailable)?;
        Self::expire(&mut state);
        Ok(state
            .intents
            .iter()
            .filter(|entry| entry.receipt.status == UiIntentStatus::Pending)
            .map(|entry| entry.receipt.clone())
            .collect())
    }

    fn receipt(&self, id: &str) -> Result<UiIntentReceipt, UiError> {
        let mut state = self.state.lock().map_err(|_| UiError::Unavailable)?;
        Self::expire(&mut state);
        state
            .intents
            .iter()
            .find(|entry| entry.receipt.id == id)
            .map(|entry| entry.receipt.clone())
            .ok_or(UiError::Unavailable)
    }

    pub(crate) fn settle_intent(
        &self,
        id: &str,
        window: &str,
        status: UiIntentStatus,
    ) -> Result<bool, UiError> {
        let mut state = self.state.lock().map_err(|_| UiError::Unavailable)?;
        Self::expire(&mut state);
        let entry = state
            .intents
            .iter_mut()
            .find(|entry| entry.receipt.id == id)
            .ok_or(UiError::Unavailable)?;
        if status == UiIntentStatus::Claimed && entry.receipt.status == UiIntentStatus::Pending {
            entry.receipt.status = status;
            entry.claimant = Some(window.to_owned());
            return Ok(true);
        }
        if matches!(status, UiIntentStatus::Applied | UiIntentStatus::Failed)
            && entry.receipt.status == UiIntentStatus::Claimed
            && entry.claimant.as_deref() == Some(window)
        {
            entry.receipt.status = status;
            return Ok(true);
        }
        Ok(false)
    }
}

impl ApplicationState {
    pub(crate) fn ui_session(
        &self,
        project: &ProjectInstanceId,
    ) -> Result<std::sync::Arc<ApplicationSession>, UiError> {
        let captured = self.capture_session().map_err(|_| UiError::Session)?;
        if captured.project_instance_id() != project {
            return Err(UiError::Session);
        }
        Ok(captured)
    }

    pub fn inspect_ui(
        &self,
        project: &ProjectInstanceId,
        request: InspectUiRequest,
    ) -> Result<UiInspection, UiError> {
        let session = self.ui_session(project)?;
        let result = match request {
            InspectUiRequest::Catalog => catalog(),
            InspectUiRequest::Page { source } => {
                validate_source(&session, &source, true)?;
                UiInspection::Page {
                    page: session.presentation.page(&source)?,
                }
            }
            InspectUiRequest::Intent { id } => UiInspection::Intent {
                receipt: session.presentation.receipt(&id)?,
            },
        };
        self.revalidate_captured_session(&session)
            .map_err(|_| UiError::Session)?;
        Ok(result)
    }

    pub fn update_ui(
        &self,
        project: &ProjectInstanceId,
        request: UpdateUiRequest,
    ) -> Result<UiUpdate, UiError> {
        request.validate().map_err(|_| UiError::Invalid)?;
        let session = self.ui_session(project)?;
        validate_source(&session, &request.source, true)?;
        self.revalidate_captured_session(&session)
            .map_err(|_| UiError::Session)?;
        session.presentation.update(request)
    }

    pub fn request_ui_intent(
        &self,
        project: &ProjectInstanceId,
        caller: &str,
        request: RequestUiIntent,
    ) -> Result<UiIntentReceipt, UiError> {
        request.validate().map_err(|_| UiError::Invalid)?;
        let session = self.ui_session(project)?;
        match &request.intent {
            UiIntent::OpenResult { source } => validate_source(&session, source, false)?,
            UiIntent::OpenGraph {
                graph_path,
                node_id,
            } => {
                let index = session
                    .project()
                    .read_project_index(project)
                    .map_err(|_| UiError::Unavailable)?;
                if !index.graphs.iter().any(|graph| &graph.path == graph_path)
                    || node_id
                        .as_ref()
                        .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
                {
                    return Err(UiError::Unavailable);
                }
            }
            UiIntent::ShowPanel { .. } => {}
        }
        self.revalidate_captured_session(&session)
            .map_err(|_| UiError::Session)?;
        session
            .presentation
            .request_intent(format!("{caller}:{}", request.client_key), request.intent)
    }

    pub fn activate_ui_element(
        &self,
        project: &ProjectInstanceId,
        caller: &str,
        request: ActivateUiRequest,
    ) -> Result<UiIntentReceipt, UiError> {
        let session = self.ui_session(project)?;
        validate_source(&session, &request.source, true)?;
        let page = session.presentation.page(&request.source)?;
        if page.revision != request.base_revision {
            return Err(UiError::Conflict);
        }
        let element = page
            .spec
            .elements
            .get(&request.id)
            .filter(|element| element.visible)
            .ok_or(UiError::Invalid)?;
        let UiComponent::Button { intent, .. } = &element.component else {
            return Err(UiError::Invalid);
        };
        self.request_ui_intent(
            project,
            caller,
            RequestUiIntent {
                client_key: request.client_key,
                intent: intent.clone(),
            },
        )
    }
}

fn validate_source(
    session: &ApplicationSession,
    source: &UiSource,
    report: bool,
) -> Result<(), UiError> {
    source.validate().map_err(|_| UiError::Invalid)?;
    if source.execution_session_id != session.execution_session_id().as_uuid().to_string() {
        return Err(UiError::Unavailable);
    }
    let id: u64 = source.result_id.parse().map_err(|_| UiError::Invalid)?;
    if id.to_string() != source.result_id {
        return Err(UiError::Invalid);
    }
    let result = session
        .execution()
        .query_result(ResultId::from_existing(id))
        .ok_or(UiError::Unavailable)?;
    if report
        && !matches!(
            result.value().value().unannotated(),
            RuntimeValue::LinearRegression(_)
        )
    {
        return Err(UiError::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe(
        session: &PresentationSession,
    ) -> (UiSubscription, std::sync::mpsc::Receiver<UiEvent>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let subscription = session
            .subscribe(Arc::new(move |event| {
                let _ = sender.send(event);
            }))
            .unwrap();
        (subscription, receiver)
    }

    #[test]
    fn gui_and_harness_share_pages_and_reject_stale_result_sessions() {
        use yss_harness_contract::{
            AutomationCapabilityRequest, AutomationCapabilityResult, CancellationToken,
            CapabilityControl, CapabilityInvocationContext, CapabilityInvocationId,
            HarnessSessionId, PrincipalId, ProjectSessionBinding,
        };
        let (application, reference, _) = crate::graph::results::report::tests::fixture(12);
        let session = application.capture_session().unwrap();
        let project = session.project_instance_id();
        let source = UiSource {
            execution_session_id: reference.execution_session_id.as_uuid().to_string(),
            result_id: reference.result_id.get().to_string(),
        };
        application
            .inspect_ui(
                project,
                InspectUiRequest::Page {
                    source: source.clone(),
                },
            )
            .unwrap();
        application
            .update_ui(
                project,
                UpdateUiRequest {
                    source: source.clone(),
                    base_revision: 1,
                    action: UiAction::Visibility {
                        id: "anova".into(),
                        visible: false,
                    },
                },
            )
            .unwrap();
        let context = CapabilityInvocationContext::new(
            PrincipalId::try_new("user").unwrap(),
            HarnessSessionId::try_new("session").unwrap(),
            CapabilityInvocationId::try_new("invocation").unwrap(),
            ProjectSessionBinding::new(project.clone(), session.project_session_id().clone()),
        );
        let control = CapabilityControl::new(CancellationToken::default(), Duration::from_secs(10));
        let result = application
            .invoke_automation_capability(
                context.clone(),
                AutomationCapabilityRequest::InspectUi(InspectUiRequest::Page {
                    source: source.clone(),
                }),
                &control,
            )
            .unwrap();
        let AutomationCapabilityResult::UiInspection(UiInspection::Page { page }) = result else {
            panic!()
        };
        assert_eq!(page.revision, 2);
        assert!(!page.spec.elements["anova"].visible);
        let (_subscription, stream) = observe(&session.presentation);
        application
            .invoke_automation_capability(
                context,
                AutomationCapabilityRequest::UpdateUi(UpdateUiRequest {
                    source: source.clone(),
                    base_revision: 2,
                    action: UiAction::Reset,
                }),
                &control,
            )
            .unwrap();
        assert!(matches!(
            stream.try_recv().unwrap(),
            UiEvent::Update {
                update: UiUpdate::Patch { revision: 3, .. }
            }
        ));
        let UiInspection::Page { page } = application
            .inspect_ui(
                project,
                InspectUiRequest::Page {
                    source: source.clone(),
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert!(page.spec.elements["anova"].visible);
        let stale = UiSource {
            execution_session_id: uuid::Uuid::new_v4().to_string(),
            ..source
        };
        assert!(matches!(
            application.update_ui(
                project,
                UpdateUiRequest {
                    source: stale,
                    base_revision: 3,
                    action: UiAction::Reset
                }
            ),
            Err(UiError::Unavailable)
        ));
        session.presentation.session_changed();
        assert!(matches!(
            stream.try_recv().unwrap(),
            UiEvent::SessionChanged
        ));
    }

    #[test]
    fn competing_edits_and_invalid_batches_leave_the_committed_page_intact() {
        let session = PresentationSession::default();
        let source = UiSource {
            execution_session_id: "test".into(),
            result_id: "1".into(),
        };
        session.page(&source).unwrap();
        let (_subscription, events) = observe(&session);
        session
            .update(UpdateUiRequest {
                source: source.clone(),
                base_revision: 1,
                action: UiAction::Visibility {
                    id: "anova".into(),
                    visible: false,
                },
            })
            .unwrap();
        assert!(matches!(
            events.try_recv().unwrap(),
            UiEvent::Update {
                update: UiUpdate::Patch {
                    base_revision: 1,
                    revision: 2,
                    ..
                }
            }
        ));
        assert!(matches!(
            session.update(UpdateUiRequest {
                source: source.clone(),
                base_revision: 1,
                action: UiAction::Reset
            }),
            Err(UiError::Conflict)
        ));
        assert!(matches!(
            session.update(UpdateUiRequest {
                source: source.clone(),
                base_revision: 2,
                action: UiAction::Patch {
                    operations: vec![UiPatch::Remove { id: "anova".into() }]
                }
            }),
            Err(UiError::Invalid)
        ));
        let page = session.page(&source).unwrap();
        assert_eq!(page.revision, 2);
        assert!(!page.spec.elements["anova"].visible);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn intent_receipts_deduplicate_claim_once_and_require_the_claiming_window() {
        let session = PresentationSession::default();
        let intent = UiIntent::ShowPanel {
            panel: UiPanel::Assistant,
        };
        assert!(matches!(
            session.request_intent("key".into(), intent.clone()),
            Err(UiError::Workbench)
        ));
        session.attach_workbench();
        let receipt = session
            .request_intent("key".into(), intent.clone())
            .unwrap();
        assert_eq!(
            session.request_intent("key".into(), intent).unwrap(),
            receipt
        );
        assert!(
            session
                .settle_intent(&receipt.id, "main", UiIntentStatus::Claimed)
                .unwrap()
        );
        assert!(
            !session
                .settle_intent(&receipt.id, "main", UiIntentStatus::Claimed)
                .unwrap()
        );
        assert!(
            !session
                .settle_intent(&receipt.id, "other", UiIntentStatus::Applied)
                .unwrap()
        );
        assert!(
            session
                .settle_intent(&receipt.id, "main", UiIntentStatus::Applied)
                .unwrap()
        );
        assert_eq!(
            session.receipt(&receipt.id).unwrap().status,
            UiIntentStatus::Applied
        );
        assert!(session.pending().unwrap().is_empty());
    }
}
