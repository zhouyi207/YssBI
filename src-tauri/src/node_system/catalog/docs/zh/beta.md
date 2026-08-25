# Beta

Beta 分布 $\mathrm{Beta}(\alpha, \beta)$ 支撑于 $(0,1)$，常用于比例与概率建模：

$$
f(x)=\frac{x^{\alpha-1}(1-x)^{\beta-1}}{B(\alpha,\beta)},\quad 0 < x < 1
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Alpha** | 形状参数 $\alpha > 0$ |
| **Beta** | 形状参数 $\beta > 0$ |
| **N** | 样本量 |

## 用法

设置 **Alpha**、**Beta** 与 **N** 后执行图。**Samples** 输出 $(0,1)$ 内的 `DataSeries<Float64>`。适用于概率先验、比例不确定性及 [0,1] 区间上的随机权重生成。
