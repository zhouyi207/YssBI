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
扩展定义使用 `register_builtin_nodes(&mut NodeRegistryBuilder)` 将内置节点加入同一个 builder，
再注册应用 provider 并冻结；不另建内置定义副本。执行函数及其冻结注册表由 [yss-node-kernel](../yss-node-kernel/README.md) 拥有，目录不依赖该执行实现 crate。应用的 `NodeComponents` 校验已安装 kernel 的
参数字段与输出数量。Application 按当前会话的冻结内核注册表标注目录项的 `available`；结构节点无需叶节点内核。
GUI 创建目录保留完整分类与节点，兼容节点目录在端口匹配后同样保留不可用节点；缺少实现的节点置灰并标注“暂不可用”，禁止点击、键盘选择和拖拽创建。AI 搜索只返回可用项。
已有图中的缺少实现节点仍由编辑解析返回阻断诊断。
目录不读取项目文件，不维护图中实例，也不推导连接后的类型、Schema 或血缘。

“运算”下按“算术”“逻辑”“转换”排列；“类别转换”节点位于“转换”中，处理标量和数列的语义转换。分类及其中英文名称由 Rust 目录统一提供。

“逻辑”的六个比较节点同时支持标量和逐元素数列比较，数据序列目录不另设数值或字符串比较节点。“整体相等”独立比较完整的内存列表/记录，返回单个二元值。

“数据处理”下“数据帧”和“数据序列”为同级目录，按主要操作对象归类。时间序列对齐、面板对齐归入“数据帧”；时间序列差分、百分比变化、滚动均值、滞后及面板差分归入“数据序列”。面板差分的对齐数据帧用于提供分组上下文，实际变换和输出对象是数列。“时间序列”和“面板数据”不再单独设目录，节点定义和标识保持不变。

三个 crate 均不依赖 Graph。Node Protocol 依赖序列化基础库和 Data Contract 的唯一基础语义定义；Registry 另用规范化哈希；
Catalog 消费 Protocol、Registry 和 SCI 的中立配置契约。
图文档与语义快照属于 Graph，计划构建与缓存属于 Execution；图诊断定义、校验和前端模板生成
属于 Graph 诊断链路，不会装配进节点目录。
