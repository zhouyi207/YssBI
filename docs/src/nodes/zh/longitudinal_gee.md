# 广义估计方程 GEE

一个分组列标识相互独立的受试者或簇。默认 Gaussian 恒等链接，可选二项 logit 或 Poisson log。二项响应须为 0/1，计数响应须为非负整数。工作相关结构默认可交换，也可选独立。

连接对齐的 Y、X₁, X₂, … 与 groups。数值固定自变量可选，分类自变量须显式编码。分组标签支持数值、文本、标识符、分类、二元或有序数列，保留原始标签。固定效应设计须满秩且观测数大于系数数。缺失和非有限值会被拒绝，不隐式删除行。须有至少两个实际组。独立相关结构允许每组仅一行；交换型相关结构需要组内观测对。输入须等长并按当前位置逐项对应，可混合数据库与内存数列。

参数：constant=true（固定截距），max_iterations=500（正整数），tolerance=1e-7（1e-12–0.01）。未收敛时执行失败。

## 方法

求解总体平均响应的估计方程。第 $g$ 组的工作协方差为 $V_g=\phi A_g^{1/2}R_gA_g^{1/2}$，$D_g=\partial\mu_g/\partial\beta$，$A_g$ 为方差函数对角阵：

$$
\sum_g D_g^T V_g^{-1}(y_g-\mu_g)=0,\qquad \widehat{\operatorname{Var}}(\hat\beta)=B^{-1}\left(\sum_g U_gU_g^T\right)B^{-1}.
$$

其中 $U_g=D_g^TV_g^{-1}(y_g-\mu_g)$，$B=\sum_gD_g^TV_g^{-1}D_g$。推断使用未经小样本修正的按组三明治协方差及标准正态参考，依赖足够多的独立组。Gaussian 尺度为 Pearson 残差平方和除以 $n-p$，二项/Poisson 尺度固定为 1。可交换相关系数由带参数数目修正的残差矩估计；相关矩阵无效时明确失败。

## 输出

唯一结构化 result 可在 Inspect 查看。包含 coefficients、coefficient_covariance、observations、group_counts、iterations、scale、适用的似然/AIC、variance_components 和 random_effects。固定系数采用 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$ 的双侧检验，$z=\hat\beta_j/SE_j$，参考标准正态分布并提供 95% 区间；未定义统计量为 null。

group_labels 按首次出现顺序保留原标签。固定系数按自变量端口顺序给出：intercept（constant=true 时）、x1、x2 等。数组保持输入行序，fixed_fitted 与 fitted_values 均为总体平均预测，residuals=response−fitted_values。随机效应与方差分量数组为空，似然/AIC 为 null；correlation 与 working_correlation 记录估计的工作相关结构。

[方法参考](https://www.statsmodels.org/stable/gee.html)
