//! Content-addressed per-cell capture cache backed by any `dui_ports::AppStore`.

use dui_core::model::{
    CacheEntry, CacheInputs, CacheKey, CacheOutcome, CellStatus, ContentHash, Evidence, ImageBlob,
    Screen, StateId, Width, evaluate_cache,
};
use dui_ports::AppStore;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env, fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub enum CacheLookup {
    Hit {
        key: CacheKey,
        image_name: String,
        status: CellStatus,
        observation: Value,
    },
    Miss {
        outcome: CacheOutcome,
    },
}

pub struct CellCache<S: AppStore> {
    store: Option<S>,
    source_files: BTreeMap<String, ContentHash>,
    module_graph: BTreeMap<String, Vec<String>>,
    route_files: BTreeMap<String, BTreeSet<String>>,
    shell_files: BTreeSet<String>,
    locale_files: BTreeMap<String, ContentHash>,
    compiler: Value,
    versions: Value,
    canonical_facts: Vec<Value>,
    types_digest: ContentHash,
    rule_pack_digest: ContentHash,
    renderer_env_digest: ContentHash,
    dui_version: String,
    force: bool,
}

fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    if bytes.len() >= 24 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        (w, h)
    } else {
        (0, 0)
    }
}

fn hash_file(path: &Path) -> Option<ContentHash> {
    fs::read(path).ok().map(|b| ContentHash::from_bytes(&b))
}

fn transitive_closure(
    seeds: &BTreeSet<String>,
    graph: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut queue: VecDeque<String> = seeds.iter().cloned().collect();
    while let Some(file) = queue.pop_front() {
        if !visited.insert(file.clone()) {
            continue;
        }
        if let Some(deps) = graph.get(&file) {
            for dep in deps {
                if !visited.contains(dep) {
                    queue.push_back(dep.clone());
                }
            }
        }
    }
    visited
}

impl<S: AppStore> CellCache<S> {
    pub fn with_store(
        store: Option<S>,
        project_dir: Option<&Path>,
        analysis: &Value,
        spec: &Value,
        browser_version: Option<&Value>,
        force: bool,
    ) -> Self {
        let mut source_files = BTreeMap::new();
        let mut locale_files = BTreeMap::new();
        if let Some(root) = project_dir {
            for rel in analysis["sourceFiles"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if let Some(digest) = hash_file(&root.join(rel)) {
                    source_files.insert(rel.to_owned(), digest);
                }
            }
            for extra in ["index.html", "package.json", "tsconfig.json"] {
                if let Some(digest) = hash_file(&root.join(extra)) {
                    source_files.entry(extra.to_owned()).or_insert(digest);
                }
            }
            let locale_dirs: Vec<std::path::PathBuf> =
                if let Some(rel) = spec["localesDir"].as_str() {
                    vec![root.join(rel)]
                } else {
                    vec![root.join("locales"), root.join("messages")]
                };
            for dir in locale_dirs {
                if !dir.is_dir() {
                    continue;
                }
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("json")
                            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                            && let Some(digest) = hash_file(&path)
                        {
                            locale_files.insert(stem.to_owned(), digest);
                        }
                    }
                }
                if !locale_files.is_empty() {
                    break;
                }
            }
        }

        let canonical_facts: Vec<Value> = analysis["facts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                let mut entry = f.clone();
                if let Some(obj) = entry.as_object_mut()
                    && obj.get("kind").and_then(Value::as_str) == Some("renders")
                    && let Some(target) = obj.get("target").and_then(Value::as_str)
                    && let Some(file) = obj.get("file").and_then(Value::as_str)
                    && target.ends_with(file)
                {
                    obj.insert("target".into(), json!(file));
                }
                entry
            })
            .collect();
        let mut module_graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(obj) = analysis["moduleGraph"].as_object() {
            for (file, deps) in obj {
                let list = deps
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                module_graph.insert(file.clone(), list);
            }
        }

        let mut route_files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut record_route = |path: &str, file: &str| {
            if !path.is_empty() && !file.is_empty() {
                route_files
                    .entry(path.to_owned())
                    .or_default()
                    .insert(file.to_owned());
            }
        };
        for fact in analysis["facts"].as_array().into_iter().flatten() {
            if fact["kind"].as_str() == Some("route")
                && let Some(label) = fact["label"].as_str()
                && let Some(file) = fact["file"].as_str()
            {
                record_route(label, file);
            }
        }
        if let Some(sources) = spec["screenDiscovery"]["sources"].as_object() {
            for (path, src) in sources {
                if let Some(file) = src.as_str() {
                    record_route(path, file);
                }
            }
        }
        for candidate in
            std::iter::once(spec).chain(spec["scenarios"].as_array().into_iter().flatten())
        {
            if let Some(path) = candidate["path"].as_str()
                && let Some(file) = candidate["sourceFile"]
                    .as_str()
                    .or_else(|| candidate["entryFile"].as_str())
            {
                record_route(path, file);
            }
        }

        let mut owned_by_any_screen = BTreeSet::new();
        for seeds in route_files.values() {
            owned_by_any_screen.extend(transitive_closure(seeds, &module_graph));
        }
        let shell_files: BTreeSet<String> = source_files
            .keys()
            .filter(|f| !owned_by_any_screen.contains(*f))
            .cloned()
            .collect();

        let compiler = analysis["compiler"].clone();
        let versions = analysis["versions"].clone();
        let types_digest = ContentHash::from_canonical(
            "dui:types:v1",
            &json!({
                "compiler": &compiler,
                "facts": &canonical_facts,
                "versions": &versions,
            }),
        );

        let mut pack_hashes = BTreeMap::new();
        if let Some(root) = project_dir {
            for entry in spec["rulePacks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let path = Path::new(entry);
                let resolved = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    root.join(path)
                };
                if let Some(digest) = hash_file(&resolved) {
                    pack_hashes.insert(entry.to_owned(), digest);
                }
            }
        }
        let rule_pack_digest = ContentHash::from_canonical(
            "dui:rule-packs:v1",
            &json!({
                "packs": pack_hashes,
                "builtin": env!("CARGO_PKG_VERSION"),
            }),
        );

        let dui_version = env::var("DUI_CACHE_VERSION")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());

        let renderer_override = env::var("DUI_CACHE_RENDERER")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let renderer_env_digest = ContentHash::from_canonical(
            "dui:renderer-env:v4",
            &json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "browser": browser_version,
                "override": renderer_override,
            }),
        );

        Self {
            store,
            source_files,
            module_graph,
            route_files,
            shell_files,
            locale_files,
            compiler,
            versions,
            canonical_facts,
            types_digest,
            rule_pack_digest,
            renderer_env_digest,
            dui_version,
            force,
        }
    }

    /// Key entries by the running build, for example `0.2.0-dev+1e5fe29…`. Development builds
    /// share the crate version, so without this a build with changed verification or discovery
    /// would reuse the previous build's cells. `DUI_CACHE_VERSION` still overrides it.
    pub fn built_by(mut self, build: &str) -> Self {
        if env::var("DUI_CACHE_VERSION").map_or(true, |v| v.trim().is_empty()) {
            self.dui_version = build.to_owned();
        }
        self
    }

    pub fn build(&self) -> &str {
        &self.dui_version
    }

    pub fn is_forced(&self) -> bool {
        self.force
    }

    pub fn source_file_hashes(&self) -> BTreeMap<String, String> {
        let mut map: BTreeMap<String, String> = self
            .source_files
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().to_owned()))
            .collect();
        for (loc, hash) in &self.locale_files {
            map.insert(format!("locale:{loc}"), hash.as_str().to_owned());
        }
        map
    }

    fn scoped_files(&self, path: &str, spec: &Value) -> Option<BTreeSet<String>> {
        let mut seeds = BTreeSet::new();
        if let Some(file) = spec["sourceFile"]
            .as_str()
            .or_else(|| spec["entryFile"].as_str())
        {
            seeds.insert(file.to_owned());
        }
        if let Some(files) = self.route_files.get(path) {
            seeds.extend(files.iter().cloned());
        }
        if seeds.is_empty() {
            return None;
        }
        let mut scoped = transitive_closure(&seeds, &self.module_graph);
        scoped.extend(self.shell_files.iter().cloned());
        Some(scoped)
    }

    pub fn build_inputs(
        &self,
        scenario: &str,
        spec: &Value,
        row: &BTreeMap<String, Value>,
        width: u32,
        locale: Option<&str>,
    ) -> Option<CacheInputs> {
        let width = Width::new(width).ok()?;
        let path = spec["path"].as_str().unwrap_or("/");
        let screen_id = Screen::derive_id(path);
        let state_id = StateId::derive(&json!([screen_id, scenario, locale, row]));
        let (mut sources, types_digest) = if let Some(scoped) = self.scoped_files(path, spec) {
            let files: BTreeMap<String, ContentHash> = self
                .source_files
                .iter()
                .filter(|(k, _)| scoped.contains(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let facts: Vec<&Value> = self
                .canonical_facts
                .iter()
                .filter(|f| {
                    f["file"]
                        .as_str()
                        .map(|file| scoped.contains(file))
                        .unwrap_or(true)
                })
                .collect();
            let digest = ContentHash::from_canonical(
                "dui:types:v1",
                &json!({
                    "compiler": &self.compiler,
                    "facts": facts,
                    "versions": &self.versions,
                }),
            );
            (files, digest)
        } else {
            (self.source_files.clone(), self.types_digest.clone())
        };
        if let Some(loc) = locale
            && let Some(digest) = self.locale_files.get(loc)
        {
            sources.insert(format!("locale:{loc}"), digest.clone());
        }
        let mocks_digest = ContentHash::from_canonical(
            "dui:mocks:v1",
            &json!({
                "cookies": spec["cookies"],
                "storageState": spec["storageState"],
                "localStorage": spec["localStorage"],
                "sessionStorage": spec["sessionStorage"],
                "authFlow": spec["authFlow"],
                "extraHeaders": spec["extraHeaders"],
                "mockResponses": spec["mockResponses"],
                "generatedGetResponses": spec["generatedGetResponses"],
                "generatedCsvResponses": spec["generatedCsvResponses"],
                "requiredRequests": spec["requiredRequests"],
                "expectedApiErrors": spec["expectedApiErrors"],
                "settleTimeoutMs": spec["settleTimeoutMs"],
                "readySelector": spec["readySelector"],
                "slotAssertions": spec["slotAssertions"],
                "routeSelector": spec["routeSelector"],
                "discoverOverlays": spec["discoverOverlays"],
                "hasScreenAssertion": spec["hasScreenAssertion"],
            }),
        );
        Some(CacheInputs {
            source_files: sources,
            types_digest,
            mocks_digest,
            state_id,
            width,
            locale: locale.map(str::to_owned),
            dui_version: self.dui_version.clone(),
            rule_pack_digest: self.rule_pack_digest.clone(),
            renderer_env_digest: self.renderer_env_digest.clone(),
        })
    }

    pub fn lookup(&mut self, inputs: &CacheInputs, out: &Path) -> CacheLookup {
        let key = inputs.key();
        let Some(store) = self.store.as_mut() else {
            return CacheLookup::Miss {
                outcome: evaluate_cache(inputs, None, self.force),
            };
        };

        if !self.force
            && let Ok(Some(mut entry)) = store.get_cache(&key)
            && let Some(ev_id) = entry.evidence_id().cloned()
            && let Ok(Some(evidence)) = store.get_evidence(&ev_id)
            && let Some(blob_id) = entry.image_blob().cloned()
            && store.blob_path(&blob_id).is_file()
        {
            let image_name = format!("{}-{}.png", blob_id.as_str(), inputs.width.pixels());
            if store
                .link_or_copy_blob(&blob_id, &out.join(&image_name))
                .is_ok()
            {
                let mut obs = evidence_to_observation(&evidence);
                if let Some(overlays) = obs.get_mut("overlays").and_then(Value::as_array_mut) {
                    let mut restored = Vec::with_capacity(overlays.len());
                    for ov in overlays.drain(..) {
                        if let (Some(img_name), Some(bid_raw)) =
                            (ov["image"].as_str(), ov["blobId"].as_str())
                            && let Ok(ov_blob_id) = dui_core::model::BlobId::parse(bid_raw)
                            && store.blob_path(&ov_blob_id).is_file()
                            && store
                                .link_or_copy_blob(&ov_blob_id, &out.join(img_name))
                                .is_ok()
                        {
                            restored.push(ov);
                        }
                    }
                    *overlays = restored;
                }
                // The HTML snapshot travels with the cell; without its blob the card uses the PNG.
                let restored_snapshot = match (
                    obs["htmlSnapshot"]["file"].as_str(),
                    obs["htmlSnapshot"]["blobId"].as_str(),
                ) {
                    (Some(file), Some(raw)) => dui_core::model::BlobId::parse(raw)
                        .ok()
                        .filter(|id| store.blob_path(id).is_file())
                        .is_some_and(|id| store.link_or_copy_blob(&id, &out.join(file)).is_ok()),
                    _ => true,
                };
                if !restored_snapshot && let Some(fields) = obs.as_object_mut() {
                    fields.remove("htmlSnapshot");
                }
                let _ = entry.record_hit(now_epoch_ms());
                let _ = store.put_cache(&entry);
                return CacheLookup::Hit {
                    key,
                    image_name,
                    status: entry.status().clone(),
                    observation: obs,
                };
            }
        }

        let previous = store
            .find_previous_cache_inputs(&inputs.state_id, inputs.width, inputs.locale.as_deref())
            .ok()
            .flatten();
        CacheLookup::Miss {
            outcome: evaluate_cache(inputs, previous.as_ref(), self.force),
        }
    }

    pub fn store_rendered(
        &mut self,
        inputs: Option<&CacheInputs>,
        bytes: &[u8],
        out_image: &Path,
        status: &CellStatus,
        obs: &Value,
    ) -> Result<(), String> {
        let Some(store) = self.store.as_mut() else {
            fs::write(out_image, bytes).map_err(|e| e.to_string())?;
            return Ok(());
        };
        let (w, h) = png_dimensions(bytes);
        let blob = ImageBlob::from_bytes("image/png", bytes, w, h).map_err(|e| e.to_string())?;
        store.put_blob(&blob, bytes).map_err(|e| e.to_string())?;
        store
            .link_or_copy_blob(blob.id(), out_image)
            .map_err(|e| e.to_string())?;

        if let Some(inputs) = inputs {
            let mut obs_for_cache = obs.clone();
            if let Some(out_dir) = out_image.parent()
                && let Some(overlays) = obs_for_cache
                    .get_mut("overlays")
                    .and_then(Value::as_array_mut)
            {
                for ov in overlays {
                    if let Some(img_name) = ov["image"].as_str()
                        && let Ok(ov_bytes) = fs::read(out_dir.join(img_name))
                    {
                        let (ow, oh) = png_dimensions(&ov_bytes);
                        if let Ok(ov_blob) = ImageBlob::from_bytes("image/png", &ov_bytes, ow, oh)
                            && store.put_blob(&ov_blob, &ov_bytes).is_ok()
                        {
                            ov["blobId"] = json!(ov_blob.id().as_str());
                        }
                    }
                }
            }
            if let Some(out_dir) = out_image.parent()
                && let Some(file) = obs_for_cache["htmlSnapshot"]["file"].as_str()
                && let Ok(html) = fs::read(out_dir.join(file))
                && let Ok(html_blob) = ImageBlob::from_bytes("text/html", &html, 0, 0)
                && store.put_blob(&html_blob, &html).is_ok()
            {
                obs_for_cache["htmlSnapshot"]["blobId"] = json!(html_blob.id().as_str());
            }
            let evidence = observation_to_evidence(&obs_for_cache);
            let ev_id = evidence.id().clone();
            store.put_evidence(&evidence).map_err(|e| e.to_string())?;
            let blob_ref = (!status.is_unrenderable()).then(|| blob.id().clone());
            if let Ok(entry) = CacheEntry::new(
                inputs.clone(),
                status.clone(),
                blob_ref,
                Some(ev_id),
                now_epoch_ms(),
            ) {
                let _ = store.put_cache(&entry);
            }
        }
        Ok(())
    }
}

fn observation_to_evidence(obs: &Value) -> Evidence {
    let slots = obs["slots"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let hits = obs["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let links = obs["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let responses = obs["responses"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_u64().map(|code| (k.clone(), code as u16)))
                .collect()
        })
        .unwrap_or_default();
    let mut assertions = obs["assertions"].as_array().cloned().unwrap_or_default();
    if let Some(path) = obs["redirectedTo"].as_str() {
        assertions.push(json!({"__duiRedirectedTo": path}));
    }
    if let Some(audit) = obs.get("localeAudit").filter(|v| v.is_object()) {
        assertions.push(json!({"__duiLocaleAudit": audit}));
    }
    if let Some(tree) = obs.get("renderTree").filter(|v| v.is_array()) {
        assertions.push(json!({"__duiRenderTree": tree}));
    }
    if let Some(overlays) = obs.get("overlays").filter(|v| v.is_array()) {
        assertions.push(json!({"__duiOverlays": overlays}));
    }
    // Everything else the capture observed (response checks, API errors, settle evidence,
    // dialogs, witnesses, …), so a cache hit can be judged again exactly as a fresh capture.
    let rest: serde_json::Map<String, Value> = obs
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, _)| !EVIDENCE_FIELDS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if !rest.is_empty() {
        assertions.push(json!({"__duiObservation": rest}));
    }
    Evidence::new(
        obs["path"].as_str().map(str::to_owned),
        obs["width"].as_u64().map(|w| w as u32),
        obs["ready"].as_bool(),
        obs["settled"].as_bool(),
        slots,
        BTreeMap::new(),
        hits,
        links,
        responses,
        assertions,
        vec![],
    )
    .with_screen_evidence(obs.get("screenEvidence"))
}

/// Observation fields stored in `Evidence`'s own slots or as their own assertion items.
const EVIDENCE_FIELDS: [&str; 14] = [
    "path",
    "width",
    "ready",
    "settled",
    "slots",
    "hits",
    "links",
    "responses",
    "assertions",
    "screenEvidence",
    "redirectedTo",
    "localeAudit",
    "renderTree",
    "overlays",
];

fn evidence_to_observation(ev: &Evidence) -> Value {
    let mut assertions = Vec::new();
    let mut rest = None;
    let mut locale_audit = None;
    let mut render_tree = None;
    let mut overlays = None;
    let mut redirected_to = None;
    for item in ev.assertions() {
        if let Some(audit) = item.get("__duiLocaleAudit") {
            locale_audit = Some(audit.clone());
        } else if let Some(tree) = item.get("__duiRenderTree") {
            render_tree = Some(tree.clone());
        } else if let Some(path) = item.get("__duiRedirectedTo") {
            redirected_to = Some(path.clone());
        } else if let Some(ov) = item.get("__duiOverlays") {
            overlays = Some(ov.clone());
        } else if let Some(fields) = item.get("__duiObservation") {
            rest = fields.as_object().cloned();
        } else {
            assertions.push(item.clone());
        }
    }
    let response_checks: BTreeMap<String, bool> = ev
        .responses()
        .iter()
        .map(|(k, code)| (k.clone(), (200..300).contains(code)))
        .collect();
    let mut obs = json!({
        "path": ev.observed_path(),
        "width": ev.observed_width(),
        "ready": ev.ready().unwrap_or(false),
        "settled": ev.settled().unwrap_or(false),
        "slots": ev.slots(),
        "hits": ev.hits().iter().collect::<Vec<_>>(),
        "links": ev.links().iter().collect::<Vec<_>>(),
        "responses": ev.responses(),
        "responseChecks": response_checks,
        "assertions": assertions,
    });
    if let Some(value) = ev.screen_evidence() {
        obs["screenEvidence"] = value.clone();
    }
    if let Some(path) = redirected_to {
        obs["redirectedTo"] = path;
    }
    if let Some(audit) = locale_audit {
        obs["localeAudit"] = audit;
    }
    if let Some(tree) = render_tree {
        obs["renderTree"] = tree;
    }
    if let Some(ov) = overlays {
        obs["overlays"] = ov;
    }
    // The stored fields replace the ones rebuilt above (`responseChecks` is only a stand-in
    // for entries written before the full observation was kept).
    for (key, value) in rest.into_iter().flatten() {
        obs[key] = value;
    }
    obs
}

#[cfg(test)]
mod observation_round_trip {
    use super::{evidence_to_observation, observation_to_evidence};
    use serde_json::json;

    #[test]
    fn a_cached_observation_keeps_every_field_the_verifier_reads() {
        let observed = json!({
            "path": "/orders", "width": 375, "ready": true, "settled": false,
            "slots": {}, "hits": [], "links": ["/"], "responses": {"/orders": 200, "/favicon.ico": 404},
            "responseChecks": {}, "assertions": [true],
            "apiErrors": {"/api/me": 401},
            "unsettled": [{"node": "li#3", "change": "added"}],
            "ignoredChanges": [{"node": "#clock", "reason": "changes every sample"}],
            "overlaySlots": ["main.tsx#1:2:overlay"],
            "visualWitness": {"slot": "visible-change"},
            "renderTree": [{"id": "h1#0", "text": "Orders"}],
        });
        let restored = evidence_to_observation(&observation_to_evidence(&observed));
        assert_eq!(restored, observed);
    }
}
