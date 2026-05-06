use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub type TransportHandler = Arc<dyn Fn(Value) -> Result<Value, String> + Send + Sync + 'static>;

pub trait ServerModuleTransport: Send + Sync {
    fn subscribe(&self, subject: String, handler: TransportHandler) -> Result<(), String>;
}

#[derive(Clone, Default)]
pub struct InMemoryServerModuleTransport {
    handlers: Arc<Mutex<HashMap<String, TransportHandler>>>,
}

impl InMemoryServerModuleTransport {
    pub fn list_subjects(&self) -> Vec<String> {
        let Ok(handlers) = self.handlers.lock() else {
            return vec![];
        };
        let mut subjects = handlers.keys().cloned().collect::<Vec<_>>();
        subjects.sort();
        subjects
    }

    pub fn request(&self, subject: &str, payload: Value) -> Result<Value, String> {
        let handler = self
            .handlers
            .lock()
            .map_err(|_| "server module transport lock poisoned".to_string())?
            .get(subject)
            .cloned()
            .ok_or_else(|| format!("missing handler for {subject}"))?;

        handler(payload)
    }
}

impl ServerModuleTransport for InMemoryServerModuleTransport {
    fn subscribe(&self, subject: String, handler: TransportHandler) -> Result<(), String> {
        self.handlers
            .lock()
            .map_err(|_| "server module transport lock poisoned".to_string())?
            .insert(subject, handler);
        Ok(())
    }
}

pub fn create_in_memory_server_module_transport() -> InMemoryServerModuleTransport {
    InMemoryServerModuleTransport::default()
}
