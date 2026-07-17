# Bayesian Inference Architecture Plan

本文描述 YssBI 中“可视化贝叶斯参数估计器”的模块化架构。目标不是让前端拼接并执行 Julia 代码，而是建立一套稳定的结构化建模协议：

```text
Frontend UI
  → Rust application / sci API
  → Julia scientific backend
  → standardized inference result
```

核心原则：

- 前端和项目文件保存 YssBI 自己的 `ModelSpec`，不保存 Julia/Turing 源码。
- Rust 是项目、数据、校验、任务状态和权限的权威来源。
- Julia 只是内部科学计算后端，负责 MCMC/诊断/样本汇总。
- 前端只感知“贝叶斯推断任务”，不感知 Julia worker 协议。
- 数据表走 Arrow/Parquet，模型配置和结果摘要走 JSON。
- 第一版先复用当前 `src/sci + Julia worker` 架构，稳定后再考虑独立 sidecar/plugin。

---

## 总体分层

```text
┌──────────────────────────────────────────────┐
│ Frontend Bayesian UI Framework               │
│ 模型编辑、参数表、采样设置、诊断图表          │
└──────────────────────┬───────────────────────┘
                       │ service DTO
                       ▼
┌──────────────────────────────────────────────┐
│ Rust Bayesian Application Framework          │
│ 命令/API、任务管理、数据选择、结果读取         │
└──────────────────────┬───────────────────────┘
                       │ sci/api/bayes
                       ▼
┌──────────────────────────────────────────────┐
│ Bayesian Model Specification Framework       │
│ ModelSpec / Expression AST / Prior / Config  │
└──────────────────────┬───────────────────────┘
                       │ validated task
                       ▼
┌──────────────────────────────────────────────┐
│ Bayesian Validation Framework                │
│ 符号解析、类型检查、维度检查、安全约束          │
└──────────────────────┬───────────────────────┘
                       │ backend request
                       ▼
┌──────────────────────────────────────────────┐
│ Scientific Backend Framework                 │
│ Rust backend / Julia backend / future engines│
└──────────────────────┬───────────────────────┘
                       │ JSON + Arrow
                       ▼
┌──────────────────────────────────────────────┐
│ Julia Bayesian Engine Framework              │
│ AST 解释、Turing 模型、MCMC、诊断、输出         │
└──────────────────────┬───────────────────────┘
                       │ result refs
                       ▼
┌──────────────────────────────────────────────┐
│ Inference Result Framework                   │
│ 摘要、诊断、样本文件、后验预测、日志             │
└──────────────────────────────────────────────┘
```

---

## 1. Frontend Bayesian UI Framework

详细设计见：[`bayesian-frontend-ui.md`](./bayesian-frontend-ui.md)。

### 目标

提供用户可理解的贝叶斯建模界面：

- 在 Formula 中编辑完整观测模型；
- 将 predictor 安全解析为 `RawExpressionDTO`；
- 识别方程符号并确认 dependent / independent / parameter 角色；
- 为 dependent / independent symbols 绑定数据库列；
- 为 parameter symbols 设置约束、先验分布和先验参数；
- 设置采样参数；
- 提交推断任务；
- 查看参数摘要、诊断图、后验预测和日志。

### 边界

前端不负责：

- 生成 Julia 代码；
- 直接调用 Julia worker；
- 直接读取 Julia worker 临时目录；
- 执行模型安全校验的最终判断；
- 判断 MCMC 是否可信的核心规则。

前端负责：

- 交互式构建结构化 DTO；
- 基础表单校验；
- 展示后端返回的 validation report；
- 展示任务状态和结果。

### 约定

前端只通过 service 调用后端，例如：

```text
src/services/bayes/
  modelService.ts
  inferenceService.ts
```

视图不得直接调用 `invoke`。

前端提交的数据应是结构化模型定义，而不是代码字符串。采用“model equation parts → raw predictor expression → symbol roles → data bindings / priors → bound expression”的模型优先流程：

```ts
interface FormulaDraftDTO {
  formulaText: string;
  responseSymbol?: string;
  rawPredictor: RawExpressionDTO;
}

interface BayesModelDraftDTO {
  formulaText: string;
  responseSymbol?: string;
  rawPredictor: RawExpressionDTO | null;
  symbols: SymbolDraftDTO[];
  responseBinding: ResponseBindingDTO | null;
  dataBindings: Record<string, string>;
  boundPredictor: ExpressionDTO | null;
  likelihood: LikelihoodSpecDTO;
  parameters: ParameterSpecDTO[];
}
```

公式编辑器可以接受 LaTeX 风格输入，但 predictor 部分必须先转换成受限表达式 AST，再提交给 Rust；`formulaText` 只作为展示文本和审计信息，不作为可执行代码。

---

## 2. Bayesian Model Specification Framework

### 目标

定义跨前端、Rust、Julia 后端都稳定的贝叶斯模型协议。它是系统的核心，不绑定 Turing.jl。

建议 Rust 目录：

```text
src-tauri/src/sci/api/bayes/
  mod.rs
  model_spec.rs
  expression.rs
  prior.rs
  likelihood.rs
  inference_config.rs
  result.rs
```

### 核心对象

```text
DatasetRef
ModelSpec
Expression
LikelihoodSpec
ParameterSpec
PriorSpec
InferenceConfig
InferenceResult
```

### `ModelSpec`

```rust
pub struct ModelSpec {
    pub response: String,
    pub predictor: Expression,
    pub likelihood: LikelihoodSpec,
    pub parameters: Vec<ParameterSpec>,
    pub sampler: InferenceConfig,
}
```

含义：

- `response` 是观测变量，例如 `y`；
- `predictor` 是预测方程 AST，例如 `a * x + b`；
- `likelihood` 是观测分布，例如 `Normal(mu, sigma)`；
- `parameters` 是未知参数定义，例如 `a`, `b`, `sigma`。

### `Expression`

表达式是受限 AST，不是 Julia/Rust 代码。

```rust
pub enum Expression {
    Number(f64),
    Column(String),
    Parameter(String),
    Unary {
        op: UnaryOp,
        arg: Box<Expression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Call {
        function: MathFunction,
        args: Vec<Expression>,
    },
}
```

第一版允许：

```text
数字
列名
参数名
+ - * / ^
括号
exp log sqrt abs sin cos min max
```

禁止：

```text
任意 Julia 语法
文件访问
网络访问
模块加载
宏
函数定义
循环
系统命令
环境变量访问
```

### `ParameterSpec`

```rust
pub struct ParameterSpec {
    pub name: String,
    pub constraint: ParameterConstraint,
    pub prior: PriorSpec,
}
```

第一版约束：

```text
real        无约束
positive    大于 0
unit        0 到 1
```

后续再扩展：

```text
bounded(lower, upper)
integer
simplex
ordered
vector
matrix
```

### `PriorSpec`

第一版支持：

```text
Normal
LogNormal
Uniform
Beta
Gamma
Exponential
StudentT
Cauchy
HalfNormal
```

约定：

- 先验参数必须在 Rust 端验证数量和取值域；
- 参数约束不能只从先验推断，必须显式保存；
- Julia 后端负责把 `PriorSpec` 映射到 Distributions.jl。

### `LikelihoodSpec`

数学方程和观测分布必须分离。

示例：

```text
predictor: a * x + b
likelihood: y ~ Normal(mu = predictor, sigma = sigma)
```

第一版支持：

```text
Normal
BernoulliLogit
PoissonLog
```

建议 MVP 先只实现：

```text
Normal
```

---

## 3. Bayesian Expression Framework

### 目标

提供安全、可验证、可序列化的表达式系统，用于用户自定义非线性方程。

### Rust 职责

Rust 负责：

- 文本表达式解析；
- AST 构建；
- 函数白名单校验；
- 列名和参数名解析；
- 禁止未知符号；
- 表达式复杂度限制；
- 生成 Julia 可解释的 JSON AST。

### Julia 职责

Julia 负责：

- 读取 JSON AST；
- 在每个观测点上解释执行 AST；
- 使用当前参数值和数据列计算预测值。

第一版使用解释器：

```julia
evaluate_expression(ast, env)
```

不要第一版就生成 Julia 源码并 `eval`。

### 约定

表达式必须是纯函数：

```text
same columns + same parameters → same value
```

表达式不能访问：

```text
文件系统
网络
随机数
全局状态
Julia module
```

表达式错误必须可结构化返回，例如：

```json
{
  "code": "MODEL_DOMAIN_ERROR",
  "message": "log received a non-positive value",
  "path": "predictor.right.arg"
}
```

---

## 4. Bayesian Validation Framework

### 目标

在任务提交到 Julia 之前，Rust 先完成尽可能多的验证，减少运行时失败，并给用户明确反馈。

建议目录：

```text
src-tauri/src/sci/api/bayes/validation.rs
```

### 验证内容

#### 数据验证

- 响应变量存在；
- predictor 中引用的列存在；
- 所有参与 MCMC 的列是数值型；
- 缺失值策略明确；
- 行数大于最低要求；
- 数据量过大时给出提示或抽样建议。

#### 参数验证

- 参数名合法；
- 参数名不与列名冲突；
- 参数名不重复；
- predictor 中的未知符号必须属于列名或参数名；
- likelihood 中使用的参数必须存在；
- 先验分布参数数量正确；
- 先验参数满足取值域。

#### 模型验证

- likelihood 和 response 类型匹配；
- `Normal` 的 `sigma` 必须 positive；
- `BernoulliLogit` 的 response 应为 0/1；
- `PoissonLog` 的 response 应为非负整数；
- 表达式深度和节点数不超过限制。

#### 采样配置验证

- chains >= 1；
- samples > 0；
- warmup >= 0；
- target_accept 在合理范围内；
- seed 可选但必须为有效整数。

### 输出

验证结果应结构化：

```rust
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}
```

不要只返回字符串。

---

## 5. Rust Bayesian Application Framework

### 目标

把贝叶斯推断纳入应用层工作流，保持 Tauri command 轻薄，避免前端直接接触后端实现细节。

建议目录：

```text
src-tauri/src/features/application/bayes/   // 如果后续建立应用层 feature
src-tauri/src/commands/command_bayes.rs
src-tauri/src/sci/api/bayes/
```

### Tauri command 约定

Tauri command 只做：

- 解析 DTO；
- 调用 application/sci API；
- 映射错误；
- 返回任务 ID 或结果引用。

不要在 command 中写：

- 长流程；
- 文件 I/O 细节；
- Julia worker 调用；
- 模型业务校验；
- MCMC 结果解析。

### 推荐命令

MVP 可以先同步：

```text
fit_bayes_model
```

但正式功能应使用任务式接口：

```text
submit_bayes_inference
get_bayes_inference_status
cancel_bayes_inference
read_bayes_inference_result
```

返回给前端的是 YssBI 任务状态，不是 Julia worker 状态。

---

## 6. Scientific Backend Framework

### 目标

为贝叶斯推断提供可替换后端。第一版实现 Julia/Turing，未来可扩展 Stan、Rust MCMC 或远程推断。

建议目录：

```text
src-tauri/src/sci/backends/julia/bayes/
  mod.rs
  fit.rs
  io.rs

src-tauri/src/sci/backends/rust/bayes/
  mod.rs
  validate.rs
```

### Engine 约定

沿用现有 `SciContext` / `SciEngine`：

```text
Rust
Julia
JuliaWithRustFallback
```

但贝叶斯 MCMC 第一版通常只有 Julia 后端。此时：

- `SciEngine::Julia` 执行 Turing；
- `SciEngine::Rust` 可只做 validation 或返回 unsupported；
- `JuliaWithRustFallback` 不应静默换成不等价的 Rust 算法，除非 Rust backend 明确实现同一模型。

### 后端输入输出

后端输入：

```text
ModelSpec JSON
InferenceConfig JSON
Data Arrow
```

后端输出：

```text
summary JSON
metadata JSON
samples Arrow/Parquet
logs text
```

### 约定

`src-tauri/src/julia/worker.rs` 保持通用 worker 生命周期和协议，不添加 `run_bayes_*` 专用函数。

专用逻辑放在：

```text
src-tauri/src/sci/backends/julia/bayes/fit.rs
```

---

## 7. Julia Bayesian Engine Framework

### 目标

在 Julia worker 中实现贝叶斯推断 op。第一阶段复用当前 worker，而不是新建独立 sidecar。

建议目录：

```text
src-tauri/julia/ops/bayes_fit.jl
src-tauri/julia/ops/bayes/
  expression.jl
  priors.jl
  likelihoods.jl
  model_builder.jl
  diagnostics.jl
  serialization.jl
```

如果文件过多，再拆为 Julia module。

### 依赖

预计依赖：

```text
Turing
Distributions
StatsBase
MCMCChains
JSON3
Arrow
```

约定：

- `Project.toml` 和 `Manifest.toml` 必须锁版本；
- Turing 相关测试必须环境变量门控；
- 不让普通 `pnpm rust:check` 依赖 Julia/Turing 可用；
- 不让前端直接调用 bayes worker op。

### Julia op 输入

```json
{
  "operation": "bayes_fit",
  "parameters": {
    "modelSpecPath": "model.json",
    "configPath": "config.json",
    "inputPath": "input.arrow",
    "summaryPath": "summary.json",
    "samplesPath": "samples.arrow",
    "metadataPath": "metadata.json"
  }
}
```

也可以沿用当前 worker 的：

```text
inputPath
outputPath
metadataPath
parameters
```

但对于 MCMC，建议显式拆出：

```text
summaryPath
samplesPath
logPath
```

### Julia op 输出

```json
{
  "taskId": "...",
  "operation": "bayes_fit",
  "summaryPath": "...",
  "samplesPath": "...",
  "metadataPath": "...",
  "diagnostics": {
    "chains": 4,
    "drawsPerChain": 2000,
    "divergences": 0
  }
}
```

### 安全约定

Julia 不执行用户源码。

允许：

```text
解释 Rust 验证过的 AST
映射白名单 prior/likelihood
运行 Turing 模型
```

禁止：

```text
eval(user_input)
Meta.parse(user_input)
include(user_path)
run(command)
读取任意用户文件
访问网络
```

---

## 8. Bayesian Task Framework

### 目标

MCMC 是长任务，不能长期阻塞 UI。需要通用任务系统承载状态、取消和结果读取。

建议目录：

```text
src-tauri/src/tasks/
  mod.rs
  task_id.rs
  task_status.rs
  task_registry.rs

src-tauri/src/sci/api/bayes/tasks.rs
```

### 任务状态

```rust
pub enum TaskStatus {
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Completed,
    Failed,
}
```

任务记录：

```rust
pub struct InferenceTask {
    pub id: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result_ref: Option<InferenceResultRef>,
    pub error: Option<TaskError>,
}
```

### 约定

前端订阅的是通用任务事件：

```text
sci-task-updated
```

不是：

```text
julia-worker-progress
```

Julia worker 的取消能力可以被 Rust task manager 使用，但不直接暴露给前端。

### MVP 取舍

MVP 可以先同步执行固定小模型，但正式贝叶斯功能必须任务化。

推荐路线：

```text
Phase 1: 同步 PoC，验证 Turing 链路
Phase 2: 后端任务化，支持状态和取消
Phase 3: 长 MCMC 独立子进程隔离
```

---

## 9. Inference Result Framework

### 目标

定义稳定的推断结果格式，不泄漏 Turing/MCMCChains 内部对象。

建议目录：

```text
src-tauri/src/sci/api/bayes/result.rs
```

### 结果对象

```rust
pub struct InferenceResult {
    pub summaries: Vec<ParameterSummary>,
    pub diagnostics: InferenceDiagnostics,
    pub samples: SampleResultRef,
    pub posterior_predictive: Option<PosteriorPredictiveRef>,
    pub log_path: Option<PathBuf>,
}
```

参数摘要：

```rust
pub struct ParameterSummary {
    pub parameter: String,
    pub mean: f64,
    pub sd: f64,
    pub median: f64,
    pub q025: f64,
    pub q975: f64,
    pub rhat: Option<f64>,
    pub ess_bulk: Option<f64>,
    pub ess_tail: Option<f64>,
}
```

诊断：

```rust
pub struct InferenceDiagnostics {
    pub chains: usize,
    pub draws_per_chain: usize,
    pub warmup: usize,
    pub divergences: Option<usize>,
    pub max_treedepth_hits: Option<usize>,
    pub warnings: Vec<DiagnosticWarning>,
}
```

### 文件约定

```text
summary.json       小体积摘要
samples.arrow      完整后验样本
metadata.json      任务元数据
run.log            运行日志
ppc.arrow          后验预测，可选
```

完整样本不要通过 JSON 返回给前端。

前端图表按需读取下采样或聚合后的数据。

---

## 10. Bayesian Diagnostics Framework

### 目标

MCMC 不能只显示“运行成功”。必须显示收敛和采样质量。

### 最低诊断

第一版至少包含：

```text
R-hat
ESS bulk
ESS tail
trace plot data
density plot data
autocorrelation
posterior predictive check
```

如果 Turing/NUTS 可提供，还应包含：

```text
divergences
max treedepth hits
acceptance rate
```

### UI 状态约定

后端应给出机器可读 warning：

```text
RHAT_TOO_HIGH
ESS_TOO_LOW
DIVERGENCES_FOUND
TREEDEPTH_HITS_FOUND
PARAMETER_CORRELATION_HIGH
```

前端展示为：

```text
✓ R-hat 全部小于 1.01
⚠ 参数 a 的 ESS 偏低
⚠ 发现 divergence，结果可能不可靠
```

不要只显示：

```text
采样成功
```

---

## 11. Data Exchange Framework

### 目标

保持 Rust 数据层权威，Julia 只读取推断所需的数据快照。

### 输入数据

Rust 从当前项目数据层提取列：

```text
response
predictor columns
group columns（后续层级模型）
```

写入：

```text
input.arrow
```

### 配置数据

写入 JSON：

```text
model_spec.json
inference_config.json
```

### 输出数据

Julia 输出：

```text
summary.json
samples.arrow
metadata.json
run.log
```

Rust 读取 summary/metadata，保存 result ref；前端通过后端命令读取图表所需数据。

### 约定

- 不通过 JSON 传大表；
- 不让 Julia 直接修改项目 DuckDB；
- 不让 Julia 成为项目状态权威；
- 所有结果路径由 Rust 分配和管理；
- 临时文件和结果文件生命周期由 Rust 控制。

---

## 12. Packaging / Runtime Framework

### 目标

短期复用当前 Julia runtime/worker；长期支持可选 Bayesian Engine 插件化。

### MVP

使用当前架构：

```text
src-tauri/julia/worker.jl
src-tauri/julia/ops/bayes_fit.jl
```

用户只通过已有 Julia runtime 检测/安装入口准备 Julia。

### 后续

当 Turing 功能稳定后，再考虑：

```text
PackageCompiler create_app
Bayesian Engine sidecar
可选插件安装
```

原因：

- Turing + AD + Julia runtime 体积大；
- 首次预编译慢；
- 独立 sidecar 签名、升级、平台适配成本高；
- 过早 sidecar 化会拖慢 MVP。

### 约定

前端仍然只暴露：

```text
Julia runtime status
Julia runtime install
```

不要暴露：

```text
prepare bayes worker
run julia bayes
cancel julia task
```

---

## 13. Testing Framework

### 目标

用 golden fixture 验证模型协议和后端结果，避免只做 Rust-vs-Julia 互相比较。

### 测试目录

```text
src-tauri/tests/sci/fixtures/bayes/
  linear_normal/
    simple.json
  nonlinear_normal/
    exponential_decay.json

src-tauri/tests/sci_api_bayes_linear_normal_golden_test.rs
```

### 测试策略

#### Rust validation tests

默认运行，验证：

- 表达式解析；
- 符号解析；
- 参数/先验/likelihood 校验；
- 错误报告稳定。

#### Julia backend golden tests

环境变量门控：

```text
YSSBI_RUN_JULIA_BAYES_TESTS=1
```

验证：

- Julia worker 能加载 Turing；
- 固定 seed 下摘要结果落在容差范围；
- 输出文件存在且 schema 正确；
- 错误模型能返回结构化错误。

### 约定

贝叶斯 MCMC 结果有随机性，golden tests 不应要求每个样本完全一致。

可以验证：

```text
posterior mean 在容差内
credible interval 覆盖预期范围
R-hat 小于阈值
summary schema 稳定
```

不要默认在普通 `pnpm rust:check` 或 `pnpm typecheck` 中跑长时间 MCMC。

---

## 14. MVP Phase Plan

### Phase 0：协议和边界

完成：

- `ModelSpec` / `Expression` / `PriorSpec` / `InferenceConfig` / `InferenceResult`；
- Rust validation；
- fixture-driven tests；
- 不接 Turing。

目标：确认建模协议稳定。

### Phase 1：固定 Bayesian linear regression

完成：

- 固定线性模型；
- Normal likelihood；
- Turing NUTS；
- summary JSON；
- samples Arrow；
- Julia golden test 门控。

目标：跑通 `Rust → Julia → Turing → Result` 链路。

### Phase 2：安全表达式 + 非线性 Normal regression

完成：

- 表达式 parser；
- AST evaluator in Julia；
- 连续标量参数；
- Normal likelihood；
- NUTS。

目标：支持科学参数估计常见模型，例如：

```text
a * exp(-b * x) + c
```

### Phase 3：多 likelihood

加入：

```text
BernoulliLogit
PoissonLog
LogNormal
StudentT
```

目标：支持二分类、计数和鲁棒回归。

### Phase 4：任务化和诊断 UI

完成：

- submit/status/cancel/result；
- trace/density/autocorrelation；
- posterior predictive check；
- diagnostic warning。

目标：变成可用产品功能。

### Phase 5：sidecar/plugin 化

完成：

- PackageCompiler create_app；
- 可选 Bayesian Engine 安装；
- 平台分发和升级策略。

目标：降低普通用户首次使用 Julia/Turing 的配置成本。

---

## 15. 不做事项

第一版明确不做：

- 前端生成 Julia/Turing 源码；
- Julia `eval` 用户输入；
- 任意自定义 Julia 函数；
- 层级模型；
- ODE 参数反演；
- 离散参数采样；
- 自定义 likelihood；
- 多 worker 并发 MCMC；
- 独立 PackageCompiler sidecar；
- Julia 直接连接数据库；
- Julia 直接修改项目 DuckDB。

---

## 16. 最终约定摘要

```text
Frontend
  负责交互和展示，不接触 Julia worker。

Rust application
  负责命令、任务、数据、验证、结果生命周期。

src/sci/api/bayes
  负责稳定模型协议和 engine orchestration。

src/sci/backends/julia/bayes
  负责把 ModelSpec/Data 转换为 Julia worker task。

src/julia/worker.rs
  只负责通用 worker 生命周期，不写 bayes 专用逻辑。

src-tauri/julia/ops/bayes_fit.jl
  负责 Turing 推断、诊断和文件输出。

Project files
  保存 YssBI ModelSpec，不保存 Julia 源码。
```

最重要的架构判断：

> YssBI 的贝叶斯功能应该围绕 `DatasetRef + ModelSpec + InferenceConfig + InferenceResult` 建立，而不是围绕 Turing.jl 源代码建立。
