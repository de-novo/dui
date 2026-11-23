use crate::{
    Reply, analysis, failure,
    ports::{Renderer, TypeOracle},
    surface::{NoopOracle, SurfaceAdapter},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use dui_core::{
    surface::{
        SURFACE_UNIVERSAL_TUI, SupportTier, SurfaceCapabilities, SurfaceDescriptor, SurfaceKind,
        UnrenderableReason,
    },
    tui::{DEFAULT_BG, TerminalGrid, encode_key_token},
};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const CELL_W: i32 = 12;
const CELL_H: i32 = 20;
const PAD_X: i32 = 16;
const PAD_Y: i32 = 16;
const MAX_TUI_CACHE_FILES: usize = 500;

const PTY_HELPER_PY: &str = r#"
import base64, errno, fcntl, json, os, pty, select, signal, struct, sys, termios, time

cfg = json.loads(sys.stdin.read())
cmd = cfg["command"]
cwd = cfg["cwd"]
cols = int(cfg["cols"])
rows = int(cfg["rows"])
keys = [base64.b64decode(k) for k in cfg.get("keys", [])]
settle_s = max(0.05, float(cfg.get("settleMs", 150)) / 1000.0)
timeout_s = max(0.5, float(cfg.get("timeoutMs", 4000)) / 1000.0)
extra_env = cfg.get("env", {})

master_fd, slave_fd = pty.openpty()
winsize = struct.pack("HHHH", rows, cols, 0, 0)
fcntl.ioctl(slave_fd, termios.TIOCSWINSZ, winsize)

env = os.environ.copy()
env["TERM"] = "xterm-256color"
env["COLORTERM"] = "truecolor"
env["COLUMNS"] = str(cols)
env["LINES"] = str(rows)
for k, v in extra_env.items():
    env[str(k)] = str(v)

pid = os.fork()
if pid == 0:
    os.setsid()
    fcntl.ioctl(slave_fd, termios.TIOCSCTTY, 0)
    os.dup2(slave_fd, 0)
    os.dup2(slave_fd, 1)
    os.dup2(slave_fd, 2)
    os.close(master_fd)
    os.close(slave_fd)
    os.chdir(cwd)
    os.execvpe(cmd[0], cmd, env)

os.close(slave_fd)
chunks = []

def drain(max_wait, idle_wait):
    deadline = time.monotonic() + max_wait
    last_data = time.monotonic()
    got_any = False
    while time.monotonic() < deadline:
        rem = min(0.05, max(0.0, deadline - time.monotonic()))
        r, _, _ = select.select([master_fd], [], [], rem)
        if r:
            try:
                data = os.read(master_fd, 8192)
            except OSError as exc:
                if exc.errno == errno.EIO:
                    return False
                raise
            if not data:
                return False
            chunks.append(data)
            got_any = True
            last_data = time.monotonic()
        elif got_any and (time.monotonic() - last_data) >= idle_wait:
            return True
    return True

try:
    alive = drain(timeout_s, settle_s)
    for key_bytes in keys:
        if not alive:
            break
        os.write(master_fd, key_bytes)
        alive = drain(min(timeout_s, 1.5), settle_s)
finally:
    try:
        os.close(master_fd)
    except OSError:
        pass
    try:
        os.killpg(pid, signal.SIGTERM)
    except OSError:
        pass
    for _ in range(10):
        try:
            wpid, _ = os.waitpid(pid, os.WNOHANG)
            if wpid == pid:
                break
        except ChildProcessError:
            break
        time.sleep(0.02)
    else:
        try:
            os.killpg(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        except OSError:
            pass

raw = b"".join(chunks)
sys.stdout.write(json.dumps({"bytes": base64.b64encode(raw).decode("ascii")}))
"#;

fn python_executable() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("DUI_PTY_PYTHON") {
        let path = PathBuf::from(explicit);
        return path.is_file().then_some(path);
    }
    for candidate in [
        "/usr/bin/python3",
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| PathBuf::from("python3"))
}

fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '"' => [0x0a, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00],
        '#' => [0x0a, 0x1f, 0x0a, 0x0a, 0x1f, 0x0a, 0x00],
        '$' => [0x04, 0x0f, 0x14, 0x0e, 0x05, 0x1e, 0x04],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '&' => [0x08, 0x14, 0x14, 0x08, 0x15, 0x12, 0x0d],
        '\'' => [0x04, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '*' => [0x00, 0x0a, 0x04, 0x1f, 0x04, 0x0a, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x04, 0x04, 0x08],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        ':' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
        ';' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x08],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '=' => [0x00, 0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '?' => [0x0e, 0x11, 0x01, 0x06, 0x04, 0x00, 0x04],
        '@' => [0x0e, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0f],
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1c, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1c],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1b, 0x11],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        '\\' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        ']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        '^' => [0x04, 0x0a, 0x11, 0x00, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        '`' => [0x08, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00],
        'a' => [0x00, 0x00, 0x0e, 0x01, 0x0f, 0x11, 0x0f],
        'b' => [0x10, 0x10, 0x1e, 0x11, 0x11, 0x11, 0x1e],
        'c' => [0x00, 0x00, 0x0e, 0x10, 0x10, 0x11, 0x0e],
        'd' => [0x01, 0x01, 0x0f, 0x11, 0x11, 0x11, 0x0f],
        'e' => [0x00, 0x00, 0x0e, 0x11, 0x1f, 0x10, 0x0e],
        'f' => [0x06, 0x08, 0x1e, 0x08, 0x08, 0x08, 0x08],
        'g' => [0x00, 0x0f, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        'h' => [0x10, 0x10, 0x1e, 0x11, 0x11, 0x11, 0x11],
        'i' => [0x04, 0x00, 0x0c, 0x04, 0x04, 0x04, 0x0e],
        'j' => [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0c],
        'k' => [0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12],
        'l' => [0x0c, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'm' => [0x00, 0x00, 0x1a, 0x15, 0x15, 0x11, 0x11],
        'n' => [0x00, 0x00, 0x1e, 0x11, 0x11, 0x11, 0x11],
        'o' => [0x00, 0x00, 0x0e, 0x11, 0x11, 0x11, 0x0e],
        'p' => [0x00, 0x00, 0x1e, 0x11, 0x1e, 0x10, 0x10],
        'q' => [0x00, 0x00, 0x0f, 0x11, 0x0f, 0x01, 0x01],
        'r' => [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
        's' => [0x00, 0x00, 0x0f, 0x10, 0x0e, 0x01, 0x1e],
        't' => [0x08, 0x08, 0x1e, 0x08, 0x08, 0x09, 0x06],
        'u' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x13, 0x0d],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'w' => [0x00, 0x00, 0x11, 0x11, 0x15, 0x15, 0x0a],
        'x' => [0x00, 0x00, 0x11, 0x0a, 0x04, 0x0a, 0x11],
        'y' => [0x00, 0x11, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        'z' => [0x00, 0x00, 0x1f, 0x02, 0x04, 0x08, 0x1f],
        '{' => [0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '}' => [0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08],
        '~' => [0x00, 0x00, 0x08, 0x15, 0x02, 0x00, 0x00],
        // Box drawing & common TUI symbols
        '─' | '═' | '━' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '│' | '║' | '┃' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '┌' | '╔' | '╭' | '┏' => [0x00, 0x00, 0x00, 0x07, 0x04, 0x04, 0x04],
        '┐' | '╗' | '╮' | '┓' => [0x00, 0x00, 0x00, 0x1c, 0x04, 0x04, 0x04],
        '└' | '╚' | '╰' | '┗' => [0x04, 0x04, 0x04, 0x07, 0x00, 0x00, 0x00],
        '┘' | '╝' | '╯' | '┛' => [0x04, 0x04, 0x04, 0x1c, 0x00, 0x00, 0x00],
        '├' => [0x04, 0x04, 0x04, 0x07, 0x04, 0x04, 0x04],
        '┤' => [0x04, 0x04, 0x04, 0x1c, 0x04, 0x04, 0x04],
        '┬' => [0x00, 0x00, 0x00, 0x1f, 0x04, 0x04, 0x04],
        '┴' => [0x04, 0x04, 0x04, 0x1f, 0x00, 0x00, 0x00],
        '┼' => [0x04, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x04],
        '█' | '▓' => [0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f],
        '▒' | '░' => [0x15, 0x0a, 0x15, 0x0a, 0x15, 0x0a, 0x15],
        '▌' | '▍' => [0x1c, 0x1c, 0x1c, 0x1c, 0x1c, 0x1c, 0x1c],
        '▎' | '▏' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10],
        '▐' => [0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07],
        '▀' => [0x1f, 0x1f, 0x1f, 0x00, 0x00, 0x00, 0x00],
        '▄' => [0x00, 0x00, 0x00, 0x00, 0x1f, 0x1f, 0x1f],
        '•' | '●' => [0x00, 0x00, 0x0e, 0x0e, 0x0e, 0x00, 0x00],
        '▶' => [0x08, 0x0c, 0x0e, 0x0f, 0x0e, 0x0c, 0x08],
        _ => [0x1f, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1f],
    }
}

pub(crate) fn render_grid_to_png(
    grid: &TerminalGrid,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let img_w = (PAD_X * 2 + (grid.cols() as i32) * CELL_W) as usize;
    let img_h = (PAD_Y * 2 + (grid.rows() as i32) * CELL_H) as usize;
    let mut pixels = vec![0u8; img_w * img_h * 4];

    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = DEFAULT_BG[0];
        chunk[1] = DEFAULT_BG[1];
        chunk[2] = DEFAULT_BG[2];
        chunk[3] = 255;
    }

    let mut set_px = |x: usize, y: usize, rgb: [u8; 3]| {
        if x < img_w && y < img_h {
            let idx = (y * img_w + x) * 4;
            pixels[idx] = rgb[0];
            pixels[idx + 1] = rgb[1];
            pixels[idx + 2] = rgb[2];
            pixels[idx + 3] = 255;
        }
    };

    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            let cell = grid.cell(row, col);
            let (mut fg, bg) = if cell.inverse {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };
            if cell.dim {
                fg = [fg[0] / 2, fg[1] / 2, fg[2] / 2];
            }
            let base_x = (PAD_X + (col as i32) * CELL_W) as usize;
            let base_y = (PAD_Y + (row as i32) * CELL_H) as usize;
            if bg != DEFAULT_BG {
                for dy in 0..(CELL_H as usize) {
                    for dx in 0..(CELL_W as usize) {
                        set_px(base_x + dx, base_y + dy, bg);
                    }
                }
            }
            if cell.ch != ' ' {
                let bitmap = glyph_5x7(cell.ch);
                for (gy, bits) in bitmap.iter().enumerate() {
                    for gx in 0..5usize {
                        if (bits & (1 << (4 - gx))) != 0 {
                            let px = base_x + 1 + gx * 2;
                            let py = base_y + 3 + gy * 2;
                            set_px(px, py, fg);
                            set_px(px + 1, py, fg);
                            set_px(px, py + 1, fg);
                            set_px(px + 1, py + 1, fg);
                            if cell.bold {
                                set_px(px + 2, py, fg);
                                set_px(px + 2, py + 1, fg);
                            }
                        }
                    }
                }
            }
            if cell.underline {
                let uy = base_y + (CELL_H as usize) - 2;
                for dx in 0..(CELL_W as usize) {
                    set_px(base_x + dx, uy, fg);
                }
            }
        }
    }

    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, img_w as u32, img_h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(&pixels)?;
    Ok(())
}

fn collect_tui_source_files(project: &Path) -> Vec<String> {
    let Ok(project_root) = project.canonicalize() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut stack = vec![project_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for path in children {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || matches!(name, "node_modules" | "target" | "dist") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file()
                && let Ok(rel) = path.strip_prefix(&project_root)
            {
                files.push(rel.to_string_lossy().replace('\\', "/"));
                if files.len() >= MAX_TUI_CACHE_FILES {
                    files.sort();
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

struct TuiOracle;

impl TypeOracle for TuiOracle {
    fn facts(&self, project: &Path, spec: &Path) -> Reply {
        let mut facts = NoopOracle.facts(project, spec)?;
        facts["host"] = json!("pty");
        facts["sourceFiles"] = json!(collect_tui_source_files(project));
        Ok(facts)
    }
}

struct PtyRenderer<'a> {
    project: &'a Path,
    python: PathBuf,
}

impl PtyRenderer<'_> {
    fn resolve_command(spec: &Value) -> Result<Vec<String>, String> {
        match spec.get("command") {
            Some(Value::Array(items)) if !items.is_empty() => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "TUI command array entries must be strings".to_owned())
                })
                .collect(),
            Some(Value::String(script)) if !script.trim().is_empty() => {
                Ok(vec!["/bin/sh".into(), "-c".into(), script.clone()])
            }
            _ => Err("host \"pty\" requires a non-empty `command` string or string array".into()),
        }
    }

    fn resolve_rows(spec: &Value, cols: u32) -> usize {
        if let Some(sizes) = spec["terminalSizes"].as_array() {
            for entry in sizes {
                if entry["cols"].as_u64() == Some(u64::from(cols))
                    && let Some(r) = entry["rows"].as_u64()
                    && r > 0
                {
                    return r as usize;
                }
            }
        }
        spec["rows"].as_u64().filter(|r| *r > 0).unwrap_or(24) as usize
    }
}

impl Renderer for PtyRenderer<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        _input: &Value,
        width: u32,
        image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let command = Self::resolve_command(spec)?;
        let cols = width.max(1) as usize;
        let rows = Self::resolve_rows(spec, width);
        let keys: Vec<String> = spec["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|tok| STANDARD.encode(encode_key_token(tok)))
            .collect();
        let mut env_map = serde_json::Map::new();
        if let Some(envs) = spec["env"].as_object() {
            for (k, v) in envs {
                if let Some(val) = v.as_str() {
                    env_map.insert(k.clone(), json!(val));
                }
            }
        }
        let payload = json!({
            "command": command,
            "cwd": self.project.to_string_lossy(),
            "cols": cols,
            "rows": rows,
            "keys": keys,
            "settleMs": spec["settleMs"].as_u64().unwrap_or(150),
            "timeoutMs": spec["timeoutMs"].as_u64().unwrap_or(4000),
            "env": env_map,
        });
        let mut child = Command::new(&self.python)
            .args(["-c", PTY_HELPER_PY])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(payload.to_string().as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("PTY helper failed: {stderr}").into());
        }
        let parsed: Value = serde_json::from_slice(&output.stdout)?;
        let raw_b64 = parsed["bytes"]
            .as_str()
            .ok_or("PTY helper returned no bytes")?;
        let raw_bytes = STANDARD.decode(raw_b64)?;

        let mut grid = TerminalGrid::new(cols, rows);
        grid.feed(&raw_bytes);
        render_grid_to_png(&grid, image)?;
        let render_tree = grid.to_render_tree(CELL_W, CELL_H, PAD_X, PAD_Y);
        let path = spec["path"].as_str().unwrap_or("/");
        Ok(json!({
            "ready": true,
            "settled": true,
            "path": path,
            "width": width,
            "slots": {},
            "hits": [],
            "links": [],
            "responses": {},
            "responseChecks": {},
            "assertions": [],
            "text": grid.plain_text(),
            "renderTree": render_tree,
            "terminal": {
                "cols": cols,
                "rows": rows,
                "cellWidth": CELL_W,
                "cellHeight": CELL_H
            }
        }))
    }
}

pub(crate) struct UniversalTuiSurface;

impl SurfaceAdapter for UniversalTuiSurface {
    fn descriptor(&self) -> SurfaceDescriptor {
        SurfaceDescriptor {
            id: SURFACE_UNIVERSAL_TUI.into(),
            kind: SurfaceKind::Tui,
            tier: SupportTier::Universal,
            host_kinds: vec!["pty".into()],
            capabilities: SurfaceCapabilities {
                state_injection: false,
                live_edit: false,
                link_discovery: false,
                render_tree: true,
                box_model: true,
                pixel_capture: true,
            },
        }
    }

    fn oracle(&self) -> &dyn TypeOracle {
        &TuiOracle
    }

    fn check_env(&self) -> Result<(), UnrenderableReason> {
        if python_executable().is_some() {
            Ok(())
        } else {
            Err(UnrenderableReason::new(
                "no-pty-env",
                "python3 PTY runtime not available (set DUI_PTY_PYTHON to override)",
            ))
        }
    }

    fn run(&self, project: &Path, spec: &Path, out: &Path, force: bool) -> Reply {
        let Some(python) = python_executable() else {
            return Err(failure(3, "python3 PTY runtime not available"));
        };
        let facts = analysis::analyze_with_oracle(self.oracle(), project, spec)?;
        let mut renderer = PtyRenderer { project, python };
        let runtime_tag = Some(json!({"product": "dui-pty-vt100"}));
        analysis::exercise_with_renderer(
            Some(&mut renderer),
            facts,
            spec,
            Some(project),
            out,
            runtime_tag,
            force,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_terminal_grid_to_valid_png_with_expected_dimensions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("tui.png");
        let mut grid = TerminalGrid::new(40, 10);
        grid.feed(
            "\x1b[2J\x1b[1;1H\x1b[1;36m┌────────────────────┐\x1b[0m\r\n│  dui TUI Preview   │"
                .as_bytes(),
        );
        render_grid_to_png(&grid, &png_path).expect("render png");

        let decoder = png::Decoder::new(std::io::BufReader::new(
            File::open(&png_path).expect("open png"),
        ));
        let reader = decoder.read_info().expect("read png info");
        let info = reader.info();
        assert_eq!(info.width, (PAD_X * 2 + 40 * CELL_W) as u32);
        assert_eq!(info.height, (PAD_Y * 2 + 10 * CELL_H) as u32);
    }
}
