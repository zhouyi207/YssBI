# 动态面板（一步差分 GMM）

输入等长且按当前位置逐项对应的数值 **Y**、可选的严格外生 **X₁, X₂, …**、数值实体编号 **entity** 和整数期次 **time**。数列必须有限、无缺失；节点自动按键排序。要求平衡面板，各实体具有相同起止期，每期恰好一行且期次连续；至少 4 期、3 个实体，且实体数必须大于工具变量数。

模型为 $y_{it}=\rho y_{i,t-1}+x_{it}'\beta+\alpha_i+\varepsilon_{it}$。差分消除 $\alpha_i$，使用 Arellano–Bond 一步 GMM：

$$
\hat\theta=(X'ZWZ'X)^{-1}X'ZWZ'\Delta y,\qquad
W=\left(\sum_i Z_i'H_iZ_i\right)^{-1}.
$$

$X$ 包含滞后因变量的一阶差分与自变量差分；$H_i$ 的对角为 2、相邻元素为 -1。每个因变量滞后阶只占一列折叠工具，当前严格外生自变量差分作为自身工具。不在差分方程中另加截距。

**参数**：

- max*instrument_lag 默认 3，为至少 2 的整数，且必须小于期数。工具为水平 $y*{i,t-2}$ 至指定阶；观测尚无该滞后时对应工具为零。
- covariance 默认 robust：按实体的矩条件得分计算 sandwich，无小样本修正。nonrobust 假设水平误差同方差且无序列相关，以差分残差平方均值的一半估计水平误差方差。

唯一 **result** 包含 coefficients、parameter_names、inference（协方差、标准误、z、双侧 p 值及 95% 正态区间）、instruments、entities、time_periods、observations、fitted、residuals 和 source_rows。系数检验针对零系数；前两期不进入差分方程。fitted/residuals 是差分尺度，source_rows 为各方程当前期的零起始原始行号。

该估计要求水平误差没有序列相关、滞后水平工具有效，且 predictors 严格外生。当前入口不提供系统 GMM、两步 GMM、内生自变量工具、时间虚拟变量或 AR/Sargan/Hansen 检验；不能将系数显著性视为工具有效性的验证。工具或设计秩不足会失败。

方法背景见 [Stata Arellano–Bond 手册](https://www.stata.com/manuals/xtxtabond.pdf)。
