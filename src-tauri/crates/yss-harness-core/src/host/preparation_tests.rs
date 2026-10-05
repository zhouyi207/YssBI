use crate::test_support::{
    FixedClock, InMemoryHarnessStore, MockAgentDriver, RejectingCapabilityGateway, SequentialIds,
    model_identity,
};
use crate::{HarnessError, HarnessHost, HarnessPorts};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;
use yss_harness_contract::*;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

#[derive(Default)]
struct PausedReferences {
    started: Arc<Notify>,
    release: Arc<Notify>,
    finished: Arc<Notify>,
}

impl HarnessResourceResolverPort for PausedReferences {
    fn resolve<'a>(
        &'a self,
        _: &'a ProjectSessionBinding,
        references: &'a [ProjectResourceRef],
        _: CancellationToken,
    ) -> AgentFuture<'a, Result<Vec<HarnessResourceReference>, CapabilityFailure>> {
        let started = self.started.clone();
        let release = self.release.clone();
        let finished = self.finished.clone();
        let references = references.to_vec();
        Box::pin(async move {
            // Like spawn_blocking, the lookup may finish after its waiter is dropped.
            tokio::spawn(async move {
                started.notify_one();
                release.notified().await;
                finished.notify_one();
                Ok(references
                    .into_iter()
                    .map(|resource| HarnessResourceReference {
                        name: resource.id.clone(),
                        resource,
                    })
                    .collect())
            })
            .await
            .unwrap()
        })
    }
}

#[derive(Default)]
struct ObservedModels(AtomicUsize);

impl LanguageModelResolverPort for ObservedModels {
    fn resolve<'a>(
        &'a self,
        _: Option<&'a LanguageModelSelection>,
    ) -> AgentFuture<'a, Result<ResolvedLanguageModel, AgentDriverFailure>> {
        Box::pin(async {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ResolvedLanguageModel {
                identity: model_identity(),
                driver: Arc::new(MockAgentDriver::new("Complete")),
            })
        })
    }
}

#[tokio::test]
async fn cancelled_reference_lookup_cannot_start_a_model_or_block_the_next_turn() {
    let store = Arc::new(InMemoryHarnessStore::default());
    let resources = Arc::new(PausedReferences::default());
    let models = Arc::new(ObservedModels::default());
    let host = Arc::new(
        HarnessHost::new(HarnessPorts {
            models: models.clone(),
            resources: resources.clone(),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            approvals: store,
            clock: Arc::new(FixedClock::new(1000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap(),
    );
    let session = host
        .create_session(
            PrincipalId::try_new("user").unwrap(),
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project".into()),
                ProjectSessionId::new("project-session"),
            ),
        )
        .await
        .unwrap();
    let before = host.events_after(&session.id, 0).await.unwrap();
    let running = {
        let host = host.clone();
        let session = session.clone();
        tokio::spawn(async move {
            host.submit_turn(
                &session.id,
                &session.project,
                "Inspect the dataset".into(),
                vec![ProjectResourceRef {
                    kind: ProjectResourceKind::Database,
                    id: "dataset".into(),
                }],
                None,
            )
            .await
        })
    };
    resources.started.notified().await;
    assert!(host.cancel_turn(&session.id));
    let cancelled = tokio::time::timeout(std::time::Duration::from_secs(1), running)
        .await
        .expect("cancellation must not wait for project I/O")
        .unwrap();
    assert!(matches!(cancelled, Err(HarnessError::Cancelled)));
    resources.release.notify_one();
    resources.finished.notified().await;
    assert_eq!(models.0.load(Ordering::SeqCst), 0);
    assert_eq!(host.events_after(&session.id, 0).await.unwrap(), before);
    let next = host
        .submit_turn(
            &session.id,
            &session.project,
            "Continue".into(),
            vec![],
            None,
        )
        .await
        .unwrap();
    assert_eq!(next.final_text, "Complete");
    assert_eq!(models.0.load(Ordering::SeqCst), 1);
}
