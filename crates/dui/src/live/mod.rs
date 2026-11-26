//! One loopback owner per open cell. Only its local clients receive the session ID.
mod session;
pub(crate) const MAX_LAYOUT_SIZE: u64 = 4096;
pub(crate) const MAX_LAYOUT_SAMPLES: u64 = 32;
use crate::{
    Reply, analysis, failure,
    replay::{io, read},
};
use serde_json::{Value, json};
use session::Session;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn launches() -> Value {
    use std::sync::atomic::Ordering::Relaxed;
    let host_builds = crate::host::HOST_BUILDS.load(Relaxed);
    let browser_launches = crate::chromium::BROWSER_LAUNCHES.load(Relaxed);
    let native_launches = crate::gpui::NATIVE_LAUNCHES.load(Relaxed);
    if native_launches > 0 {
        json!({"hostBuilds":host_builds,"browserLaunches":browser_launches,"nativeLaunches":native_launches})
    } else {
        json!({"hostBuilds":host_builds,"browserLaunches":browser_launches})
    }
}
fn invalid(s: &str) -> (u8, Value) {
    failure(2, s)
}
fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str, (u8, Value)> {
    v[k].as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid(&format!("missing {k}")))
}
fn root() -> Result<PathBuf, (u8, Value)> {
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| invalid("invalid HOME"))?
        .join(".dui/live");
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(io)?;
    let meta = fs::symlink_metadata(&dir).map_err(io)?;
    if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
        return Err(invalid("live registry must be private"));
    }
    Ok(dir)
}
fn random() -> Result<String, (u8, Value)> {
    let mut bytes = [0; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(io)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn decode(v: Value) -> Reply {
    if v["ok"] == true {
        Ok(v["body"].clone())
    } else {
        Err((v["code"].as_u64().unwrap_or(3) as u8, v["body"].clone()))
    }
}
fn encode(result: Reply) -> Value {
    match result {
        Ok(body) => json!({"ok":true,"body":body}),
        Err((code, body)) => json!({"ok":false,"code":code,"body":body}),
    }
}
fn send(record: &Value, r: &Value) -> Reply {
    let port = record["port"]
        .as_u64()
        .and_then(|p| u16::try_from(p).ok())
        .ok_or_else(|| invalid("invalid endpoint"))?;
    let mut stream =
        TcpStream::connect((Ipv4Addr::LOCALHOST, port)).map_err(|_| invalid("session-expired"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(io)?;
    writeln!(stream, "{}", json!({"token":record["token"],"request":r})).map_err(io)?;
    let response = serde_json::from_reader(BufReader::new(stream)).map_err(io)?;
    decode(response)
}
pub fn command(r: &Value) -> Reply {
    if field(r, "op")? == "open" {
        return open(r);
    }
    let id = field(r, "session")?;
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("invalid-session"));
    }
    let path = root()?.join(id);
    if !active(&path) {
        return Err(invalid("session-expired"));
    }
    let record = read(&path).map_err(|_| invalid("session-expired"))?;
    send(&record, r)
}
fn active(path: &std::path::Path) -> bool {
    File::open(path)
        .is_ok_and(|file| matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock)))
}
fn open(request: &Value) -> Reply {
    let dir = root()?;
    let mut r = request.clone();
    for k in ["project", "manifest", "run"] {
        if k == "manifest" && request.get(k).is_none() {
            continue;
        }
        r[k] = json!(fs::canonicalize(field(request, k)?).map_err(io)?);
    }
    let key = json!(format!(
        "{:x}",
        Sha256::digest(field(&r, "requestId")?.as_bytes())
    ));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(dir.join("open.lock"))
        .map_err(io)?;
    lock.lock().map_err(io)?;
    for entry in fs::read_dir(&dir).map_err(io)? {
        let path = entry.map_err(io)?.path();
        if path.file_name().is_none_or(|n| n.len() != 64) {
            continue;
        }
        if !active(&path) {
            continue;
        }
        if let Ok(record) = read(&path)
            && record["key"] == key
        {
            if record["open"] != r {
                return Err(invalid("request-id-reused"));
            }
            return send(&record, &json!({"op":"opened","session":record["session"]}));
        }
    }
    // A warm owner for the same project, run and source state adds the session to its browser.
    if let Some(pool) = pool_key(&r) {
        let path = dir.join(format!("pool-{pool}"));
        if active(&path)
            && let Ok(record) = read(&path)
            && let Ok(opened) = send(&record, &json!({"op":"attach","open":r}))
        {
            return Ok(opened);
        }
    }
    let mut child = Command::new(std::env::current_exe().map_err(io)?)
        .arg("live-owner")
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(io)?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io("missing owner stdin"))?;
    serde_json::to_writer(stdin, &r).map_err(io)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io("missing owner stdout"))?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).map_err(io)?;
    decode(serde_json::from_str(&line).map_err(io)?)
}
struct Registration(PathBuf);
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn owner() -> Reply {
    // Startup errors use the same frame as the ready handshake.
    if let Err(error) = serve() {
        println!("{}", encode(Err(error)));
    }
    Ok(Value::Null)
}
/// Sessions sharing one workbench: same project, run, manifest choice and source state.
/// `None` when the source state is unknown, so nothing is reused across edits.
fn pool_key(open: &Value) -> Option<String> {
    let project = open["project"].as_str()?;
    let state = source_state(std::path::Path::new(project))?;
    let identity = json!([
        project,
        open["run"],
        open["manifest"],
        open["webLayoutOnly"],
        state
    ]);
    Some(format!("{:x}", Sha256::digest(identity.to_string())))
}

/// The project's commit and every changed or untracked file with its size and modification
/// time, from git. A different value means the host must be built again.
fn source_state(project: &std::path::Path) -> Option<Value> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(project)
            .args(args)
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
    };
    // A repository without commits has no HEAD; its files all show as untracked.
    let status = git(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
    let head = git(&["rev-parse", "HEAD"])
        .and_then(|out| String::from_utf8(out).ok())
        .unwrap_or_default();
    let changed: Vec<Value> = status
        .split(|b| *b == 0)
        .filter(|entry| entry.len() > 3)
        .map(|entry| {
            let path = String::from_utf8_lossy(&entry[3..]).into_owned();
            let meta = fs::metadata(project.join(&path)).ok();
            json!([
                path,
                meta.as_ref().map(fs::Metadata::len),
                meta.and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos().to_string())
            ])
        })
        .collect();
    Some(json!([head.trim(), changed]))
}

/// A session of this owner, with its registration and lease.
struct Live {
    session: Session,
    _lease: File,
    _file: Registration,
    last: Instant,
}

/// Open a session for `open` in this owner and register it for its clients.
fn attach(
    open: &Value,
    bench: &mut Option<std::rc::Rc<session::Workbench>>,
    port: u16,
    token: &str,
) -> Result<(String, Live, Value), (u8, Value)> {
    let id = random()?;
    let session = Session::open(&id, open, bench)?;
    let file = Registration(root()?.join(&id));
    let key = format!("{:x}", Sha256::digest(field(open, "requestId")?.as_bytes()));
    analysis::store(
        &file.0,
        &json!({"port":port,"token":token,"key":key,"open":open,"session":id}),
    )?;
    let lease = File::open(&file.0).map_err(io)?;
    lease.lock().map_err(io)?;
    let opened = json!({"session":id,"revision":0,"cell":session.cell,"provenance":session.provenance,"environment":session.environment,"launches":launches()});
    Ok((
        id,
        Live {
            session,
            _lease: lease,
            _file: file,
            last: Instant::now(),
        },
        opened,
    ))
}

/// How long an owner with a workbench and no sessions stays warm for the next one: long enough
/// for a preview's next request, short enough not to hold a browser's memory while idle.
const POOL_IDLE: Duration = Duration::from_secs(20);

fn serve() -> Result<(), (u8, Value)> {
    let open: Value = serde_json::from_reader(std::io::stdin().lock()).map_err(io)?;
    let token = random()?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(io)?;
    let port = listener.local_addr().map_err(io)?.port();
    let mut bench = None;
    let mut sessions: std::collections::HashMap<String, Live> = Default::default();
    let mut opened = std::collections::HashMap::new();
    let (first, live, reply) = attach(&open, &mut bench, port, &token)?;
    sessions.insert(first.clone(), live);
    opened.insert(first, reply.clone());
    // Only web sessions share a workbench; other owners serve their one session.
    let pool = match (&bench, pool_key(&open)) {
        (Some(_), Some(pool)) => {
            let file = Registration(root()?.join(format!("pool-{pool}")));
            analysis::store(&file.0, &json!({"port":port,"token":token,"pool":pool}))?;
            let lease = File::open(&file.0).map_err(io)?;
            lease.lock().map_err(io)?;
            Some((file, lease))
        }
        _ => None,
    };
    let (tx, connections) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        for connection in listener.incoming() {
            if tx.send(connection).is_err() {
                break;
            }
        }
    });
    writeln!(std::io::stdout(), "{}", encode(Ok(reply))).map_err(io)?;
    let mut empty_since = None;
    loop {
        // Sessions idle for half an hour close; an owner without sessions ends, after a warm
        // period when it holds a workbench.
        sessions.retain(|_, live| live.last.elapsed() < Duration::from_secs(1800));
        if sessions.is_empty() {
            let since = *empty_since.get_or_insert_with(Instant::now);
            if pool.is_none() || since.elapsed() >= POOL_IDLE {
                return Ok(());
            }
        } else {
            empty_since = None;
        }
        let stream = match connections.recv_timeout(Duration::from_secs(1)) {
            Ok(connection) => connection.map_err(io)?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                sessions.retain(|_, live| live.session.tick().is_ok());
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(io)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(io)?;
        let mut bytes = Vec::new();
        if BufReader::new((&stream).take(1024 * 1024))
            .read_until(b'\n', &mut bytes)
            .is_err()
            || bytes.last() != Some(&b'\n')
        {
            continue;
        }
        let Ok(frame) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if frame["token"] != token {
            continue;
        }
        let r = &frame["request"];
        if r["op"] == "attach" && r.get("session").is_none() {
            let result = if bench.is_some() {
                attach(&r["open"], &mut bench, port, &token).map(|(id, live, reply)| {
                    sessions.insert(id.clone(), live);
                    opened.insert(id, reply.clone());
                    reply
                })
            } else {
                Err(invalid("pool-unavailable"))
            };
            let _ = serde_json::to_writer(stream, &encode(result));
            continue;
        }
        let Some(id) = r["session"].as_str().map(str::to_owned) else {
            continue;
        };
        let Some(live) = sessions.get_mut(&id) else {
            continue;
        };
        live.last = Instant::now();
        let close = r["op"] == "close";
        let result = if close {
            Ok(json!({"session":id,"closed":true}))
        } else if r["op"] == "opened" {
            Ok(opened.get(&id).cloned().unwrap_or(Value::Null))
        } else {
            live.session.handle(r)
        };
        let terminate = close
            || (matches!(result, Err((3, _)))
                && (r["op"] != "save" || live.session.tick().is_err()));
        let response = encode(result);
        if terminate {
            sessions.remove(&id);
            opened.remove(&id);
        }
        let _ = serde_json::to_writer(stream, &response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_change_starts_a_new_workbench() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path();
        let open = json!({"project": project, "run": "/run", "webLayoutOnly": true});
        // Outside a repository the source state is unknown: no workbench is shared.
        assert_eq!(pool_key(&open), None);
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(project)
                .args(args)
                .output()
                .is_ok_and(|o| o.status.success());
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        fs::write(project.join("main.tsx"), "export {}").expect("write");
        let first = pool_key(&open).expect("key in a repository without commits");
        assert_eq!(pool_key(&open).as_deref(), Some(first.as_str()));
        // Same size, later time: still a different source state.
        std::thread::sleep(Duration::from_millis(20));
        fs::write(project.join("main.tsx"), "export {}").expect("rewrite");
        let edited = pool_key(&open).expect("key");
        assert_ne!(edited, first);
        // Another run of the same project is another workbench.
        let other = json!({"project": project, "run": "/other", "webLayoutOnly": true});
        assert_ne!(pool_key(&other), Some(edited));
    }
}
