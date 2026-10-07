# 调节作用（二阶交互）

连接 response Y、predictor X、moderator W，可另接协变量。
拟合模型 `Y = 截距 + bX×Xc + bW×Wc + bXW×Xc×Wc + 协变量`。
X 的条件斜率为 `bX + bXW×Wc`；中心化使 bX 对应 W 处于样本均值时的斜率。

所有输入须为对齐的有限数值列，不自动删除缺失行。X、调节变量和协变量均减去各自样本均值，报告 `input_centers`，系数保留原始单位。
协变量只加入线性主效应，不自动参与交互。设计须满秩，观测数须大于参数数目。

**result** 包含 OLS 系数、经典同方差协方差、95% t 区间及拟合指标。**observations** 是包含全部观测、响应、拟合值、残差的分页表，不设固定行数上限。

**details.effects** 报告 X 的条件斜率、标准误、双侧 t 检验及 95% 区间。`probe_sd` 默认 1，探查点为各调节变量的均值及均值 ± 指定倍数的样本标准差，使用原始变量单位。
`in_observed_ranges` 标记探查点是否落在观测范围内；超出范围的点属于外推。

**details.johnson_neyman** 通过完整系数协方差求解双侧 5% 显著性边界，只返回观测到的 W 最小值至最大值内的边界和显著区间。
边界本身对应 p=0.05，区间内为 p<0.05；空边界可能表示全范围显著或全范围不显著，应结合 `significant_ranges` 判断。
这是逐点推断，没有对遍历多个 W 值做多重检验校正；零方差时不输出无穷大的检验统计量。

模型分析连续变量的线性交互，假定独立同方差误差。交互关联不自动构成因果调节证据，不包括分类调节变量的自动编码、稳健/聚类标准误或非线性结果模型。

[Reference: interactions simple slopes](https://interactions.jacob-long.com/reference/sim_slopes.html)
[Reference: Johnson–Neyman](https://interactions.jacob-long.com/reference/johnson_neyman.html)
