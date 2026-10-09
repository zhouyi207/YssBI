# Node registry

> Status: Current
> Scope: 提供者登记、冻结节点定义与类型/分类索引、注册一致性校验和语义指纹
> Canonical owners: [注册与冻结入口](src/lib.rs)、[只读模型](src/model.rs)、[注册校验](src/validation.rs)与[指纹投影](src/fingerprint.rs)
> Update when: 注册入口、冻结生命周期、定义校验、函数角色或指纹语义改变时

本 crate 接收 [Node Protocol](../yss-node-protocol/README.md) 声明并构造只读定义注册表。
[Catalog](../yss-node-catalog/README.md) 提供内置定义、创建描述与本地化；
[Kernel](../yss-node-kernel/README.md) 拥有实际执行实现。
Registry 不持有图实例、项目资源或执行状态，不推导连接后的类型、Schema 和血缘。

`NodeRegistryBuilder::register_provider` 拥有提供者 ID 唯一性检查。
重复注册返回 `DuplicateProvider`，保留此前已接受的内容。
`register_nominal_validator` 以类型 ID 登记验证器，其身份和版本参与语义指纹。
`freeze` 消耗 builder，校验跨提供者的类型、类型类、构造器、分类、节点、解析器和文本键；
校验分类父链、接口、参数、类型/Schema 引用、声明的 managed 角色和 nominal 验证器要求，
随后计算指纹并发布不可变数据。冻结结果的读取接口只返回只读借用。

`NodeRegistry::clone` 共享同一份冻结数据及全部索引、Manifest 和验证器。
函数计划捕获和其他消费者可以保留定义快照，克隆不会复制元数据或创建另一份定义事实。
登记阶段的提供者唯一性由 builder 保证；冻结阶段不重建第二套提供者 ID 集合。

`RegisteredNode` 明确区分叶实现、结构角色和透明角色，三者互斥。
`TransparentNodeRole::Reroute` 要求无参数的确定性 Identity 协议：同类型的一个固定输入和输出，
输入只允许单条连线，不接收 literal/default；Schema 从输入继承，不另声明表格 Schema。
该角色按注册事实识别，不依赖内置节点 ID，也不登记计算内核。
`StructuralNodeRole` 拥有函数引用字段：Call、GroupApply、GroupTransform 使用 `target`，
Entry/Return 使用 `function`。读取复用 Protocol 的 `Parameters::effective_text`；
Registry 不解释引用中的项目路径。四个函数 interface resolver ID 和角色映射由这里唯一声明；
使用这些 resolver 的叶节点仍由其登记的执行角色判定。

协议指纹保留执行相关声明及参数验证/规范化契约，忽略普通显示信息。
指纹投影直接借用 Protocol 字段序列化，不复制临时 JSON 树；参数按键规范排序由
Canonical Hash 负责，与分组和显示顺序无关。
注册表指纹组合提供者、类型/构造器/类型类、解析器 ID、协议指纹、执行角色/实现身份和
nominal 验证器身份/版本。序列化与规范编码使用 [Canonical Hash](../yss-canonical-hash/README.md)。
指纹字符串由 `hex` 编码为完整的 64 个小写十六进制字符。
注册失败通过 `NodeRegistrationError` 区分一致性校验和规范编码错误；
`InvalidNodeProtocol` 保留节点身份与原始 `ProtocolError` 来源。

Focused validation:

```sh
cargo test -p yss-node-registry --lib
cargo test -p yss-node-catalog --lib
cargo test -p yss-graph-execution --lib
cargo clippy -p yss-node-registry -p yss-node-catalog --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-node-registry --check
```
