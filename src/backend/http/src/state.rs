use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};

use serde_json::Value;

use crate::config::HttpShellConfig;

#[derive(Debug, Clone)]
pub struct HttpAppState {
    pub config: HttpShellConfig,
    llm_runtime_configs: Arc<Mutex<HashMap<i64, Value>>>,
}

impl HttpAppState {
    pub fn new(config: HttpShellConfig) -> Result<Self, Infallible> {
        Ok(Self {
            config,
            llm_runtime_configs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn get_llm_runtime_config(&self, user_id: i64) -> Option<Value> {
        self.llm_runtime_configs
            .lock()
            .ok()
            .and_then(|configs| configs.get(&user_id).cloned())
    }

    pub fn set_llm_runtime_config(&self, user_id: i64, config: Value) {
        if let Ok(mut configs) = self.llm_runtime_configs.lock() {
            configs.insert(user_id, config);
        }
    }

    pub fn clear_llm_runtime_config(&self, user_id: i64) {
        if let Ok(mut configs) = self.llm_runtime_configs.lock() {
            configs.remove(&user_id);
        }
    }
}
