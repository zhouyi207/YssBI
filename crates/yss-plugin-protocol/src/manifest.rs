use crate::{PROTOCOL_MAJOR, PROTOCOL_MINOR, PluginFailure, ResourceBudget};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub publisher: String,
    pub version: String,
    pub host_api: String,
    pub protocol: ProtocolRange,
    pub target: String,
    pub executable: String,
    pub execution: ExecutionMode,
    pub contributes: Contributions,
    pub permissions: Vec<String>,
    pub ui_methods: Vec<String>,
    pub resource_budget: ResourceBudget,
    #[serde(default)]
    pub cache_directories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolRange {
    pub major: u32,
    pub min_minor: u32,
    pub max_minor: u32,
    pub required_features: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionMode {
    TrustedNative,
    SandboxRequired,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contributions {
    pub views: Vec<PluginView>,
    pub commands: Vec<PluginCommand>,
    pub task_types: Vec<TaskType>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginView {
    pub id: String,
    pub title: String,
    pub entry: String,
    pub location: ViewLocation,
    pub scope: ViewScope,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ViewScope {
    Application,
    Project,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskType {
    pub id: String,
    pub produces_artifacts: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ViewLocation {
    Sidebar,
    Editor,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub size: String,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageSignature {
    pub key_id: String,
    pub public_key: String,
    pub signature: String,
}

pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
        })
        && !value.contains("..")
}
pub fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() < 512
        && value.is_ascii()
        && value.bytes().all(|byte| {
            byte >= b' ' && !matches!(byte, b'\\' | b':' | b'<' | b'>' | b'"' | b'|' | b'?' | b'*')
        })
        && value.split('/').all(|part| {
            let stem = part
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            !part.is_empty()
                && !matches!(part, "." | "..")
                && !part.ends_with(['.', ' '])
                && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
        })
}

impl PluginManifest {
    pub fn validate(&self) -> Result<(), PluginFailure> {
        let invalid = || PluginFailure::new("plugin_manifest_invalid");
        if self.cache_directories.len() > 16 {
            return Err(invalid());
        }
        let mut directories = Vec::<String>::with_capacity(self.cache_directories.len());
        for path in &self.cache_directories {
            if !valid_relative_path(path) {
                return Err(invalid());
            }
            let path = path.to_ascii_lowercase();
            if directories.iter().any(|other| {
                path == *other
                    || path
                        .strip_prefix(other.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
                    || other
                        .strip_prefix(path.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
            }) {
                return Err(invalid());
            }
            directories.push(path);
        }
        if self.schema_version != 1
            || self.protocol.major != PROTOCOL_MAJOR
            || self.protocol.min_minor > PROTOCOL_MINOR
            || self.protocol.min_minor > self.protocol.max_minor
            || !self.protocol.required_features.is_empty()
        {
            return Err(PluginFailure::new("plugin_protocol_incompatible"));
        }
        if !valid_id(&self.id)
            || !valid_id(&self.publisher)
            || !self.id.starts_with(&format!("{}.", self.publisher))
            || self.name.is_empty()
            || self.name.len() > 128
            || self.description.len() > 1024
            || self.target.len() > 96
            || !valid_relative_path(&self.executable)
            || semver::Version::parse(&self.version).is_err()
            || !semver::VersionReq::parse(&self.host_api)
                .map_err(|_| invalid())?
                .matches(&semver::Version::new(1, 0, 0))
        {
            return Err(invalid());
        }
        if self.contributes.views.len() > 16
            || self.contributes.commands.len() > 64
            || self.contributes.task_types.len() > 32
            || self.permissions.len() > 32
            || self.ui_methods.len() > 64
        {
            return Err(invalid());
        }
        let mut ids = BTreeSet::new();
        for view in &self.contributes.views {
            if !valid_id(&view.id)
                || !ids.insert(&view.id)
                || !valid_relative_path(&view.entry)
                || !view.entry.ends_with(".view.json")
                || view.title.is_empty()
                || view.title.len() > 128
            {
                return Err(invalid());
            }
        }
        ids.clear();
        for command in &self.contributes.commands {
            if !valid_id(&command.id)
                || !ids.insert(&command.id)
                || command.title.is_empty()
                || command.title.len() > 128
            {
                return Err(invalid());
            }
        }
        ids.clear();
        if self
            .contributes
            .task_types
            .iter()
            .any(|task| !valid_id(&task.id) || !ids.insert(&task.id))
            || self
                .permissions
                .iter()
                .chain(&self.ui_methods)
                .any(|value| !valid_id(value))
        {
            return Err(invalid());
        }
        self.resource_budget.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_type_identity_cannot_select_conflicting_artifact_rules_by_order() {
        let mut manifest = manifest();
        assert!(manifest.validate().is_ok());

        manifest.contributes.task_types[1].id = "statistics.run".into();
        for _ in 0..2 {
            assert_eq!(
                manifest.validate().unwrap_err().code,
                "plugin_manifest_invalid"
            );
            manifest.contributes.task_types.reverse();
        }
    }

    fn manifest() -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "id": "example.statistics",
            "name": "Statistics",
            "description": "",
            "publisher": "example",
            "version": "1.0.0",
            "hostApi": "^1",
            "protocol": {
                "major": PROTOCOL_MAJOR,
                "minMinor": 0,
                "maxMinor": PROTOCOL_MINOR,
                "requiredFeatures": []
            },
            "target": "x86_64-pc-windows-msvc",
            "executable": "bin/plugin.exe",
            "execution": "trustedNative",
            "contributes": {
                "views": [],
                "commands": [],
                "taskTypes": [
                    { "id": "statistics.run", "producesArtifacts": false },
                    { "id": "statistics.export", "producesArtifacts": true }
                ]
            },
            "permissions": ["results.write"],
            "uiMethods": ["tasks.start"],
            "resourceBudget": ResourceBudget::default()
        }))
        .unwrap()
    }

    #[test]
    fn portable_paths_reject_reserved_filename_characters_and_controls() {
        for character in ['<', '>', ':', '"', '\\', '|', '?', '*']
            .into_iter()
            .chain((0..32).map(|code| char::from_u32(code).unwrap()))
        {
            let path = format!("bin/plugin{character}.exe");
            assert!(!valid_relative_path(&path), "accepted {path:?}");
            let mut manifest = manifest();
            manifest.executable = path;
            assert_eq!(
                manifest.validate().unwrap_err().code,
                "plugin_manifest_invalid"
            );
        }
        assert!(valid_relative_path("bin/plugin-name_2.exe"));
        assert!(valid_relative_path("views/my view.html"));
    }

    #[test]
    fn cache_directory_overlap_uses_portable_case_insensitive_identity() {
        for directories in [
            ["Cache", "cache"],
            ["Cache", "cache/results"],
            ["Cache/results", "cache"],
        ] {
            let mut manifest = manifest();
            manifest.cache_directories = directories.map(str::to_owned).into();
            assert_eq!(
                manifest.validate().unwrap_err().code,
                "plugin_manifest_invalid"
            );
        }
        let mut manifest = manifest();
        manifest.cache_directories = ["Cache", "cache-results"].map(str::to_owned).into();
        assert!(manifest.validate().is_ok());
    }
}
