use faer::Col;

pub struct StandardizeStats1D {
    mean: f64,
    std: f64,
}

pub struct StandardizeTransform1D {
    data: Option<StandardizeStats1D>,
}

impl StandardizeTransform1D {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn fit(&mut self, data: &Col<f64>) {
        assert!(data.nrows() > 0, "cannot standardize an empty sample");
        let mean = data.iter().sum::<f64>() / data.nrows() as f64;
        let mut running_mean = 0.0;
        let mut squared_deviations = 0.0;
        for (index, &value) in data.iter().enumerate() {
            let delta = value - running_mean;
            running_mean += delta / (index + 1) as f64;
            squared_deviations = (value - running_mean).mul_add(delta, squared_deviations);
        }
        let std = (squared_deviations / (data.nrows() - 1) as f64).sqrt();
        self.data = Some(StandardizeStats1D { mean, std });
    }

    pub fn fit_transform(&mut self, data: &Col<f64>) -> Col<f64> {
        self.fit(data);
        self.transform(data)
    }

    pub fn transform(&self, data: &Col<f64>) -> Col<f64> {
        if let Some(stats) = &self.data {
            let eps = f64::EPSILON;
            if stats.std < eps || !stats.std.is_finite() {
                Col::zeros(data.nrows())
            } else {
                Col::from_fn(data.nrows(), |i| (data[i] - stats.mean) / stats.std)
            }
        } else {
            panic!("StandardizeTransform1D not fitted");
        }
    }

    pub fn inverse_transform(&self, data: &Col<f64>) -> Col<f64> {
        if let Some(stats) = &self.data {
            Col::from_fn(data.nrows(), |i| data[i] * stats.std + stats.mean)
        } else {
            panic!("StandardizeTransform1D not fitted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::col;

    #[test]
    fn standardization_preserves_sample_scale_and_degenerate_inputs() {
        let mut transform = StandardizeTransform1D::new();
        let sample = col![1.0, 2.0, 3.0];
        let standardized = transform.fit_transform(&sample);
        assert_eq!(standardized, col![-1.0, 0.0, 1.0]);
        assert_eq!(transform.inverse_transform(&standardized), sample);

        assert_eq!(transform.fit_transform(&col![2.0]), col![0.0]);
        assert_eq!(transform.fit_transform(&col![2.0, 2.0]), col![0.0, 0.0]);
        assert_eq!(
            transform.fit_transform(&col![f64::NAN, 2.0]),
            col![0.0, 0.0]
        );
    }
}
