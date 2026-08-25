# Panel DID (TWFE)

$2\times2$ 设计的双向固定效应 DID。

对 **Y** 回归可选 **X** 与 **Treat×Post** — Treat、Post 主效应被个体与时间 FE 吸收：

$$
Y_{it} = \alpha_i + \gamma_t + \beta (Treat_i \times Post_t) + X_{it}'\delta + \varepsilon_{it}
$$

## 输入

- **Y**、可选 **X**
- **Entity ID**、**Time ID**
- **Treat**、**Post** — 布尔 `DataSeries`
- 可选 **Panel Configure**

**Treat×Post** 的系数为 DID 估计量。报告可含平行趋势与安慰剂检验（由 config 控制）。
