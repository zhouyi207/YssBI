# Graph resource contract

> Status: Current
> Scope: 不可变函数/数据库 Schema 事实与解析资源依赖
> Canonical owners: [catalog.rs](src/catalog.rs)、[dependencies.rs](src/dependencies.rs) 与 [schema.rs](src/schema.rs)
> Update when: 资源事实、读取追踪或依赖匹配契约改变时

`ResourceCatalogSnapshot` 保存本次捕获的函数声明/正文与数据库 Schema。
`tracked()` 建立一次解析的读取范围；读取已有资源和查找缺失资源都会记录身份，
缓存复用通过 `record_dependencies` 将之前的读取纳入同一范围。

`GraphDependencyManifest` 按类型化 `GraphDependencyKey` 保存实际读取的资源指纹或缺失状态，
由 `GraphSemanticSnapshot.dependencies()` 持有。Runtime 缓存匹配、函数执行包检查和
Application 结果依据重验复用该记录，不转换成另一张字符串版本表。
`matches_dependencies` 校验实际依赖，未读取资源的变化不影响匹配；
解析缓存还使用 `resolution_dependency_fingerprint` 纳入显示与诊断需要的当前资源元数据。

本模块不读取项目文件、不授权资源、不管理项目 revision。
项目事实捕获和交付重验由 [Application](../yss-application/src/graph/README.md) 编排，
Project 授权与 Execution 资源准备保留各自的版本和会话检查。
