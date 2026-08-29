use crate::{base::LikelihoodModel, tools::ArrayLike2D};

pub trait RegressionModel: LikelihoodModel {
    fn whiten(self, value: impl ArrayLike2D);

    fn fit(self);
}
