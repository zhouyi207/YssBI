//! Stateless correlation and agreement entry points over neutral contracts.
use yss_sci_contract::association::*;
use yss_sci_contract::execution::{ScientificComputationError, ScientificExecutionControl};
use yss_sci_contract::hypothesis::Alternative;
type Result<T> = std::result::Result<T, ScientificComputationError>;

pub fn pearson(
    x: &[f64],
    y: &[f64],
    options: CorrelationOptions,
    control: &ScientificExecutionControl,
) -> Result<CorrelationResult> {
    yss_sci::association::pearson(x, y, options, control)
}
pub fn partial(
    x: &[f64],
    y: &[f64],
    controls: &[Vec<f64>],
    options: CorrelationOptions,
    control: &ScientificExecutionControl,
) -> Result<CorrelationResult> {
    yss_sci::association::partial(x, y, controls, options, control)
}
pub fn spearman(
    x: &[f64],
    y: &[f64],
    options: RankCorrelationOptions,
    control: &ScientificExecutionControl,
) -> Result<CorrelationResult> {
    yss_sci::association::spearman(x, y, options, control)
}
pub fn kendall(
    x: &[f64],
    y: &[f64],
    options: RankCorrelationOptions,
    control: &ScientificExecutionControl,
) -> Result<CorrelationResult> {
    yss_sci::association::kendall(x, y, options, control)
}
pub fn kappa(
    ratings: &[Vec<usize>],
    categories: usize,
    options: KappaOptions,
    control: &ScientificExecutionControl,
) -> Result<KappaResult> {
    yss_sci::association::kappa(ratings, categories, options, control)
}
pub fn icc(
    ratings: &[Vec<f64>],
    kind: IccType,
    confidence: f64,
    control: &ScientificExecutionControl,
) -> Result<IccResult> {
    yss_sci::association::icc(ratings, kind, confidence, control)
}
pub fn bland_altman(
    x: &[f64],
    y: &[f64],
    coverage: f64,
    confidence: f64,
    control: &ScientificExecutionControl,
) -> Result<BlandAltmanResult> {
    yss_sci::association::bland_altman(x, y, coverage, confidence, control)
}
pub fn kendall_w(
    ratings: &[Vec<f64>],
    control: &ScientificExecutionControl,
) -> Result<ConcordanceResult> {
    yss_sci::association::kendall_w(ratings, control)
}
pub fn ridit(
    sample: &[usize],
    reference: &[usize],
    categories: usize,
    alternative: Alternative,
    continuity: bool,
    control: &ScientificExecutionControl,
) -> Result<RiditResult> {
    yss_sci::association::ridit(
        sample,
        reference,
        categories,
        alternative,
        continuity,
        control,
    )
}
pub fn rwg(
    items: &[Vec<f64>],
    null: AgreementNull,
    control: &ScientificExecutionControl,
) -> Result<RwgResult> {
    yss_sci::association::rwg(items, null, control)
}
