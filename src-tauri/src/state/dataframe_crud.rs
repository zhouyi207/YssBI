//! DataFrame CRUD 操作

use polars::prelude::*;

use super::project_state::ProjectState;
use crate::project::DataFrameData;

impl ProjectState {
    // ==================== DataFrame 操作 ====================

    pub fn add_dataframe(
        &self,
        id: String,
        df: DataFrame,
        source_path: Option<String>,
    ) -> Result<DataFrameData, String> {
        let row_count = df.height();
        let column_count = df.width();

        // 获取列信息
        let columns = df
            .get_columns()
            .iter()
            .map(|col| crate::project::DataFrameColumn {
                name: col.name().to_string(),
                column_type: format!("{:?}", col.dtype()),
            })
            .collect();

        // 生成预览数据 (前 100 行)
        let preview_df = df.head(Some(100));
        let mut rows = Vec::new();

        // 转换预览数据为 JSON
        for i in 0..preview_df.height() {
            let mut row = Vec::new();
            for col_idx in 0..preview_df.width() {
                let val = preview_df.get_columns()[col_idx].get(i).unwrap();
                let json_val = match val {
                    AnyValue::Null => serde_json::Value::Null,
                    AnyValue::Boolean(b) => serde_json::Value::Bool(b),
                    AnyValue::String(s) => serde_json::Value::String(s.to_string()),
                    AnyValue::StringOwned(s) => serde_json::Value::String(s.to_string()),
                    AnyValue::Int8(v) => serde_json::json!(v),
                    AnyValue::Int16(v) => serde_json::json!(v),
                    AnyValue::Int32(v) => serde_json::json!(v),
                    AnyValue::Int64(v) => serde_json::json!(v),
                    AnyValue::UInt8(v) => serde_json::json!(v),
                    AnyValue::UInt16(v) => serde_json::json!(v),
                    AnyValue::UInt32(v) => serde_json::json!(v),
                    AnyValue::UInt64(v) => serde_json::json!(v),
                    AnyValue::Float32(v) => serde_json::json!(v),
                    AnyValue::Float64(v) => serde_json::json!(v),
                    _ => serde_json::Value::String(format!("{:?}", val)),
                };
                row.push(json_val);
            }
            rows.push(row);
        }

        let df_data = DataFrameData {
            id: id.clone(),
            name: id.clone(), // 默认使用 ID 作为名称
            columns,
            rows,
            row_count,
            column_count,
            source_path,
        };

        // 存入内存 store
        self.df_store.write().unwrap().insert(id.clone(), df);

        // 更新项目数据
        self.data
            .write()
            .unwrap()
            .dataframes
            .insert(id, df_data.clone());

        Ok(df_data)
    }

    pub fn get_dataframe(&self, id: &str) -> Option<DataFrame> {
        self.df_store.read().unwrap().get(id).cloned()
    }

    pub fn delete_dataframe(&self, id: &str) -> Result<(), String> {
        self.df_store.write().unwrap().remove(id);
        self.data.write().unwrap().dataframes.remove(id);
        Ok(())
    }

    pub fn create_dataframe(
        &self,
        id: String,
        data: DataFrameData,
    ) -> Result<DataFrameData, String> {
        let mut project = self.data.write().unwrap();
        if project.dataframes.contains_key(&id) {
            return Err(format!("DataFrame with id '{}' already exists", id));
        }
        project.dataframes.insert(id, data.clone());
        Ok(data)
    }
}
