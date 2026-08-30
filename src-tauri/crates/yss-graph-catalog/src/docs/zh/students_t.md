# Student's t

Student's t 分布（自由度 $\nu$）用于小样本均值推断，本节点固定位置 0、尺度 1：

$$
f(x)=\frac{\Gamma\!\left(\frac{\nu+1}{2}\right)}{\sqrt{\nu\pi}\,\Gamma\!\left(\frac{\nu}{2}\right)}\left(1+\frac{x^2}{\nu}\right)^{-\frac{\nu+1}{2}}
$$

## 用法

连接 **DF** 与 **N** 后执行图。**Samples** 输出 `DataSeries<Float64>`。适用于厚尾随机误差、小样本 $t$ 统计量模拟及与正态分布尾部行为的对比。
