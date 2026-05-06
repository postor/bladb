use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

use crate::transport::ServerModuleTransport;

pub type ServerModuleHandler =
    Arc<dyn Fn(ServerModuleInvocation) -> Result<Value, String> + Send + Sync + 'static>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerModuleInvocation {
    pub app: String,
    pub module: String,
    pub method: String,
    #[serde(default)]
    pub input: Option<Value>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub db: Option<Value>,
    #[serde(default)]
    pub meta: Option<Value>,
}

pub trait ServerModuleRegistry: Send + Sync {
    fn list_methods_for_module(&self, module_name: &str) -> Vec<String>;
    fn invoke(&self, invocation: ServerModuleInvocation) -> Result<Value, String>;
}

#[derive(Clone, Default)]
pub struct StaticServerModuleRegistry {
    modules: HashMap<String, HashMap<String, ServerModuleHandler>>,
}

impl StaticServerModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_handler(
        mut self,
        module_name: impl Into<String>,
        method: impl Into<String>,
        handler: ServerModuleHandler,
    ) -> Self {
        self.modules
            .entry(module_name.into())
            .or_default()
            .insert(method.into(), handler);
        self
    }
}

impl ServerModuleRegistry for StaticServerModuleRegistry {
    fn list_methods_for_module(&self, module_name: &str) -> Vec<String> {
        let Some(methods) = self.modules.get(module_name) else {
            return vec![];
        };

        let mut listed = methods.keys().cloned().collect::<Vec<_>>();
        listed.sort();
        listed
    }

    fn invoke(&self, invocation: ServerModuleInvocation) -> Result<Value, String> {
        let Some(module) = self.modules.get(&invocation.module) else {
            return Err(format!("unknown server module `{}`", invocation.module));
        };
        let Some(handler) = module.get(&invocation.method) else {
            return Err(format!(
                "server module `{}` does not export method `{}`",
                invocation.module, invocation.method
            ));
        };

        handler(invocation)
    }
}

pub struct CreateServerModuleLauncherOptions {
    pub app: String,
    pub transport: Arc<dyn ServerModuleTransport>,
    pub registry: Arc<dyn ServerModuleRegistry>,
    pub modules: Vec<String>,
}

pub trait ServerModuleLauncher: Send + Sync {
    fn start(&self) -> Result<Vec<String>, String>;
}

pub struct StaticServerModuleLauncher {
    app: String,
    transport: Arc<dyn ServerModuleTransport>,
    registry: Arc<dyn ServerModuleRegistry>,
    modules: Vec<String>,
}

impl ServerModuleLauncher for StaticServerModuleLauncher {
    fn start(&self) -> Result<Vec<String>, String> {
        let mut subjects = Vec::new();
        for module_name in &self.modules {
            for method in self.registry.list_methods_for_module(module_name) {
                let subject = subject_for_server_module(&self.app, module_name, &method);
                let registry = Arc::clone(&self.registry);
                self.transport.subscribe(
                    subject.clone(),
                    Arc::new(move |payload| {
                        let invocation: ServerModuleInvocation =
                            serde_json::from_value(payload).map_err(|error| error.to_string())?;
                        match registry.invoke(invocation.clone()) {
                            Ok(data) => Ok(json!({
                                "ok": true,
                                "data": data,
                                "requestId": invocation.request_id,
                            })),
                            Err(message) => Ok(json!({
                                "ok": false,
                                "error": {
                                    "code": "SERVER_MODULE_ERROR",
                                    "message": message,
                                    "module": invocation.module,
                                    "method": invocation.method,
                                },
                                "requestId": invocation.request_id,
                            })),
                        }
                    }),
                )?;
                subjects.push(subject);
            }
        }

        subjects.sort();
        Ok(subjects)
    }
}

pub struct StartedServerModules {
    pub subjects: Vec<String>,
}

pub fn create_server_module_launcher(
    options: CreateServerModuleLauncherOptions,
) -> Arc<dyn ServerModuleLauncher> {
    Arc::new(StaticServerModuleLauncher {
        app: options.app,
        transport: options.transport,
        registry: options.registry,
        modules: options.modules,
    })
}

pub fn start_server_modules(
    options: CreateServerModuleLauncherOptions,
) -> Result<StartedServerModules, String> {
    let launcher = create_server_module_launcher(options);
    let subjects = launcher.start()?;
    Ok(StartedServerModules { subjects })
}

pub fn subject_for_server_module(app: &str, module_name: &str, method: &str) -> String {
    format!("bladb.app.{app}.module.{module_name}.{method}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create_in_memory_server_module_transport;

    #[test]
    fn registers_subjects_and_invokes_handlers() {
        let transport = Arc::new(create_in_memory_server_module_transport());
        let registry = Arc::new(StaticServerModuleRegistry::new().register_handler(
            "user",
            "login",
            Arc::new(|invocation| {
                let email = invocation
                    .input
                    .and_then(|value| value.get("email").cloned())
                    .and_then(|value| value.as_str().map(str::to_string))
                    .ok_or_else(|| "missing email".to_string())?;
                Ok(json!({ "email": email, "token": "session-demo" }))
            }),
        ));

        let started = start_server_modules(CreateServerModuleLauncherOptions {
            app: "demo".into(),
            transport: transport.clone(),
            registry,
            modules: vec!["user".into()],
        })
        .expect("start rust server modules");

        assert_eq!(
            started.subjects,
            vec!["bladb.app.demo.module.user.login".to_string()]
        );

        let response = transport
            .request(
                "bladb.app.demo.module.user.login",
                json!({
                    "app": "demo",
                    "module": "user",
                    "method": "login",
                    "input": {
                        "email": "member@example.com"
                    }
                }),
            )
            .expect("invoke in-memory transport");

        assert_eq!(response["ok"], true);
        assert_eq!(response["data"]["email"], "member@example.com");
    }
}
