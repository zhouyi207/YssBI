//! Multivariate numerical methods; graph, Arrow and labels remain adapter-owned.
mod canonical;
mod common;
mod discriminant;
mod factor;
mod ordination;
#[cfg(test)]
mod tests;

pub use canonical::canonical_correlation;
pub use discriminant::discriminant;
pub use factor::exploratory_factor;
pub use ordination::{correspondence, mds, pca, rda};
