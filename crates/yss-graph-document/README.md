# Graph document

> Status: Current
> Scope: 图持久化文档、稳定地址、可逆补丁和常量输入
> Canonical owners: [model.rs](src/model.rs)、[change.rs](src/change.rs)、[constant_value.rs](src/constant_value.rs) 与 [semantic_hash.rs](src/semantic_hash.rs)
> Update when: 文档格式、地址、补丁、常量或图指纹契约改变时

`GraphDocument` 保存节点、连线、动态端口绑定、输入字面量和图内常量。
Project 拥有当前可编辑文档及历史；本模块只定义持久化事实与可逆补丁。
[Graph Document Edit](../yss-application/src/graph/README.md#module-ownership) 负责结构校验及补丁的原子应用，
Analysis 拥有解析后的类型、Schema、血缘和诊断，不把解析结果写回文档。

表格常量输入按 JSON 列对象读取，保留原始列顺序和重复列供契约校验。
输入只解析一次，再复用 Data Contract 的标量反序列化和表格校验；有符号与无符号整数保持精度。
非对象、非数组列、嵌套单元格、重复列和不等长列分别交付现有类型化错误。
全部校验完成后才将常量文本替换为不可变 `TabularSnapshot`，失败不改写原值或快照。
持久化表格常量仅保存该快照和 `Null` 数据值，复制只分配新的常量身份。

`resolution_document_fingerprint` 包含解析快照需要的诊断地址、常量名称及 orphan 元数据。
`semantic_document_fingerprint` 保留执行语义和有效输入顺序，排除画布位置、标签和未引用的派生元数据。
两类指纹分别服务 Runtime 解析缓存及执行身份；图资源路径由项目目录约定与资源名称校验定义。
