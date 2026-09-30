# Breusch–Pagan / Koenker 异方差检验

连接 OLS 或 WLS 的 `model`，检查误差方差是否随所选变量变化。复用拟合样本；不支持 GLS。

| 参数      | 默认值  | 含义                                             |
| --------- | ------- | ------------------------------------------------ |
| `rhs`     | `false` | 以截距和拟合值作为辅助变量；开启后使用原设计矩阵 |
| `koenker` | `false` | 原始 BP；开启后使用 Koenker 学生化形式           |

## 原始 Breusch–Pagan

- **原假设 $H_0$：** 同方差，辅助方差方程的非常数系数全为零。
- **备择假设 $H_1$：** 至少一个系数非零，方差随所选变量变化。

将标准化平方残差 $g_i=u_i^2/\hat\sigma^2$ 对所选辅助变量回归，其中 $\hat\sigma^2=\sum_i u_i^2/n$：

$$
LM_{\mathrm{BP}}=\tfrac12\mathrm{ESS}_{\mathrm{aux}}
\overset{H_0}{\approx}\chi_q^2.
$$

$\mathrm{ESS}_{\mathrm{aux}}$ 为辅助回归的解释平方和。原始 BP 的标定依赖正态误差。

## Koenker 学生化形式

- **原假设 $H_0$：** 同方差，辅助方差方程的非常数系数全为零。
- **备择假设 $H_1$：** 至少一个系数非零，存在所选变量可解释的异方差。

使用同一辅助回归的决定系数：

$$
LM_{\mathrm K}=nR_{\mathrm{aux}}^2
\overset{H_0}{\approx}\chi_q^2.
$$

Koenker 放宽正态性要求，不自动处理序列相关或聚类。$n$ 为拟合样本数，默认 $q=1$；`rhs=true` 时 $q$ 为原设计列数减 1，原模型应含截距。

WLS 保留原权重 $w_i$：BP 对 $u_i^2/(\sum_iw_i u_i^2/n)-1$ 作加权辅助回归，以 $\tfrac12\sum_iw_i\hat f_i^2$ 为统计量（$\hat f_i$ 为该回归拟合值）；Koenker 使用加权辅助 $R^2$。

## 输出与判读

`result` 为结构化结果，`test=breusch_pagan`，内层 `result` 含 `lm_stat`、`df`、`p_value`。每次仅运行所选变体，p 值取对应卡方右尾；$p<\alpha$ 拒绝同方差。

默认至少 4 个观测，RHS 要求样本数至少为设计列数加 2。残差方差为零或辅助设计奇异时无法计算。

方法细节：[BP / Koenker](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_breuschpagan.html)。
