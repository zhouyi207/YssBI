# Kendall 秩相关

连接对齐的 Numeric 或 Ordinal **X**、**Y**，至少需要两个完整观测；有序列按声明等级取序。本节点计算修正两列并列值的 tau-b：

$$\tau_b=\frac{C-D}{\sqrt{(n_0-T_X)(n_0-T_Y)}},\quad n_0=n(n-1)/2.$$

C、D 分别为一致、逆序观测对数；$T_X,T_Y$ 包含每列的全部并列对，含两列同时并列的对。任一变量为常量时分母为零，会报错。

原假设为无有序相关，备择可选默认 `two_sided`、`greater`、`less`。`auto` 在不超过九个观测时精确置换位置并支持并列值，更多观测对 $C-D$ 使用并列修正的正态近似。`permutation_exact` 最多九个观测；`asymptotic` 至少三个。精确双侧 p 值取两侧包含观察值的尾概率中较小者的两倍，上限为 1。

`result` 含 tau-b、样本数、观测对/并列计数和推断。本节点的精确模式按观察到的秩枚举位置置换，不是仅适用于无并列数据的精确分布。不提供置信区间。

参考：[SciPy tau-b](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.kendalltau.html)、[置换检验](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.permutation_test.html)。
