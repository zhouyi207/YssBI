# 配对样本 t 检验

检验同一对象两次测量的平均差是否为零。

## 输入与参数

连接 `before`、`after` 数值序列，按同一对象的顺序逐项配对，长度相等且至少有 2 对有限观测。先在上游共同处理缺失值，节点不会自动匹配对象或删除不完整配对。`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

定义 $d_i=before_i-after_i$。$H_0:\mu_d=0$；备择为 $\mu_d\ne0$、$\mu_d>0$ 或 $\mu_d<0$。

$$
t=\frac{\bar d}{s_d/\sqrt n},\qquad t\overset{H_0}{\sim}t_{n-1}.
$$

$n$ 为配对数，$\bar d$、$s_d$ 为差值均值和样本标准差。不同对象之间应独立；小样本时差值应近似正态，差值标准差必须大于零。

## 输出与判读

`result` 为结构化结果。`statistic` 为 t，`estimate` 为平均 `before−after` 差，`standard_error` 为 $s_d/\sqrt n$，`degrees_of_freedom` 为 `[n−1]`，`sample_sizes` 为 `[n]`。

`greater` 检验前测总体均值高于后测，`less` 方向相反。`p_value` 小于 $\alpha$ 时拒绝零均值差。
