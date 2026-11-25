//! Embedded loopback HTTP server for `apps/canvas` (`dui open`).

use crate::{Reply, archive, audit, comments, diff, failure, sqlite_store::SqliteStore};
use dui_core::storage::{RunFilter, RunRepository};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

mod responsive;

mod canvas_assets {
    include!(concat!(env!("OUT_DIR"), "/canvas_assets.rs"));
}

#[derive(Clone)]
struct ArchiveEntry {
    root: PathBuf,
    run: Value,
}

struct ViewerState {
    responsive_sessions: BTreeSet<String>,
    live_project: Option<PathBuf>,
    cli_paths: Vec<PathBuf>,
    extra_saved_paths: BTreeSet<PathBuf>,
    diff_overlay_dirs: BTreeMap<String, PathBuf>,
    run_roots: BTreeMap<String, PathBuf>,
    default_run_key: Option<String>,
    archive_images: BTreeMap<String, Arc<BTreeSet<String>>>,
    archive_graphs: BTreeMap<String, Arc<Vec<u8>>>,
    audit_cache: BTreeMap<String, Value>,
}

impl Drop for ViewerState {
    fn drop(&mut self) {
        for dir in self.diff_overlay_dirs.values() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

fn sha256_hex(input: &str) -> String {
    format!("{:x}", Sha256::digest(input.as_bytes()))
}

fn read_optional_json(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn resolve_project_path(meta: Option<&Value>) -> Option<PathBuf> {
    let meta = meta?;
    let base = meta["worktree"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| meta["projectRoot"].as_str().filter(|s| !s.is_empty()))?;
    let app = meta["app"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(".");
    let full = Path::new(base).join(app);
    if full.exists() {
        return Some(full);
    }
    if let Some(root) = meta["projectRoot"].as_str().filter(|s| *s != base) {
        let fallback = Path::new(root).join(app);
        if fallback.exists() {
            return Some(fallback);
        }
    }
    None
}

fn list_subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            (!s.starts_with('.') && e.file_type().ok().is_some_and(|t| t.is_dir()))
                .then(|| e.path())
        })
        .collect()
}

fn list_archives(state: &ViewerState) -> Vec<ArchiveEntry> {
    let mut paths: Vec<PathBuf> = state.cli_paths.clone();
    paths.extend(state.extra_saved_paths.iter().cloned());

    if state.cli_paths.is_empty()
        && let Ok(home) = archive::home_dir()
    {
        if let Ok((store, _)) = SqliteStore::open(&home)
            && let Ok(summaries) = store.list_runs(&RunFilter::default())
        {
            for summary in summaries {
                let run_dir = home
                    .join("projects")
                    .join(summary.run.project_id().as_str())
                    .join("runs")
                    .join(summary.run.id().as_str());
                if run_dir.join("graph.json").is_file() {
                    paths.push(run_dir);
                }
            }
        }
        // Include any unindexed disk run directories not yet present in SQLite.
        let mut discovered = Vec::new();
        for project in list_subdirs(&home.join("projects")) {
            for run in list_subdirs(&project.join("runs")) {
                if run.join("graph.json").is_file() {
                    discovered.push(run);
                }
            }
        }
        discovered.sort_by(|a, b| {
            let an = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let bn = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
            bn.cmp(an)
        });
        paths.extend(discovered);
    }

    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for path in paths {
        if let Ok(canon) = path.canonicalize()
            && seen.insert(canon.clone())
        {
            unique.push(canon);
        }
    }

    let dui_projects = archive::home_dir().ok().map(|h| h.join("projects"));

    unique
        .into_iter()
        .map(|root| {
            let meta = read_optional_json(&root.join("snapshot.json"));
            let root_str = root.to_string_lossy();
            let key = sha256_hex(&root_str);

            let project_record = dui_projects.as_deref().and_then(|projects_root| {
                let runs_dir = root.parent()?;
                let proj_dir = runs_dir.parent()?;
                let proj_name = proj_dir.file_name()?.to_str()?;
                let expected = projects_root.join(proj_name).join("runs");
                let same_id = meta
                    .as_ref()
                    .and_then(|m| m["projectId"].as_str())
                    .is_some_and(|id| id == proj_name);
                if runs_dir == expected && same_id {
                    read_optional_json(&proj_dir.join("project.json"))
                } else {
                    None
                }
            });

            let project_path = resolve_project_path(meta.as_ref());
            let project_id = meta
                .as_ref()
                .and_then(|m| m["projectId"].as_str())
                .unwrap_or(&key)
                .to_owned();
            let project_name = project_record
                .as_ref()
                .and_then(|r| r["name"].as_str())
                .or_else(|| meta.as_ref().and_then(|m| m["projectName"].as_str()))
                .or_else(|| {
                    meta.as_ref()
                        .and_then(|m| m["projectRoot"].as_str())
                        .and_then(|p| Path::new(p).file_name().and_then(|n| n.to_str()))
                })
                .or_else(|| root.file_name().and_then(|n| n.to_str()))
                .unwrap_or("Unnamed project")
                .to_owned();
            let branch = meta
                .as_ref()
                .and_then(|m| m["branch"].as_str())
                .map(str::to_owned);
            let version = meta
                .as_ref()
                .and_then(|m| m["id"].as_str())
                .or_else(|| root.file_name().and_then(|n| n.to_str()))
                .unwrap_or("")
                .to_owned();

            let run = json!({
                "key": key,
                "projectId": project_id,
                "project": project_name,
                "branch": branch,
                "version": version,
                "liveAvailable": project_path.is_some(),
                "responsiveAvailable": responsive::available(state, project_path.as_deref(), &root),
            });
            ArchiveEntry { root, run }
        })
        .collect()
}

fn read_png_dimensions(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut file = fs::File::open(path).ok()?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header).ok()?;
    if &header[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    (width > 0 && height > 0).then_some((width, height))
}

fn open_archive(root: &Path) -> Result<(Vec<u8>, BTreeSet<String>), String> {
    let mut graph: Value =
        serde_json::from_slice(&fs::read(root.join("graph.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let report = read_optional_json(&root.join("report.json"));
    let captures = report
        .as_ref()
        .and_then(|r| r["cells"].as_array())
        .or_else(|| graph["overview"].as_array())
        .cloned()
        .unwrap_or_default();

    let mut cells = Vec::with_capacity(captures.len());
    let mut images = BTreeSet::new();
    let mut dims_cache: BTreeMap<String, Option<(u32, u32)>> = BTreeMap::new();
    let mut png_dims = |img: &str| -> Option<(u32, u32)> {
        if let Some(cached) = dims_cache.get(img) {
            return *cached;
        }
        let found = read_png_dimensions(&root.join(img));
        dims_cache.insert(img.to_owned(), found);
        found
    };
    for item in &captures {
        let mut cell = json!({
            "path": item["path"],
            "width": item["width"],
            "image": item["image"],
            "status": item["status"],
            "reason": item["reason"],
            "state": item["state"],
            "scenario": item["scenario"],
        });
        if let Some(loc) = item.get("locale").filter(|v| v.is_string()) {
            cell["locale"] = loc.clone();
        }
        if let Some(audit) = item.get("localeAudit").filter(|v| v.is_object()) {
            cell["localeAudit"] = audit.clone();
        }
        if let Some(evidence) = item["observation"].get("screenEvidence") {
            cell["screenEvidence"] = evidence.clone();
        }
        if let Some(path) = item["observation"]["redirectedTo"].as_str() {
            cell["redirectedTo"] = json!(path);
        }
        if let Some(tree) = item["observation"]
            .get("renderTree")
            .filter(|v| v.is_array())
        {
            cell["renderTree"] = tree.clone();
        }
        if let Some(img) = item["image"].as_str()
            && is_safe_png_name(img)
        {
            images.insert(img.to_owned());
            if let Some((iw, ih)) = png_dims(img) {
                cell["imageWidth"] = json!(iw);
                cell["imageHeight"] = json!(ih);
            }
        }
        cells.push(cell);
    }
    if let Some(overview) = graph["overview"].as_array_mut() {
        for item in overview {
            if let Some(img) = item["image"].as_str().map(str::to_owned)
                && is_safe_png_name(&img)
            {
                images.insert(img.clone());
                if let Some((iw, ih)) = png_dims(&img) {
                    item["imageWidth"] = json!(iw);
                    item["imageHeight"] = json!(ih);
                }
            }
        }
    }
    if let Some(obj) = graph.as_object_mut() {
        obj.insert("cells".into(), Value::Array(cells));
    }
    let body = serde_json::to_vec(&graph).map_err(|e| e.to_string())?;
    Ok((body, images))
}

fn is_safe_png_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name.ends_with(".png")
}

fn is_valid_overlay_name(name: &str) -> bool {
    name.len() == 25
        && name.ends_with("-diff.png")
        && name[..16].bytes().all(|b| b.is_ascii_hexdigit())
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_query(raw: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        map.insert(percent_decode(k), percent_decode(v));
    }
    map
}

fn is_same_origin(headers: &BTreeMap<String, String>) -> bool {
    let Some(origin) = headers.get("origin") else {
        return true;
    };
    let Some(host) = headers.get("host") else {
        return false;
    };
    origin == &format!("http://{host}")
}

fn send_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    cache_control: &str,
    body: &[u8],
) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: {cache_control}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn send_json(stream: &mut TcpStream, status: u16, body: &Value) {
    let bytes = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    send_response(
        stream,
        status,
        "application/json; charset=utf-8",
        "no-store",
        &bytes,
    );
}

fn handle_connection(mut stream: TcpStream, state: Arc<Mutex<ViewerState>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.trim().is_empty() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let Some(method) = parts.next() else {
        return;
    };
    let Some(target) = parts.next() else {
        return;
    };

    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }

    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if content_length > 65536 {
        send_json(
            &mut stream,
            413,
            &json!({"status":"error","reason":"request-too-large"}),
        );
        return;
    }
    let mut body_bytes = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body_bytes).is_err() {
        send_json(
            &mut stream,
            400,
            &json!({"status":"error","reason":"incomplete-body"}),
        );
        return;
    }

    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let path = percent_decode(raw_path);
    let query = parse_query(raw_query);

    if path == "/api/responsive" {
        let host = headers.get("host").map(String::as_str).unwrap_or_default();
        let loopback = stream.local_addr().is_ok_and(|addr| {
            host == addr.to_string() || host == format!("localhost:{}", addr.port())
        });
        if method != "POST"
            || !loopback
            || headers.get("origin") != Some(&format!("http://{host}"))
            || headers.get("content-type").map(String::as_str) != Some("application/json")
        {
            send_json(&mut stream, 403, &json!({"reason":"forbidden-origin"}));
            return;
        }
        let result = serde_json::from_slice(&body_bytes)
            .map_err(|_| failure(2, "invalid-json"))
            .and_then(|body| responsive::command(&state, &body));
        match result {
            Ok(body) => send_json(&mut stream, 200, &body),
            Err((code, body)) => send_json(&mut stream, if code == 2 { 400 } else { 500 }, &body),
        }
        return;
    }
    if let Some(archive_name) = path.strip_prefix("/archive/") {
        if method != "GET" {
            send_response(&mut stream, 405, "text/plain", "no-store", b"");
            return;
        }
        if archive_name == "runs.json" {
            let archives = match state.lock() {
                Ok(mut guard) => {
                    let listed = list_archives(&guard);
                    guard.default_run_key = listed
                        .first()
                        .and_then(|a| a.run["key"].as_str())
                        .map(str::to_owned);
                    for a in &listed {
                        if let Some(k) = a.run["key"].as_str() {
                            guard.run_roots.insert(k.to_owned(), a.root.clone());
                        }
                    }
                    listed
                }
                Err(_) => {
                    send_json(&mut stream, 500, &json!({"status":"error"}));
                    return;
                }
            };
            let runs: Vec<Value> = archives.into_iter().map(|a| a.run).collect();
            send_json(&mut stream, 200, &Value::Array(runs));
            return;
        }

        struct ResolvedArchive {
            run_key: String,
            root: PathBuf,
            cached_graph: Option<Arc<Vec<u8>>>,
            cached_images: Option<Arc<BTreeSet<String>>>,
        }
        let requested_key = query.get("run").map(String::as_str);
        // Fast O(1) lookup for cached run root + image manifest without holding the mutex during disk I/O.
        let resolved: Option<ResolvedArchive> = {
            let Ok(mut guard) = state.lock() else {
                send_json(&mut stream, 500, &json!({"status":"error"}));
                return;
            };
            let key_opt = match requested_key {
                Some(k) => Some(k.to_owned()),
                None => guard.default_run_key.clone(),
            };
            let mut root_opt = key_opt
                .as_deref()
                .and_then(|k| guard.run_roots.get(k).cloned());
            let mut final_key = key_opt;
            if root_opt.is_none() {
                let listed = list_archives(&guard);
                guard.default_run_key = listed
                    .first()
                    .and_then(|a| a.run["key"].as_str())
                    .map(str::to_owned);
                for a in &listed {
                    if let Some(k) = a.run["key"].as_str() {
                        guard.run_roots.insert(k.to_owned(), a.root.clone());
                    }
                }
                let picked = match requested_key {
                    None => listed.first(),
                    Some(k) => listed.iter().find(|a| a.run["key"].as_str() == Some(k)),
                };
                if let Some(entry) = picked
                    && let Some(k) = entry.run["key"].as_str()
                {
                    final_key = Some(k.to_owned());
                    root_opt = Some(entry.root.clone());
                }
            }
            match (final_key, root_opt) {
                (Some(run_key), Some(root)) => {
                    let cached_graph = guard.archive_graphs.get(&run_key).cloned();
                    let cached_images = guard.archive_images.get(&run_key).cloned();
                    Some(ResolvedArchive {
                        run_key,
                        root,
                        cached_graph,
                        cached_images,
                    })
                }
                _ => None,
            }
        };

        let Some(ResolvedArchive {
            run_key,
            root,
            cached_graph,
            cached_images,
        }) = resolved
        else {
            send_response(&mut stream, 404, "text/plain", "no-store", b"");
            return;
        };

        let (graph_bytes, images) = match (cached_graph, cached_images) {
            (Some(g), Some(imgs)) => (g, imgs),
            _ => {
                let Ok((g, imgs)) = open_archive(&root) else {
                    send_response(&mut stream, 404, "text/plain", "no-store", b"");
                    return;
                };
                let g = Arc::new(g);
                let imgs = Arc::new(imgs);
                if let Ok(mut guard) = state.lock() {
                    guard.archive_graphs.insert(run_key.clone(), Arc::clone(&g));
                    guard.archive_images.insert(run_key, Arc::clone(&imgs));
                }
                (g, imgs)
            }
        };

        if archive_name == "graph.json" {
            send_response(
                &mut stream,
                200,
                "application/json; charset=utf-8",
                "no-store",
                &graph_bytes,
            );
            return;
        }
        if images.contains(archive_name)
            && let Ok(img_path) = root.join(archive_name).canonicalize()
            && img_path.parent() == Some(root.as_path())
            && let Ok(png) = fs::read(&img_path)
        {
            // Saved archive PNGs are content-immutable per run directory.
            send_response(
                &mut stream,
                200,
                "image/png",
                "public, max-age=31536000, immutable",
                &png,
            );
            return;
        }
        send_response(&mut stream, 404, "text/plain", "no-store", b"");
        return;
    }

    if path == "/api/audit" || path == "/api/audit/" {
        if !is_same_origin(&headers) {
            send_json(
                &mut stream,
                403,
                &json!({"status":"error","reason":"forbidden-origin"}),
            );
            return;
        }
        if method != "GET" {
            send_response(&mut stream, 405, "text/plain", "no-store", b"");
            return;
        }
        let run_key = query.get("run").map(String::as_str);
        let goal = query.get("goal").and_then(|g| g.parse::<f64>().ok());
        let (resolved_key, root, cached_audit) = {
            let Ok(mut guard) = state.lock() else {
                send_json(&mut stream, 500, &json!({"status":"error"}));
                return;
            };
            let requested = run_key.filter(|s| !s.is_empty());
            let mut key_opt = requested
                .map(str::to_owned)
                .or_else(|| guard.default_run_key.clone());
            let mut root_opt = key_opt
                .as_deref()
                .and_then(|k| guard.run_roots.get(k).cloned());
            if root_opt.is_none() {
                let listed = list_archives(&guard);
                guard.default_run_key = listed
                    .first()
                    .and_then(|a| a.run["key"].as_str())
                    .map(str::to_owned);
                for a in &listed {
                    if let Some(k) = a.run["key"].as_str() {
                        guard.run_roots.insert(k.to_owned(), a.root.clone());
                    }
                }
                let picked = match requested {
                    None => listed.first(),
                    Some(k) => listed.iter().find(|a| a.run["key"].as_str() == Some(k)),
                };
                if let Some(entry) = picked
                    && let Some(k) = entry.run["key"].as_str()
                {
                    key_opt = Some(k.to_owned());
                    root_opt = Some(entry.root.clone());
                }
            }
            let Some(k) = key_opt else {
                send_json(
                    &mut stream,
                    404,
                    &json!({"status":"error","reason":"run-not-found"}),
                );
                return;
            };
            let Some(r) = root_opt else {
                send_json(
                    &mut stream,
                    404,
                    &json!({"status":"error","reason":"run-not-found"}),
                );
                return;
            };
            let cache_k = format!(
                "{k}:{}",
                goal.map_or("default".to_owned(), |g| format!("{g:.2}"))
            );
            let cached = guard.audit_cache.get(&cache_k).cloned();
            (k, r, cached)
        };
        if let Some(reply) = cached_audit {
            send_json(&mut stream, 200, &reply);
            return;
        }
        match audit::audit_run(&root, goal) {
            Ok(mut reply) => {
                if let Some(obj) = reply.as_object_mut() {
                    obj.remove("run");
                    obj.insert("runKey".into(), json!(resolved_key));
                }
                let cache_k = format!(
                    "{resolved_key}:{}",
                    goal.map_or("default".to_owned(), |g| format!("{g:.2}"))
                );
                if let Ok(mut guard) = state.lock() {
                    guard.audit_cache.insert(cache_k, reply.clone());
                }
                send_json(&mut stream, 200, &reply);
            }
            Err((code, err)) => {
                send_json(&mut stream, if code == 2 { 400 } else { 500 }, &err);
            }
        }
        return;
    }

    if path == "/api/comments" || path == "/api/comments/" {
        if !is_same_origin(&headers) {
            send_json(
                &mut stream,
                403,
                &json!({"status":"error","reason":"forbidden-origin"}),
            );
            return;
        }
        if method == "GET" {
            let mut args = serde_json::Map::new();
            if let Some(pid) = query.get("projectId").filter(|s| !s.is_empty()) {
                args.insert("projectId".into(), json!(pid));
            }
            if let Some(run_key) = query.get("run").filter(|s| !s.is_empty())
                && let Ok(guard) = state.lock()
                && let Some(entry) = list_archives(&guard)
                    .into_iter()
                    .find(|a| a.run["key"].as_str() == Some(run_key.as_str()))
            {
                args.insert("run".into(), json!(entry.root.display().to_string()));
            }
            if let Some(status) = query.get("status").filter(|s| !s.is_empty()) {
                args.insert("status".into(), json!(status));
            }
            if let Some(screen_path) = query.get("path").filter(|s| !s.is_empty()) {
                args.insert("path".into(), json!(screen_path));
            }
            if let Some(w) = query.get("width").and_then(|s| s.parse::<u64>().ok()) {
                args.insert("width".into(), json!(w));
            }
            match comments::list(&Value::Object(args)) {
                Ok(reply) => send_json(&mut stream, 200, &reply),
                Err((code, err)) => {
                    send_json(&mut stream, if code == 2 { 400 } else { 500 }, &err);
                }
            }
            return;
        }
        if method != "POST" {
            send_response(&mut stream, 405, "text/plain", "no-store", b"");
            return;
        }
        let Ok(mut body) = serde_json::from_slice::<Value>(&body_bytes) else {
            send_json(
                &mut stream,
                400,
                &json!({"status":"error","reason":"invalid-json"}),
            );
            return;
        };
        if !body.is_object() {
            send_json(
                &mut stream,
                400,
                &json!({"status":"error","reason":"invalid-json"}),
            );
            return;
        }
        let op = body["op"].as_str().unwrap_or("").to_owned();
        if op == "add" {
            if let Some(run_key) = body["run"].as_str().filter(|s| !s.is_empty()) {
                let archive = state
                    .lock()
                    .ok()
                    .map(|g| list_archives(&g))
                    .and_then(|list| {
                        list.into_iter()
                            .find(|a| a.run["key"].as_str() == Some(run_key))
                    });
                let Some(entry) = archive else {
                    send_json(
                        &mut stream,
                        404,
                        &json!({"status":"error","reason":"run-not-found"}),
                    );
                    return;
                };
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("run".into(), json!(entry.root.display().to_string()));
                }
            }
            match comments::add(&body) {
                Ok(reply) => {
                    if let Ok(mut guard) = state.lock() {
                        guard.audit_cache.clear();
                    }
                    send_json(&mut stream, 200, &reply);
                }
                Err((code, err)) => {
                    send_json(&mut stream, if code == 2 { 400 } else { 500 }, &err);
                }
            }
            return;
        }
        if op == "resolve" {
            match comments::resolve(&body) {
                Ok(reply) => {
                    if let Ok(mut guard) = state.lock() {
                        guard.audit_cache.clear();
                    }
                    send_json(&mut stream, 200, &reply);
                }
                Err((code, err)) => {
                    send_json(&mut stream, if code == 2 { 400 } else { 500 }, &err);
                }
            }
            return;
        }
        if op == "reopen" {
            match comments::reopen(&body) {
                Ok(reply) => {
                    if let Ok(mut guard) = state.lock() {
                        guard.audit_cache.clear();
                    }
                    send_json(&mut stream, 200, &reply);
                }
                Err((code, err)) => {
                    send_json(&mut stream, if code == 2 { 400 } else { 500 }, &err);
                }
            }
            return;
        }
        send_json(
            &mut stream,
            400,
            &json!({"status":"error","reason":"unknown-operation"}),
        );
        return;
    }

    if path == "/api/diff" || path == "/api/diff/" || path == "/api/diff/overlay" {
        if !is_same_origin(&headers) {
            send_json(
                &mut stream,
                403,
                &json!({"status":"error","reason":"forbidden-origin"}),
            );
            return;
        }
        if method != "GET" {
            send_response(&mut stream, 405, "text/plain", "no-store", b"");
            return;
        }
        let Some(before_key) = query.get("before").filter(|s| !s.is_empty()) else {
            send_json(
                &mut stream,
                400,
                &json!({"status":"error","reason":"missing-before-or-after"}),
            );
            return;
        };
        let Some(after_key) = query.get("after").filter(|s| !s.is_empty()) else {
            send_json(
                &mut stream,
                400,
                &json!({"status":"error","reason":"missing-before-or-after"}),
            );
            return;
        };
        let pair_key = format!("{before_key}:{after_key}");

        if path == "/api/diff/overlay" {
            let Some(file) = query.get("file").filter(|f| is_valid_overlay_name(f)) else {
                send_response(&mut stream, 404, "text/plain", "no-store", b"");
                return;
            };
            let dir = state
                .lock()
                .ok()
                .and_then(|g| g.diff_overlay_dirs.get(&pair_key).cloned());
            if let Some(dir) = dir
                && let Ok(dir_canon) = dir.canonicalize()
                && let Ok(target) = dir.join(file).canonicalize()
                && target.parent() == Some(dir_canon.as_path())
                && let Ok(png) = fs::read(&target)
            {
                send_response(&mut stream, 200, "image/png", "no-store", &png);
                return;
            }
            send_response(&mut stream, 404, "text/plain", "no-store", b"");
            return;
        }

        let (before_root, after_root, overlay_dir) = {
            let Ok(mut guard) = state.lock() else {
                send_json(&mut stream, 500, &json!({"status":"error"}));
                return;
            };
            let archives = list_archives(&guard);
            let before = archives
                .iter()
                .find(|a| a.run["key"].as_str() == Some(before_key.as_str()))
                .map(|a| a.root.clone());
            let after = archives
                .iter()
                .find(|a| a.run["key"].as_str() == Some(after_key.as_str()))
                .map(|a| a.root.clone());
            let (Some(b), Some(a)) = (before, after) else {
                send_json(
                    &mut stream,
                    404,
                    &json!({"status":"error","reason":"run-not-found"}),
                );
                return;
            };
            let dir = guard
                .diff_overlay_dirs
                .entry(pair_key)
                .or_insert_with(|| {
                    std::env::temp_dir().join(format!(
                        "dui-viewer-diff-{}",
                        &sha256_hex(&format!("{}:{}", b.display(), a.display()))[..16]
                    ))
                })
                .clone();
            (b, a, dir)
        };

        let include_pixels =
            !matches!(query.get("pixels").map(String::as_str), Some("0" | "false"));
        let overlay_target = include_pixels.then_some(overlay_dir.as_path());
        match diff::compare_with_options(&before_root, &after_root, overlay_target, include_pixels)
        {
            Ok(parsed) => {
                send_json(
                    &mut stream,
                    200,
                    &json!({
                        "beforeRunKey": before_key,
                        "afterRunKey": after_key,
                        "summary": parsed["summary"],
                        "screens": parsed["screens"],
                    }),
                );
            }
            Err((code, err)) => {
                send_json(&mut stream, if code == 2 { 400 } else { 500 }, &err);
            }
        }
        return;
    }

    if method != "GET" {
        send_response(&mut stream, 405, "text/plain", "no-store", b"");
        return;
    }

    if let Some((bytes, mime)) = canvas_assets::asset(&path) {
        let cache = if path.starts_with("/assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-store"
        };
        send_response(&mut stream, 200, mime, cache, bytes);
        return;
    }

    if !path.contains('.')
        && let Some((bytes, mime)) = canvas_assets::asset("/")
    {
        send_response(&mut stream, 200, mime, "no-store", bytes);
        return;
    }

    send_response(&mut stream, 404, "text/plain", "no-store", b"Not Found");
}

pub fn serve(run_dirs: Vec<PathBuf>, port: u16, open_browser: bool) -> Reply {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|e| failure(3, &format!("failed to bind viewer port: {e}")))?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| failure(3, &e.to_string()))?
        .port();
    let url = format!("http://127.0.0.1:{actual_port}");

    println!("{url}");
    let _ = std::io::stdout().flush();

    if open_browser {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let _ = Command::new(opener).arg(&url).spawn();
    }

    let state = Arc::new(Mutex::new(ViewerState {
        responsive_sessions: BTreeSet::new(),
        live_project: std::env::current_dir()
            .ok()
            .and_then(|p| p.canonicalize().ok()),
        cli_paths: run_dirs,
        extra_saved_paths: BTreeSet::new(),
        diff_overlay_dirs: BTreeMap::new(),
        run_roots: BTreeMap::new(),
        default_run_key: None,
        archive_images: BTreeMap::new(),
        archive_graphs: BTreeMap::new(),
        audit_cache: BTreeMap::new(),
    }));

    for incoming in listener.incoming() {
        let Ok(stream) = incoming else {
            continue;
        };
        let state = Arc::clone(&state);
        thread::spawn(move || handle_connection(stream, state));
    }

    Ok(json!({ "url": url }))
}
