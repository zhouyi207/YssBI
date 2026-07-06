
pub fn apply_docs(mut def: crate::graph::node::NodeDefinition, name: &str) -> crate::graph::node::NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Get DataFrame" => (GET_DF_ZH, GET_DF_EN),
        "Decompose DataFrame" => (DECOMPOSE_DF_ZH, DECOMPOSE_DF_EN),
        "Combine DataFrame" => (COMBINE_DF_ZH, COMBINE_DF_EN),
        "Filter DataFrame" => (FILTER_DF_ZH, FILTER_DF_EN),
        "Standardize DataSeries" => (STANDARDIZE_ZH, STANDARDIZE_EN),
        "Inverse Standardize DataSeries" => (INV_STANDARDIZE_ZH, INV_STANDARDIZE_EN),
        "Add Dummy Info" => (ADD_DUMMY_INFO_ZH, ADD_DUMMY_INFO_EN),
        _ => return None,
    })
}

pub const GET_DF_ZH: &str = include_str!("zh/get_dataframe.md");
pub const GET_DF_EN: &str = include_str!("en/get_dataframe.md");
pub const DECOMPOSE_DF_ZH: &str = include_str!("zh/decompose_dataframe.md");
pub const DECOMPOSE_DF_EN: &str = include_str!("en/decompose_dataframe.md");
pub const COMBINE_DF_ZH: &str = include_str!("zh/combine_dataframe.md");
pub const COMBINE_DF_EN: &str = include_str!("en/combine_dataframe.md");
pub const FILTER_DF_ZH: &str = include_str!("zh/filter_dataframe.md");
pub const FILTER_DF_EN: &str = include_str!("en/filter_dataframe.md");
pub const STANDARDIZE_ZH: &str = include_str!("zh/standardize_dataseries.md");
pub const STANDARDIZE_EN: &str = include_str!("en/standardize_dataseries.md");
pub const INV_STANDARDIZE_ZH: &str = include_str!("zh/inverse_standardize_dataseries.md");
pub const INV_STANDARDIZE_EN: &str = include_str!("en/inverse_standardize_dataseries.md");
pub const ADD_DUMMY_INFO_ZH: &str = include_str!("zh/add_dummy_info.md");
pub const ADD_DUMMY_INFO_EN: &str = include_str!("en/add_dummy_info.md");
