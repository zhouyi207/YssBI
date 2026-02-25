import pandas as pd
import statsmodels.api as sm

# 1. 读取和 Rust 完全相同的 csv
df = pd.read_csv("tests/iris.csv")

y = df['sepal_length'].values

X = df[['sepal_width', 'petal_length', 'petal_width']].values
X = sm.add_constant(X)   # β0 截距项（等价于你 Rust 的 vec![1.0; n]）

# 2. OLS 拟合
model = sm.OLS(y, X)
result = model.fit()

print(result.summary())

#                             OLS Regression Results
# ==============================================================================
# Dep. Variable:                      y   R-squared:                       0.859
# Model:                            OLS   Adj. R-squared:                  0.856
# Method:                 Least Squares   F-statistic:                     297.0
# Date:                Wed, 25 Feb 2026   Prob (F-statistic):           6.28e-62
# Time:                        14:27:49   Log-Likelihood:                -37.000
# No. Observations:                 150   AIC:                             82.00
# Df Residuals:                     146   BIC:                             94.04
# Df Model:                           3
# Covariance Type:            nonrobust
# ==============================================================================
#                  coef    std err          t      P>|t|      [0.025      0.975]
# ------------------------------------------------------------------------------
# const          1.8451      0.250      7.368      0.000       1.350       2.340
# x1             0.6549      0.067      9.823      0.000       0.523       0.787
# x2             0.7111      0.057     12.560      0.000       0.599       0.823
# x3            -0.5626      0.127     -4.426      0.000      -0.814      -0.311
# ==============================================================================
# Omnibus:                        0.265   Durbin-Watson:                   2.053
# Prob(Omnibus):                  0.876   Jarque-Bera (JB):                0.432
# Skew:                           0.003   Prob(JB):                        0.806
# Kurtosis:                       2.737   Cond. No.                         54.7
# ==============================================================================