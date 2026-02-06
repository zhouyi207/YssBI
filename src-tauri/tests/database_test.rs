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
        .get_preview(10)
        .expect("Failed to get preview");

    println!("预览数据 (前 10 行):");
    println!("{}", preview_df);

    // 验证数据结构
    assert_eq!(preview_df.width(), 5, "Iris 数据集应该有 5 列");
    assert!(preview_df.height() <= 10, "预览数据不应超过 10 行");

    // 6. 使用 Preview 访问模式
    println!("\n=== 使用 Preview 访问模式 ===");
    let preview_view = db_instance
        .access(DatabaseAccess::Preview)
        .expect("Failed to access preview");

    match preview_view {
        DatabaseView::Preview {
            rows,
            row_count,
            column_count,
        } => {
            println!("预览视图信息:");
            println!("  行数: {}", row_count);
            println!("  列数: {}", column_count);
            println!("  前 3 行数据:");
            for (i, row) in rows.iter().take(3).enumerate() {
                println!("    行 {}: {:?}", i, row.cells);
            }

            assert_eq!(column_count, 5, "应该有 5 列");
            assert!(row_count <= 100, "预览不应超过 100 行");
        }
        _ => panic!("Expected Preview view"),
    }

    // 7. 加载完整数据到内存
    println!("\n=== 加载完整数据到内存 ===");
    let full_df = db_instance
        .ensure_loaded()
        .expect("Failed to load full dataframe");

    println!("完整数据集信息:");
    println!("  总行数: {}", full_df.height());
    println!("  总列数: {}", full_df.width());
    println!("  列名: {:?}", full_df.get_column_names());

    // 验证 Iris 数据集的基本属性
    assert_eq!(full_df.width(), 5, "Iris 数据集应该有 5 列");
    assert_eq!(full_df.height(), 150, "Iris 数据集应该有 150 行");

    // 8. 使用 Execution 访问模式
    println!("\n=== 使用 Execution 访问模式 ===");
    let exec_view = db_instance
        .access(DatabaseAccess::Execution)
        .expect("Failed to access execution");

    match exec_view {
        DatabaseView::Execution { dataframe } => {
            println!("执行视图信息:");
            println!("  数据框行数: {}", dataframe.height());
            println!("  数据框列数: {}", dataframe.width());

            assert_eq!(dataframe.height(), 150, "应该有 150 行");
            assert_eq!(dataframe.width(), 5, "应该有 5 列");
        }
        _ => panic!("Expected Execution view"),
    }

    println!("\n=== 测试完成 ===");
}