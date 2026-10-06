# Data contracts

> Status: Current
> Scope: 共享语义、值树、物理载体与图/执行消费者的契约
> Canonical owners: 本 crate 类型定义；Arrow 和运行值实现分别归各自适配器
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## 语义与值载体

数据 Detail 中的 Physical 与七种 Semantic 是独立的字段元数据，契约由
[Dataset store](../yss-database-store/README.md#field-meaning-and-physical-conversion) 维护。
七种基础语义统一由 `yss-data-contract::SemanticType` 定义，`ValueType::Scalar` 引用它。
DataSeries、DataFrame 及内部结构/专用产物描述保留各自职责，Physical 不进入端口类型层级。
`GROUPED_DATAFRAME_TYPE_ID` 声明 `tabular.grouped_dataframe`，使用既有 `ValueType::Struct` 表达。
它表示源表与分组键；运行句柄、分组遍历和行对应证明归 Relational Contract/Engine，不进入持久化值树。
旧 `DataType` 枚举已删除，Graph 不再将 Int64、Float64、Boolean、String、Date、Time 注册为基础语义。
分解 DataFrame 或选列得到 `DataSeries<Numeric>`、`DataSeries<Identifier>` 等精确语义；类别、等级、
二元映射及精确 Physical 保留在数据元数据中，并参与捕获资源的依赖身份。
NumericFold 只推导语义与标量/数列结构，整数/浮点选择、广播和精度校验由执行适配与内核根据实际
输入完成。非 Numeric 语义不能因底层为整数或浮点数进入数值计算。节点不改写用户设置，数据视图
保持原始值；Schema revision 和完整元数据的依赖身份负责解析与结果失效。

常量、函数签名、编辑器投影和前端解析器共享该契约。标量类型 wire 为
`{ "kind": "Scalar", "inner": "Numeric" }`，数列在 `DataSeries.inner` 中引用它。
`yss-data-contract::DataValue` 是常量、端口字面量及节点默认值共用的持久化值树。`TypedValue` 附加协议 `TypeExpr`；参数不再定义另一份相同的 typed value。整数使用带标签的十进制字符串传输，避免 JavaScript 舍入 `i64/u64`。`DecimalLiteral` 保存规范数字文本，支持精确的 Arrow Decimal 筛选输入，不代表内核定点算术。

`RuntimeValue` 负责运行期列表、记录、资源及模型的容器。其 `Scalar(TabularScalar)` 复用中立标量；`Float64(FiniteFloat64)` 明确表示有限二进制浮点数，所有入口共用有限性校验。Arrow `DataType`、数组及 `RecordBatch` 负责列的物理表示。这些载体不定义第二套基础语义。
项目尚未发布，文件读取与实时命令均直接使用当前类型契约，不提供旧类型声明的迁移或兼容转换。
`ConversionDomain` 拥有转换映射的类别数与编码/标签字节限额，手动映射校验和 Arrow 自动分类映射共用这些限额。自动映射读取实际取值的过程归 Arrow 与节点内核，不在图解析阶段扫描数据。

`aggregation` 拥有描述所支持的语义集合、内部摘要关系的字段，以及分组聚合的操作身份、语义约束和输出命名。内部描述摘要包含有效数、缺失数、Numeric 指标和分类类别数；完整类别频数由关系频数入口提供，节点内核组合为按类型区分的 Result。Catalog、Graph Analysis 与关系执行复用这些契约，不各自推测数值编码是否代表分类变量。

`TabularColumnName` 统一校验列名，只拒绝空串或全空白名称，保留其他名称的精确字符串，包括两侧空格。表格快照、Protocol 选列与筛选条件、连接键共用这项规则，不通过 trim 改写列身份；列表唯一性、连接键组形状及筛选操作和值的约束仍由各自契约校验。

节点配置需要的 `TabularColumnName`、`DecimalLiteral`、`FilterLiteral`、`SemanticValue` 和
`ConversionDomain` 从本 crate 的类型生成 JSON Schema。整数筛选值和精确小数继续使用字符串，
Schema 不将它们改成 JSON number。生成的结构描述供 Node Protocol 投影使用，范围、关联字段、
累计字节数等业务校验仍由原有反序列化与验证入口执行。

Harness 常量工具直接复用 `ValueType`、`DataValue` 与 `TabularSnapshot` 的生成 schema。
DataValue 的有符号/无符号整数 schema 与实际 wire 一致，要求规范十进制字符串；表格列映射的 schema
由本 crate 声明，列名唯一、等长及有限数值仍由原 TabularSnapshot 反序列化校验。
