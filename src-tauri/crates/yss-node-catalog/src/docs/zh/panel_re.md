# 随机效应 RE

输入为对齐、有限且无缺失的数值 response、至少一个 predictors、entity 编号和 time。实体与时间组合必须唯一，节点自动排序；文本实体需先编码。

**参数**：constant 默认 true；effects 默认 entity，可选 time、two_way；covariance 默认 nonrobust，可选 HC0–HC3 或按实体聚类的 cluster。此入口使用可行广义最小二乘（FGLS）；极大似然估计可在通用“拟合面板模型”中选择。

实体模型为 $y_{it}=\alpha+x_{it}'\beta+u_i+\varepsilon_{it}$，要求随机效应与解释变量不相关。FGLS 估计方差分量并准去均值；time 和 two_way 对应时间或双向随机效应。

唯一输出 **model** 可连接“面板模型汇总”和估计尺度预测，保留系数、协方差、方差分量、准去均值样本及源行分组。系数检验采用 $z_j=\hat\beta_j/SE(\hat\beta_j)$ 的标准正态双侧检验，原假设为系数等于零；估计器统计含整体 Wald 检验，系数区间为 95% 正态区间。

缺少必要的组内/组间变化、设计不可识别或估计失败会报错。输出样本属于准去均值尺度，不含对新实体随机效应的预测。
