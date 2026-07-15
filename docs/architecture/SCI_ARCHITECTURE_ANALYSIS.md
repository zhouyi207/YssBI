# `src-tauri/sci` 架构与文件组织缺陷分析

> 目标：分析 `src-tauri/sci` 当前实现中的文件组织与架构问题，并给出可执行的改进方向。  
> 范围：仅针对 `yss-sci` crate，不包含前端和 Tauri 命令层实现细节。

---

## 一、总体结论

`src-tauri/sci` 当前处于 **“功能可用，但架构边界开始失控”** 的状态。核心问题并非算法正确性，而是：

1. 领域边界混杂（科学计算与应用数据编辑耦合）
2. 模块职责不清（重复能力 + 命名分裂）
3. 文件粒度失衡（超大文件/神文件）
4. API 暴露面偏宽（重构成本高）
5. 抽象层存在半成品与危险实现（维护风险）

若不收敛，后续新增模型/检验时会持续放大维护成本，且更容易引入行为回归。

---

## 二、关键问题清单

### 问题 1：科学计算层与应用数据编辑层耦合

**严重程度：🔴 高**

**现象：**

- `sci` 内既有计量算法模块（`regression`、`ts`、`stats`），又有明显应用层能力（`database/edit_operation.rs`、`database/export.rs`）。
- `EditHistory`、`EditOperation` 这类“UI/交互语义”被放进了科学计算 crate。

**影响：**

- 算法库复用性下降（对外复用会被迫带上编辑语义和依赖）。
- 领域变更耦合：数据编辑需求变更会影响算法 crate 的发布与测试范围。

**建议：**

- 将 `database` 中“编辑历史/导出/数据表操作”迁移到上层业务 crate。
- `sci` 只保留纯函数型统计/计量能力（输入输出尽可能与 UI 无关）。

---

### 问题 2：重复领域模块与命名分裂

**严重程度：🔴 高**

**现象：**

- `panel` 与 `regression::panel` 同时存在：
  - `panel`：对齐与差分（`panel/align.rs`）
  - `regression::panel`：FE/FD/RE/LSDV（`regression/panel/*`）
- 诊断能力分散：
  - `diagnostics/resident.rs` 中有 DW/正态性检验
  - `ts/serial_correlation.rs` 中也有 DW/BG/LB
- 存在命名不一致：`durbin_waston`（拼写）与 `durbin_watson`（标准拼写）并存。

**影响：**

- 调用方心智负担高：同一能力“应该去哪里找”不明确。
- 后续重构容易出现“修一处漏一处”。

**建议：**

- 统一“残差诊断”入口（建议单独聚合为 `diagnostics/residual/*`）。
- 对 `panel` 语义做明确分层：`panel_data`（对齐/差分）与 `panel_model`（估计器），避免同名歧义。
- 修复命名历史债务（保留 deprecated alias 做兼容过渡）。

---

### 问题 3：超大文件（神文件）导致职责过载

**严重程度：🔴 高**

**典型文件：**

- `regression/linear_model/iv2sls.rs`
- `regression/panel/re.rs`
- `regression/diagnostics.rs`
- `regression/panel/fe.rs`

**现象：**

- 单文件内混合：配置定义、核心估计、统计检验、兼容分支、结果结构、辅助工具。
- 文件长度已超出“单人快速心智建模”的舒适区。

**影响：**

- 修改任意局部逻辑都需要大范围回归验证。
- 代码评审质量下降（reviewer 难以建立完整上下文）。
- 单测颗粒度不足，问题定位耗时增加。

**建议：**

- 以“无行为改动”为原则做机械拆分：
  - `*_types.rs`：配置/结果结构
  - `*_fit.rs`：主估计流程
  - `*_tests.rs`：测试与 golden 验证
  - `*_stats.rs`：统计量与检验辅助

---

### 问题 4：工具层存在危险抽象实现

**严重程度：🔴 高**

**现象：**

- `tools/typing.rs` 中 `ArrayLike2D for Vec<Vec<f64>>` 通过 `Box::leak` 返回 `ArrayView`。
- 这是隐式泄漏语义，调用方几乎无法从接口层感知风险。

**影响：**

- 潜在内存增长问题难排查。
- 违背“类型/生命周期清晰可控”的 Rust 设计初衷。

**建议：**

- 废弃该实现（保留短期兼容后移除）。
- 统一要求上游在边界处显式转换为 `Array2<f64>`，在调用层承担一次性分配成本。

---

### 问题 5：对外 API 暴露面偏宽

**严重程度：🟠 中**

**现象：**

- `lib.rs` 直接暴露大量一级模块。
- 多处 `mod.rs` 使用大范围 `pub use` 聚合导出。

**影响：**

- 内部实现细节容易外溢为“事实标准 API”。
- 未来重构时 break 风险和迁移成本上升。

**建议：**

- 定义“稳定 API 白名单”，其余改为 crate 内部可见或按子模块路径访问。
- 先冻结导出策略，再推进拆文件与模块迁移。

---

### 问题 6：抽象层半成品与空模块降低可理解性

**严重程度：🟡 中**

**现象：**

- `base/likelihood_model.rs`、`regression/linear_model/regression_model.rs` 提供 trait，但整体实现体系未形成统一约束。
- `types/mod.rs` 为空，`data/mod.rs` 内容很少且实际使用有限。

**影响：**

- 新维护者难判断哪些抽象是“当前主路径”，哪些是“未来预留”。
- 容易引入“再造一套抽象”的重复建设。

**建议：**

- 明确抽象状态：
  - 要么落地（补齐实现 + 强约束接入）
  - 要么收敛（删减预留层，避免误导）

---

### 问题 7：测试组织规范不一致

**严重程度：🟡 中**

**现象：**

- 同类测试数据路径不一致：既有 `tests/data/*.csv`，又有代码引用 `tests/iris.csv` 的写法。
- 模块内测试与集成测试边界不够清晰。

**影响：**

- 本地/CI 环境下路径与工作目录差异可能导致不稳定。
- 测试维护成本增加。

**建议：**

- 统一测试数据目录（例如固定 `tests/data/`）。
- 明确约定：
  - 单元测试只做纯函数验证
  - 集成测试承载跨模块行为与 golden 对齐

---

## 三、建议的分阶段重构路线

### Phase 1：边界收敛（低风险，优先执行）

- 将 `database` 的编辑/导出能力迁出 `sci`。
- 统一诊断模块入口，处理命名与重复实现。
- 建立 API 白名单，减少外泄接口。

### Phase 2：文件拆分与职责重组（中风险）

- 拆解 `iv2sls.rs`、`re.rs`、`diagnostics.rs` 等超大文件。
- 统一“配置、结果、拟合、检验、工具”目录结构。

### Phase 3：抽象层清理（中高风险）

- 移除或替换 `Box::leak` 风险实现。
- 对半成品抽象做取舍：保留并落地，或直接删减。

### Phase 4：回归验证与兼容收口

- 对外接口保留兼容别名（带 deprecate 提示）。
- 强化 regression/panel/ts 回归测试，确保行为不漂移。

---

## 四、优先级执行清单（建议）

1. 先处理高风险实现：`tools/typing.rs` 泄漏语义
2. 再统一诊断与 panel 命名体系
3. 再做神文件机械拆分（不改行为）
4. 最后做 API 收口与模块迁移

---

## 五、验收标准（可用于后续 PR）

- `sci` 中不再包含 UI/编辑历史语义类型
- DW/BG/LB/normality 等诊断能力只有一个主入口
- 核心大文件被拆分，单文件职责可一句话描述
- 无 `Box::leak` 形式的隐藏生命周期实现
- 公共导出 API 有白名单与文档说明

---

## 六、补充说明

本报告聚焦于“架构可维护性”，不代表当前统计计算结果错误。  
建议采用“先结构、后能力扩展”的策略，避免在当前组织形态下继续叠加新模型，导致技术债进一步指数增长。本文件是迁移提案，不代表目标目录已经存在；实施时应同步迁移调用方并删除旧入口。

---

## 七、建议迁移映射表（旧路径 -> 新路径）

> 说明：这是“目标结构提案”，用于拆分重构任务。  
> 原则：迁移调用方与模块路径同步完成，验证通过后直接删除旧入口；不新增 deprecated re-export 或兼容别名，避免双份实现长期并存。

### 7.1 顶层模块重命名与收敛

| 旧路径 | 建议新路径 | 目的 |
|---|---|---|
| `src-tauri/sci/src/panel/*` | `src-tauri/sci/src/panel_data/*` | 与 `regression::panel` 语义解耦，避免同名歧义 |
| `src-tauri/sci/src/diagnostics/*` | `src-tauri/sci/src/diagnostics/residual/*` | 将残差诊断聚合到单入口 |
| `src-tauri/sci/src/database/*`（编辑/导出相关） | `src-tauri/src/database_ops/*`（上层 crate） | 迁出应用层能力，保持 `sci` 纯计算 |
| `src-tauri/sci/src/types/mod.rs`（空） | 删除或改为 `src-tauri/sci/src/core_types/mod.rs` | 清理空抽象或转为真实公共类型层 |

### 7.2 `panel` 域拆分

| 旧路径 | 建议新路径 | 迁移要点 |
|---|---|---|
| `src-tauri/sci/src/panel/align.rs` | `src-tauri/sci/src/panel_data/align.rs` | 保留函数签名，先做 `pub use` 过渡 |
| `src-tauri/sci/src/panel/mod.rs` | `src-tauri/sci/src/panel_data/mod.rs` | 完成调用方迁移后删除旧 `panel/mod.rs` |
| `src-tauri/sci/src/regression/panel/mod.rs` | `src-tauri/sci/src/panel_model/mod.rs` | 统一估计器语义命名（FE/FD/RE/LSDV） |
| `src-tauri/sci/src/regression/panel/fe.rs` | `src-tauri/sci/src/panel_model/fe/{types.rs, fit.rs, stats.rs}` | 先机械拆分，无行为改动 |
| `src-tauri/sci/src/regression/panel/fd.rs` | `src-tauri/sci/src/panel_model/fd/{types.rs, fit.rs}` | 与 FE/RE 保持一致粒度 |
| `src-tauri/sci/src/regression/panel/re.rs` | `src-tauri/sci/src/panel_model/re/{types.rs, fit.rs, variance.rs}` | 优先拆解超大文件 |
| `src-tauri/sci/src/regression/panel/lsdv.rs` | `src-tauri/sci/src/panel_model/lsdv/{types.rs, fit.rs}` | 拆出 collinearity 处理辅助 |

### 7.3 诊断域统一

| 旧路径 | 建议新路径 | 迁移要点 |
|---|---|---|
| `src-tauri/sci/src/diagnostics/resident.rs` | `src-tauri/sci/src/diagnostics/residual/normality.rs` | Omnibus/JB 保留，修正命名 |
| `src-tauri/sci/src/ts/serial_correlation.rs` | `src-tauri/sci/src/diagnostics/residual/serial.rs` | DW/BG/LB 统一放入 residual serial |
| `src-tauri/sci/src/regression/diagnostics.rs` | `src-tauri/sci/src/diagnostics/regression/{heteroskedasticity.rs, reset.rs, vif.rs, leverage.rs}` | 按检验类型拆文件 |
| `durbin_waston` | `durbin_watson` | 迁移调用方后删除拼写错误的旧函数，不保留 alias |

### 7.4 线性模型域拆分

| 旧路径 | 建议新路径 | 迁移要点 |
|---|---|---|
| `src-tauri/sci/src/regression/linear_model/ols.rs` | `src-tauri/sci/src/regression/linear_model/ols/{types.rs, fit.rs}` | 配置/结果类型与算法分离 |
| `src-tauri/sci/src/regression/linear_model/wls.rs` | `src-tauri/sci/src/regression/linear_model/wls/{types.rs, fit.rs}` | 与 OLS 目录结构保持一致 |
| `src-tauri/sci/src/regression/linear_model/gls.rs` | `src-tauri/sci/src/regression/linear_model/gls/{types.rs, fit.rs}` | 统一实现组织 |
| `src-tauri/sci/src/regression/linear_model/prais.rs` | `src-tauri/sci/src/regression/linear_model/prais/{types.rs, fit.rs}` | 与 serial diagnostics 交叉依赖下沉到接口层 |
| `src-tauri/sci/src/regression/linear_model/iv2sls.rs` | `src-tauri/sci/src/regression/linear_model/iv2sls/{types.rs, fit.rs, first_stage.rs, weak_iv.rs}` | 大文件优先拆分 |
| `src-tauri/sci/src/regression/linear_model/ivliml.rs` | `src-tauri/sci/src/regression/linear_model/ivliml/{types.rs, fit.rs, weak_iv.rs}` | 与 2SLS 共享弱工具变量逻辑 |

### 7.5 工具层与基础抽象

| 旧路径 | 建议新路径 | 迁移要点 |
|---|---|---|
| `src-tauri/sci/src/tools/typing.rs` | `src-tauri/sci/src/tools/array_like.rs`（移除 `Box::leak` 实现） | 保留 `Array1/Array2` 明确边界 |
| `src-tauri/sci/src/tools/transform.rs` | `src-tauri/sci/src/tools/bridge/{ndarray_faer.rs}` | 显式表达“桥接层”职责 |
| `src-tauri/sci/src/tools/matrix.rs` | `src-tauri/sci/src/tools/linalg/rank.rs` | 细化线代工具目录 |
| `src-tauri/sci/src/base/likelihood_model.rs` | `src-tauri/sci/src/modeling/likelihood.rs`（或移除） | 抽象落地/删除二选一 |
| `src-tauri/sci/src/regression/linear_model/regression_model.rs` | `src-tauri/sci/src/modeling/regression.rs`（或移除） | 避免半成品抽象长期悬挂 |

### 7.6 数据与统计模块

| 旧路径 | 建议新路径 | 迁移要点 |
|---|---|---|
| `src-tauri/sci/src/data/mod.rs` | `src-tauri/sci/src/dataset/mod.rs`（若保留） | 明确是否作为公共输入模型 |
| `src-tauri/sci/src/stats/t_test.rs` | `src-tauri/sci/src/stats/hypothesis/t_test.rs` | 与 `wald_test` 同层组织 |
| `src-tauri/sci/src/stats/wald_test.rs` | `src-tauri/sci/src/stats/hypothesis/wald_test.rs` | 建立 `stats/hypothesis/mod.rs` 聚合 |

### 7.7 测试与数据目录统一

| 旧路径/写法 | 建议新路径/写法 | 迁移要点 |
|---|---|---|
| `tests/iris.csv`（代码中引用） | `tests/data/iris.csv` | 统一路径，避免工作目录差异 |
| 分散测试命名 | `tests/{ts,panel,regression}_*.rs` | 用域前缀统一组织 |
| 大量内联测试数据 | `tests/data/*.csv` + helper loader | 提高可读性与复用性 |

---

## 八、建议迁移顺序（可拆任务）

1. 建新目录骨架并同步迁移调用方（不改行为）
2. 先迁移 `panel` 命名冲突与 diagnostics 统一入口
3. 拆分 `iv2sls.rs`、`re.rs`、`diagnostics.rs` 三个高风险神文件
4. 清理 `tools/typing.rs` 中泄漏实现
5. 收口 `lib.rs` 导出白名单，删除旧入口与重复导出

> 推荐策略：每次 PR 只做一类动作（仅迁移/仅拆分/仅 API 收口），避免“结构重构 + 行为改动”混合提交。

