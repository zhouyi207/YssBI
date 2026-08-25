# OLS Fixed Scale Config

生成 `cov_type = 'fixed scale'` 所需的 **OLSFixedScaleConfig**。连接正数 **Scale**，将 **Config** 接入 **OLS & WLS Configure** → **VCE**。

协方差矩阵使用用户指定的尺度因子，而非仅由残差估计 $\hat\sigma^2$。

## 用法

1. **Scale** 设为所需正数常数。
2. **Config** 输出接入 **OLS & WLS Configure** → **VCE**。
3. 将得到的 **OLSConfigure** 接入 **OLS** / **WLS**。
