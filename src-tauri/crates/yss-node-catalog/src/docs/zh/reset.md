# Ramsey RESET 设定检验

连接 OLS 或 WLS 的 `model`，检查新增非线性幂次是否改善原均值方程。复用拟合样本和 WLS 权重；不支持 GLS。

| 参数                | 扩展项                                                          |
| ------------------- | --------------------------------------------------------------- |
| `rhs=false`（默认） | 将拟合值归一化至 0–1，加入二、三、四次幂，共 $q=3$ 项           |
| `rhs=true`          | 将各非常量、非 0/1 解释变量归一化至 0–1，分别加入二、三、四次幂 |

## 假设与公式

- **原假设 $H_0$：** 所选新增项的系数联合为零。
- **备择假设 $H_1$：** 至少一个新增系数非零。

$$
F=\frac{(\mathrm{RSS}_R-\mathrm{RSS}_U)/q}
{\mathrm{RSS}_U/(n-k-q)},\qquad
F\overset{H_0}{\sim}F_{q,n-k-q}.
$$

$\mathrm{RSS}_R$、$\mathrm{RSS}_U$ 分别为原方程和扩展方程的残差平方和；$n$ 为样本数、$k$ 为原设计列数、$q$ 为新增项数。WLS 的两个平方和均使用原权重。

要求 $n>k+q$ 且扩展设计满秩。RHS 模式排除常量和 0/1 变量，不生成交互项；没有合格变量时失败。

## 输出与判读

`result` 为结构化结果，`test=reset`，内层 `result` 含 `f_stat`、`df1=q`、`df2=n-k-q`、`p_value`。

p 值取 F 分布右尾；$p<\alpha$ 表示所选幂次联合显著，提示检查均值方程设定，但不直接指出应加入哪个变量。上游稳健协方差设置不会将本节点改为稳健 RESET。

方法细节：[Ramsey RESET](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.linear_reset.html)。
