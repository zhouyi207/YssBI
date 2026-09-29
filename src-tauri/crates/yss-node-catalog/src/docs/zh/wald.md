# 系数线性约束 t / Wald 检验

连接已拟合线性 `model`，通过 `hypothesis` 指定对系数的线性约束。默认 `x1 = 0`，最多 4096 个字符；变量名按模型报告中的实际系数名填写。使用原模型的系数估计、协方差矩阵与残差自由度，不重新拟合。

## 单约束 t 检验

例如 `x1 = 0`、`x1 = x2` 或 `2*x1 + x2 = 1`。令 $\hat\beta$ 为系数列向量、$V$ 为其协方差矩阵，约束写成行向量 $R$ 与标量 $r$。

**原假设 $H_0$：** $R\beta=r$。

**备择假设 $H_1$：** $R\beta\ne r$（单条等式采用双侧检验）。

$$
t=\frac{R\hat\beta-r}{\sqrt{RVR'}},
\qquad t\overset{H_0}{\sim}t_\nu,\qquad
p=2[1-F_{t_\nu}(|t|)].
$$

$\nu$ 为原模型残差自由度。单条不等式则指定备择方向：

| 输入示例              | 原假设         | 备择假设    | p 值             |
| --------------------- | -------------- | ----------- | ---------------- |
| `x1 > 0` 或 `x1 >= 0` | $\beta_1\leq0$ | $\beta_1>0$ | $1-F_{t_\nu}(t)$ |
| `x1 < 0` 或 `x1 <= 0` | $\beta_1\geq0$ | $\beta_1<0$ | $F_{t_\nu}(t)$   |

不等式在等号边界处标定；输入中的方向是欲寻找证据的方向。当前小于方向会内部取反，输出的 `stat` 与 `r_beta_minus_r` 也随之取反，应同时阅读 `h0_form`、`h1_form` 与 `alternative`。

## 多约束 Wald F 检验

用逗号分隔联合等式，例如 `x1 = 0, x2 = 0`。令 $R$ 为 $q\times k$ 约束矩阵，$r$ 为 $q$ 维目标，$q$ 为独立约束条数。

**原假设 $H_0$：** $R\beta=r$ 中所有等式同时成立。

**备择假设 $H_1$：** 至少一个等式不成立。

$$
W=(R\hat\beta-r)'(RVR')^{-1}(R\hat\beta-r),
\qquad F=\frac Wq,
$$

$$
F\overset{H_0}{\sim}F_{q,\nu},\qquad
p=1-F_{F_{q,\nu}}(F).
$$

**节点 `stat` 返回 $F=W/q$，不是 $W$ 或卡方统计量。** 多条约束只支持等式；冗余约束使 $RVR'$ 奇异时失败，不自动减少自由度。Wald 线性约束原理见 [statsmodels wald_test](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.wald_test.html)。

## 输出、条件与示例

`result`、`report` 相同，含 `test_type`（`t` / `wald`）、`h0_form`、`h1_form`、`alternative`、`stat`、`df1`、`df2`、`p_value` 和 `r_beta_minus_r`。t 检验的 `df1=1`、`df2=ν`；Wald 的 `df1=q`、`df2=ν`。多约束时 `r_beta_minus_r` 为占位值 0，不能把它解释为所有约束偏差均为零。

例如在 5% 水平联合检验 `x1 = 0, x2 = 0`，若 $p<0.05$，表示至少一个系数非零，不能得出两个都非零。未拒绝表示证据不足，不是证明系数恰好等于零。

经典同方差正态线性模型提供精确 t/F 标定；若上游使用稳健、聚类或其他协方差，节点沿用该 $V$ 与模型的 $\nu$，推断应按相应近似理解，不重新估计标准误。要求正残差自由度、可计算的约束协方差和有效线性表达式；未知系数、系数相乘等非线性约束、混合/联合不等式或非有限结果会失败。
