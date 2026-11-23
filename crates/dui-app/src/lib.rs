//! Application shell and port-driven use cases for `dui`.
//!
//! `dui-app` depends ONLY on `dui-core` and `dui-ports` — never on concrete storage,
//! compiler, host, or renderer adapters.

pub mod audit;
pub mod cache;
pub mod comments;
pub mod compiler_oracle;
pub mod diff;
pub mod graph_view;
pub mod inspect;

use dui_ports::{Reply, failure};
use std::{fs, path::Path};

pub fn read_json(path: &Path) -> Reply {
    let bytes = fs::read(path).map_err(|e| failure(2, &format!("{}: {e}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|e| failure(2, &format!("{}: {e}", path.display())))
}
