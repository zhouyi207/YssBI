# NRI 和 IDI

连接对齐的 **outcome**（事件为 1/true，非事件为 0/false）、**reference**（基准模型事件概率）和 **new**（新模型事件概率）。两种结局都必须出现，概率须在 $[0,1]$；不删除缺失行。

**重分类方式**默认 continuous：新概率高于旧概率记为上移，低于记为下移，相等不变。此时 **风险分界点**留空。categorical 模式按严格递增且位于 $(0,1)$ 的分界点分组，比较风险组的上移/下移；等于分界点进入较高组。

$$
NRI=[P(up\mid Y=1)-P(down\mid Y=1)]+[P(down\mid Y=0)-P(up\mid Y=0)].
$$

$$
IDI=(\bar p_{new,1}-\bar p_{new,0})-(\bar p_{ref,1}-\bar p_{ref,0}).
$$

下标 1/0 表示事件/非事件组；IDI 始终使用原概率，不受风险分组影响。

**result** 提供两组的样本数、上移/下移/不变人数，nri_events、nri_nonevents、nri，两模型的 discrimination_slope 及 idi。正值表示该指标上的改善。当前输出点估计，不提供置信区间或显著性检验，也不处理删失结局。两组预测必须来自相同验证样本、相同事件定义和预测时点。
