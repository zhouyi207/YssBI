# Cook 距离

连接 OLS/WLS 的 **model**。节点使用其拟合残差、设计和精度权重，衡量删除单个观测对拟合的影响；不重新拟合原模型，无配置参数。要求 $n>k$ 且设计满秩，当前不支持 GLS。

令 $u_i=\sqrt{w_i}(y_i-\hat y_i)$，OLS 中 $w_i=1$；$s^2=\sum u_i^2/(n-k)$，$h_i$ 为加权帽子矩阵对角元，$k$ 为包含截距的系数数目：

$$
D_i=\frac{u_i^2}{k s^2}\frac{h_i}{(1-h_i)^2}.
$$

**result** 包含样本数、参数数、残差摘要、最大杠杆值及 maximum_cooks_distance。**observations** 提供每个拟合观测的 observation（从 1 开始）、fitted、residual、weighted_residual、leverage、standardized_residual、studentized_residual 和 cooks_distance，可分页、选列并继续绘图。

零残差方差或单位杠杆等情况下未定义的诊断为 null。Cook 距离较大表示影响较强，但其本身不是异常值显著性检验；节点不输出 p 值或按固定阈值自动删除观测。WLS 结果对应加权后的回归几何。
