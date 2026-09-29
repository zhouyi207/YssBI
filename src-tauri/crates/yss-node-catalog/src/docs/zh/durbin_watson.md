# Durbin–Watson 残差序列相关诊断

连接按时间顺序排列的残差 `series`，至少 4 个无空值、有限观测。无配置参数，按当前行顺序计算。

## 假设与公式

对于一阶误差相关系数 $\rho$：

- **原假设 $H_0$：** $\rho=0$。
- **备择假设 $H_1$：** $\rho\ne0$；单侧问题可取 $\rho>0$ 或 $\rho<0$。本节点不配置方向，仅计算统计量。

$$
d=\frac{\sum_{t=2}^n(e_t-e_{t-1})^2}{\sum_{t=1}^n e_t^2}.
$$

$e_t$ 为输入残差。$d$ 接近 2 提示无明显一阶相关，低于 2 提示正相关，高于 2 提示负相关。

## 输出与判读

`result`、`report` 相同，仅含 `d`，不返回 p 值或临界值。DW 的零假设分布依赖原回归设计，不能仅凭 `d` 作显著性判定；含滞后因变量的模型不宜直接套用经典 DW 推断，可考虑 Breusch–Godfrey。

全零残差当前返回 `d=2`，属于退化约定，不能解释为通过检验。

方法细节：[Durbin–Watson](https://www.statsmodels.org/stable/generated/statsmodels.stats.stattools.durbin_watson.html)。
