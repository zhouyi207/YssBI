//! Shared ordinary least-squares configuration.
pub mod report;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum OlsCovariance {
    #[default]
    NonRobust,
    Hc0,
    Hc1,
    Hc2,
    Hc3,
    FixedScale {
        scale: f64,
    },
    Cluster {
        cluster_id: Vec<usize>,
        xtreg_fe_style: bool,
    },
    Hac {
        kernel: String,
        bandwidth: Option<i64>,
    },
    Newey {
        lag: Option<i64>,
    },
}

impl OlsCovariance {
    pub fn name(&self) -> &'static str {
        match self {
            Self::NonRobust => "nonrobust",
            Self::Hc0 => "HC0",
            Self::Hc1 => "HC1",
            Self::Hc2 => "HC2",
            Self::Hc3 => "HC3",
            Self::FixedScale { .. } => "fixed scale",
            Self::Cluster { .. } => "cluster",
            Self::Hac { .. } => "HAC",
            Self::Newey { .. } => "newey",
        }
    }

    pub fn parameters(&self) -> Option<CovParams> {
        match self {
            Self::NonRobust | Self::Hc0 | Self::Hc1 | Self::Hc2 | Self::Hc3 => None,
            Self::FixedScale { scale } => Some(CovParams::FixedScale { scale: *scale }),
            Self::Cluster {
                cluster_id,
                xtreg_fe_style,
            } => Some(CovParams::Cluster {
                cluster_id: cluster_id.clone(),
                xtreg_fe_style: *xtreg_fe_style,
            }),
            Self::Hac { kernel, bandwidth } => Some(CovParams::HAC {
                kernel: kernel.clone(),
                bandwidth: *bandwidth,
            }),
            Self::Newey { lag } => Some(CovParams::Newey { lag: *lag }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OlsOptions {
    pub constant: bool,
    pub covariance: OlsCovariance,
}

impl Default for OlsOptions {
    fn default() -> Self {
        Self {
            constant: true,
            covariance: OlsCovariance::default(),
        }
    }
}

impl OlsOptions {
    /// Parse a named covariance selection at callers that expose textual model options.
    pub fn from_covariance_parts(
        constant: bool,
        name: &str,
        parameters: Option<&CovParams>,
    ) -> Result<Self, InvalidOlsOptions> {
        let covariance = match (name, parameters) {
            ("" | "nonrobust", _) => OlsCovariance::NonRobust,
            ("HC0", _) => OlsCovariance::Hc0,
            ("HC1", _) => OlsCovariance::Hc1,
            ("HC2", _) => OlsCovariance::Hc2,
            ("HC3", _) => OlsCovariance::Hc3,
            ("fixed scale", Some(CovParams::FixedScale { scale })) => {
                OlsCovariance::FixedScale { scale: *scale }
            }
            (
                "cluster",
                Some(CovParams::Cluster {
                    cluster_id,
                    xtreg_fe_style,
                }),
            ) => OlsCovariance::Cluster {
                cluster_id: cluster_id.clone(),
                xtreg_fe_style: *xtreg_fe_style,
            },
            ("HAC", Some(CovParams::HAC { kernel, bandwidth })) => OlsCovariance::Hac {
                kernel: kernel.clone(),
                bandwidth: *bandwidth,
            },
            ("newey", Some(CovParams::Newey { lag })) => OlsCovariance::Newey { lag: *lag },
            ("hac-panel", _) => return Err(InvalidOlsOptions::NotImplemented("hac-panel")),
            ("hac-groupsum", _) => return Err(InvalidOlsOptions::NotImplemented("hac-groupsum")),
            _ => return Err(InvalidOlsOptions::InvalidCovariance),
        };
        Ok(Self {
            constant,
            covariance,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidOlsOptions {
    #[error("OLS covariance selection or parameters are invalid")]
    InvalidCovariance,
    #[error("cov_type '{0}' not yet implemented")]
    NotImplemented(&'static str),
}

/// 协方差计算所需的额外参数（cluster、HAC、fixed scale 等）
#[derive(Debug, Clone, PartialEq)]
pub enum CovParams {
    FixedScale {
        scale: f64,
    },
    Cluster {
        cluster_id: Vec<usize>,
        /// When true, use Stata xtreg,fe style: denom = (N-k-1) instead of (N-k).
        /// Only for FE within estimator where design matrix has slopes only (no absorbed dummies).
        /// For LSDV: use false — x already includes all dummies, (N-k) is correct.
        xtreg_fe_style: bool,
    },
    HAC {
        kernel: String,
        bandwidth: Option<i64>,
    },
    /// Stata newey: Bartlett kernel + n/(n-k) finite-sample adjustment (与 ivreg2 HAC 不同)
    Newey {
        lag: Option<i64>,
    },
    HacPanel {
        entity_id: Vec<usize>,
        time_id: Vec<usize>,
    },
    HacGroupsum {
        group_id: Vec<usize>,
    },
}
