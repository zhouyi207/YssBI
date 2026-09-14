1. 同一条日志只脱敏一次，减少字段复制。

    目前 collector (/D:/Desktop/YssBI/src-tauri/crates/tauri-plugin-tracing/src/collector/layer.rs:55) 脱敏后，经过字段
    复制，又在 dispatcher 入队时 (/D:/Desktop/YssBI/src-tauri/crates/tauri-plugin-tracing/src/stream/dispatcher.rs:281)
    重复脱敏。

    可以统一由现有后台 dispatcher 完成脱敏、记录规范化和编码，之后再分发给各个输出。计算线程只负责受限的字段捕获、发生
    时间记录和非阻塞入队。所有控制台、SQLite、Channel 输出都使用处理后的记录。

    这里有个限制：tracing::Event 包含借用数据，不能直接扔给后台线程；必要的 Debug/Display 格式化仍需要在调用处完成。

2. 让长度限制真正减少格式化工作。

    BoundedFormatter (/D:/Desktop/YssBI/src-tauri/crates/tauri-plugin-tracing/src/collector/layer.rs:175) 截断后仍返回
    Ok(())，只是停止保存字符串，大对象的格式化可能继续遍历。

    应在达到限额时返回格式化错误，让遵循错误传播的格式化实现提前结束。同时，把字段数量、字符串长度和总字节限制提前到捕
    获阶段；明确会被屏蔽的字段，直接跳过值的格式化。

3. 分别控制 SQLite、控制台和 Channel 的输出频率。

    SQLite 已经使用后台线程、WAL 和批量事务，可以继续复用。当前落盘和实时推送共用 16ms/128 条的批次条件，建议分别配置：

    输出       可以先尝试的策略
    ━━━━━━━━━  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    SQLite     50–100ms 或达到条数、字节阈值后提交
    ─────────  ─────────────────────────────────────────
    Channel    合并已提交记录，每 100–200ms 推送
    ─────────  ─────────────────────────────────────────
    控制台     独立级别过滤，默认减少 DEBUG/TRACE 输出

    这些数值是调优起点，需要实测。继续保持“提交成功后再向前端展示”的顺序。

4. 前端按刷新周期生成快照，减少隐藏面板的工作。

    当前 logBuffer (/D:/Desktop/YssBI/src/features/application/log/logBuffer.ts:101) 每个批次都会复制数组并通知 React。
    建议先积累增量，刷新时统一生成不可变快照，再执行过滤和自动滚动。

    日志面板已经使用虚拟列表，下一步重点是：隐藏时暂停渲染通知，显示后统一刷新；避免每个 Channel 批次都触发全量过滤和滚
    动。

5. 把默认采集级别作为单独的策略选择。

    如果允许减少日常保存量，可以默认 INFO，排查时按模块开启 DEBUG/TRACE。面板隐藏 DEBUG 只减少显示，无法消除后端采集成
    本。

    如果仍需要保存全量 TRACE，就保持采集范围，先完成前四项；完整日志仍然存在必要的格式化和存储成本。




• 已实现 1、2、5：

  - 前后端默认采集 INFO/WARN/ERROR，提前过滤 DEBUG/TRACE。
  - 脱敏、时间格式化和记录整理移到后台，减少计算线程上的复制与重复处理。
  - 提前限制字段数量和字节数，长日志达到上限后停止格式化。

  验证通过：Rust 21 个测试、前端 5 个测试、TypeScript、Clippy 和前端静态检查。

  重启应用后生效。之前设置的 RUST_LOG=warn 会覆盖默认值，请改成 info 或移除。实际图计算耗时尚未复测。