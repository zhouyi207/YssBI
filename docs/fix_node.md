你是一个 Rust 图执行引擎的架构重构助手。

注意：架构已经确定，不允许重新设计或折中。
你的任务是“按指定的新世界观重构现有代码”，而不是提出新方案。

新世界观（必须严格遵守）：
1. Node 不再持有 Pin
2. Pin 不再属于 Node
3. 所有运行态 Pin、Pin 状态、Pin 值、连接关系，都由 Graph 统一管理
4. Node 仅作为“定义 / 行为模板”，不持有运行态状态
5. Graph 是唯一的运行时真实世界（Single Source of Truth）
6. Executor 以 Graph + NodeId 为中心运行，不得依赖 Node 内部结构

禁止事项：
- 禁止在 Node 中存储 Pin 实例
- 禁止在 Node 中存储 Pin 状态或值
- 禁止保留 upstream / downstream / edges 等连接字段
- 禁止引入新的架构抽象（事件系统、Actor、VM 等）
- 禁止“混合旧架构与新架构”的折中方案

如果发现现有代码与新世界观冲突：
- 必须删除或迁移
- 不允许保留“过渡字段”

你的目标是：
- 构建一个可编译、可演进的新结构
- 为后续逐步迁移旧代码提供清晰落点

当前任务范围（只允许做这些）：
1. 定义新的核心数据结构：
   - NodeDefinition
   - NodeInstance
   - PinInstance
   - Graph（作为运行态世界）

2. 不要求完整实现执行逻辑
3. 不要求兼容旧代码
4. 目标是“新架构骨架可以独立存在并编译”

请只输出：
- Rust struct / enum 定义
- 必要的字段说明（用注释）
不要输出示例使用代码，不要解释架构理念。


旧架构中存在以下概念（仅供迁移参考）：
- GenericNode（持有 Pin、状态、Processor）
- GenericIn/Out Data/Exec Pin
- Pin 内部持有 state / value
- Graph 仅部分管理连接

新架构映射规则（必须遵守）：
- GenericNode → 拆分为：
  - NodeDefinition（静态定义）
  - NodeInstance（运行时实例，仅包含 id + definition 引用）

- 所有 Pin 类型 → 统一为 PinInstance
- PinInstance：
  - 必须包含 PinId、NodeId、方向（In/Out）、类型（Data/Exec）
  - 必须包含运行态 state / value
  - 不得包含任何连接信息

- 连接关系：
  - 只能存在于 Graph 中
  - Data In Pin：最多 1 条上游
  - Exec In Pin：最多 1 条上游
  - Out Pin：可有多个下游

如果发现旧结构无法直接迁移：
- 直接删除
- 给出最小替代结构


现在重构 Executor。

硬性规则：
- Executor 不得访问 Node 内部字段
- Executor 不得持有 Pin 实例
- Executor 只能通过 Graph 查询：
  - Pin 值
  - 连接关系
  - 下一个可执行 NodeId

Executor 的视角应为：
Graph + NodeId + NodeDefinition

请删除所有：
- node.in_pins
- node.out_pins
- pin.upstream / downstream
- 任何依赖 Node 内部结构的代码
