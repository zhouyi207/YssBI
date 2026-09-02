# YssBI Statistical Harness 架构

状态：MVP Implemented；后续高风险阶段 gated

日期：2026-09-01

本规范定义 YssBI 专业统计 Harness 的目标边界、authority、contracts、状态机、持久化、Tool、Workflow、Skill、Memory、Knowledge Base、Rig 与 MCP adapter，以及前后端交付顺序。

在 Foundation、Harness Core、Persistence 和 read-only capabilities 分别通过验收之前，本设计保持 production-unrouted；不得为尽快启用 Assistant 而增加直接 ProjectState、Tauri command、Rig tool 或 loopback MCP 路径。

截至 2026-09-01，Foundation、Durable Harness Core、SQLite、read-only Gateway、Rig、Tauri ordered event replay、assistant-ui projection、builtin Skill、lexical Knowledge、Session Memory 与 MVP Workflow 已实现。`apply_graph_edit` 的 typed batch、approval、ledger 与 receipt 基础已实现但未注册到默认 Rig/MCP/Tauri 路径；MCP Client、external write tools、未知 commit outcome reconciliation 和 persistent User Memory 继续保持 gated。

## 1. 问题与目标

本规范起草时，Assistant 仅是 frontend-only Workbench shell：

- assistant-ui ExternalStoreRuntime 使用空消息投影；
- composer 可编辑但发送禁用；
- 没有模型、后端 session、IPC、Tool、Memory 或 MCP 路径。

上述起点现已由 Rust-authoritative Harness event projection 取代，保留在此作为问题背景而非当前实现描述。

目标不是增加一个普通聊天接口，而是建立：

> 面向专业统计研究的、可审计、可恢复、可引用、可扩展的 Agent Runtime。

Statistical Harness 必须支持：

- 类型化 Tool 系统；
- 可验证、可持久化、可恢复的 Workflow；
- 版本化统计 Skill；
- MCP Server 与 MCP Client adapter；
- 分层、可治理、可失效的 Memory；
- 结构化统计方法库和可引用 Knowledge Base；
- Rig 或其他模型框架的可替换 driver；
- project/session/revision-aware execution；
- approval、deadline、cancellation、idempotency 和 durable receipt；
- 基于 Statistical Evidence 的解释与报告。

## 2. 非目标

本设计明确不做：

- 把现有 Tauri commands 自动暴露为模型 Tool；
- 让 Rig、rmcp、assistant-ui 或向量数据库成为领域 authority；
- 使用 MCP 作为 YssBI 内部模块总线；
- 让模型直接访问 ProjectState、GraphDocument、DataFrame、SQL 或文件系统；
- 让模型自行计算 p-value、标准误、置信区间或诊断统计量；
- 将完整聊天记录等同于 Memory；
- 将向量索引等同于 Knowledge authority；
- 把 prompt、数据行、SQL、连接字符串、token 或模型输出写入 diagnostics logs；
- 在前端建立 conversation、workflow 或 approval authority；
- 首阶段引入多 Agent swarm、自治后台执行或无确认的项目写入。

## 3. 核心术语

| 概念         | 定义                                                                          | 示例                               |
| ------------ | ----------------------------------------------------------------------------- | ---------------------------------- |
| Capability   | Application 提供的稳定业务能力                                                | Inspect Graph、Apply Graph Edit    |
| Tool         | Capability 的 model-facing typed projection                                   | `inspect_dataset_schema`           |
| Workflow     | 版本化、可持久执行的 typed step graph                                         | OLS 完整诊断流程                   |
| Skill        | 领域方法包：说明、Workflow、允许 Tool、知识范围和 eval                        | Panel Model Selection              |
| Memory       | 与用户、项目或研究过程相关的可治理长期事实                                    | 变量含义、方法决策                 |
| Knowledge    | 有来源、版本和引用的专业知识                                                  | 方法假设、软件文档                 |
| Evidence     | Tool 产生的结构化统计证据                                                     | Estimate、SE、Diagnostic、ResultId |
| Harness      | 组合 session、planning、workflow、tools、skills、memory 和 knowledge 的运行时 |
| Agent Driver | 模型 provider/framework adapter                                               | Rig driver                         |
| MCP Adapter  | 外部 MCP wire 与内部 contracts 的映射层                                       | rmcp server/client                 |

Tool、Workflow、Skill、Memory、Knowledge 和 MCP 不能互相替代。

## 4. Authority 不变量

### 4.1 业务 authority

Rust 现有 owner 保持不变：

| 状态                                | Authority                                           |
| ----------------------------------- | --------------------------------------------------- |
| Project、资源、revision、history    | `yss-project`                                       |
| Graph document、编辑与编译          | Graph crates                                        |
| Database declaration/runtime/schema | Database crates                                     |
| Execution、Result、Run Output       | Execution/Result owners                             |
| Statistical algorithms              | SCI/Bayes runtime owners                            |
| Chart document                      | Chart document owner（由 Worksheet 全栈改名后形成） |

Harness 只能持有业务资源的 opaque references、captured revisions、receipts 和 bounded projections。

### 4.2 Harness authority

`yss-statistical-harness` 唯一拥有：

- Harness session；
- Assistant turn；
- conversation transcript；
- Statistical Plan；
- Workflow run/step state；
- Tool invocation ledger；
- approval lifecycle；
- Memory proposal lifecycle；
- ordered Harness event sequence；
- context assembly policy；
- late-result suppression。

### 4.3 Persistence authority

- Harness operational database 是 session、turn、workflow、ledger 和 user-memory 的 authority。
- Project Memory 若需要随项目移动，必须成为 Project-owned explicit resource，不能只保存在 app-data DB。
- Knowledge source document/manifest 是 authority；lexical/vector index 是可重建 projection。
- Result payload 继续由 ResultStore 拥有；Harness 只保存 `ResultId` 和 Evidence references。

### 4.4 Frontend authority

React 只保存 Harness ordered events 的 UI projection。assistant-ui ExternalStoreRuntime 只负责消息、状态、composer 和 UI actions 的投影，不成为 conversation 或 workflow authority。

## 5. 总体依赖结构

```text
Assistant UI / Workflow UI / CLI / MCP
                   │
                   ▼
       ┌─────────────────────────┐
       │ yss-statistical-harness │
       │ Session / Turn          │
       │ Planner / Workflow      │
       │ Tool / Skill            │
       │ Memory / Knowledge      │
       │ Policy / Approval       │
       │ Ordered Events          │
       └────────────┬────────────┘
                    │ typed ports
        ┌───────────┼────────────┐
        ▼           ▼            ▼
 AgentDriver    Capability    Persistence
    Port         Gateway         Ports
        │           │            │
        ▼           ▼            ▼
 yss-agent-rig  yss-application  SQLite / Index
```

禁止依赖：

```text
yss-statistical-harness -> yss-application concrete types
yss-statistical-harness -> ProjectState / Graph / Database concrete owners
yss-statistical-harness -> Tauri / yss-api
yss-statistical-harness -> Rig / rmcp / SQLite / vector store
yss-agent-rig           -> Project / Graph / Database authority
yss-application         -> Rig / rmcp
yss-mcp-*               -> ProjectState / Graph runtime directly
```

## 6. Crate 布局

### 6.1 第一阶段 crates

只创建具有真实责任的四个 crate：

```text
yss-automation-contract
yss-statistical-harness
yss-statistical-harness-sqlite
yss-agent-rig
```

### 6.2 后续可提取 crates

只有出现独立实现或多个真实消费者后再提取：

```text
yss-statistical-knowledge
yss-statistical-memory
yss-mcp-server
yss-mcp-client
```

不得先创建空 crate 或兼容 facade。

### 6.3 `yss-automation-contract`

Pure Leaf，拥有跨 Harness、Application、Rig 和 MCP 共享的稳定 typed contracts：

```text
ToolId / ToolVersion
CapabilityId / CapabilityVersion
WorkflowId / WorkflowVersion / WorkflowRunId
SkillId / SkillVersion
MemoryRecordId
KnowledgeSourceId / KnowledgeCitation
HarnessSessionId / HarnessTurnId
CapabilityInvocationId
ApprovalGrantId
Principal
ProjectSessionBinding
Deadline / CancellationReason
AutomationFailureCode / AutomationFailureDetails
```

不包含：

- Rig/rmcp/Tauri types；
- ProjectState、GraphDocument 或 DataFrame；
- provider response；
- user-facing localized prose；
- storage implementation types。

### 6.4 `yss-statistical-harness`

Application service / Automation Core，拥有 provider-neutral behavior。

建议模块：

```text
src/
├─ lib.rs
├─ host.rs
├─ session.rs
├─ turn.rs
├─ planner.rs
├─ context.rs
├─ events.rs
├─ error.rs
├─ ports.rs
├─ policy/
│  ├─ approval.rs
│  ├─ data_access.rs
│  ├─ retention.rs
│  └─ tool_policy.rs
├─ tools/
│  ├─ descriptor.rs
│  ├─ registry.rs
│  ├─ executor.rs
│  └─ ledger.rs
├─ workflow/
│  ├─ definition.rs
│  ├─ compiler.rs
│  ├─ runtime.rs
│  ├─ recovery.rs
│  └─ state.rs
├─ skills/
│  ├─ manifest.rs
│  ├─ package.rs
│  ├─ registry.rs
│  └─ resolver.rs
├─ memory/
│  ├─ record.rs
│  ├─ proposal.rs
│  ├─ policy.rs
│  ├─ retrieval.rs
│  └─ invalidation.rs
└─ knowledge/
   ├─ document.rs
   ├─ method_card.rs
   ├─ query.rs
   ├─ citation.rs
   └─ retrieval.rs
```

### 6.5 `yss-statistical-harness-sqlite`

Backend Adapter，实现 Harness persistence ports。不得拥有 Harness policy、workflow transition 或 memory validation。

### 6.6 `yss-agent-rig`

Automation Adapter，实现 `AgentDriverPort`：

- Rig Agent 构造；
- provider/model 配置；
- completion/streaming；
- model-facing Tool schema 映射；
- tool-call argument decode；
- embedding port adapter；
- provider error 分类。

不拥有 approval、project currentness、session、workflow、memory 或 capability authorization。

## 7. Harness Ports

Harness 通过 constructor injection 获取全部外部能力，不使用 service locator。

### 7.1 Agent Driver

```rust
pub trait AgentDriverPort: Send + Sync {
    fn run_turn(
        &self,
        request: AgentTurnRequest,
        capabilities: Arc<dyn ModelCapabilityExecutor>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> BoxFuture<'static, Result<AgentTurnResult, AgentDriverError>>;
}
```

### 7.2 Capability Gateway

```rust
pub trait CapabilityGatewayPort: Send + Sync {
    fn invoke(
        &self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
    ) -> BoxFuture<'static, Result<AutomationCapabilityResult, CapabilityError>>;
}
```

具体实现由 `yss-application` 提供。Gateway 是 Harness、MCP Server 和其他 automation clients 复用业务能力的唯一入口。

### 7.3 Persistence Ports

```text
HarnessSessionStorePort
HarnessEventStorePort
WorkflowStorePort
ToolInvocationLedgerPort
ApprovalStorePort
MemoryStorePort
KnowledgeSourceStorePort
KnowledgeIndexPort
SkillSourcePort
CredentialPort
ClockPort
IdGeneratorPort
```

CredentialPort 必须由平台安全存储实现；token/API key 不进入 SQLite、Project 或 logs。

## 8. Capability Gateway

不得将 Tauri command registry 直接转换成 Tools。

`yss-application` 新增 automation modules：

```text
automation/
├─ gateway.rs
├─ inspect_project.rs
├─ inspect_graph.rs
├─ inspect_dataset_schema.rs
├─ inspect_dataset_profile.rs
├─ inspect_result.rs
├─ search_node_catalog.rs
└─ apply_graph_edit.rs
```

每次调用必须验证：

- principal；
- Harness session；
- project instance/session；
- resource revision；
- approval grant；
- deadline；
- cancellation；
- invocation idempotency；
- bounded data exposure。

第一阶段 gateway 只实现 read-only capabilities，不连接 production Assistant。

## 9. Tool 系统

### 9.1 Tool Descriptor

```rust
pub struct ToolDescriptor {
    pub id: ToolId,
    pub version: ToolVersion,
    pub input_schema: JsonSchema,
    pub output_schema: JsonSchema,
    pub effect: ToolEffect,
    pub approval: ApprovalPolicy,
    pub data_access: DataAccessPolicy,
    pub timeout: Duration,
    pub idempotency: IdempotencyPolicy,
    pub result_budget: ResultBudget,
}
```

```rust
pub enum ToolEffect {
    Inspect,
    Compute,
    Mutate,
    Destructive,
    External,
}
```

Tool schema 在 model boundary 生成；Harness 内部继续使用强类型 request/result，不使用任意 JSON 作为行为契约。

### 9.2 第一阶段 Tools

Read-only：

```text
inspect_project
inspect_graph
inspect_dataset_schema
inspect_dataset_profile
inspect_result
search_node_catalog
search_statistical_knowledge
```

Compute：

```text
run_descriptive_statistics
run_correlation_analysis
run_hypothesis_test
run_regression_diagnostics
run_stationarity_tests
run_serial_tests
run_bayesian_diagnostics
```

Mutating tools 后置：

```text
apply_graph_edit
create_chart
update_chart
annotate_variable
save_analysis_report
```

### 9.3 Statistical Evidence

Tool 不能只返回自然语言：

```rust
pub struct StatisticalEvidence {
    pub method_id: StatisticalMethodId,
    pub result_refs: Vec<ResultId>,
    pub dataset_revision: ResourceRevision,
    pub graph_revision: Option<GraphRevision>,
    pub estimates: Vec<EstimateSummary>,
    pub uncertainty: Vec<UncertaintySummary>,
    pub diagnostics: Vec<DiagnosticSummary>,
    pub provenance: EvidenceProvenance,
}
```

模型只解释 Evidence，不自行生成数值事实。

### 9.4 Tool Ledger

每次调用持久化：

```text
session/turn/workflow/step identity
tool id/version/schema hash
invocation id/idempotency key
principal/project/session binding
captured resource revisions
approval receipt
start/deadline/end
structured receipt/result refs
failure code/incident id
```

active session ledger 不允许淘汰。

## 10. Professional Statistical Plan

专业 Harness 不采用完全自由的 ReAct loop。标准路径：

```text
User Question
    ↓
Question Framing
    ↓
Model proposes StatisticalPlan
    ↓
Harness compiles and validates
    ↓
Approval when required
    ↓
Durable Workflow execution
    ↓
Assumption / Diagnostic gates
    ↓
Evidence-based synthesis
    ↓
Reproducible report
```

```rust
pub struct StatisticalPlan {
    pub research_question: ResearchQuestion,
    pub analysis_mode: AnalysisMode,
    pub study_design: StudyDesign,
    pub estimands: Vec<Estimand>,
    pub variable_roles: Vec<VariableRoleAssignment>,
    pub candidate_methods: Vec<StatisticalMethodId>,
    pub selected_workflow: WorkflowId,
    pub required_diagnostics: Vec<DiagnosticRequirement>,
    pub robustness_checks: Vec<RobustnessCheck>,
    pub reporting_contract: ReportingContract,
}
```

`AnalysisMode` 至少区分：

```text
Confirmatory
Exploratory
PostHoc
```

## 11. Durable Workflow

### 11.1 Workflow Definition

```rust
pub struct WorkflowDefinition {
    pub id: WorkflowId,
    pub version: WorkflowVersion,
    pub input_schema: JsonSchema,
    pub steps: Vec<WorkflowStep>,
    pub policies: WorkflowPolicies,
    pub output_contract: WorkflowOutputContract,
}
```

```rust
pub enum WorkflowStepKind {
    Tool(ToolInvocationTemplate),
    Decision(DecisionRule),
    Approval(ApprovalRequirement),
    ModelPlanning(ModelPlanningStep),
    ModelSynthesis(ModelSynthesisStep),
    Subworkflow(WorkflowId),
}
```

### 11.2 Run 状态

```rust
pub enum WorkflowRunState {
    Planned,
    WaitingForApproval,
    Ready,
    Running,
    Paused,
    WaitingForExternalInput,
    Completed,
    Failed,
    Cancelled,
}
```

```rust
pub enum WorkflowStepState {
    Pending,
    Running,
    Succeeded,
    RetriableFailure,
    TerminalFailure,
    Skipped,
}
```

### 11.3 Recovery

应用重启后：

- read-only/idempotent steps 可在 currentness 验证后恢复；
- mutating steps 必须先检查 durable commit receipt；
- 未知 commit outcome 进入 reconciliation，不允许盲目重试；
- Project replacement 使旧 project-bound run pause/stale；
- commit point-of-no-return 后不能把已发生写入报告为取消成功。

### 11.4 内置 Workflows

```text
dataset_quality_review
exploratory_data_analysis
ols_model_and_diagnostics
panel_data_model_selection
time_series_stationarity_and_modeling
causal_did_analysis
instrumental_variable_analysis
bayesian_model_building
bayesian_convergence_review
robustness_and_sensitivity_analysis
publication_report_generation
```

## 12. Skill 系统

Skill 是版本化统计方法包，不是可执行任意脚本。

```text
skills/
└─ ols-diagnostics/
   ├─ skill.toml
   ├─ SKILL.md
   ├─ workflows/
   │  └─ default.workflow.yaml
   ├─ references/
   └─ evals/
```

```toml
id = "yssbi.statistics.ols-diagnostics"
version = "1.0.0"
domain = "regression"
entry_workflow = "ols_model_and_diagnostics"

allowed_tools = [
  "inspect_dataset_schema",
  "run_descriptive_statistics",
  "run_regression_diagnostics"
]

knowledge_scopes = [
  "statistics.regression.ols",
  "statistics.diagnostics"
]

memory_policy = "project-read-session-write"
approval_policy = "compute-auto-mutate-confirm"
```

Skill 必须定义：

- 适用场景和非适用场景；
- 必要输入；
- variable role 与 study design 要求；
- method selection rules；
- assumptions 和 diagnostics；
- stop/fallback conditions；
- alternatives；
- reporting contract；
- allowed tools；
- workflow templates；
- representative eval fixtures。

有效权限为：

```text
Skill allowed tools
∩ principal/project policy
∩ Tool policy
∩ current approval grant
```

Skill scopes：

```text
builtin  immutable, versioned, trusted
project  explicit project resource
user     local installed
remote   untrusted until reviewed/installed
```

禁止 silent shadowing；Skill resolution 必须返回 exact `SkillId + version + source hash`，并写入 WorkflowRun。

## 13. Memory

Memory 不等同于 transcript 或 vector index。

### 13.1 Memory scopes

```text
Turn Memory       scratchpad，不持久化
Session Memory    当前 Harness session
Project Memory    研究问题、变量含义、方法决策
User Memory       用户偏好和报告偏好
Episodic Memory   已完成 Workflow 的摘要和经验
```

### 13.2 Memory Record

```rust
pub struct MemoryRecord {
    pub id: MemoryRecordId,
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub value: StructuredMemoryValue,
    pub source_refs: Vec<MemorySourceRef>,
    pub confidence: MemoryConfidence,
    pub status: MemoryStatus,
    pub project_binding: Option<ProjectBinding>,
    pub sensitivity: SensitivityClass,
    pub created_by: MemoryAuthor,
    pub supersedes: Option<MemoryRecordId>,
    pub retention: RetentionPolicy,
}
```

```rust
pub enum MemoryKind {
    ResearchQuestion,
    DatasetSemantic,
    VariableRole,
    StudyDesign,
    MethodDecision,
    ModelDecision,
    UserPreference,
    ReportingPreference,
    WorkflowSummary,
}
```

```rust
pub enum MemoryStatus {
    Proposed,
    Approved,
    Active,
    Superseded,
    Invalidated,
    Deleted,
}
```

### 13.3 Memory writes

模型没有直接 `write_memory` 权限：

```text
Agent proposes memory
    ↓
MemoryPolicy validates source/sensitivity/currentness
    ↓
Approval when required
    ↓
MemoryStore commit
```

必须保存来源与 revision。Dataset/resource revision 或 source hash 变化时，相关 memory 自动 invalidated。

禁止保存：

- 原始数据行或敏感单元格；
- token/API key/连接字符串；
- SQL、完整文档 payload 或 clipboard；
- 无来源的模型猜测；
- 完整 prompt/debug trace。

用户必须能查看、编辑、导出和删除持久 Memory。

## 14. Knowledge Base

专业 Harness 不能只依赖向量 RAG。Knowledge 由三部分组成：

```text
Structured Statistical Method Registry
+ Citable Document Corpus
+ Hybrid Retrieval Index
```

### 14.1 Statistical Method Card

```rust
pub struct StatisticalMethodCard {
    pub id: StatisticalMethodId,
    pub version: MethodVersion,
    pub estimands: Vec<EstimandKind>,
    pub supported_designs: Vec<StudyDesignKind>,
    pub variable_requirements: Vec<VariableRequirement>,
    pub assumptions: Vec<Assumption>,
    pub diagnostics: Vec<DiagnosticRequirement>,
    pub alternatives: Vec<StatisticalMethodId>,
    pub robustness_checks: Vec<RobustnessCheck>,
    pub reporting_requirements: Vec<ReportingRequirement>,
    pub knowledge_refs: Vec<KnowledgeCitation>,
}
```

Method Card 是 method selection 和 workflow gate 的结构化规则来源。

### 14.2 Knowledge Sources

允许：

- YssBI 自有统计方法说明；
- 算法、节点和 backend 文档；
- 官方软件/包文档；
- 有授权的方法学资料；
- project research protocol；
- variable codebook/data dictionary；
- 用户研究笔记；
- 组织内部方法规范。

禁止未经许可索引商业教材全文。

### 14.3 Index pipeline

```text
Source
  ↓ parse
Normalized Document
  ↓ taxonomy / metadata
Chunks
  ├─ lexical index
  ├─ embedding index
  └─ structured method index
       ↓
Hybrid Retrieval
       ↓
Rerank
       ↓
Cited Knowledge Result
```

metadata：

```text
method family
study design
estimand
variable types
assumptions
diagnostics
remedies
software/backend version
source/license/version/date/hash
```

Knowledge index 是可重建 projection。优先级固定为：

```text
Tool Evidence
> Project facts
> Structured Method Rules
> Cited Knowledge Prose
> Model prior
```

## 15. Statistical Quality Gates

专业 Workflow 必须覆盖：

```text
research question
data source/sample
variable roles/measurement scale
missingness/outliers
study design
estimand
method applicability
assumptions
diagnostics
robustness/sensitivity
multiple testing
effect size/uncertainty
limitations
reproducibility
```

标准阶段：

```text
1. Question Framing
2. Dataset Semantics
3. Data Quality
4. Study Design
5. Estimand Selection
6. Method Selection
7. Assumption Preflight
8. Estimation
9. Diagnostics
10. Robustness
11. Interpretation
12. Reproducible Report
```

模型不能因为用户只询问显著性就跳过 diagnostics/multiple-testing gates。

## 16. Approval 与风险等级

```rust
pub enum CapabilityRisk {
    Inspect,
    Compute,
    ProjectMutation,
    Destructive,
    ExternalNetwork,
}
```

默认策略：

| 风险                | 默认行为                            |
| ------------------- | ----------------------------------- |
| Inspect             | currentness 验证后自动              |
| Compute             | 用户设置允许时自动，否则确认        |
| ProjectMutation     | 一次性 invocation-bound approval    |
| Destructive         | 每次显式确认                        |
| ExternalNetwork/MCP | 每 server/tool/schema hash 显式授权 |

Approval 必须绑定：

```text
principal
harness session
project session
workflow/step/invocation
tool id/version/schema hash
request fingerprint
expiry/binding nonce
```

Skill、prompt 或 MCP Server 不能扩大 approval scope。

## 17. Persistence

### 17.1 App-data SQLite

建议表：

```text
assistant_session
assistant_turn
assistant_message
assistant_event
workflow_definition
workflow_run
workflow_step
tool_invocation
tool_receipt
approval_grant
memory_record
knowledge_source
knowledge_chunk
knowledge_index_meta
skill_installation
```

约束：

```text
(session_id, sequence) unique
(workflow_run_id, step_id) unique
(session_id, invocation_id) unique
idempotency_key unique in binding scope
```

SQLite 使用短事务；模型调用、embedding、工具执行和文件 I/O 不得持锁。

### 17.2 Project portability

Harness operational journal 不进入 Project DuckDB。需要随项目移动的内容必须通过 Project authority 显式保存为：

```text
Project Memory
Workflow Manifest
Workflow Receipt
Analysis Report
Knowledge Source Manifest
```

具体磁盘布局由 `yss-project-layout` 统一定义。

## 18. Harness Session 与项目生命周期

Assistant panel 是 layout-persisted UI，但 Harness session 必须 project-bound。

Project replacement：

1. 关闭旧 session admission；
2. 取消或暂停未提交 turns/workflows；
3. drain active invocations；
4. 保留已提交 receipts；
5. 失效旧 project memory/context；
6. 创建或恢复新 project-bound session；
7. late events 不得进入新 session。

身份必须分离：

```text
ProjectRegistrationId
ProjectInstanceId
ProjectSessionId
HarnessSessionId
HarnessTurnId
WorkflowRunId
ToolInvocationId
ResultId
MemoryRecordId
```

## 19. `apply_graph_edit`

写能力必须最后实现。它是单 graph 的 revision-aware batch：

```text
one graph
one clientKey
one batch request
one staged validation
one Project commit
one revision advance
one history entry
one publication
```

`OperationId` 只作业务 correlation/admission，不充当 Harness invocation 或 idempotency key。

Harness 必须保存：

- request fingerprint；
- base revision；
- approval receipt；
- commit receipt；
- resulting revision/history identity；
- undo capability/receipt。

## 20. Rig Adapter

Rig 只实现 Agent Driver，不拥有 Tool Registry 或 Workflow Runtime。

Adapter mapping：

```text
Harness AgentTurnRequest
    → Rig messages/config
Automation ToolDescriptor
    → Rig tool schema
Rig tool call
    → typed CapabilityRequest
CapabilityResult
    → bounded Rig tool result
Rig stream
    → Harness AgentEvent
```

Provider token 和模型配置通过 platform configuration/credential ports 注入。

## 21. MCP

### 21.1 原则

MCP 是外部协议 adapter，不是内部调用链。

```text
Internal Assistant → Capability Gateway

External Agent → MCP Server → Capability Gateway
```

不得使用：

```text
Internal Assistant → loopback MCP → Application
MCP → Harness → ProjectState
```

### 21.2 MCP Server mapping

```text
MCP Tools      → Capability Gateway
MCP Resources  → schema/result descriptor/knowledge document
MCP Prompts    → optional Skill entry prompt projection
MCP Tasks      → WorkflowRun external handle
```

MCP protocol 自身不拥有 YssBI session/workflow state。当前 MCP 规范采用 stateless protocol core，持久状态必须位于 handler 外；Tasks extension 只投影 long-running request。

### 21.3 MCP Client

外部 tools 进入独立 registry：

```rust
pub struct ExternalMcpToolDescriptor {
    pub server_id: McpServerId,
    pub remote_name: String,
    pub schema_hash: SchemaHash,
    pub trust: ExternalToolTrust,
    pub effect: ToolEffect,
    pub approval: ApprovalPolicy,
}
```

默认：

```text
untrusted
external effect
explicit approval
bounded result
no direct Memory write
no direct Project mutation
```

MCP prompt 不自动成为 trusted Skill；MCP resource 不自动进入 Knowledge index。

## 22. Tauri API 与事件流

`yss-api` 保持薄 transport：

```text
create_harness_session
submit_harness_turn
approve_harness_invocation
cancel_harness_turn
pause_workflow_run
resume_workflow_run
close_harness_session
subscribe_harness_events
```

高频 ordered output 使用 Tauri Channel：

```rust
pub enum HarnessEventDto {
    SessionCreated,
    TurnStarted,
    TextDelta,
    PlanProposed,
    WorkflowStarted,
    StepStarted,
    ToolInvocationRequested,
    ApprovalRequired,
    ToolInvocationCompleted,
    MemoryProposed,
    KnowledgeCited,
    StepCompleted,
    WorkflowCompleted,
    TurnCompleted,
    TurnFailed,
    TurnCancelled,
}
```

每个事件包含：

```text
streamId
sequence
sessionId
turnId
workflowRunId?
stepId?
```

Channel 必须定义 capacity、backpressure/drop policy、gap recovery 和 terminal event。

## 23. Frontend Assistant

```text
Harness Channel
    ↓
Assistant Event Parser
    ↓
Assistant Projection Store/ExternalStore Adapter
    ↓
assistant-ui ExternalStoreRuntime
```

Frontend 不建立 Rust-authoritative message/workflow mirror；projection 可从 snapshot/event replay 重建。

UI 应支持：

- conversation；
- Statistical Plan；
- Workflow step 状态；
- Tool invocation cards；
- approval；
- Evidence；
- citations；
- Memory proposals；
- pause/resume/cancel；
- Result/Chart/Report navigation。

## 24. Error、Diagnostics 与数据安全

- Harness、Tool、Workflow、Memory、Knowledge 和 adapter 分别使用 typed `thiserror`。
- Tauri seam 映射成 `{ code, details, incidentId }`。
- React 本地化错误文案。
- Harness operational ledger 与 diagnostics logs 分离。
- prompt、transcript、Tool payload、数据行、SQL、Memory 内容不写入 logs。
- Assistant 文本进入 Harness event channel，不进入 Run Output 或 Diagnostics。
- Provider、MCP 和 embedding 网络传输必须由用户配置和 data-sharing policy 控制。
- 外部 payload 必须进行 size/depth/schema/time validation。

## 25. Knowledge 与 Memory 隐私

- 默认不索引数据行；优先索引 schema、codebook、统计摘要和明确文档。
- Project Memory 与 User Memory 分库/分 scope 查询。
- 所有 retrieval 先应用 principal、project 和 sensitivity filters。
- 用户可查看、删除、导出和关闭持久 Memory。
- Knowledge source 记录 license、source hash、版本和删除状态。
- 删除 source 后异步清理 derived chunks/embeddings；查询立即拒绝 deleted source。

## 26. Delivery Phases

### Phase 1：Capability Foundation

1. `yss-automation-contract`；
2. Tool descriptors/schema；
3. Application Capability Gateway；
4. read-only inspect tools；
5. policy/approval types；
6. production-unrouted architecture gates。

### Phase 2：Durable Harness Core

1. `yss-statistical-harness`；
2. session/turn authority；
3. Workflow state machine；
4. Tool ledger；
5. cancellation/deadline；
6. MockAgentDriver；
7. in-memory persistence tests。

### Phase 3：SQLite Persistence

1. `yss-statistical-harness-sqlite`；
2. crash recovery；
3. Workflow resume；
4. event replay；
5. retention/cleanup；
6. project replacement tests。

### Phase 4：Statistical Skills 与 Knowledge

1. Method Card；
2. builtin Skill packages；
3. Skill registry/resolution；
4. lexical knowledge search；
5. citations；
6. embeddings/hybrid retrieval 后置。

### Phase 5：Memory

1. Session Memory；
2. Project Memory proposal；
3. approval/invalidation；
4. User Memory；
5. Memory UI。

### Phase 6：Rig 与 Assistant UI

1. `yss-agent-rig`；
2. AgentDriverPort；
3. ordered Tauri Channel；
4. assistant-ui ExternalStore；
5. plan/tool/approval/citation UI。

### Phase 7：Write Capabilities

1. `apply_graph_edit`；
2. `create_chart`/`update_chart`；
3. revision-aware batch mutation；
4. one-time approval；
5. durable receipts/undo。

### Phase 8：MCP

1. MCP Server；
2. resources/prompts；
3. Workflow→Tasks mapping；
4. MCP Client；
5. external tool trust/sandbox/policy。

## 27. Testing Strategy

### Contract tests

- exact JSON schema golden；
- Tool ID/version uniqueness；
- schema hash determinism；
- DTO strict parsing；
- adapter exhaustive mapping。

### Harness behavior tests

- session/turn state transitions；
- concurrent turns admission；
- approval binding；
- deadline/cancellation；
- ledger idempotency；
- late result suppression；
- project replacement；
- commit reconciliation。

### Workflow tests

- compile validation；
- DAG ordering；
- retry eligibility；
- crash recovery；
- read/write resume semantics；
- immutable definition/version binding。

### Statistical tests

- representative external-reference golden fixtures；
- method applicability；
- assumption/diagnostic gates；
- multiple-testing policy；
- evidence provenance；
- report completeness。

### Skill/Knowledge/Memory tests

- manifest validation；
- permission intersection；
- exact version resolution；
- source/citation integrity；
- memory proposal/approval/supersede/invalidate；
- scope/sensitivity filtering；
- deleted source exclusion。

### Adapter tests

- Rig provider/model/tool mapping；
- rmcp conformance at adapter boundary；
- SQLite transaction/recovery；
- Tauri Channel sequence/gap/terminal semantics。

## 28. Architecture Fitness Gates

Production audit 必须保证：

```text
Harness -> Rig/rmcp/Tauri/SQLite dependency = 0
Rig/rmcp adapter -> Project/Graph/Database concrete owner dependency = 0
Capability Gateway bypass = 0
raw Tauri command as model tool = 0
frontend conversation/workflow authority = 0
untyped tool result contract = 0
direct model Memory write = 0
Skill permission expansion = 0
Knowledge index authority = 0
prompt/user data diagnostics logging = 0
```

## 29. MVP Acceptance

首个可启用 Assistant 的最小范围：

- 一个 project-bound Harness session；
- 一个 Mock/Rig AgentDriver；
- ordered Assistant event channel；
- `inspect_graph`、`inspect_dataset_schema`、`inspect_dataset_profile`、`inspect_result`；
- read-only statistical plan；
- 一个可持久恢复的 `dataset_quality_review` Workflow；
- builtin Skill exact-version resolution；
- lexical Knowledge retrieval 与 citations；
- Session Memory；
- 无 Project mutation、MCP Client、external network Tool 或 persistent User Memory。

发送按钮只有在上述 contracts、state transitions、recovery 和 event projection 全部通过后才启用。

## 30. 后续决策点

以下选择通过 ports 延后，不阻塞 Foundation：

- provider/model selection；
- embedding model；
- lexical/vector index implementation；
- Project Memory 具体资源布局；
- transcript retention/encryption policy；
- remote Skill signing/distribution；
- MCP HTTP/stdio transport；
- background Workflow scheduling policy。

这些决策不得改变 Harness Core 的 typed contracts 和 authority 方向。

## 31. 外部协议与框架参考

- [Rig 官方文档](https://docs.rig.rs/)
- [MCP 2026-07-28 官方说明](https://blog.modelcontextprotocol.io/posts/2026-07-28/)
- [MCP 官方 Rust SDK / rmcp](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/README.md)
- [MCP Tasks extension](https://tasks.extensions.modelcontextprotocol.io/specification/draft/tasks)

当前 MCP core 是 stateless；任何 Harness session、Memory、Workflow 或 durable task state 都必须由 YssBI 自己的 authority 持有，不能存放在 rmcp request handler 中。
