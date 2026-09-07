//! 时间序列统一时间类型
//!
//! 支持数字时间与日期时间，用于 lag / align 等严格按时间对齐的操作。

use chrono::NaiveDate;

/// 统一时间值：数字时间或日期
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum TimeValue {
    /// 数字时间（如 t=1,2,3 或 20200101）
    Num(i64),
    /// 日期时间
    Date(NaiveDate),
}

impl TimeValue {
    /// 按间隔步进
    pub fn add_interval(&self, interval: i64) -> Self {
        match self {
            TimeValue::Num(v) => TimeValue::Num(v + interval),
            TimeValue::Date(d) => TimeValue::Date(*d + chrono::Duration::days(interval)),
        }
    }

    /// 计算与另一时间点的差值（用于 interval 推导）
    pub fn diff(&self, other: &Self) -> f64 {
        match (self, other) {
            (TimeValue::Num(a), TimeValue::Num(b)) => (*a - *b) as f64,
            (TimeValue::Date(a), TimeValue::Date(b)) => {
                a.signed_duration_since(*b).num_days() as f64
            }
            _ => 0.0,
        }
    }
}
