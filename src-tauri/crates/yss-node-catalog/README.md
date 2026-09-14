# yss-node-catalog

> Status: Current
> Scope: 内置节点定义、创建描述、分类、文档与节点目录本地化
> Canonical owners: 本 crate 的源码拥有内置目录；Node 与 Graph 的边界见 [Graph 与 Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md#2-module-ownership)
> Update when: 节点目录装配、创建描述、本地化接口或依赖边界改变时

Node 由三个 crate 组成：

| Crate               | 职责                                                           |
| ------------------- | -------------------------------------------------------------- |
| `yss-node-protocol` | 节点类型、端口、参数、类型和 Schema 约束、执行语义声明与值校验 |
| `yss-node-registry` | 节点、类型和提供者注册，注册一致性校验与指纹                   |
| `yss-node-catalog`  | 内置节点定义、分类、创建描述、帮助文档与节点本地化目录         |

`build_builtin_node_system()` 组装并校验内置注册表和节点目录，返回共享的 `NodeRegistry` 与
`BuiltinCatalog`。`localize_with_resources` 合并调用方提供的资源创建描述；`text` 查询节点元数据文本。
目录不读取项目文件，不维护图中实例，也不推导连接后的类型、Schema 或血缘。

三个 crate 均不依赖 Graph。Node Protocol 只依赖序列化基础库；Registry 另用规范化哈希；
Catalog 消费 Protocol、Registry 和 SCI 的中立配置契约。
图文档、语义快照与编译产物属于 Graph；编译诊断定义、校验和前端模板生成属于 Graph 诊断链路，
不会装配进节点目录。
