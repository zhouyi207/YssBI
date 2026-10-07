//! Parameters use statistical conventions, independent of node identities and editors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SamplingDistribution {
    Normal {
        mean: f64,
        standard_deviation: f64,
    },
    Uniform {
        lower: f64,
        upper: f64,
    },
    Exponential {
        rate: f64,
    },
    Gamma {
        shape: f64,
        rate: f64,
    },
    Beta {
        alpha: f64,
        beta: f64,
    },
    StudentsT {
        degrees_of_freedom: f64,
    },
    Cauchy {
        location: f64,
        scale: f64,
    },
    ChiSquared {
        degrees_of_freedom: f64,
    },
    LogNormal {
        mu: f64,
        sigma: f64,
    },
    Weibull {
        shape: f64,
        scale: f64,
    },
    Laplace {
        location: f64,
        scale: f64,
    },
    Pareto {
        shape: f64,
        scale: f64,
    },
    InverseGamma {
        shape: f64,
        scale: f64,
    },
    Triangular {
        minimum: f64,
        maximum: f64,
        mode: f64,
    },
    FisherSnedecor {
        numerator: f64,
        denominator: f64,
    },
    Erlang {
        shape: u64,
        rate: f64,
    },
    Bernoulli {
        probability: f64,
    },
    Binomial {
        trials: u64,
        probability: f64,
    },
    Poisson {
        rate: f64,
    },
    /// Number of trials INCLUDING the first success, with support starting at one.
    Geometric {
        probability: f64,
    },
    /// Number of failures before r successes; positive real r is supported.
    NegativeBinomial {
        successes: f64,
        probability: f64,
    },
    DiscreteUniform {
        lower: i64,
        upper: i64,
    },
    Hypergeometric {
        population: u64,
        successes: u64,
        draws: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SampleValue {
    Integer(i64),
    Float(f64),
}
