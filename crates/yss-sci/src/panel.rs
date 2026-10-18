//! Panel data regression: FE (Within), LSDV, FD, RE

mod data;
mod dynamic;
mod fd;
mod fe;
mod lsdv;
mod nonstationary;
mod re;

pub use fd::fit_panel_fd;
pub use fe::{fit_panel_fe, fit_panel_fe_time, fit_panel_fe_twoway};
pub use lsdv::{fit_panel_lsdv, fit_panel_lsdv_time, fit_panel_lsdv_twoway};
pub use re::{
    fit_panel_re_be, fit_panel_re_be_time, fit_panel_re_fgls, fit_panel_re_fgls_time,
    fit_panel_re_fgls_twoway, fit_panel_re_mle, fit_panel_re_mle_time, fit_panel_re_mle_twoway,
};

use crate::regression::design::covariance_rows;
pub use yss_sci_contract::panel::{
    ObsPerGroupStats, PanelFEStats, PanelFit, PanelR2Stats, SigmaStats, ThetaStats,
};
use yss_sci_contract::panel::{PanelEstimatorStatistics, PanelModelStatistics};
use yss_sci_contract::regression::fit::{
    LinearRegressionStatistics, RegressionCoefficientStatistics,
};

pub mod fit;
pub use dynamic::difference_gmm;
pub use nonstationary::{fisher_cointegration, fisher_unit_root};
