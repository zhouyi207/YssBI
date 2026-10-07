# Nelson–Aalen 累积风险

连接逐行对齐的 `time` 与 `event` 数列。时间必须有限且严格大于 0，单位保持一致；`event=1`（或 true）表示事件发生，`0`（或 false）表示右删失。缺失值会导致计算失败，不自动删行。假设在给定模型或分组条件下删失独立于事件过程；输入须等长并按当前位置逐项对应，可混合数据库与内存数列。

可选 `groups` 最多连接一个分组标签数列；不连接时计算总体曲线，标签按原值保留。此节点无参数。并列时点先计事件、后移除删失。

## 估计与解释

令 $d_j$ 为时点事件数、$r_j$ 为风险集人数：

$$ \widehat H(t)=\sum_{t_j\le t}d_j/r_j,\qquad
\widehat{\mathrm{Var}}\{\widehat H(t)\}=\sum_{t_j\le t}d_j/r_j^2.$$
方差采用计数过程/泊松约定。`result.curves` 包含分组、样本数、事件数、中位生存时间以及各时点风险集、事件和删失计数。`estimate` 与 `cumulative_hazard` 均为累积风险 $\widehat H$；`standard_error` 为其标准误。`survival` 使用 $\exp(-\widehat H)$，与 Kaplan–Meier 估计不同。

95% 风险区间为 $\widehat H\exp\{\pm1.96\,SE/\widehat H\}$；累积风险为零时记为 [0,0]。中位生存时间为该生存概率首次不大于 0.5 的时点，未达到时返回 null；最后发生事件不保证该估计达到中位数。全部删失时累积风险为零。节点不进行假设检验，也不提供 p 值。仅支持独立个体的右删失，不包含延迟入组、区间删失和权重。
$$
