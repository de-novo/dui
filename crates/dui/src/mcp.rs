use crate::{analysis, archive, archive::Capture, failure, replay};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::Path,
};
fn schema(names: &[&str]) -> Value {
    let properties: serde_json::Map<String, Value> = names
        .iter()
        .map(|name| (name.to_string(), json!({"type":"string"})))
        .collect();
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":names})
}
fn capture_schema(required: &[&str], with_force: bool) -> Value {
    let mut value = schema(required);
    value["properties"]["output"] = json!({"type":"string"});
    if with_force {
        value["properties"]["force"] = json!({"type":"boolean"});
    }
    value
}
fn recheck_tool() -> Value {
    let mut inputs = schema(&["run", "project"]);
    inputs["properties"]["manifest"] = json!({"type":"string"});
    json!({"name":"recheck","description":"Re-render only unsuccessful cells from an archived run using the current project","inputSchema":inputs})
}
fn live_tool() -> Value {
    json!({
        "name":"live",
        "description":"Open one archived cell, inspect its running controls, set/reset one value with a visible text assertion, measure and resize a mounted web viewport or CSS query container, sample a size interval, fill a web text field, save the revision, or close the session. Open requires project, run, cell and requestId; manifest is optional for map captures. Edits require session, cell, slot, instance, expectedRevision, requestId and expect.",
        "inputSchema":{
            "type":"object","required":["op"],"additionalProperties":false,
            "properties":{
                "op":{"enum":["open","inspect","frame","layout","resize","sample","fill","set","reset","save","close"]},
                "project":{"type":"string"},"manifest":{"type":"string"},"run":{"type":"string"},
                "session":{"type":"string"},"requestId":{"type":"string"},
                "cell":{"type":"object","required":["state","width"],"properties":{"state":{"type":"string"},"width":{"type":"integer","minimum":1}}},
                "expectedRevision":{"type":"integer","minimum":0},
                "axis":{"enum":["width","height"]},"min":{"type":"integer","minimum":1},"max":{"type":"integer","maximum":crate::live::MAX_LAYOUT_SIZE},"budget":{"type":"integer","minimum":2,"maximum":crate::live::MAX_LAYOUT_SAMPLES},
                "width":{"type":"integer","minimum":1,"maximum":crate::live::MAX_LAYOUT_SIZE},"height":{"type":"integer","minimum":1,"maximum":crate::live::MAX_LAYOUT_SIZE},"container":{"type":"integer","minimum":1},"field":{"type":"integer","minimum":1},"text":{"type":"string"},
                "slot":{"type":"string"},"instance":{"type":"string"},"value":{},
                "expect":{"type":"object","properties":{"selector":{"type":"string"},"text":{"type":"string"},"label":{"type":"string"},"checked":{"type":"boolean"}}}
            }
        }
    })
}
fn dispatch(r: &Value) -> Result<Value, (i32, String)> {
    match r["method"].as_str() {
        Some("initialize") => Ok(
            json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"dui","version":crate::update::VERSION}}),
        ),
        Some("ping") => Ok(json!({})),
        Some("tools/list") => Ok(
            json!({"tools":[live_tool(),recheck_tool(),{"name":"inspect","description":"Inspect committed state observations in one archived cell; optionally validate a proposed value","inputSchema":{"type":"object","additionalProperties":false,"required":["directory","selection"],"properties":{"directory":{"type":"string"},"selection":{"type":"object","required":["state","width"],"additionalProperties":false,"properties":{"state":{"type":"string"},"width":{"type":"integer","minimum":1},"slot":{"type":"string"},"instance":{"type":"string"},"value":{}}}}}},{"name":"diff","description":"Compare two captures (by directories or by git branch names via SQLite) and list only added, removed, or changed cells with changed source files and pixel-diff metrics","inputSchema":{"type":"object","additionalProperties":false,"properties":{"before":{"type":"string"},"after":{"type":"string"},"baseBranch":{"type":"string"},"headBranch":{"type":"string"},"project":{"type":"string"},"overlayDir":{"type":"string"},"regressionsOnly":{"type":"boolean","description":"Return only newly failing verification checks and their comparison counts"}}}},{"name":"transitions","description":"Replay screen/state transition actions from initial state in Chromium and mark edges as extracted, inferred, or verified","inputSchema":{"type":"object","additionalProperties":false,"required":["project","manifest"],"properties":{"project":{"type":"string"},"manifest":{"type":"string"},"runDir":{"type":"string"}}}},{"name":"comment_add","description":"Attach a review comment to a screen, state, width/cell, or transition stored in ~/.dui/dui.db","inputSchema":{"type":"object","required":["target","body"],"properties":{"run":{"type":"string"},"projectId":{"type":"string"},"runId":{"type":"string"},"author":{"type":"string"},"body":{"type":"string"},"target":{"type":"object","required":["kind"],"properties":{"kind":{"enum":["screen","state","cell","width","transition","edge"]},"path":{"type":"string"},"state":{"type":"string"},"width":{"type":"integer","minimum":1},"to":{"type":"string"}}}}}},{"name":"comment_list","description":"List review comments filtered by project, run, status, path, or width","inputSchema":{"type":"object","properties":{"run":{"type":"string"},"projectId":{"type":"string"},"runId":{"type":"string"},"status":{"enum":["open","resolved"]},"path":{"type":"string"},"width":{"type":"integer","minimum":1}}}},{"name":"comment_resolve","description":"Mark an open review comment as resolved","inputSchema":schema(&["id"])},{"name":"comment_reopen","description":"Reopen a resolved review comment","inputSchema":schema(&["id"])},{"name":"audit","description":"Audit captured run coverage against the 99% goal with verified/renderable counts, gap reasons, and unresolved review comments","inputSchema":{"type":"object","required":["run"],"properties":{"run":{"type":"string"},"goal":{"type":"number","minimum":0,"maximum":100}}}},{"name":"map","description":"Discover and capture project screens","inputSchema":schema(&["project"])},{"name":"history","description":"List archived captures across local projects, branches and worktrees","inputSchema":schema(&[])},{"name":"prune","description":"Remove unreferenced blobs and optionally cap cache size without breaking listed captures","inputSchema":{"type":"object","additionalProperties":false,"properties":{"maxBytes":{"type":"integer","minimum":0}}}},{"name":"scan","description":"Extract facts from an installed project compiler","inputSchema":schema(&["project","manifest"])},{"name":"run","description":"Build and exercise a Vite application on loopback","inputSchema":capture_schema(&["project","manifest"], true)},{"name":"replay","description":"Render every cell of a saved plan and compare unbound states against independent setter replays","inputSchema":capture_schema(&["project","manifest","plan"], false)},{"name":"replay_report","description":"Verify and summarize saved replay evidence","inputSchema":schema(&["manifest","plan","output"])}]}),
        ),
        Some("tools/call") => {
            let args = &r["params"]["arguments"];
            let path = |k: &str| {
                args[k]
                    .as_str()
                    .map(Path::new)
                    .ok_or((-32602, format!("missing {k}")))
            };
            let result = match r["params"]["name"].as_str() {
                Some("live") => crate::live::command(args),
                Some("recheck") => crate::recheck::run(
                    path("run")?,
                    path("project")?,
                    args.get("manifest").map(|_| path("manifest")).transpose()?,
                ),
                Some("inspect") => {
                    crate::inspect::archive(path("directory")?, args["selection"].clone())
                }
                Some("diff") => {
                    let overlay_dir = args
                        .get("overlayDir")
                        .map(|_| path("overlayDir"))
                        .transpose()?;
                    let regressions_only = match args.get("regressionsOnly") {
                        None => false,
                        Some(value) => value
                            .as_bool()
                            .ok_or((-32602, "regressionsOnly must be boolean".into()))?,
                    };
                    let mode = crate::diff::mode(regressions_only, overlay_dir);
                    if let Some(base_branch) = args["baseBranch"].as_str().filter(|s| !s.is_empty())
                    {
                        let project = args
                            .get("project")
                            .map(|_| path("project"))
                            .transpose()?
                            .unwrap_or_else(|| Path::new("."));
                        let head_branch = args["headBranch"].as_str().filter(|s| !s.is_empty());
                        crate::diff::compare_branches(project, base_branch, head_branch, mode)
                    } else {
                        crate::diff::compare(path("before")?, path("after")?, mode)
                    }
                }
                Some("transitions") => {
                    let run_dir = args.get("runDir").map(|_| path("runDir")).transpose()?;
                    crate::transition::replay(path("project")?, path("manifest")?, run_dir)
                }
                Some("comment_add") => crate::comments::add(args),
                Some("comment_list") => crate::comments::list(args),
                Some("comment_resolve") => crate::comments::resolve(args),
                Some("comment_reopen") => crate::comments::reopen(args),
                Some("audit") => crate::audit::command(args),
                Some("map") => crate::screen_map::capture(path("project")?),
                Some("scan") => analysis::analyze(path("project")?, path("manifest")?),
                Some("replay_report") => {
                    replay::summarize(path("manifest")?, path("plan")?, path("output")?)
                }
                Some("history") => archive::history(),
                Some("prune") => archive::prune(args["maxBytes"].as_u64()),
                Some(name @ ("run" | "replay")) => {
                    let output = args.get("output").map(|_| path("output")).transpose()?;
                    let action = if name == "run" {
                        Capture::Run {
                            force: args["force"].as_bool().unwrap_or(false),
                        }
                    } else {
                        Capture::Replay(path("plan")?, None)
                    };
                    archive::capture(path("project")?, path("manifest")?, output, action)
                }
                _ => return Err((-32602, "unknown tool".into())),
            };
            let (failed, body) = match result {
                Ok(v) => (false, v),
                Err((_, v)) => (true, v),
            };
            Ok(json!({"isError":failed,"content":[{"type":"text","text":body.to_string()}]}))
        }
        _ => Err((-32601, "unknown method".into())),
    }
}
pub fn serve() -> Result<(), (u8, Value)> {
    let mut out = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|e| failure(3, &e.to_string()))?;
        let request: Value = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => {
                writeln!(out,"{}",json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"invalid JSON"}})).map_err(|e|failure(3,&e.to_string()))?;
                continue;
            }
        };
        let Some(id) = request.get("id") else {
            continue;
        };
        let response = match dispatch(&request) {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, msg)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}})
            }
        };
        writeln!(out, "{response}")
            .and_then(|_| out.flush())
            .map_err(|e| failure(3, &e.to_string()))?;
    }
    Ok(())
}
