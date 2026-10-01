# Score / LM 检验

连接同一批观测的 **restricted** 与 **full** 拟合结果。支持相同方法的 OLS/WLS、Logit、Probit，不支持 GLS。无参数；复用模型，不重新拟合。要求相同观测顺序、响应数值、相同相对 WLS 权重和真正嵌套的设计，$q=k_f-k_r>0$ 且 $n>k_f$。

原假设 $H_0$ 为新增系数约束成立。Score 在受限估计处计算：

$$
LM=U^\mathsf{T}I^{-1}U\ \mathrel{\dot\sim}\ \chi_q^2.
$$

$U$ 是完整模型在受限估计处的得分，$I$ 是期望信息矩阵，包含原有系数以调整干扰参数。Logit/Probit 由 Bernoulli 概率和相应链接导数形成 $U,I$。OLS/WLS 等价于 $n(1-SSR_f/SSR_r)$，$SSR$ 为相同相对权重下的残差平方和。

**result** 输出两模型的信息准则摘要、restrictions 和 score（statistic、degrees_of_freedom、p_value）。likelihood_ratio/f_test 为 null。

小 p 值支持新增系数不全为零。参考分布依赖模型的常规似然假设，本节点不提供异方差稳健 Score 或其他模型族的专用 LM 检验。Logit/Probit 的受限概率须严格介于 0 与 1。
