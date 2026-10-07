fn main() {
    use yss_plugin_protocol::*;
    let schema = serde_json::json!({
        "InstalledPlugin":schemars::schema_for!(InstalledPlugin),
        "PackageInspection":schemars::schema_for!(PackageInspection),
        "ViewSession":schemars::schema_for!(ViewSession),
        "TaskSnapshot":schemars::schema_for!(TaskSnapshot),
        "TaskHistoryPage":schemars::schema_for!(TaskHistoryPage),
        "PluginDiagnostic":schemars::schema_for!(PluginDiagnostic),
        "PluginStorageUsage":schemars::schema_for!(PluginStorageUsage)
    });
    println!(
        "{}",
        serde_json::to_string(&schema).expect("serialize protocol schema")
    );
}
