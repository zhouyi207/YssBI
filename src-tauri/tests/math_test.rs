use yss_math::array::Array;

#[test]
fn test_array_sum() {
    let arr = Array::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(arr.sum(), 15.0);
}

#[test]
fn test_array_mean() {
    let arr = Array::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(arr.mean(), 3.0);
}

#[test]
fn test_array_var() {
    let arr = Array::from_vec(vec![2.0, 4.0, 6.0, 8.0, 10.0]);
    // mean = 6.0
    // var = ((2-6)^2 + (4-6)^2 + (6-6)^2 + (8-6)^2 + (10-6)^2) / 5
    // var = (16 + 4 + 0 + 4 + 16) / 5 = 40 / 5 = 8.0
    assert_eq!(arr.var(), 8.0);
}

#[test]
fn test_array_std() {
    let arr = Array::from_vec(vec![2.0, 4.0, 6.0, 8.0, 10.0]);
    // std = sqrt(8.0) ≈ 2.828...
    let expected_std = 8.0_f64.sqrt();
    assert!((arr.std() - expected_std).abs() < 1e-10);
}

#[test]
fn test_array_stats_single_value() {
    let arr = Array::from_vec(vec![5.0]);
    assert_eq!(arr.sum(), 5.0);
    assert_eq!(arr.mean(), 5.0);
    assert_eq!(arr.var(), 0.0);
    assert_eq!(arr.std(), 0.0);
}

#[test]
fn test_array_stats_negative_values() {
    let arr = Array::from_vec(vec![-2.0, -1.0, 0.0, 1.0, 2.0]);
    assert_eq!(arr.sum(), 0.0);
    assert_eq!(arr.mean(), 0.0);
    assert_eq!(arr.var(), 2.0);
}
