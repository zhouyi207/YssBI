# Breusch–Pagan / Koenker 异方差检验

检查回归误差方差是否随指定变量系统变化。连接 OLS 或 WLS 的 `model`，使用其拟合样本、残差 $u_i$、拟合值 $\hat y_i$ 和设计矩阵 $X$；不重新拟合原模型。当前不支持 GLS。

## 参数与辅助回归

| 参数      | 默认值  | 含义                                                   |
| --------- | ------- | ------------------------------------------------------ |
| `rhs`     | `false` | 使用 $Z=[\mathbf1,\hat y]$；开启后使用原设计矩阵 $Z=X$ |
| `koenker` | `false` | 原始 BP；开启后采用 Koenker 学生化形式                 |

以下标准解释要求原模型含截距。开启 `rhs` 不会另行补截距；当前自由度固定取 $q=\operatorname{cols}(Z)-1$，无截距模型不能直接套用标准 BP 的推断。

## 原始 Breusch–Pagan

**原假设 $H_0$：** $\operatorname{Var}(u_i\mid Z_i)=\sigma^2$，即同方差，辅助方差方程中所有非常数项系数为零。

**备择假设 $H_1$：** 方差随 $Z$ 中至少一个非常数项变化。

对 OLS 定义

$$
\hat\sigma^2=\frac1n\sum_i u_i^2,\quad
g_i=\frac{u_i^2}{\hat\sigma^2},\quad
\hat g=Z(Z'Z)^{-1}Z'g.
$$

令 $\mathrm{TSS}=\sum_i(g_i-1)^2$、$\mathrm{RSS}=\sum_i(g_i-\hat g_i)^2$，则

$$
LM_{\mathrm{BP}}=\tfrac12(\mathrm{TSS}-\mathrm{RSS})
\overset{H_0}{\approx}\chi_q^2,\qquad
p=1-F_{\chi_q^2}(LM_{\mathrm{BP}}).
$$

$n$ 是拟合样本数；默认 $q=1$，RHS 模式为设计列数减 1。原始 BP 的标定依赖正态误差及独立观测等条件。

## Koenker 学生化形式

**原假设 $H_0$：** 同方差，所选方差方程的非常数系数全为零。

**备择假设 $H_1$：** 至少一个非常数系数不为零，存在所选变量能解释的异方差。

使用相同的 $g$ 和 $Z$，以辅助回归的决定系数代替原始缩放：

$$
R_{\mathrm{aux}}^2=1-\frac{\mathrm{RSS}}{\mathrm{TSS}},\qquad
LM_{\mathrm K}=nR_{\mathrm{aux}}^2
\overset{H_0}{\approx}\chi_q^2,\qquad
p=1-F_{\chi_q^2}(LM_{\mathrm K}).
$$

该形式放宽误差正态性的要求，仍需要独立性和适当矩条件；不是对序列相关或聚类自动稳健的检验。两种方法分别说明于 [statsmodels BP/Koenker 文档](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_breuschpagan.html)。

## WLS 的计算口径

保留原模型的正精度权重 $w_i$，令 $W=\operatorname{diag}(w_i)$、$\hat\sigma_w^2=\sum_i w_i u_i^2/n$。BP 对 $f_i=u_i^2/\hat\sigma_w^2-1$ 作加权辅助回归：

$$
\hat f=Z(Z'WZ)^{-1}Z'Wf,\qquad
LM_{\mathrm{BP},w}=\tfrac12\sum_iw_i\hat f_i^2.
$$

Koenker 对 $g_i=u_i^2/\hat\sigma_w^2$ 作同样的加权回归，并计算

$$
\bar g_w=\frac{\sum_iw_i g_i}{\sum_iw_i},\quad
R_w^2=1-\frac{\sum_iw_i(g_i-\hat g_i)^2}
{\sum_iw_i(g_i-\bar g_w)^2},\qquad LM_{\mathrm K,w}=nR_w^2.
$$

自由度和右尾 p 值规则沿用上文。这里描述的是节点当前的加权口径，权重不作为重复样本数；解释时须结合原模型的精度权重及方差设定。

## 输出、判读与限制

`result` 与 `report` 相同，结构为 `test=breusch_pagan`，内层 `result` 含 `lm_stat`、`df`、`p_value`。节点每次仅运行所选变体，不同时输出 BP 和 Koenker。

预设 $\alpha$ 后，$p<\alpha$ 表示拒绝同方差；否则只是未发现所选方差方程下的充分证据。例如，同一模型可用默认设置检查方差随拟合均值变化，再用 `rhs=true` 检查各解释变量；两个结果回答的方差设定问题不同。拒绝不指出异方差的具体函数形式，不拒绝也不排除未纳入 $Z$ 的模式。

默认模式至少需要 4 个观测，RHS 模式要求 $n\geq k+2$（$k$ 为设计列数）。残差方差为零、辅助矩阵奇异或非有限计算会失败；常量拟合值可能导致默认辅助矩阵奇异。平方残差没有变动时，Koenker 的辅助 $R^2$ 按 0 处理，不能据此宣称已证明同方差。上游选择稳健协方差不会改变这里的 LM 公式。
