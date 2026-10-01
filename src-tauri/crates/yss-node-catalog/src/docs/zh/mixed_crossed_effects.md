# 交叉随机效应模型

连接至少两个分组列，例如受试者与题目。每个分组因素贡献独立随机截距，观测可交叉属于不同因素的组。协方差分量须可识别，重复分组划分会被拒绝。

连接对齐的 response、predictors 与 groups。数值固定自变量可选，分类自变量须显式编码。分组标签支持数值、文本、标识符、分类、二元或有序数列，保留原始标签。固定效应设计须满秩且观测数大于系数数。缺失和非有限值会被拒绝，不隐式删除行。每个分组列须有至少两个实际组，协方差分量须可识别。输入须具有可证明的共同行域，或均为等长物化列。

参数：constant=true（固定截距），max_iterations=500（正整数），tolerance=1e-7（1e-12–0.01）。未收敛时执行失败。

## 方法

通过 mixed_estimation 选择 Gaussian 混合效应模型的 REML（默认）或 ML：

$$
y=X\beta+Zb+\varepsilon,\quad b\sim N(0,G),\quad \varepsilon\sim N(0,\sigma^2I),\quad V=ZGZ^T+\sigma^2I.
$$

随机项使用独立方差分量。固定效应在估计方差下通过 GLS 拟合，协方差为 $(X^TV^{-1}X)^{-1}$；随机效应为 BLUP：$GZ^TV^{-1}(y-X\hat\beta)$。比较不同固定效应设计时应使用 ML；REML 的 log_likelihood 是限制对数似然，不提供 AIC。近零方差分量用 boundary 标记。系数推断使用近似正态 Wald 统计量，不包含 Satterthwaite/Kenward–Roger 校正，不对各组随机效应生成显著性检验。

## 输出

唯一结构化 result 可在 Inspect 查看。包含 coefficients、coefficient_covariance、observations、group_counts、iterations、scale、适用的似然/AIC、variance_components 和 random_effects。固定系数采用 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$ 的双侧检验，$z=\hat\beta_j/SE_j$，参考标准正态分布并提供 95% 区间；未定义统计量为 null。

group_labels 按首次出现顺序保留原标签。random_effects 的 grouping、level 从 1 开始；term=0 表示随机截距。固定系数依次为 intercept（constant=true 时）、x1、x2 等。数组保持输入行序：fixed_fitted 将随机效应设为零（对 GLMM 而言不是积分后的总体均值），fitted_values 包含估计的组效应，residuals=response−fitted_values。

[方法参考](https://lme4.github.io/lme4/reference/lmer.html)
