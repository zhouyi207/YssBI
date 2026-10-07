//! Run reproducible Harness tasks against an isolated ledger and the real Application.

#[path = "measure_harness/concurrency.rs"]
mod concurrency;
#[path = "measure_harness/fixture.rs"]
mod fixture;
#[path = "measure_harness/measurement.rs"]
mod measurement;
#[path = "measure_harness/model_calls.rs"]
mod model_calls;
#[path = "measure_harness/runtime.rs"]
mod runtime;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;
use yss_application::ApplicationState;
use yss_harness_contract::{
    HarnessSessionRecord, HarnessTurnOptions, LanguageModelSelection, PrincipalId,
};
use yss_project_identity::OperationId;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    settings_dir: PathBuf,
    output_dir: PathBuf,
    project_path: Option<PathBuf>,
    dataset_csv: Option<PathBuf>,
    #[serde(default)]
    seed_regression_graph: bool,
    #[serde(default)]
    inject_database_conflict: bool,
    model: Option<LanguageModelSelection>,
    #[serde(default)]
    options: HarnessTurnOptions,
    max_duration_seconds: Option<u64>,
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Task {
    label: String,
    prompt_path: PathBuf,
    #[serde(default)]
    new_conversation: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [config_path] = args.as_slice() else {
        return Err("usage: pnpm measure:harness <configuration.json>".into());
    };
    let config_path = fs::canonicalize(config_path)?;
    let base = config_path.parent().ok_or("configuration has no parent")?;
    let config: Config = serde_json::from_slice(&fs::read(&config_path)?)?;
    if config.tasks.is_empty() || config.max_duration_seconds == Some(0) {
        return Err("provide tasks and a positive optional duration".into());
    }
    if config.seed_regression_graph
        && (config.dataset_csv.is_none() || config.project_path.is_some())
    {
        return Err("seedRegressionGraph requires datasetCsv and a new measurement project".into());
    }
    if config.inject_database_conflict
        && (config.dataset_csv.is_none() || config.project_path.is_some())
    {
        return Err(
            "injectDatabaseConflict requires datasetCsv and a new measurement project".into(),
        );
    }
    let prompts = config
        .tasks
        .iter()
        .map(|task| {
            let path = resolve(base, &task.prompt_path);
            if path.extension().is_none_or(|extension| extension != "md") {
                return Err("task prompts must be Markdown files".into());
            }
            Ok(fs::read_to_string(path)?)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let output_dir = resolve(base, &config.output_dir);
    if let Some(parent) = output_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&output_dir)?;
    copy_settings(&resolve(base, &config.settings_dir), &output_dir)?;
    let requested_project = config.project_path.as_ref().map(|path| resolve(base, path));
    let dataset_csv = config.dataset_csv.as_ref().map(|path| resolve(base, path));
    let fixture_dir = output_dir.clone();
    let seed_regression_graph = config.seed_regression_graph;
    let (application, project_path) = tokio::task::spawn_blocking(move || {
        prepare_application(
            &fixture_dir,
            requested_project,
            dataset_csv,
            seed_regression_graph,
        )
    })
    .await??;
    let (host, schemas, calls) = runtime::initialize(
        application.clone(),
        output_dir.clone(),
        config.inject_database_conflict,
    )
    .await?;
    fs::write(
        output_dir.join("run.json"),
        serde_json::to_vec_pretty(&json!({
            "projectPath": project_path,
            "options": config.options,
            "requestedModel": config.model,
            "maxDurationSeconds": config.max_duration_seconds,
            "seedRegressionGraph": config.seed_regression_graph,
            "injectDatabaseConflict": config.inject_database_conflict,
            "schemaMetric": "Compact UTF-8 JSON input schemas per AgentDriver admission; excludes tool descriptions and provider envelopes. Not cumulative request bytes.",
            "resultMetric": "Compact JSON of the public capability result or failure projection; Unicode scalar characters and UTF-8 bytes. Control outcomes are recorded separately.",
            "duplicateMetric": "Successful inspect calls with identical public arguments and identical public results within one task; retries are repeated arguments after a failed invocation.",
            "timingMetric": "Turn wall time and ledger start/end include binding and scheduling. Tool totals may overlap; the difference is not pure model time."
        }))?,
    )?;
    let mut session: Option<HarnessSessionRecord> = None;
    let mut failed_turns = 0;
    for (index, (task, prompt)) in config.tasks.iter().zip(prompts).enumerate() {
        if session.is_none() || task.new_conversation {
            // Session creation uses the same captured project binding as the desktop.
            session = Some(
                application
                    .create_harness_session(&host, PrincipalId::try_new("harness-measurement")?)
                    .await?,
            );
        }
        let session = session.as_ref().ok_or("missing session")?;
        let task_dir = output_dir.join(format!("{:02}", index + 1));
        fs::create_dir(&task_dir)?;
        fs::write(task_dir.join("prompt.md"), &prompt)?;
        let previous = host.events_after(&session.id, 0).await?;
        let sequence = previous.last().map_or(0, |event| event.sequence);
        let schema_offset = schemas.lock().unwrap_or_else(|e| e.into_inner()).len();
        let call_offset = calls.lock().unwrap_or_else(|e| e.into_inner()).len();
        println!("Starting task {}: {}", index + 1, task.label);
        let started = Instant::now();
        let work = host.submit_turn(
            &session.id,
            &session.project,
            prompt,
            vec![],
            config.model.clone(),
            config.options,
        );
        tokio::pin!(work);
        let mut cancelled = false;
        let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
        let outcome = loop {
            tokio::select! {
                result = &mut work => break result,
                _ = heartbeat.tick() => {
                    let elapsed = started.elapsed();
                    println!("Task {} elapsed {} s", index + 1, elapsed.as_secs());
                    if !cancelled && config.max_duration_seconds.is_some_and(|limit| elapsed.as_secs() >= limit) {
                        cancelled = host.cancel_turn(&session.id);
                    }
                }
            }
        };
        let schemas = schemas.lock().unwrap_or_else(|e| e.into_inner())[schema_offset..].to_vec();
        let model_calls = calls.lock().unwrap_or_else(|e| e.into_inner())[call_offset..].to_vec();
        measurement::write(
            &host,
            session,
            sequence,
            &task_dir,
            measurement::TaskMeasurement {
                label: &task.label,
                elapsed: started.elapsed(),
                outcome: &outcome,
                schemas,
                model_calls,
            },
        )
        .await?;
        if outcome.is_err() {
            failed_turns += 1;
        }
        println!(
            "Task {} finished: {}",
            index + 1,
            if outcome.is_ok() {
                "completed"
            } else {
                "failed"
            }
        );
    }
    println!("Measurements: {}", output_dir.display());
    if failed_turns != 0 {
        return Err(format!("{failed_turns} measured turns failed; see task summaries").into());
    }
    Ok(())
}

fn resolve(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

fn copy_settings(source: &Path, output: &Path) -> Result<(), Error> {
    let mut settings: serde_json::Value =
        serde_json::from_slice(&fs::read(source.join("language-models.json"))?)?;
    // The isolated reader must never clean up credentials retired by the desktop.
    settings
        .as_object_mut()
        .ok_or("invalid model settings")?
        .insert("retiredCredentials".into(), json!([]));
    fs::create_dir(output.join("settings"))?;
    fs::write(
        output.join("settings/language-models.json"),
        serde_json::to_vec_pretty(&settings)?,
    )?;
    Ok(())
}

fn prepare_application(
    output: &Path,
    project_path: Option<PathBuf>,
    dataset_csv: Option<PathBuf>,
    seed_regression_graph: bool,
) -> Result<(ApplicationState, PathBuf), Error> {
    let project_path = match project_path {
        Some(path) => path,
        None => {
            yss_project::ProjectState::new()
                .create_project_transaction(
                    "Harness measurement",
                    &output.join("project"),
                    OperationId::new(),
                )?
                .metadata_path
        }
    };
    let application = ApplicationState::initialize()?;
    application
        .load_project_for_application(project_path.to_str().ok_or("project path is not UTF-8")?)?;
    if let Some(path) = dataset_csv {
        let captured = application.capture_session()?;
        let instance = captured.project_instance_id().clone();
        drop(captured);
        let imported = application.load_database_for_application(
            instance,
            OperationId::new(),
            yss_database_contract::DatabaseImportSource::Csv {
                path: path.to_str().ok_or("CSV path is not UTF-8")?.to_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(1000),
            },
            Some("Synthetic regression".into()),
        )?;
        if seed_regression_graph {
            fixture::regression_graph(&application, &imported.data.id)?;
        }
    }
    Ok((application, project_path))
}
