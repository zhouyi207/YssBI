# Pearson 相关

将对齐的 Numeric 数列连接到 **X**、**Y**，至少需要三个有限的配对观测，两列均须有变异。缺失值会报错；请在上游共同筛选样本。

$$r=\frac{\sum_i(x_i-\bar x)(y_i-\bar y)}{\sqrt{\sum_i(x_i-\bar x)^2\sum_i(y_i-\bar y)^2}}.$$

备择假设默认 `two_sided`，`greater` 检验正相关，`less` 检验负相关。原假设为 $H_0:\rho=0$。独立二元正态观测下，$t=r\sqrt{(n-2)/(1-r^2)}$ 服从自由度 $n-2$ 的 t 分布。

置信水平默认 `0.95`，须严格在 0 与 1 之间。双侧 Fisher 区间在 $\operatorname{atanh}(r)\pm z_{1-\alpha/2}/\sqrt{n-3}$ 上计算后用 tanh 变回相关系数；只有三个观测时区间为 null。备择方向只影响检验，不改变区间。

唯一结构化 `result` 含 `coefficient`、`observations`、`alternative`、`inference` 和 `confidence_interval`。完全相关时无法有限表示的检验统计量为 null，p 值按极限计算。该指标描述线性相关，不能据此认定两种测量方法一致。

参考：[SciPy Pearson 相关](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.pearsonr.html)。
