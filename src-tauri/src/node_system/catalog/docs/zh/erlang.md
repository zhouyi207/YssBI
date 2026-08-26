# Erlang

Erlang 分布为 **Gamma** 的特例：形状参数 **K** 为正整数，表示 **K** 个独立同分布指数等待时间之和：

$$
X=\sum_{i=1}^{K} \mathrm{Exp}(\lambda)
$$

## 用法

设置 **K**、**Rate** 与 **N** 后执行图。**Samples** 输出非负的 `DataSeries<Float64>`。适用于 $K$ 阶段排队总等待时间、电话交换系统及服务流程建模。
