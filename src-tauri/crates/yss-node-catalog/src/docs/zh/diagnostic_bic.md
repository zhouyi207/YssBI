# BIC 贝叶斯信息准则

连接已拟合 OLS/WLS、Logit 或 Probit 的 **model**，无配置参数，不重新拟合。当前不支持 GLS；高斯模型要求正残差平方和，二元模型要求收敛。

$$
BIC=-2\ell+K\log n.
$$

$\ell$ 为最大化对数似然，$n$ 为拟合观测数。OLS/WLS 的 $K$ 为回归系数数（含截距）加一个误差方差参数；WLS 高斯似然保留精度权重的行列式项。Logit/Probit 使用已保存的 Bernoulli 似然，$K$ 为系数数。

**result** 输出 criterion（bic）、value、family、observations、parameters、log_likelihood。高斯模型计入误差尺度，与忽略尺度的计数口径相比 BIC 多 $\log n$。该计数与 AIC 节点一致。

同一响应、同一批观测及同一似然口径下，较小的 BIC 更优。该节点不检验模型嵌套性或独立核实两次拟合的样本身份，不输出 p 值。
