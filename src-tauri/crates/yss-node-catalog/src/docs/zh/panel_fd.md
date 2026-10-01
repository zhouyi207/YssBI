# 第一差分模型

输入同一行域的数值 response、至少一个 predictors、entity 编号和整数期次 time；数列等长、有限且无缺失，实体与时间组合唯一。节点按实体、时间排序，仅保留同一实体中时间差恰好为 1 的相邻观测对，缺口不会被当作一期差分。

模型为 $\Delta y_{it}=\Delta x_{it}'\beta+\Delta\varepsilon_{it}$，时间不变的实体截距被消去。此节点固定实体维度并移除截距；不会额外拟合差分方程截距。

**参数** covariance 默认 nonrobust，可选 HC0–HC3 或按实体聚类的 cluster。

唯一输出 **model** 可连接“面板模型汇总”。模型中的 estimation 样本处于一阶差分尺度；sourceRows 每项包含差分前后两个零起始原始行号。系数检验为 $H_0:\beta_j=0$ 对 $H_1:\beta_j\ne0$，使用估计值/标准误及模型的 t 自由度。必须有足够的有效差分和可识别的设计；不自动处理滞后因变量造成的内生性。
