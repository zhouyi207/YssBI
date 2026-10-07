# AIC 赤池信息准则

连接已拟合 OLS/WLS、Logit 或 Probit 的 **model**。无配置参数，不重新拟合。当前不支持 GLS；高斯模型的残差平方和必须为正，二元模型须已收敛。

$$
AIC=-2\ell+2K.
$$

$\ell$ 为最大化对数似然，$K$ 为估计参数总数。OLS/WLS 使用高斯剖面对数似然，精度权重的对数行列式计入 WLS 似然；$K$ 等于回归系数数（含截距）加一个误差方差参数。Logit/Probit 使用拟合中保留的 Bernoulli 对数似然，$K$ 仅计回归系数。

**result** 输出 criterion（aic）、value、family、observations、parameters、log_likelihood。本节点的高斯参数计数包括误差尺度；与不计尺度的软件默认值比较时，AIC 会相差 2。[参数计数口径](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.info_criteria.html)

在相同响应、样本和似然口径下，较小值更优。AIC 是相对比较指标，不提供 p 值；单独得到数值不意味着任意两个模型可直接比较。
