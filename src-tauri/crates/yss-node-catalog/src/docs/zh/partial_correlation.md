# 偏相关

连接对齐的 Numeric **X**、**Y** 以及一个或多个 **Control variable**，在详细面板添加控制变量，最多支持 16 列。所有列必须对应同一样本；有 $q$ 个控制变量时要求 $n>q+2$。

分别将 X、Y 对截距和控制变量回归，再计算残差间的 Pearson 相关 $r_{XY\cdot C}$。常量控制列、线性相关的控制列或零残差变异会报错，不会自动删列。

原假设为 $H_0:\rho_{XY\cdot C}=0$，统计量为

$$t=r_{XY\cdot C}\sqrt{\frac{n-q-2}{1-r_{XY\cdot C}^2}},\quad df=n-q-2.$$

备择默认 `two_sided`，可选 `greater`、`less`。置信水平默认 `0.95`，双侧 Fisher 近似区间采用标准误 $1/\sqrt{n-q-3}$；$n\leq q+3$ 时区间为 null。推断要求独立观测和适当的线性、正态残差模型。

`result` 含相关系数、`control_variables`、样本数、推断和置信区间。本节点计算 Pearson 偏相关。

参考：[R 相关检验](https://search.r-project.org/R/refmans/stats/html/cor.test.html)。
