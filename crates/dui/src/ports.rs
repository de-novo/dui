use crate::Reply;
use serde_json::Value;
use std::path::Path;

pub trait TypeOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply;
}

pub trait Renderer {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>>;
}

pub trait Host {
    type Session;
    fn start(
        &self,
        project: &Path,
        manifest: &Path,
        facts: &Value,
    ) -> Result<Self::Session, (u8, Value)>;
}
