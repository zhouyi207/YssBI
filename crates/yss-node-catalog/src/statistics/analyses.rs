//! Executable diagnostics and post-estimation protocols.
use super::*;
mod models;

const SPECS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "yssbi.statistics.diagnostic.breusch_pagan",
        "Breusch–Pagan / Koenker",
        "Breusch–Pagan / Koenker 异方差检验",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.white",
        "White test",
        "White 异方差检验",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.information_matrix",
        "Cameron–Trivedi IM test",
        "Cameron–Trivedi IM 分解",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.reset",
        "Ramsey RESET",
        "Ramsey RESET 设定检验",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.vif",
        "Variance inflation factors",
        "VIF 方差膨胀因子",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.leverage",
        "Leverage",
        "杠杆值",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.breusch_godfrey",
        "Breusch–Godfrey",
        "Breusch–Godfrey 序列相关检验",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.diagnostic.wald",
        "Linear coefficient hypothesis",
        "系数线性约束 t / Wald 检验",
        "statistics.diagnostics",
        "linear",
    ),
    (
        "yssbi.statistics.test.normality",
        "Jarque–Bera / Omnibus",
        "Jarque–Bera / Omnibus 正态性检验",
        "statistics.tests",
        "series",
    ),
    (
        "yssbi.statistics.diagnostic.durbin_watson",
        "Durbin–Watson",
        "Durbin–Watson 检验",
        "statistics.diagnostics",
        "series",
    ),
    (
        "yssbi.statistics.diagnostic.ljung_box",
        "Ljung–Box",
        "Ljung–Box 白噪声检验",
        "statistics.diagnostics",
        "series",
    ),
    (
        "yssbi.statistics.timeseries.acf",
        "Autocorrelation",
        "ACF 自相关函数",
        "statistics.timeseries",
        "series",
    ),
    (
        "yssbi.statistics.timeseries.pacf",
        "Partial autocorrelation",
        "PACF 偏自相关函数",
        "statistics.timeseries",
        "series",
    ),
    (
        "yssbi.statistics.timeseries.granger",
        "VAR Granger Wald tests",
        "VAR Granger 因果检验",
        "statistics.timeseries",
        "var",
    ),
    (
        "yssbi.statistics.timeseries.irf",
        "VAR orthogonalized impulse responses",
        "VAR 正交脉冲响应",
        "statistics.timeseries",
        "var",
    ),
    (
        "yssbi.statistics.timeseries.fevd",
        "VAR forecast error variance decomposition",
        "VAR 预测误差方差分解",
        "statistics.timeseries",
        "var",
    ),
    (
        "yssbi.statistics.diagnostic.hausman",
        "IV Hausman test",
        "IV Hausman 检验",
        "statistics.diagnostics",
        "iv_2sls",
    ),
];

fn help(id: &str) -> (&'static str, &'static str) {
    match id.rsplit('.').next().unwrap_or_default() {
        "breusch_pagan" => (
            "Connect an OLS or WLS model. By default, tests residual variance against fitted values. rhs uses explanatory variables; koenker selects the studentized variant. Reuses the fitted sample and original WLS weights. Outputs the computed statistic, degrees of freedom and p-value as result; failed diagnostics are errors.",
            "连接 OLS 或 WLS 模型，默认以拟合值检验残差异方差。rhs 改用解释变量，koenker 选择学生化变体。复用拟合样本和原 WLS 权重，result 输出统计量、自由度和 p 值；无法计算时明确报错。",
        ),
        "reset" => (
            "Connect an OLS or WLS model. RESET augments the regression with powers of fitted values, or explanatory variables when rhs is enabled. Reuses fitted observations and WLS weights. Outputs the F statistic, degrees of freedom and p-value. Rank-deficient expansions or insufficient observations fail explicitly.",
            "连接 OLS 或 WLS 模型。RESET 默认添加拟合值幂次，开启 rhs 后添加解释变量幂次。复用拟合样本和 WLS 权重，输出 F 统计量、自由度和 p 值。扩展矩阵秩不足或观测不足时明确报错。",
        ),
        "breusch_godfrey" => (
            "Connect a fitted linear model in observation/time order. Select lags (1–40); the runtime caps the effective order at min(n / 2 - 1, 40). bg_nomiss0 fills initial lagged residuals with zero when enabled. Reuses the source model's serial-test analysis and returns LM, p-value and effective lags without refitting that model.",
            "连接按观测/时间顺序拟合的线性模型。lags 可选 1–40，实际阶数受 min(n / 2 - 1, 40) 限制。bg_nomiss0 控制初始滞后残差是否补零。复用模型的序列检验分析，输出 LM、p 值和实际阶数，不重新拟合原模型。",
        ),
        "wald" => (
            "Connect a fitted linear model and enter a coefficient hypothesis, for example x1 = 0 or x1 = x2. Coefficient names and ordering come from the fitted report. Uses the existing covariance and residual degrees of freedom; a single supported constraint uses a t test and joint equalities use Wald inference. Outputs the parsed hypothesis and computed result without refitting.",
            "连接已拟合线性模型，填写系数约束，例如 x1 = 0 或 x1 = x2。变量名与顺序以模型报告为准。使用既有协方差及残差自由度，单条受支持约束使用 t 检验，联合等式使用 Wald 推断。输出解析后的假设与实际结果，不重新拟合。",
        ),
        "vif" => (
            "Connect a fitted linear model. Computes VIF and tolerance for each original predictor; the configured intercept has null entries. This is a design diagnostic, including for GLS; it does not whiten the design or refit the response model.",
            "连接已拟合线性模型。对原始自变量设计逐列计算 VIF 和容忍度，已配置截距的结果为空。GLS 同样检查原始设计，不对白化后的矩阵计算，也不重新拟合因变量模型。",
        ),
        "leverage" => (
            "Connect an OLS or WLS model. Returns one diagonal hat-matrix value per fitted observation, preserving observation order and applying the original WLS precision weights. GLS is not supported.",
            "连接 OLS 或 WLS 模型，输出每个拟合观测的帽子矩阵对角值，保留观测顺序；WLS 使用原精度权重。当前不支持 GLS。",
        ),
        "normality" => (
            "Connect a finite numeric series, typically fitted residuals. At least eight observations are required for the combined Jarque–Bera and Omnibus result. Outputs both statistics and p-values; unavailable calculations fail explicitly.",
            "连接有限数值序列，通常为拟合残差。组合的 Jarque–Bera 与 Omnibus 检验至少需要 8 个观测。输出两种统计量和 p 值，无法计算时明确报错。",
        ),
        "acf" | "pacf" => (
            "Connect a finite numeric series in time order with at least four observations. lags is 1–40, capped by min(n / 2 - 1, 40). ACF values start at lag zero; PACF values start at lag one. Outputs function, observations and values. Constant series retain only lag-zero ACF and empty PACF.",
            "连接按时间顺序排列的有限数值序列，至少 4 个观测。lags 为 1–40，实际受 min(n / 2 - 1, 40) 限制。ACF 数组从 0 阶开始，PACF 从 1 阶开始；输出 function、observations、values。常量序列仅保留 0 阶 ACF，PACF 为空。",
        ),
        "durbin_watson" | "ljung_box" => (
            "Connect a finite series of residuals in time order, with at least four observations. Returns Durbin–Watson, or the Ljung–Box Q statistic, p-value and effective lags. For Ljung–Box, lags is 1–40 and capped at min(n / 2 - 1, 40). No model-degree adjustment is supplied by this series-only node.",
            "连接按时间顺序排列的有限残差序列，至少 4 个观测。输出 Durbin–Watson 统计量，或 Ljung–Box 的 Q、p 值和实际阶数。Ljung–Box 的 lags 为 1–40，实际受 min(n / 2 - 1, 40) 限制。本序列节点不提供模型自由度修正。",
        ),
        "granger" | "irf" | "fevd" => (
            "Connect the model output of VAR Fit. Computes Granger Wald tests, orthogonalized impulse responses or forecast-error variance decomposition from the fit without refitting it. IRF/FEVD use steps (1–1000, default 8) and retain horizons zero through steps. Orthogonalization depends on the fitted variable ordering. Results retain their multivariate dimensions.",
            "连接 VAR Fit 的 model，基于拟合结果计算 Granger Wald 检验、正交脉冲响应或预测误差方差分解，不重新拟合。IRF/FEVD 的 steps 可选 1–1000，默认 8，输出第 0 到 steps 期；正交化结果依赖拟合时的变量顺序，输出保留多变量维度。",
        ),
        "hausman" => (
            "Connect a nonrobust IV 2SLS model. Computes the Hausman endogeneity test from its retained observations, design and IV coefficients. This is not a panel FE/RE comparison; unavailable tests are errors.",
            "连接 nonrobust IV 2SLS 模型，复用其观测、设计列及 IV 系数计算 Hausman 内生性检验。本节点不是面板 FE/RE 比较；不可计算的检验会报错。",
        ),
        _ => (
            "Connect an OLS or WLS model for the White heteroskedasticity test or Cameron–Trivedi information-matrix decomposition. Uses fitted observations, original design columns and WLS weights. Outputs actual test statistics, degrees of freedom and p-values as result. GLS and unavailable diagnostics fail explicitly.",
            "连接 OLS 或 WLS 模型，进行 White 异方差检验或 Cameron–Trivedi 信息矩阵分解。复用拟合观测、原设计列和 WLS 权重，result 输出统计量、自由度和 p 值。GLS 及不可计算的诊断明确报错。",
        ),
    }
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    models::append(fragment)?;
    for &(id, en, zh, category, input) in SPECS {
        let port = if input == "series" {
            data_input("series", "DataSeries", series_type()?)?
        } else {
            let model_type = match input {
                "linear" => "statistics.model.linear",
                "var" => "statistics.model.var",
                _ => "statistics.model.iv_2sls",
            };
            data_input("model", "Model", concrete(model_type)?)?
        };
        let parameters = if id.ends_with("breusch_pagan") {
            vec![
                toggle_parameter("rhs", false)?,
                toggle_parameter("koenker", false)?,
            ]
        } else if id.ends_with("reset") {
            vec![toggle_parameter("rhs", false)?]
        } else if id.ends_with("breusch_godfrey") {
            vec![lag()?, toggle_parameter("bg_nomiss0", true)?]
        } else if id.ends_with("wald") {
            vec![parameter(
                "hypothesis",
                concrete("core.text")?,
                ParameterEditorSpec::Text { multiline: false },
                DataValue::String("x1 = 0".into()),
                vec![
                    ParameterConstraint::Required,
                    ParameterConstraint::Length {
                        min: Some(1),
                        max: Some(4096),
                    },
                ],
            )?]
        } else if id.ends_with("irf") || id.ends_with("fevd") {
            vec![bounded_integer_parameter("steps", 8, 1, 1000)?]
        } else if id.ends_with("acf") || id.ends_with("ljung_box") {
            vec![lag()?]
        } else {
            vec![]
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: None,
                    category_id: sid(category, NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(
                    id,
                    vec![port, data_output("result", "Result", report_type()?)?],
                    vec![],
                    vec![],
                )?,
                parameters: assembled_parameters(id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
        let (en_help, zh_help) = help(id);
        for (locale, title, doc) in [("en-US", en, en_help), ("zh-CN", zh, zh_help)] {
            fragment
                .messages
                .push((locale, node_key_text(id, "title"), Text(title)));
            fragment
                .messages
                .push((locale, node_key_text(id, "documentation"), Text(doc)));
        }
    }
    Ok(())
}
fn lag() -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = positive_integer_parameter("lags", 1)?;
    parameter.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(1),
        max: Some(40),
    }];
    Ok(parameter)
}
