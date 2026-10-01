# 嵌套模型比较

连接受限模型 **restricted** 和完整模型 **full**。支持同方法的 OLS/WLS、Logit 或 Probit，无参数、不重新拟合。完整模型必须增加系数，并且 $n>k_f$。检查逐行响应、相对 WLS 权重以及受限设计列空间的包含关系；上游负责提供相同观测与行序。不支持 GLS。

$H_0$ 为受限模型的新增系数约束成立，备择为至少一个约束不成立。令 $q=k_f-k_r$：

$$
LR=2(\ell_f-\ell_r)\ \mathrel{\dot\sim}\ \chi_q^2,\qquad
LM=U^\mathsf{T}I^{-1}U\ \mathrel{\dot\sim}\ \chi_q^2.
$$

Score 使用受限估计处的期望信息。对 OLS/WLS，另提供

$$
F=\frac{(SSR_r-SSR_f)/q}{SSR_f/(n-k_f)}\sim F_{q,n-k_f}.
$$

$SSR$ 使用相同相对精度权重。F 检验要求独立正态误差和正确权重；LR/Score 使用常规似然渐近参考分布。

**result** 同时提供两模型的 family、observations、parameters、log_likelihood、aic、bic，以及 restrictions、likelihood_ratio、score、f_test。二元模型没有本节点定义的 F 检验，其 f_test 为 null。高斯信息准则额外计入一个误差方差参数；检验自由度仅取回归系数数目之差。非嵌套模型不会被强行比较。
