mod analysis;
mod android;
mod archive;
use archive::Capture;
mod audit;
mod cache;
mod chromium;
mod comments;
mod compiler_oracle;
mod diff;
mod electron;
mod gpui;
mod graph_view;
mod host;
mod inspect;
mod ios;
mod live;
mod mcp;
mod nextjs;
mod ports;
mod reference;
mod replay;
mod retry;
mod screen_map;
mod sqlite_store;
mod surface;
mod transition;
mod tui;
mod typescript;
mod universal_web;
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

fn resolve_desktop_binary() -> Option<std::path::PathBuf> {
    if let Ok(explicit) = env::var("DUI_DESKTOP_BIN")
        && !explicit.trim().is_empty()
    {
        let path = std::path::PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(if cfg!(windows) {
            "dui-desktop.exe"
        } else {
            "dui-desktop"
        });
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    None
}

fn launch_desktop(tail: &[std::ffi::OsString]) -> Reply {
    let mut cmd = if let Some(bin) = resolve_desktop_binary() {
        let mut c = Command::new(bin);
        for arg in tail {
            c.arg(arg);
        }
        c
    } else {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(|repo| repo.join("apps/desktop/Cargo.toml"))
            .filter(|p| p.is_file())
            .ok_or_else(|| {
                failure(
                    3,
                    "dui-desktop binary not found next to dui and apps/desktop/Cargo.toml unavailable",
                )
            })?;
        let mut c = Command::new("cargo");
        if let Some(tc) = gpui::select_toolchain() {
            c.arg(format!("+{tc}"));
        }
        c.args(["run", "--quiet", "--manifest-path"])
            .arg(&manifest_dir)
            .arg("--");
        for arg in tail {
            c.arg(arg);
        }
        c
    };
    let output = cmd.output().map_err(|e| failure(3, &e.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(failure(3, stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout.is_empty() {
        return Ok(json!({ "ok": true }));
    }
    serde_json::from_str(&stdout).map_err(|e| failure(3, &format!("invalid desktop output: {e}")))
}

fn scan(root: &Path) -> Reply {
    if !root.is_dir() {
        return Err(failure(2, "project directory does not exist"));
    }
    let root = root
        .canonicalize()
        .map_err(|_| failure(3, "cannot resolve project"))?;
    let package = root
        .ancestors()
        .map(|dir| dir.join("node_modules/typescript"))
        .find(|p| p.join("package.json").is_file())
        .unwrap_or_else(|| root.join("node_modules/typescript"));
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
        && cmd == "toolchains"
    {
        return Ok(compiler_oracle::probe_all_toolchains_json());
    }
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
        let mut live_opts = viewer::LiveViewerOptions::default();
        let mut iter = tail.iter();
        while let Some(arg) = iter.next() {
            if arg == "--no-open" {
                open_browser = false;
            } else if arg == "--live" {
                live_opts.auto_live = true;
            } else if arg == "--live-url" {
                let Some(val) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --live-url"));
                };
                live_opts.live_url = Some(val.to_owned());
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
        return viewer::serve(run_dirs, port, open_browser, live_opts);
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
        && cmd == "audit"
    {
        return match tail {
            [run_dir] => audit::audit_run(Path::new(run_dir), None),
            [run_dir, flag, goal_raw] if flag == "--goal" => {
                let goal: f64 = goal_raw
                    .to_str()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| failure(2, "invalid --goal percentage"))?;
                audit::audit_run(Path::new(run_dir), Some(goal))
            }
            _ => Err(failure(2, "expected audit RUN_DIR [--goal PERCENT]")),
        };
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "auth"
    {
        let Some((sub, rest)) = tail.split_first() else {
            return Err(failure(
                2,
                "expected auth login URL [--out FILE] [--wait-url PATTERN] [--timeout SECS] [--headless]",
            ));
        };
        if sub != "login" {
            return Err(failure(2, "expected auth login URL ..."));
        }
        let mut url: Option<String> = None;
        let mut out_path = std::path::PathBuf::from(".dui-auth-state.json");
        let mut wait_url: Option<String> = None;
        let mut timeout_secs: u64 = 300;
        let mut headless = false;
        let mut iter = rest.iter();
        while let Some(arg) = iter.next() {
            if arg == "--out" {
                let Some(val) = iter.next() else {
                    return Err(failure(2, "missing value for --out"));
                };
                out_path = std::path::PathBuf::from(val);
            } else if arg == "--wait-url" {
                let Some(val) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --wait-url"));
                };
                wait_url = Some(val.to_owned());
            } else if arg == "--timeout" {
                let Some(val) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --timeout"));
                };
                timeout_secs = val
                    .parse::<u64>()
                    .map_err(|_| failure(2, "invalid --timeout seconds"))?;
            } else if arg == "--headless" {
                headless = true;
            } else if url.is_none() {
                url = Some(
                    arg.to_str()
                        .ok_or_else(|| failure(2, "invalid UTF-8 login URL"))?
                        .to_owned(),
                );
            } else {
                return Err(failure(2, "unexpected argument for auth login"));
            }
        }
        let raw_url = url.ok_or_else(|| {
            failure(
                2,
                "expected auth login URL [--out FILE] [--wait-url PATTERN] [--timeout SECS] [--headless]",
            )
        })?;
        let valid_url = dui_core::auth::validate_login_url(&raw_url).map_err(|e| failure(2, e))?;
        let exe = chromium::executable();
        let captured = chromium::capture_login_session(
            &exe,
            &valid_url,
            wait_url.as_deref(),
            std::time::Duration::from_secs(timeout_secs.max(1)),
            headless,
        )
        .map_err(|e| failure(3, &e.to_string()))?;
        if let Some(parent) = out_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| failure(3, &e.to_string()))?;
        }
        let body = serde_json::to_vec_pretty(&captured.storage_state)
            .map_err(|e| failure(3, &e.to_string()))?;
        std::fs::write(&out_path, body).map_err(|e| failure(3, &e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o600));
        }
        let abs_out = out_path.canonicalize().unwrap_or(out_path);
        return Ok(json!({
            "status": "ok",
            "output": abs_out,
            "url": captured.url,
            "origin": captured.origin,
            "cookies": captured.cookies_count,
            "localStorage": captured.local_storage_count,
            "sessionStorage": captured.session_storage_count,
        }));
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "desktop"
    {
        return launch_desktop(tail);
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "comment"
    {
        return match tail {
            [sub, payload] if sub == "add" => {
                let raw = payload
                    .to_str()
                    .ok_or_else(|| failure(2, "invalid UTF-8 comment JSON"))?;
                let parsed: Value = serde_json::from_str(raw)
                    .map_err(|e| failure(2, &format!("invalid comment JSON: {e}")))?;
                comments::add(&parsed)
            }
            [sub] if sub == "list" => comments::list(&json!({})),
            [sub, payload] if sub == "list" => {
                let raw = payload
                    .to_str()
                    .ok_or_else(|| failure(2, "invalid UTF-8 filter JSON"))?;
                let parsed: Value = serde_json::from_str(raw)
                    .map_err(|e| failure(2, &format!("invalid filter JSON: {e}")))?;
                comments::list(&parsed)
            }
            [sub, id] if sub == "resolve" => {
                let id_str = id
                    .to_str()
                    .ok_or_else(|| failure(2, "invalid comment id"))?;
                comments::resolve(&json!({ "id": id_str }))
            }
            [sub, id] if sub == "reopen" => {
                let id_str = id
                    .to_str()
                    .ok_or_else(|| failure(2, "invalid comment id"))?;
                comments::reopen(&json!({ "id": id_str }))
            }
            _ => Err(failure(
                2,
                "expected comment <add JSON | list [JSON] | resolve ID | reopen ID>",
            )),
        };
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "transitions"
    {
        let (root, spec, run_dir) = match tail {
            [root, spec] => (Path::new(root), Path::new(spec), None),
            [root, spec, run_dir] => (Path::new(root), Path::new(spec), Some(Path::new(run_dir))),
            _ => {
                return Err(failure(
                    2,
                    "expected transitions PROJECT MANIFEST [RUN_DIR]",
                ));
            }
        };
        return transition::replay(root, spec, run_dir);
    }
    if let [cmd, tail @ ..] = args.as_slice()
        && cmd == "diff"
    {
        let mut overlay_dir: Option<&Path> = None;
        let mut base_branch: Option<&str> = None;
        let mut head_branch: Option<&str> = None;
        let mut project_dir: Option<&Path> = None;
        let mut markdown_mode = false;
        let mut positional = Vec::new();
        let mut iter = tail.iter();
        while let Some(arg) = iter.next() {
            if arg == "--overlay-dir" {
                let Some(dir) = iter.next() else {
                    return Err(failure(2, "missing value for --overlay-dir"));
                };
                overlay_dir = Some(Path::new(dir));
            } else if arg == "--base" {
                let Some(b) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --base"));
                };
                base_branch = Some(b);
            } else if arg == "--head" {
                let Some(h) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --head"));
                };
                head_branch = Some(h);
            } else if arg == "--project" {
                let Some(p) = iter.next() else {
                    return Err(failure(2, "missing value for --project"));
                };
                project_dir = Some(Path::new(p));
            } else if arg == "--markdown" {
                markdown_mode = true;
            } else if arg == "--format" {
                let Some(fmt) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --format"));
                };
                match fmt {
                    "markdown" | "md" => markdown_mode = true,
                    "json" => markdown_mode = false,
                    _ => return Err(failure(2, "expected --format json|markdown")),
                }
            } else {
                positional.push(arg);
            }
        }
        let reply = if let Some(base) = base_branch {
            let proj = project_dir.unwrap_or_else(|| Path::new("."));
            diff::compare_branches(proj, base, head_branch, overlay_dir)?
        } else {
            let (before, after) = match positional.as_slice() {
                [before, after] => (Path::new(before), Path::new(after)),
                _ => {
                    return Err(failure(
                        2,
                        "expected diff BEFORE AFTER [--overlay-dir DIR] [--markdown] | diff --base BRANCH [--head BRANCH] [--project DIR] [--markdown]",
                    ));
                }
            };
            diff::compare(before, after, overlay_dir)?
        };
        if markdown_mode {
            let md = dui_core::diff::format_diff_markdown(&reply);
            return Ok(json!({"__rawStdout": md}));
        }
        return Ok(reply);
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
        let mut changed_since: Option<String> = None;
        let mut positional = Vec::new();
        let mut iter = rest.iter();
        while let Some(arg) = iter.next() {
            if arg == "--force" || arg == "--no-cache" {
                force = true;
            } else if arg == "--changed-since" {
                let Some(base_ref) = iter.next().and_then(|v| v.to_str()) else {
                    return Err(failure(2, "missing value for --changed-since"));
                };
                changed_since = Some(base_ref.to_owned());
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
                    "expected run [--force] [--changed-since REF] PROJECT MANIFEST [OUTPUT]",
                ));
            }
        };
        let mut result = archive::capture(root, spec, output, Capture::Run { force });
        if let Some(base_ref) = changed_since {
            let reply_ref = match &mut result {
                Ok(body) | Err((5, body)) => Some(body),
                Err(_) => None,
            };
            if let Some(reply) = reply_ref
                && let Some(out_dir) = reply["output"].as_str().map(str::to_owned)
            {
                let facts_json = replay::read(&Path::new(&out_dir).join("facts.json")).ok();
                let module_graph = facts_json
                    .as_ref()
                    .map(|f| &f["moduleGraph"])
                    .unwrap_or(&Value::Null);
                let summary = diff::changed_since_summary(root, &base_ref, module_graph);
                if let Some(obj) = reply.as_object_mut() {
                    obj.insert("changedSince".into(), summary.clone());
                }
                let report_path = Path::new(&out_dir).join("report.json");
                if let Ok(mut report) = replay::read(&report_path)
                    && let Some(robj) = report.as_object_mut()
                {
                    robj.insert("changedSince".into(), summary);
                    let _ = fs::write(
                        &report_path,
                        serde_json::to_vec_pretty(&report).unwrap_or_default(),
                    );
                }
            }
        }
        return result;
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
    if let Some(raw) = body.get("__rawStdout").and_then(Value::as_str) {
        print!("{raw}");
    } else if !body.is_null() {
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
