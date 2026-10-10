# 均匀设计（中心拉丁超立方）

无输入。**result** 输出设计摘要，**design** 输出完整编码表：`run` 与 `factor1`…`factorK`，序号及水平从 1 开始。
因素数改变时，下游选列同步更新。运行顺序为生成顺序，不是随机化的现场试验顺序；请在实际试验中另行安排随机化、重复和区组。

不设置固定行数或因素数上限；整数溢出、浮点整数无法精确表示及运行资源预算会阻止不可执行的设计，不截断结果。

参数为 **factors**（默认 3）、**runs**（默认 12）、**candidates**（默认 32）和 **seed**（默认 42）。
各因素的 1 至 runs 水平恰好各出现一次。种子控制候选设计的随机排列；选取中心化 L2 偏差最小的候选。

度量使用归一化坐标 `(level−0.5)/runs`，报告 `centered_l2_discrepancy` 为平方偏差，与 SciPy `qmc.discrepancy(method="CD")` 的约定相同，不再开平方。
映射到物理区间 [a,b] 时可使用 `a + (b−a)×(level−0.5)/runs`。

这是有限候选的空间填充搜索，不保证全局最优，不是特定 U 表的查表复现。计算量约为候选数×试验次数平方×因素数，可取消或超时。

[参考 / Reference](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.qmc.discrepancy.html)
