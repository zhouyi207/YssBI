pub mod typing;
pub mod transform;
pub mod standard;
pub mod matrix;
pub mod skewness_kurtosis;

pub use matrix::matrix_rank;
pub use skewness_kurtosis::skewness_kurtosis;
pub use standard::{StandardizeStats1D, StandardizeTransform1D};
pub use transform::{IntoFaer, IntoFaerCol, IntoNdarray};
pub use typing::{ArrayLike1D, ArrayLike2D};