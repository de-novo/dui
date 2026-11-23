use dui_core::compiler_oracle::{ToolchainKind, ToolchainSupport, check_toolchain_version};
use serde_json::{Value, json};
use std::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolchainProbe {
    pub(crate) kind: ToolchainKind,
    pub(crate) binary: &'static str,
    pub(crate) available: bool,
    pub(crate) version: Option<String>,
    pub(crate) support: ToolchainSupport,
}

fn toolchain_command(kind: ToolchainKind) -> (&'static str, &'static str) {
    match kind {
        ToolchainKind::TypeScript => ("tsc", "--version"),
        ToolchainKind::Rustc => ("rustc", "--version"),
        ToolchainKind::Go => ("go", "version"),
        ToolchainKind::Swiftc => ("swiftc", "--version"),
        ToolchainKind::Kotlinc => ("kotlinc", "-version"),
        ToolchainKind::Dart => ("dart", "--version"),
    }
}

pub(crate) fn probe_toolchain(kind: ToolchainKind) -> ToolchainProbe {
    let (binary, arg) = toolchain_command(kind);
    let output = Command::new(binary).arg(arg).output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            let raw = if !stdout.is_empty() { stdout } else { stderr };
            let first_line = raw.lines().next().unwrap_or("").trim().to_owned();
            let support = check_toolchain_version(kind, &first_line);
            ToolchainProbe {
                kind,
                binary,
                available: true,
                version: (!first_line.is_empty()).then_some(first_line),
                support,
            }
        }
        Ok(_) | Err(_) => ToolchainProbe {
            kind,
            binary,
            available: false,
            version: None,
            support: ToolchainSupport::Unsupported(format!(
                "official toolchain `{binary}` not found on PATH"
            )),
        },
    }
}

pub(crate) fn probe_all_toolchains_json() -> Value {
    let kinds = [
        ToolchainKind::TypeScript,
        ToolchainKind::Rustc,
        ToolchainKind::Go,
        ToolchainKind::Swiftc,
        ToolchainKind::Kotlinc,
        ToolchainKind::Dart,
    ];
    let entries: Vec<Value> = kinds
        .into_iter()
        .map(|k| {
            let p = probe_toolchain(k);
            let (status, reason) = match &p.support {
                ToolchainSupport::Supported => ("supported", None),
                ToolchainSupport::Unsupported(msg) => ("unsupported", Some(msg.as_str())),
            };
            json!({
                "toolchain": p.kind,
                "binary": p.binary,
                "available": p.available,
                "version": p.version,
                "status": status,
                "reason": reason,
            })
        })
        .collect();
    json!({ "toolchains": entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_installed_rustc_and_serializes_all_six_toolchains() {
        let rust = probe_toolchain(ToolchainKind::Rustc);
        assert!(
            rust.available,
            "rustc must be available in test environment"
        );
        assert_eq!(rust.support, ToolchainSupport::Supported);

        let all = probe_all_toolchains_json();
        assert_eq!(all["toolchains"].as_array().map(Vec::len), Some(6));
    }
}
