# Frontend Bayesian UI Framework Design

本文细化 `bayesian-inference.md` 中的第一部分：`Frontend Bayesian UI Framework`。

目标是设计一套可维护、可渐进实现的贝叶斯参数估计前端框架。它不直接生成或执行 Julia 代码，而是帮助用户构建结构化的 `BayesModelDraft`，再通过后端 service 提交给 Rust 校验和推断。

---

## 1. 设计目标

前端需要达到的效果：

1. 用户可以通过可视化流程完成一次贝叶斯参数估计配置；
2. 用户能清楚区分“预测方程”和“观测分布”；
3. 用户能编辑参数、约束、先验和采样设置；
4. 用户能提交后端校验，并理解错误/警告；
5. 用户能提交推断任务，并查看任务状态；
6. 用户能查看推断摘要、诊断图、后验预测和日志；
7. 前端不暴露 Julia worker、Turing、MCMCChains 等后端实现细节。

核心用户流程采用“模型优先”：

```text
Model tab
  → 编辑完整观测模型：response ~ Distribution(predictor, noise)
  → 安全解析 predictor 为 RawExpressionDTO
  → 用户确认符号角色：dependent / independent / parameter
  → 为 dependent / independent symbols 绑定数据列
  → 为 parameter symbols 设置 constraint、prior distribution、prior args
  → 设置 sampler
  → validate / run
Results tab
  → view result summary
  → view diagnostics
```

---

## 2. 前端模块划分

建议新增前端模块：

```text
src/shared/types/bayes/
  expression.ts
  modelSpec.ts
  prior.ts
  likelihood.ts
  inferenceConfig.ts
  validation.ts
  result.ts
  index.ts

src/services/bayes/
  bayesModelService.ts
  bayesInferenceService.ts
  index.ts

src/features/domain/bayes/
  expressionAst.ts
  expressionSymbols.ts
  parameterInference.ts
  priorDefaults.ts
  likelihoodDefaults.ts
  samplerDefaults.ts
  validationFormatting.ts

src/features/application/bayes/
  bayesActions.ts
  useBayesModelDraft.ts
  useBayesValidation.ts
  useBayesInferenceTask.ts
  index.ts

src/views/BayesView/
  BayesView.tsx
  components/
    BayesPanels.tsx
      FormulaStep
      SymbolRoleStep
      SamplerStep
      ResultOverview
      ResultSummaryContent
      DiagnosticsContent
```

当前前端实现已经把原先单独的 Data Binding、Parameters、Likelihood、Validation、Run 面板收敛到更紧凑的结构：

- Formula 负责完整观测模型编辑，并同步 `formulaText`、`responseSymbol`、`likelihood`、`rawPredictor`、`boundPredictor`；
- Symbols 负责数据源选择、符号角色、数据列绑定、参数约束、prior distribution 和 prior args；
- Sampler 负责可编辑采样参数；
- Validate / Run 是页面右上角操作，不作为独立步骤；
- Results tab 展示 Result Summary 和 Diagnostics。

约定：

- `shared/types/bayes` 只放 DTO 和类型守卫，不放 UI；
- `features/domain/bayes` 放纯函数，不 import React、不 import service、不 import Zustand；
- `features/application/bayes` 放用例编排 hook，可以调用 service；
- `services/bayes` 是唯一调用 Tauri command 的前端入口；
- `views/BayesView` 只做页面组合和展示，不直接调用 `invoke`。

---

## 3. 页面框架

### 3.1 推荐页面形态

第一版页面使用两个顶层 tabs，避免在贝叶斯独立窗口中重复标题：

```text
┌──────────────────────────────────────────────────────────────┐
│ Model | Results                         Validate Run Cancel  │
├──────────────────────────────────────────────────────────────┤
│ Model tab                                                    │
│   1 Formula                                                  │
│   2 Symbols                                                  │
│   3 Sampler                                                  │
├──────────────────────────────────────────────────────────────┤
│ Results tab                                                  │
│   Result Summary                                             │
│   Diagnostics                                                │
└──────────────────────────────────────────────────────────────┘
```

后续可以在 Results tab 中继续增加 Trace、Density、Posterior predictive、Log 等区域，但不需要改变 Model tab 的配置协议。

### 3.2 为什么先用 wizard

贝叶斯模型配置有强顺序依赖，但用户通常先有数学模型，再绑定数据：

```text
观测模型 → predictor raw expression → 符号角色 → 数据绑定 / 参数约束和先验 → sampler → validation
```

Wizard 可以减少一次性暴露过多概念。后续高级用户可以提供“专家模式”，用同一套 draft state 展示成表单/脚本式布局。

### 3.3 布局约定

- 使用现有 shadcn/ui primitives；
- 普通提示使用项目 shared toast；
- 不使用 `alert` / `confirm` / `prompt`；
- 大内容区域使用 `OverlayScrollbar`；
- 保持 `flex`, `min-h-0`, `flex-1` 的滚动布局约定。

---

## 4. Draft State 框架

### 4.1 Draft 数据模型

前端维护的是“草稿模型”，不是后端最终模型。

```ts
export interface FormulaDraftDTO {
  formulaText: string;
  responseSymbol?: string;
  rawPredictor: RawExpressionDTO;
}

interface BayesModelDraftDTO {
  formulaText: string;
  responseSymbol?: string;
  rawPredictor: RawExpressionDTO | null;
  symbols: SymbolDraftDTO[];

  dataset: BayesDatasetSelectionDTO | null;
  responseBinding: ResponseBindingDTO | null;
  dataBindings: Record<string, string>;

  boundPredictor: ExpressionDTO | null;
  likelihood: LikelihoodSpecDTO;
  parameters: ParameterSpecDTO[];
  sampler: InferenceConfigDTO;
}
```

其中 `formulaText` 用于展示完整观测模型；Formula 保存时必须从分布参数中的 predictor 输入解析出 `rawPredictor`，再结合 Symbols 中的角色生成 `boundPredictor`。提交推断前必须完成符号角色确认、数据源选择、数据列绑定、参数约束和 prior args 配置。

### 4.2 Dataset selection

```ts
export interface BayesDatasetSelectionDTO {
  sourceType: 'table' | 'query' | 'result_source';
  sourceId: string;
  columns: BayesColumnMetaDTO[];
}

export interface BayesColumnMetaDTO {
  name: string;
  dtype: 'number' | 'integer' | 'boolean' | 'string' | 'date' | 'unknown';
  nullable: boolean;
}
```

第一版只允许数值列进入模型。分类变量、分组变量、日期变量后续再扩展。

### 4.3 Draft state 生命周期

```text
create draft
  → update model equation parts
  → parse predictor text to RawExpressionDTO
  → extract raw symbols
  → classify symbols as dependent / independent / parameter
  → select dataset
  → bind dependent / independent symbols to dataset columns
  → edit parameter constraints, prior distribution, prior args
  → build boundPredictor
  → edit sampler
  → validate draft through service
  → submit inference through service
```

### 4.4 State 存放约定

第一版建议使用页面内 hook：

```text
useBayesModelDraft
```

暂不放全局 Zustand，除非后续需要跨页面恢复或多任务管理。

可以把跨 session 偏好放 localStorage，例如：

```text
默认 chains
默认 samples
默认 prior 模板
最近使用 likelihood
```

不要把运行中任务状态只放 localStorage；任务状态以后应以后端任务系统为准。

---

## 5. Expression Editor 框架

### 5.1 UI 效果

用户输入：

```text
a * exp(-b * x) + c
```

界面展示：

```text
识别的数据列：x
识别的参数：a, b, c
不支持的符号：无
```

如果输入：

```text
a * unknown + b
```

且 `unknown` 既不是数据列也不是参数，前端可以先标注为潜在参数，但最终以后端 validation 为准。

### 5.2 前端解析策略

MVP 有两种选择：

#### 方案 A：前端只做轻量 tokenizer，后端做正式 parse

优点：实现快，避免前后端 parser 不一致。

缺点：输入时的实时反馈较弱。

#### 方案 B：前端实现同一套受限表达式 parser

优点：实时反馈好。

缺点：要保证 Rust 和 TypeScript parser 规则一致。

建议路线：

```text
Phase 1: 方案 A
Phase 2: 补 TypeScript parser，用 golden fixture 保持和 Rust parser 一致
```

### 5.3 Expression DTO

```ts
export type ExpressionDTO =
  | { type: 'number'; value: number }
  | { type: 'column'; name: string }
  | { type: 'parameter'; name: string }
  | { type: 'unary'; op: UnaryOpDTO; arg: ExpressionDTO }
  | { type: 'binary'; op: BinaryOpDTO; left: ExpressionDTO; right: ExpressionDTO }
  | { type: 'call'; function: MathFunctionDTO; args: ExpressionDTO[] };
```

允许函数：

```ts
export type MathFunctionDTO =
  | 'exp'
  | 'log'
  | 'sqrt'
  | 'abs'
  | 'sin'
  | 'cos'
  | 'min'
  | 'max';
```

### 5.4 约定

- 前端不得把 `formulaText` 直接提交给 Julia；
- 前端可以把 `formulaText` 提交给 Rust parse command，Rust 返回 response symbol 和 raw predictor AST；
- 后端返回的 AST 是提交推断的唯一表达式来源；
- 前端显示的公式预览可以由 AST 格式化生成，避免显示和实际运行不一致。

---

## 6. Likelihood UI 框架

### 6.1 第一版支持

MVP 建议只实现：

```text
Normal
```

界面：

```text
观测模型
  y ~ Normal(mu, sigma)

mu
  predictor expression

sigma
  parameter: sigma
```

后续扩展：

```text
BernoulliLogit
PoissonLog
LogNormal
StudentT
```

### 6.2 Likelihood DTO

```ts
export type LikelihoodSpecDTO =
  | {
      type: 'normal';
      mean: { source: 'predictor' };
      sigma: { parameter: string };
    }
  | {
      type: 'bernoulli_logit';
      logit: { source: 'predictor' };
    }
  | {
      type: 'poisson_log';
      logRate: { source: 'predictor' };
    };
```

### 6.3 UI 约定

不要让用户只输入：

```text
y = a*x+b
```

必须明确展示为：

```text
y ~ Normal(a*x+b, sigma)
```

这能帮助用户理解“数学方程”和“观测噪声”是两件事。

---

## 7. Parameter Table 框架

### 7.1 UI 效果

参数表：

| 参数 | 约束 | 先验 | 参数值 | 状态 |
|---|---|---|---|---|
| a | real | Normal(0, 10) | | ok |
| b | real | Normal(0, 10) | | ok |
| sigma | positive | Exponential(1) | | ok |

### 7.2 Parameter DTO

```ts
export interface ParameterSpecDTO {
  name: string;
  constraint: ParameterConstraintDTO;
  prior: PriorSpecDTO;
}

export type ParameterConstraintDTO =
  | { type: 'real' }
  | { type: 'positive' }
  | { type: 'unit' };
```

### 7.3 Prior DTO

```ts
export type PriorSpecDTO =
  | { distribution: 'normal'; args: [number, number] }
  | { distribution: 'log_normal'; args: [number, number] }
  | { distribution: 'uniform'; args: [number, number] }
  | { distribution: 'beta'; args: [number, number] }
  | { distribution: 'gamma'; args: [number, number] }
  | { distribution: 'exponential'; args: [number] }
  | { distribution: 'student_t'; args: [number, number, number] }
  | { distribution: 'cauchy'; args: [number, number] }
  | { distribution: 'half_normal'; args: [number] };
```

### 7.4 自动参数合并规则

当用户修改方程后，前端重新识别参数。合并规则：

```text
旧参数仍存在 → 保留用户设置
新参数出现 → 使用默认 prior
旧参数消失 → 标记为 unused，可提示删除
sigma 如果 likelihood 需要但不存在 → 自动添加 positive + Exponential(1)
```

不要因为用户改公式就重置所有先验。

### 7.5 默认先验建议

MVP：

```text
real 参数      Normal(0, 10)
positive 参数  Exponential(1)
unit 参数      Beta(2, 2)
sigma          Exponential(1)
```

这些只是 UI 默认值，后端 validation 仍然要验证。

---

## 8. Sampler Settings 框架

### 8.1 UI 效果

基础设置：

```text
算法：NUTS
链数：4
每条链样本数：2000
预热次数：1000
随机种子：1234
目标接受率：0.8
```

高级设置折叠：

```text
最大树深度：10
初始化策略
线程策略
保存完整样本
```

### 8.2 DTO

```ts
export interface InferenceConfigDTO {
  algorithm: 'nuts';
  chains: number;
  samples: number;
  warmup: number;
  seed?: number;
  targetAccept?: number;
  maxTreeDepth?: number;
  saveSamples: boolean;
}
```

### 8.3 默认值

```ts
export const DEFAULT_BAYES_SAMPLER: InferenceConfigDTO = {
  algorithm: 'nuts',
  chains: 4,
  samples: 2000,
  warmup: 1000,
  targetAccept: 0.8,
  maxTreeDepth: 10,
  saveSamples: true,
};
```

### 8.4 约定

- 第一版不提供十几种算法；
- 普通用户只看到 NUTS；
- 离散参数、SMC、MH 等后续再扩展；
- 前端基础范围校验只是用户体验，最终以后端 validation 为准。

---

## 9. Validation UI 框架

### 9.1 Validation report DTO

```ts
export interface ValidationReportDTO {
  ok: boolean;
  errors: ValidationIssueDTO[];
  warnings: ValidationIssueDTO[];
}

export interface ValidationIssueDTO {
  code: string;
  severity: 'error' | 'warning';
  message: string;
  path?: string;
  hint?: string;
}
```

### 9.2 UI 效果

校验面板：

```text
Errors
  ✕ predictor: log(x) 中 x 存在非正值
  ✕ sigma 必须是 positive 参数

Warnings
  ⚠ samples 较少，ESS 可能不足
  ⚠ 数据行数超过建议规模，MCMC 可能较慢
```

点击 issue 可以跳转到对应 step/field。

### 9.3 校验触发策略

```text
字段编辑时：前端轻量校验
点击 Validate：后端完整校验
点击 Run：如果没有 fresh validation，先 validate 再 submit
```

### 9.4 Fresh validation 约定

记录 draft hash：

```ts
interface BayesValidationState {
  draftHash: string;
  report: ValidationReportDTO | null;
}
```

如果用户修改了 draft，旧 validation report 变成 stale：

```text
当前模型已修改，请重新校验
```

---

## 10. Run / Task UI 框架

### 10.1 MVP 同步模式

如果后端第一版还是同步 PoC，前端可以只显示：

```text
Running...
```

但 UI 设计上应预留任务化：

```ts
interface BayesInferenceTaskDTO {
  taskId: string;
  status: 'queued' | 'running' | 'cancelling' | 'cancelled' | 'completed' | 'failed';
  progress?: TaskProgressDTO;
  result?: InferenceResultRefDTO;
  error?: TaskErrorDTO;
}
```

### 10.2 正式任务模式

命令流：

```text
submitBayesInference
  → taskId
getBayesInferenceStatus(taskId)
cancelBayesInference(taskId)
readBayesInferenceResult(taskId)
```

前端不得调用：

```text
run_julia_bayes
cancel_julia_task
prepare_bayes_worker
```

### 10.3 进度展示

前端展示通用任务进度：

```text
Validating
Preparing data
Starting sampler
Sampling chain 1/4
Writing result
Completed
```

不要展示 Julia worker 内部协议名。

---

## 11. Result UI 框架

### 11.1 Summary panel

展示参数摘要：

| parameter | mean | sd | median | 2.5% | 97.5% | rhat | ess_bulk | ess_tail |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| a | 1.96 | 0.08 | 1.96 | 1.80 | 2.12 | 1.001 | 3240 | 2810 |

### 11.2 Diagnostics panel

展示诊断状态：

```text
✓ R-hat 全部小于 1.01
✓ ESS 充足
✓ 未发现 divergence
⚠ 参数 a 和 b 后验相关性较高
```

### 11.3 Charts

第一版图表：

```text
trace plot
density plot
autocorrelation plot
posterior predictive check
```

数据读取约定：

- summary JSON 可一次读取；
- samples 不通过 command 一次全量返回；
- 图表数据由后端提供下采样/聚合接口。

例如：

```text
read_bayes_trace_data(taskId, parameter, maxPoints)
read_bayes_density_data(taskId, parameter)
read_bayes_ppc_data(taskId)
```

---

## 12. Services 框架

### 12.1 Service 文件

```text
src/services/bayes/bayesModelService.ts
src/services/bayes/bayesInferenceService.ts
src/services/bayes/index.ts
```

### 12.2 命令包装

```ts
export async function parseBayesExpression(input: ParseExpressionRequestDTO): Promise<ParseExpressionResponseDTO>;

export async function validateBayesModel(input: ValidateBayesModelRequestDTO): Promise<ValidationReportDTO>;

export async function submitBayesInference(input: SubmitBayesInferenceRequestDTO): Promise<BayesInferenceTaskDTO>;

export async function getBayesInferenceStatus(taskId: string): Promise<BayesInferenceTaskDTO>;

export async function cancelBayesInference(taskId: string): Promise<void>;

export async function readBayesInferenceResult(taskId: string): Promise<InferenceResultDTO>;
```

### 12.3 约定

- services 是唯一 `invoke` 入口；
- services 必须 normalize/validate 后端响应；
- views 不接收 `unknown` 后端响应；
- 后端错误映射为用户可读 Error，但保留 `code` 供 UI 判断。

---

## 13. Application Hooks 框架

### 13.1 `useBayesModelDraft`

职责：

- 管理 draft；
- 更新字段；
- 合并参数表；
- 计算 draft hash；
- 提供 dirty/stale 状态。

不负责：

- 调用 Julia；
- 读取结果文件；
- 实际执行后端推断。

### 13.2 `useBayesValidation`

职责：

- 调用 `validateBayesModel`；
- 管理 validation loading/error/report；
- 判断 validation 是否 stale；
- 将 issue 映射到 UI step/field。

不负责：

- 修改模型 draft；
- 绕过后端 validation 直接提交任务。

### 13.3 `useBayesInferenceTask`

职责：

- 提交推断任务；
- 轮询或订阅任务状态；
- 取消任务；
- 读取结果；
- 统一处理 task error。

不负责：

- 解析 samples 文件；
- 直接访问 Julia worker；
- 直接读取本地结果路径。

### 13.4 `bayesActions.ts`

作为 application 层统一导出：

```ts
export {
  parseBayesExpression,
  validateBayesModel,
  submitBayesInference,
  getBayesInferenceStatus,
  cancelBayesInference,
  readBayesInferenceResult,
} from '@/services/bayes';
```

视图 import application actions，而不是直接 import service。

---

## 14. Component Framework

### 14.1 `FormulaStep`

目标：先输入模型方程，解析 response symbol、raw predictor 和 raw symbols。

需要展示：

- predictor text input；
- 支持函数说明；
- response symbol；
- raw predictor preview；
- 识别出的 raw symbols；
- parse error。

约定：

- 文本输入不等于可执行代码；
- parse 成功后生成 response symbol 和 `rawPredictor`，不直接判定 column/parameter；
- parse 失败时不能继续提交推断。

### 14.2 `SymbolRoleStep`

目标：展示方程识别出的符号，并让用户确认每个符号是 data variable 还是 parameter。

需要展示：

- symbol name；
- inferred role；
- current role；
- user edited 状态；
- 切换 data / parameter 的控件。

约定：

- 自动分类只是猜测，用户必须可修改；
- 角色改变后重新生成 `boundPredictor`；
- 角色改变后参数表必须合并，不丢失仍存在参数的 prior 设置。

### 14.3 `DataBindingStep`

目标：选择数据源，将 data symbols 绑定到数据库列，并选择响应变量列。

需要展示：

- 数据源名称；
- 响应变量列；
- 每个 data symbol 的数据库列绑定；
- 列类型和 nullable 状态；
- 未绑定提示。

约定：

- 模型符号和数据库列名解耦，例如 `x → time_seconds`；
- 第一版只允许数值列绑定到 data symbol；
- 响应变量也必须绑定到数值列；
- 真实可用性以后端 schema/validation 为准。

### 14.4 `ParametersStep`

目标：只为 parameter symbols 编辑约束和先验。

需要支持：

- 添加/删除参数；
- 修改 constraint；
- 修改 prior distribution；
- 修改 prior args；
- 显示 unused/missing 状态；
- 一键恢复默认先验。

约定：

- 从表达式识别出来的参数不能被静默删除；
- 删除仍被表达式引用的参数时必须阻止或提示；
- 修改 constraint 后，应提示 prior 是否兼容，但最终以后端 validation 为准。

### 14.5 `LikelihoodStep`

目标：选择观测模型，并展示响应变量绑定后的概率模型。

MVP：

```text
y ~ Normal(predictor, sigma)
```

需要展示：

- 响应变量符号和数据库列绑定；
- predictor 作为均值；
- sigma 参数选择或自动创建；
- likelihood 说明。

### 14.6 `SamplerStep`

目标：配置采样参数。

MVP 只展示 NUTS。

基础字段：

```text
chains
samples
warmup
seed
targetAccept
```

高级字段：

```text
maxTreeDepth
saveSamples
```

约定：

- 数字输入需要 clamp 到合理范围；
- 不隐藏后端 warning，例如样本太少；
- 用户修改默认值后可以保存为偏好。

### 14.7 `ValidationPanel`

目标：展示后端 validation report。

需要支持：

- errors/warnings 分组；
- 点击 issue 跳转字段；
- stale validation 提示；
- validate 按钮；
- validate loading 状态。

### 14.8 `RunPanel`

目标：提交任务和展示任务状态。

需要支持：

- Run 按钮；
- Cancel 按钮；
- task status；
- progress 文案；
- 错误详情；
- 跳转结果。

约定：

- 如果 validation stale，Run 前先触发 validation；
- validation 有 error 时不能 Run；
- warning 不阻止 Run，但要明确展示。

### 14.9 `ResultSummaryPanel`

目标：展示参数摘要表。

需要支持：

- 参数名；
- mean/sd/median/credible interval；
- rhat/ess；
- 按诊断状态高亮。

### 14.10 `DiagnosticsPanel`

目标：展示采样质量。

第一版展示：

```text
R-hat summary
ESS summary
divergence count
warning list
```

后续加图表。

### 14.11 `PosteriorPredictivePanel`

目标：展示后验预测检查。

MVP 可以先占位：

```text
Posterior predictive checks will be available after backend support.
```

不要为了 UI 完整而伪造数据。

### 14.12 `RunLogPanel`

目标：展示后端结构化日志或文本日志。

约定：

- 日志通过后端读取；
- 前端不直接读取本地路径；
- 日志默认折叠，错误时自动展开。

---

## 15. Route / Navigation 设计

### 15.1 推荐路由

如果作为独立页面：

```text
/bayes
```

如果作为数据表或图节点结果的子功能，可以后续加入：

```text
/database/:tableId/bayes
/result/:resultId/bayes
```

### 15.2 与 Graph 的关系

MVP 可以先做独立页面或结果页工具，不急于做图节点。

后续图节点化时，节点配置仍然复用同一套：

```text
BayesModelDraftDTO
ModelSpecDTO
InferenceConfigDTO
```

不要为图节点另建一套模型格式。

### 15.3 导航约定

- 使用 React Router；
- 不使用 active-page flags；
- 页面状态能从 route/context 恢复时优先使用 route state；
- 长任务状态以后端任务 ID 为准。

---

## 16. Error / Empty State 设计

### 16.1 空状态

```text
未选择数据：请选择一个数据表或结果源。
无数值列：当前数据源没有可用于建模的数值列。
未输入方程：请输入模型方程，例如 y = a * x + b。
未校验：运行前需要先校验模型。
未运行：校验通过后可以开始采样。
无结果：任务完成后会显示参数摘要和诊断。
```

### 16.2 错误状态

错误分层：

```text
FormError       前端表单错误
ValidationError 后端模型校验错误
TaskError       推断任务失败
ServiceError    IPC/响应格式错误
RuntimeError    Julia runtime 不可用
```

UI 不应该把 Rust/Julia stack trace 直接展示给普通用户。可以提供“复制详情”。

### 16.3 Julia runtime 错误

如果后端返回 Julia runtime 不可用，前端应该展示普通产品文案：

```text
贝叶斯推断需要 Julia 运行环境。请先安装或修复 Julia。
```

并跳转到现有 Julia runtime status/install 入口。

不要展示：

```text
JuliaWorkerManager failed...
```

---

## 17. Accessibility / Interaction 约定

- 所有 input/select/button 必须有 label；
- 参数表可键盘操作；
- 错误 issue 可以聚焦到对应字段；
- 长任务按钮要有 loading/disabled 状态；
- 颜色不能作为唯一状态表达，错误/警告要有文字和图标；
- 图表需要提供表格或摘要替代信息。

---

## 18. Testing 设计

### 18.1 类型和 normalize 测试

目录建议：

```text
src/shared/types/bayes/*.test.ts
```

测试：

- DTO normalize；
- invalid response rejection；
- prior args validation helper；
- validation issue parser。

### 18.2 domain 纯函数测试

目录建议：

```text
src/features/domain/bayes/*.test.ts
```

测试：

- 参数合并规则；
- 默认 prior；
- draft hash；
- expression symbol extraction；
- validation issue formatting。

### 18.3 component 测试

重点测：

- 修改方程后参数表保留旧设置；
- validation stale 状态；
- errors 阻止 Run；
- warnings 不阻止 Run；
- service error 展示。

### 18.4 不测试内容

前端测试不负责验证：

- Turing 采样正确性；
- Julia worker 协议；
- MCMC 数值结果。

这些属于 Rust/Julia 后端 golden tests。

---

## 19. 实施阶段

### Phase F0：类型和 mock UI

完成：

- `shared/types/bayes` DTO；
- `features/domain/bayes` 默认值和参数合并；
- `BayesView` 静态页面；
- mock validation/result。

目标：确定交互和数据结构。

### Phase F1：接入后端 validation

完成：

- `parseBayesExpression` service；
- `validateBayesModel` service；
- ValidationPanel；
- fresh/stale validation 状态。

目标：前端不再自己判断模型是否可运行。

### Phase F2：接入同步 PoC 推断

完成：

- `fitBayesModel` 或 `submitBayesInference` MVP；
- RunPanel；
- SummaryPanel；
- Runtime error 处理。

目标：跑通最小 Bayesian linear regression。

### Phase F3：任务化

完成：

- submit/status/cancel/result；
- task hook；
- task event/轮询；
- 结果读取。

目标：支持长时间 MCMC。

### Phase F4：图表和诊断完善

完成：

- trace plot；
- density plot；
- autocorrelation；
- posterior predictive check；
- diagnostic warnings。

目标：用户能够判断结果是否可信。

---

## 20. 明确不做

前端第一版不做：

- 直接生成 Julia/Turing 源码；
- 直接调用 Julia worker；
- 自定义 Julia 函数编辑器；
- 任意模型脚本；
- 层级模型 UI；
- ODE 模型 UI；
- 多任务并发管理 UI；
- 直接读取 samples 文件路径；
- 伪造诊断或后验预测数据。

---

## 21. 验收标准

第一阶段前端框架完成时，应满足：

1. 所有后端调用都在 `src/services/bayes`；
2. `BayesView` 不直接调用 `invoke`；
3. draft 可以完整表达：数据、响应变量、预测方程、likelihood、参数、sampler；
4. 修改方程后参数表按规则合并，不丢失用户已编辑先验；
5. validation report 可以展示、定位、区分 stale；
6. Run 前会确保 validation fresh；
7. 结果展示使用标准 `InferenceResultDTO`；
8. 没有 Julia/Turing 术语泄漏到普通 UI 文案；
9. 前端测试覆盖 DTO normalize 和参数合并规则；
10. UI 遵守项目交互约定，不使用 browser alert/confirm/prompt。
