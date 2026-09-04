use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use tauri::ipc::Channel;
use uuid::Uuid;
use yss_application::execution::ApplicationState;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_project_identity::ProjectInstanceId;

use crate::schema::application_event::graph_projection_replacement_to_transport;
use crate::schema::graph_projection_channel::{
    GraphProjectionChannelEventDto, GraphProjectionPublicationDto, GraphProjectionPublicationKey,
    GraphProjectionSnapshotDto, GraphProjectionSubscriptionDto,
};

const MAX_PENDING_GRAPH_RESOLUTIONS: usize = 256;
const MAX_PENDING_SUBSCRIBER_GRAPHS: usize = 256;
const MAX_GRAPH_PROJECTION_SUBSCRIBERS: usize = 16;
const MAX_TRACKED_GRAPH_PROJECTIONS: usize = 4_096;

#[derive(Clone)]
pub(crate) struct ResolveGraphProjectionRequest {
    pub project_instance_id: ProjectInstanceId,
    pub graph_session_id: String,
    pub graph_path: GraphResourcePath,
    pub request_generation: u64,
    pub locale: String,
    pub document: GraphDocument,
}

impl ResolveGraphProjectionRequest {
    fn key(&self) -> GraphProjectionPublicationKey {
        GraphProjectionPublicationKey {
            project_instance_id: self.project_instance_id.to_string(),
            graph_session_id: self.graph_session_id.clone(),
            graph_path: self.graph_path.as_str().to_owned(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GraphProjectionRuntimeError {
    #[error("graph projection request is invalid")]
    InvalidRequest,
    #[error("graph projection resolver queue is full")]
    QueueFull,
    #[error("graph projection runtime is unavailable")]
    Unavailable,
    #[error("graph projection worker failed to start")]
    WorkerSpawn(#[source] std::io::Error),
    #[error("graph projection subscription was not found")]
    SubscriptionNotFound,
    #[error("graph projection subscriber limit was reached")]
    SubscriberLimit,
}

struct ResolverQueueState {
    pending: BTreeMap<GraphProjectionPublicationKey, ResolveGraphProjectionRequest>,
    order: VecDeque<GraphProjectionPublicationKey>,
    shutdown: bool,
}

struct ResolverQueue {
    state: Mutex<ResolverQueueState>,
    wake: Condvar,
}

impl ResolverQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(ResolverQueueState {
                pending: BTreeMap::new(),
                order: VecDeque::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
        }
    }

    fn enqueue(
        &self,
        request: ResolveGraphProjectionRequest,
    ) -> Result<(), GraphProjectionRuntimeError> {
        let key = request.key();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.shutdown {
            return Err(GraphProjectionRuntimeError::Unavailable);
        }
        if let std::collections::btree_map::Entry::Occupied(mut entry) =
            state.pending.entry(key.clone())
        {
            entry.insert(request);
            self.wake.notify_one();
            return Ok(());
        }
        if state.pending.len() >= MAX_PENDING_GRAPH_RESOLUTIONS {
            return Err(GraphProjectionRuntimeError::QueueFull);
        }
        state.order.push_back(key.clone());
        state.pending.insert(key, request);
        self.wake.notify_one();
        Ok(())
    }

    fn next(&self) -> Option<ResolveGraphProjectionRequest> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if state.shutdown {
                return None;
            }
            while let Some(key) = state.order.pop_front() {
                if let Some(request) = state.pending.remove(&key) {
                    return Some(request);
                }
            }
            state = self
                .wake
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn shutdown(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.shutdown = true;
        state.pending.clear();
        state.order.clear();
        self.wake.notify_all();
    }
}

struct SubscriberQueueState {
    pending: BTreeMap<String, GraphProjectionChannelEventDto>,
    order: VecDeque<String>,
    closed: bool,
}

struct ProjectionSubscriber {
    project_instance_id: String,
    state: Arc<(Mutex<SubscriberQueueState>, Condvar)>,
    active: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ProjectionSubscriber {
    fn start(
        project_instance_id: String,
        channel: Channel<GraphProjectionChannelEventDto>,
    ) -> Result<Self, GraphProjectionRuntimeError> {
        let state = Arc::new((
            Mutex::new(SubscriberQueueState {
                pending: BTreeMap::new(),
                order: VecDeque::new(),
                closed: false,
            }),
            Condvar::new(),
        ));
        let active = Arc::new(AtomicBool::new(true));
        let worker_state = Arc::clone(&state);
        let worker_active = Arc::clone(&active);
        let worker = thread::Builder::new()
            .name("yssbi-graph-projection-subscriber".into())
            .spawn(move || run_subscriber(worker_state, worker_active, channel))
            .map_err(GraphProjectionRuntimeError::WorkerSpawn)?;
        Ok(Self {
            project_instance_id,
            state,
            active,
            worker: Some(worker),
        })
    }

    fn enqueue(&self, event: GraphProjectionChannelEventDto) -> bool {
        if !self.active.load(Ordering::Acquire) {
            return false;
        }
        let key = event.coalescing_key();
        let (state, wake) = &*self.state;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return false;
        }
        if let std::collections::btree_map::Entry::Occupied(mut entry) =
            state.pending.entry(key.clone())
        {
            entry.insert(event);
            wake.notify_one();
            return true;
        }
        if state.pending.len() >= MAX_PENDING_SUBSCRIBER_GRAPHS {
            state.closed = true;
            self.active.store(false, Ordering::Release);
            wake.notify_all();
            return false;
        }
        state.order.push_back(key.clone());
        state.pending.insert(key, event);
        wake.notify_one();
        true
    }

    fn close(&self) {
        self.active.store(false, Ordering::Release);
        let (state, wake) = &*self.state;
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        state.closed = true;
        state.pending.clear();
        state.order.clear();
        wake.notify_all();
    }
}

impl Drop for ProjectionSubscriber {
    fn drop(&mut self) {
        self.close();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!(
                target: "yssbi::graph_projection",
                diagnostic_domain = "system",
                diagnostic_event = "graphProjectionSubscriberJoinFailed",
                "Graph Projection subscriber worker terminated unexpectedly"
            );
        }
    }
}

fn run_subscriber(
    state: Arc<(Mutex<SubscriberQueueState>, Condvar)>,
    active: Arc<AtomicBool>,
    channel: Channel<GraphProjectionChannelEventDto>,
) {
    loop {
        let event = {
            let (state, wake) = &*state;
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if state.closed {
                    return;
                }
                if let Some(key) = state.order.pop_front() {
                    if let Some(event) = state.pending.remove(&key) {
                        break event;
                    }
                    continue;
                }
                state = wake.wait(state).unwrap_or_else(PoisonError::into_inner);
            }
        };
        if channel.send(event).is_err() {
            active.store(false, Ordering::Release);
            let (state, wake) = &*state;
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            state.closed = true;
            wake.notify_all();
            return;
        }
    }
}

struct RuntimeState {
    stream_id: String,
    latest_requested: BTreeMap<GraphProjectionPublicationKey, u64>,
    published: BTreeMap<GraphProjectionPublicationKey, GraphProjectionPublicationDto>,
    subscribers: BTreeMap<String, ProjectionSubscriber>,
}

impl RuntimeState {
    fn snapshot(&self, project_instance_id: &str) -> GraphProjectionSnapshotDto {
        let projections = self
            .published
            .values()
            .filter(|entry| entry.project_instance_id == project_instance_id)
            .cloned()
            .collect::<Vec<_>>();
        let latest_generation_by_graph = self
            .latest_requested
            .iter()
            .filter(|(key, _)| key.project_instance_id == project_instance_id)
            .map(|(key, generation)| (key.graph_path.clone(), *generation))
            .collect();
        GraphProjectionSnapshotDto {
            project_instance_id: project_instance_id.to_owned(),
            stream_id: self.stream_id.clone(),
            projections,
            latest_generation_by_graph,
        }
    }
}

struct GraphProjectionHub {
    state: Mutex<RuntimeState>,
}

impl GraphProjectionHub {
    fn new() -> Self {
        Self {
            state: Mutex::new(RuntimeState {
                stream_id: Uuid::new_v4().to_string(),
                latest_requested: BTreeMap::new(),
                published: BTreeMap::new(),
                subscribers: BTreeMap::new(),
            }),
        }
    }

    fn accept_request(
        &self,
        request: &ResolveGraphProjectionRequest,
    ) -> Result<(), GraphProjectionRuntimeError> {
        if request.graph_session_id.is_empty() || request.request_generation == 0 {
            return Err(GraphProjectionRuntimeError::InvalidRequest);
        }
        let key = request.key();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .latest_requested
            .get(&key)
            .is_some_and(|current| request.request_generation <= *current)
        {
            return Err(GraphProjectionRuntimeError::InvalidRequest);
        }
        let stale_keys = state
            .latest_requested
            .keys()
            .filter(|candidate| {
                candidate.project_instance_id == key.project_instance_id
                    && candidate.graph_path == key.graph_path
                    && candidate.graph_session_id != key.graph_session_id
            })
            .cloned()
            .collect::<Vec<_>>();
        for stale in stale_keys {
            state.latest_requested.remove(&stale);
            state.published.remove(&stale);
        }
        if !state.latest_requested.contains_key(&key)
            && state.latest_requested.len() >= MAX_TRACKED_GRAPH_PROJECTIONS
        {
            return Err(GraphProjectionRuntimeError::QueueFull);
        }
        state
            .latest_requested
            .insert(key, request.request_generation);
        Ok(())
    }

    fn is_latest(&self, key: &GraphProjectionPublicationKey, generation: u64) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .latest_requested
            .get(key)
            .is_some_and(|latest| *latest == generation)
    }

    fn publish(&self, publication: GraphProjectionPublicationDto) -> bool {
        let key = publication.key();
        let event = GraphProjectionChannelEventDto::Replaced {
            project_instance_id: publication.project_instance_id.clone(),
            graph_session_id: publication.graph_session_id.clone(),
            graph_path: publication.graph_path.clone(),
            request_generation: publication.request_generation,
            replacement: Box::new(publication.replacement.clone()),
        };
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .latest_requested
            .get(&key)
            .is_none_or(|latest| *latest != publication.request_generation)
        {
            return false;
        }
        if state.published.get(&key).is_some_and(|current| {
            publication.replacement.projection.source_revision
                < current.replacement.projection.source_revision
        }) {
            return false;
        }
        state.published.insert(key, publication.clone());
        state.subscribers.retain(|_, subscriber| {
            subscriber.project_instance_id != publication.project_instance_id
                || subscriber.enqueue(event.clone())
        });
        true
    }

    fn invalidate(
        &self,
        request: &ResolveGraphProjectionRequest,
        reason_code: &str,
        incident_id: Option<&str>,
    ) {
        let key = request.key();
        let event = GraphProjectionChannelEventDto::Invalidated {
            project_instance_id: key.project_instance_id.clone(),
            graph_session_id: key.graph_session_id.clone(),
            graph_path: key.graph_path.clone(),
            request_generation: request.request_generation,
            reason_code: reason_code.to_owned(),
            incident_id: incident_id.map(str::to_owned),
        };
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .latest_requested
            .get(&key)
            .is_none_or(|latest| *latest != request.request_generation)
        {
            return;
        }
        state.subscribers.retain(|_, subscriber| {
            subscriber.project_instance_id != key.project_instance_id
                || subscriber.enqueue(event.clone())
        });
    }

    fn subscribe(
        &self,
        project_instance_id: String,
        channel: Channel<GraphProjectionChannelEventDto>,
    ) -> Result<GraphProjectionSubscriptionDto, GraphProjectionRuntimeError> {
        if project_instance_id.is_empty() {
            return Err(GraphProjectionRuntimeError::InvalidRequest);
        }
        let subscription_id = Uuid::new_v4().to_string();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .latest_requested
            .retain(|key, _| key.project_instance_id == project_instance_id);
        state
            .published
            .retain(|key, _| key.project_instance_id == project_instance_id);
        state
            .subscribers
            .retain(|_, current| current.project_instance_id == project_instance_id);
        if state.subscribers.len() >= MAX_GRAPH_PROJECTION_SUBSCRIBERS {
            return Err(GraphProjectionRuntimeError::SubscriberLimit);
        }
        let subscriber = ProjectionSubscriber::start(project_instance_id.clone(), channel)?;
        state
            .subscribers
            .insert(subscription_id.clone(), subscriber);
        Ok(GraphProjectionSubscriptionDto {
            subscription_id,
            snapshot: state.snapshot(&project_instance_id),
        })
    }

    fn snapshot(&self, project_instance_id: &str) -> GraphProjectionSnapshotDto {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .snapshot(project_instance_id)
    }

    fn unsubscribe(&self, subscription_id: &str) -> Result<(), GraphProjectionRuntimeError> {
        let subscriber = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .subscribers
            .remove(subscription_id)
            .ok_or(GraphProjectionRuntimeError::SubscriptionNotFound)?;
        subscriber.close();
        Ok(())
    }

    fn shutdown(&self) {
        let subscribers = std::mem::take(
            &mut self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .subscribers,
        );
        for subscriber in subscribers.into_values() {
            subscriber.close();
        }
    }
}

pub struct GraphProjectionRuntime {
    hub: Arc<GraphProjectionHub>,
    queue: Arc<ResolverQueue>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl GraphProjectionRuntime {
    pub fn initialize(application: ApplicationState) -> Result<Self, GraphProjectionRuntimeError> {
        let hub = Arc::new(GraphProjectionHub::new());
        let queue = Arc::new(ResolverQueue::new());
        let worker_application = application.clone();
        let worker_hub = Arc::clone(&hub);
        let worker_queue = Arc::clone(&queue);
        let worker = thread::Builder::new()
            .name("yssbi-graph-projection-resolver".into())
            .spawn(move || run_resolver(worker_application, worker_hub, worker_queue))
            .map_err(GraphProjectionRuntimeError::WorkerSpawn)?;
        Ok(Self {
            hub,
            queue,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub(crate) fn submit(
        &self,
        request: ResolveGraphProjectionRequest,
    ) -> Result<(), GraphProjectionRuntimeError> {
        self.hub.accept_request(&request)?;
        if let Err(error) = self.queue.enqueue(request.clone()) {
            self.hub
                .invalidate(&request, "graph_projection_queue_full", None);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn subscribe(
        &self,
        project_instance_id: String,
        channel: Channel<GraphProjectionChannelEventDto>,
    ) -> Result<GraphProjectionSubscriptionDto, GraphProjectionRuntimeError> {
        self.hub.subscribe(project_instance_id, channel)
    }

    pub(crate) fn snapshot(&self, project_instance_id: &str) -> GraphProjectionSnapshotDto {
        self.hub.snapshot(project_instance_id)
    }

    pub(crate) fn unsubscribe(
        &self,
        subscription_id: &str,
    ) -> Result<(), GraphProjectionRuntimeError> {
        self.hub.unsubscribe(subscription_id)
    }
}

impl Drop for GraphProjectionRuntime {
    fn drop(&mut self) {
        self.queue.shutdown();
        self.hub.shutdown();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            && worker.join().is_err()
        {
            tracing::error!(
                target: "yssbi::graph_projection",
                diagnostic_domain = "system",
                diagnostic_event = "graphProjectionResolverJoinFailed",
                "Graph Projection resolver worker terminated unexpectedly"
            );
        }
    }
}

fn run_resolver(
    application: ApplicationState,
    hub: Arc<GraphProjectionHub>,
    queue: Arc<ResolverQueue>,
) {
    while let Some(request) = queue.next() {
        let started_at = Instant::now();
        let key = request.key();
        if !hub.is_latest(&key, request.request_generation) {
            continue;
        }
        match application.resolve_graph_draft_projection(
            request.project_instance_id.clone(),
            request.graph_path.clone(),
            request.locale.clone(),
            request.document.clone(),
        ) {
            Ok(replacement) => {
                let publication = GraphProjectionPublicationDto {
                    project_instance_id: key.project_instance_id.clone(),
                    graph_session_id: key.graph_session_id.clone(),
                    graph_path: key.graph_path.clone(),
                    request_generation: request.request_generation,
                    replacement: graph_projection_replacement_to_transport(&replacement),
                };
                if hub.publish(publication) {
                    tracing::debug!(
                        target: "yssbi::graph_projection",
                        diagnostic_domain = "graph",
                        diagnostic_event = "graphProjectionResolved",
                        project_instance_id = %key.project_instance_id,
                        graph_session_id = %key.graph_session_id,
                        graph_path = %key.graph_path,
                        request_generation = request.request_generation,
                        duration_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                        affected_graph_count = 1_u64,
                        "Resolved a Graph Projection"
                    );
                } else {
                    tracing::debug!(
                        target: "yssbi::graph_projection",
                        diagnostic_domain = "graph",
                        diagnostic_event = "staleProjectionDiscarded",
                        project_instance_id = %key.project_instance_id,
                        graph_session_id = %key.graph_session_id,
                        graph_path = %key.graph_path,
                        request_generation = request.request_generation,
                        "Discarded a stale Graph Projection result"
                    );
                }
            }
            Err(error) => {
                let incident_id = Uuid::new_v4().to_string();
                tracing::error!(
                    target: "yssbi::graph_projection",
                    diagnostic_domain = "graph",
                    diagnostic_event = "graphProjectionResolutionFailed",
                    incident_id = %incident_id,
                    project_instance_id = %key.project_instance_id,
                    graph_session_id = %key.graph_session_id,
                    graph_path = %key.graph_path,
                    request_generation = request.request_generation,
                    error = %error,
                    "Graph Projection resolution failed"
                );
                hub.invalidate(
                    &request,
                    "graph_projection_resolution_failed",
                    Some(&incident_id),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::application_event::GraphProjectionReplacementDto;
    use crate::schema::editor_projection_types::{
        CompilationOutcomeDto, EditorGraphProjectionDto, ProjectionBasis,
    };
    use yss_graph_registry::RegistryFingerprint;

    fn request(project: &str, session: &str, generation: u64) -> ResolveGraphProjectionRequest {
        ResolveGraphProjectionRequest {
            project_instance_id: ProjectInstanceId::from_existing(project.to_owned()),
            graph_session_id: session.to_owned(),
            graph_path: GraphResourcePath::new("events/main.yssbi-event")
                .expect("test Graph path is valid"),
            request_generation: generation,
            locale: "en-US".into(),
            document: GraphDocument::default(),
        }
    }

    fn publication(
        request: &ResolveGraphProjectionRequest,
        source_revision: u64,
    ) -> GraphProjectionPublicationDto {
        let graph_path = request.graph_path.as_str().to_owned();
        GraphProjectionPublicationDto {
            project_instance_id: request.project_instance_id.to_string(),
            graph_session_id: request.graph_session_id.clone(),
            graph_path: graph_path.clone(),
            request_generation: request.request_generation,
            replacement: GraphProjectionReplacementDto {
                graph_path: graph_path.clone(),
                projection: EditorGraphProjectionDto {
                    basis: ProjectionBasis {
                        graph_path: graph_path.clone().into(),
                        registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                        resource_versions: Default::default(),
                    },
                    graph_path: graph_path.into(),
                    source_revision,
                    nodes: Vec::new(),
                    connections: Vec::new(),
                    diagnostics: Vec::new(),
                    outcome: CompilationOutcomeDto::Success,
                    has_blocking_diagnostics: false,
                },
                function_editor_projection: None,
            },
        }
    }

    #[test]
    fn resolver_queue_coalesces_each_graph_to_the_latest_pending_generation() {
        let queue = ResolverQueue::new();
        queue.enqueue(request("project-a", "session-a", 1)).unwrap();
        queue.enqueue(request("project-a", "session-a", 2)).unwrap();

        let pending = queue.next().expect("latest request remains queued");

        assert_eq!(pending.request_generation, 2);
        queue.shutdown();
    }

    #[test]
    fn hub_rejects_stale_generation_revision_and_session_publications() {
        let hub = GraphProjectionHub::new();
        let first = request("project-a", "session-a", 1);
        hub.accept_request(&first).unwrap();
        assert!(hub.publish(publication(&first, 4)));

        let second = request("project-a", "session-a", 2);
        hub.accept_request(&second).unwrap();
        assert!(!hub.publish(publication(&first, 5)));
        assert!(!hub.publish(publication(&second, 3)));
        assert!(hub.publish(publication(&second, 5)));

        let replacement_session = request("project-a", "session-b", 1);
        hub.accept_request(&replacement_session).unwrap();
        assert!(!hub.publish(publication(&second, 6)));
        assert!(hub.publish(publication(&replacement_session, 5)));

        let snapshot = hub.snapshot("project-a");
        assert_eq!(snapshot.projections.len(), 1);
        assert_eq!(snapshot.projections[0].graph_session_id, "session-b");
        assert_eq!(
            snapshot.latest_generation_by_graph["events/main.yssbi-event"],
            1
        );
        assert!(hub.snapshot("project-b").projections.is_empty());
    }

    #[test]
    fn channel_event_serializes_the_canonical_projection_replaced_tag() {
        let request = request("project-a", "session-a", 2);
        let publication = publication(&request, 4);
        let event = GraphProjectionChannelEventDto::Replaced {
            project_instance_id: publication.project_instance_id,
            graph_session_id: publication.graph_session_id,
            graph_path: publication.graph_path,
            request_generation: publication.request_generation,
            replacement: Box::new(publication.replacement),
        };

        let wire = serde_json::to_value(event).expect("channel event serializes");

        assert_eq!(wire["type"], "projectionReplaced");
        assert_eq!(wire["graphSessionId"], "session-a");
        assert_eq!(wire["requestGeneration"], 2);
        assert_eq!(wire["replacement"]["graphPath"], "events/main.yssbi-event");

        let invalidated = GraphProjectionChannelEventDto::Invalidated {
            project_instance_id: "project-a".into(),
            graph_session_id: "session-a".into(),
            graph_path: "events/main.yssbi-event".into(),
            request_generation: 2,
            reason_code: "graph_projection_resolution_failed".into(),
            incident_id: Some("incident-42".into()),
        };
        let invalidated_wire =
            serde_json::to_value(invalidated).expect("invalidation event serializes");
        assert_eq!(invalidated_wire["type"], "projectionInvalidated");
        assert_eq!(invalidated_wire["incidentId"], "incident-42");
    }
}
