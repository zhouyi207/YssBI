use faer::Col;

pub fn skewness_kurtosis(x: &Col<f64>) -> (f64, f64) {
    assert!(x.nrows() > 0, "cannot compute moments of an empty sample");
    let n = x.nrows() as f64;
    let mean = x.iter().sum::<f64>() / n;

    let mut m2 = 0.0;
    let mut m3 = 0.0;
    let mut m4 = 0.0;

    for &v in x.iter() {
        let d = v - mean;
        m2 += d.powi(2);
        m3 += d.powi(3);
        m4 += d.powi(4);
    }

    m2 /= n;
    m3 /= n;
    m4 /= n;

    let skew = m3 / m2.powf(1.5);
    let kurt = m4 / m2.powi(2); // Pearson kurtosis

    (skew, kurt)
}
