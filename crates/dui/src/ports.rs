use crate::Reply;
use serde_json::Value;
use std::path::Path;

pub trait TypeOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply;
}

pub struct CaptureBatchJob<'a> {
    pub spec: &'a Value,
    pub input: &'a Value,
    pub width: u32,
    pub image: &'a Path,
}

pub trait Renderer {
    fn capture(
        &mut self,
        spec: &Value,
        input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>>;

    fn capture_batch(&mut self, jobs: &[CaptureBatchJob<'_>]) -> Vec<Result<Value, String>> {
        jobs.iter()
            .map(|job| {
                self.capture(job.spec, job.input, job.width, job.image)
                    .map_err(|e| e.to_string())
            })
            .collect()
    }
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
