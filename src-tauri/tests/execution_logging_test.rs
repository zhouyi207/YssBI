use yssbi_lib::project::ProjectData;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_execution_log_creation() {
    // 创建测试项目数据
    let mut project = ProjectData::new();
    project.update_metadata();
    
    // 序列化为 JSON
    let json = project.to_json().expect("Failed to serialize project");
    
    // 验证 JSON 包含必要字段
    assert!(json.contains("globalVariables"));
    assert!(json.contains("events"));
    assert!(json.contains("functions"));
    assert!(json.contains("macros"));
    assert!(json.contains("metadata"));
    assert!(json.contains("exportTime"));
}

#[test]
fn test_logs_directory_creation() {
    // 测试 logs 目录可以被创建
    let logs_dir = PathBuf::from("test_logs");
    
    // 清理可能存在的测试目录
    if logs_dir.exists() {
        fs::remove_dir_all(&logs_dir).ok();
    }
    
    // 创建目录
    fs::create_dir_all(&logs_dir).expect("Failed to create test logs directory");
    
    // 验证目录存在
    assert!(logs_dir.exists());
    assert!(logs_dir.is_dir());
    
    // 清理
    fs::remove_dir_all(&logs_dir).ok();
}

#[test]
fn test_timestamp_format() {
    use chrono::Utc;
    
    // 测试时间戳格式
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("execution_{}.json", timestamp);
    
    // 验证文件名格式
    assert!(filename.starts_with("execution_"));
    assert!(filename.ends_with(".json"));
    assert!(filename.len() > 20); // execution_YYYYMMDD_HHMMSS.json
}
