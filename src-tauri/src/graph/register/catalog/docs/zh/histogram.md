# Histogram

对数值 **Values**（`Float64` 或 `Int64` `DataSeries`）绘制直方图。

分箱数使用 Sturges 规则：$k = \lceil \log_2 n + 1 \rceil$（上限 100）。null 与非有限值在绘图前剔除。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **In** | 执行输入 | 控制流入口 |
| **Values** | 输入 | 数值 `DataSeries` |
| **Out** | 执行输出 | 控制流出口 |

## 用法

执行图后自动打开 **Plot** 窗口展示直方图；同一运行中多次执行会刷新 Plot 内容。须至少 1 个有效数值。
