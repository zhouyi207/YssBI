//! Emit production survival plot payloads for wire validation and manual previews.
use std::time::{Duration, Instant};
use yss_sci::survival::{cox, evaluation};
use yss_sci_contract::{execution::*, survival::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/survival_category_reference.json"
    ))?;
    let time: Vec<f64> = serde_json::from_value(f["data"]["time"].clone())?;
    let event: Vec<f64> = serde_json::from_value(f["data"]["event"].clone())?;
    let x: Vec<Vec<f64>> = serde_json::from_value(f["data"]["predictors"].clone())?;
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let model = cox::fit(&time, &event, &x, CoxOptions::default(), &control)?;
    let risk = cox::event_probabilities(&model, 2.0)?;
    let payload = serde_json::json!({"payloads":[
        {"name":"Cox nomogram","chart":"nomogram","data":evaluation::nomogram(&model,2.0,5,&control)?},
        {"name":"Survival calibration","chart":"line","data":evaluation::calibration(&time,&event,&risk,2.0,5,&control)?},
        {"name":"Survival decision curve","chart":"line","data":evaluation::decision_curve(&time,&event,&risk,DecisionOptions { horizon:2.0,minimum_threshold:0.01,maximum_threshold:0.6,points:60 },&control)?}
    ]});
    let json = serde_json::to_string_pretty(&payload)? + "\n";
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, json)?;
    } else {
        println!("{json}");
    }
    Ok(())
}
