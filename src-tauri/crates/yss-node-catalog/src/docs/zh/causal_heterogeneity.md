# 处理效应异质性检验

检验不同观测组的处理效应是否相同，并可加入具有共同斜率的协变量。

## 输入

连接对齐的数值 `response`、二元或数值 0/1 的 `treatment`、一列分类 `groups`，以及可选数值 `predictors`。
布尔 false/true 分别表示对照／处理。分组接受数值编码、二元、分类、有序、文本或标识符值；原始标签按首次出现顺序保留。至少须有两组，且每组都包含处理与对照观测。缺失值、秩不足及杠杆值为 1 的设计会被拒绝。节点没有方法参数。

## 模型与检验

第一组为参考组。模型包含截距、协变量、处理变量、组虚拟变量以及处理与组别的交互：

$$
Y_i=\alpha+X_i'\beta+\tau D_i+
\sum_{g=2}^G\gamma_g1[G_i=g]+
\sum_{g=2}^G\delta_gD_i1[G_i=g]+\epsilon_i.
$$

$H_0:\delta_2=\cdots=\delta_G=0$ 表示所有组的条件处理效应相同；$H_1$ 表示至少一组不同。HC3 Wald 统计量为：

$$
W=\hat\delta'\widehat{\operatorname{Var}}(\hat\delta)^{-1}\hat\delta.
$$

原假设下参考分布为渐近 $\chi^2_{G-1}$。完整模型须有正的残差自由度，交互系数协方差须可逆；较小 p 值支持组间存在差异。

参考组效应为 $\tau$，其他组为 $\tau+\delta_g$。各组零效应检验及系数检验采用正态 Wald 统计量、双侧 p 值和 95% 区间。此节点检验观测分组交互，不是 Meta 分析的 Cochran Q。因果解释仍须有合适的处理识别设计，异质性检验本身不能消除混杂。

## 输出

`result` 保留原始 `groups`、对应顺序的 `group_effects`、`equality_test`、完整模型的 `coefficients` 及 HC3 `covariance`。
系数名称中的组号指向组列表中从 1 开始的位置。
