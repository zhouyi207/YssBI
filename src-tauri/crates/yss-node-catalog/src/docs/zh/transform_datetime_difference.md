# 日期差值

连接 Series 和 Other，后者可以是 Datetime 标量或已对齐数列。默认单位 day，也可选周、时、分、秒、毫秒、微秒、纳秒。

输出为 Series 减 Other，以所选单位表达的 Float64 时长，保留小数。Null 传播，日历值沿用项目的无时区解释。
