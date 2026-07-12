pub const OLS_ZH: &str = include_str!("zh/ols.md");
pub const OLS_EN: &str = include_str!("en/ols.md");
pub const OLS_SUMMARY_ZH: &str = include_str!("zh/ols_summary.md");
pub const OLS_SUMMARY_EN: &str = include_str!("en/ols_summary.md");
pub const OLS_CONFIGURE_ZH: &str = include_str!("zh/ols_configure.md");
pub const OLS_CONFIGURE_EN: &str = include_str!("en/ols_configure.md");
pub const OLS_FIXED_SCALE_CONFIG_ZH: &str = include_str!("zh/ols_fixed_scale_config.md");
pub const OLS_FIXED_SCALE_CONFIG_EN: &str = include_str!("en/ols_fixed_scale_config.md");
pub const OLS_CLUSTER_CONFIG_ZH: &str = include_str!("zh/ols_cluster_config.md");
pub const OLS_CLUSTER_CONFIG_EN: &str = include_str!("en/ols_cluster_config.md");
pub const OLS_HAC_CONFIG_ZH: &str = include_str!("zh/ols_hac_config.md");
pub const OLS_HAC_CONFIG_EN: &str = include_str!("en/ols_hac_config.md");
pub const OLS_NEWEY_CONFIG_ZH: &str = include_str!("zh/ols_newey_config.md");
pub const OLS_NEWEY_CONFIG_EN: &str = include_str!("en/ols_newey_config.md");
pub const VCE_NONROBUST_ZH: &str = include_str!("zh/vce_nonrobust.md");
pub const VCE_NONROBUST_EN: &str = include_str!("en/vce_nonrobust.md");
pub const VCE_HC0_ZH: &str = include_str!("zh/vce_hc0.md");
pub const VCE_HC0_EN: &str = include_str!("en/vce_hc0.md");
pub const VCE_HC1_ZH: &str = include_str!("zh/vce_hc1.md");
pub const VCE_HC1_EN: &str = include_str!("en/vce_hc1.md");
pub const VCE_HC2_ZH: &str = include_str!("zh/vce_hc2.md");
pub const VCE_HC2_EN: &str = include_str!("en/vce_hc2.md");
pub const VCE_HC3_ZH: &str = include_str!("zh/vce_hc3.md");
pub const VCE_HC3_EN: &str = include_str!("en/vce_hc3.md");

/// VCE constant node documentation by struct key.
pub fn vce_documentation(struct_key: &str) -> Option<(&'static str, &'static str)> {
    Some(match struct_key {
        "VCENonRobust" => (VCE_NONROBUST_ZH, VCE_NONROBUST_EN),
        "VCEHC0" => (VCE_HC0_ZH, VCE_HC0_EN),
        "VCEHC1" => (VCE_HC1_ZH, VCE_HC1_EN),
        "VCEHC2" => (VCE_HC2_ZH, VCE_HC2_EN),
        "VCEHC3" => (VCE_HC3_ZH, VCE_HC3_EN),
        _ => return None,
    })
}
