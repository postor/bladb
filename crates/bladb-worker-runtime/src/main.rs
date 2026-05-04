use bladb_worker_runtime::{StepExecutorRegistry, WorkerRuntimeApp, WorkerRuntimeConfig};
use std::{env, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("describe") => {
            let config = load_config(args.get(1).map(String::as_str))?;
            let app = WorkerRuntimeApp::from_config(config, StepExecutorRegistry::new(vec![]))
                .map_err(|error| error.to_string())?;
            let rendered = serde_json::to_string_pretty(&app.status_json())
                .map_err(|error| error.to_string())?;
            println!("{rendered}");
            Ok(())
        }
        Some("serve") | None => {
            let config = load_config(args.get(1).map(String::as_str))?;
            let app = WorkerRuntimeApp::from_config(config, StepExecutorRegistry::new(vec![]))
                .map_err(|error| error.to_string())?;
            let status = app.service().status();
            let consumer = app.transport_consumer();
            Err(format!(
                "worker runtime bootstrap is ready for worker `{}` with metrics on `{}` and subject `{:?}` (max_batch={}, idle_sleep_ms={}), but a concrete transport binding still needs to provide WorkerRuntimeTransport",
                status.worker,
                status.metrics_bind_addr,
                status.trigger_subject,
                consumer.loop_config().max_batch,
                consumer.loop_config().idle_sleep_ms
            ))
        }
        _ => Err(
            "usage: bladb-worker-runtime [serve [config.yaml|config.json] | describe [config.yaml|config.json]]"
                .into(),
        ),
    }
}

fn load_config(path: Option<&str>) -> Result<WorkerRuntimeConfig, String> {
    match path {
        Some(path) => WorkerRuntimeConfig::from_path(path).map_err(|error| error.to_string()),
        None => WorkerRuntimeConfig::from_env().map_err(|error| error.to_string()),
    }
}
