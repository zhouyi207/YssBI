# Panel DID (TWFE)

$2\times2$ 设计的双向固定效应 DID。

对 **Y** 回归可选 **X** 与 **Treat×Post** — Treat、Post 主效应被个体与时间 FE 吸收：

$$
Y_{it} = \alpha_i + \gamma_t + \beta (Treat_i \times Post_t) + X_{it}'\delta + \varepsilon_{it}
$$

本节点属于 Fit，接收 `response`、`predictors`、`entity`、`time`、`treatment`，参数为 `event_study` 和 `placebo_repetitions`，输出 `model`、`fitted`、`residuals` 和 `report`。执行内核尚未注册。
