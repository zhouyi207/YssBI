use yss_node_registry::RegistryFingerprint;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAnalysisBasis {
    pub registry_fingerprint: RegistryFingerprint,
    pub kernel_fingerprint: [u8; 32],
}
