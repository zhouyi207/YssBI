# Graph analysis contract

> Status: Current
> Scope: 图分析依据、资源读取观测与诊断身份
> Canonical owners: [basis.rs](src/basis.rs) 与 [diagnostic.rs](src/diagnostic.rs)
> Update when: 分析依据、资源观测或诊断定位契约改变时

`GraphAnalysisBasis` 将冻结 Registry、Kernel 指纹和一次解析实际读取的资源观测绑定在一起。
`ResourceObservationSet` 按资源身份记录 `Present(version)` 或 `Absent(version)`；存在资源的版本
由同一观测携带，不维护另一张版本表。未读取的资源不进入该集合。

[Graph Runtime](../yss-graph-runtime/README.md) 从受跟踪的资源读取生成分析依据；
[Graph Analysis](../yss-graph-analysis/README.md) 将其绑定到唯一的语义快照，
Editor 只投影捕获的依据。项目资源授权及提交 revision 由 Project/Application 管理，
Execution 的计划依据继续由其资源准备入口核验，不用分析指纹替代授权版本。

本模块只定义可序列化契约，不解析图、不读取项目或资源，也不依赖界面和执行运行时。
诊断契约提供代码、参数、严重度及类型化位置；诊断定义和本地化模板由
[Graph Diagnostics](../yss-graph-diagnostics/README.md) 拥有。
