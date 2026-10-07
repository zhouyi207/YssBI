//! Session-scoped workbench intents. Business data stays with Project and Results.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use yss_graph_execution::result::ResultId;
use yss_project_identity::ProjectInstanceId;
use yss_ui_contract::*;

use crate::session::{ApplicationSession, ApplicationState};

#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("ui_intent_invalid")]
    Invalid,
    #[error("ui_intent_conflict")]
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
            Self::Invalid => "ui_intent_invalid",
            Self::Conflict => "ui_intent_conflict",
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

/// A desktop workbench's session-scoped delivery and claim ownership.
pub struct WorkbenchBinding {
    application: ApplicationState,
    session: Arc<ApplicationSession>,
    claimant: String,
    _subscription: UiSubscription,
}

impl WorkbenchBinding {
    pub fn pending(&self) -> Result<Vec<UiIntentReceipt>, UiError> {
        self.validate()?;
        self.session.presentation.pending()
    }

    pub fn settle(&self, id: &str, status: UiIntentStatus) -> Result<bool, UiError> {
        self.validate()?;
        self.session
            .presentation
            .settle_intent(id, &self.claimant, status)
    }

    fn validate(&self) -> Result<(), UiError> {
        self.application
            .revalidate_captured_session(&self.session)
            .map_err(|_| UiError::Session)
    }
}

impl Drop for WorkbenchBinding {
    fn drop(&mut self) {
        self.session.presentation.detach_workbench();
    }
}

impl Drop for UiSubscription {
    fn drop(&mut self) {
        let observer = self
            .source
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .observers
            .remove(&self.id);
        drop(observer);
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
    pub fn attach_workbench(
        &self,
        project: &ProjectInstanceId,
        claimant: String,
        observer: Arc<dyn Fn(UiEvent) + Send + Sync>,
    ) -> Result<WorkbenchBinding, UiError> {
        if claimant.is_empty() {
            return Err(UiError::Invalid);
        }
        let session = self.ui_session(project)?;
        let subscription = session.presentation.subscribe(observer)?;
        self.revalidate_captured_session(&session)
            .map_err(|_| UiError::Session)?;
        session.presentation.attach_workbench();
        Ok(WorkbenchBinding {
            application: self.clone(),
            session,
            claimant,
            _subscription: subscription,
        })
    }

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

    pub fn inspect_ui_intent(
        &self,
        project: &ProjectInstanceId,
        request: InspectUiIntentRequest,
    ) -> Result<UiIntentReceipt, UiError> {
        let session = self.ui_session(project)?;
        let receipt = session.presentation.receipt(&request.id)?;
        self.revalidate_captured_session(&session)
            .map_err(|_| UiError::Session)?;
        Ok(receipt)
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
            UiIntent::OpenResult { source } => validate_source(&session, source)?,
            UiIntent::OpenResource { resource, node_id } => {
                let index = session
                    .project()
                    .read_project_index(project)
                    .map_err(|_| UiError::Unavailable)?;
                use yss_project_identity::ProjectResourceKind as Kind;
                let present = match resource.kind {
                    Kind::EventGraph => index
                        .event_graphs
                        .iter()
                        .any(|entry| entry.path == resource.id),
                    Kind::FunctionGraph => index
                        .function_graphs
                        .iter()
                        .any(|entry| entry.path == resource.id),
                    Kind::Chart => index
                        .charts
                        .iter()
                        .any(|entry| entry.chart_path.as_str() == resource.id),
                    Kind::Mind => index
                        .minds
                        .iter()
                        .any(|entry| entry.path.as_str() == resource.id),
                    Kind::Doc => index
                        .docs
                        .iter()
                        .any(|entry| entry.path.as_str() == resource.id),
                    Kind::Database => index.databases.iter().any(|entry| entry.id == resource.id),
                };
                if !present
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
}

fn validate_source(session: &ApplicationSession, source: &UiSource) -> Result<(), UiError> {
    source.validate().map_err(|_| UiError::Invalid)?;
    if source.execution_session_id != session.execution_session_id().as_uuid().to_string() {
        return Err(UiError::Unavailable);
    }
    let id: u64 = source.result_id.parse().map_err(|_| UiError::Invalid)?;
    if id.to_string() != source.result_id {
        return Err(UiError::Invalid);
    }
    session
        .execution()
        .query_result(ResultId::from_existing(id))
        .ok_or(UiError::Unavailable)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_binding_delivers_and_settles_intents_then_detaches_on_drop() {
        let application = ApplicationState::initialize().unwrap();
        let project = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let delivered = events.clone();
        let binding = application
            .attach_workbench(
                &project,
                "native-main".into(),
                Arc::new(move |event| delivered.lock().unwrap().push(event)),
            )
            .unwrap();
        let request = |key: &str| RequestUiIntent {
            client_key: key.into(),
            intent: UiIntent::ShowPanel {
                panel: UiPanel::Project,
            },
        };
        let receipt = application
            .request_ui_intent(&project, "test", request("open-project"))
            .unwrap();
        assert!(
            matches!(&events.lock().unwrap()[0],UiEvent::Intent{receipt:delivered} if delivered==&receipt)
        );
        assert_eq!(binding.pending().unwrap(), vec![receipt.clone()]);
        assert!(
            binding
                .settle(&receipt.id, UiIntentStatus::Claimed)
                .unwrap()
        );
        assert!(
            binding
                .settle(&receipt.id, UiIntentStatus::Applied)
                .unwrap()
        );
        assert!(binding.pending().unwrap().is_empty());
        drop(binding);
        assert!(matches!(
            application.request_ui_intent(&project, "test", request("after-close")),
            Err(UiError::Workbench)
        ));
        assert!(
            application
                .capture_session()
                .unwrap()
                .presentation
                .events
                .lock()
                .unwrap()
                .observers
                .is_empty()
        );
    }

    #[test]
    fn subscription_releases_observer_outside_registry_lock() {
        use std::sync::{Weak, atomic::AtomicBool};

        struct ReentrantDrop {
            session: Weak<PresentationSession>,
            lock_available: Arc<AtomicBool>,
            reentered: Arc<AtomicBool>,
        }

        impl Drop for ReentrantDrop {
            fn drop(&mut self) {
                let session = self.session.upgrade().unwrap();
                let lock_available = session.events.try_lock().is_ok();
                self.lock_available.store(lock_available, Ordering::SeqCst);
                if lock_available {
                    let subscription = session.subscribe(Arc::new(|_| {})).unwrap();
                    drop(subscription);
                    self.reentered.store(true, Ordering::SeqCst);
                }
            }
        }

        let session = Arc::new(PresentationSession::default());
        let lock_available = Arc::new(AtomicBool::new(false));
        let reentered = Arc::new(AtomicBool::new(false));
        let cleanup = ReentrantDrop {
            session: Arc::downgrade(&session),
            lock_available: lock_available.clone(),
            reentered: reentered.clone(),
        };
        let subscription = session
            .subscribe(Arc::new(move |_| {
                let _ = &cleanup;
            }))
            .unwrap();

        drop(subscription);
        assert!(lock_available.load(Ordering::SeqCst));
        assert!(reentered.load(Ordering::SeqCst));
        assert!(session.events.lock().unwrap().observers.is_empty());
    }

    #[test]
    fn open_result_intents_follow_result_retention() {
        let (application, reference, _) = crate::graph::results::report::tests::fixture(12);
        let session = application.capture_session().unwrap();
        let project = session.project_instance_id();
        session.presentation.attach_workbench();
        let source = UiSource {
            execution_session_id: reference.execution_session_id.as_uuid().to_string(),
            result_id: reference.result_id.get().to_string(),
        };
        let open = || {
            application.request_ui_intent(
                project,
                "test",
                RequestUiIntent {
                    client_key: "open-result".into(),
                    intent: UiIntent::OpenResult {
                        source: source.clone(),
                    },
                },
            )
        };
        let original = open().unwrap();
        let [first, second] = [uuid::Uuid::new_v4(), uuid::Uuid::new_v4()];
        application
            .retain_result(reference, first, "first", None)
            .unwrap();
        application
            .retain_result(reference, second, "second", None)
            .unwrap();
        session
            .execution()
            .invalidate_graph_results("events/report.yssbi-event");
        application.release_result_lease(first, "first").unwrap();
        assert_eq!(open().unwrap(), original);
        application.release_result_lease(second, "second").unwrap();
        assert!(matches!(open(), Err(UiError::Unavailable)));
    }

    #[test]
    fn gui_and_harness_inspect_the_same_intent_receipt() {
        use yss_harness_contract::{
            AutomationCapabilityRequest, AutomationCapabilityResult, CancellationToken,
            CapabilityControl, CapabilityInvocationContext, CapabilityInvocationId,
            HarnessSessionId, PrincipalId, ProjectSessionBinding,
        };
        let (application, _, _) = crate::graph::results::report::tests::fixture(12);
        let session = application.capture_session().unwrap();
        let project = session.project_instance_id();
        session.presentation.attach_workbench();
        let receipt = application
            .request_ui_intent(
                project,
                "test",
                RequestUiIntent {
                    client_key: "show-details".into(),
                    intent: UiIntent::ShowPanel {
                        panel: UiPanel::Details,
                    },
                },
            )
            .unwrap();
        let request = InspectUiIntentRequest {
            id: receipt.id.clone(),
        };
        let context = CapabilityInvocationContext::new(
            PrincipalId::try_new("user").unwrap(),
            HarnessSessionId::try_new("session").unwrap(),
            CapabilityInvocationId::try_new("invocation").unwrap(),
            ProjectSessionBinding::new(project.clone(), session.project_session_id().clone()),
        );
        let control = CapabilityControl::new(CancellationToken::default(), Duration::from_secs(10));
        let result = application
            .invoke_automation_capability(
                context,
                AutomationCapabilityRequest::InspectUiIntent(request.clone()),
                &control,
                &mut |_| {},
            )
            .unwrap();
        let AutomationCapabilityResult::UiIntentInspection(inspected) = result else {
            panic!()
        };
        assert_eq!(inspected, receipt);
        assert_eq!(
            application.inspect_ui_intent(project, request).unwrap(),
            receipt
        );
        let (sender, receiver) = std::sync::mpsc::channel();
        let _subscription = session
            .presentation
            .subscribe(Arc::new(move |event| {
                let _ = sender.send(event);
            }))
            .unwrap();
        session.presentation.session_changed();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            UiEvent::SessionChanged
        ));
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
