use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

pub trait BackgroundTask: Send + Sync + std::fmt::Debug {
  fn process(&self) -> Result<(), Box<dyn std::error::Error>>;
  fn payload(&self) -> Json;
}

