# IV LIML Summary

连接 IV LIML Fit 产生的 `model`。Configure 默认选择模型概览和系数推断；开启 `first_stage` 或 `overidentification` 后，复用同一拟合样本和模型设定计算相应分析。

`result` 只包含所选内容。不接收原始数据或估计配置，不重新拟合 LIML 模型。过度识别检验需要额外工具变量及 nonrobust 协方差；不可用统计量显示为 null。
