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
