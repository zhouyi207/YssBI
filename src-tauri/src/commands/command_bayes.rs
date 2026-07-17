use serde_json::{json, Value};

use crate::error::AppError;

#[tauri::command]
pub fn parse_bayes_expression(input: Value) -> Result<Value, AppError> {
    let formula = input
        .get("formula")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if formula.is_empty() {
        return Err(AppError::new("bayes_formula_empty", "Formula is required"));
    }

    Ok(json!({
        "formula": {
            "formulaText": formula,
            "rawPredictor": null
        },
        "symbols": collect_symbols(formula)
    }))
}

#[tauri::command]
pub fn validate_bayes_model(input: Value) -> Result<Value, AppError> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if input.get("dataset").is_none() || input.get("dataset").is_some_and(Value::is_null) {
        errors.push(issue("DATASET_REQUIRED", "请选择数据源。", "dataset"));
    }
    if input.get("responseBinding").is_none() || input.get("responseBinding").is_some_and(Value::is_null) {
        errors.push(issue("RESPONSE_REQUIRED", "请选择响应变量列。", "responseBinding"));
    }
    if input
        .get("formulaText")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        errors.push(issue("FORMULA_REQUIRED", "请输入模型方程。", "formulaText"));
    }
    if input.get("boundPredictor").is_none() || input.get("boundPredictor").is_some_and(Value::is_null) {
        errors.push(issue("PREDICTOR_REQUIRED", "预测表达式尚未解析或绑定。", "boundPredictor"));
    }
    if input
        .get("parameters")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        warnings.push(issue("NO_PARAMETERS", "当前模型尚未识别出未知参数。", "parameters"));
    }

    Ok(json!({
        "ok": errors.is_empty(),
        "errors": errors,
        "warnings": warnings
    }))
}

#[tauri::command]
pub fn submit_bayes_inference(input: Value) -> Result<Value, AppError> {
    let report = validate_bayes_model(input)?;
    if !report.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(AppError::new("bayes_validation_failed", "Bayesian model validation failed"));
    }
    let task_id = format!("bayes-{}", chrono::Utc::now().timestamp_millis());
    Ok(json!({
        "taskId": task_id,
        "status": "completed",
        "result": {
            "taskId": task_id,
            "summaryPath": "memory://bayes-summary.json"
        }
    }))
}

#[tauri::command]
pub fn get_bayes_inference_status(task_id: String) -> Result<Value, AppError> {
    Ok(json!({
        "taskId": task_id,
        "status": "completed",
        "result": {
            "taskId": task_id,
            "summaryPath": "memory://bayes-summary.json"
        }
    }))
}

#[tauri::command]
pub fn cancel_bayes_inference(_task_id: String) -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
pub fn read_bayes_inference_result(_task_id: String) -> Result<Value, AppError> {
    Ok(json!({
        "summaries": [],
        "diagnostics": {
            "chains": 0,
            "drawsPerChain": 0,
            "warmup": 0,
            "divergences": 0,
            "maxTreedepthHits": 0,
            "warnings": [{
                "code": "BAYES_ENGINE_NOT_IMPLEMENTED",
                "message": "Bayesian Julia inference engine is not implemented yet."
            }]
        }
    }))
}

fn issue(code: &str, message: &str, path: &str) -> Value {
    json!({
        "code": code,
        "severity": if code.starts_with("NO_") { "warning" } else { "error" },
        "message": message,
        "path": path
    })
}

fn collect_symbols(formula: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    let mut current = String::new();
    for character in formula.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            current.push(character);
            continue;
        }
        push_symbol(&mut symbols, &mut current);
    }
    push_symbol(&mut symbols, &mut current);
    symbols.sort();
    symbols.dedup();
    symbols
}

fn push_symbol(symbols: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    if current.chars().next().is_some_and(|character| character.is_ascii_alphabetic())
        && !matches!(current.as_str(), "Normal" | "BernoulliLogit" | "PoissonLog")
    {
        symbols.push(current.clone());
    }
    current.clear();
}
