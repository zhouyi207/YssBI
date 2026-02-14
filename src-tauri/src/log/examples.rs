//! 日志系统使用示例
//! 
//! 这个文件展示了如何使用新的日志宏系统

#![allow(dead_code)]

use crate::log::{log_app, log_exec, log_sys};

/// 示例：应用程序日志
fn example_app_logs() {
    // 使用不同级别的日志
    log_app::trace!("This is a trace message");
    log_app::debug!("Debug information: value = {}", 42);
    log_app::info!("Application started successfully");
    log_app::warn!("Warning: configuration file not found, using defaults");
    log_app::error!("Error: failed to connect to server");

    // 格式化多个参数
    let user = "Alice";
    let action = "login";
    log_app::info!("User {} performed action: {}", user, action);

    // 使用复杂表达式
    let items = vec![1, 2, 3, 4, 5];
    log_app::debug!("Processing {} items: {:?}", items.len(), items);
}

/// 示例：执行日志
fn example_exec_logs() {
    let node_id = "node_123";
    let node_type = "Add";

    log_exec::info!("Starting execution of node: {} (type: {})", node_id, node_type);
    
    // 模拟执行过程
    log_exec::debug!("Fetching input values for node {}", node_id);
    log_exec::debug!("Computing result...");
    
    // 成功情况
    log_exec::info!("Node {} executed successfully in 15ms", node_id);
    
    // 错误情况
    log_exec::error!("Node {} execution failed: division by zero", node_id);
    
    // 警告情况
    log_exec::warn!("Node {} execution slow: took 5000ms", node_id);
}

/// 示例：系统日志
fn example_sys_logs() {
    // 数据库操作
    log_sys::info!("Initializing database connection");
    log_sys::debug!("Database connection pool size: {}", 10);
    log_sys::info!("Database connected successfully");
    
    // 文件操作
    let file_path = "/path/to/project.json";
    log_sys::info!("Loading project from: {}", file_path);
    log_sys::warn!("Project file is large: {} MB", 150);
    
    // 系统错误
    log_sys::error!("Failed to allocate memory: out of memory");
    
    // 资源管理
    log_sys::debug!("Memory usage: {} MB", 512);
    log_sys::debug!("CPU usage: {}%", 75);
}

/// 示例：在函数中使用日志
fn process_data(data: &[i32]) -> Result<i32, String> {
    log_app::debug!("process_data called with {} items", data.len());
    
    if data.is_empty() {
        log_app::warn!("process_data received empty data");
        return Err("Empty data".to_string());
    }
    
    log_app::trace!("Computing sum of data");
    let sum: i32 = data.iter().sum();
    
    log_app::info!("Data processed successfully, sum = {}", sum);
    Ok(sum)
}

/// 示例：在错误处理中使用日志
fn handle_error_example() {
    match risky_operation() {
        Ok(result) => {
            log_app::info!("Operation succeeded: {}", result);
        }
        Err(e) => {
            log_app::error!("Operation failed: {}", e);
            // 可以继续处理错误...
        }
    }
}

fn risky_operation() -> Result<String, String> {
    Err("Something went wrong".to_string())
}

/// 示例：在循环中使用日志
fn loop_example() {
    let items = vec!["item1", "item2", "item3"];
    
    log_app::info!("Processing {} items", items.len());
    
    for (i, item) in items.iter().enumerate() {
        log_app::debug!("Processing item {}: {}", i + 1, item);
        
        // 模拟处理
        if item.len() > 10 {
            log_app::warn!("Item {} is too long: {} characters", i + 1, item.len());
        }
    }
    
    log_app::info!("All items processed");
}

/// 示例：条件日志
fn conditional_logging(verbose: bool) {
    log_app::info!("Function started");
    
    if verbose {
        log_app::debug!("Verbose mode enabled");
        log_app::trace!("Detailed trace information...");
    }
    
    log_app::info!("Function completed");
}

/// 示例：性能日志
fn performance_logging() {
    use std::time::Instant;
    
    let start = Instant::now();
    log_exec::info!("Starting heavy computation");
    
    // 模拟耗时操作
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    let duration = start.elapsed();
    log_exec::info!("Computation completed in {:?}", duration);
    
    if duration.as_millis() > 1000 {
        log_exec::warn!("Computation took longer than expected: {:?}", duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_examples() {
        // 注意：这些测试需要日志管理器已初始化
        example_app_logs();
        example_exec_logs();
        example_sys_logs();
        
        let _ = process_data(&[1, 2, 3, 4, 5]);
        handle_error_example();
        loop_example();
        conditional_logging(true);
        performance_logging();
    }
}
