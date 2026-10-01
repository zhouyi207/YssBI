# 比例风险 PH 假设检验

连接同一批观测的 **time**（正随访时间）、**event**（1/true 表示事件，0/false 表示右删失）及一个或多个数值 **predictors**。不隐式删行。节点先拟合静态、无分层的 Cox 模型，再计算时间交互的有效 Score 检验。

**并列事件**默认 efron，可选 breslow，拟合与检验使用同一偏似然。**时间变换**默认 rank（全部随访时间的平均秩），可选 log、identity。**最大迭代次数**默认 500，**收敛容差**默认 $10^{-7}$，允许 $10^{-12}$ 至 $0.01$。

考察 $\beta(t)=\beta+\gamma g(t)$。原假设 $H_0:\gamma=0$，即协变量效应不随所选时间函数变化；备择为至少一个时间交互非零。消去原系数的干扰信息后，

$$
U_e=U_\gamma-I_{\gamma\beta}I_{\beta\beta}^{-1}U_\beta,\qquad
I_e=I_{\gamma\gamma}-I_{\gamma\beta}I_{\beta\beta}^{-1}I_{\beta\gamma}.
$$

全局统计量为 $U_e^\mathsf{T}I_e^{-1}U_e\mathrel{\dot\sim}\chi_p^2$，$p$ 为协变量数。逐变量检验为 $U_{e,j}^2/I_{e,jj}\mathrel{\dot\sim}\chi_1^2$；其他时间交互固定为零。

**result** 提供样本数、事件数、并列处理、时间变换、拟合对数似然与系数，以及 terms 和 global 的统计量、自由度、p 值。至少需要两个不同事件时间、收敛的 Cox 拟合及可识别的时间交互信息。小 p 值提示所检验的 PH 假设不成立；未拒绝不代表所有时间变化都已排除。[Score 检验说明](https://stat.ethz.ch/R-manual/R-devel/library/survival/html/cox.zph.html)
