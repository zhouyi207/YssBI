//! Headless composition using the same adapters as the desktop.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use yss_application::{
    ApplicationState,
    harness::{
        ApplicationCapabilityGateway, ApplicationResourceResolver,
        knowledge::ProjectKnowledgeService,
        models::{LanguageModelService, SystemModelCredentials},
    },
};
use yss_harness_contract::*;
use yss_harness_core::{HarnessHost, HarnessPorts};

use super::Error;
use super::model_calls::{CallLog, ObservedExecutor};

pub type SchemaLog = Arc<Mutex<Vec<SchemaMeasurement>>>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaMeasurement {
    role: AgentRole,
    tools: BTreeMap<String, usize>,
    total_bytes: usize,
}

pub async fn initialize(
    application: ApplicationState,
    output: PathBuf,
    inject_database_conflict: bool,
) -> Result<(Arc<HarnessHost>, SchemaLog, CallLog), Error> {
    let store = Arc::new(yss_harness_sqlite::SqliteHarnessStore::connect(output.clone()).await?);
    let schemas = SchemaLog::default();
    let calls = CallLog::default();
    let gateway = ApplicationCapabilityGateway::new(application.clone(), Arc::new(|_| {}));
    let gateway: Arc<dyn CapabilityGatewayPort> = if inject_database_conflict {
        Arc::new(super::concurrency::ConcurrentEdit {
            gateway,
            pending: std::sync::atomic::AtomicBool::new(true),
            evidence_path: output.join("concurrent-commit.json"),
        })
    } else {
        Arc::new(gateway)
    };
    let models = Arc::new(MeasuredModels {
        service: LanguageModelService::new(output, Arc::new(SystemModelCredentials)),
        schemas: schemas.clone(),
        calls: calls.clone(),
    });
    let clock = Arc::new(Clock);
    let knowledge = Arc::new(ProjectKnowledgeService::new(
        application.clone(),
        store.clone(),
        clock.clone(),
    ));
    let host = application
        .initialize_harness(HarnessPorts {
            models,
            resources: Arc::new(ApplicationResourceResolver(application.clone())),
            capability_gateway: gateway,
            sessions: store.clone(),
            events: store.clone(),
            event_sink: Arc::new(Progress),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            approvals: store,
            knowledge,
            knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            clock,
            ids: Arc::new(Ids),
        })
        .await?;
    Ok((host, schemas, calls))
}

struct Clock;
impl ClockPort for Clock {
    fn now(&self) -> UnixMillis {
        UnixMillis::from_existing(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| {
                    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
                }),
        )
    }
}

struct Ids;
impl IdGeneratorPort for Ids {
    fn next_id(&self, _kind: AutomationIdKind) -> Result<String, IdGenerationFailure> {
        Ok(uuid::Uuid::new_v4().to_string())
    }
}

struct Progress;
impl HarnessEventSinkPort for Progress {
    fn publish<'a>(
        &'a self,
        envelope: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        match &envelope.event {
            HarnessEvent::AgentRunStarted { role, .. } => println!("Agent started: {role:?}"),
            HarnessEvent::AgentRunFinished { outcome } => {
                println!("Agent finished: {:?}", outcome.state)
            }
            HarnessEvent::Agent(event) | HarnessEvent::AgentRunOutput { event, .. } => {
                match event {
                    AgentEvent::ToolInvocationStarted { capability_id, .. } => {
                        println!("Tool started: {}", capability_id.as_str())
                    }
                    AgentEvent::ToolInvocationFailed {
                        capability_id,
                        failure_code,
                        ..
                    } => println!("Tool failed: {} ({failure_code})", capability_id.as_str()),
                    AgentEvent::RuntimeStatus { phase, attempt } => {
                        println!("Agent status: {phase:?} ({attempt})")
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Box::pin(async { Ok(()) })
    }
}

struct MeasuredModels {
    service: LanguageModelService,
    schemas: SchemaLog,
    calls: CallLog,
}
impl LanguageModelResolverPort for MeasuredModels {
    fn resolve<'a>(
        &'a self,
        selection: Option<&'a LanguageModelSelection>,
    ) -> AgentFuture<'a, Result<ResolvedLanguageModel, AgentDriverFailure>> {
        Box::pin(async move {
            let resolved = self.service.resolve(selection).await?;
            Ok(ResolvedLanguageModel {
                identity: resolved.identity,
                driver: Arc::new(MeasuredDriver {
                    driver: resolved.driver,
                    schemas: self.schemas.clone(),
                    calls: self.calls.clone(),
                }),
            })
        })
    }
}

struct MeasuredDriver {
    driver: Arc<dyn AgentDriverPort>,
    schemas: SchemaLog,
    calls: CallLog,
}
impl AgentDriverPort for MeasuredDriver {
    fn run_turn<'a>(
        &'a self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            let measured = measure_schemas(&request)
                .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure))?;
            self.schemas
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(measured);
            let role = request.role;
            self.driver
                .run_turn(
                    request,
                    Arc::new(ObservedExecutor {
                        inner: capabilities,
                        role,
                        calls: self.calls.clone(),
                    }),
                    output,
                    cancellation,
                )
                .await
        })
    }
}

fn measure_schemas(request: &AgentTurnRequest) -> Result<SchemaMeasurement, serde_json::Error> {
    let mut tools = BTreeMap::new();
    for tool in &request.tools {
        tools.insert(
            tool.capability_id.as_str().to_owned(),
            serde_json::to_vec(&tool.input_schema)?.len(),
        );
    }
    for tool in &request.control_tools {
        let (name, schema) = match tool {
            AgentControlTool::DelegateTask => ("delegate_task", agent_task_schema()),
            AgentControlTool::FollowupTask => ("followup_task", agent_followup_schema()),
            AgentControlTool::ProposeStatisticalPlan => {
                ("propose_statistical_plan", statistical_plan_schema())
            }
        };
        tools.insert(name.to_owned(), serde_json::to_vec(&schema)?.len());
    }
    Ok(SchemaMeasurement {
        role: request.role,
        total_bytes: tools.values().sum(),
        tools,
    })
}
