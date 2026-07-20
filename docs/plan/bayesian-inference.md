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

F1 已落地的 Rust 目录：

```text
src-tauri/src/sci/api/bayes/
  mod.rs          # public API exports and focused tests
  draft.rs        # frontend BayesModelDraft IPC DTO
  model.rs        # validated BayesModelSpec and stable model DTOs
  convert.rs      # draft validation and draft -> spec conversion
  validation.rs   # ValidationReport / ValidationIssue
  result.rs       # task, result, summary, diagnostics DTOs
```

Tauri IPC 薄封装位于：

```text
src-tauri/src/commands/command_bayes.rs
```

命令层只做参数接收、调用 `sci::api::bayes`、映射 AppError，不持有贝叶斯建模规则。

### 核心对象

```text
DatasetRef
ResponseSpec
BayesModelDraft
BayesModelSpec
Expression
LikelihoodSpec
ParameterSpec
PriorSpec
InferenceConfig
ValidationReport
BayesInferenceTask
InferenceResult
```

### `ModelSpec`

```rust
pub struct BayesModelSpec {
    pub dataset: DatasetRef,
    pub response: ResponseSpec,
    pub predictor: Expression,
    pub data_variables: BTreeMap<String, String>,
    pub likelihood: LikelihoodSpec,
    pub parameters: Vec<ParameterSpec>,
    pub sampler: InferenceConfig,
    pub display_formula: String,
}
```

含义：

- `dataset` 是后端数据源引用，不把整张表塞入模型配置；
- `response` 是观测变量符号及其绑定列，例如 `y -> response`；
- `predictor` 是预测方程 AST，例如 `a * x + b`；
- `data_variables` 保存模型自变量符号到数据列的映射，例如 `x -> time`；
- `likelihood` 是观测分布，例如 `Normal(mu, sigma)`；
- `parameters` 是未知参数定义，例如 `a`, `b`, `sigma`；
- `sampler` 是采样配置；
- `display_formula` 仅用于展示/审计，不作为可执行代码。

### `Expression`

表达式是受限 AST，不是 Julia/Rust 代码。

```rust
pub enum Expression {
    Number { value: f64 },
    DataVariable { name: String },
    Column { name: String },
    Parameter { name: String },
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

### Draft → Spec 转换

前端提交的是 `BayesModelDraft`，不是直接提交 `BayesModelSpec`。Rust 通过：

```rust
validate_draft(&draft) -> ValidationReport
draft_to_model_spec(draft) -> Result<BayesModelSpec, ValidationReport>
```

转换时必须确认：

- dataset 存在且有 source id；
- 恰好一个 dependent symbol；
- response binding 存在并指向 dataset column；
- independent symbols 都绑定到 dataset column；
- predictor 已经是 `Expression`，并且其中数据符号和参数都已配置；
- sampler 的 chains/samples/target_accept 等基础取值合法。

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
real                         (-∞, ∞)
positive                     (0, ∞)
unit                         (0, 1)
bounded(lower, upper, ...)   用户指定上下界，可配置开闭区间
```

`bounded` 是一等约束对象，而不是 UI 附加字段：

```rust
Bounded {
    lower: f64,
    upper: f64,
    include_lower: bool,
    include_upper: bool,
}
```

后续再扩展：

```text
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

数学方程和观测分布在结构化协议中必须分离；UI 可以把它们组合为一条 LaTeX 观测模型展示。

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

当前 Julia backend 已真实执行：

```text
Normal
BernoulliLogit
PoissonLog
```

其中 `Normal` 支持固定线性 fast path 和安全 AST 通用非线性路径；`BernoulliLogit` / `PoissonLog` 走安全 AST 通用 Turing 路径。

### F1 当前边界

已经完成：

- Rust `BayesModelDraft` / `BayesModelSpec` / `Expression` / `PriorSpec` / `LikelihoodSpec` / `InferenceConfig` DTO；
- `validate_draft` 和 `draft_to_model_spec`；
- command 层的 `validate_bayes_model`、`submit_bayes_inference` 等结构化入口；
- 参数约束支持 `real`、`positive`、`unit`、`bounded`；
- prior args 取值域校验和 constraint/prior 基础兼容性 warning；
- focused Rust tests 覆盖有效 draft 转 spec、缺少 dataset、非法 bounds / prior args 的校验。

尚未完成：

- 真实 Julia/Turing MCMC 执行；
- Rust 侧完整文本表达式 parser，当前前端先负责 predictor → RawExpressionDTO/Expression；
- result source 可枚举 registry；
- 更完整的分布定义域与截断/变换策略验证。

---

## 3. Bayesian Expression Framework

### 目标

提供安全、可验证、可序列化的表达式系统，用于用户自定义非线性方程。

### Rust 职责

Rust 负责：

- 文本表达式解析；
- LaTeX 常见操作符归一化，例如 `\\cdot`、`\\times`、`\\sigma`；
- `response = predictor` 和 `response ~ Distribution(predictor, ...)` 的基础拆分；
- AST 构建；
- 函数白名单校验；
- symbol collection；
- 表达式节点数和深度限制；
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

### F2 已落地实现

Rust 实现位于：

```text
src-tauri/src/sci/api/bayes/expression.rs
```

提供：

```rust
parse_model_expression(input) -> ParsedExpression
parse_predictor_expression(input) -> RawExpression
collect_raw_symbols(expression, symbols)
```

当前支持：

```text
数字
符号
+ - * / ^
括号
exp log sqrt abs sin cos min max
常见 LaTeX token: \\cdot, \\times, \\sigma, \\sim, \\left, \\right
```

当前限制：

```text
MAX_EXPRESSION_NODES = 256
MAX_EXPRESSION_DEPTH = 32
```

`parse_bayes_expression` command 已接入该 parser，并返回：

```ts
{
  formulaText: string,
  responseSymbol?: string,
  rawPredictor: RawExpressionDTO,
  symbols: string[]
}
```

前端 Formula 保存时优先调用后端 parser；如果后端不可用，开发阶段 fallback 到前端 parser。

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

F3 已落地目录：

```text
src-tauri/src/sci/api/bayes/
  validation.rs   # ValidationReport / ValidationIssue 数据结构
  validators.rs   # 分层 draft validators
  convert.rs      # 只保留 draft -> spec 转换
```

`validate_draft` 已从 `convert.rs` 中拆出，避免转换模块变成 god module。

### 验证内容

#### 已实现的数据验证

- dataset 必须存在；
- dataset source id 必须存在；
- 必须且只能有一个 dependent symbol；
- response binding 必须存在；
- response column 必须存在于 dataset columns；
- independent symbols 必须绑定数据列；
- independent binding column 必须存在于 dataset columns。

#### 已实现的表达式验证

- predictor 必须存在；
- predictor 中的 data variables 必须已配置为 independent symbols；
- predictor 中的 parameters 必须存在于 `ParameterSpec`；
- expression number 必须 finite；
- function arity 校验：
  - `exp/log/sqrt/abs/sin/cos`: 1 个参数；
  - `min/max`: 至少 2 个参数。

#### 已实现的 likelihood 验证

- `Normal` response 必须是 number/integer；
- `Normal` predictor columns 必须是 number/integer；
- `Normal` sigma parameter 必须存在；
- `Normal` sigma parameter 非 positive-compatible constraint 时给 warning；
- `BernoulliLogit` response 必须是 boolean/integer/number；
- `PoissonLog` response 必须是 integer/number；
- submit 时会在 Rust 应用层扫描已物化的窄列数据，提前拦截 Bernoulli 非 0/1、Poisson 负数/小数、Normal/自变量缺失或非有限值，不把明显非法输入交给 Julia。

#### 已实现的参数验证

- parameter name 不能为空；
- parameter name 不能重复；
- `bounded` lower/upper 必须 finite 且 lower < upper；
- prior args 基础取值域校验；
- constraint 与 prior support 基础兼容性 warning。

#### 已实现的采样配置验证

- chains >= 1；
- samples > 0；
- target_accept 在 0 到 1 之间；
- max_tree_depth 如果设置，必须 > 0。

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

### F3 当前边界

已经完成：

- `validate_draft` 分层 validators；
- dataset/binding validation；
- expression semantic validation；
- likelihood/response dtype validation；
- parameter bounds/prior validation；
- sampler validation；
- `validate_bayes_input_table` 运行前数据扫描，覆盖 response 与 predictor columns 的缺列、类型和值域错误；
- focused Rust tests 覆盖 response dtype 和 function arity。

尚未完成：

- 缺失值策略；
- 行数和数据量检查；
- Poisson 非负整列扫描；
- 参数名与列名冲突策略；
- 更严格的 distribution support / truncation / transform validation。

---

## 5. Rust Bayesian Application Framework

### 目标

把贝叶斯推断纳入应用层工作流，保持 Tauri command 轻薄，避免前端直接接触后端实现细节。

F4 已落地目录：

```text
src-tauri/src/application/bayes.rs          # BayesInferenceService / task registry / result store
src-tauri/src/commands/command_bayes.rs     # thin Tauri commands
src-tauri/src/sci/api/bayes/                # model spec / validation / result DTOs
```

服务在 Tauri setup 中注册为 managed state：

```rust
.manage(application::bayes::BayesInferenceService::new())
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

正式功能使用任务式接口：

```text
submit_bayes_inference
get_bayes_inference_status
cancel_bayes_inference
read_bayes_inference_result
```

返回给前端的是 YssBI 任务状态，不是 Julia worker 状态。

### F4 已实现服务

```rust
BayesInferenceService::submit(draft) -> BayesInferenceTask
BayesInferenceService::status(task_id) -> BayesInferenceTask
BayesInferenceService::cancel(task_id)
BayesInferenceService::result(task_id) -> InferenceResult
```

当前实现使用内存：

```rust
HashMap<TaskId, BayesInferenceTask>
HashMap<TaskId, InferenceResult>
```

提交任务时：

```text
BayesModelDraft
  → validate_draft
  → draft_to_model_spec
  → BayesBackend::fit
  → stored task/result
```

这保证 command / frontend / task API 已经稳定。后续接 Julia/Turing 时只替换 `BayesBackend` 实现，不需要改 command API。

### F4 当前边界

已经完成：

- `BayesInferenceService`；
- task registry；
- result store；
- command 薄封装；
- Tauri managed state 注册；
- focused Rust tests 覆盖 submit、invalid draft、unknown task、backend 调用、backend 失败。

尚未完成：

- 异步队列和后台执行；
- 真实 Julia/Turing backend；
- 结果落盘 / result source 集成；
- 应用重启后的任务恢复；
- 进度事件推送。

---

## 6. Scientific Backend Framework

### 目标

为贝叶斯推断提供可替换后端。第一版实现 Julia/Turing，未来可扩展 Stan、Rust MCMC 或远程推断。

F5 已落地目录：

```text
src-tauri/src/sci/backends/bayes/mod.rs
```

该模块定义贝叶斯后端统一接口和当前 placeholder backend。后续 Julia/Turing backend 可以放在：

```text
src-tauri/src/sci/backends/julia/bayes/
  mod.rs
  fit.rs
  io.rs
```

并实现同一个 `BayesBackend` trait。

### Backend trait

F5 当前不复用 `SciEnginePolicy` 做贝叶斯选择，而是先建立更直接的后端接口：

```rust
pub trait BayesBackend: Send + Sync {
    fn fit(&self, spec: &BayesModelSpec) -> Result<InferenceResult, BayesBackendError>;
}
```

应用层只依赖 trait：

```text
BayesInferenceService
  → Arc<dyn BayesBackend>
  → InferenceResult
```

这样 command 与前端不会感知 Julia/Turing/worker 细节。

### 已实现 backend

```rust
PlaceholderBayesBackend
```

它返回 `InferenceResult::engine_not_implemented()`，用于稳定任务/结果链路。`BayesInferenceService::new()` 默认注入该 backend；测试可以通过 `BayesInferenceService::with_backend(...)` 注入 mock backend。

backend 失败时，service 会记录一个 `failed` task，并把 backend error 映射到 `TaskError`。

### 后端输入输出

后端输入：

```text
BayesModelSpec
Data Arrow/Parquet 或后端可解析 DatasetRef
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

### F5 当前边界

已经完成：

- `BayesBackend` trait；
- `BayesBackendError`；
- `PlaceholderBayesBackend`；
- `JuliaBayesBackend` skeleton，复用通用 `JuliaWorkerManager::run_task` 调用 `bayes_fit` op；
- `BayesInferenceService` 通过 `Arc<dyn BayesBackend>` 调用 backend；
- backend success 存储 result；
- backend failure 存储 failed task；
- focused Rust tests 覆盖 placeholder backend、backend 注入、backend failure、Julia skeleton result 解析。

尚未完成：

- result source / query source 数据物化；
- 更完整的运行前数据扫描报告聚合，目前遇到首个错误即返回；
- Turing MCMC 实现；
- backend progress/cancel 协议；
- backend result source / samples 文件输出。

---

## 7. Julia Bayesian Engine Framework

### 目标

在 Julia worker 中实现贝叶斯推断 op。第一阶段复用当前 worker，而不是新建独立 sidecar。

F6.1 已先落地最小 skeleton：

```text
BayesBackend trait
  → JuliaBayesBackend
  → JuliaWorkerManager::run_task(operation = "bayes_fit")
  → src-tauri/julia/ops/bayes_fit.jl
  → metadata.json(InferenceResult)
```

这个版本只证明 Rust → Julia → 标准结果 DTO 的链路可达，不运行 Turing，也不产生真实后验样本。

F6.2 已补充最小数据链路：

```text
submit_bayes_inference
  → BayesInferenceService::submit_from_project
  → validate draft / draft_to_model_spec
  → ProjectState.with_database_mut(...load_columns)
  → BayesBackendRequest { spec, input_table }
  → JuliaBayesBackend writes input.arrow
  → bayes_fit.jl reads Arrow.Table(inputPath)
```

Rust 只物化模型需要的窄列集合：response column + independent variable binding columns；不整表加载，不让前端或 Julia 直接访问项目数据库。

F6.3 已将应用默认 backend 切换为 Julia backend：

```text
tauri setup
  → create shared JuliaWorkerManager
  → manage JuliaWorkerManager
  → manage BayesInferenceService::with_backend(JuliaBayesBackend)
```

贝叶斯任务现在默认走 Julia worker；如果系统 Julia 不可用或 worker 失败，错误会被封装为 backend failed task 返回给前端。

F6.4 已补充 Julia 侧安全表达式解释层：

```text
src-tauri/julia/ops/bayes/expression.jl
  → prior default values
  → numeric Arrow column access
  → Expression AST evaluator
  → predictor preview smoke evaluation
```

它只解释 Rust 已验证过的结构化 AST，不解析或执行用户 Julia 源码。当前用于在进入 Turing 前验证 Julia 可以读取数据列、识别参数默认值并计算 predictor。

F6.5/F6.6/F6.10 已补充 Turing 执行路径：

```text
src-tauri/julia/Project.toml
  → Turing
  → Distributions
  → MCMCChains
  → StatsBase

src-tauri/julia/ops/bayes/turing_linear.jl
  → fixed Normal linear model y ~ Normal(a * x + b, sigma)

src-tauri/julia/ops/bayes/turing_generic_normal.jl
  → generic safe-AST regression model
  → Normal / BernoulliLogit / PoissonLog likelihoods
  → NUTS sampling
  → chain summary to InferenceResult.summaries
```

当前已支持：

- `Normal` likelihood：固定 `a * x + b` fast path + 通用安全 AST 非线性路径；
- `BernoulliLogit` likelihood：响应列运行时校验为 boolean 或 0/1；
- `PoissonLog` likelihood：响应列运行时校验为非负整数计数；
- prior 映射：`Normal`、`LogNormal`、`Uniform`、`Beta`、`Gamma`、`Exponential`、`StudentT`、`Cauchy`、`HalfNormal`；
- 输出 summary；
- 当 `sampler.saveSamples = true` 时，将 posterior draws 写入 Arrow 长表：`parameter`, `chain`, `draw`, `value`；
- `InferenceResult.samples` 返回 samples Arrow 路径；
- posterior predictive 写入 Arrow 并通过 result artifact manifest 暴露；
- `rhat` / `essBulk` / `essTail` 已从 `MCMCChains.summarystats` 转换到 `InferenceResult.summaries`；
- R-hat 偏高和 ESS 偏低会生成参数级 diagnostic warning。

F6.8 已补充 Rust → Julia/Turing 的环境变量门控集成测试：

```text
src-tauri/tests/sci_api_bayes_julia_integration_test.rs
```

默认测试只编译并跳过；设置下面环境变量后才会准备 Julia worker 并运行 Turing PoC：

```sh
YSSBI_RUN_JULIA_BAYES_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml julia_bayes_fixed_linear_poc_runs_when_enabled --test sci_api_bayes_julia_integration_test
```

测试覆盖：

- `JuliaWorkerManager::prepare`；
- `JuliaBayesBackend::fit`；
- Arrow input 写入；
- `bayes_fit` / Turing fixed linear 与通用 safe-AST regression；
- `Normal`、`BernoulliLogit`、`PoissonLog` fixture；
- 参数 summary；
- `rhat` / `essBulk` / `essTail`；
- `saveSamples = true` 时返回 samples ref。

F6.9 已补充 Julia Bayes backend 错误分类。worker 原始错误会保留在 `TaskError.detail`，同时映射为稳定用户可读错误码：

```text
JULIA_BAYES_RUNTIME_UNAVAILABLE
JULIA_BAYES_PACKAGE_UNAVAILABLE
JULIA_BAYES_MODEL_UNSUPPORTED
JULIA_BAYES_INVALID_DATA
JULIA_BAYES_SAMPLING_FAILED
JULIA_BAYES_BACKEND_FAILED
```

当前 samples 生命周期策略：

- 无 samples 的任务完成后清理 worker task 目录；
- 有 samples 的任务保留 task 目录，`InferenceResult.samples.samplesPath` 指向 Arrow draws 文件；
- F7 结果页接入 samples 可视化前，需要补一个后端 samples paging command，前端不应直接读取 worker 临时文件。

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

F6.1/F6.2 当前沿用通用 worker envelope，`BayesModelSpec` 直接放在 `parameters.model` 中，数据表走 `inputPath` Arrow IPC：

```json
{
  "operation": "bayes_fit",
  "inputPath": "input.arrow",
  "outputPath": "output.arrow",
  "metadataPath": "metadata.json",
  "parameters": {
    "model": {
      "dataset": { "sourceType": "table", "sourceId": "..." },
      "response": { "symbol": "y", "column": "..." },
      "predictor": { "type": "binary", "op": "add" },
      "likelihood": { "type": "normal" },
      "parameters": [],
      "sampler": { "algorithm": "nuts" }
    }
  }
}
```

后续真实 MCMC 版本如模型配置变大，可以把 `parameters.model` 改成文件化输入：

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

F6.1/F6.2 skeleton 的 `metadata.json` 直接写标准 `InferenceResult`，并用 warning 暴露 Julia 是否收到输入表：

```json
{
  "summaries": [],
  "diagnostics": {
    "chains": 4,
    "drawsPerChain": 2000,
    "warmup": 1000,
    "divergences": 0,
    "maxTreedepthHits": 0,
    "warnings": [
      {
        "code": "JULIA_BAYES_ENGINE_READY",
        "message": "Julia Bayesian engine op is reachable; Turing sampling is not implemented yet.",
        "parameter": null
      },
      {
        "code": "JULIA_BAYES_INPUT_READY",
        "message": "Julia received 100 rows and 2 columns: y, x.",
        "parameter": null
      },
      {
        "code": "JULIA_BAYES_PREDICTOR_READY",
        "message": "Predictor AST evaluated successfully for preview values: 1.0, 2.0, 3.0.",
        "parameter": null
      },
      {
        "code": "JULIA_BAYES_TURING_LINEAR_POC",
        "message": "Fixed Normal linear regression was sampled with Turing.jl.",
        "parameter": null
      }
    ]
  },
  "samples": null,
  "logPath": null
}
```

worker RPC response 仍只返回 task/file paths：

```json
{
  "taskId": "...",
  "operation": "bayes_fit",
  "outputPath": "...",
  "metadataPath": "..."
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
metadata.json      任务元数据和完整 InferenceResult
run.log            运行日志
ppc.arrow          后验预测，可选
```

完整样本不要通过 JSON 返回给前端。

前端图表按需读取下采样或聚合后的数据。

### F9.1 artifact manifest

已落地稳定的结果产物清单，避免前端或上层应用依赖 Julia/Turing 内部对象：

```rust
pub struct ResultArtifactManifest {
    pub task_id: String,
    pub summary_path: Option<String>,
    pub metadata_path: Option<String>,
    pub samples_path: Option<String>,
    pub posterior_predictive_path: Option<String>,
    pub log_path: Option<String>,
    pub artifacts: Vec<ResultArtifact>,
}
```

`InferenceResult` 中通过 `artifactManifest` 暴露该清单。前端只展示清单和通过 command/service 读取分页或聚合数据，不直接读取 artifact 文件。

Julia Bayesian backend 现在写出：

```text
summary.json       summaries + diagnostics
metadata.json      完整 InferenceResult + artifactManifest
output.arrow       posterior samples，当前 worker output 路径
posterior_predictive.arrow  posterior predictive data，可选
```

清理任务时，Rust application service 会优先使用 `artifactManifest.artifacts` 识别需要清理的 artifact 目录。

### F7.1 samples paging boundary

已落地后端 samples 分页读取边界：

```text
read_bayes_posterior_samples(taskId, offset, limit, parameter?)
  → BayesInferenceService::sample_page
  → InferenceResult.samples.samplesPath
  → read Arrow IPC
  → PosteriorSamplePage
```

DTO：

```rust
pub struct PosteriorSampleRow {
    pub parameter: String,
    pub chain: usize,
    pub draw: usize,
    pub value: f64,
}

pub struct PosteriorSamplePage {
    pub rows: Vec<PosteriorSampleRow>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}
```

前端 service：

```text
src/services/bayes/bayesInferenceService.ts
  readBayesPosteriorSamples(taskId, offset, limit, parameter?)
```

约定：前端只能通过 command/service 分页读取 posterior samples，不直接读取 worker task 目录或 Arrow 文件路径。

### F7.2 plot aggregation data boundary

Plot 聚合不在后端生成图片，也不绑定前端图表库。后端只返回可视化数据 DTO，前端用丰富组件渲染。

已落地 command/service：

```text
read_bayes_trace_plot_data(taskId, parameter?, maxPointsPerChain?)
read_bayes_density_plot_data(taskId, parameter?, bins?)
```

Trace DTO：

```rust
pub struct TracePlotData {
    pub series: Vec<TraceSeries>,
    pub max_points_per_chain: usize,
    pub stride: usize,
}

pub struct TraceSeries {
    pub parameter: String,
    pub chain: usize,
    pub points: Vec<TracePoint>,
}
```

Density DTO：

```rust
pub struct DensityPlotData {
    pub series: Vec<DensitySeries>,
    pub bins: usize,
}

pub struct DensitySeries {
    pub parameter: String,
    pub points: Vec<DensityPoint>,
}
```

当前 density 先使用 histogram density 聚合；后续如果 UI 需要更平滑曲线，可以在不改变前端调用方式的前提下替换为 KDE。

### F7.3 frontend plot rendering

已在 Results tab 中接入数据驱动的前端渲染：

```text
PosteriorTracePreview
  → readBayesTracePlotData
  → SVG line preview

PosteriorDensityPreview
  → readBayesDensityPlotData
  → SVG density preview
```

当前 SVG preview 是轻量首版，用于验证数据边界和交互流程。后续可以替换为 D3/Recharts/Canvas/WebGL 等更丰富组件，只要继续消费 `TracePlotDataDTO` / `DensityPlotDataDTO`，不需要改后端协议。

### F7.4 posterior predictive data boundary

已落地 posterior predictive check 的核心数据边界：

```text
Turing fixed linear PoC
  → posterior_predictive.arrow
  → InferenceResult.posteriorPredictive
  → read_bayes_posterior_predictive(taskId, offset, limit)
  → PosteriorPredictivePage
```

PPC DTO：

```rust
pub struct PosteriorPredictiveRow {
    pub observation: usize,
    pub observed: f64,
    pub mean: f64,
    pub q025: f64,
    pub q975: f64,
}
```

当前 Rust/TS service 已可读取 PPC 分页数据，前端暂不做复杂展示，后续可以用该 DTO 渲染 observed vs predictive interval 图。

### F7.5 diagnostics and samples interaction

已增强 Results tab 交互：

- `DiagnosticsContent` 展示全局 warning 和参数级 warning；
- posterior samples preview 支持参数过滤；
- posterior samples preview 支持上一页 / 下一页分页；
- 分页仍通过 `readBayesPosteriorSamples` command/service，不直接读取 samples 文件。

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

### F10.1 已落地诊断数据边界

当前已落地：

- `ParameterSummary.rhat`；
- `ParameterSummary.essBulk`；
- `ParameterSummary.essTail`；
- `InferenceDiagnostics.divergences`；
- `InferenceDiagnostics.maxTreedepthHits`；
- `read_bayes_trace_plot_data`；
- `read_bayes_density_plot_data`；
- `read_bayes_autocorrelation_data`；
- `read_bayes_posterior_predictive`。

Autocorrelation 由 Rust application service 从 posterior samples Arrow 聚合，返回数据 DTO 而不是图片：

```rust
pub struct AutocorrelationPlotData {
    pub series: Vec<AutocorrelationSeries>,
    pub max_lag: usize,
}
```

前端 Results tab 已增加 Autocorrelation 数据预览，仍通过 service 调用后端 command，不直接读取 samples 文件。

Julia Bayesian backend 的 R-hat / ESS warning code 已使用稳定命名：

```text
RHAT_TOO_HIGH
ESS_TOO_LOW
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

Julia 输出:

```text
summary.json
samples.arrow
metadata.json
run.log
```

Rust 读取 summary/metadata，保存 result ref；前端通过后端命令读取图表所需数据。

### F11.1 已落地数据交换清单

已新增 `BayesDataExchangeManifest`，用于描述一次 Bayesian inference 的数据交换快照：

```rust
pub struct BayesDataExchangeManifest {
    pub version: u32,
    pub task_id: String,
    pub input_table_path: String,
    pub model_spec_path: String,
    pub inference_config_path: String,
    pub output_path: String,
    pub metadata_path: String,
    pub input_rows: usize,
    pub input_columns: Vec<BayesExchangeColumn>,
}
```

Rust Julia backend 现在会在每个 worker task 目录中写出：

```text
input.arrow
model_spec.json
inference_config.json
exchange_manifest.json
```

Julia Bayesian op 优先读取 `exchange_manifest.json`，再从 `model_spec.json` 读取模型规范；如果 manifest 不存在，则回退到旧的 inline `parameters.model`，保持 PoC 和旧测试兼容。

### 约定

- 不通过 JSON 传大表；
- 不让 Julia 直接修改项目 DuckDB；
- 不让 Julia 成为项目状态权威；
- 所有结果路径由 Rust 分配和管理；
- 临时文件和结果文件生命周期由 Rust 控制。

---

## 12. Packaging / Runtime Framework

### 目标

复用当前 Julia runtime/worker，保持系统 Julia 检测、安装和 worker 环境准备清晰可靠。

当前产品取舍：**不做 Bayesian Engine 插件化**。计算能力默认基于 Julia，后续重点是 runtime 检测、依赖准备、版本诊断和 worker 稳定性，而不是把 Bayesian Engine 拆成可选插件。

### MVP

使用当前架构：

```text
src-tauri/julia/worker.jl
src-tauri/julia/ops/bayes_fit.jl
```

用户只通过已有 Julia runtime 检测/安装入口准备 Julia。

### 后续

后续只在确有分发需求时再评估 PackageCompiler 或 sidecar 打包；不是当前架构主线。

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

### F13.1 已落地 fixture-driven tests

已新增默认运行的 fixture golden tests：

```text
src-tauri/tests/sci/fixtures/bayes/linear_normal/simple.json
src-tauri/tests/sci/fixtures/bayes/invalid/*.json
src-tauri/tests/sci_api_bayes_linear_normal_golden_test.rs
src-tauri/tests/sci_api_bayes_validation_golden_test.rs
```

`simple.json` 包含：

- 输入数据列；
- `BayesModelSpec`；
- golden 参数列表；
- posterior mean 容差；
- R-hat 阈值；
- samples / posterior predictive artifact 期望。

默认测试验证：

- fixture 能稳定反序列化为 `BayesModelSpec`；
- likelihood / prior / constraint / sampler 协议稳定；
- fixture 可 materialize 为 Polars `DataFrame`；
- `BayesDataExchangeManifest` camelCase schema 稳定；
- invalid draft fixtures 返回预期 validation error code；
- validation error code 使用稳定的机器可读 `SCREAMING_SNAKE_CASE`；
- validation issue 包含 path 和 message。

当前 invalid fixtures 覆盖：

```text
missing_dataset.json      DATASET_REQUIRED
missing_sigma.json        LIKELIHOOD_SIGMA_PARAMETER_REQUIRED
unbound_predictor.json    DATA_BINDING_REQUIRED
invalid_prior_args.json   PARAMETER_PRIOR_ARGS_INVALID
```

Julia env-gated integration test 也改为读取同一个 valid fixture，避免 Rust fixture 与 Julia fixture 分叉：

```text
src-tauri/tests/sci_api_bayes_julia_integration_test.rs
```

开启 `YSSBI_RUN_JULIA_BAYES_TESTS=1` 时，测试会基于 fixture 检查 summary schema、artifact 存在性、posterior mean 容差和 R-hat 阈值。

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
