mod analysis;
mod chromium;
mod graph_view;
mod host;
mod mcp;
mod ports;
mod reference;
mod replay;
mod retry;
mod typescript;
use serde_json::{Value, json};
use std::{
    env, fs,
    path::Path,
    process::{Command, ExitCode},
};

type Reply = Result<Value, (u8, Value)>;

fn failure(code: u8, reason: &str) -> (u8, Value) {
    (code, json!({"status":"error","reason":reason}))
}

fn scan(root: &Path) -> Reply {
    if !root.is_dir() {
        return Err(failure(2, "project directory does not exist"));
    }
    let root = root
        .canonicalize()
        .map_err(|_| failure(3, "cannot resolve project"))?;
    let package = root.join("node_modules/typescript");
    let unsupported = |reason: &str| (4, json!({"status":"unsupported","reason":reason}));
    let manifest = fs::read(package.join("package.json"))
        .map_err(|_| unsupported("typescript package unavailable"))?;
    let metadata: Value =
        serde_json::from_slice(&manifest).map_err(|_| failure(3, "invalid compiler manifest"))?;
    let official_alias = metadata["name"] == "@typescript/typescript6"
        && metadata["version"]
            .as_str()
            .is_some_and(|v| v.starts_with("6."));
    if metadata["name"] != "typescript" && !official_alias {
        return Err(unsupported("unrecognized compiler package"));
    }
    let bin = metadata
        .pointer(if official_alias {
            "/bin/tsc6"
        } else {
            "/bin/tsc"
        })
        .and_then(Value::as_str)
        .ok_or_else(|| unsupported("tsc missing"))?;
    let bin = package
        .join(bin)
        .canonicalize()
        .map_err(|_| unsupported("tsc unavailable"))?;
    if !bin.starts_with(
        package
            .canonicalize()
            .map_err(|_| failure(3, "cannot resolve compiler package"))?,
    ) {
        return Err(unsupported("unexpected compiler entry point"));
    }
    let output = Command::new(bin)
        .arg("--version")
        .current_dir(root)
        .output()
        .map_err(|_| unsupported("tsc unavailable"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout.trim().strip_prefix("Version ").unwrap_or("");
    // The official TS6 wrapper has its own version and delegates to the project's
    // resolved TypeScript 6 compiler. Compare the CLI with that compiler's API.
    let matches_install = if official_alias {
        let api = Command::new("node")
            .args([
                "--input-type=module",
                "--eval",
                include_str!(concat!(env!("OUT_DIR"), "/typescript/version.mjs")),
            ])
            .arg(&package)
            .output()
            .map_err(|_| unsupported("compiler API unavailable"))?;
        api.status.success()
            && version.starts_with("6.")
            && String::from_utf8_lossy(&api.stdout).trim() == version
    } else {
        metadata["version"] == version
    };
    if !output.status.success() || !matches_install || !is_supported_ts(version) {
        return Err((
            4,
            json!({"status":"unsupported","reason":"project requires official stable TypeScript 6.x or 7.x","observed":version}),
        ));
    }
    Ok(json!({"status":"ok","checked":"typescript","version":version}))
}

fn is_supported_ts(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && matches!(parts[0], "6" | "7")
        && parts[1..].iter().all(|s| {
            !s.is_empty()
                && (s.len() == 1 || !s.starts_with('0'))
                && s.bytes().all(|c| c.is_ascii_digit())
        })
}

fn run() -> Reply {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if let [flag] = args.as_slice()
        && (flag == "--version" || flag == "-V")
    {
        return Ok(json!({"name": "dui", "version": env!("CARGO_PKG_VERSION")}));
    }
    if let [cmd, manifest, plan, out] = args.as_slice()
        && cmd == "replay-report"
    {
        return replay::summarize(Path::new(manifest), Path::new(plan), Path::new(out));
    }
    if let [cmd, project, manifest, plan, out, tail @ ..] = args.as_slice()
        && cmd == "replay"
    {
        let port = match tail {
            [] => None,
            [p] => Some(
                p.to_str()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| failure(2, "invalid port"))?,
            ),
            _ => return Err(failure(2, "unexpected arguments")),
        };
        return replay::run(
            Path::new(project),
            Path::new(manifest),
            Path::new(plan),
            Path::new(out),
            port,
        );
    }
    if let [cmd] = args.as_slice()
        && cmd == "mcp"
    {
        mcp::serve()?;
        return Ok(Value::Null);
    }
    if let [cmd, root, spec, out] = args.as_slice()
        && cmd == "run"
    {
        return host::run(Path::new(root), Path::new(spec), Path::new(out));
    }
    if let [cmd, root, spec] = args.as_slice()
        && cmd == "scan"
    {
        return analysis::analyze(Path::new(root), Path::new(spec));
    }
    if let [cmd, root, spec, out, port] = args.as_slice()
        && cmd == "exercise"
    {
        return analysis::exercise(
            Path::new(root),
            Path::new(spec),
            Path::new(out),
            port.to_str()
                .and_then(|p| p.parse().ok())
                .ok_or_else(|| failure(2, "invalid port"))?,
        );
    }
    let [command, path] = args.as_slice() else {
        return Err(failure(
            2,
            "expected scan PROJECT MANIFEST, run PROJECT MANIFEST OUTPUT, replay PROJECT MANIFEST PLAN_REPORT OUTPUT, replay-report MANIFEST PLAN_REPORT OUTPUT, exercise PROJECT MANIFEST OUTPUT PORT, mcp, plan FIXTURE, or check-ts PROJECT",
        ));
    };
    if command == "check-ts" {
        return scan(Path::new(path));
    }
    if command != "plan" {
        return Err(failure(2, "unknown command"));
    }
    let data = fs::read(path).map_err(|_| failure(2, "cannot read fixture"))?;
    let fixture = serde_json::from_slice(&data).map_err(|e| failure(2, &e.to_string()))?;
    let jobs = dui_core::plan(&fixture).map_err(|e| failure(2, e))?;
    Ok(json!({"jobs":jobs}))
}

fn main() -> ExitCode {
    let (code, body) = match run() {
        Ok(body) => (0, body),
        Err(err) => err,
    };
    if !body.is_null() {
        println!("{body}");
    }
    ExitCode::from(code)
}

#[cfg(test)]
mod compiler_support_tests {
    use super::is_supported_ts;

    #[test]
    fn accepts_only_stable_six_and_seven() {
        for version in ["6.0.2", "6.0.3", "7.0.2"] {
            assert!(is_supported_ts(version));
        }
        for version in [
            "5.9.3",
            "8.0.0",
            "6.0.0-beta",
            "7.1.0-rc.1",
            "6.00.1",
            "6.0",
        ] {
            assert!(!is_supported_ts(version), "{version}");
        }
    }
}
