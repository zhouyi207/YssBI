use ndarray::Array1;

pub struct StandardizeStats1D {
    mean: f64,
    std: f64,
}

pub struct StandardizeTransform1D {
    data: Option<StandardizeStats1D>,
}

impl StandardizeTransform1D {
    pub fn new() -> Self {
        Self {
            data: None,
        }
    }

    pub fn fit(&mut self, data: &Array1<f64>) {
        let mean = data.mean().unwrap();
        let std = data.std(1.0); // 在这里使用 ddof = 1.0 来计算标准差
        self.data = Some(StandardizeStats1D { mean, std });
    }

    pub fn fit_transform(&mut self, data: &Array1<f64>) -> Array1<f64> {
        self.fit(data);
        self.transform(data)
    }

    pub fn transform(&self, data: &Array1<f64>) -> Array1<f64> {
        if let Some(stats) = &self.data {
            data.mapv(|x| (x - stats.mean) / stats.std)
        } else {
            panic!("StandardizeTransform1D not fitted");
        }
    }

    pub fn inverse_transform(&self, data: &Array1<f64>) -> Array1<f64> {
        if let Some(stats) = &self.data {
            data.mapv(|x| x * stats.std + stats.mean)
        } else {
            panic!("StandardizeTransform1D not fitted");
        }
    }
}
