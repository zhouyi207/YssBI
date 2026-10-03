//! Multivariate numerical methods; graph, Arrow and labels remain adapter-owned.
mod adequacy;
mod canonical;
mod common;
mod discriminant;
mod factor;
mod ordination;
#[cfg(test)]
mod tests;

pub use adequacy::sampling_adequacy;
pub use canonical::canonical_correlation;
pub use discriminant::discriminant;
pub use factor::exploratory_factor;
pub use ordination::{correspondence, mds, pca, rda};
