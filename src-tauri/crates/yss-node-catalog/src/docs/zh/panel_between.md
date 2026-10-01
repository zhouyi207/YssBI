# 组间估计 Between

连接数值 response、至少一个 predictors、entity 编号及 time。输入必须对齐、有限、无缺失，且实体与时间组合唯一。

**参数**：constant 默认 true；effects 默认 entity，也可选择 time。仅使用 nonrobust 协方差。

entity 按实体分别取均值并拟合 $\bar y_i=\alpha+\bar x_i'\beta+\bar\varepsilon_i$；time 按时期取跨实体均值。每组在组均值回归中等权，不按组观测数加权。省略截距时拟合无常数模型。

唯一输出 **model** 可连接“面板模型汇总”。估计样本每行对应一个组均值，sourceRows 保留该组全部原始行号；它不是逐观测拟合值。系数的双侧 t 检验针对 $H_0:\beta_j=0$，残差自由度为组数减设计秩。

需要足够的组数和组间变化。Between 只利用组间信息，不能据此解释实体内部的变化效应。
