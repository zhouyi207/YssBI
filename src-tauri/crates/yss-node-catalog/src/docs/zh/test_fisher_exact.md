# Fisher 精确检验

对两个二分类变量执行双侧条件精确独立性检验。

## 输入与参数

`row`、`column` 为逐行对应、等长、非空的分类序列，每个变量必须恰有 2 个实际类别。支持数值、分类、有序或二元语义；关系数列须共享行域。空值会被拒绝。无配置参数，当前只提供双侧检验。

## 假设与统计量

$H_0$：两变量独立（总体比值比为 1）；$H_1$：两变量有关联。将观测表记为 $\begin{pmatrix}a&b\\c&d\end{pmatrix}$，令 $r_1=a+b$、$r_2=c+d$、$c_1=a+c$、$N=r_1+r_2$。给定边际合计时：

$$
P(A=x)=\frac{\binom{r_1}{x}\binom{r_2}{c_1-x}}{\binom N{c_1}}.
$$

双侧 p 值累加所有概率不大于观测表概率的合法表。它不使用卡方近似或参考自由度；观测仍须独立。

## 输出与判读

`result`、`report` 返回同一报告，`p_value` 是双侧条件精确 p 值。`details` 中的 `a`、`b`、`c`、`d` 给出按类别编码顺序形成的表，`sample_sizes` 为 `[N]`。`estimate`、`standard_error` 为空，自由度为空。

当前 `statistic_name` 标为 `odds_ratio`，但 `statistic` 实际存放 $ad-bc$，不能按比值比解释。应依据 `p_value` 判读独立性，$p<\alpha$ 时拒绝 $H_0$。
