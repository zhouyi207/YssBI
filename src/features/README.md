# Features

- `application/` 编排跨领域用户用例，调用 services 并协调投影发布。
- `core/` 保存 Rust 投影、显式未保存草稿和共享交互状态。
- `domain/` 提供不依赖 React、Tauri 或 services 的领域规则。

Application 可以依赖 Core、Domain 和 Services；依赖不能从 Core 或 Domain 反向指向 Application 或界面。完整边界由[当前架构](../../docs/architecture/ARCHITECTURE.md#3-layer-and-dependency-direction)和[架构门禁](../../docs/development/ARCHITECTURE_GATES.md)维护。
