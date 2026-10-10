# Project file operations 参考

> Status: Historical
> Scope: React 文件操作、输入缓冲与资源身份的参考行为
> Canonical owners: 参考源码的 resourceActions/fileManagement；当前资源用例与持久化由 Rust Application/Project 拥有
> Update when: 参考行为或当前契约入口改变时

本目录不参与原生构建。当前入口见[根 README](../../../../../README.md)、[文档索引](../../../../../docs/README.md)、[Project application](../../../../../crates/yss-application/src/project/README.md)和 [Project](../../../../../crates/yss-project/README.md)。原生界面见 [GPUI host](../../../../../crates/yss-desktop-gpui/README.md)。

## 文件操作参考

- Event Graph、Function Graph、Chart、Mind、Doc 是独立文件类型；参考 `fileResourceHandlers` 按类型分派创建、重命名、复制、删除和保存。Database 操作保留自己的 owner。
- 菜单、侧栏和快捷键复用同一操作入口。资源 ID 不透明，不能从路径猜测类型；编辑版本、资源修订与面板身份不合并成一个键。
- 重命名表单、删除确认及保存捕获原项目身份；等待确认或刷新后重验身份，避免把旧操作提交给后继项目。
- Graph 文档、历史和保存身份属于 Rust；参考 `ResourceStore` 只原子发布会话、实体、结果摘要及资源标记，Results 另行管理数据持有关系。
- Mind 与 Doc 使用各自类型化快照和命令。共用队列与发布机制不意味着混合树编辑和 Markdown 编辑契约。

## 未提交输入参考

- `documentInputs` 按项目、路径及字段/会话身份保留输入，`useFileTextInput` 绑定视图；缓冲不是第二份文档 authority。
- 保存、放弃、重命名和释放遍历同一缓冲表。一次编辑完成只结算捕获的 generation，保留后来的输入。
- 关闭或切换 Details 不丢失尚未提交的 Mind 主题文字；最后一个文件面板关闭、显式删除或项目替换才按所属生命周期释放。
- 重命名迁移原输入身份，只有匹配的基线版本才能前进；冲突不能静默合并。同步通知中被替换的缓冲不归旧操作清理。
- 外部文件消失时保留 dirty 内容与未提交文字，但不得凭编辑状态恢复资源索引成员。显式放弃可释放这些内容。
- Save/Discard/Cancel、全部保存和项目关闭先等待所属编辑队列，再读取 dirty 状态。失败保留输入，不隐式重试。

[文档编辑器参考](../../../modules/document-editor/README.md)保留 Mind/Doc 的交互对照；此处不声明原生移植或人工验收完成。
