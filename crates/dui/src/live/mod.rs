//! One loopback owner per open cell. Only its local clients receive the session ID.
mod session;
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
    json!({"hostBuilds":crate::host::HOST_BUILDS.load(Relaxed),"browserLaunches":crate::chromium::BROWSER_LAUNCHES.load(Relaxed)})
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
fn serve() -> Result<(), (u8, Value)> {
    let open: Value = serde_json::from_reader(std::io::stdin().lock()).map_err(io)?;
    let id = random()?;
    let token = random()?;
    let mut session = Session::open(&id, &open)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(io)?;

    let file = Registration(root()?.join(&id));
    let key = format!(
        "{:x}",
        Sha256::digest(field(&open, "requestId")?.as_bytes())
    );
    analysis::store(
        &file.0,
        &json!({"port":listener.local_addr().map_err(io)?.port(),"token":token,"key":key,"open":open,"session":id}),
    )?;
    let lease = File::open(&file.0).map_err(io)?;
    lease.lock().map_err(io)?;
    let opened = json!({"session":id,"revision":0,"cell":session.cell,"provenance":session.provenance,"environment":session.environment,"launches":launches()});
    let (tx, connections) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        for connection in listener.incoming() {
            if tx.send(connection).is_err() {
                break;
            }
        }
    });
    writeln!(std::io::stdout(), "{}", encode(Ok(opened.clone()))).map_err(io)?;
    let mut last = Instant::now();
    while last.elapsed() < Duration::from_secs(1800) {
        let stream = match connections.recv_timeout(Duration::from_secs(1)) {
            Ok(connection) => connection.map_err(io)?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if session.tick().is_err() {
                    break;
                }
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
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
        let r = &frame["request"];
        if frame["token"] != token || r["session"] != id {
            continue;
        }
        last = Instant::now();
        let close = r["op"] == "close";
        let result = if close {
            Ok(json!({"session":id,"closed":true}))
        } else if r["op"] == "opened" {
            Ok(opened.clone())
        } else {
            session.handle(r)
        };
        let terminate = close
            || (matches!(result, Err((3, _))) && (r["op"] != "save" || session.tick().is_err()));
        let response = encode(result);
        if terminate {
            drop(session);
            drop(file);
            let _ = serde_json::to_writer(stream, &response);
            return Ok(());
        }
        let _ = serde_json::to_writer(stream, &response);
    }
    Ok(())
}
