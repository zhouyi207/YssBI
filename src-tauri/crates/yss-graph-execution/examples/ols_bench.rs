//! OLS measurement using the first 100,000 rows of the data-engine benchmark fixture.
use std::time::{Duration, Instant};
use yss_sci_contract::scientific::{
    OlsRequest, ScientificCancellationToken, ScientificExecutionControl,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let predictor = (0..100_000)
        .map(|row| row as f64 / 1000.0)
        .collect::<Vec<_>>();
    let response = predictor
        .iter()
        .map(|x| 3.0 + 2.0 * x + (x * 0.2).sin())
        .collect();
    let start = Instant::now();
    let result = yss_sci_runtime::ols(
        OlsRequest {
            response,
            predictors: vec![predictor],
            options: Default::default(),
        },
        &ScientificExecutionControl {
            cancellation: ScientificCancellationToken::new(),
            deadline: Instant::now() + Duration::from_secs(300),
        },
    )?;
    println!(
        "{}",
        serde_json::json!({
            "scenario": "ols_matrix",
            "elapsed_ms": start.elapsed().as_secs_f64() * 1000.0,
            "detail": {"rows": result.fitted.len(), "coefficients": result.coefficients},
        })
    );
    assert_eq!(result.fitted.len(), 100_000);
    Ok(())
}
