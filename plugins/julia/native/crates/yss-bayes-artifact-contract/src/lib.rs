//! Backend-neutral Bayesian artifact access contract.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::Path;

use yss_bayes_result::{
    AutocorrelationPlotData, DensityPlotData, PosteriorPredictivePage, PosteriorSamplePage,
    TracePlotData,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayesArtifactReadError {
    Read,
    InvalidSamples,
    InvalidPosteriorPredictive,
    Export,
}

impl fmt::Display for BayesArtifactReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Read => "Bayesian artifact could not be read",
            Self::InvalidSamples => "Bayesian posterior samples are invalid",
            Self::InvalidPosteriorPredictive => "Bayesian posterior predictive artifact is invalid",
            Self::Export => "Bayesian artifact could not be exported",
        })
    }
}

impl std::error::Error for BayesArtifactReadError {}

pub trait BayesArtifactReader: Send + Sync {
    fn export_csv(&self, source: &Path, destination: &Path) -> Result<(), BayesArtifactReadError>;

    fn sample_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
        parameter: Option<&str>,
    ) -> Result<PosteriorSamplePage, BayesArtifactReadError>;

    fn trace_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_points_per_chain: usize,
    ) -> Result<TracePlotData, BayesArtifactReadError>;

    fn density_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        grid_points: usize,
    ) -> Result<DensityPlotData, BayesArtifactReadError>;

    fn autocorrelation_plot_data(
        &self,
        source: &Path,
        parameter: Option<&str>,
        max_lag: usize,
    ) -> Result<AutocorrelationPlotData, BayesArtifactReadError>;

    fn posterior_predictive_page(
        &self,
        source: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorPredictivePage, BayesArtifactReadError>;
}
