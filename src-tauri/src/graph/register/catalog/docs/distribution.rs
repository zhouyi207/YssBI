//! Random distribution node documentation.

use crate::graph::node::NodeDefinition;

pub fn apply_docs(mut def: NodeDefinition, name: &str) -> NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(node_name: &str) -> Option<(&'static str, &'static str)> {
    Some(match node_name {
        "Bernoulli" => (BERNOULLI_ZH, BERNOULLI_EN),
        "Binomial" => (BINOMIAL_ZH, BINOMIAL_EN),
        "Poisson" => (POISSON_ZH, POISSON_EN),
        "Geometric" => (GEOMETRIC_ZH, GEOMETRIC_EN),
        "NegativeBinomial" => (NEGBIN_ZH, NEGBIN_EN),
        "DiscreteUniform" => (DISC_UNIFORM_ZH, DISC_UNIFORM_EN),
        "Hypergeometric" => (HYPERGEO_ZH, HYPERGEO_EN),
        "Normal" => (NORMAL_ZH, NORMAL_EN),
        "Uniform" => (UNIFORM_ZH, UNIFORM_EN),
        "Exponential" => (EXPONENTIAL_ZH, EXPONENTIAL_EN),
        "Gamma" => (GAMMA_ZH, GAMMA_EN),
        "Beta" => (BETA_ZH, BETA_EN),
        "StudentsT" => (STUDENTST_ZH, STUDENTST_EN),
        "Cauchy" => (CAUCHY_ZH, CAUCHY_EN),
        "ChiSquared" => (CHISQ_ZH, CHISQ_EN),
        "LogNormal" => (LOGNORMAL_ZH, LOGNORMAL_EN),
        "Weibull" => (WEIBULL_ZH, WEIBULL_EN),
        "Laplace" => (LAPLACE_ZH, LAPLACE_EN),
        "Pareto" => (PARETO_ZH, PARETO_EN),
        "InverseGamma" => (INVGAMMA_ZH, INVGAMMA_EN),
        "Triangular" => (TRIANGULAR_ZH, TRIANGULAR_EN),
        "FisherSnedecor" => (FISHER_ZH, FISHER_EN),
        "Erlang" => (ERLANG_ZH, ERLANG_EN),
        _ => return None,
    })
}

pub const BERNOULLI_ZH: &str = include_str!("zh/bernoulli.md");
pub const BERNOULLI_EN: &str = include_str!("en/bernoulli.md");
pub const BINOMIAL_ZH: &str = include_str!("zh/binomial.md");
pub const BINOMIAL_EN: &str = include_str!("en/binomial.md");
pub const POISSON_ZH: &str = include_str!("zh/poisson.md");
pub const POISSON_EN: &str = include_str!("en/poisson.md");
pub const GEOMETRIC_ZH: &str = include_str!("zh/geometric.md");
pub const GEOMETRIC_EN: &str = include_str!("en/geometric.md");
pub const NEGBIN_ZH: &str = include_str!("zh/negative_binomial.md");
pub const NEGBIN_EN: &str = include_str!("en/negative_binomial.md");
pub const DISC_UNIFORM_ZH: &str = include_str!("zh/discrete_uniform.md");
pub const DISC_UNIFORM_EN: &str = include_str!("en/discrete_uniform.md");
pub const HYPERGEO_ZH: &str = include_str!("zh/hypergeometric.md");
pub const HYPERGEO_EN: &str = include_str!("en/hypergeometric.md");
pub const NORMAL_ZH: &str = include_str!("zh/normal.md");
pub const NORMAL_EN: &str = include_str!("en/normal.md");
pub const UNIFORM_ZH: &str = include_str!("zh/uniform.md");
pub const UNIFORM_EN: &str = include_str!("en/uniform.md");
pub const EXPONENTIAL_ZH: &str = include_str!("zh/exponential.md");
pub const EXPONENTIAL_EN: &str = include_str!("en/exponential.md");
pub const GAMMA_ZH: &str = include_str!("zh/gamma.md");
pub const GAMMA_EN: &str = include_str!("en/gamma.md");
pub const BETA_ZH: &str = include_str!("zh/beta.md");
pub const BETA_EN: &str = include_str!("en/beta.md");
pub const STUDENTST_ZH: &str = include_str!("zh/students_t.md");
pub const STUDENTST_EN: &str = include_str!("en/students_t.md");
pub const CAUCHY_ZH: &str = include_str!("zh/cauchy.md");
pub const CAUCHY_EN: &str = include_str!("en/cauchy.md");
pub const CHISQ_ZH: &str = include_str!("zh/chi_squared.md");
pub const CHISQ_EN: &str = include_str!("en/chi_squared.md");
pub const LOGNORMAL_ZH: &str = include_str!("zh/log_normal.md");
pub const LOGNORMAL_EN: &str = include_str!("en/log_normal.md");
pub const WEIBULL_ZH: &str = include_str!("zh/weibull.md");
pub const WEIBULL_EN: &str = include_str!("en/weibull.md");
pub const LAPLACE_ZH: &str = include_str!("zh/laplace.md");
pub const LAPLACE_EN: &str = include_str!("en/laplace.md");
pub const PARETO_ZH: &str = include_str!("zh/pareto.md");
pub const PARETO_EN: &str = include_str!("en/pareto.md");
pub const INVGAMMA_ZH: &str = include_str!("zh/inverse_gamma.md");
pub const INVGAMMA_EN: &str = include_str!("en/inverse_gamma.md");
pub const TRIANGULAR_ZH: &str = include_str!("zh/triangular.md");
pub const TRIANGULAR_EN: &str = include_str!("en/triangular.md");
pub const FISHER_ZH: &str = include_str!("zh/fisher_snedecor.md");
pub const FISHER_EN: &str = include_str!("en/fisher_snedecor.md");
pub const ERLANG_ZH: &str = include_str!("zh/erlang.md");
pub const ERLANG_EN: &str = include_str!("en/erlang.md");
