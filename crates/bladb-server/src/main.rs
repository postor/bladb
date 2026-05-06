use bladb_server::{
    start_http_server_modules, ServerModuleHandler, StartHttpServerModulesOptions,
    StaticServerModuleRegistry,
};
use serde_json::{json, Value};
use std::{env, sync::Arc, thread, time::Duration};

fn main() {
    let app = env::var("BLADB_SERVER_APP").unwrap_or_else(|_| "user-module-demo".into());
    let host = env::var("BLADB_SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = env::var("BLADB_SERVER_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8791);

    let registry = Arc::new(StaticServerModuleRegistry::new().register_handler(
        "user",
        "health",
        Arc::new(|_| -> Result<Value, String> { Ok(json!({ "ok": true })) }) as ServerModuleHandler,
    ));

    let (transport, subjects) = start_http_server_modules(StartHttpServerModulesOptions {
        app,
        host,
        port,
        registry,
        modules: vec!["user".into()],
    })
    .unwrap_or_else(|error| panic!("failed to start rust server launcher: {error}"));

    println!(
        "Bladb rust server modules listening on {}",
        transport.base_url()
    );
    for subject in subjects {
        println!("- {subject}");
    }

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
