# Spearman 秩相关

连接对齐的 Numeric 或 Ordinal **X**、**Y**，至少需要两个完整观测。Ordinal 使用显式声明的等级顺序，数值按升序取秩，并列值取平均秩。常量排序的相关系数未定义，会报错。

$$\rho_s=\operatorname{corr}(R_X,R_Y).$$

原假设为无秩相关。备择默认 `two_sided`，可选 `greater`、`less`。p 值方法默认 `auto`：不超过九个观测时精确置换 Y 的观测位置，支持并列值；更多观测采用 t 近似。`permutation_exact` 最多九个观测，`asymptotic` 至少三个。

精确双侧 p 值为包含观察值的两侧尾概率中较小者的两倍，上限为 1。渐近检验使用 $t=\rho_s\sqrt{(n-2)/(1-\rho_s^2)}$、自由度 $n-2$，大样本时近似较好。不报告秩相关置信区间。

唯一 `result` 记录 rho、样本数、实际推断方法、统计量和 p 值。缺失值会报错，不会分别删去两列中的不同观测。

参考：[SciPy Spearman](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.spearmanr.html)、[精确置换检验](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.permutation_test.html)。
