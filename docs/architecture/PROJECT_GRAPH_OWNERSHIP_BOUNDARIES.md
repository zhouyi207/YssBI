# Project 与 Graph ownership 边界

本文档定义 Project–Graph 解耦目标，并承担 `project-graph` 精确债务组的清理责任。
当前实现地图仍以 [ARCHITECTURE.md](./ARCHITECTURE.md) 为准。

## Authority

Project 唯一拥有：

- project/session identity 与 lifecycle admission；
- resource path、revision、authority generation 与 transaction publication；
- project filesystem、registry、save/load 和 runtime session ownership；
- database/variable/graph resource 是否存在及是否 resident 的权威事实。

Graph 唯一拥有：

- graph document、node/pin/connection mutation 与 history；
- node protocol、catalog、registry 与 semantic analysis；
- compiler、validated immutable plan 和 graph diagnostics；
- graph projection 的 domain facts，不包含 UI 或 transport DTO。

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

Project 不导入 graph document、compiler、catalog、analysis 或 node DTO。Graph 不导入
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
