use super::{field, invalid};
use crate::{
    Reply, analysis,
    chromium::{self, Browser},
    gpui,
    host::{RunningHost, ViteHost},
    ports::Host,
    replay::{io, read},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use dui_core::edit::{Decision, decide};
use dui_core::inspect::{Selection, inspect};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::atomic::Ordering,
};
use tempfile::TempDir;

struct GpuiProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl GpuiProcess {
    fn spawn(binary: &Path, init: &Value) -> Result<(Self, Value), (u8, Value)> {
        gpui::NATIVE_LAUNCHES.fetch_add(1, Ordering::Relaxed);
        let mut child = Command::new(binary)
            .arg("--session")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(io)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| io("missing gpui session stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io("missing gpui session stdout"))?;
        writeln!(stdin, "{init}").map_err(io)?;
        stdin.flush().map_err(io)?;
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        stdout.read_line(&mut line).map_err(io)?;
        if line.trim().is_empty() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io("gpui-session-closed"));
        }
        let reply: Value = serde_json::from_str(&line).map_err(io)?;
        if reply["ok"] != true {
            let reason = reply["error"]
                .as_str()
                .unwrap_or("gpui-session-init-failed");
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid(reason));
        }
        Ok((
            Self {
                child,
                stdin,
                stdout,
            },
            reply,
        ))
    }

    fn call(&mut self, request: &Value) -> Reply {
        writeln!(self.stdin, "{request}").map_err(io)?;
        self.stdin.flush().map_err(io)?;
        let mut line = String::new();
        self.stdout.read_line(&mut line).map_err(io)?;
        if line.trim().is_empty() {
            return Err(io("gpui-session-disconnected"));
        }
        let reply: Value = serde_json::from_str(&line).map_err(io)?;
        if reply["ok"] != true {
            let reason = reply["error"].as_str().unwrap_or("gpui-command-failed");
            return Err(invalid(reason));
        }
        Ok(reply)
    }
}

impl Drop for GpuiProcess {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "{}", json!({"op": "close"}));
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct WebBackend {
    browser: Browser,
    _host: RunningHost,
    page: String,
    facts: Vec<Value>,
    baseline: Value,
}

enum Backend {
    Web(Box<WebBackend>),
    Gpui { process: GpuiProcess },
}

pub struct Session {
    backend: Backend,
    profile: TempDir,
    pub environment: Value,
    source_run: Value,
    path: Value,
    edits: Vec<Value>,
    pub cell: Value,
    id: String,
    revision: u64,
    pub provenance: Value,
    receipts: HashMap<String, (Value, Reply)>,
}

impl Session {
    pub fn open(id: &str, request: &Value) -> Result<Self, (u8, Value)> {
        let project = Path::new(field(request, "project")?);
        let directory = Path::new(field(request, "run")?);
        let cell = &request["cell"];
        let state = field(cell, "state")?;
        let width = cell["width"]
            .as_u64()
            .and_then(|w| u32::try_from(w).ok())
            .filter(|w| *w > 0)
            .ok_or_else(|| invalid("invalid-width"))?;
        let archived = read(&directory.join("snapshot.json"))?;
        let current = crate::archive::provenance(project)?;
        if archived["projectId"] != current["projectId"] || archived["app"] != current["app"] {
            return Err(invalid("project-mismatch"));
        }
        let provenance = json!({"archive":{"commit":archived["commit"],"dirty":archived["dirty"],"worktree":archived["worktree"]},"live":current});
        let report = read(&directory.join("report.json"))?;
        let mut rows = report["cells"]
            .as_array()
            .ok_or_else(|| invalid("invalid-report"))?
            .iter()
            .filter(|c| c["state"] == state && c["width"] == width);
        let row = rows.next().ok_or_else(|| invalid("cell-not-found"))?;
        if rows.next().is_some() {
            return Err(invalid("ambiguous-cell"));
        }
        let profile = tempfile::tempdir().map_err(io)?;
        let generated = profile.path().join("map.json");
        let archived_manifest = directory.join("manifest.json");
        let manifest = if request.get("manifest").is_some() {
            Path::new(field(request, "manifest")?)
        } else if archived_manifest.is_file() {
            &archived_manifest
        } else {
            crate::screen_map::discover(project, &generated)?;
            &generated
        };
        if row["scenario"] == "live" {
            return Err(invalid("open-source-cell-for-live-editing"));
        }

        let mut spec = read(manifest)?;
        if gpui::is_gpui_manifest(&spec) {
            let toolchain = gpui::select_toolchain();
            let binary = gpui::build_fixture(project, toolchain.as_deref())?;
            let height = spec["height"].as_u64().unwrap_or(320) as u32;
            let (process, init_reply) = GpuiProcess::spawn(
                &binary,
                &json!({
                    "width": width,
                    "height": height,
                    "input": row["input"],
                }),
            )?;
            let render_path = init_reply["renderPath"].as_str().unwrap_or("unknown");
            let environment = json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "compiler": gpui::rustc_version(toolchain.as_deref()),
                "versions": gpui::lockfile_versions(project),
                "renderer": format!("gpui-kit/{render_path}"),
                "renderPath": render_path,
                "elementTreeAvailable": init_reply["elementTreeAvailable"],
            });
            return Ok(Self {
                backend: Backend::Gpui { process },
                profile,
                environment,
                source_run: archived["id"].clone(),
                path: row["path"].clone(),
                edits: vec![],
                cell: cell.clone(),
                id: id.into(),
                revision: 0,
                provenance,
                receipts: HashMap::new(),
            });
        }

        let analysis = analysis::analyze(project, manifest)?;
        let host = ViteHost.start(project, manifest, &analysis)?;
        if let Some(index) = field(row, "scenario")?.strip_prefix("link-") {
            let index = index
                .parse::<usize>()
                .map_err(|_| invalid("unknown-scenario"))?;
            let path = report["discoveredRoutes"][index]
                .as_str()
                .ok_or_else(|| invalid("unknown-route"))?;
            spec = analysis::linked_spec(&spec, path);
        } else if row["scenario"] != "default" {
            let index = field(row, "scenario")?
                .strip_prefix("case-")
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|n| n.checked_sub(1))
                .ok_or_else(|| invalid("unknown-scenario"))?;
            let overrides = spec["scenarios"][index]
                .as_object()
                .cloned()
                .ok_or_else(|| invalid("unknown-scenario"))?;
            for (k, v) in overrides {
                spec[k] = v;
            }
        }
        if spec["path"] != row["path"] || !row["input"].is_object() {
            return Err(invalid("cell-manifest-mismatch"));
        }
        analysis::apply_generated(&mut spec, &analysis["autoMocks"]);

        let mut browser =
            Browser::local(&chromium::executable(), profile.path(), host.port).map_err(io)?;
        let (_, page, baseline) = browser
            .open_page(
                &spec,
                &row["input"],
                width,
                &profile.path().join("initial.png"),
            )
            .map_err(io)?;
        if baseline["ready"] != true
            || baseline["settled"] != true
            || baseline["path"] != row["path"]
            || baseline["width"] != width
            || !baseline["instances"].is_object()
        {
            return Err(invalid("cell-not-ready"));
        }
        let environment = json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"compiler":analysis["compiler"],"versions":analysis["versions"],"renderer":browser.version().map_err(io)?["product"]});
        let mut session = Self {
            backend: Backend::Web(Box::new(WebBackend {
                browser,
                _host: host,
                page,
                facts: analysis["facts"]
                    .as_array()
                    .cloned()
                    .ok_or_else(|| invalid("invalid-facts"))?,
                baseline,
            })),
            profile,
            environment,
            source_run: archived["id"].clone(),
            path: row["path"].clone(),
            edits: vec![],
            cell: cell.clone(),
            id: id.into(),
            revision: 0,
            provenance,
            receipts: HashMap::new(),
        };
        session.eval_web(&format!("globalThis.__duiLivePage={}", json!(id)))?;
        Ok(session)
    }

    fn eval_web(&mut self, expression: &str) -> Reply {
        let Backend::Web(web) = &mut self.backend else {
            return Err(io("not-a-web-session"));
        };
        let result = web
            .browser
            .call(
                Some(&web.page),
                "Runtime.evaluate",
                json!({"expression":expression,"awaitPromise":true,"returnByValue":true}),
            )
            .map_err(io)?;
        result["result"]
            .get("value")
            .cloned()
            .ok_or_else(|| io("live-page-unavailable"))
    }

    pub fn tick(&mut self) -> Reply {
        match &mut self.backend {
            Backend::Web(_) => self
                .eval_web(&format!("globalThis.__duiLivePage==={}", json!(self.id)))
                .and_then(|same| {
                    if same == true {
                        Ok(same)
                    } else {
                        Err(io("live-page-expired"))
                    }
                }),
            Backend::Gpui { process } => process.call(&json!({"op": "tick"})),
        }
    }

    fn web_instances(&mut self) -> Reply {
        let value = self.eval_web(&format!(
            "globalThis.__duiLivePage==={} ? globalThis.__duiProbe.instances : null",
            json!(self.id)
        ))?;
        if !value.is_object() {
            return Err(io("live-page-expired"));
        }
        Ok(value)
    }

    fn web_controls(
        &self,
        instances: Value,
        slot: Option<String>,
        instance: Option<String>,
        value: Option<Value>,
    ) -> Reply {
        let Backend::Web(web) = &self.backend else {
            return Err(io("not-a-web-session"));
        };
        let q = Selection {
            state: field(&self.cell, "state")?.into(),
            width: self.cell["width"].as_u64().unwrap_or_default() as u32,
            slot,
            instance,
            value,
        };
        let row = json!({"state":q.state,"width":q.width,"observation":{"instances":instances}});
        let mut controls = inspect(&web.facts, &[row], &q).map_err(invalid)?["controls"].take();
        if let Some(items) = controls.as_array_mut() {
            for control in items {
                if let (Some(slot), Some(instance)) =
                    (control["slot"].as_str(), control["instance"].as_str())
                    && let Some(initial) = web.baseline["instances"][slot][instance].get("value")
                {
                    control["initialValue"] = initial.clone();
                }
            }
        }
        Ok(controls)
    }

    fn frame(&mut self) -> Reply {
        match &mut self.backend {
            Backend::Web(_) => {
                self.tick()?;
                let Backend::Web(web) = &mut self.backend else {
                    return Err(io("not-a-web-session"));
                };
                let image = web
                    .browser
                    .call(
                        Some(&web.page),
                        "Page.captureScreenshot",
                        json!({"format":"png"}),
                    )
                    .map_err(io)?;
                Ok(json!({
                    "session": self.id,
                    "cell": self.cell,
                    "revision": self.revision,
                    "mimeType": "image/png",
                    "imageBase64": field(&image, "data")?,
                }))
            }
            Backend::Gpui { process } => {
                let target = self
                    .profile
                    .path()
                    .join(format!("frame-{}.png", self.revision));
                let _ = process.call(&json!({
                    "op": "frame",
                    "imagePath": target.display().to_string(),
                }))?;
                let bytes = fs::read(&target).map_err(io)?;
                Ok(json!({
                    "session": self.id,
                    "cell": self.cell,
                    "revision": self.revision,
                    "mimeType": "image/png",
                    "imageBase64": STANDARD.encode(bytes),
                }))
            }
        }
    }

    pub fn handle(&mut self, r: &Value) -> Reply {
        let accepted = std::time::Instant::now();
        let op = field(r, "op")?;
        if r["cell"] != self.cell {
            return Err(invalid("cell-mismatch"));
        }
        if op == "inspect" {
            let controls = match &mut self.backend {
                Backend::Web(_) => {
                    let instances = self.web_instances()?;
                    self.web_controls(instances, None, None, None)?
                }
                Backend::Gpui { process } => {
                    let reply = process.call(&json!({"op": "inspect"}))?;
                    reply["controls"].clone()
                }
            };
            return Ok(json!({
                "session": self.id,
                "cell": self.cell,
                "revision": self.revision,
                "controls": controls,
                "observation": "live",
            }));
        }
        if op == "frame" {
            return self.frame();
        }
        if !matches!(op, "set" | "reset" | "save") {
            return Err(invalid("unknown-operation"));
        }
        let key = field(r, "requestId")?;
        let result = match decide(self.revision, r, self.receipts.get(key)) {
            Decision::Replay(result) => return result.clone(),
            Decision::Reject("request-id-reused") => return Err(invalid("request-id-reused")),
            Decision::Reject(reason) => Err(invalid(reason)),
            Decision::Write { .. } if op == "save" => self.save(),
            Decision::Write { revision } => self.edit(op, r, revision).map(|mut body| {
                body["launches"] = super::launches();
                body["elapsedMs"] = json!(accepted.elapsed().as_secs_f64() * 1000.0);
                body["op"] = op.into();
                self.edits.push(body.clone());
                body
            }),
        };
        // A failed archive write publishes nothing. The same request may be retried.
        if op == "save" && matches!(result, Err((3, _))) {
            return result;
        }
        self.receipts
            .insert(key.into(), (r.clone(), result.clone()));
        result
    }

    fn edit(&mut self, op: &str, r: &Value, revision: u64) -> Reply {
        let slot = field(r, "slot")?;
        let instance = field(r, "instance")?;
        match &mut self.backend {
            Backend::Gpui { process } => {
                let mut cmd = json!({
                    "op": op,
                    "slot": slot,
                    "instance": instance,
                });
                if op == "set" {
                    cmd["value"] = r
                        .get("value")
                        .cloned()
                        .ok_or_else(|| invalid("missing-value"))?;
                }
                if let Some(expect) = r.get("expect").filter(|v| !v.is_null()) {
                    cmd["expect"] = expect.clone();
                }
                let mut result = process.call(&cmd)?;
                self.revision = revision;
                for k in ["session", "cell", "slot", "instance", "requestId"] {
                    result[k] = r[k].clone();
                }
                result["revision"] = self.revision.into();
                Ok(result)
            }
            Backend::Web(web) => {
                let value = if op == "reset" {
                    web.baseline["instances"][slot][instance]
                        .get("value")
                        .ok_or_else(|| invalid("no-baseline"))?
                } else {
                    r.get("value").ok_or_else(|| invalid("missing-value"))?
                }
                .clone();
                let expect = r.get("expect").filter(|v| !v.is_null());
                let (selector, text) = match expect {
                    Some(expect) => (
                        Some(field(expect, "selector")?),
                        Some(
                            expect["text"]
                                .as_str()
                                .ok_or_else(|| invalid("missing-expected-text"))?,
                        ),
                    ),
                    None => (None, None),
                };
                let instances = self.web_instances()?;
                self.web_controls(
                    instances,
                    Some(slot.into()),
                    Some(instance.into()),
                    Some(value.clone()),
                )?;
                let expr = format!(
                    "globalThis.__duiLivePage==={} ? globalThis.__duiRuntime.liveEdit({},{},{},{},{}) : null",
                    json!(self.id),
                    json!(slot),
                    json!(instance),
                    value,
                    json!(selector),
                    json!(text)
                );
                let mut result = self.eval_web(&expr)?;
                if result.is_null() {
                    return Err(io("live-page-expired"));
                }
                if let Some(reason) = result["error"].as_str() {
                    return Err(invalid(reason));
                }
                result["expectedValue"] = value;
                self.revision = revision;
                for k in ["session", "cell", "slot", "instance", "requestId"] {
                    result[k] = r[k].clone();
                }
                result["revision"] = self.revision.into();
                Ok(result)
            }
        }
    }

    fn save(&mut self) -> Reply {
        let result = self
            .edits
            .last()
            .cloned()
            .ok_or_else(|| invalid("no-edited-revision"))?;

        match &mut self.backend {
            Backend::Gpui { process } => {
                let target = self
                    .profile
                    .path()
                    .join(format!("save-{}.png", self.revision));
                let frame = process.call(&json!({
                    "op": "frame",
                    "imagePath": target.display().to_string(),
                }))?;
                let bytes = fs::read(&target).map_err(io)?;
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let filename = format!("{digest}.png");
                let reason = if result["status"] != "verified" {
                    result["reason"].as_str().or(Some("unverified-edit"))
                } else {
                    None
                };
                let observation = json!({
                    "model": frame["model"],
                    "controls": frame["controls"],
                    "elements": frame["elements"],
                    "pixelWidth": frame["pixelWidth"],
                    "pixelHeight": frame["pixelHeight"],
                });
                let state = format!(
                    "live-{:x}",
                    Sha256::digest(
                        json!([self.source_run, self.cell, self.revision, observation]).to_string()
                    )
                );
                let cell = json!({
                    "path": self.path,
                    "scenario": "live",
                    "state": state,
                    "width": self.cell["width"],
                    "planIndex": 0,
                    "status": if reason.is_some() { "unverified" } else { "verified" },
                    "reason": reason,
                    "image": filename,
                    "imageSha256": digest,
                    "observation": observation,
                    "assertion": {
                        "expectedValue": result["expectedValue"],
                        "observedValue": result["observedValue"],
                        "slot": result["slot"],
                        "instance": result["instance"],
                        "evidence": result["evidence"],
                    },
                });
                let report = json!({
                    "runtime": "gpui-kit",
                    "path": self.path,
                    "live": {
                        "sourceRun": self.source_run,
                        "sourceCell": self.cell,
                        "revision": self.revision,
                        "edits": self.edits,
                        "environment": self.environment,
                    },
                    "coverage": {
                        "verified": u8::from(reason.is_none()),
                        "renderable": 1,
                        "attempted": 1,
                        "unrenderable": 0,
                    },
                    "cells": [cell],
                });
                let published = crate::archive::publish(self.provenance["live"].clone(), |dir| {
                    fs::create_dir(dir).map_err(io)?;
                    fs::write(dir.join(&filename), &bytes).map_err(io)?;
                    analysis::store(&dir.join("report.json"), &report)?;
                    let graph =
                        crate::graph_view::with_cells(json!({"nodes":[],"edges":[]}), &report)?;
                    analysis::store(&dir.join("graph.json"), &graph)?;
                    Ok(report)
                })?;
                Ok(json!({
                    "project": published["snapshot"]["projectId"],
                    "run": published["snapshot"]["id"],
                    "output": published["output"],
                    "cell": { "state": state, "width": self.cell["width"] },
                    "revision": self.revision,
                    "status": cell["status"],
                    "reason": reason,
                    "image": cell["image"],
                    "imageSha256": digest,
                }))
            }
            Backend::Web(web) => {
                let facts = web.facts.clone();
                let selector = result["evidence"].get("selector").filter(|v| v.is_string());
                let sample = match selector {
                    Some(selector) => format!(
                        "globalThis.__duiLivePage==={} ? ({{instances:globalThis.__duiProbe.instances,visibleText:globalThis.__duiRuntime.visibleText({selector}),path:location.pathname+location.search+location.hash,width:innerWidth}}) : null",
                        json!(self.id)
                    ),
                    None => format!(
                        "globalThis.__duiLivePage==={} ? ({{instances:globalThis.__duiProbe.instances,path:location.pathname+location.search+location.hash,width:innerWidth}}) : null",
                        json!(self.id)
                    ),
                };
                let before = self.eval_web(&sample)?;
                let Backend::Web(web) = &mut self.backend else {
                    return Err(io("not-a-web-session"));
                };
                let image = web
                    .browser
                    .call(
                        Some(&web.page),
                        "Page.captureScreenshot",
                        json!({"format":"png"}),
                    )
                    .map_err(io)?;
                let bytes = STANDARD.decode(field(&image, "data")?).map_err(io)?;
                let observation = self.eval_web(&sample)?;
                if !observation.is_object() {
                    return Err(io("live-page-expired"));
                }
                let slot = field(&result, "slot")?;
                let instance = field(&result, "instance")?;
                let value = observation["instances"][slot][instance].get("value");
                let reason = if before != observation {
                    Some("changed-during-capture")
                } else if observation["path"] != self.path
                    || observation["width"] != self.cell["width"]
                {
                    Some("cell-mismatch")
                } else if result["status"] != "verified" {
                    result["reason"].as_str().or(Some("unverified-edit"))
                } else if value != result.get("expectedValue") {
                    Some("value-mismatch")
                } else if selector.is_some()
                    && observation["visibleText"] != result["evidence"]["expectedText"]
                {
                    Some("semantic-mismatch")
                } else {
                    None
                };
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let filename = format!("{digest}.png");
                let state = format!(
                    "live-{:x}",
                    Sha256::digest(
                        json!([self.source_run, self.cell, self.revision, observation]).to_string()
                    )
                );
                let assertion = match selector {
                    Some(selector) => json!({
                        "selector": selector,
                        "expectedText": result["evidence"]["expectedText"],
                        "visibleText": observation["visibleText"],
                        "expectedValue": result["expectedValue"],
                        "observedValue": value,
                        "slot": slot,
                        "instance": instance,
                    }),
                    None => json!({
                        "expectedValue": result["expectedValue"],
                        "observedValue": value,
                        "slot": slot,
                        "instance": instance,
                        "evidence": result["evidence"],
                    }),
                };
                let cell = json!({"path":self.path,"scenario":"live","state":state,"width":self.cell["width"],"planIndex":0,
                    "status":if reason.is_some(){"unverified"}else{"verified"},"reason":reason,"image":filename,"imageSha256":digest,"observation":observation,
                    "assertion":assertion});
                let report = json!({"path":self.path,"live":{"sourceRun":self.source_run,"sourceCell":self.cell,"revision":self.revision,"edits":self.edits,"environment":self.environment},
                    "coverage":{"verified":u8::from(reason.is_none()),"renderable":1,"attempted":1,"unrenderable":0},"cells":[cell]});
                let published = crate::archive::publish(self.provenance["live"].clone(), |dir| {
                    fs::create_dir(dir).map_err(io)?;
                    fs::write(dir.join(&filename), &bytes).map_err(io)?;
                    analysis::store(
                        &dir.join("facts.json"),
                        &json!({"facts":facts,"compiler":self.environment["compiler"],"versions":self.environment["versions"]}),
                    )?;
                    analysis::store(&dir.join("report.json"), &report)?;
                    let graph =
                        crate::graph_view::with_cells(json!({"nodes":[],"edges":[]}), &report)?;
                    analysis::store(&dir.join("graph.json"), &graph)?;
                    Ok(report)
                })?;
                Ok(json!({
                    "project": published["snapshot"]["projectId"],
                    "run": published["snapshot"]["id"],
                    "output": published["output"],
                    "cell": { "state": state, "width": self.cell["width"] },
                    "revision": self.revision,
                    "status": cell["status"],
                    "reason": reason,
                    "image": cell["image"],
                    "imageSha256": digest,
                }))
            }
        }
    }
}
