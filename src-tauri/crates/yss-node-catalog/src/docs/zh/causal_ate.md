# 平均处理效应 ATE

从已拟合的处理效应结果中提取平均处理效应。

$$
ATE=E[Y(1)-Y(0)].
$$

$Y(1),Y(0)$ 是潜在结果，$D$ 为观测处理状态。目标是包含处理与对照组的完整样本协变量总体。

## 输入

将 PSM、IPW、回归调整或 AIPW 的专用类型 `result` 连接至 `effects`。不接收原始结果数列或拟合参数，也不重新拟合估计器；识别方式、匹配、权重和不确定性均来自上游方法。不接受任意报告、RDD 局部效应或合成控制的时间差距。

## 输出与解释

`result` 包含 `estimand`、上游 `method`、全样本 `observations`、`target_observations`、`effect`、`inference` 和 `bootstrap_replications`。效应保留上游估计值、标准误、统计量、p 值及置信区间；为空的推断字段仍为空。

若上游已启用 bootstrap 推断，检验仍为 $H_0:\theta=0$ 对 $H_1:\theta\ne0$，使用正态 $z=\hat\theta/SE$、双侧 p 值和 95% 正态区间。PSM 结果仅有点估计；这里不追加检验、抽样或总体重加权。

解释时须遵守原估计器的识别假设。处理效应随协变量变化时，ATE 与 ATT 可能不同，不能互换。
