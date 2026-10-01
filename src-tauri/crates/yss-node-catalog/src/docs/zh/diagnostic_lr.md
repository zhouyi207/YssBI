# 似然比检验（LR）

将受限模型接到 **restricted**，增加参数后的完整模型接到 **full**。支持 OLS/WLS、Logit、Probit，同一对模型必须采用相同估计方法。无配置参数，不重新拟合。

上游必须使用相同观测和行序。节点逐行核对重构的响应数值、WLS 权重（允许整体比例缩放），并验证受限设计列空间包含于完整设计。完整模型须增加至少一个系数且保留正残差自由度；二元拟合须收敛。不支持 GLS。

原假设 $H_0$：受限模型的新增系数约束成立；备择为至少一个约束不成立。

$$
LR=2(\ell_f-\ell_r)\ \mathrel{\dot\sim}\ \chi_q^2,\qquad q=k_f-k_r.
$$

下标 $f/r$ 为完整/受限模型，$k$ 为系数数。高斯模型采用剖面似然；二元模型采用 Bernoulli 似然。常规参考分布要求模型设定正确、参数处于内部且可识别；线性模型要求独立正态误差和正确的方差/权重设定。

**result** 包含 restricted/full 信息准则摘要、restrictions 和 likelihood_ratio（statistic、degrees_of_freedom、p_value；denominator_df 为 null）。未请求的 score/f_test 为 null。较小 p 值支持完整模型的新增项，但不直接判断预测表现。
