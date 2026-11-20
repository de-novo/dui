use super::{field, invalid};
use crate::{
    Reply, analysis,
    chromium::{self, Browser},
    host::{RunningHost, ViteHost},
    ports::Host,
    replay::{io, read},
};
use dui_core::edit::{Decision, decide};
use dui_core::inspect::{Selection, inspect};
use serde_json::{Value, json};
use std::{collections::HashMap, path::Path};
use tempfile::TempDir;

pub struct Session {
    browser: Browser,
    _host: RunningHost,
    _profile: TempDir,
    page: String,
    facts: Vec<Value>,
    baseline: Value,
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
        let manifest = if request.get("manifest").is_some() {
            Path::new(field(request, "manifest")?)
        } else {
            crate::screen_map::discover(project, &generated)?;
            &generated
        };
        if row["scenario"] == "live" {
            return Err(invalid("open-source-cell-for-live-editing"));
        }
        let analysis = analysis::analyze(project, manifest)?;
        let host = ViteHost.start(project, manifest, &analysis)?;
        let mut spec = read(manifest)?;
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
            browser,
            _host: host,
            _profile: profile,
            page,
            facts: analysis["facts"]
                .as_array()
                .cloned()
                .ok_or_else(|| invalid("invalid-facts"))?,
            baseline,
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
        session.eval(&format!("globalThis.__duiLivePage={}", json!(id)))?;
        Ok(session)
    }
    fn eval(&mut self, expression: &str) -> Reply {
        let result = self
            .browser
            .call(
                Some(&self.page),
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
        self.eval(&format!("globalThis.__duiLivePage==={}", json!(self.id)))
            .and_then(|same| {
                if same == true {
                    Ok(same)
                } else {
                    Err(io("live-page-expired"))
                }
            })
    }
    fn instances(&mut self) -> Reply {
        let value = self.eval(&format!(
            "globalThis.__duiLivePage==={} ? globalThis.__duiProbe.instances : null",
            json!(self.id)
        ))?;
        if !value.is_object() {
            return Err(io("live-page-expired"));
        }
        Ok(value)
    }
    fn controls(
        &self,
        instances: Value,
        slot: Option<String>,
        instance: Option<String>,
        value: Option<Value>,
    ) -> Reply {
        let q = Selection {
            state: field(&self.cell, "state")?.into(),
            width: self.cell["width"].as_u64().unwrap_or_default() as u32,
            slot,
            instance,
            value,
        };
        let row = json!({"state":q.state,"width":q.width,"observation":{"instances":instances}});
        Ok(inspect(&self.facts, &[row], &q).map_err(invalid)?["controls"].take())
    }
    pub fn handle(&mut self, r: &Value) -> Reply {
        let accepted = std::time::Instant::now();
        let op = field(r, "op")?;
        if r["cell"] != self.cell {
            return Err(invalid("cell-mismatch"));
        }
        if op == "inspect" {
            let instances = self.instances()?;
            return Ok(
                json!({"session":self.id,"cell":self.cell,"revision":self.revision,"controls":self.controls(instances,None,None,None)?,"observation":"live"}),
            );
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
        let value = if op == "reset" {
            self.baseline["instances"][slot][instance]
                .get("value")
                .ok_or_else(|| invalid("no-baseline"))?
        } else {
            r.get("value").ok_or_else(|| invalid("missing-value"))?
        }
        .clone();
        let selector = field(&r["expect"], "selector")?;
        let text = r["expect"]["text"]
            .as_str()
            .ok_or_else(|| invalid("missing-expected-text"))?;
        let instances = self.instances()?;
        self.controls(
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
        let mut result = self.eval(&expr)?;
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

impl Session {
    fn save(&mut self) -> Reply {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use sha2::{Digest, Sha256};
        let result = self
            .edits
            .last()
            .cloned()
            .ok_or_else(|| invalid("no-edited-revision"))?;
        let selector = &result["evidence"]["selector"];
        let sample = format!(
            "globalThis.__duiLivePage==={} ? ({{instances:globalThis.__duiProbe.instances,visibleText:globalThis.__duiRuntime.visibleText({selector}),path:location.pathname+location.search+location.hash,width:innerWidth}}) : null",
            json!(self.id)
        );
        let before = self.eval(&sample)?;
        let image = self
            .browser
            .call(
                Some(&self.page),
                "Page.captureScreenshot",
                json!({"format":"png"}),
            )
            .map_err(io)?;
        let bytes = STANDARD.decode(field(&image, "data")?).map_err(io)?;
        let observation = self.eval(&sample)?;
        if !observation.is_object() {
            return Err(io("live-page-expired"));
        }
        let slot = field(&result, "slot")?;
        let instance = field(&result, "instance")?;
        let value = observation["instances"][slot][instance].get("value");
        let reason = if before != observation {
            Some("changed-during-capture")
        } else if observation["path"] != self.path || observation["width"] != self.cell["width"] {
            Some("cell-mismatch")
        } else if result["status"] != "verified" {
            result["reason"].as_str().or(Some("unverified-edit"))
        } else if value != result.get("expectedValue") {
            Some("value-mismatch")
        } else if observation["visibleText"] != result["evidence"]["expectedText"] {
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
        let cell = json!({"path":self.path,"scenario":"live","state":state,"width":self.cell["width"],"planIndex":0,
            "status":if reason.is_some(){"unverified"}else{"verified"},"reason":reason,"image":filename,"imageSha256":digest,"observation":observation,
            "assertion":{"selector":selector,"expectedText":result["evidence"]["expectedText"],"visibleText":observation["visibleText"],"expectedValue":result["expectedValue"],"observedValue":value,"slot":slot,"instance":instance}});
        let report = json!({"path":self.path,"live":{"sourceRun":self.source_run,"sourceCell":self.cell,"revision":self.revision,"edits":self.edits,"environment":self.environment},
            "coverage":{"verified":u8::from(reason.is_none()),"renderable":1,"attempted":1,"unrenderable":0},"cells":[cell]});
        let published = crate::archive::publish(self.provenance["live"].clone(), |dir| {
            std::fs::create_dir(dir).map_err(io)?;
            std::fs::write(dir.join(filename), bytes).map_err(io)?;
            analysis::store(
                &dir.join("facts.json"),
                &json!({"facts":self.facts,"compiler":self.environment["compiler"],"versions":self.environment["versions"]}),
            )?;
            analysis::store(&dir.join("report.json"), &report)?;
            let graph = crate::graph_view::with_cells(json!({"nodes":[],"edges":[]}), &report)?;
            crate::graph_view::write(dir, &graph)?;
            analysis::store(&dir.join("graph.json"), &graph)?;
            Ok(report)
        })?;
        Ok(
            json!({"project":published["snapshot"]["projectId"],"run":published["snapshot"]["id"],"output":published["output"],
            "cell":{"state":state,"width":self.cell["width"]},"revision":self.revision,"status":cell["status"],"reason":reason,"image":cell["image"],"imageSha256":digest}),
        )
    }
}
