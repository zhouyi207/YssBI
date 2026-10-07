# 系数线性约束 t / Wald 检验

连接线性 `model`，在 `hypothesis` 中填写系数约束，默认 `x1 = 0`，最多 4096 字符。系数名以模型报告为准；复用已有系数、协方差和残差自由度。

## 单约束 t 检验

支持 `x1 = 0`、`x1 = x2`、`2*x1 + x2 = 1` 等线性表达式。

- **原假设 $H_0$：** $R\beta=r$。
- **备择假设 $H_1$：** $R\beta\ne r$。

$$
t=\frac{R\hat\beta-r}{\sqrt{RVR'}},\qquad
t\overset{H_0}{\sim}t_\nu,\qquad
p=2[1-F_{t_\nu}(|t|)].
$$

$\hat\beta$、$V$、$\nu$ 分别为模型系数、协方差和残差自由度，$R\beta=r$ 为输入约束。单条不等式指定单侧备择：

| 输入                  | 原假设         | 备择假设    | p 值             |
| --------------------- | -------------- | ----------- | ---------------- |
| `x1 > 0` 或 `x1 >= 0` | $\beta_1\leq0$ | $\beta_1>0$ | $1-F_{t_\nu}(t)$ |
| `x1 < 0` 或 `x1 <= 0` | $\beta_1\geq0$ | $\beta_1<0$ | $F_{t_\nu}(t)$   |

小于方向的输出统计量会取反，需结合 `h0_form`、`h1_form` 和 `alternative` 读取。

## 多约束 Wald F 检验

用逗号分隔等式，如 `x1 = 0, x2 = 0`。

- **原假设 $H_0$：** $R\beta=r$ 中所有等式成立。
- **备择假设 $H_1$：** 至少一个等式不成立。

$$
F=\frac{(R\hat\beta-r)'(RVR')^{-1}(R\hat\beta-r)}q,
\qquad F\overset{H_0}{\sim}F_{q,\nu}.
$$

$q$ 为约束数，p 值取 F 分布右尾。多约束仅支持等式，冗余或非线性约束无法计算。

## 输出与判读

`result` 为结构化结果，含 `test_type`、`h0_form`、`h1_form`、`alternative`、`stat`、`df1`、`df2`、`p_value` 和 `r_beta_minus_r`。Wald 的 `stat` 是 **F 统计量**；多约束时 `r_beta_minus_r` 为占位值 0。

$p<\alpha$ 拒绝对应原假设；联合拒绝只说明至少一个约束不成立。上游稳健或聚类协方差沿用原模型的 $V$ 和 $\nu$，按相应近似解释。

方法细节：[线性 Wald 检验](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.wald_test.html)。
