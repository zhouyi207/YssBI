# While Loop

在 **Condition** 为真时重复执行 **Body**。**MaxIterations** 限制最大轮次以防死循环。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **In** | Exec 输入 | 启动或重新进入；**Body** 末端接回此处 |
| **Condition** | 输入（可选） | `Boolean`；未连接视为 false |
| **MaxIterations** | 输入（可选） | `Int64` 安全上限；默认 1000 |
| **Body** | Exec 输出 | 条件为真且未达上限时执行 |
| **Completed** | Exec 输出 | 条件为假或达到上限时执行 |

## 连线

**Body** 链末端接回 **In**。每轮 **Body** 结束后重新求值 **Condition**。

## 限制

超过 **MaxIterations** 时，即使 **Condition** 仍为真，也会从 **Completed** 退出。
