// 这个玩意最好不要使用，因为编辑器会默认严格定义类型，因此让只能使用 ndarray 来执行就好了

use ndarray::{Array1, Array2, ArrayView1, ArrayView2};

// 定义 1D 2D array like input

pub trait ArrayLike1D {
    fn as_view(&self) -> ArrayView1<'_, f64>;
}

pub trait ArrayLike2D {
    fn as_view(&self) -> ArrayView2<'_, f64>;
}

// 给 ndarray 实现 ArrayLike1D 和 ArrayLike2D  trait

impl ArrayLike1D for Array1<f64> {
    fn as_view(&self) -> ArrayView1<'_, f64> {
        self.view()
    }
}

impl ArrayLike2D for Array2<f64> {
    fn as_view(&self) -> ArrayView2<'_, f64> {
        self.view()
    }
}

// 给 vec 实现 ArrayLike1D 和 ArrayLike2D  trait

impl ArrayLike1D for Vec<f64> {
    fn as_view(&self) -> ArrayView1<'_, f64> {
        ArrayView1::from(self)
    }
}

// @deprecated: use Array2<f64> instead
impl ArrayLike2D for Vec<Vec<f64>> {
    fn as_view(&self) -> ArrayView2<'_, f64> {
        let rows = self.len();
        let cols = self.first().map(|r| r.len()).unwrap_or(0);
        let flat: Vec<f64> = self.iter().flat_map(|r| r.iter()).copied().collect();
        let array =
            Array2::from_shape_vec((rows, cols), flat).expect("Invalid Vec<Vec<f64>> shape");
        // Leak the boxed array to produce a static view
        Box::leak(Box::new(array)).view()
    }
}
