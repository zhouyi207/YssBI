//! 执行期数据缓存
//!
//! 在图执行过程中，节点可能产生中间 DataFrame / Series 结果。
//! `ExecutionDataStore` 以 copy-on-write 语义存储这些中间产物，
//! 避免修改原始数据，同时允许下游节点通过 ID 引用中间结果。

use polars::prelude::{DataFrame, Series};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// 执行期临时数据存储
///
/// 生命周期与单次图执行对齐：
/// - 执行开始时创建（空）
/// - 节点产出中间结果时写入
/// - 下游节点通过 ID 读取
/// - 执行结束后丢弃
pub struct ExecutionDataStore {
    dataframes: HashMap<String, Arc<DataFrame>>,
    series: HashMap<String, Series>,
    /// 通用不透明句柄存储（Struct 类型值的实际对象）
    handles: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl ExecutionDataStore {
    pub fn new() -> Self {
        Self {
            dataframes: HashMap::new(),
            series: HashMap::new(),
            handles: HashMap::new(),
        }
    }

    /// 按 ID 获取缓存的 DataFrame
    pub fn get_dataframe(&self, id: &str) -> Option<Arc<DataFrame>> {
        self.dataframes.get(id).cloned()
    }

    /// 存入中间 DataFrame，返回生成的 UUID 引用 ID
    pub fn put_dataframe(&mut self, df: DataFrame) -> String {
        let id = format!("exec_{}", Uuid::new_v4());
        self.dataframes.insert(id.clone(), Arc::new(df));
        id
    }

    /// 以指定 ID 存入 DataFrame（用于将原始数据注入缓存）
    pub fn put_dataframe_with_id(&mut self, id: String, df: Arc<DataFrame>) {
        self.dataframes.insert(id, df);
    }

    /// 按 ID 获取缓存的 Series
    pub fn get_series(&self, id: &str) -> Option<&Series> {
        self.series.get(id)
    }

    /// 存入中间 Series，返回生成的 UUID 引用 ID
    pub fn put_series(&mut self, s: Series) -> String {
        let id = format!("series_{}", Uuid::new_v4());
        self.series.insert(id.clone(), s);
        id
    }

    /// 以指定 ID 存入 Series
    pub fn put_series_with_id(&mut self, id: String, s: Series) {
        self.series.insert(id, s);
    }

    // ========================================================================
    // 通用句柄存储（Struct 类型）
    // ========================================================================

    /// 存入不透明对象，返回生成的句柄 ID
    pub fn put_handle<T: Any + Send + Sync + 'static>(&mut self, value: T) -> String {
        self.put_handle_arc(Arc::new(value))
    }

    /// 存入已装箱的不透明对象
    pub fn put_handle_boxed(&mut self, value: Box<dyn Any + Send + Sync>) -> String {
        self.put_handle_arc(Arc::from(value))
    }

    /// 存入 Arc 包装的不透明对象
    pub fn put_handle_arc(&mut self, value: Arc<dyn Any + Send + Sync>) -> String {
        let id = format!("handle_{}", Uuid::new_v4());
        self.handles.insert(id.clone(), value);
        id
    }

    /// 按 ID 获取 Arc 句柄（可跨 Mutex 安全传递）
    pub fn get_handle(&self, id: &str) -> Option<Arc<dyn Any + Send + Sync>> {
        self.handles.get(id).cloned()
    }

    /// 按 ID 获取句柄并 downcast 为具体类型
    pub fn get_handle_as<T: Any + Send + Sync>(&self, id: &str) -> Option<Arc<T>> {
        self.handles
            .get(id)
            .and_then(|v| v.clone().downcast::<T>().ok())
    }

    /// 清除所有缓存
    pub fn clear(&mut self) {
        self.dataframes.clear();
        self.series.clear();
        self.handles.clear();
    }
}
