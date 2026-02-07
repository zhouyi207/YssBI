use crate::array::Array;
use statrs::distribution::{ContinuousCDF, StudentsT};

/// 单样本 t 检验结果
#[derive(Debug, Clone)]
pub struct OneSampleTTestResult {
    pub t_statistic: f64,
    pub df: f64,           // 自由度
    pub p_value: f64,      // 双侧 p 值
    pub mean: f64,
    pub std_error: f64,    // 标准误
}

/// 独立样本 t 检验结果
#[derive(Debug, Clone)]
pub struct TwoSampleTTestResult {
    pub t_statistic: f64,
    pub df: f64,
    pub p_value: f64,
    pub mean1: f64,
    pub mean2: f64,
    pub std_error: f64,
}

/// 配对样本 t 检验结果
#[derive(Debug, Clone)]
pub struct PairedTTestResult {
    pub t_statistic: f64,
    pub df: f64,
    pub p_value: f64,
    pub mean_diff: f64,
    pub std_error: f64,
}

/// 单样本 t 检验
/// 
/// 检验样本均值是否与给定的总体均值 mu 有显著差异
/// 
/// # 参数
/// * `sample` - 样本数据
/// * `mu` - 假设的总体均值
/// 
/// # 返回
/// 返回 t 检验结果，包含 t 统计量、自由度、p 值等
pub fn one_sample_ttest(sample: &Array, mu: f64) -> OneSampleTTestResult {
    let n = sample.len() as f64;
    let mean = sample.mean();
    let std = sample.std();
    
    // 标准误 = std / sqrt(n)
    let std_error = std / n.sqrt();
    
    // t = (mean - mu) / std_error
    let t_statistic = (mean - mu) / std_error;
    
    // 自由度 = n - 1
    let df = n - 1.0;
    
    // 计算双侧 p 值
    let t_dist = StudentsT::new(0.0, 1.0, df).unwrap();
    let p_value = 2.0 * (1.0 - t_dist.cdf(t_statistic.abs()));
    
    OneSampleTTestResult {
        t_statistic,
        df,
        p_value,
        mean,
        std_error,
    }
}

/// 独立样本 t 检验（假设方差相等）
/// 
/// 检验两个独立样本的均值是否有显著差异
/// 
/// # 参数
/// * `sample1` - 第一组样本
/// * `sample2` - 第二组样本
/// 
/// # 返回
/// 返回 t 检验结果
pub fn two_sample_ttest(sample1: &Array, sample2: &Array) -> TwoSampleTTestResult {
    let n1 = sample1.len() as f64;
    let n2 = sample2.len() as f64;
    
    let mean1 = sample1.mean();
    let mean2 = sample2.mean();
    
    let var1 = sample1.var();
    let var2 = sample2.var();
    
    // 合并方差 (pooled variance)
    let pooled_var = ((n1 - 1.0) * var1 + (n2 - 1.0) * var2) / (n1 + n2 - 2.0);
    
    // 标准误
    let std_error = (pooled_var * (1.0 / n1 + 1.0 / n2)).sqrt();
    
    // t 统计量
    let t_statistic = (mean1 - mean2) / std_error;
    
    // 自由度
    let df = n1 + n2 - 2.0;
    
    // 计算双侧 p 值
    let t_dist = StudentsT::new(0.0, 1.0, df).unwrap();
    let p_value = 2.0 * (1.0 - t_dist.cdf(t_statistic.abs()));
    
    TwoSampleTTestResult {
        t_statistic,
        df,
        p_value,
        mean1,
        mean2,
        std_error,
    }
}

/// 独立样本 t 检验（Welch's t-test，不假设方差相等）
/// 
/// 使用 Welch-Satterthwaite 方程计算自由度
/// 
/// # 参数
/// * `sample1` - 第一组样本
/// * `sample2` - 第二组样本
/// 
/// # 返回
/// 返回 t 检验结果
pub fn welch_ttest(sample1: &Array, sample2: &Array) -> TwoSampleTTestResult {
    let n1 = sample1.len() as f64;
    let n2 = sample2.len() as f64;
    
    let mean1 = sample1.mean();
    let mean2 = sample2.mean();
    
    let var1 = sample1.var();
    let var2 = sample2.var();
    
    // 标准误
    let std_error = (var1 / n1 + var2 / n2).sqrt();
    
    // t 统计量
    let t_statistic = (mean1 - mean2) / std_error;
    
    // Welch-Satterthwaite 自由度
    let numerator = (var1 / n1 + var2 / n2).powi(2);
    let denominator = (var1 / n1).powi(2) / (n1 - 1.0) + (var2 / n2).powi(2) / (n2 - 1.0);
    let df = numerator / denominator;
    
    // 计算双侧 p 值
    let t_dist = StudentsT::new(0.0, 1.0, df).unwrap();
    let p_value = 2.0 * (1.0 - t_dist.cdf(t_statistic.abs()));
    
    TwoSampleTTestResult {
        t_statistic,
        df,
        p_value,
        mean1,
        mean2,
        std_error,
    }
}

/// 配对样本 t 检验
/// 
/// 检验配对样本的差值均值是否为 0
/// 
/// # 参数
/// * `sample1` - 第一组样本
/// * `sample2` - 第二组样本（必须与 sample1 长度相同）
/// 
/// # 返回
/// 返回 t 检验结果
/// 
/// # Panics
/// 如果两个样本长度不同会 panic
pub fn paired_ttest(sample1: &Array, sample2: &Array) -> PairedTTestResult {
    assert_eq!(
        sample1.len(),
        sample2.len(),
        "配对样本必须有相同的长度"
    );
    
    // 计算差值
    let diff_data: Vec<f64> = sample1
        .data()
        .iter()
        .zip(sample2.data().iter())
        .map(|(a, b)| a - b)
        .collect();
    
    // 使用单样本 t 检验检验差值是否为 0
    let diff_array = Array::from_vec(diff_data);
    let result = one_sample_ttest(&diff_array, 0.0);
    
    PairedTTestResult {
        t_statistic: result.t_statistic,
        df: result.df,
        p_value: result.p_value,
        mean_diff: result.mean,
        std_error: result.std_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_sample_ttest() {
        let data = vec![2.0, 3.0, 4.0, 5.0, 6.0];
        let sample = Array::from_vec(data);
        
        let result = one_sample_ttest(&sample, 3.0);
        
        assert_eq!(result.mean, 4.0);
        assert!(result.t_statistic > 0.0);
        assert_eq!(result.df, 4.0);
        assert!(result.p_value > 0.0 && result.p_value < 1.0);
        println!("单样本 t 检验: t={:.4}, df={:.0}, p={:.4}", 
                 result.t_statistic, result.df, result.p_value);
    }

    #[test]
    fn test_two_sample_ttest() {
        let data1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let data2 = vec![2.0, 3.0, 4.0, 5.0, 6.0];
        
        let sample1 = Array::from_vec(data1);
        let sample2 = Array::from_vec(data2);
        
        let result = two_sample_ttest(&sample1, &sample2);
        
        assert_eq!(result.mean1, 3.0);
        assert_eq!(result.mean2, 4.0);
        assert!(result.t_statistic < 0.0);
        assert_eq!(result.df, 8.0);
        assert!(result.p_value > 0.0 && result.p_value < 1.0);
        println!("独立样本 t 检验: t={:.4}, df={:.0}, p={:.4}", 
                 result.t_statistic, result.df, result.p_value);
    }

    #[test]
    fn test_welch_ttest() {
        let data1 = vec![1.0, 2.0, 3.0];
        let data2 = vec![4.0, 5.0, 6.0, 7.0, 8.0];
        
        let sample1 = Array::from_vec(data1);
        let sample2 = Array::from_vec(data2);
        
        let result = welch_ttest(&sample1, &sample2);
        
        assert!(result.t_statistic < 0.0);
        assert!(result.df > 0.0);
        assert!(result.p_value > 0.0 && result.p_value < 1.0);
        println!("Welch t 检验: t={:.4}, df={:.2}, p={:.4}", 
                 result.t_statistic, result.df, result.p_value);
    }

    #[test]
    fn test_paired_ttest() {
        // 前后测试数据
        let before = vec![100.0, 105.0, 110.0, 115.0, 120.0];
        let after = vec![102.0, 108.0, 112.0, 118.0, 125.0];
        
        let sample1 = Array::from_vec(before);
        let sample2 = Array::from_vec(after);
        
        let result = paired_ttest(&sample1, &sample2);
        
        // 差值均值应该是负数（after > before）
        assert!(result.mean_diff < 0.0);
        assert_eq!(result.df, 4.0);
        assert!(result.p_value > 0.0 && result.p_value < 1.0);
        println!("配对样本 t 检验: t={:.4}, df={:.0}, p={:.4}, mean_diff={:.2}", 
                 result.t_statistic, result.df, result.p_value, result.mean_diff);
    }
}
