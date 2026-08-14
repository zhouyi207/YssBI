//! Stable application-facing API surface.
//!
//! Existing module paths remain available for compatibility. New app code should
//! prefer these curated re-exports over reaching into implementation modules.

pub mod regression {
    pub use crate::regression::covariance::CovParams;
    pub use crate::regression::diagnostics;
    pub use crate::regression::linear_model::{
        GLS, GLSConfig, GLSResult, IV2SLS, IV2SLSConfig, IV2SLSResult, IVLIML, IVLIMLConfig,
        IVLIMLResult, OLS, OLSConfig, OLSResult, Prais, PraisConfig, PraisResult, WLS, WLSConfig,
        WLSResult,
    };
    pub use crate::regression::panel::{
        PanelFEStats, PanelOLSResult, fit_panel_fd, fit_panel_fe, fit_panel_fe_time,
        fit_panel_fe_twoway, fit_panel_lsdv, fit_panel_lsdv_time, fit_panel_lsdv_twoway,
        fit_panel_re_be, fit_panel_re_be_time, fit_panel_re_fgls, fit_panel_re_fgls_time,
        fit_panel_re_fgls_twoway, fit_panel_re_mle, fit_panel_re_mle_time, fit_panel_re_mle_twoway,
    };
}

pub mod time_series {
    pub use crate::ts::unit_root::{AdfRegRow, AdfRegression, AdfResult, adf_test};
    pub use crate::ts::var::{
        VAR, VARConfig, VARResult, VARSocResult, var_regression_times_stata, var_varsoc,
    };
    pub use crate::ts::vec::{
        VECConfig, VECResult, VecRankResult, VecTrendSpec, vec_estimate, vec_vecrank_stats,
    };
    pub use crate::ts::{acf_pacf, align, diff, lag, pct_change, rolling, serial_correlation};
}

pub mod tools {
    pub use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, StandardizeTransform1D};
}
