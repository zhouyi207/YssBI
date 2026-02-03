你正在为一个“类 Unreal Blueprint”的节点系统设计架构。

请遵循以下强制架构规则（不可违反）：

1. Pin 不允许通过 index（如 inputs[0]）或隐式顺序参与逻辑。
2. Pin 的逻辑绑定必须通过“语义角色（PinRole）”完成，而不是 Pin 名称。
3. Pin 名称仅用于 UI / Debug，不是逻辑锚点。
4. Graph 是唯一的连接关系所有者，Node / Pin 不保存 upstream / downstream。
5. NodeDefinition 是静态描述（定义 Pin、Role、Processor），不包含运行时状态。
6. Node Processor 只能通过 Context API 访问 Pin 数据，不允许直接访问 PinInstance。
7. 必须支持 Blueprint 风格的“动态 Pin”（如 Add / Sequence），通过 Role 或 Group 访问。

在此基础上，请设计或修改代码。


请为节点系统设计一个 PinRole（或等价语义标识）体系，要求：

1. 能区分静态语义 Pin（如 Condition / True / False）
2. 能支持动态语义 Pin（如 Add 的 Operands，Sequence 的 Steps）
3. 不依赖 Pin 名称或顺序
4. 适用于 Data Pin 和 Exec Pin
5. 便于 AI 自动生成节点定义

请给出：
- PinRole enum 示例
- PinDefinition 中 role / group 字段的设计
- 示例：Add、If-Else、Sequence 节点如何使用这些 Role


请设计 NodeExecutionContext API，使 Node Processor：

1. 不能通过 PinId / index / name 直接访问 PinInstance
2. 只能通过语义方式访问输入和输出
3. 同时支持：
   - 单一语义 Pin（如 Condition）
   - 多个动态 Pin（如 Operands / Sequence Steps）

Processor API 设计示例方向：
- get_input_by_role(...)
- get_inputs_by_role(...)
- emit_output_by_role(...)

请给出 Rust trait 定义和使用示例。


请基于以下约束编写节点定义代码：

- 使用 PinRole，而不是 Pin 名称或顺序
- 动态输入 Pin 使用相同 Role 或 Group
- Processor 中通过 Context 的语义 API 遍历动态 Pin
- 不允许出现 inputs[0]、outputs[1] 等代码

请实现一个示例节点：
1. Add（支持任意数量 Operand）
或
2. Sequence（支持任意数量 Exec Step）
