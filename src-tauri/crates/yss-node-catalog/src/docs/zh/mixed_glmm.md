# 广义线性混合模型 GLMM

连接一个分组列。可选二项 logit（默认）、Poisson log 或负二项 NB2 log。当前估计 Gaussian 随机截距，不提供随机斜率或交叉分组。二项响应须为 0/1，计数须为非负整数。

连接对齐的 response、predictors 与 groups。数值固定自变量可选，分类自变量须显式编码。分组标签支持数值、文本、标识符、分类、二元或有序数列，保留原始标签。固定效应设计须满秩且观测数大于系数数。缺失和非有限值会被拒绝，不隐式删除行。每个分组列须有至少两个实际组，协方差分量须可识别。输入须等长并按当前位置逐项对应，可混合数据库与内存数列。

参数：constant=true（固定截距），max_iterations=500（正整数），tolerance=1e-7（1e-12–0.01）。未收敛时执行失败。

## 方法

采用每组一维 Laplace 近似的最大似然，估计固定系数与随机截距方差：

$$
g(\mu_{gi})=x_{gi}^T\beta+b_g,\quad b_g\sim N(0,\tau^2),\qquad
\ell\approx\sum_g\left[\log p(y_g\mid\hat u_g)-\frac{\hat u_g^2}{2}-\frac12\log H_g\right].
$$

其中 $b_g=\tau u_g$，$\hat u_g$ 为条件众数，$H_g$ 为标准正态坐标下负条件对数后验的曲率。估计采用 Laplace 近似，不提供自适应求积或 PQL。Wald 标准误使用近似似然的观测信息，包含干扰参数的不确定性。随机效应输出条件众数，近零方差以 boundary 标记。组内观测较少或事件稀少时，近似精度可能降低。

## 输出

唯一结构化 result 可在 Inspect 查看。包含 coefficients、coefficient_covariance、observations、group_counts、iterations、scale、适用的似然/AIC、variance_components 和 random_effects。固定系数采用 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$ 的双侧检验，$z=\hat\beta_j/SE_j$，参考标准正态分布并提供 95% 区间；未定义统计量为 null。

group_labels 按首次出现顺序保留原标签。random_effects 的 grouping、level 从 1 开始；term=0 表示随机截距。固定系数依次为 intercept（constant=true 时）、x1、x2 等。数组保持输入行序：fixed_fitted 将随机效应设为零（对 GLMM 而言不是积分后的总体均值），fitted_values 包含估计的组效应，residuals=response−fitted_values。

[方法参考](https://lme4.github.io/lme4/reference/glmer.html)
