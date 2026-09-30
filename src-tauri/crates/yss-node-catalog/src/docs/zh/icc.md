# ICC 组内相关系数

每个 **Rater** 输入为 Numeric 测量列，每行在所有列中对应同一个对象。至少两个对象、两位评定者，最多 64 列；数据必须完整且对齐。

ICC 定义默认 `ICC2`：

| 选项         | 模型与定义         | 测量                  |
| ------------ | ------------------ | --------------------- |
| ICC1 / ICC1k | 单向随机、绝对一致 | 单次 / k 位评定者平均 |
| ICC2 / ICC2k | 双向随机、绝对一致 | 单次 / k 位评定者平均 |
| ICC3 / ICC3k | 双向混合、一致性   | 单次 / k 位评定者平均 |

设有 n 个对象、k 位评定者，$MS_B,MS_J,MS_E,MS_W$ 分别为对象、评定者、双向残差和单向对象内均方。单次测量估计为

$$ICC1=\frac{MS_B-MS_W}{MS_B+(k-1)MS_W},$$
$$ICC2=\frac{MS_B-MS_E}{MS_B+(k-1)MS_E+k(MS_J-MS_E)/n},$$
$$ICC3=\frac{MS_B-MS_E}{MS_B+(k-1)MS_E}.$$

平均测量的分母依次为 $MS_B$、$MS_B+(MS_J-MS_E)/n$、$MS_B$。零 ICC 的 F 检验在 ICC1 中使用 $MS_B/MS_W$、自由度 $(n-1,n(k-1))$，其余使用 $MS_B/MS_E$、自由度 $(n-1,(n-1)(k-1))$。置信水平默认 0.95，区间使用 F 分布，ICC2 采用 Satterthwaite 近似。

`result` 明确记录模型、测量方式、一致性定义、系数、ANOVA 均方、推断和区间。负估计值保留；系数分母非正或没有变异时会报错。无法有限表示的 F 统计量和不可用区间为 null。本节点未提供双向混合模型的绝对一致定义。

参考：[Shrout、Fleiss（1979）](https://doi.org/10.1037/0033-2909.86.2.420)、[参考实现](https://github.com/raphaelvallat/pingouin/blob/main/src/pingouin/reliability.py)。
