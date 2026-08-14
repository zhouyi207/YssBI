use super::DatabaseEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDecl {
    pub id: String,
    pub engine: DatabaseEngine,
    pub schema_version: u32,
    pub required: bool,
    /// 显示名称（unique name），导入时由后端生成，用于 EditorView 与 DataViewer 同步
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_declaration_requires_name() {
        let declaration = DatabaseDecl {
            id: "sales".into(),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: false,
            name: "Sales".into(),
        };
        let mut value = serde_json::to_value(declaration).unwrap();
        value.as_object_mut().unwrap().remove("name");

        let error = serde_json::from_value::<DatabaseDecl>(value).unwrap_err();

        assert!(error.to_string().contains("missing field `name`"));
    }
}
