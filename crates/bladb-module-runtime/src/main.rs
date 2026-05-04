use bladb_module_runtime::{AdapterRegistry, ModuleRuntimeApp, ModuleRuntimeConfig};
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
            let app = ModuleRuntimeApp::from_config(config, AdapterRegistry::new(vec![]))
                .map_err(|error| error.to_string())?;
            let rendered = serde_json::to_string_pretty(&app.status_json())
                .map_err(|error| error.to_string())?;
            println!("{rendered}");
            Ok(())
        }
        Some("serve") | None => {
            let config = load_config(args.get(1).map(String::as_str))?;
            let app = ModuleRuntimeApp::from_config(config, AdapterRegistry::new(vec![]))
                .map_err(|error| error.to_string())?;
            let status = app.service().status();
            let server = app.transport_server();
            Err(format!(
                "module runtime bootstrap is ready for cluster `{}` on `{}` with subject `{:?}` (max_batch={}, idle_sleep_ms={}), but a concrete transport binding still needs to provide ModuleRpcInbox",
                status.cluster,
                status.bind_addr,
                status.transport_subject,
                server.loop_config().max_batch,
                server.loop_config().idle_sleep_ms
            ))
        }
        _ => Err(
            "usage: bladb-module-runtime [serve [config.yaml|config.json] | describe [config.yaml|config.json]]"
                .into(),
        ),
    }
}

fn load_config(path: Option<&str>) -> Result<ModuleRuntimeConfig, String> {
    match path {
        Some(path) => ModuleRuntimeConfig::from_path(path).map_err(|error| error.to_string()),
        None => ModuleRuntimeConfig::from_env().map_err(|error| error.to_string()),
    }
}
