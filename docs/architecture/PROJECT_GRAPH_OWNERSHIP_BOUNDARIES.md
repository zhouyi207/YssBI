# Project 与 Graph ownership 边界

本文档定义 Project–Graph 解耦目标，并承担 `project-graph` 精确债务组的清理责任。
当前实现地图仍以 [ARCHITECTURE.md](./ARCHITECTURE.md) 为准。

## Authority

Project 唯一拥有：

- project/session identity 与 lifecycle admission；
- Operation/History correlation identity、通用 resource/project/transaction revision、
  authority generation 与 transaction publication；
- project filesystem、registry、save/load 和 runtime session ownership；
- database/variable/graph resource 是否存在及是否 resident 的权威事实。

Graph 唯一拥有：

- node/pin/connection mutation、validation、patch/apply 与 compile semantics；
- node protocol、catalog、registry 与 semantic analysis；
- compiler、validated immutable plan 和 graph diagnostics；
- graph projection 的 domain facts，不包含 UI 或 transport DTO。

Pure Leaf `graph_document` 唯一拥有 serialized `GraphDocument`、`GraphResourcePath`、
`GraphRevision` 及 document node/connection/port identities。其 `model.rs` 保留既有
`TypedValue = serde_json::Value` untagged persisted wire；该许可不扩展到 Graph behavior、
错误、compiler/runtime package 或其他 Pure Leaf 文件。

## Dependency direction

```text
             Application
              /        \
             ▼          ▼
         Project       Graph
             \          /
              ▼        ▼
          opaque Pure Leaf contracts
```

Project 可导入 `graph_document` 数据契约，但不导入 `crate::graph` behavior layer、compiler、
catalog、analysis 或 node DTO。Graph 不导入
ProjectState、ProjectStore、project filesystem、Tauri 或 event/schema transport。
Application 通过 opaque resource path、revision、session token 和 owner-defined result 编排
两侧；共享 identity/value shape 必须有一个中立 canonical owner。

跨 owner 操作采用 capture → 锁外工作 → revalidate → commit：Project 先捕获 session 与
resource basis，Graph 在纯输入上分析或编译，Application 在 commit 前要求 Project 重新验证
同一 basis。Graph 不回调 Project 获取“最新值”，Project 也不解释 Graph 内部结构。

## 迁移规则

一次迁移只移动一个明确 authority 或 mapping。新 owner 正常编译并覆盖既有行为后，在同一
slice 切换全部 caller、删除旧 owner 和对应精确债务。不得保留两套 projection、revision、
resource lookup 或 resident-install implementation。

完成条件：`debt/project_graph.rs` 为空，Project 与 Graph 的 production dependency 仅通过
Application orchestration 和中立 contract 形成，相关 focused tests 与 production
architecture audit 全部通过。

## Final cutover checkpoint（2026-08-29）

Project–Graph Tasks 1–10 已完成：graph-document data、opaque path、Graph catalog/registry/
analysis/compiler、Graph mutation/compatibility/subgraph 与 Project history/lifecycle 均已
收归最终 owner。Project 只保留 durable authority 与 pure persisted contracts；Application
负责捕获 session、调用 Graph、重验 Project authority 并提交线性 mutation facts。

Graph production source 不导入 ProjectState、ProjectStore、filesystem、Tauri 或 transport
DTO；Graph compiler 只输出 neutral package，Application 在 Graph 与 Execution 之间完成
typed mapping。`rust_architecture_debt()` 对 Project–Graph group 为空，并由 production
architecture audit 双向核对。
