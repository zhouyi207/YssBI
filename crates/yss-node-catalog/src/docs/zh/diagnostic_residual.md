# 残差分析

连接 OLS 或 WLS 的 **model**；沿用拟合样本、设计列和原精度权重，不重新拟合。无配置参数。当前不接受 GLS，要求满秩设计且 $n>k$，其中 $k$ 包含截距。

原残差为 $e_i=y_i-\hat y_i$，加权残差为 $u_i=\sqrt{w_i}e_i$（OLS 取 $w_i=1$），$s^2=\sum_i u_i^2/(n-k)$。杠杆值来自加权设计的帽子矩阵对角元 $h_i$。内部标准化残差为

$$
r_i=\frac{u_i}{s\sqrt{1-h_i}}.
$$

外部学生化残差使用删除该观测后的方差 $s_{(-i)}^2=[\sum u^2-u_i^2/(1-h_i)]/(n-k-1)$ 替代 $s^2$。Cook 距离为 $r_i^2h_i/[k(1-h_i)]$。

**result** 提供原残差均值、样本标准差、范围、加权残差平方和、残差标准误、最大杠杆值及最大 Cook 距离。

**observations** 是可分页、可继续连接的数据表：observation（从 1 开始）、fitted、residual、weighted_residual、leverage、standardized_residual、studentized_residual、cooks_distance。行序与拟合样本一致。零残差方差、单位杠杆或删除后自由度/方差不足时，对应未定义值为 null。该节点不自动执行正态性或异方差检验。
