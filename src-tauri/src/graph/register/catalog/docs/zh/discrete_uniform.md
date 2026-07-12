# Discrete Uniform

离散均匀分布：在整数区间 $[\mathrm{Low}, \mathrm{High}]$ 上每个值等概率出现：

$$
P(X=k)=\frac{1}{\mathrm{High}-\mathrm{Low}+1},\quad k=\mathrm{Low},\ldots,\mathrm{High}
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Low** | 下界（含） |
| **High** | 上界（含），需满足 $\mathrm{Low} \le \mathrm{High}$ |
| **N** | 样本量 |

## 用法

设置 **Low**、**High** 与 **N** 后执行图。**Samples** 输出 `DataSeries<Int64>`。适用于公平随机整数、骰子类模拟及离散均匀抽样基准。
