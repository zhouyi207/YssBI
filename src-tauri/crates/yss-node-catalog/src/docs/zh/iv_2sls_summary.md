# IV 2SLS Summary

连接 IV 2SLS Fit 产生的 `model`。Configure 默认选择模型概览和系数推断；可开启 `first_stage`、`overidentification` 或 `endogeneity`，仅计算所选分析，复用同一拟合样本和模型设定。

`result` 只包含所选内容。不接收原始数据或估计配置，不重新拟合 IV 模型。过度识别检验需要额外工具变量，当前内生性检验要求 nonrobust 协方差；不可用统计量显示为 null。
