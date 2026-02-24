use ndarray::Array1;
use ndarray::Array2;

pub trait LikelihoodModel {
    /// 观测数据（endog）
    type Endog: Clone; // 通常是 Array1<f64> 或 Vec<bool> 等
    /// 解释变量（exog），可以是 None
    type Exog: Clone; // 通常 Array2<f64>

    /// 模型参数的维度（k_params）
    fn n_params(&self) -> usize;

    /// 对数似然函数：log L(θ)
    /// 返回值是 log-likelihood（越高越好）
    fn log_likelihood(&self, params: &Array1<f64>) -> f64;

    /// 一阶导数（score vector）：∂logL / ∂θ
    /// 大多数优化器需要这个（梯度）
    fn score(&self, params: &Array1<f64>) -> Array1<f64>;

    /// 二阶导数（Hessian）：∂²logL / ∂θ∂θ'
    /// Newton 法需要；其他方法可选（可以数值近似）
    fn hessian(&self, params: &Array1<f64>) -> Array2<f64>;

    /// Fisher 信息矩阵（默认实现：-Hessian）
    fn information(&self, params: &Array1<f64>) -> Array2<f64> {
        -self.hessian(params)
    }

    /// 获取观测数 n（常用于标准化 loglike / score）
    fn nobs(&self) -> usize;

    // ────────────────────────────────────────────────
    // 以下是可选的默认实现（像 statsmodels 一样提供便利）
    // ────────────────────────────────────────────────

    /// 标准化后的负对数似然（优化目标：最小化这个）
    fn neg_log_likelihood_normalized(&self, params: &Array1<f64>) -> f64 {
        -self.log_likelihood(params) / self.nobs() as f64
    }

    /// 标准化 score（很多优化器期望这个）
    fn score_normalized(&self, params: &Array1<f64>) -> Array1<f64> {
        self.score(params) / self.nobs() as f64
    }

    /// 标准化 hessian
    fn hessian_normalized(&self, params: &Array1<f64>) -> Array2<f64> {
        self.hessian(params) / self.nobs() as f64
    }

    // 初始参数（可以被子类覆盖）
    fn start_params(&self) -> Array1<f64> {
        Array1::zeros(self.n_params())
    }
}
