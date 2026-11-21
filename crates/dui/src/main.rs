mod analysis;
mod archive;
use archive::Capture;
mod cache;
mod chromium;
mod diff;
mod gpui;
mod graph_view;
mod host;
mod inspect;
mod live;
mod mcp;
mod ports;
mod reference;
mod replay;
mod retry;
mod screen_map;
mod sqlite_store;
mod typescript;
mod update;
mod viewer;
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
    if let [cmd] = args.as_slice()
        && cmd == "live-owner"
    {
        return live::owner();
    }
    if let [cmd, request] = args.as_slice()
        && cmd == "live"
    {
        let request = request
            .to_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .ok_or_else(|| failure(2, "expected live REQUEST_JSON"))?;
        return live::command(&request);
    }
    if let [cmd, root] = args.as_slice()
        && cmd == "map"
    {
        return screen_map::capture(Path::new(root));
    }
    if let [flag] = args.as_slice()
        && (flag == "--version" || flag == "-V")
    {
        return Ok(json!({
            "name": "dui",
            "version": update::current_version_string(),
            "tag": update::current_tag(),
        }));
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "update"
    {
        let mut check_only = false;
        let mut repo_override: Option<String> = None;
        let mut requested_tag: Option<String> = None;
        let mut install_dir: Option<std::path::PathBuf> = None;
        let mut iter = tail.iter();
        while let Some(arg) = iter.next() {
            if arg == "--check" {
                check_only = true;
            } else if arg == "--repo" {
                let Some(v) = iter.next().and_then(|s| s.to_str()) else {
                    return Err(failure(2, "missing value for --repo"));
                };
                repo_override = Some(v.to_owned());
            } else if arg == "--tag" {
                let Some(v) = iter.next().and_then(|s| s.to_str()) else {
                    return Err(failure(2, "missing value for --tag"));
                };
                requested_tag = Some(v.to_owned());
            } else if arg == "--install-dir" {
                let Some(v) = iter.next() else {
                    return Err(failure(2, "missing value for --install-dir"));
                };
                install_dir = Some(std::path::PathBuf::from(v));
            } else {
                return Err(failure(
                    2,
                    "expected update [--check] [--tag TAG] [--repo URL] [--install-dir DIR]",
                ));
            }
        }
        let repo = update::repo_url(repo_override.as_deref());
        return if check_only {
            update::check(&repo)
        } else {
            update::apply(&repo, requested_tag.as_deref(), install_dir.as_deref())
        };
    }
    if let [cmd] = args.as_slice()
        && cmd == "history"
    {
        return archive::history();
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && (cmd == "open" || cmd == "serve")
    {
        let mut port: u16 = 0;
        let mut open_browser = cmd == "open";
        let mut run_dirs = Vec::new();
        let mut iter = tail.iter();
        while let Some(arg) = iter.next() {
            if arg == "--no-open" {
                open_browser = false;
            } else if arg == "--port" {
                let Some(val) = iter.next() else {
                    return Err(failure(2, "missing value for --port"));
                };
                port = val
                    .to_str()
                    .and_then(|s| s.parse::<u16>().ok())
                    .ok_or_else(|| failure(2, "invalid --port"))?;
            } else {
                run_dirs.push(std::path::PathBuf::from(arg));
            }
        }
        return viewer::serve(run_dirs, port, open_browser);
    }
    if let [cmd, manifest, plan, out] = args.as_slice()
        && cmd == "replay-report"
    {
        return replay::summarize(Path::new(manifest), Path::new(plan), Path::new(out));
    }
    if let [cmd, project, manifest, plan] = args.as_slice()
        && cmd == "replay"
    {
        return archive::capture(
            Path::new(project),
            Path::new(manifest),
            None,
            Capture::Replay(Path::new(plan), None),
        );
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
        return archive::capture(
            Path::new(project),
            Path::new(manifest),
            Some(Path::new(out)),
            Capture::Replay(Path::new(plan), port),
        );
    }
    if let [cmd] = args.as_slice()
        && cmd == "mcp"
    {
        mcp::serve()?;
        return Ok(Value::Null);
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "diff"
    {
        let mut overlay_dir: Option<&Path> = None;
        let mut positional = Vec::new();
        let mut iter = tail.iter();
        while let Some(arg) = iter.next() {
            if arg == "--overlay-dir" {
                let Some(dir) = iter.next() else {
                    return Err(failure(2, "missing value for --overlay-dir"));
                };
                overlay_dir = Some(Path::new(dir));
            } else {
                positional.push(arg);
            }
        }
        let (before, after) = match positional.as_slice() {
            [before, after] => (Path::new(before), Path::new(after)),
            _ => {
                return Err(failure(2, "expected diff BEFORE AFTER [--overlay-dir DIR]"));
            }
        };
        return diff::compare(before, after, overlay_dir);
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "prune"
    {
        let max_bytes = match tail {
            [] => None,
            [flag, value] if flag == "--max-bytes" => Some(
                value
                    .to_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or_else(|| failure(2, "invalid --max-bytes"))?,
            ),
            _ => return Err(failure(2, "expected prune [--max-bytes BYTES]")),
        };
        return archive::prune(max_bytes);
    }
    if let [cmd, rest @ ..] = args.as_slice()
        && cmd == "run"
    {
        let mut force = false;
        let mut positional = Vec::new();
        for arg in rest {
            if arg == "--force" || arg == "--no-cache" {
                force = true;
            } else {
                positional.push(arg);
            }
        }
        let (root, spec, output) = match positional.as_slice() {
            [root, spec] => (Path::new(root), Path::new(spec), None),
            [root, spec, out] => (Path::new(root), Path::new(spec), Some(Path::new(out))),
            _ => {
                return Err(failure(
                    2,
                    "expected run [--force] PROJECT MANIFEST [OUTPUT]",
                ));
            }
        };
        return archive::capture(root, spec, output, Capture::Run { force });
    }
    if let [cmd, directory, selection] = args.as_slice()
        && cmd == "inspect"
    {
        let selection = selection
            .to_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .ok_or_else(|| failure(2, "expected inspect RUN_DIRECTORY SELECTION_JSON"))?;
        return inspect::archive(Path::new(directory), selection);
    }
    if let [cmd, root, spec] = args.as_slice()
        && cmd == "scan"
    {
        return analysis::analyze(Path::new(root), Path::new(spec));
    }
    if let [cmd, root, spec, rest @ ..] = args.as_slice()
        && cmd == "exercise"
    {
        let (out, port) = match rest {
            [port] => (None, port),
            [out, port] => (Some(Path::new(out)), port),
            _ => {
                return Err(failure(
                    2,
                    "expected exercise PROJECT MANIFEST [OUTPUT] PORT",
                ));
            }
        };
        let port = port
            .to_str()
            .and_then(|p| p.parse().ok())
            .ok_or_else(|| failure(2, "invalid port"))?;
        return archive::capture(
            Path::new(root),
            Path::new(spec),
            out,
            Capture::Exercise(port),
        );
    }
    let [command, path] = args.as_slice() else {
        return Err(failure(
            2,
            "expected live REQUEST_JSON, inspect RUN_DIRECTORY SELECTION_JSON, map PROJECT, scan PROJECT MANIFEST, run PROJECT MANIFEST [OUTPUT], history, open, replay PROJECT MANIFEST PLAN_REPORT [OUTPUT], replay-report MANIFEST PLAN_REPORT OUTPUT, exercise PROJECT MANIFEST OUTPUT PORT, mcp, plan FIXTURE, or check-ts PROJECT",
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
