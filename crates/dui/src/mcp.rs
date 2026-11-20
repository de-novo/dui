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
fn capture_schema(required: &[&str]) -> Value {
    let mut value = schema(required);
    value["properties"]["output"] = json!({"type":"string"});
    value
}
fn dispatch(r: &Value) -> Result<Value, (i32, String)> {
    match r["method"].as_str() {
        Some("initialize") => Ok(
            json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"dui","version":env!("CARGO_PKG_VERSION")}}),
        ),
        Some("ping") => Ok(json!({})),
        Some("tools/list") => Ok(
            json!({"tools":[{"name":"map","description":"Discover and capture project screens","inputSchema":schema(&["project"])},{"name":"history","description":"List archived captures across local projects, branches and worktrees","inputSchema":schema(&[])},{"name":"scan","description":"Extract facts from an installed project compiler","inputSchema":schema(&["project","manifest"])},{"name":"run","description":"Build and exercise a Vite application on loopback","inputSchema":capture_schema(&["project","manifest"])},{"name":"replay","description":"Render every cell of a saved plan and compare unbound states against independent setter replays","inputSchema":capture_schema(&["project","manifest","plan"])},{"name":"replay_report","description":"Verify and summarize saved replay evidence","inputSchema":schema(&["manifest","plan","output"])}]}),
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
                Some("map") => crate::screen_map::capture(path("project")?),
                Some("scan") => analysis::analyze(path("project")?, path("manifest")?),
                Some("replay_report") => {
                    replay::summarize(path("manifest")?, path("plan")?, path("output")?)
                }
                Some("history") => archive::history(),
                Some(name @ ("run" | "replay")) => {
                    let output = args.get("output").map(|_| path("output")).transpose()?;
                    let action = if name == "run" {
                        Capture::Run
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
