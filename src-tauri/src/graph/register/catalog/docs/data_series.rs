pub const GET_DATASERIES_ZH: &str = include_str!("zh/get_dataseries.md");
pub const GET_DATASERIES_EN: &str = include_str!("en/get_dataseries.md");
pub const INT_RANGE_ZH: &str = include_str!("zh/int_range.md");
pub const INT_RANGE_EN: &str = include_str!("en/int_range.md");
pub const DATASERIES_LENGTH_ZH: &str = include_str!("zh/dataseries_length.md");
pub const DATASERIES_LENGTH_EN: &str = include_str!("en/dataseries_length.md");
pub const DATASERIES_SUM_ZH: &str = include_str!("zh/dataseries_sum.md");
pub const DATASERIES_SUM_EN: &str = include_str!("en/dataseries_sum.md");
pub const DATASERIES_MEAN_ZH: &str = include_str!("zh/dataseries_mean.md");
pub const DATASERIES_MEAN_EN: &str = include_str!("en/dataseries_mean.md");
pub const DATASERIES_GT_ZH: &str = include_str!("zh/dataseries_gt.md");
pub const DATASERIES_GT_EN: &str = include_str!("en/dataseries_gt.md");
pub const DATASERIES_LT_ZH: &str = include_str!("zh/dataseries_lt.md");
pub const DATASERIES_LT_EN: &str = include_str!("en/dataseries_lt.md");
pub const DATASERIES_GTE_ZH: &str = include_str!("zh/dataseries_gte.md");
pub const DATASERIES_GTE_EN: &str = include_str!("en/dataseries_gte.md");
pub const DATASERIES_LTE_ZH: &str = include_str!("zh/dataseries_lte.md");
pub const DATASERIES_LTE_EN: &str = include_str!("en/dataseries_lte.md");
pub const DATASERIES_EQ_ZH: &str = include_str!("zh/dataseries_eq.md");
pub const DATASERIES_EQ_EN: &str = include_str!("en/dataseries_eq.md");
pub const DATASERIES_NEQ_ZH: &str = include_str!("zh/dataseries_neq.md");
pub const DATASERIES_NEQ_EN: &str = include_str!("en/dataseries_neq.md");


pub fn compare_documentation(node_name: &str) -> Option<(&'static str, &'static str)> {
    Some(match node_name {
        "DataSeries Greater Than (>)" => (DATASERIES_GT_ZH, DATASERIES_GT_EN),
        "DataSeries Less Than (<)" => (DATASERIES_LT_ZH, DATASERIES_LT_EN),
        "DataSeries Greater Equal (>=)" => (DATASERIES_GTE_ZH, DATASERIES_GTE_EN),
        "DataSeries Less Equal (<=)" => (DATASERIES_LTE_ZH, DATASERIES_LTE_EN),
        "DataSeries Equal (==)" => (DATASERIES_EQ_ZH, DATASERIES_EQ_EN),
        "DataSeries Not Equal (!=)" => (DATASERIES_NEQ_ZH, DATASERIES_NEQ_EN),
        _ => return None,
    })
}
