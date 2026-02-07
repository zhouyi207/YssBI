use std::vec;

use faer::Mat;

#[derive(Clone, Debug)]
pub struct Array {
    shape: Vec<usize>, // e.g. [n], [n, m]
    data: Vec<f64>,    // row-major
}

impl Array {
    pub fn from_vec(data: Vec<f64>) -> Self {
        Self {
            shape: vec![data.len()],
            data,
        }
    }

    pub fn from_shape(shape: Vec<usize>, data: Vec<f64>) -> Self {
        let size: usize = shape.iter().product();
        assert_eq!(size, data.len());
        Self { shape, data }
    }

    pub fn zeros(shape: Vec<usize>) -> Self {
        let size: usize = shape.iter().product();
        Self {
            shape,
            data: vec![0.0; size],
        }
    }

    pub fn ones(shape: Vec<usize>) -> Self {
        let size: usize = shape.iter().product();
        Self {
            shape,
            data: vec![1.0; size],
        }
    }

    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn get(&self, index: usize) -> Option<f64> {
        self.data.get(index).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &f64> {
        self.data.iter()
    }

    pub fn data(&self) -> &[f64] {
        &self.data
    }
}

impl Array {
    pub fn as_mat(&self) -> Mat<f64> {
        assert_eq!(self.shape.len(), 2);
        let (rows, cols) = (self.shape[0], self.shape[1]);
        Mat::from_fn(rows, cols, |i, j| self.data[i * cols + j])
    }

    pub fn from_mat(mat: &Mat<f64>) -> Self {
        let rows = mat.nrows();
        let cols = mat.ncols();

        let mut data = Vec::with_capacity(rows * cols);
        for i in 0..rows {
            for j in 0..cols {
                data.push(mat[(i, j)]);
            }
        }

        Self {
            shape: vec![rows, cols],
            data,
        }
    }
}

impl Array {
    pub fn sum(&self) -> f64 {
        self.data.iter().copied().sum()
    }

    pub fn mean(&self) -> f64 {
        self.sum() / self.len() as f64
    }

    pub fn var(&self) -> f64 {
        let mean = self.mean();
        let mut var = 0.0;
        for x in self.data.iter() {
            var += (x - mean).powi(2);
        }
        var / self.len() as f64
    }

    pub fn std(&self) -> f64 {
        self.var().sqrt()
    }
}
