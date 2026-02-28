use yssbi_lib::database::{
    DatabaseAccess, DatabaseDecl, DatabaseEngine, DatabaseInstance, DatabaseState, DatabaseView,
};

/// 测试使用 DatabaseEngine::Csv 读取 iris.csv 文件
#[test]
fn test_read_iris_csv() {
    println!("\n=== 测试读取 iris.csv 文件 ===");

    // 1. 创建 CSV 数据库引擎配置
    let engine = DatabaseEngine::Csv {
        path: "tests/data/iris.csv".to_string(),
        delimiter: ',',
        has_header: true,
        infer_schema_length: Some(100),
    };

    println!("创建 CSV 引擎配置:");
    println!("  路径: tests/data/iris.csv");
    println!("  分隔符: ','");
    println!("  包含表头: true");

    // 2. 创建数据库声明
    let decl = DatabaseDecl {
        id: "iris_dataset".to_string(),
        engine: engine.clone(),
        schema_version: 1,
        required: true,
        name: Some("iris".to_string()),
    };

    println!("\n创建数据库声明:");
    println!("  ID: {}", decl.id);
    println!("  Schema 版本: {}", decl.schema_version);

    // 3. 构建 LazyFrame
    let lazy_frame = engine
        .build_lazy()
        .expect("Failed to build lazy frame from CSV");

    println!("\n成功构建 LazyFrame");

    // 4. 创建数据库实例
    let mut db_instance = DatabaseInstance {
        decl,
        state: DatabaseState::Lazy { lazy_frame },
    };

    println!("\n创建数据库实例 (Lazy 状态)");

    // 5. 获取预览数据（前 100 行）
    println!("\n=== 获取预览数据 ===");
    let preview_df = db_instance
        .access(DatabaseAccess::Execution)
        .expect("Failed to get preview");

    println!("预览数据 (前 10 行):");
    println!("{:?}", preview_df.dataframe);
}
