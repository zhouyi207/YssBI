# Prais

Prais–Winsten / Cochrane–Orcutt AR(1) 误差修正回归：

$$
y_t = x_t'\beta + u_t,\quad u_t = \rho u_{t-1} + \varepsilon_t
$$

按时间顺序连接因变量和有序自变量，要求有限且对齐。参数包含 transform（prais_winsten 或 cochrane_orcutt）、截距、最大迭代次数（正整数）与正容差。默认 Prais–Winsten、包含截距、迭代 100 次、容差 1e-6。达到迭代上限但未收敛会报错。

输出 model 及原始观测尺度的 fitted、residuals。Prais Summary 读取既有系数、推断、rho 和迭代统计，不重新拟合。
